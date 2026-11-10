//! Store tests for contacts, the sent log and the activity log.

use super::*;
use crate::model::{Contact, LogLevel};
use std::path::Path;

fn contact(name: &str) -> Contact {
    Contact {
        id: 0,
        name: name.to_string(),
        ..Default::default()
    }
}

fn names(list: &[Contact]) -> Vec<&str> {
    list.iter().map(|c| c.name.as_str()).collect()
}

#[test]
fn contact_insert_trims_and_normalises_fax_number() {
    let store = Store::open_in_memory().expect("open");
    let id = store
        .upsert_contact(&Contact {
            id: 0,
            name: "  王小明 ".to_string(),
            company: " 甲公司 ".to_string(),
            fax_number: "+886 (2) 2345-6789".to_string(),
            phone: " 02 ".to_string(),
            email: " a@example.com ".to_string(),
            note: " 備註 ".to_string(),
        })
        .expect("insert");
    assert!(id > 0);

    let saved = store.get_contact(id).expect("get").expect("exists");
    assert_eq!(saved.id, id);
    assert_eq!(saved.name, "王小明");
    assert_eq!(saved.company, "甲公司");
    assert_eq!(saved.fax_number, "+886223456789");
    assert_eq!(saved.phone, "02");
    assert_eq!(saved.email, "a@example.com");
    assert_eq!(saved.note, "備註");
}

#[test]
fn contact_update_keeps_id_and_missing_id_is_not_found() {
    let store = Store::open_in_memory().expect("open");
    let id = store.upsert_contact(&contact("Old")).expect("insert");

    let updated = store
        .upsert_contact(&Contact {
            id,
            name: "New".to_string(),
            fax_number: String::new(),
            ..Default::default()
        })
        .expect("update");
    assert_eq!(updated, id);
    let saved = store.get_contact(id).expect("get").expect("exists");
    assert_eq!(saved.name, "New");
    assert_eq!(saved.fax_number, "");

    let missing = Contact {
        id: 9999,
        ..contact("Ghost")
    };
    assert!(matches!(
        store.upsert_contact(&missing),
        Err(StoreError::NotFound)
    ));
}

#[test]
fn contact_requires_non_empty_name() {
    let store = Store::open_in_memory().expect("open");
    let err = store
        .upsert_contact(&contact("   "))
        .expect_err("blank name rejected");
    assert!(matches!(err, StoreError::Invalid(ref field) if field == "name"));
    assert!(store.list_contacts("").expect("list").is_empty());
}

#[test]
fn contact_list_sorts_case_insensitively_and_filters_substrings() {
    let store = Store::open_in_memory().expect("open");
    store
        .upsert_contact(&Contact {
            company: "Acme Ltd".to_string(),
            ..contact("Alice Wang")
        })
        .expect("alice");
    store
        .upsert_contact(&Contact {
            email: "bob@acme.test".to_string(),
            ..contact("bob")
        })
        .expect("bob");
    store
        .upsert_contact(&Contact {
            fax_number: "0223456789".to_string(),
            ..contact("陳小美")
        })
        .expect("chen");
    store.upsert_contact(&contact("Charlie")).expect("charlie");

    let all = store.list_contacts("").expect("all");
    assert_eq!(names(&all), vec!["Alice Wang", "bob", "Charlie", "陳小美"]);
    assert_eq!(store.list_contacts("   ").expect("blank filter").len(), 4);

    let by_company_and_email = store.list_contacts("ACME").expect("acme");
    assert_eq!(names(&by_company_and_email), vec!["Alice Wang", "bob"]);

    let by_number = store.list_contacts("0223").expect("number");
    assert_eq!(names(&by_number), vec!["陳小美"]);

    assert_eq!(
        names(&store.list_contacts("小美").expect("cjk")),
        vec!["陳小美"]
    );
    assert!(store.list_contacts("%").expect("percent").is_empty());
    assert!(store.list_contacts("_").expect("underscore").is_empty());
}

#[test]
fn contact_by_number_normalises_input() {
    let store = Store::open_in_memory().expect("open");
    let id = store
        .upsert_contact(&Contact {
            fax_number: "+886 2 2345 6789".to_string(),
            ..contact("Printer Shop")
        })
        .expect("insert");

    let hit = store
        .contact_by_number(" +886-2-2345-6789 ")
        .expect("lookup")
        .expect("found");
    assert_eq!(hit.id, id);

    assert!(store.contact_by_number("0223456789").expect("no").is_none());
    assert!(store.contact_by_number("").expect("empty").is_none());
    assert!(store.contact_by_number("ext").expect("letters").is_none());
}

#[test]
fn contact_delete_then_missing_is_not_found() {
    let store = Store::open_in_memory().expect("open");
    let id = store.upsert_contact(&contact("Temp")).expect("insert");
    store.delete_contact(id).expect("delete");
    assert!(store.get_contact(id).expect("get").is_none());
    assert!(matches!(
        store.delete_contact(id),
        Err(StoreError::NotFound)
    ));
}

#[test]
fn activity_log_lists_newest_first_and_clears() {
    let store = Store::open_in_memory().expect("open");
    store.append_log(LogLevel::Info, "第一筆").expect("log 1");
    store.append_log(LogLevel::Warn, "第二筆").expect("log 2");
    store.append_log(LogLevel::Error, "第三筆").expect("log 3");

    let recent = store.recent_logs(2).expect("recent");
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].message, "第三筆");
    assert_eq!(recent[0].level, LogLevel::Error);
    assert_eq!(recent[1].message, "第二筆");
    assert_eq!(recent[1].level, LogLevel::Warn);
    assert!(recent[0].id > recent[1].id);

    store.clear_logs().expect("clear");
    assert!(store.recent_logs(10).expect("empty").is_empty());
}

#[test]
fn sent_log_lists_newest_first_with_limit() {
    let store = Store::open_in_memory().expect("open");
    let first = store
        .insert_sent(
            Path::new("archive/a.pdf"),
            "EPSON FAX",
            "0223456789",
            "第一",
        )
        .expect("first");
    let second = store
        .insert_sent(Path::new("archive/b.pdf"), "EPSON FAX", "+886 800", "")
        .expect("second");
    assert!(second > first);

    let sent = store.list_sent(1).expect("limit 1");
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].id, second);
    assert_eq!(sent[0].file_path, Path::new("archive/b.pdf"));
    assert_eq!(sent[0].printer, "EPSON FAX");
    assert_eq!(sent[0].recipient, "+886 800");

    let all = store.list_sent(10).expect("all");
    assert_eq!(
        all.iter().map(|s| s.id).collect::<Vec<_>>(),
        vec![second, first]
    );
    assert_eq!(all[1].note, "第一");
}
