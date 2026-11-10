//! Address book (validation, duplicates, delete, CSV round trip) and the Send page
//! (printer choice, hand-over to the fax driver, history, errors).

use super::tests::{Harness, fax_name, fax_pdf, zh};
use crate::app::msg::{ConfirmAction, Field, Msg, Route};
use crate::app::state::ToastKind;
use crate::config;
use crate::i18n::{Lang, tf};
use crate::ui::runtime::UiEvent;

/// Fill the contact editor and save it.
fn save_contact(h: &mut Harness, name: &str, fax: &str) {
    h.controller.update(Msg::NewContact);
    h.controller.set_field(Field::ContactName, name.to_owned());
    h.controller.set_field(Field::ContactFax, fax.to_owned());
    h.controller.update(Msg::SaveContact);
}

#[test]
fn contact_save_validates_name_and_fax_then_normalises_the_number() {
    let mut h = Harness::new();
    h.controller.update(Msg::NewContact);
    assert_eq!(h.controller.state.editing_contact, Some(0));

    h.controller.update(Msg::SaveContact);
    assert_eq!(h.toast_text(), zh("app.contact_invalid_name"));

    h.controller
        .set_field(Field::ContactName, "王小明".to_owned());
    h.controller.set_field(Field::ContactFax, "abc".to_owned());
    h.controller.update(Msg::SaveContact);
    assert_eq!(h.toast_text(), zh("app.contact_invalid_fax"));
    assert_eq!(
        h.controller.state.editing_contact,
        Some(0),
        "editor stays open"
    );

    h.controller
        .set_field(Field::ContactFax, "02-2345 6789".to_owned());
    h.controller.update(Msg::SaveContact);
    assert_eq!(h.toast_text(), zh("app.contact_saved"));
    assert_eq!(h.controller.state.editing_contact, None);
    assert_eq!(h.controller.state.contacts.len(), 1);
    assert_eq!(h.controller.state.contacts[0].fax_number, "0223456789");
}

#[test]
fn saving_a_number_fills_missing_sender_names_of_matching_faxes() {
    let mut h = Harness::new();
    let name = fax_name(2026, 3, 5, 9, 0, "0223456789");
    h.import_fax(&name, 1);
    let id = h.id_of(&name);
    assert!(h.fax(id).sender_name.is_none());

    save_contact(&mut h, "李老師", "0223456789");
    assert_eq!(h.fax(id).sender_name.as_deref(), Some("李老師"));
    assert_eq!(
        h.controller.state.results.items[0].sender_name.as_deref(),
        Some("李老師")
    );
}

#[test]
fn a_fax_number_may_belong_to_only_one_contact() {
    let mut h = Harness::new();
    save_contact(&mut h, "甲", "0223456789");
    save_contact(&mut h, "乙", "02-2345-6789");
    assert_eq!(h.toast_text(), zh("app.contact_exists"));
    assert_eq!(h.controller.state.contacts.len(), 1);
    assert_eq!(h.controller.state.contacts[0].name, "甲");
}

#[test]
fn editing_a_contact_primes_the_fields_and_delete_needs_confirmation() {
    let mut h = Harness::new();
    save_contact(&mut h, "王小明", "0223456789");
    let id = h.controller.state.contacts[0].id;

    h.controller.update(Msg::EditContact(id));
    assert_eq!(h.controller.state.editing_contact, Some(id));
    assert_eq!(h.controller.state.field(Field::ContactName), "王小明");
    assert_eq!(h.controller.state.field(Field::ContactFax), "0223456789");

    h.controller.update(Msg::AskDeleteContact(id));
    let dialog = h.controller.state.confirm.clone().expect("confirm open");
    assert_eq!(dialog.action, ConfirmAction::DeleteContact(id));
    assert!(dialog.body.contains("王小明"));

    h.controller.update(Msg::ConfirmYes);
    assert!(h.controller.state.contacts.is_empty());
    assert_eq!(h.controller.state.editing_contact, None);
    assert_eq!(h.toast_text(), zh("app.contact_deleted"));
}

#[test]
fn contact_search_filters_the_list_in_the_store() {
    let mut h = Harness::new();
    save_contact(&mut h, "王小明", "0223456789");
    save_contact(&mut h, "李大華", "0987654321");
    h.controller
        .set_field(Field::ContactSearch, "李".to_owned());
    assert_eq!(h.controller.state.contacts.len(), 1);
    assert_eq!(h.controller.state.contacts[0].name, "李大華");
}

