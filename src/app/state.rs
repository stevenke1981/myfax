//! View-model read by `ui::views` and written only by the controller
//! (`app::controller`). Orchestrator-owned contract.

use super::msg::{ConfirmAction, DatePreset, Field, Route, StatsRange};
use crate::config::{Settings, ThemeChoice};
use crate::i18n::Lang;
use crate::model::{
    Contact, ContactId, FaxQuery, FaxRecord, FolderCounts, LogEntry, SearchPage, SentFax, Stats,
};
use chrono::{DateTime, Local};
use std::collections::BTreeMap;

/// Day-level node of the date tree.
#[derive(Clone, Debug, PartialEq)]
pub struct DayNode {
    pub day: u32,
    pub count: u64,
}

/// Month-level node of the date tree.
#[derive(Clone, Debug, PartialEq)]
pub struct MonthNode {
    pub month: u32,
    pub count: u64,
    /// Ascending days. Only shown when this month is `AppState::expanded_month`.
    pub days: Vec<DayNode>,
}

/// Year-level node of the date tree (newest year first in `AppState::date_tree`).
#[derive(Clone, Debug, PartialEq)]
pub struct YearNode {
    pub year: i32,
    pub count: u64,
    pub expanded: bool,
    /// Newest month first.
    pub months: Vec<MonthNode>,
}

/// State of the page preview in the detail pane.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum PreviewStatus {
    #[default]
    Empty,
    Ready,
    /// Preview not possible (encrypted, unsupported filter…). Text is localised.
    Unavailable(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreviewState {
    /// Zero-based page currently shown.
    pub page: u32,
    pub page_count: u32,
    /// 1.0 = fit to pane width. Clamped to 0.5..=3.0 in 0.25 steps.
    pub zoom: f32,
    pub status: PreviewStatus,
    /// Natural pixel size of the rendered page (0×0 when not ready).
    pub image_width: u32,
    pub image_height: u32,
}

impl Default for PreviewState {
    fn default() -> Self {
        Self {
            page: 0,
            page_count: 0,
            zoom: 1.0,
            status: PreviewStatus::Empty,
            image_width: 0,
            image_height: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

/// Transient message shown at the bottom of the window. Text is localised.
#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    pub kind: ToastKind,
    pub text: String,
}

/// Modal confirmation. `title` / `body` are localised.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfirmDialog {
    pub title: String,
    pub body: String,
    pub action: ConfirmAction,
}

/// Folder-watcher status shown in the status bar.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum WatcherStatus {
    #[default]
    Stopped,
    /// Watching this many folders.
    Running(usize),
    /// Localised error text.
    Error(String),
}

/// Whole UI view-model.
#[derive(Clone, Debug, PartialEq)]
pub struct AppState {
    pub lang: Lang,
    pub theme: ThemeChoice,
    pub route: Route,

    // inbox
    pub query: FaxQuery,
    pub date_preset: DatePreset,
    pub results: SearchPage,
    pub counts: FolderCounts,
    pub date_tree: Vec<YearNode>,
    pub expanded_month: Option<(i32, u32)>,
    /// (tag, count) of non-deleted faxes, most used first.
    pub all_tags: Vec<(String, u64)>,
    pub selected: Option<FaxRecord>,
    pub preview: PreviewState,

    // text fields (missing key == empty string)
    pub fields: BTreeMap<Field, String>,

    // contacts
    pub contacts: Vec<Contact>,
    /// `Some(0)` = creating a new contact, `Some(id)` = editing, `None` = not editing.
    pub editing_contact: Option<ContactId>,

    // send
    pub printers: Vec<String>,
    pub selected_printer: Option<String>,
    pub sent: Vec<SentFax>,

    // stats / log
    pub stats_range: StatsRange,
    pub stats: Stats,
    pub logs: Vec<LogEntry>,

    // settings (persisted copy; text fields are edited through `fields`)
    pub settings: Settings,

    // chrome
    pub toast: Option<Toast>,
    /// Number of faxes received since the banner was last dismissed (0 = hidden).
    pub new_fax_banner: u64,
    pub confirm: Option<ConfirmDialog>,
    pub watcher: WatcherStatus,
    pub last_scan: Option<DateTime<Local>>,
}

impl AppState {
    /// Fresh state for the given settings (nothing loaded from the store yet).
    pub fn new(settings: Settings) -> Self {
        let query = FaxQuery {
            page_size: settings.page_size,
            ..FaxQuery::default()
        };
        Self {
            lang: settings.language,
            theme: settings.theme,
            route: Route::Inbox,
            query,
            date_preset: DatePreset::AnyTime,
            results: SearchPage::default(),
            counts: FolderCounts::default(),
            date_tree: Vec::new(),
            expanded_month: None,
            all_tags: Vec::new(),
            selected: None,
            preview: PreviewState::default(),
            fields: BTreeMap::new(),
            contacts: Vec::new(),
            editing_contact: None,
            printers: Vec::new(),
            selected_printer: settings.fax_printer.clone(),
            sent: Vec::new(),
            stats_range: StatsRange::default(),
            stats: Stats::default(),
            logs: Vec::new(),
            settings,
            toast: None,
            new_fax_banner: 0,
            confirm: None,
            watcher: WatcherStatus::Stopped,
            last_scan: None,
        }
    }

    /// Current text of a field ("" when unset).
    pub fn field(&self, field: Field) -> &str {
        self.fields.get(&field).map(String::as_str).unwrap_or("")
    }

    /// Total result pages for the current query (at least 1).
    pub fn page_count(&self) -> u32 {
        let size = u64::from(self.query.page_size.max(1));
        let pages = self.results.total.div_ceil(size).max(1);
        u32::try_from(pages).unwrap_or(u32::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_count_rounds_up() {
        let mut state = AppState::new(Settings::default());
        state.query.page_size = 50;
        state.results.total = 0;
        assert_eq!(state.page_count(), 1);
        state.results.total = 101;
        assert_eq!(state.page_count(), 3);
        assert_eq!(state.field(Field::Search), "");
    }
}
