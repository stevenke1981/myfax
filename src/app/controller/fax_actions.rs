//! Single-fax actions: selection, read/star, file actions, trash, edits and the
//! link to the address book.

use super::{Controller, PREVIEW_IMAGE_KEY};
use crate::app::msg::{Field, Route};
use crate::app::state::{PreviewState, ToastKind};
use crate::config::Settings;
use crate::i18n::{t, tf};
use crate::model::{FaxId, FaxRecord, LogLevel, normalize_number, normalize_tags};
use chrono::{Local, TimeDelta};
use std::path::Path;

/// Tag separators accepted in the tags field (ASCII and full-width comma).
const TAG_SEPARATORS: [char; 2] = [',', '，'];

/// Split the tags text into normalised tags.
pub(super) fn parse_tags(text: &str) -> Vec<String> {
    normalize_tags(text.split(|c: char| TAG_SEPARATORS.contains(&c)))
}

/// Delete a file; a file that is already gone counts as deleted.
pub(super) fn remove_file_if_exists(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Delete an archived file during trash cleanup. A missing file counts as deleted
/// only while the archive root is reachable; otherwise it may sit on an offline drive.
fn remove_archived_file(path: &Path, archive_root: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && archive_root.is_dir() => {
            Ok(())
        }
        Err(error) => Err(error),
    }
}

impl Controller {
    /// Fetch one record. A store error is reported; a missing record gives `None`.
    pub(super) fn load_fax(&mut self, id: FaxId) -> Option<FaxRecord> {
        self.or_report(self.store.get_fax(id)).flatten()
    }

    /// Display name used in messages: sender name, else number, else "unknown sender".
    pub(super) fn fax_name(&self, record: &FaxRecord) -> String {
        record
            .display_sender()
            .map(str::to_owned)
            .unwrap_or_else(|| t(self.state.lang, "app.unknown_sender").to_owned())
    }

    /// Select a fax: mark it read, prime the edit fields and load page 0 of its preview.
    pub(super) fn select_fax(&mut self, id: FaxId) {
        let Some(mut record) = self.load_fax(id) else {
            self.clear_selection();
            return;
        };
        let became_read =
            !record.is_read && self.or_report(self.store.set_read(id, true)).is_some();
        if became_read {
            record.is_read = true;
        }
        self.state.fields.insert(
            Field::FaxSenderName,
            record.sender_name.clone().unwrap_or_default(),
        );
        self.state
            .fields
            .insert(Field::FaxTags, record.tags.join(", "));
        self.state
            .fields
            .insert(Field::FaxNote, record.note.clone());
        self.state.selected = Some(record);
        let zoom = self.state.preview.zoom;
        self.state.preview = PreviewState {
            zoom,
            ..PreviewState::default()
        };
        if became_read {
            self.patch_fax(id, |fax| fax.is_read = true);
            self.refresh_counts();
        }
        self.load_preview_page(0);
    }

    /// Apply `change` to the row in the results and to the selected record.
    pub(super) fn patch_fax(&mut self, id: FaxId, change: impl Fn(&mut FaxRecord)) {
        for item in self
            .state
            .results
            .items
            .iter_mut()
            .filter(|item| item.id == id)
        {
            change(item);
        }
        if let Some(selected) = self
            .state
            .selected
            .as_mut()
            .filter(|record| record.id == id)
        {
            change(selected);
        }
    }

    pub(super) fn clear_selection(&mut self) {
        self.state.selected = None;
        self.clear_preview();
    }

    /// Drop the preview state and its image (the zoom level is kept).
    pub(super) fn clear_preview(&mut self) {
        self.state.preview = PreviewState {
            zoom: self.state.preview.zoom,
            ..PreviewState::default()
        };
        self.images.remove(PREVIEW_IMAGE_KEY);
    }

    pub(super) fn close_detail(&mut self) {
        self.clear_selection();
    }

    pub(super) fn toggle_read(&mut self, id: FaxId) {
        let Some(record) = self.load_fax(id) else {
            return;
        };
        let read = !record.is_read;
        if self.or_report(self.store.set_read(id, read)).is_none() {
            return;
        }
        self.patch_fax(id, |fax| fax.is_read = read);
        self.refresh_counts();
    }

    pub(super) fn toggle_star(&mut self, id: FaxId) {
        let Some(record) = self.load_fax(id) else {
            return;
        };
        let starred = !record.is_starred;
        if self
            .or_report(self.store.set_starred(id, starred))
            .is_none()
        {
            return;
        }
        self.patch_fax(id, |fax| fax.is_starred = starred);
        self.refresh_counts();
    }

