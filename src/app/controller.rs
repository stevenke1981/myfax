//! Application controller: owns the store, settings, watcher and preview image;
//! applies `Msg`s to `AppState`. Owner: controller worker. Public API = contract.
//!
//! Behaviour is split by concern into private submodules under `controller/`;
//! every submodule adds `impl Controller` methods and reports failures through
//! `feedback` (toast + activity log + tracing).

mod confirm;
mod contacts;
mod date_range;
mod date_tree;
mod fax_actions;
mod feedback;
mod fields;
mod import;
mod inbox;
mod nav;
mod preview;
mod refresh;
mod send;
mod settings;
mod stats;
mod watch;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_contacts_send;
#[cfg(test)]
mod tests_faxes;
#[cfg(test)]
mod tests_inbox;
#[cfg(test)]
mod tests_preview;
#[cfg(test)]
mod tests_settings;
#[cfg(test)]
mod tests_watch;

use super::msg::{Field, Msg};
use super::platform::Platform;
use super::state::{AppState, WatcherStatus};
use crate::archive::watcher::WatcherHandle;
use crate::config::{self, Settings};
use crate::i18n::{t, tf};
use crate::model::LogLevel;
use crate::store::Store;
use crate::ui::runtime::UiEvent;
use acme_core::Rgba8Image;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

/// Wake callback shared with the watcher thread.
pub type WakeFn = Arc<dyn Fn() + Send + Sync + 'static>;

/// Image slot key for the detail-pane page preview.
pub const PREVIEW_IMAGE_KEY: &str = "img:preview";

pub struct Controller {
    pub state: AppState,
    store: Store,
    platform: Box<dyn Platform>,
    images: HashMap<String, Rgba8Image>,
    settings_path: PathBuf,
    db_path: PathBuf,
    watcher: Option<WatcherHandle>,
    /// Last wake callback, reused when the watcher restarts after a settings change.
    wake: Option<WakeFn>,
    preview_revision: u64,
    /// Explicit expand/collapse choices for year nodes; others default to "current year open".
    year_expanded: HashMap<i32, bool>,
    /// When the visible toast appeared (drives auto-hide).
    toast_started: Option<Instant>,
    /// `Some(n)` while a manual scan is pending: `n` faxes imported since it was requested.
    manual_scan: Option<u64>,
}

impl Controller {
    /// Production start-up: load settings (corrupt → defaults + warn log), create
    /// app-data dir, open store at `db_path`, purge expired trash, load lists.
    pub fn new(
        settings_path: PathBuf,
        db_path: PathBuf,
        platform: Box<dyn Platform>,
    ) -> anyhow::Result<Self> {
        ensure_parent_dirs(&[&settings_path, &db_path])?;
        let (settings, load_error) = match config::load_settings(&settings_path) {
            Ok(settings) => (settings, None),
            Err(error) => (Settings::default(), Some(error)),
        };
        let store = Store::open(&db_path)?;
        let mut controller = Self::assemble(settings, store, platform, settings_path, db_path);
        let lang = controller.state.lang;
        if let Some(error) = load_error {
            let text = tf(
                lang,
                "app.settings_load_failed",
                &[("error", &format!("{error:#}"))],
            );
            controller.log(LogLevel::Warn, &text);
        }
        controller.log(LogLevel::Info, t(lang, "app.log_started"));
        controller.purge_expired_trash();
        controller.refresh_all();
        Ok(controller)
    }

    /// Test / embedding constructor: no file loading, no watcher.
    pub fn with_store(
        settings: Settings,
        store: Store,
        platform: Box<dyn Platform>,
        settings_path: PathBuf,
        db_path: PathBuf,
    ) -> Self {
        let mut controller = Self::assemble(settings, store, platform, settings_path, db_path);
        controller.purge_expired_trash();
        controller.refresh_all();
        controller
    }

    fn assemble(
        settings: Settings,
        store: Store,
        platform: Box<dyn Platform>,
        settings_path: PathBuf,
        db_path: PathBuf,
    ) -> Self {
        let mut controller = Self {
            state: AppState::new(settings),
            store,
            platform,
            images: HashMap::new(),
            settings_path,
            db_path,
            watcher: None,
            wake: None,
            preview_revision: 0,
            year_expanded: HashMap::new(),
            toast_started: None,
            manual_scan: None,
        };
        controller.prime_settings_fields();
        controller
    }

