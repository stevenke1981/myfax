//! Fax actions: read/star, trash and restore, confirmed deletes and the confirm gate,
//! edits, the address-book link, file actions, export and start-up trash purge.

use super::tests::{Harness, fax_name, fax_pdf, zh};
use crate::app::msg::{ConfirmAction, Field, Msg, Route};
use crate::app::state::ToastKind;
use crate::config::Settings;
use crate::i18n::{Lang, tf};
use crate::model::{Contact, FaxSource, Folder, NewFax};
use crate::store::Store;
use crate::ui::runtime::UiEvent;
use chrono::{Local, TimeDelta};

fn first_name() -> String {
    fax_name(2026, 3, 5, 14, 7, "0912345678")
}

#[test]
fn toggling_read_and_star_updates_rows_counts_and_selection() {
    let mut h = Harness::new();
    h.import_fax(&first_name(), 1);
    let id = h.id_of(&first_name());

    h.controller.update(Msg::ToggleRead(id));
    assert!(!h.fax(id).is_read);
    assert_eq!(h.controller.state.counts.unread, 1);
    assert!(!h.controller.state.results.items[0].is_read);

    h.controller.update(Msg::ToggleStar(id));
    assert!(h.fax(id).is_starred);
    assert_eq!(h.controller.state.counts.starred, 1);
    assert!(
        h.controller
            .state
            .selected
            .as_ref()
            .is_some_and(|r| r.is_starred)
    );

    h.controller.update(Msg::MarkAllRead);
    assert_eq!(h.controller.state.counts.unread, 0);
    assert_eq!(
        h.toast_text(),
        tf(Lang::ZhTw, "app.marked_all_read", &[("count", "1")])
    );
}

#[test]
fn trash_restore_then_confirmed_delete_removes_the_file() {
    let mut h = Harness::new();
    h.import_fax(&first_name(), 1);
    let id = h.id_of(&first_name());
    let archived = h.fax(id).file_path;
    assert!(archived.is_file());

    h.controller.update(Msg::MoveToTrash(id));
    assert_eq!(h.controller.state.counts.trash, 1);
    assert!(
        h.controller.state.selected.is_none(),
        "trashed fax leaves the detail pane"
    );

    h.controller.update(Msg::Restore(id));
    assert_eq!(h.controller.state.counts.trash, 0);
    assert_eq!(h.controller.state.results.total, 1);

    h.controller.update(Msg::MoveToTrash(id));
    h.controller.update(Msg::AskDeletePermanently(id));
    let dialog = h.controller.state.confirm.clone().expect("confirm open");
    assert_eq!(dialog.action, ConfirmAction::DeletePermanently(id));
    assert!(
        dialog.body.contains("0912345678"),
        "body names the sender: {}",
        dialog.body
    );

    h.controller.update(Msg::ConfirmYes);
    assert!(h.controller.state.confirm.is_none());
    assert!(!archived.exists(), "archived file removed");
    assert!(h.controller.store.get_fax(id).expect("store").is_none());
    assert_eq!(h.toast_text(), zh("app.deleted_forever"));
}

#[test]
fn open_confirm_dialog_gates_every_message_except_answers() {
    let mut h = Harness::new();
    h.import_fax(&first_name(), 1);
    let id = h.id_of(&first_name());

    h.controller.update(Msg::AskEmptyTrash);
    assert!(h.controller.state.confirm.is_some());

    h.controller.update(Msg::Navigate(Route::Contacts));
    h.controller.update(Msg::ToggleStar(id));
    assert_eq!(h.controller.state.route, Route::Inbox);
    assert!(!h.fax(id).is_starred);

    h.controller.update(Msg::DismissToast);
    assert!(
        h.controller.state.confirm.is_some(),
        "DismissToast is not an answer"
    );

    h.controller.update(Msg::ConfirmNo);
    assert!(h.controller.state.confirm.is_none());
    h.controller.update(Msg::Navigate(Route::Contacts));
    assert_eq!(h.controller.state.route, Route::Contacts);

    h.controller.update(Msg::AskEmptyTrash);
    h.controller.on_ui_event(UiEvent::Cancel);
    assert!(h.controller.state.confirm.is_none(), "Escape answers No");
}

