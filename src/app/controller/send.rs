//! Send via the printer's fax driver: printer list, recipient, hand-over and history.

use super::Controller;
use super::import::file_label;
use crate::app::msg::{Field, Route};
use crate::app::state::ToastKind;
use crate::i18n::{t, tf};
use crate::model::{ContactId, LogLevel};
use crate::system::fax_printers;
use std::path::PathBuf;

impl Controller {
    /// Reload installed printers. Without `announce` the refresh is silent (used when
    /// entering the Send page). Auto-selects the first fax printer when none is chosen.
    pub(super) fn refresh_printers(&mut self, announce: bool) {
        let lang = self.state.lang;
        match self.platform.list_printers() {
            Ok(printers) => {
                if self.state.selected_printer.is_none() {
                    self.state.selected_printer = fax_printers(&printers).into_iter().next();
                }
                let count = printers.len().to_string();
                self.state.printers = printers;
                if announce {
                    self.toast(
                        ToastKind::Success,
                        tf(lang, "app.printers_refreshed", &[("count", &count)]),
                    );
                }
            }
            Err(error) => {
                let text = tf(
                    lang,
                    "app.printers_error",
                    &[("error", &format!("{error:#}"))],
                );
                self.report_error(text);
            }
        }
    }

    /// Choose the printer and remember it in the settings.
    pub(super) fn select_printer(&mut self, name: String) {
        let mut next = self.state.settings.clone();
        next.fax_printer = Some(name.clone());
        self.state.selected_printer = Some(name);
        self.commit_settings(next);
    }

    /// Fill the recipient field with "name (fax number)" and open the Send page.
    pub(super) fn use_contact_for_send(&mut self, id: ContactId) {
        let contact = match self.or_report(self.store.get_contact(id)) {
            Some(Some(contact)) => contact,
            Some(None) => {
                self.refresh_contacts();
                return;
            }
            None => return,
        };
        let recipient = if contact.fax_number.is_empty() {
            contact.name
        } else {
            format!("{} ({})", contact.name, contact.fax_number)
        };
        self.state.fields.insert(Field::SendRecipient, recipient);
        if self.state.route != Route::Send {
            self.navigate(Route::Send);
        }
    }

    /// Hand the chosen file to the printer's fax driver and record it in the history.
    pub(super) fn send_fax(&mut self) {
        let lang = self.state.lang;
        let Some(printer) = self.state.selected_printer.clone() else {
            self.report_error(t(lang, "app.send_no_printer").to_owned());
            return;
        };
        let file_text = self.state.field(Field::SendFile).trim().to_owned();
        if file_text.is_empty() {
            self.report_error(t(lang, "app.send_no_file").to_owned());
            return;
        }
        let file = PathBuf::from(file_text);
        if !file.is_file() {
            let shown = file.display().to_string();
            let text = tf(lang, "app.send_file_missing", &[("path", &shown)]);
            self.report_error(text);
            return;
        }
        if let Err(error) = self.platform.print_to(&file, &printer) {
            let text = tf(lang, "app.send_error", &[("error", &format!("{error:#}"))]);
            self.report_error(text);
            return;
        }
        let recipient = self.state.field(Field::SendRecipient).trim().to_owned();
        let note = self.state.field(Field::SendNote).trim().to_owned();
        if self
            .or_report(self.store.insert_sent(&file, &printer, &recipient, &note))
            .is_none()
        {
            return;
        }
        let label = file_label(&file);
        self.toast(
            ToastKind::Info,
            tf(lang, "app.send_handed", &[("printer", &printer)]),
        );
        self.log(
            LogLevel::Info,
            &tf(
                lang,
                "app.log_send_handed",
                &[("file", &label), ("printer", &printer)],
            ),
        );
        self.state.fields.insert(Field::SendNote, String::new());
        self.refresh_sent();
    }
}