#[test]
fn csv_export_to_a_folder_and_import_back_round_trips() {
    let mut source = Harness::new();
    save_contact(&mut source, "王小明", "0223456789");
    save_contact(&mut source, "李大華", "0987654321");
    let folder = source.path("out");
    std::fs::create_dir_all(&folder).expect("mkdir");
    source
        .controller
        .set_field(Field::CsvPath, folder.display().to_string());
    source.controller.update(Msg::ExportContactsCsv);
    let file = folder.join("contacts.csv");
    assert!(file.is_file(), "folder target writes contacts.csv");
    assert_eq!(
        source.toast_text(),
        tf(
            Lang::ZhTw,
            "app.csv_exported",
            &[("count", "2"), ("path", &file.display().to_string())]
        )
    );

    let mut target = Harness::new();
    target
        .controller
        .set_field(Field::CsvPath, file.display().to_string());
    target.controller.update(Msg::ImportContactsCsv);
    assert_eq!(target.controller.state.contacts.len(), 2);
    assert_eq!(
        target.toast_text(),
        tf(
            Lang::ZhTw,
            "app.csv_imported",
            &[("count", "2"), ("skipped", "0")]
        )
    );
}

#[test]
fn csv_export_refuses_to_overwrite_an_existing_file() {
    let mut h = Harness::new();
    save_contact(&mut h, "王小明", "0223456789");
    let file = h.write("out/contacts.csv", b"keep me");
    h.controller
        .set_field(Field::CsvPath, file.display().to_string());
    h.controller.update(Msg::ExportContactsCsv);
    assert_eq!(h.toast().map(|(kind, _)| kind), Some(ToastKind::Error));
    assert_eq!(
        h.toast_text(),
        tf(
            Lang::ZhTw,
            "app.csv_export_exists",
            &[("path", &file.display().to_string())]
        )
    );
    assert_eq!(std::fs::read(&file).expect("file kept"), b"keep me");
}

#[test]
fn csv_export_to_a_folder_never_overwrites_and_picks_an_unused_name() {
    let mut h = Harness::new();
    save_contact(&mut h, "王小明", "0223456789");
    let folder = h.path("out");
    std::fs::create_dir_all(&folder).expect("mkdir");
    std::fs::write(folder.join("contacts.csv"), b"old").expect("seed");
    std::fs::write(folder.join("contacts (2).csv"), b"older").expect("seed");
    h.controller
        .set_field(Field::CsvPath, folder.display().to_string());
    h.controller.update(Msg::ExportContactsCsv);
    assert_eq!(
        std::fs::read(folder.join("contacts.csv")).expect("kept"),
        b"old"
    );
    assert_eq!(
        std::fs::read(folder.join("contacts (2).csv")).expect("kept"),
        b"older"
    );
    assert!(
        folder.join("contacts (3).csv").is_file(),
        "next free name used"
    );
    assert_eq!(h.toast().map(|(kind, _)| kind), Some(ToastKind::Success));
}

#[test]
fn csv_reimport_skips_contacts_already_present_by_name_and_number() {
    let mut h = Harness::new();
    let file = h.write(
        "in.csv",
        "name,fax
Alice,02-2345-6789
Bob,
"
        .as_bytes(),
    );
    h.controller
        .set_field(Field::CsvPath, file.display().to_string());
    h.controller.update(Msg::ImportContactsCsv);
    assert_eq!(h.controller.state.contacts.len(), 2);

    h.controller.update(Msg::ImportContactsCsv);
    assert_eq!(
        h.controller.state.contacts.len(),
        2,
        "re-import of contacts without a number adds nothing"
    );
    assert_eq!(
        h.toast_text(),
        tf(
            Lang::ZhTw,
            "app.csv_imported",
            &[("count", "0"), ("skipped", "2")]
        )
    );

    let same_name_other_number = h.write(
        "other.csv",
        "name,fax
alice,0987654321
"
        .as_bytes(),
    );
    h.controller
        .set_field(Field::CsvPath, same_name_other_number.display().to_string());
    h.controller.update(Msg::ImportContactsCsv);
    assert_eq!(
        h.controller.state.contacts.len(),
        3,
        "same name with a different number is a new contact"
    );
}

#[test]
fn csv_import_reports_missing_path_and_bad_files() {
    let mut h = Harness::new();
    h.controller.update(Msg::ImportContactsCsv);
    assert_eq!(h.toast_text(), zh("app.csv_no_path"));

    let bad = h.write("bad.csv", b"fax,phone\n1,2\n");
    h.controller
        .set_field(Field::CsvPath, bad.display().to_string());
    h.controller.update(Msg::ImportContactsCsv);
    assert_eq!(h.toast().map(|t| t.0), Some(ToastKind::Error));
    assert!(h.controller.state.contacts.is_empty());
}

#[test]
fn printers_refresh_auto_selects_the_first_fax_printer() {
    let printers = vec!["HP LaserJet".to_owned(), "EPSON FAX Driver".to_owned()];
    let mut h = Harness::with_platform(printers, false);
    h.controller.update(Msg::RefreshPrinters);
    assert_eq!(h.controller.state.printers.len(), 2);
    assert_eq!(
        h.controller.state.selected_printer.as_deref(),
        Some("EPSON FAX Driver")
    );
    assert_eq!(
        h.toast_text(),
        tf(Lang::ZhTw, "app.printers_refreshed", &[("count", "2")])
    );
}