    pub(super) fn mark_all_read(&mut self) {
        let lang = self.state.lang;
        let Some(count) = self.or_report(self.store.mark_all_read()) else {
            return;
        };
        let count_text = count.to_string();
        self.toast(
            ToastKind::Success,
            tf(lang, "app.marked_all_read", &[("count", &count_text)]),
        );
        self.refresh_results();
    }

    /// Load a fax and confirm its archived file exists; otherwise report it missing.
    fn existing_fax(&mut self, id: FaxId) -> Option<FaxRecord> {
        let record = self.load_fax(id)?;
        if record.file_path.is_file() {
            return Some(record);
        }
        let path = record.file_path.display().to_string();
        let text = tf(self.state.lang, "app.file_missing", &[("path", &path)]);
        self.report_error(text);
        None
    }

    pub(super) fn open_external(&mut self, id: FaxId) {
        let Some(record) = self.existing_fax(id) else {
            return;
        };
        if let Err(error) = self.platform.open_path(&record.file_path) {
            let text = tf(
                self.state.lang,
                "app.open_failed",
                &[("error", &format!("{error:#}"))],
            );
            self.report_error(text);
        }
    }

    pub(super) fn print_fax(&mut self, id: FaxId) {
        let lang = self.state.lang;
        let Some(record) = self.existing_fax(id) else {
            return;
        };
        match self.platform.print_file(&record.file_path) {
            Ok(()) => {
                let name = self.fax_name(&record);
                self.log(
                    LogLevel::Info,
                    &tf(lang, "app.log_printed", &[("name", &name)]),
                );
                self.toast(
                    ToastKind::Success,
                    tf(lang, "app.print_sent", &[("name", &name)]),
                );
            }
            Err(error) => {
                let text = tf(lang, "app.print_error", &[("error", &format!("{error:#}"))]);
                self.report_error(text);
            }
        }
    }

    pub(super) fn reveal_in_folder(&mut self, id: FaxId) {
        let Some(record) = self.existing_fax(id) else {
            return;
        };
        if let Err(error) = self.platform.reveal_in_folder(&record.file_path) {
            let text = tf(
                self.state.lang,
                "app.reveal_failed",
                &[("error", &format!("{error:#}"))],
            );
            self.report_error(text);
        }
    }

    pub(super) fn move_to_trash(&mut self, id: FaxId) {
        let lang = self.state.lang;
        let Some(record) = self.load_fax(id) else {
            return;
        };
        if self
            .or_report(self.store.move_to_trash(id, Local::now()))
            .is_none()
        {
            return;
        }
        if self
            .state
            .selected
            .as_ref()
            .is_some_and(|selected| selected.id == id)
        {
            self.clear_selection();
        }
        let name = self.fax_name(&record);
        self.log(
            LogLevel::Info,
            &tf(lang, "app.log_trashed", &[("name", &name)]),
        );
        self.toast(ToastKind::Success, t(lang, "app.moved_to_trash").to_owned());
        self.refresh_all();
    }

    pub(super) fn restore(&mut self, id: FaxId) {
        let lang = self.state.lang;
        let Some(record) = self.load_fax(id) else {
            return;
        };
        if self.or_report(self.store.restore(id)).is_none() {
            return;
        }
        let name = self.fax_name(&record);
        self.log(
            LogLevel::Info,
            &tf(lang, "app.log_restored", &[("name", &name)]),
        );
        self.toast(ToastKind::Success, t(lang, "app.restored").to_owned());
        self.refresh_all();
    }

    /// Remove the index row, then the archived file (a missing file is fine).
    /// Returns a localised error text on failure.
    pub(super) fn purge_record(&self, record: &FaxRecord) -> Result<(), String> {
        let lang = self.state.lang;
        self.store
            .delete_permanently(record.id)
            .map_err(|error| tf(lang, "app.store_error", &[("error", &error.to_string())]))?;
        remove_file_if_exists(&record.file_path).map_err(|error| {
            tf(
                lang,
                "app.file_delete_failed",
                &[("error", &error.to_string())],
            )
        })
    }

