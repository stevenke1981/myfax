//! Text-field events: live edits, Enter (`Submit`) map and Escape (`Cancel`).

use super::Controller;
use crate::app::msg::{Field, Msg};

impl Controller {
    /// Store a field value and run the side effect that field drives.
    pub(super) fn apply_field_text(&mut self, field: Field, value: String) {
        self.state.fields.insert(field, value);
        match field {
            Field::Search => self.requery_search(),
            Field::ContactSearch => self.refresh_contacts(),
            _ => {}
        }
    }

    /// Re-query the inbox from page 0 with the trimmed search text.
    pub(super) fn requery_search(&mut self) {
        let text = self.state.field(Field::Search).trim().to_owned();
        self.state.query.text = text;
        self.state.query.page = 0;
        self.refresh_results();
    }

    /// Enter pressed in a text field (UI spec 9).
    pub(super) fn submit_field(&mut self, field: Field) {
        match field {
            Field::Search => self.requery_search(),
            Field::DateFrom | Field::DateTo => self.update(Msg::ApplyCustomRange),
            Field::ImportPath => self.update(Msg::ImportFromField),
            Field::WatchFolderInput => self.update(Msg::AddWatchFolder),
            Field::ContactName
            | Field::ContactCompany
            | Field::ContactFax
            | Field::ContactPhone
            | Field::ContactEmail
            | Field::ContactNote => self.update(Msg::SaveContact),
            Field::FaxSenderName | Field::FaxTags | Field::FaxNote => {
                if let Some(id) = self.state.selected.as_ref().map(|record| record.id) {
                    self.update(Msg::SaveFaxEdits(id));
                }
            }
            _ => {}
        }
    }

    /// Escape: close the confirm dialog first, otherwise the contact editor.
    pub(super) fn cancel(&mut self) {
        if self.state.confirm.is_some() {
            self.update(Msg::ConfirmNo);
        } else if self.state.editing_contact.is_some() {
            self.update(Msg::CancelContactEdit);
        }
    }
}
