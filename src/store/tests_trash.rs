//! Store tests for trash lifecycle, aggregates and single-row mutations.

use super::tests_faxes::{at, date, fax, query, search_ids, tags};
use super::*;
use crate::model::{FaxId, Folder, NewFax};

#[test]
fn trash_restore_and_delete_permanently_cascade_tags() {
    let store = Store::open_in_memory().expect("open");
    let id = store.insert_fax(&fax("tr", at(3, 1, 9))).expect("insert");
    store.set_tags(id, &tags(&["x", "y"])).expect("tags");

    store.move_to_trash(id, at(3, 5, 10)).expect("trash");
    assert!(search_ids(&store, &query(Folder::All)).is_empty());
    assert_eq!(search_ids(&store, &query(Folder::Trash)), vec![id]);
    let trashed = store.trashed().expect("trashed");
    assert_eq!(trashed.len(), 1);
    assert_eq!(trashed[0].tags, tags(&["x", "y"]));
    assert_eq!(trashed[0].deleted_at, Some(at(3, 5, 10)));

    // Trashing again keeps the original deletion time.
    store.move_to_trash(id, at(3, 9, 10)).expect("trash again");
    assert_eq!(
        store.trashed().expect("trashed")[0].deleted_at,
        Some(at(3, 5, 10))
    );

    store.restore(id).expect("restore");
    assert_eq!(search_ids(&store, &query(Folder::All)), vec![id]);
    assert!(store.trashed().expect("trashed").is_empty());

    let removed = store.delete_permanently(id).expect("delete");
    assert_eq!(removed.id, id);
    assert_eq!(removed.tags, tags(&["x", "y"]));
    assert!(store.get_fax(id).expect("get").is_none());
    let tag_rows: i64 = store
        .conn
        .query_row("SELECT COUNT(*) FROM fax_tags", [], |row| row.get(0))
        .expect("tag rows");
    assert_eq!(tag_rows, 0, "ON DELETE CASCADE removes tags");

    assert!(matches!(
        store.delete_permanently(id),
        Err(StoreError::NotFound)
    ));
}

#[test]
fn trashed_before_uses_strict_cutoff_on_instants() {
    let store = Store::open_in_memory().expect("open");
    let early = store.insert_fax(&fax("e1", at(3, 1, 9))).expect("early");
    let late = store.insert_fax(&fax("e2", at(3, 2, 9))).expect("late");
    store
        .move_to_trash(early, at(3, 1, 12))
        .expect("trash early");
    store
        .move_to_trash(late, at(3, 10, 12))
        .expect("trash late");

    let old = store.trashed_before(at(3, 5, 0)).expect("before");
    assert_eq!(old.iter().map(|r| r.id).collect::<Vec<_>>(), vec![early]);

    let none = store.trashed_before(at(3, 1, 12)).expect("boundary");
    assert!(
        none.is_empty(),
        "deleted_at must be strictly before the cutoff"
    );
}

/// Fixture for exact aggregate assertions. Non-deleted: c1, c2, c3, c5; trashed: c4.
fn seed_aggregate_fixture(store: &Store) {
    let c1 = store
        .insert_fax(&NewFax {
            sender_name: Some("Alice".to_string()),
            pages: 2,
            ..fax("c1", at(3, 1, 9))
        })
        .expect("c1");
    store
        .set_tags(c1, &tags(&["finance", "urgent"]))
        .expect("c1 tags");

    let c2 = store
        .insert_fax(&NewFax {
            sender_name: None,
            sender_number: Some("0223456789".to_string()),
            pages: 3,
            ..fax("c2", at(3, 1, 15))
        })
        .expect("c2");
    store.set_tags(c2, &tags(&["finance"])).expect("c2 tags");
    store.set_read(c2, true).expect("c2 read");
    store.set_starred(c2, true).expect("c2 star");

    let c3 = store
        .insert_fax(&NewFax {
            sender_name: Some("Alice ".to_string()),
            pages: 1,
            ..fax("c3", at(3, 2, 9))
        })
        .expect("c3");
    store.set_read(c3, true).expect("c3 read");

    let c4 = store
        .insert_fax(&NewFax {
            sender_name: None,
            sender_number: None,
            pages: 4,
            ..fax("c4", at(3, 5, 9))
        })
        .expect("c4");
    store.move_to_trash(c4, at(3, 6, 9)).expect("c4 trash");

    let c5 = store
        .insert_fax(&NewFax {
            sender_name: Some("Bob".to_string()),
            pages: 5,
            ..fax("c5", at(3, 9, 9))
        })
        .expect("c5");
    store.set_tags(c5, &tags(&["urgent"])).expect("c5 tags");
}

#[test]
fn folder_counts_buckets_and_tag_counts_are_exact() {
    let store = Store::open_in_memory().expect("open");
    seed_aggregate_fixture(&store);

    assert_eq!(
        store.folder_counts().expect("counts"),
        FolderCounts {
            all: 4,
            unread: 2,
            starred: 1,
            trash: 1
        }
    );
    assert_eq!(
        store.date_buckets().expect("buckets"),
        vec![
            DateBucket {
                date: date(3, 1),
                count: 2
            },
            DateBucket {
                date: date(3, 2),
                count: 1
            },
            DateBucket {
                date: date(3, 9),
                count: 1
            },
        ]
    );
    assert_eq!(
        store.tag_counts().expect("tags"),
        vec![("finance".to_string(), 2), ("urgent".to_string(), 2)]
    );
}

