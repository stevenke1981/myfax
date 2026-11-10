//! Activity log page (UI_SPEC §7). Owner: views worker (log).

use crate::app::{AppState, Msg};
use crate::i18n::{Lang, t, tf};
use crate::model::{LogEntry, LogLevel};
use crate::ui::views::kit::{self, Tone, glyph};
use acme_widgets::style::prelude::*;
use acme_widgets::{
    ButtonSize, ButtonVariant, LabelWrap, WidgetNode, button, column, label_builder, row,
};
use chrono::{DateTime, Local};

/// Newest entries shown; older ones are dropped from the view (not from the store).
const LOG_LIMIT: usize = 500;
const TIME_W: f32 = 152.0;
const LEVEL_W: f32 = 64.0;

/// Whole page body (right of the nav rail).
pub fn view(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let count = state.logs.len().to_string();
    let subtitle = tf(lang, "log.count", &[("count", count.as_str())]);
    let clear = button("btn.clear_log", t(lang, "log.clear"))
        .variant(ButtonVariant::Danger)
        .size(ButtonSize::Small)
        .disabled(state.logs.is_empty())
        .on_click(Msg::AskClearLog);
    let body = if state.logs.is_empty() {
        kit::empty_state(glyph::NAV_LOG, t(lang, "log.empty"), "", None)
    } else {
        list(state)
    };
    column::<Msg>()
        .key("page.log")
        .flex_1()
        .flex_shrink(1.0)
        .min_h(crate::ui::views::kit::px(0.0))
        .child(kit::page_header(
            t(lang, "log.title"),
            Some(subtitle),
            vec![clear],
        ))
        .child(body)
        .build()
}

fn list(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let mut rows = column::<Msg>()
        .key("scroll.log")
        .flex_1()
        .flex_shrink(1.0)
        .min_h(crate::ui::views::kit::px(0.0))
        .overflow_scroll()
        .px(24.0)
        .py(8.0);
    for entry in state.logs.iter().take(LOG_LIMIT) {
        rows = rows.child(log_row(lang, entry)).child(kit::hdivider());
    }
    if state.logs.len() > LOG_LIMIT {
        let limit = LOG_LIMIT.to_string();
        let notice = tf(lang, "log.truncated", &[("count", limit.as_str())]);
        rows = rows.child(kit::caption(&notice));
    }
    rows.build()
}

fn log_row(lang: Lang, entry: &LogEntry) -> WidgetNode<Msg> {
    let time = log_time(entry.at);
    row::<Msg>()
        .key(format!("log.{}", entry.id))
        .py(8.0)
        .gap(12.0)
        .child(
            label_builder::<Msg>(time)
                .font_size(12.0)
                .text_muted()
                .w(kit::px(TIME_W))
                .build(),
        )
        .child(
            column::<Msg>()
                .w(kit::px(LEVEL_W))
                .child(kit::badge(
                    level_text(lang, entry.level),
                    level_tone(entry.level),
                ))
                .build(),
        )
        .child(
            label_builder::<Msg>(entry.message.as_str())
                .font_size(14.0)
                .wrap(LabelWrap::Word)
                .flex_1()
                .flex_shrink(1.0)
                .build(),
        )
        .build()
}

/// Badge tone of a log level. `LogLevel` has no success level, so Info uses
/// the success tone (UI_SPEC §7 deviation note).
fn level_tone(level: LogLevel) -> Tone {
    match level {
        LogLevel::Info => Tone::Success,
        LogLevel::Warn => Tone::Warning,
        LogLevel::Error => Tone::Danger,
    }
}

fn level_text(lang: Lang, level: LogLevel) -> &'static str {
    match level {
        LogLevel::Info => t(lang, "log.level_info"),
        LogLevel::Warn => t(lang, "log.level_warn"),
        LogLevel::Error => t(lang, "log.level_error"),
    }
}

