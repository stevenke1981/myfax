//! Settings page: language, theme, watch folders, toggles, text-field settings.

use super::Controller;
use super::folders::is_same_or_inside;
use crate::app::msg::{BoolSetting, Field};
use crate::app::state::ToastKind;
use crate::config::{self, Settings, ThemeChoice};
use crate::i18n::{Lang, t, tf};
use crate::model::LogLevel;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Seconds between folder polls.
const POLL_SECS_RANGE: RangeInclusive<u64> = 2..=3600;
/// Days a trash entry is kept (0 = forever).
const TRASH_DAYS_RANGE: RangeInclusive<u32> = 0..=3650;
/// Items per inbox page.
const PAGE_SIZE_RANGE: RangeInclusive<u32> = 10..=500;

/// Which text-field setting failed validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsIssue {
    Archive,
    Poll,
    Trash,
    Page,
    WatchInsideArchive,
    WatchIsForward,
}

/// Parse `text` (trimmed) and accept it only inside `range`.
fn parse_in_range<T>(text: &str, range: RangeInclusive<T>) -> Option<T>
where
    T: FromStr + PartialOrd + Copy,
{
    text.trim()
        .parse::<T>()
        .ok()
        .filter(|value| range.contains(value))
}

/// Localised reason shown in the "settings not saved" toast.
fn issue_text(lang: Lang, issue: SettingsIssue) -> &'static str {
    match issue {
        SettingsIssue::Archive => t(lang, "app.settings_invalid_archive"),
        SettingsIssue::Poll => t(lang, "app.settings_invalid_poll"),
        SettingsIssue::Trash => t(lang, "app.settings_invalid_trash"),
        SettingsIssue::Page => t(lang, "app.settings_invalid_page"),
        SettingsIssue::WatchInsideArchive => t(lang, "app.watch_inside_archive"),
        SettingsIssue::WatchIsForward => t(lang, "app.watch_is_forward"),
    }
}

/// The watch-folder rule a folder breaks under `settings`, if any: a watch folder may
/// not be the archive root or inside it, nor the copy folder or inside it.
fn watch_conflict(settings: &Settings, folder: &Path) -> Option<SettingsIssue> {
    if is_same_or_inside(folder, &settings.archive_root) {
        return Some(SettingsIssue::WatchInsideArchive);
    }
    let forward = settings.forward_copy_folder.as_deref()?;
    is_same_or_inside(folder, forward).then_some(SettingsIssue::WatchIsForward)
}

/// First conflict between any watch folder and the archive / copy folder of `settings`.
fn settings_conflict(settings: &Settings) -> Option<SettingsIssue> {
    settings
        .watch_folders
        .iter()
        .find_map(|folder| watch_conflict(settings, folder))
}

impl Controller {
    /// Copy persisted settings into the text fields (start-up and after every save).
    pub(super) fn prime_settings_fields(&mut self) {
        let settings = &self.state.settings;
        let values = [
            (
                Field::ArchiveRoot,
                settings.archive_root.display().to_string(),
            ),
            (Field::PollInterval, settings.poll_interval_secs.to_string()),
            (
                Field::TrashRetention,
                settings.trash_retention_days.to_string(),
            ),
            (Field::PageSize, settings.page_size.to_string()),
            (
                Field::ForwardFolder,
                settings
                    .forward_copy_folder
                    .as_ref()
                    .map(|folder| folder.display().to_string())
                    .unwrap_or_default(),
            ),
        ];
        for (field, value) in values {
            self.state.fields.insert(field, value);
        }
    }

    /// Apply `next` to the state at once, then persist it. Returns false when saving
    /// failed (the failure is already reported).
    pub(super) fn commit_settings(&mut self, next: Settings) -> bool {
        self.state.lang = next.language;
        self.state.theme = next.theme;
        self.state.query.page_size = next.page_size;
        self.state.settings = next;
        match config::save_settings(&self.settings_path, &self.state.settings) {
            Ok(()) => true,
            Err(error) => {
                let text = tf(
                    self.state.lang,
                    "app.settings_save_failed",
                    &[("error", &format!("{error:#}"))],
                );
                self.report_error(text);
                false
            }
        }
    }

    pub(super) fn set_language(&mut self, lang: Lang) {
        let mut next = self.state.settings.clone();
        next.language = lang;
        self.commit_settings(next);
    }

    pub(super) fn set_theme(&mut self, theme: ThemeChoice) {
        let mut next = self.state.settings.clone();
        next.theme = theme;
        self.commit_settings(next);
    }

    pub(super) fn toggle_setting(&mut self, setting: BoolSetting) {
        let mut next = self.state.settings.clone();
        let flag = match setting {
            BoolSetting::DeleteSourceAfterImport => &mut next.delete_source_after_import,
            BoolSetting::AcceptTiff => &mut next.accept_tiff,
            BoolSetting::AutoPrint => &mut next.auto_print,
            BoolSetting::NotifyNewFax => &mut next.notify_new_fax,
        };
        *flag = !*flag;
        // The watcher thread keeps the settings it was spawned with; these two are
        // read there, so it must be restarted. AutoPrint / NotifyNewFax are read by
        // the controller itself and take effect immediately.
        let read_by_watcher = matches!(
            setting,
            BoolSetting::DeleteSourceAfterImport | BoolSetting::AcceptTiff
        );
        self.commit_settings(next);
        if read_by_watcher {
            self.restart_watcher_from_state();
        }
    }