    /// (Re)start the folder watcher with current settings. No-op without folders.
    pub fn start_watcher(&mut self, wake: WakeFn) {
        self.wake = Some(Arc::clone(&wake));
        self.restart_watcher(wake);
    }

    /// Apply one user intent.
    pub fn update(&mut self, msg: Msg) {
        if self.state.confirm.is_some() && !confirm_passthrough(&msg) {
            return;
        }
        let Some(msg) = self.update_nav(msg) else {
            return;
        };
        let Some(msg) = self.update_confirm(msg) else {
            return;
        };
        let Some(msg) = self.update_inbox(msg) else {
            return;
        };
        let Some(msg) = self.update_fax(msg) else {
            return;
        };
        let Some(msg) = self.update_import(msg) else {
            return;
        };
        let Some(msg) = self.update_contacts(msg) else {
            return;
        };
        let Some(msg) = self.update_send(msg) else {
            return;
        };
        let Some(msg) = self.update_settings(msg) else {
            return;
        };
        let Some(msg) = self.update_preview(msg) else {
            return;
        };
        tracing::debug!(?msg, "message without handler");
    }

    /// Route a runtime event: `Message` → `update`; `TextChanged` → `set_field`
    /// (unknown keys ignored); `Submit` on Search → re-query, on ImportPath →
    /// `ImportFromField`, on DateFrom/DateTo → `ApplyCustomRange`,
    /// on ContactSearch → reload contacts; `Cancel` → close confirm dialog /
    /// contact editor.
    pub fn on_ui_event(&mut self, event: UiEvent<Msg>) {
        match event {
            UiEvent::Message(msg) => self.update(msg),
            UiEvent::TextChanged { key, value } => {
                if let Some(field) = Field::from_key(&key) {
                    self.set_field(field, value);
                }
            }
            UiEvent::Submit { key } => {
                if let Some(field) = Field::from_key(&key) {
                    self.submit_field(field);
                }
            }
            UiEvent::Cancel => self.cancel(),
        }
    }

    /// Store a text field. Search and ContactSearch re-query immediately.
    pub fn set_field(&mut self, field: Field, value: String) {
        self.apply_field_text(field, value);
    }

    /// Drain watcher events (refresh lists, banner, auto-print). Returns true
    /// when the state changed.
    pub fn poll_background(&mut self) -> bool {
        self.drain_watcher()
    }

    /// Files dropped on the window: import each (folders: their top-level files).
    pub fn files_dropped(&mut self, paths: Vec<PathBuf>) {
        if !paths.is_empty() {
            self.import_paths(paths);
        }
    }

    /// Periodic housekeeping (auto-hide toast after 5 s). Returns redraw flag.
    pub fn tick(&mut self) -> bool {
        self.tick_at(Instant::now())
    }

    /// Images for `img:*` slots (currently only [`PREVIEW_IMAGE_KEY`]).
    pub fn images(&self) -> &HashMap<String, Rgba8Image> {
        &self.images
    }

    /// Stop the watcher (window closing).
    pub fn shutdown(&mut self) {
        self.stop_watcher();
        self.state.watcher = WatcherStatus::Stopped;
    }

    fn update_nav(&mut self, msg: Msg) -> Option<Msg> {
        match msg {
            Msg::Navigate(route) => self.navigate(route),
            Msg::Browse(field) => self.browse(field),
            Msg::DismissToast => self.dismiss_toast(),
            Msg::DismissBanner => self.state.new_fax_banner = 0,
            other => return Some(other),
        }
        None
    }

    fn update_confirm(&mut self, msg: Msg) -> Option<Msg> {
        match msg {
            Msg::AskDeletePermanently(id) => self.ask_delete_permanently(id),
            Msg::AskEmptyTrash => self.ask_empty_trash(),
            Msg::AskDeleteContact(id) => self.ask_delete_contact(id),
            Msg::AskClearLog => self.ask_clear_log(),
            Msg::ConfirmYes => self.confirm_yes(),
            Msg::ConfirmNo => self.confirm_no(),
            other => return Some(other),
        }
        None
    }

