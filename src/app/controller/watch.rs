//! Folder watcher lifecycle and the events it sends back to the UI thread.

use super::{Controller, WakeFn};
use crate::app::state::{ToastKind, WatcherStatus};
use crate::archive::watcher::{WatchEvent, WatcherHandle};
use crate::i18n::tf;
use crate::model::{FaxId, LogLevel};
use chrono::Local;
use std::sync::Arc;

/// A wake callback that does nothing (used when no UI loop is attached).
fn noop_wake() -> WakeFn {
    let wake: WakeFn = Arc::new(|| {});
    wake
}

impl Controller {
    /// Stop any running watcher and start a new one for the current folders.
    pub(super) fn restart_watcher(&mut self, wake: WakeFn) {
        self.stop_watcher();
        let folders = self.state.settings.watch_folders.len();
        if folders == 0 {
            self.state.watcher = WatcherStatus::Stopped;
            return;
        }
        let lang = self.state.lang;
        let spawned = WatcherHandle::spawn(
            self.state.settings.clone(),
            self.db_path.clone(),
            Box::new(move || wake()),
        );
        match spawned {
            Ok(handle) => {
                self.watcher = Some(handle);
                self.state.watcher = WatcherStatus::Running(folders);
                let count = folders.to_string();
                self.log(
                    LogLevel::Info,
                    &tf(lang, "app.log_watcher_started", &[("count", &count)]),
                );
            }
            Err(error) => {
                let text = tf(
                    lang,
                    "app.watch_error_start",
                    &[("error", &error.to_string())],
                );
                self.log(LogLevel::Error, &text);
                self.state.watcher = WatcherStatus::Error(text);
            }
        }
    }

    /// Restart with the remembered wake callback (after a settings change).
    pub(super) fn restart_watcher_from_state(&mut self) {
        let wake = self.wake.clone().unwrap_or_else(noop_wake);
        self.restart_watcher(wake);
    }

    pub(super) fn stop_watcher(&mut self) {
        if let Some(mut handle) = self.watcher.take() {
            handle.stop();
        }
    }

    /// Handle every queued watcher event. Returns true when any event arrived.
    pub(super) fn drain_watcher(&mut self) -> bool {
        let Some(handle) = self.watcher.as_ref() else {
            return false;
        };
        let events = handle.try_events();
        if events.is_empty() {
            return false;
        }
        let mut imported = 0u64;
        let mut cycle_failed = false;
        for event in events {
            match event {
                WatchEvent::Imported { id, .. } => {
                    imported += 1;
                    if let Some(pending) = self.manual_scan.as_mut() {
                        *pending += 1;
                    }
                    self.on_watch_imported(id);
                }
                WatchEvent::Duplicate { .. } => {}
                WatchEvent::Error { error, .. } => {
                    cycle_failed = true;
                    self.on_watch_error(error);
                }
                WatchEvent::ScanCompleted => {
                    self.on_scan_completed(cycle_failed);
                    cycle_failed = false;
                }
            }
        }
        if imported > 0 {
            self.refresh_all();
            if self.state.settings.notify_new_fax {
                self.state.new_fax_banner += imported;
            }
        }
        true
    }

    /// Log a newly archived fax and auto-print it when enabled.
    fn on_watch_imported(&mut self, id: FaxId) {
        let lang = self.state.lang;
        let Some(record) = self.load_fax(id) else {
            return;
        };
        let name = self.fax_name(&record);
        let pages = record.pages.to_string();
        self.log(
            LogLevel::Info,
            &tf(
                lang,
                "app.log_new_fax",
                &[("name", &name), ("pages", &pages)],
            ),
        );
        if !self.state.settings.auto_print {
            return;
        }
        match self.platform.print_file(&record.file_path) {
            Ok(()) => self.log(
                LogLevel::Info,
                &tf(lang, "app.log_printed", &[("name", &name)]),
            ),
            Err(error) => {
                let text = tf(lang, "app.print_error", &[("error", &format!("{error:#}"))]);
                self.log(LogLevel::Warn, &text);
            }
        }
    }

    /// A folder or file failed; the status bar shows it until a clean cycle.
    fn on_watch_error(&mut self, error: String) {
        let lang = self.state.lang;
        self.log(
            LogLevel::Warn,
            &tf(lang, "app.log_watcher_error", &[("error", &error)]),
        );
        self.state.watcher = WatcherStatus::Error(error);
    }

    fn on_scan_completed(&mut self, cycle_failed: bool) {
        let lang = self.state.lang;
        self.state.last_scan = Some(Local::now());
        let recovering = !cycle_failed && matches!(self.state.watcher, WatcherStatus::Error(_));
        if recovering {
            self.state.watcher = WatcherStatus::Running(self.state.settings.watch_folders.len());
        }
        if let Some(count) = self.manual_scan.take() {
            let count = count.to_string();
            self.log(
                LogLevel::Info,
                &tf(lang, "app.log_scan_manual", &[("count", &count)]),
            );
            self.toast(
                ToastKind::Success,
                tf(lang, "app.scan_done", &[("count", &count)]),
            );
        }
    }

    /// Poll all watch folders now. Without a running watcher the scan reports zero.
    pub(super) fn scan_now(&mut self) {
        if let Some(handle) = self.watcher.as_ref() {
            handle.scan_now();
            self.manual_scan = Some(0);
            return;
        }
        let lang = self.state.lang;
        self.toast(
            ToastKind::Success,
            tf(lang, "app.scan_done", &[("count", "0")]),
        );
    }
}