#[test]
fn stats_totals_days_and_top_senders_are_exact() {
    let store = Store::open_in_memory().expect("open");
    seed_aggregate_fixture(&store);

    let stats = store.stats(date(3, 1), date(3, 9)).expect("stats");
    assert_eq!(stats.total_faxes, 4);
    assert_eq!(stats.total_pages, 2 + 3 + 1 + 5);
    assert_eq!(
        stats.per_day,
        vec![
            DateBucket {
                date: date(3, 1),
                count: 2
            },
            DateBucket {
                date: date(3, 2),
                count: 1
            },
            DateBucket {
                date: date(3, 9),
                count: 1
            },
        ]
    );
    // "Alice " is trimmed on insert, so both Alice rows merge.
    assert_eq!(
        stats.top_senders,
        vec![
            ("Alice".to_string(), 2),
            ("0223456789".to_string(), 1),
            ("Bob".to_string(), 1),
        ]
    );

    let narrow = store.stats(date(3, 2), date(3, 2)).expect("narrow");
    assert_eq!(narrow.total_faxes, 1);
    assert_eq!(narrow.total_pages, 1);
    assert_eq!(narrow.top_senders, vec![("Alice".to_string(), 1)]);
    assert_eq!(narrow.per_day.len(), 1);
}

#[test]
fn stats_top_senders_capped_at_ten() {
    let store = Store::open_in_memory().expect("open");
    for n in 0..12u32 {
        store
            .insert_fax(&NewFax {
                sender_name: Some(format!("sender{n:02}")),
                ..fax(&format!("top{n}"), at(3, 1, 9))
            })
            .expect("insert");
    }
    let stats = store.stats(date(3, 1), date(3, 1)).expect("stats");
    assert_eq!(stats.top_senders.len(), 10);
    assert_eq!(stats.top_senders[0].0, "sender00");
}

#[test]
fn mark_all_read_counts_only_unread_live_faxes() {
    let store = Store::open_in_memory().expect("open");
    seed_aggregate_fixture(&store);

    assert_eq!(store.mark_all_read().expect("first"), 2);
    assert_eq!(store.mark_all_read().expect("second"), 0);
    let counts = store.folder_counts().expect("counts");
    assert_eq!(counts.unread, 0);
    assert_eq!(counts.trash, 1);
}

#[test]
fn fill_sender_names_only_touches_null_names() {
    let store = Store::open_in_memory().expect("open");
    let f1 = store.insert_fax(&fax("n1", at(3, 1, 9))).expect("f1");
    let f2 = store
        .insert_fax(&NewFax {
            sender_name: Some("Existing".to_string()),
            ..fax("n2", at(3, 2, 9))
        })
        .expect("f2");
    let f3 = store
        .insert_fax(&NewFax {
            sender_number: Some("+15550009".to_string()),
            ..fax("n3", at(3, 3, 9))
        })
        .expect("f3");

    assert_eq!(
        store.fill_sender_names("0223456789", "   ").expect("blank"),
        0
    );
    // f2 already has a name, f3 has another number: only f1 qualifies.
    assert_eq!(
        store.fill_sender_names("0223456789", "Acme").expect("fill"),
        1
    );
    assert_eq!(
        store
            .get_fax(f1)
            .expect("get")
            .expect("f1")
            .sender_name
            .as_deref(),
        Some("Acme")
    );
    assert_eq!(
        store
            .get_fax(f2)
            .expect("get")
            .expect("f2")
            .sender_name
            .as_deref(),
        Some("Existing")
    );
    assert!(
        store
            .get_fax(f3)
            .expect("get")
            .expect("f3")
            .sender_name
            .is_none()
    );
}

#[test]
fn set_sender_name_blank_stores_null() {
    let store = Store::open_in_memory().expect("open");
    let id = store.insert_fax(&fax("sn", at(3, 1, 9))).expect("insert");
    store.set_sender_name(id, Some("Acme")).expect("set");
    assert_eq!(
        store
            .get_fax(id)
            .expect("get")
            .expect("exists")
            .sender_name
            .as_deref(),
        Some("Acme")
    );
    store.set_sender_name(id, Some("  ")).expect("blank");
    assert!(
        store
            .get_fax(id)
            .expect("get")
            .expect("exists")
            .sender_name
            .is_none()
    );
}

#[test]
fn mutations_on_missing_id_return_not_found() {
    let store = Store::open_in_memory().expect("open");
    let missing: FaxId = 4242;
    let results = [
        store.set_read(missing, true),
        store.set_starred(missing, true),
        store.set_tags(missing, &tags(&["a"])),
        store.set_note(missing, "x"),
        store.set_sender_name(missing, Some("x")),
        store.set_file_path(missing, std::path::Path::new("a.pdf")),
        store.move_to_trash(missing, at(3, 1, 9)),
        store.restore(missing),
    ];
    for result in results {
        assert!(matches!(result, Err(StoreError::NotFound)), "{result:?}");
    }
    assert!(matches!(
        store.delete_permanently(missing),
        Err(StoreError::NotFound)
    ));
    let tag_rows: i64 = store
        .conn
        .query_row("SELECT COUNT(*) FROM fax_tags", [], |row| row.get(0))
        .expect("tag rows");
    assert_eq!(tag_rows, 0, "failed set_tags must not leave orphans");
}
