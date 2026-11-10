//! Store tests for fax rows, search, trash and aggregates.

use super::*;
use crate::model::{FaxId, FaxQuery, FaxSource, Folder, NewFax, SearchPage, SortOrder};
use chrono::{Local, NaiveDate, TimeZone};
use std::path::PathBuf;

pub(super) fn at(month: u32, day: u32, hour: u32) -> DateTime<Local> {
    Local
        .with_ymd_and_hms(2026, month, day, hour, 0, 0)
        .single()
        .expect("local time exists")
}

pub(super) fn date(month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, month, day).expect("valid date")
}

pub(super) fn fax(sha: &str, when: DateTime<Local>) -> NewFax {
    NewFax {
        received_at: when,
        sender_number: Some("0223456789".to_string()),
        sender_name: None,
        pages: 2,
        file_path: PathBuf::from(format!("archive/{sha}.pdf")),
        file_size: 1024,
        sha256: sha.to_string(),
        original_name: format!("{sha}.pdf"),
        source: FaxSource::WatchFolder,
    }
}

pub(super) fn query(folder: Folder) -> FaxQuery {
    FaxQuery {
        folder,
        page_size: 100,
        ..Default::default()
    }
}

fn ids(page: &SearchPage) -> Vec<FaxId> {
    page.items.iter().map(|record| record.id).collect()
}

pub(super) fn search_ids(store: &Store, query: &FaxQuery) -> Vec<FaxId> {
    ids(&store.search(query).expect("search"))
}

pub(super) fn tags(list: &[&str]) -> Vec<String> {
    list.iter().map(|tag| tag.to_string()).collect()
}

#[test]
fn schema_persists_across_reopen_with_wal_and_user_version_1() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("nested").join("index.db");
    let id = {
        let store = Store::open(&path).expect("open creates parent dirs");
        let mode: String = store
            .conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("journal mode");
        assert_eq!(mode, "wal");
        store.insert_fax(&fax("aa", at(3, 1, 12))).expect("insert")
    };

    let store = Store::open(&path).expect("reopen");
    let version: i64 = store
        .conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user_version");
    assert_eq!(version, 1);
    let record = store
        .get_fax(id)
        .expect("get")
        .expect("row survives reopen");
    assert_eq!(record.sha256, "aa");
}

#[test]
fn schema_declares_expected_indexes() {
    let store = Store::open_in_memory().expect("open");
    for index in [
        "idx_faxes_received_date",
        "idx_faxes_received_ts",
        "idx_faxes_sender_number",
        "idx_faxes_deleted_at",
    ] {
        let count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                [index],
                |row| row.get(0),
            )
            .expect("index lookup");
        assert_eq!(count, 1, "missing index {index}");
    }
}

#[test]
fn insert_and_get_roundtrip_with_chinese_text_and_tags() {
    let store = Store::open_in_memory().expect("open");
    let when = at(3, 1, 12);
    let new = NewFax {
        sender_name: Some("陳經理".to_string()),
        sender_number: Some("+886221234567".to_string()),
        original_name: "傳真-第一頁.pdf".to_string(),
        ..fax("bb", when)
    };
    let id = store.insert_fax(&new).expect("insert");
    store
        .set_tags(id, &tags(&["Invoice", " urgent ", "invoice"]))
        .expect("tags");
    store.set_note(id, "請回電 ＆ 確認").expect("note");

    let record = store.get_fax(id).expect("get").expect("exists");
    assert_eq!(record.received_at, when);
    assert_eq!(record.sender_name.as_deref(), Some("陳經理"));
    assert_eq!(record.sender_number.as_deref(), Some("+886221234567"));
    assert_eq!(record.original_name, "傳真-第一頁.pdf");
    assert_eq!(record.tags, tags(&["invoice", "urgent"]));
    assert_eq!(record.note, "請回電 ＆ 確認");
    assert_eq!(record.pages, 2);
    assert_eq!(record.file_size, 1024);
    assert_eq!(record.file_path, PathBuf::from("archive/bb.pdf"));
    assert_eq!(record.source, FaxSource::WatchFolder);
    assert!(!record.is_read);
    assert!(!record.is_starred);
    assert!(record.deleted_at.is_none());
    assert!(store.get_fax(999).expect("get").is_none());
}

