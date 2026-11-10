//! Address book: editor, delete, and CSV import / export.

use super::Controller;
use crate::app::msg::Field;
use crate::app::state::ToastKind;
use crate::i18n::{t, tf};
use crate::model::{Contact, ContactId, LogLevel, normalize_number};
use crate::store::csv;
use std::path::PathBuf;

/// File name used when the CSV export target is a folder.
const CONTACTS_EXPORT_FILE: &str = "contacts.csv";

/// The six editor fields, in editor order.
const CONTACT_FIELDS: [Field; 6] = [
    Field::ContactName,
    Field::ContactCompany,
    Field::ContactFax,
    Field::ContactPhone,
    Field::ContactEmail,
    Field::ContactNote,
];

impl Controller {
    pub(super) fn clear_contact_fields(&mut self) {
        for field in CONTACT_FIELDS {
            self.state.fields.insert(field, String::new());
        }
    }

    fn prime_contact_fields(&mut self, contact: &Contact) {
        let values = [
            contact.name.clone(),
            contact.company.clone(),
            contact.fax_number.clone(),
            contact.phone.clone(),
            contact.email.clone(),
            contact.note.clone(),
        ];
        for (field, value) in CONTACT_FIELDS.into_iter().zip(values) {
            self.state.fields.insert(field, value);
        }
    }

    /// Editor fields as a contact draft (fax number normalised; empty when invalid).
    fn contact_from_fields(&self, id: ContactId) -> Contact {
        let text = |field: Field| self.state.field(field).trim().to_owned();
        Contact {
            id,
            name: text(Field::ContactName),
            company: text(Field::ContactCompany),
            fax_number: normalize_number(&text(Field::ContactFax)).unwrap_or_default(),
            phone: text(Field::ContactPhone),
            email: text(Field::ContactEmail),
            note: text(Field::ContactNote),
        }
    }

    pub(super) fn new_contact(&mut self) {
        self.clear_contact_fields();
        self.state.editing_contact = Some(0);
    }

    pub(super) fn edit_contact(&mut self, id: ContactId) {
        match self.or_report(self.store.get_contact(id)) {
            Some(Some(contact)) => {
                self.prime_contact_fields(&contact);
                self.state.editing_contact = Some(contact.id);
            }
            Some(None) => {
                self.state.editing_contact = None;
                self.refresh_contacts();
            }
            None => {}
        }
    }

    /// Validate and save the editor. A fax number may belong to only one contact.
    pub(super) fn save_contact(&mut self) {
        let Some(id) = self.state.editing_contact else {
            return;
        };
        let lang = self.state.lang;
        let typed_fax = self.state.field(Field::ContactFax).trim().to_owned();
        let draft = self.contact_from_fields(id);
        if draft.name.is_empty() {
            self.report_error(t(lang, "app.contact_invalid_name").to_owned());
            return;
        }
        if !typed_fax.is_empty() && draft.fax_number.is_empty() {
            self.report_error(t(lang, "app.contact_invalid_fax").to_owned());
            return;
        }
        if !draft.fax_number.is_empty() {
            let Some(existing) = self.or_report(self.store.contact_by_number(&draft.fax_number))
            else {
                return;
            };
            if existing.is_some_and(|other| other.id != id) {
                self.report_error(t(lang, "app.contact_exists").to_owned());
                return;
            }
        }
        if self.or_report(self.store.upsert_contact(&draft)).is_none() {
            return;
        }
        if !draft.fax_number.is_empty() {
            self.or_report(self.store.fill_sender_names(&draft.fax_number, &draft.name));
        }
        self.state.editing_contact = None;
        self.toast(ToastKind::Success, t(lang, "app.contact_saved").to_owned());
        self.log(
            LogLevel::Info,
            &tf(lang, "app.log_contact_saved", &[("name", &draft.name)]),
        );
        self.refresh_all();
    }

    /// Confirmed delete of one contact.
    pub(super) fn delete_contact(&mut self, id: ContactId) {
        let lang = self.state.lang;
        let name = match self.or_report(self.store.get_contact(id)) {
            Some(Some(contact)) => contact.name,
            Some(None) => {
                self.refresh_contacts();
                return;
            }
            None => return,
        };
        if self.or_report(self.store.delete_contact(id)).is_none() {
            return;
        }
        if self.state.editing_contact == Some(id) {
            self.state.editing_contact = None;
        }
        self.toast(
            ToastKind::Success,
            t(lang, "app.contact_deleted").to_owned(),
        );
        self.log(
            LogLevel::Info,
            &tf(lang, "app.log_contact_deleted", &[("name", &name)]),
        );
        self.refresh_contacts();
    }

    /// Insert or update one imported contact; a matching fax number is updated in place.
    /// Returns false when the store failed (already reported).
    fn import_csv_contact(&mut self, mut contact: Contact) -> bool {
        if !contact.fax_number.is_empty() {
            let Some(existing) = self.or_report(self.store.contact_by_number(&contact.fax_number))
            else {
                return false;
            };
            if let Some(existing) = existing {
                contact.id = existing.id;
            }
        }
        if self
            .or_report(self.store.upsert_contact(&contact))
            .is_none()
        {
            return false;
        }
        if !contact.fax_number.is_empty() {
            self.or_report(
                self.store
                    .fill_sender_names(&contact.fax_number, &contact.name),
            );
        }
        true
    }

    pub(super) fn import_contacts_csv(&mut self) {
        let lang = self.state.lang;
        let text = self.state.field(Field::CsvPath).trim().to_owned();
        if text.is_empty() {
            self.report_error(t(lang, "app.csv_no_path").to_owned());
            return;
        }
        let contacts = match csv::read_file(&PathBuf::from(text)) {
            Ok(contacts) => contacts,
            Err(error) => {
                let text = tf(lang, "app.csv_error", &[("error", &format!("{error:#}"))]);
                self.report_error(text);
                return;
            }
        };
        let mut count = 0u64;
        for contact in contacts {
            if !self.import_csv_contact(contact) {
                self.refresh_all();
                return;
            }
            count += 1;
        }
        let count = count.to_string();
        self.toast(
            ToastKind::Success,
            tf(lang, "app.csv_imported", &[("count", &count)]),
        );
        self.log(
            LogLevel::Info,
            &tf(lang, "app.log_csv_imported", &[("count", &count)]),
        );
        self.refresh_all();
    }

    /// Write every contact to the CSV path (a folder gets `contacts.csv` inside).
    pub(super) fn export_contacts_csv(&mut self) {
        let lang = self.state.lang;
        let text = self.state.field(Field::CsvPath).trim().to_owned();
        if text.is_empty() {
            self.report_error(t(lang, "app.csv_no_path").to_owned());
            return;
        }
        let mut path = PathBuf::from(text);
        if path.is_dir() {
            path.push(CONTACTS_EXPORT_FILE);
        }
        let Some(contacts) = self.or_report(self.store.list_contacts("")) else {
            return;
        };
        if let Err(error) = csv::write_file(&path, &contacts) {
            let text = tf(lang, "app.csv_error", &[("error", &format!("{error:#}"))]);
            self.report_error(text);
            return;
        }
        let count = contacts.len().to_string();
        let shown = path.display().to_string();
        self.toast(
            ToastKind::Success,
            tf(
                lang,
                "app.csv_exported",
                &[("count", &count), ("path", &shown)],
            ),
        );
        self.log(
            LogLevel::Info,
            &tf(lang, "app.log_csv_exported", &[("count", &count)]),
        );
    }
}