#[test]
fn empty_trash_deletes_every_trashed_file() {
    let mut h = Harness::new();
    let names = [first_name(), fax_name(2026, 3, 5, 15, 0, "0223456789")];
    for name in &names {
        h.import_fax(name, 1);
    }
    let archived: Vec<_> = names
        .iter()
        .map(|name| h.fax(h.id_of(name)).file_path)
        .collect();
    for name in &names {
        let id = h.id_of(name);
        h.controller.update(Msg::MoveToTrash(id));
    }
    assert_eq!(h.controller.state.counts.trash, 2);

    h.controller.update(Msg::AskEmptyTrash);
    h.controller.update(Msg::ConfirmYes);
    assert_eq!(h.controller.state.counts.trash, 0);
    assert!(archived.iter().all(|path| !path.exists()));
    assert_eq!(
        h.toast_text(),
        tf(Lang::ZhTw, "app.trash_emptied", &[("count", "2")])
    );
}

#[test]
fn deleting_a_fax_whose_file_is_already_gone_still_succeeds() {
    let mut h = Harness::new();
    h.import_fax(&first_name(), 1);
    let id = h.id_of(&first_name());
    std::fs::remove_file(h.fax(id).file_path).expect("remove archive file");
    h.controller.update(Msg::MoveToTrash(id));
    h.controller.update(Msg::AskDeletePermanently(id));
    h.controller.update(Msg::ConfirmYes);
    assert!(h.controller.store.get_fax(id).expect("store").is_none());
    assert_eq!(h.toast().map(|t| t.0), Some(ToastKind::Success));
}

#[test]
fn save_edits_normalises_tags_and_reprimes_the_fields() {
    let mut h = Harness::new();
    h.import_fax(&first_name(), 1);
    let id = h.id_of(&first_name());

    h.controller
        .set_field(Field::FaxSenderName, "  王小明 ".to_owned());
    h.controller
        .set_field(Field::FaxTags, "Invoice， urgent, invoice".to_owned());
    h.controller.set_field(Field::FaxNote, " 回電 ".to_owned());
    h.controller.update(Msg::SaveFaxEdits(id));

    let saved = h.fax(id);
    assert_eq!(saved.sender_name.as_deref(), Some("王小明"));
    assert_eq!(saved.tags, vec!["invoice".to_owned(), "urgent".to_owned()]);
    assert_eq!(saved.note, "回電");
    assert_eq!(h.toast_text(), zh("app.edits_saved"));

    h.controller.update(Msg::CloseDetail);
    h.controller.update(Msg::SelectFax(id));
    assert_eq!(h.controller.state.field(Field::FaxTags), "invoice, urgent");
    assert_eq!(h.controller.state.field(Field::FaxSenderName), "王小明");
}

#[test]
fn add_sender_to_contacts_opens_a_new_or_the_existing_contact() {
    let mut h = Harness::new();
    h.import_fax(&first_name(), 1);
    let id = h.id_of(&first_name());

    h.controller.update(Msg::AddSenderToContacts(id));
    assert_eq!(h.controller.state.route, Route::Contacts);
    assert_eq!(h.controller.state.editing_contact, Some(0));
    assert_eq!(h.controller.state.field(Field::ContactFax), "0912345678");

    let existing = Contact {
        name: "陳經理".to_owned(),
        fax_number: "0912345678".to_owned(),
        ..Contact::default()
    };
    let existing_id = h
        .controller
        .store
        .upsert_contact(&existing)
        .expect("insert");
    h.controller.update(Msg::AddSenderToContacts(id));
    assert_eq!(h.controller.state.editing_contact, Some(existing_id));
    assert_eq!(h.controller.state.field(Field::ContactName), "陳經理");
}

