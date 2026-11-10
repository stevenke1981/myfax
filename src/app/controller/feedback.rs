//! Toasts, activity-log lines and error reporting shared by every handler.
//! Every fallible path ends here: user-visible toast + activity log + tracing.

use super::Controller;
use crate::app::state::{Toast, ToastKind};
use crate::i18n::tf;
use crate::model::LogLevel;
use crate::store::StoreResult;
use std::time::{Duration, Instant};

/// Auto-hide delay for info and success toasts (UI spec 10, rule 2).
const INFO_TOAST_TTL: Duration = Duration::from_secs(4);
/// Auto-hide delay for error toasts (UI spec 10, rule 2).
const ERROR_TOAST_TTL: Duration = Duration::from_secs(8);

impl Controller {
    /// Show a toast and (re)start its auto-hide timer.
    pub(super) fn toast(&mut self, kind: ToastKind, text: String) {
        self.state.toast = Some(Toast { kind, text });
        self.toast_started = Some(Instant::now());
    }

    /// Error toast plus an Error activity-log line carrying the same text.
    pub(super) fn report_error(&mut self, text: String) {
        self.log(LogLevel::Error, &text);
        self.toast(ToastKind::Error, text);
    }

    /// Unwrap a store result; on failure report it and return `None`.
    pub(super) fn or_report<T>(&mut self, result: StoreResult<T>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                let text = tf(
                    self.state.lang,
                    "app.store_error",
                    &[("error", &error.to_string())],
                );
                self.report_error(text);
                None
            }
        }
    }

    /// Write an activity-log line (best effort) and mirror it to tracing.
    pub(super) fn log(&self, level: LogLevel, message: &str) {
        match level {
            LogLevel::Info => tracing::info!("{message}"),
            LogLevel::Warn => tracing::warn!("{message}"),
            LogLevel::Error => tracing::error!("{message}"),
        }
        if let Err(error) = self.store.append_log(level, message) {
            tracing::warn!(%error, "cannot write activity log");
        }
    }

    pub(super) fn dismiss_toast(&mut self) {
        self.state.toast = None;
        self.toast_started = None;
    }

    /// Hide the toast once its time-to-live has passed. Returns true when it changed.
    pub(super) fn tick_at(&mut self, now: Instant) -> bool {
        let Some(started) = self.toast_started else {
            return false;
        };
        let Some(kind) = self.state.toast.as_ref().map(|toast| toast.kind) else {
            return false;
        };
        let ttl = match kind {
            ToastKind::Error => ERROR_TOAST_TTL,
            ToastKind::Info | ToastKind::Success => INFO_TOAST_TTL,
        };
        if now.saturating_duration_since(started) < ttl {
            return false;
        }
        self.dismiss_toast();
        true
    }
}