/// `YYYY-MM-DD HH:mm:ss` in local time.
fn log_time(at: DateTime<Local>) -> String {
    at.format("%Y-%m-%d %H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;
    use crate::ui::views::shell::test_util::{button_at, find_key, keys, messages, texts};
    use chrono::TimeZone;

    fn entry(id: i64, level: LogLevel, message: &str) -> LogEntry {
        LogEntry {
            id,
            at: Local.with_ymd_and_hms(2026, 11, 10, 9, 5, 7).unwrap(),
            level,
            message: message.into(),
        }
    }

    fn state_with(logs: Vec<LogEntry>) -> AppState {
        let mut st = AppState::new(Settings::default());
        st.logs = logs;
        st
    }

    fn numbered(count: i64) -> Vec<LogEntry> {
        (0..count)
            .map(|i| entry(i, LogLevel::Info, "tick"))
            .collect()
    }

    #[test]
    fn empty_log_disables_clear_and_shows_empty_state() {
        let root = view(&state_with(Vec::new()));
        assert!(texts(&root).contains(&t(Lang::ZhTw, "log.empty").to_string()));
        assert!(button_at(&root, "btn.clear_log").expect("clear").disabled);
        assert!(!messages(&root).contains(&Msg::AskClearLog));
        assert!(find_key(&root, "scroll.log").is_none());
    }

    #[test]
    fn log_rows_show_level_badges_and_ask_clear() {
        let root = view(&state_with(vec![
            entry(1, LogLevel::Info, "匯入完成"),
            entry(2, LogLevel::Error, "列印失敗"),
        ]));
        assert!(find_key(&root, "scroll.log").is_some());
        assert!(find_key(&root, "log.1").is_some());
        assert!(find_key(&root, "log.2").is_some());
        let labels = texts(&root);
        assert!(labels.contains(&t(Lang::ZhTw, "log.level_info").to_string()));
        assert!(labels.contains(&t(Lang::ZhTw, "log.level_error").to_string()));
        assert!(labels.contains(&"列印失敗".to_string()));
        assert!(messages(&root).contains(&Msg::AskClearLog));
        assert!(!button_at(&root, "btn.clear_log").expect("clear").disabled);
    }

    #[test]
    fn header_shows_entry_count() {
        let root = view(&state_with(numbered(2)));
        let count = tf(Lang::ZhTw, "log.count", &[("count", "2")]);
        assert!(texts(&root).contains(&count));
    }

    #[test]
    fn list_is_capped_at_500_rows_with_notice() {
        let root = view(&state_with(numbered(501)));
        let rows = keys(&root)
            .into_iter()
            .filter(|k| k.starts_with("log."))
            .count();
        assert_eq!(rows, LOG_LIMIT);
        let notice = tf(Lang::ZhTw, "log.truncated", &[("count", "500")]);
        assert!(texts(&root).contains(&notice));
    }

    #[test]
    fn list_at_exactly_limit_has_no_notice() {
        let root = view(&state_with(numbered(500)));
        let notice = tf(Lang::ZhTw, "log.truncated", &[("count", "500")]);
        assert!(!texts(&root).contains(&notice));
    }

    #[test]
    fn level_tone_maps_each_level() {
        assert_eq!(level_tone(LogLevel::Info), Tone::Success);
        assert_eq!(level_tone(LogLevel::Warn), Tone::Warning);
        assert_eq!(level_tone(LogLevel::Error), Tone::Danger);
    }

    #[test]
    fn level_text_follows_language() {
        assert_eq!(level_text(Lang::En, LogLevel::Warn), "Warning");
        assert_eq!(level_text(Lang::ZhTw, LogLevel::Error), "錯誤");
    }

    #[test]
    fn log_time_has_seconds_in_local_form() {
        let at = Local.with_ymd_and_hms(2026, 11, 10, 9, 5, 7).unwrap();
        assert_eq!(log_time(at), "2026-11-10 09:05:07");
    }
}
