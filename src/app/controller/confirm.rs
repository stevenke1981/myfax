//! Confirm dialogs: opening them (`Ask*`), and running the confirmed action.

use super::Controller;
use crate::app::msg::ConfirmAction;
use crate::app::state::{ConfirmDialog, ToastKind};
use crate::i18n::{t, tf};
use crate::model::{ContactId, FaxId, LogLevel};

impl Controller {
    fn open_confirm(&mut self, title: String, body: String, action: ConfirmAction) {
        self.state.confirm = Some(ConfirmDialog {
            title,
            body,
            action,
        });
    }

    pub(super) fn ask_delete_permanently(&mut self, id: FaxId) {
        let Some(record) = self.load_fax(id) else {
            return;
        };
        let lang = self.state.lang;
        let name = self.fax_name(&record);
        let title = t(lang, "app.confirm_delete_title").to_owned();
        let body = tf(lang, "app.confirm_delete_body", &[("name", &name)]);
        self.open_confirm(title, body, ConfirmAction::DeletePermanently(id));
    }

    pub(super) fn ask_empty_trash(&mut self) {
        let lang = self.state.lang;
        let Some(count) = self.or_report(self.store.trashed()).map(|rows| rows.len()) else {
            return;
        };
        let count_text = count.to_string();
        let title = t(lang, "app.confirm_empty_title").to_owned();
        let body = tf(lang, "app.confirm_empty_body", &[("count", &count_text)]);
        self.open_confirm(title, body, ConfirmAction::EmptyTrash);
    }

    pub(super) fn ask_delete_contact(&mut self, id: ContactId) {
        let lang = self.state.lang;
        let Some(contact) = self.or_report(self.store.get_contact(id)).flatten() else {
            return;
        };
        let title = t(lang, "app.confirm_contact_title").to_owned();
        let body = tf(lang, "app.confirm_contact_body", &[("name", &contact.name)]);
        self.open_confirm(title, body, ConfirmAction::DeleteContact(id));
    }

    pub(super) fn ask_clear_log(&mut self) {
        let lang = self.state.lang;
        let title = t(lang, "app.confirm_log_title").to_owned();
        let body = t(lang, "app.confirm_log_body").to_owned();
        self.open_confirm(title, body, ConfirmAction::ClearLog);
    }

    pub(super) fn confirm_no(&mut self) {
        self.state.confirm = None;
    }

    /// Close the dialog and run its action.
    pub(super) fn confirm_yes(&mut self) {
        let Some(dialog) = self.state.confirm.take() else {
            return;
        };
        match dialog.action {
            ConfirmAction::DeletePermanently(id) => self.delete_fax_permanently(id),
            ConfirmAction::EmptyTrash => self.empty_trash(),
            ConfirmAction::DeleteContact(id) => self.delete_contact(id),
            ConfirmAction::ClearLog => self.clear_log(),
        }
    }

    /// Permanently delete every trashed fax. Failures are reported; the rest continue.
    fn empty_trash(&mut self) {
        let lang = self.state.lang;
        let Some(trashed) = self.or_report(self.store.trashed()) else {
            return;
        };
        let mut purged = 0u64;
        let mut failure: Option<String> = None;
        for record in &trashed {
            match self.purge_record(record) {
                Ok(()) => purged += 1,
                Err(text) => {
                    self.log(LogLevel::Error, &text);
                    failure = Some(text);
                }
            }
        }
        let count = purged.to_string();
        match failure {
            Some(text) => self.toast(ToastKind::Error, text),
            None => self.toast(
                ToastKind::Success,
                tf(lang, "app.trash_emptied", &[("count", &count)]),
            ),
        }
        self.log(
            LogLevel::Info,
            &tf(lang, "app.log_trash_emptied", &[("count", &count)]),
        );
        self.refresh_all();
    }
}