    /// Add a watch folder (must exist and not already be listed), then restart the watcher.
    pub(super) fn add_watch_folder(&mut self) {
        let lang = self.state.lang;
        let text = self.state.field(Field::WatchFolderInput).trim().to_owned();
        if text.is_empty() {
            self.report_error(t(lang, "app.watch_empty").to_owned());
            return;
        }
        let path = PathBuf::from(&text);
        if !path.is_absolute() {
            self.report_error(t(lang, "app.watch_relative").to_owned());
            return;
        }
        if !path.is_dir() {
            self.report_error(tf(lang, "app.watch_missing", &[("path", &text)]));
            return;
        }
        if self.state.settings.watch_folders.contains(&path) {
            self.toast(ToastKind::Info, t(lang, "app.watch_duplicate").to_owned());
            return;
        }
        if let Some(issue) = watch_conflict(&self.state.settings, &path) {
            self.report_error(issue_text(lang, issue).to_owned());
            return;
        }
        let mut next = self.state.settings.clone();
        next.watch_folders.push(path.clone());
        self.commit_settings(next);
        self.state
            .fields
            .insert(Field::WatchFolderInput, String::new());
        self.log(
            LogLevel::Info,
            &tf(
                lang,
                "app.log_watch_added",
                &[("path", &path.display().to_string())],
            ),
        );
        self.restart_watcher_from_state();
    }

    /// Remove the watch folder at `index` (ignored when out of range).
    pub(super) fn remove_watch_folder(&mut self, index: usize) {
        if index >= self.state.settings.watch_folders.len() {
            return;
        }
        let lang = self.state.lang;
        let mut next = self.state.settings.clone();
        let removed = next.watch_folders.remove(index);
        self.commit_settings(next);
        self.log(
            LogLevel::Info,
            &tf(
                lang,
                "app.log_watch_removed",
                &[("path", &removed.display().to_string())],
            ),
        );
        self.restart_watcher_from_state();
    }

    /// Validate the text fields. Nothing is saved unless every field is valid.
    fn settings_from_fields(&self) -> Result<Settings, SettingsIssue> {
        let archive = self.state.field(Field::ArchiveRoot).trim();
        if archive.is_empty() {
            return Err(SettingsIssue::Archive);
        }
        let poll = parse_in_range(self.state.field(Field::PollInterval), POLL_SECS_RANGE)
            .ok_or(SettingsIssue::Poll)?;
        let trash = parse_in_range(self.state.field(Field::TrashRetention), TRASH_DAYS_RANGE)
            .ok_or(SettingsIssue::Trash)?;
        let page = parse_in_range(self.state.field(Field::PageSize), PAGE_SIZE_RANGE)
            .ok_or(SettingsIssue::Page)?;
        let forward = self.state.field(Field::ForwardFolder).trim();
        let mut next = self.state.settings.clone();
        next.archive_root = PathBuf::from(archive);
        next.poll_interval_secs = poll;
        next.trash_retention_days = trash;
        next.page_size = page;
        next.forward_copy_folder = (!forward.is_empty()).then(|| PathBuf::from(forward));
        match settings_conflict(&next) {
            Some(issue) => Err(issue),
            None => Ok(next),
        }
    }

    /// Validate and persist the text-field settings.
    pub(super) fn save_settings(&mut self) {
        let lang = self.state.lang;
        let next = match self.settings_from_fields() {
            Ok(next) => next,
            Err(issue) => {
                let reason = issue_text(lang, issue);
                self.report_error(tf(lang, "app.settings_invalid", &[("reason", reason)]));
                return;
            }
        };
        let saved = self.commit_settings(next);
        self.prime_settings_fields();
        self.refresh_results();
        self.restart_watcher_from_state();
        if saved {
            self.toast(ToastKind::Success, t(lang, "app.settings_saved").to_owned());
            self.log(LogLevel::Info, t(lang, "app.log_settings_saved"));
        }
    }

    /// Open the archive root in Explorer (created first when missing).
    pub(super) fn open_archive_folder(&mut self) {
        let lang = self.state.lang;
        let root = self.state.settings.archive_root.clone();
        if !root.is_dir()
            && let Err(error) = std::fs::create_dir_all(&root)
        {
            let text = tf(lang, "app.open_failed", &[("error", &error.to_string())]);
            self.report_error(text);
            return;
        }
        if let Err(error) = self.platform.open_path(&root) {
            let text = tf(lang, "app.open_failed", &[("error", &format!("{error:#}"))]);
            self.report_error(text);
        }
    }
}