#[test]
fn duplicate_sha256_is_rejected_and_find_by_hash_works() {
    let store = Store::open_in_memory().expect("open");
    let first = store.insert_fax(&fax("cc", at(3, 1, 9))).expect("insert");

    let err = store
        .insert_fax(&fax("cc", at(3, 2, 9)))
        .expect_err("duplicate hash must fail");
    assert!(matches!(err, StoreError::Sqlite(_)));

    assert_eq!(store.find_by_hash("cc").expect("find"), Some(first));
    assert_eq!(store.find_by_hash("zz").expect("find"), None);
}

#[test]
fn folder_views_split_all_unread_starred_and_trash() {
    let store = Store::open_in_memory().expect("open");
    let f1 = store.insert_fax(&fax("a1", at(3, 1, 9))).expect("f1");
    let f2 = store.insert_fax(&fax("a2", at(3, 2, 9))).expect("f2");
    let f3 = store.insert_fax(&fax("a3", at(3, 3, 9))).expect("f3");
    let f4 = store.insert_fax(&fax("a4", at(3, 4, 9))).expect("f4");
    store.set_read(f2, true).expect("read");
    store.set_starred(f2, true).expect("star");
    store.set_read(f4, true).expect("read");
    store.move_to_trash(f3, at(3, 5, 9)).expect("trash");

    assert_eq!(search_ids(&store, &query(Folder::All)), vec![f4, f2, f1]);
    assert_eq!(search_ids(&store, &query(Folder::Unread)), vec![f1]);
    assert_eq!(search_ids(&store, &query(Folder::Starred)), vec![f2]);
    assert_eq!(search_ids(&store, &query(Folder::Trash)), vec![f3]);

    let counts = store.folder_counts().expect("counts");
    assert_eq!(
        counts,
        FolderCounts {
            all: 3,
            unread: 1,
            starred: 1,
            trash: 1
        }
    );
}

/// Seeds the escaping/text fixture and returns (alice, bob, chen, backslash) ids.
fn seed_text_fixture(store: &Store) -> (FaxId, FaxId, FaxId, FaxId) {
    let alice = store
        .insert_fax(&NewFax {
            sender_name: Some("Alice Wang".to_string()),
            sender_number: Some("+15550001".to_string()),
            original_name: "invoice_2026.pdf".to_string(),
            ..fax("t1", at(3, 1, 9))
        })
        .expect("alice");
    store.set_note(alice, "請款").expect("note");
    store.set_tags(alice, &tags(&["finance"])).expect("tags");

    let bob = store
        .insert_fax(&NewFax {
            sender_name: Some("Bob".to_string()),
            original_name: "file_a.pdf".to_string(),
            ..fax("t2", at(3, 2, 9))
        })
        .expect("bob");
    store.set_note(bob, "100%_done").expect("note");

    let chen = store
        .insert_fax(&NewFax {
            sender_name: Some("陳經理".to_string()),
            sender_number: None,
            original_name: "scan.pdf".to_string(),
            ..fax("t3", at(3, 3, 9))
        })
        .expect("chen");
    store.set_note(chen, "傳真內容 ACME").expect("note");

    let slash = store
        .insert_fax(&NewFax {
            sender_name: None,
            sender_number: Some("+15550002".to_string()),
            original_name: "x.pdf".to_string(),
            ..fax("t4", at(3, 4, 9))
        })
        .expect("slash");
    store.set_note(slash, "back\\slash").expect("note");

    (alice, bob, chen, slash)
}