#[test]
fn selecting_a_printer_persists_it_in_the_settings_file() {
    let mut h = Harness::with_platform(vec!["HP".to_owned()], false);
    h.controller.update(Msg::SelectPrinter("HP".to_owned()));
    let saved = config::load_settings(&h.path("settings.json")).expect("settings written");
    assert_eq!(saved.fax_printer.as_deref(), Some("HP"));
}

#[test]
fn send_without_printer_or_file_explains_what_is_missing() {
    let mut h = Harness::new();
    h.controller.update(Msg::SendFax);
    assert_eq!(h.toast_text(), zh("app.send_no_printer"));

    let mut h = Harness::with_platform(vec!["EPSON FAX".to_owned()], false);
    h.controller.update(Msg::RefreshPrinters);
    h.controller.update(Msg::SendFax);
    assert_eq!(h.toast_text(), zh("app.send_no_file"));

    let missing = h.path("gone.pdf");
    h.controller
        .set_field(Field::SendFile, missing.display().to_string());
    h.controller.update(Msg::SendFax);
    assert_eq!(h.toast().map(|t| t.0), Some(ToastKind::Error));
    assert!(h.probe.calls_with("print_to:").is_empty());
}

#[test]
fn send_hands_the_file_to_the_fax_driver_and_records_history() {
    let mut h = Harness::with_platform(vec!["EPSON FAX".to_owned()], false);
    h.controller.update(Msg::RefreshPrinters);
    let file = h.write("outbox/memo.pdf", &fax_pdf(1, "memo"));
    h.controller
        .set_field(Field::SendFile, file.display().to_string());
    h.controller
        .set_field(Field::SendRecipient, "王小明 (0223456789)".to_owned());
    h.controller.set_field(Field::SendNote, "急件".to_owned());

    h.controller.update(Msg::SendFax);

    let calls = h.probe.calls_with("print_to:");
    assert_eq!(
        calls,
        vec![format!("print_to:{}:EPSON FAX", file.display())]
    );
    assert_eq!(h.controller.state.sent.len(), 1);
    assert_eq!(h.controller.state.sent[0].recipient, "王小明 (0223456789)");
    assert_eq!(h.controller.state.sent[0].note, "急件");
    assert_eq!(h.controller.state.field(Field::SendNote), "");
    assert_eq!(h.toast().map(|t| t.0), Some(ToastKind::Info));
    assert!(h.toast_text().contains("EPSON FAX"));
}

#[test]
fn driver_failure_is_reported_and_not_recorded() {
    let mut h = Harness::with_platform(vec!["EPSON FAX".to_owned()], true);
    h.controller.update(Msg::RefreshPrinters);
    let file = h.write("outbox/memo.pdf", &fax_pdf(1, "memo"));
    h.controller
        .set_field(Field::SendFile, file.display().to_string());
    h.controller.update(Msg::SendFax);
    assert_eq!(h.toast().map(|t| t.0), Some(ToastKind::Error));
    assert!(h.toast_text().contains("driver offline"));
    assert!(h.controller.state.sent.is_empty());
}

#[test]
fn use_contact_for_send_fills_the_recipient_and_opens_send() {
    let mut h = Harness::new();
    save_contact(&mut h, "王小明", "0223456789");
    save_contact(&mut h, "無傳真", "");
    let with_fax = h
        .controller
        .state
        .contacts
        .iter()
        .find(|c| c.name == "王小明")
        .map(|c| c.id)
        .expect("contact");
    let without = h
        .controller
        .state
        .contacts
        .iter()
        .find(|c| c.name == "無傳真")
        .map(|c| c.id)
        .expect("contact");

    h.controller.update(Msg::UseContactForSend(with_fax));
    assert_eq!(
        h.controller.state.field(Field::SendRecipient),
        "王小明 (0223456789)"
    );
    assert_eq!(h.controller.state.route, Route::Send);

    h.controller.update(Msg::UseContactForSend(without));
    assert_eq!(h.controller.state.field(Field::SendRecipient), "無傳真");
}

#[test]
fn entering_send_refreshes_printers_quietly() {
    let mut h = Harness::with_platform(vec!["HP".to_owned(), "EPSON FAX".to_owned()], false);
    h.controller.update(Msg::Navigate(Route::Send));
    assert_eq!(h.controller.state.printers.len(), 2);
    assert!(
        h.toast().is_none(),
        "navigation does not announce the refresh"
    );
}

#[test]
fn cancel_closes_the_editor_without_creating_a_contact() {
    let mut h = Harness::new();
    h.controller.update(Msg::NewContact);
    h.controller.on_ui_event(UiEvent::Cancel);
    assert_eq!(h.controller.state.editing_contact, None);
    assert!(h.controller.state.contacts.is_empty());
}