#[test]
fn sender_without_number_cannot_be_added_to_contacts() {
    let mut h = Harness::new();
    h.import_fax("scan_no_number.pdf", 1);
    let id = h.id_of("scan_no_number.pdf");
    assert!(h.fax(id).sender_number.is_none());
    h.controller.update(Msg::AddSenderToContacts(id));
    assert_eq!(h.toast_text(), zh("app.no_sender_number"));
    assert_eq!(h.controller.state.editing_contact, None);
}

#[test]
fn open_and_print_report_missing_files_and_act_on_present_ones() {
    let mut h = Harness::new();
    h.import_fax(&first_name(), 1);
    let id = h.id_of(&first_name());
    let archived = h.fax(id).file_path;

    h.controller.update(Msg::OpenExternal(id));
    assert_eq!(h.probe.calls_with("open:").len(), 1);

    h.controller.update(Msg::Print(id));
    assert_eq!(h.probe.calls_with("print:").len(), 1);
    assert_eq!(h.toast().map(|t| t.0), Some(ToastKind::Success));

    h.controller.update(Msg::RevealInFolder(id));
    assert_eq!(h.probe.calls_with("reveal:").len(), 1);

    std::fs::remove_file(&archived).expect("remove archive file");
    h.controller.update(Msg::OpenExternal(id));
    assert_eq!(h.toast().map(|t| t.0), Some(ToastKind::Error));
    assert!(h.toast_text().contains(&archived.display().to_string()));
    assert_eq!(h.probe.calls_with("open:").len(), 1, "no second launch");
    assert!(
        h.log_messages()
            .iter()
            .any(|m| m.contains("找不到封存檔案"))
    );
}

#[test]
fn export_copies_the_filtered_list_and_reports_empty_results() {
    let mut h = Harness::new();
    h.import_fax(&first_name(), 1);
    h.import_fax(&fax_name(2026, 3, 6, 9, 0, "0223456789"), 1);

    h.controller.update(Msg::ExportFiltered);
    assert_eq!(h.toast_text(), zh("app.export_no_folder"));

    let export = h.path("exported");
    h.controller
        .set_field(Field::ExportFolder, export.display().to_string());
    h.controller.update(Msg::ExportFiltered);
    let copied = std::fs::read_dir(&export).expect("export dir").count();
    assert_eq!(copied, 2);
    assert_eq!(h.toast().map(|t| t.0), Some(ToastKind::Success));

    h.controller.update(Msg::SelectFolder(Folder::Starred));
    h.controller.update(Msg::ExportFiltered);
    assert_eq!(h.toast_text(), zh("app.export_none"));
}

#[test]
fn expired_trash_is_purged_when_the_controller_starts() {
    let archive = tempfile::tempdir().expect("temp dir");
    let file = archive.path().join("old.pdf");
    std::fs::write(&file, fax_pdf(1, "old")).expect("write");

    let store = Store::open_in_memory().expect("store");
    let id = store
        .insert_fax(&NewFax {
            received_at: Local::now(),
            sender_number: None,
            sender_name: None,
            pages: 1,
            file_path: file.clone(),
            file_size: 3,
            sha256: "f".repeat(64),
            original_name: "old.pdf".to_owned(),
            source: FaxSource::Manual,
        })
        .expect("insert");
    let long_ago = Local::now() - TimeDelta::try_days(40).expect("delta");
    store.move_to_trash(id, long_ago).expect("trash");

    let settings = Settings {
        trash_retention_days: 30,
        ..Settings::default()
    };
    let h = Harness::with_store(settings, store, archive.path().join("myfax.db"));
    assert_eq!(h.controller.state.counts.trash, 0);
    assert!(!file.exists());
    assert!(h.log_messages().iter().any(|m| m.contains("自動清除")));
}

#[test]
fn unreadable_fax_is_still_archived_with_zero_pages() {
    let mut h = Harness::new();
    let name = "FAX_20260305_140700_0912345678.pdf";
    let bad = h.write(&format!("incoming/{name}"), b"not a pdf");
    h.controller.files_dropped(vec![bad]);
    assert_eq!(h.controller.state.counts.all, 1);
    assert_eq!(h.fax(h.id_of(name)).pages, 0);
    assert_eq!(h.toast().map(|t| t.0), Some(ToastKind::Success));
}