#[test]
fn text_search_is_case_insensitive_and_and_joins_words() {
    let store = Store::open_in_memory().expect("open");
    let (alice, _bob, _chen, _slash) = seed_text_fixture(&store);

    let mut q = query(Folder::All);
    q.text = "alice wang".to_string();
    assert_eq!(search_ids(&store, &q), vec![alice]);

    q.text = "ALICE".to_string();
    assert_eq!(search_ids(&store, &q), vec![alice]);

    q.text = "alice bob".to_string();
    assert!(search_ids(&store, &q).is_empty());

    q.text = "invoice 2026".to_string();
    assert_eq!(search_ids(&store, &q), vec![alice]);

    q.text = "invoice nomatch".to_string();
    assert!(search_ids(&store, &q).is_empty());

    q.text = "5550001".to_string();
    assert_eq!(search_ids(&store, &q), vec![alice]);

    q.text = "finance".to_string();
    assert_eq!(search_ids(&store, &q), vec![alice], "tags are searched");
}

#[test]
fn text_search_treats_like_wildcards_and_backslash_literally() {
    let store = Store::open_in_memory().expect("open");
    let (alice, bob, _chen, slash) = seed_text_fixture(&store);

    let mut q = query(Folder::All);
    q.text = "%".to_string();
    assert_eq!(
        search_ids(&store, &q),
        vec![bob],
        "% must not match everything"
    );

    // "_" is literal: it appears in bob's note/file name and alice's "invoice_2026.pdf",
    // but would match every single-character position if left unescaped.
    q.text = "_".to_string();
    assert_eq!(
        search_ids(&store, &q),
        vec![bob, alice],
        "_ must not match any char"
    );

    q.text = "%_".to_string();
    assert_eq!(search_ids(&store, &q), vec![bob]);

    q.text = "\\".to_string();
    assert_eq!(search_ids(&store, &q), vec![slash]);
}

#[test]
fn text_search_matches_cjk_substrings() {
    let store = Store::open_in_memory().expect("open");
    let (_alice, _bob, chen, _slash) = seed_text_fixture(&store);

    let mut q = query(Folder::All);
    q.text = "傳真".to_string();
    assert_eq!(search_ids(&store, &q), vec![chen]);

    q.text = "經理".to_string();
    assert_eq!(search_ids(&store, &q), vec![chen]);

    q.text = "acme".to_string();
    assert_eq!(search_ids(&store, &q), vec![chen]);
}

#[test]
fn date_range_bounds_are_inclusive_on_local_date() {
    let store = Store::open_in_memory().expect("open");
    let before = store.insert_fax(&fax("d1", at(2, 28, 12))).expect("d1");
    let first = store.insert_fax(&fax("d2", at(3, 1, 0))).expect("d2");
    let mid = store.insert_fax(&fax("d3", at(3, 15, 12))).expect("d3");
    let last = store.insert_fax(&fax("d4", at(3, 31, 23))).expect("d4");
    let after = store.insert_fax(&fax("d5", at(4, 1, 0))).expect("d5");

    let mut q = query(Folder::All);
    q.date_from = Some(date(3, 1));
    q.date_to = Some(date(3, 31));
    q.sort = SortOrder::OldestFirst;
    let found = search_ids(&store, &q);
    assert_eq!(found, vec![first, mid, last]);
    assert!(!found.contains(&before) && !found.contains(&after));

    q.date_to = None;
    assert_eq!(search_ids(&store, &q), vec![first, mid, last, after]);
}