    fn update_inbox(&mut self, msg: Msg) -> Option<Msg> {
        match msg {
            Msg::SelectFolder(folder) => self.select_folder(folder),
            Msg::SetDatePreset(preset) => self.set_date_preset(preset),
            Msg::ApplyCustomRange => self.apply_custom_range(),
            Msg::ToggleYear(year) => self.toggle_year(year),
            Msg::SelectYear(year) => self.select_year(year),
            Msg::SelectMonth { year, month } => self.select_month(year, month),
            Msg::SelectDay(day) => self.select_day(day),
            Msg::SetTagFilter(tag) => self.set_tag_filter(tag),
            Msg::SetSort(sort) => self.set_sort(sort),
            Msg::ClearFilters => self.clear_filters(),
            Msg::PrevPage => self.prev_page(),
            Msg::NextPage => self.next_page(),
            Msg::SetStatsRange(range) => self.set_stats_range(range),
            other => return Some(other),
        }
        None
    }

    fn update_fax(&mut self, msg: Msg) -> Option<Msg> {
        match msg {
            Msg::SelectFax(id) => self.select_fax(id),
            Msg::CloseDetail => self.close_detail(),
            Msg::ToggleRead(id) => self.toggle_read(id),
            Msg::ToggleStar(id) => self.toggle_star(id),
            Msg::MarkAllRead => self.mark_all_read(),
            Msg::OpenExternal(id) => self.open_external(id),
            Msg::Print(id) => self.print_fax(id),
            Msg::RevealInFolder(id) => self.reveal_in_folder(id),
            Msg::MoveToTrash(id) => self.move_to_trash(id),
            Msg::Restore(id) => self.restore(id),
            Msg::SaveFaxEdits(id) => self.save_fax_edits(id),
            Msg::AddSenderToContacts(id) => self.add_sender_to_contacts(id),
            other => return Some(other),
        }
        None
    }

    fn update_import(&mut self, msg: Msg) -> Option<Msg> {
        match msg {
            Msg::ImportFromField => self.import_from_field(),
            Msg::ExportFiltered => self.export_filtered(),
            Msg::ScanNow => self.scan_now(),
            other => return Some(other),
        }
        None
    }

    fn update_contacts(&mut self, msg: Msg) -> Option<Msg> {
        match msg {
            Msg::NewContact => self.new_contact(),
            Msg::EditContact(id) => self.edit_contact(id),
            Msg::SaveContact => self.save_contact(),
            Msg::CancelContactEdit => self.state.editing_contact = None,
            Msg::ImportContactsCsv => self.import_contacts_csv(),
            Msg::ExportContactsCsv => self.export_contacts_csv(),
            other => return Some(other),
        }
        None
    }

    fn update_send(&mut self, msg: Msg) -> Option<Msg> {
        match msg {
            Msg::RefreshPrinters => self.refresh_printers(true),
            Msg::SelectPrinter(name) => self.select_printer(name),
            Msg::UseContactForSend(id) => self.use_contact_for_send(id),
            Msg::SendFax => self.send_fax(),
            other => return Some(other),
        }
        None
    }

    fn update_settings(&mut self, msg: Msg) -> Option<Msg> {
        match msg {
            Msg::SetLanguage(lang) => self.set_language(lang),
            Msg::SetTheme(theme) => self.set_theme(theme),
            Msg::AddWatchFolder => self.add_watch_folder(),
            Msg::RemoveWatchFolder(index) => self.remove_watch_folder(index),
            Msg::ToggleSetting(setting) => self.toggle_setting(setting),
            Msg::SaveSettings => self.save_settings(),
            Msg::OpenArchiveFolder => self.open_archive_folder(),
            other => return Some(other),
        }
        None
    }

    fn update_preview(&mut self, msg: Msg) -> Option<Msg> {
        match msg {
            Msg::PreviewPrevPage => self.preview_step(-1),
            Msg::PreviewNextPage => self.preview_step(1),
            Msg::ZoomIn => self.zoom_step(1),
            Msg::ZoomOut => self.zoom_step(-1),
            Msg::ZoomReset => self.reset_zoom(),
            other => return Some(other),
        }
        None
    }
}

/// Messages still accepted while a confirm dialog is open.
fn confirm_passthrough(msg: &Msg) -> bool {
    matches!(msg, Msg::ConfirmYes | Msg::ConfirmNo | Msg::DismissToast)
}

/// Create the parent directories of every given path (empty parents are skipped).
fn ensure_parent_dirs(paths: &[&Path]) -> anyhow::Result<()> {
    for path in paths {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}