    /// Confirmed permanent delete of one trashed (or any) fax.
    pub(super) fn delete_fax_permanently(&mut self, id: FaxId) {
        let lang = self.state.lang;
        let Some(record) = self.load_fax(id) else {
            self.refresh_all();
            return;
        };
        match self.purge_record(&record) {
            Ok(()) => {
                if self
                    .state
                    .selected
                    .as_ref()
                    .is_some_and(|selected| selected.id == id)
                {
                    self.clear_selection();
                }
                let name = self.fax_name(&record);
                self.log(
                    LogLevel::Info,
                    &tf(lang, "app.log_deleted", &[("name", &name)]),
                );
                self.toast(
                    ToastKind::Success,
                    t(lang, "app.deleted_forever").to_owned(),
                );
            }
            Err(text) => self.report_error(text),
        }
        self.refresh_all();
    }

    /// Save the sender name, tags and note of the selected fax.
    pub(super) fn save_fax_edits(&mut self, id: FaxId) {
        let lang = self.state.lang;
        let sender = self.state.field(Field::FaxSenderName).trim().to_owned();
        let sender = (!sender.is_empty()).then_some(sender);
        let tags = parse_tags(self.state.field(Field::FaxTags));
        let note = self.state.field(Field::FaxNote).trim().to_owned();
        if self.load_fax(id).is_none() {
            return;
        }
        let saved = self
            .or_report(self.store.set_sender_name(id, sender.as_deref()))
            .is_some()
            && self.or_report(self.store.set_tags(id, &tags)).is_some()
            && self.or_report(self.store.set_note(id, &note)).is_some();
        if saved {
            self.toast(ToastKind::Success, t(lang, "app.edits_saved").to_owned());
        }
        self.refresh_all();
    }

    /// Open the address-book editor for the fax's sender: edit the existing contact
    /// with that number, or start a new one with the sender prefilled.
    pub(super) fn add_sender_to_contacts(&mut self, id: FaxId) {
        let lang = self.state.lang;
        let Some(record) = self.load_fax(id) else {
            return;
        };
        let Some(number) = record.sender_number.clone() else {
            self.report_error(t(lang, "app.no_sender_number").to_owned());
            return;
        };
        let normalized = normalize_number(&number).unwrap_or(number);
        let Some(existing) = self.or_report(self.store.contact_by_number(&normalized)) else {
            return;
        };
        self.navigate(Route::Contacts);
        match existing {
            Some(contact) => self.edit_contact(contact.id),
            None => {
                self.clear_contact_fields();
                self.state.fields.insert(
                    Field::ContactName,
                    record.sender_name.clone().unwrap_or_default(),
                );
                self.state.fields.insert(Field::ContactFax, normalized);
                self.state.editing_contact = Some(0);
            }
        }
    }

    /// Startup housekeeping: purge trash older than the retention period.
    pub(super) fn purge_expired_trash(&mut self) {
        let lang = self.state.lang;
        let Some(cutoff) = trash_cutoff(&self.state.settings) else {
            return;
        };
        let Some(expired) = self.or_report(self.store.trashed_before(cutoff)) else {
            return;
        };
        if expired.is_empty() {
            return;
        }
        let archive_root = self.state.settings.archive_root.clone();
        if !archive_root.is_dir() {
            // Dropping index rows now would orphan files on an offline drive.
            self.log(LogLevel::Warn, t(lang, "app.purge_skipped_offline"));
            return;
        }
        let mut purged = 0u64;
        for record in expired {
            if let Err(error) = remove_archived_file(&record.file_path, &archive_root) {
                let text = tf(
                    lang,
                    "app.file_delete_failed",
                    &[("error", &error.to_string())],
                );
                self.log(LogLevel::Warn, &text);
                continue;
            }
            if self
                .or_report(self.store.delete_permanently(record.id))
                .is_some()
            {
                purged += 1;
            }
        }
        if purged > 0 {
            let count = purged.to_string();
            self.log(
                LogLevel::Info,
                &tf(lang, "app.log_trash_purged", &[("count", &count)]),
            );
        }
    }
}

/// Trash entries deleted before this instant are expired (`None` = never purge).
fn trash_cutoff(settings: &Settings) -> Option<chrono::DateTime<Local>> {
    if settings.trash_retention_days == 0 {
        return None;
    }
    let delta = TimeDelta::try_days(i64::from(settings.trash_retention_days))?;
    Local::now().checked_sub_signed(delta)
}

#[cfg(test)]
mod tests {
    use super::parse_tags;

    #[test]
    fn parse_tags_accepts_ascii_and_full_width_commas() {
        assert_eq!(
            parse_tags("Invoice， urgent, invoice,  "),
            vec!["invoice".to_string(), "urgent".to_string()]
        );
        assert!(parse_tags("  ，， ").is_empty());
    }
}