#[test]
fn tag_filter_is_exact_and_normalised() {
    let store = Store::open_in_memory().expect("open");
    let a = store.insert_fax(&fax("g1", at(3, 1, 9))).expect("a");
    let b = store.insert_fax(&fax("g2", at(3, 2, 9))).expect("b");
    let c = store.insert_fax(&fax("g3", at(3, 3, 9))).expect("c");
    store.set_tags(a, &tags(&["finance"])).expect("tag a");
    store
        .set_tags(b, &tags(&["finance", "urgent"]))
        .expect("tag b");
    store.set_tags(c, &tags(&["fin"])).expect("tag c");

    let mut q = query(Folder::All);
    q.tag = Some("Finance".to_string());
    assert_eq!(search_ids(&store, &q), vec![b, a]);

    q.tag = Some("fin".to_string());
    assert_eq!(search_ids(&store, &q), vec![c]);
}

/// Seven faxes on consecutive days with varied senders (ids 1..=4 in insert order).
fn seed_sort_fixture(store: &Store) -> [FaxId; 4] {
    let s1 = store
        .insert_fax(&NewFax {
            sender_name: Some("carol".to_string()),
            ..fax("s1", at(3, 1, 9))
        })
        .expect("s1");
    let s2 = store
        .insert_fax(&NewFax {
            sender_name: None,
            sender_number: Some("0223456789".to_string()),
            ..fax("s2", at(3, 2, 9))
        })
        .expect("s2");
    let s3 = store
        .insert_fax(&NewFax {
            sender_name: Some("Alice".to_string()),
            ..fax("s3", at(3, 3, 9))
        })
        .expect("s3");
    let s4 = store
        .insert_fax(&NewFax {
            sender_name: Some("bob".to_string()),
            ..fax("s4", at(3, 4, 9))
        })
        .expect("s4");
    [s1, s2, s3, s4]
}

#[test]
fn sort_orders_newest_oldest_and_sender() {
    let store = Store::open_in_memory().expect("open");
    let [s1, s2, s3, s4] = seed_sort_fixture(&store);

    let mut q = query(Folder::All);
    q.sort = SortOrder::NewestFirst;
    assert_eq!(search_ids(&store, &q), vec![s4, s3, s2, s1]);

    q.sort = SortOrder::OldestFirst;
    assert_eq!(search_ids(&store, &q), vec![s1, s2, s3, s4]);

    // Sender key: name, else number; case-insensitive. "0223..." < "Alice" < "bob" < "carol".
    q.sort = SortOrder::SenderAsc;
    assert_eq!(search_ids(&store, &q), vec![s2, s3, s4, s1]);
}

#[test]
fn paging_reports_total_and_respects_page_size() {
    let store = Store::open_in_memory().expect("open");
    let mut all = Vec::new();
    for day in 1..=7 {
        all.push(
            store
                .insert_fax(&fax(&format!("p{day}"), at(3, day, 9)))
                .expect("insert"),
        );
    }
    all.reverse(); // newest first

    let mut q = query(Folder::All);
    q.page_size = 3;
    let page0 = store.search(&q).expect("page 0");
    assert_eq!(page0.total, 7);
    assert_eq!(ids(&page0), all[0..3]);

    q.page = 1;
    assert_eq!(ids(&store.search(&q).expect("page 1")), all[3..6]);

    q.page = 2;
    let page2 = store.search(&q).expect("page 2");
    assert_eq!(ids(&page2), all[6..7]);
    assert_eq!(page2.total, 7);

    q.page = 3;
    let empty = store.search(&q).expect("page 3");
    assert!(empty.items.is_empty());
    assert_eq!(empty.total, 7);

    q.page = 0;
    q.page_size = 0;
    let defaulted = store.search(&q).expect("page_size 0");
    assert_eq!(defaulted.items.len(), 7, "0 falls back to 50 per page");
}

#[test]
fn search_all_ignores_paging() {
    let store = Store::open_in_memory().expect("open");
    for day in 1..=5 {
        store
            .insert_fax(&fax(&format!("q{day}"), at(3, day, 9)))
            .expect("insert");
    }
    let mut q = query(Folder::All);
    q.page_size = 2;
    q.page = 1;
    let everything = store.search_all(&q).expect("search_all");
    assert_eq!(everything.len(), 5);
}
