//! Application controller: owns the store, settings, watcher and preview image;
//! applies `Msg`s to `AppState`. Owner: controller worker. Public API = contract.

use super::msg::{Field, Msg};
use super::platform::Platform;
use super::state::AppState;
use crate::config::Settings;
use crate::store::Store;
use crate::ui::runtime::UiEvent;
use acme_core::Rgba8Image;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Wake callback shared with the watcher thread.
pub type WakeFn = Arc<dyn Fn() + Send + Sync + 'static>;

/// Image slot key for the detail-pane page preview.
pub const PREVIEW_IMAGE_KEY: &str = "img:preview";

pub struct Controller {
    pub state: AppState,
    _store: Store,
    _platform: Box<dyn Platform>,
    _images: HashMap<String, Rgba8Image>,
    _settings_path: PathBuf,
    _db_path: PathBuf,
}

#[allow(unused_variables)]
impl Controller {
    /// Production start-up: load settings (corrupt → defaults + warn log), create
    /// app-data dir, open store at `db_path`, purge expired trash, load lists.
    pub fn new(
        settings_path: PathBuf,
        db_path: PathBuf,
        platform: Box<dyn Platform>,
    ) -> anyhow::Result<Self> {
        todo!()
    }

    /// Test / embedding constructor: no file loading, no watcher.
    pub fn with_store(
        settings: Settings,
        store: Store,
        platform: Box<dyn Platform>,
        settings_path: PathBuf,
        db_path: PathBuf,
    ) -> Self {
        todo!()
    }

    /// (Re)start the folder watcher with current settings. No-op without folders.
    pub fn start_watcher(&mut self, wake: WakeFn) {
        todo!()
    }

    /// Apply one user intent.
    pub fn update(&mut self, msg: Msg) {
        todo!()
    }

    /// Route a runtime event: `Message` → `update`; `TextChanged` → `set_field`
    /// (unknown keys ignored); `Submit` on Search → re-query, on ImportPath →
    /// `ImportFromField`, on DateFrom/DateTo → `ApplyCustomRange`,
    /// on ContactSearch → reload contacts; `Cancel` → close confirm dialog /
    /// contact editor.
    pub fn on_ui_event(&mut self, event: UiEvent<Msg>) {
        todo!()
    }

    /// Store a text field. Search and ContactSearch re-query immediately.
    pub fn set_field(&mut self, field: Field, value: String) {
        todo!()
    }

    /// Drain watcher events (refresh lists, banner, auto-print). Returns true
    /// when the state changed.
    pub fn poll_background(&mut self) -> bool {
        todo!()
    }

    /// Files dropped on the window: import each (folders: their top-level files).
    pub fn files_dropped(&mut self, paths: Vec<PathBuf>) {
        todo!()
    }

    /// Periodic housekeeping (auto-hide toast after 5 s). Returns redraw flag.
    pub fn tick(&mut self) -> bool {
        todo!()
    }

    /// Images for `img:*` slots (currently only [`PREVIEW_IMAGE_KEY`]).
    pub fn images(&self) -> &HashMap<String, Rgba8Image> {
        &self._images
    }

    /// Stop the watcher (window closing).
    pub fn shutdown(&mut self) {
        todo!()
    }
}
