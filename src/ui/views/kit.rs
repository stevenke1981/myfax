//! Shared view kit (orchestrator-owned): metrics, text helpers, badges, page
//! header, empty state, formatting helpers and glyphs. See UI_SPEC.md §0.4.

use crate::app::Msg;
use acme_layout::Length;
use acme_widgets::style::prelude::*;
use acme_widgets::style::{ColorToken, FontWeight};
use acme_widgets::{LabelWrap, WidgetNode, column, label_builder, row, separator};
use chrono::{DateTime, Local};

/// 100 % length. Use this instead of `Styled::w_full()` / `h_full()`: at the
/// pinned AcmeUI revision those pass `Percent(100.0)` straight to Taffy, which
/// expects a 0..=1 fraction, so they size nodes at 100× their parent.
pub fn full() -> Length {
    Length::Percent(1.0)
}

/// Pixel length for `Styled::w`/`h`/`min_w`/`max_w` (they take `Length`, not `f32`).
pub fn px(value: f32) -> Length {
    Length::px(value)
}

pub const NAV_W: f32 = 152.0;
pub const FILTER_W: f32 = 224.0;
pub const DETAIL_W: f32 = 352.0;
pub const LIST_MIN_W: f32 = 280.0;
pub const STATUS_H: f32 = 28.0;
pub const BANNER_H: f32 = 40.0;
pub const PAGE_PAD: f32 = 24.0;
const HEADER_H: f32 = 56.0;
const HEADER_PAD_Y: f32 = 12.0;
const HEADER_TWO_LINE_PAD_Y: f32 = 1.5;
pub const PANE_PAD: f32 = 12.0;
pub const ROW_H: f32 = 56.0;
pub const CHART_H: f32 = 160.0;
pub const PREVIEW_MAX_H: f32 = 420.0;
pub const SENDER_MAX_UNITS: usize = 24;

/// Semantic tone for badges, chips, toasts and log levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Info,
    Success,
    Warning,
    Danger,
    Primary,
}

/// (background, foreground) colour tokens of a tone.
pub fn tone_tokens(tone: Tone) -> (ColorToken, ColorToken) {
    match tone {
        Tone::Neutral => (ColorToken::Muted, ColorToken::MutedForeground),
        Tone::Info => (ColorToken::InfoSoft, ColorToken::Info),
        Tone::Success => (ColorToken::SuccessSoft, ColorToken::Success),
        Tone::Warning => (ColorToken::WarningSoft, ColorToken::Warning),
        Tone::Danger => (ColorToken::DangerSoft, ColorToken::Danger),
        Tone::Primary => (ColorToken::Primary, ColorToken::PrimaryForeground),
    }
}

/// Text glyphs used as icons (no icon font). Avoid code points with an emoji
/// presentation (U+2611, U+2699, U+2709, U+263A): Windows draws them from the
/// colour emoji font, which the glyph atlas renders as a solid block.
pub mod glyph {
    pub const STAR: &str = "★";
    pub const STAR_OFF: &str = "☆";
    pub const DOT: &str = "●";
    pub const RING: &str = "○";
    pub const RADIO_ON: &str = "◉";
    pub const CHECK_ON: &str = "✓";
    pub const CHECK_OFF: &str = "○";
    pub const CHEVRON_RIGHT: &str = "▸";
    pub const CHEVRON_DOWN: &str = "▾";
    pub const PREV: &str = "‹";
    pub const NEXT: &str = "›";
    pub const CLOSE: &str = "×";
    pub const OK: &str = "✓";
    pub const MINUS: &str = "−";
    pub const PLUS: &str = "+";
    pub const NAV_INBOX: &str = "▤";
    pub const NAV_CONTACTS: &str = "◎";
    pub const NAV_SEND: &str = "➤";
    pub const NAV_STATS: &str = "▥";
    pub const NAV_LOG: &str = "≡";
    pub const NAV_SETTINGS: &str = "✱";
    pub const WARN: &str = "!";
    pub const INFO: &str = "i";
}

fn text(value: &str, size: f32) -> acme_widgets::LabelBuilder<Msg> {
    label_builder::<Msg>(value.to_string()).font_size(size)
}

/// 20 px semibold page title.
pub fn title(value: &str) -> WidgetNode<Msg> {
    text(value, 20.0).font_weight(FontWeight::SemiBold).build()
}

/// 15 px semibold section heading.
pub fn section(value: &str) -> WidgetNode<Msg> {
    text(value, 15.0).font_weight(FontWeight::SemiBold).build()
}

/// 14 px body text.
pub fn body(value: &str) -> WidgetNode<Msg> {
    text(value, 14.0).build()
}

/// 14 px semibold body text (unread rows).
pub fn body_strong(value: &str) -> WidgetNode<Msg> {
    text(value, 14.0).font_weight(FontWeight::SemiBold).build()
}

/// 14 px body text wrapped at word boundaries (prose inside a width-constrained parent).
pub fn body_wrap(value: &str) -> WidgetNode<Msg> {
    text(value, 14.0).wrap(LabelWrap::Word).build()
}

/// 12 px muted caption.
pub fn caption(value: &str) -> WidgetNode<Msg> {
    text(value, 12.0).text_muted().build()
}

/// Caption wrapped at word boundaries.
pub fn caption_wrap(value: &str) -> WidgetNode<Msg> {
    text(value, 12.0).text_muted().wrap(LabelWrap::Word).build()
}

/// Caption wrapped at any character (paths, file names).
pub fn caption_wrap_chars(value: &str) -> WidgetNode<Msg> {
    text(value, 12.0)
        .text_muted()
        .wrap(LabelWrap::Character)
        .build()
}

/// Label with an explicit colour token.
pub fn colored(value: &str, size: f32, color: ColorToken) -> WidgetNode<Msg> {
    text(value, size).text_color(color).build()
}

/// Pill badge.
pub fn badge(value: &str, tone: Tone) -> WidgetNode<Msg> {
    let (bg, fg) = tone_tokens(tone);
    row::<Msg>()
        .px(8.0)
        .py(2.0)
        .rounded_full()
        .bg(bg)
        .child(text(value, 12.0).text_color(fg).build())
        .build()
}

/// Rectangular chip (tags).
pub fn chip(value: &str, tone: Tone) -> WidgetNode<Msg> {
    let (bg, fg) = tone_tokens(tone);
    row::<Msg>()
        .px(8.0)
        .py(2.0)
        .rounded_sm()
        .bg(bg)
        .child(text(value, 12.0).text_color(fg).build())
        .build()
}

/// Flexible empty space.
pub fn spacer() -> WidgetNode<Msg> {
    column::<Msg>()
        .flex_1()
        .flex_shrink(1.0)
        .min_h(crate::ui::views::kit::px(0.0))
        .build()
}

/// Fixed vertical gap of `h` px.
pub fn hgap(h: f32) -> WidgetNode<Msg> {
    column::<Msg>().h(px(h)).build()
}

/// Fixed horizontal gap of `w` px.
pub fn vgap(w: f32) -> WidgetNode<Msg> {
    row::<Msg>().w(px(w)).build()
}

/// 1 px vertical divider (inside a row).
pub fn vdivider() -> WidgetNode<Msg> {
    column::<Msg>()
        .w(px(1.0))
        .h(full())
        .bg(ColorToken::Border)
        .build()
}

/// Horizontal divider.
pub fn hdivider() -> WidgetNode<Msg> {
    separator::<Msg>()
}

/// Caption above an input.
pub fn field(label_text: &str, input: WidgetNode<Msg>) -> WidgetNode<Msg> {
    column::<Msg>()
        .gap(4.0)
        .child(caption(label_text))
        .child(input)
        .build()
}

/// Page header row (56 high) with title, optional subtitle and right-aligned actions,
/// followed by a divider.
pub fn page_header(
    title_text: &str,
    subtitle: Option<String>,
    actions: Vec<WidgetNode<Msg>>,
) -> WidgetNode<Msg> {
    // Title + caption (30 + 2 + 21 px) fill the 56 px row; a title alone is centred.
    let pad_y = if subtitle.is_some() {
        HEADER_TWO_LINE_PAD_Y
    } else {
        HEADER_PAD_Y
    };
    let mut heading = column::<Msg>().gap(2.0).child(title(title_text));
    if let Some(sub) = subtitle {
        heading = heading.child(caption(&sub));
    }
    let mut action_row = row::<Msg>().gap(8.0);
    for action in actions {
        action_row = action_row.child(action);
    }
    column::<Msg>()
        .flex_shrink(0.0)
        .child(
            row::<Msg>()
                .h(px(HEADER_H))
                .px(PAGE_PAD)
                .py(pad_y)
                .gap(12.0)
                .child(heading)
                .child(spacer())
                .child(action_row.build())
                .build(),
        )
        .child(hdivider())
        .build()
}

/// Centred empty state: glyph, title, body and optional action.
pub fn empty_state(
    glyph_text: &str,
    title_text: &str,
    body_text: &str,
    action: Option<WidgetNode<Msg>>,
) -> WidgetNode<Msg> {
    let mut inner = column::<Msg>()
        .gap(8.0)
        .p(PAGE_PAD)
        .child(colored(glyph_text, 28.0, ColorToken::MutedForeground))
        .child(section(title_text))
        .child(caption_wrap(body_text));
    if let Some(action) = action {
        inner = inner.child(row::<Msg>().child(action).build());
    }
    column::<Msg>()
        .flex_1()
        .flex_shrink(1.0)
        .min_h(crate::ui::views::kit::px(0.0))
        .child(spacer())
        .child(
            row::<Msg>()
                .child(spacer())
                .child(inner.max_w(px(320.0)).build())
                .child(spacer())
                .build(),
        )
        .child(spacer())
        .build()
}

fn char_units(c: char) -> usize {
    if c.is_ascii() { 1 } else { 2 }
}

/// Shorten `s` to at most `max` display units (ASCII = 1, other = 2), appending
/// `…` (1 unit… counted as 2) when cut. Never splits a character.
pub fn truncate_units(s: &str, max: usize) -> String {
    let total: usize = s.chars().map(char_units).sum();
    if total <= max {
        return s.to_string();
    }
    let budget = max.saturating_sub(2);
    let mut used = 0;
    let mut out = String::new();
    for c in s.chars() {
        let units = char_units(c);
        if used + units > budget {
            break;
        }
        used += units;
        out.push(c);
    }
    out.push('…');
    out
}

/// Shorten a path-like string to `max` units keeping its head and tail.
pub fn truncate_middle(s: &str, max: usize) -> String {
    let total: usize = s.chars().map(char_units).sum();
    if total <= max {
        return s.to_string();
    }
    let budget = max.saturating_sub(2);
    let head_budget = budget / 3;
    let tail_budget = budget - head_budget;
    let mut head = String::new();
    let mut used = 0;
    for c in s.chars() {
        if used + char_units(c) > head_budget {
            break;
        }
        used += char_units(c);
        head.push(c);
    }
    let mut tail: Vec<char> = Vec::new();
    let mut used_tail = 0;
    for c in s.chars().rev() {
        if used_tail + char_units(c) > tail_budget {
            break;
        }
        used_tail += char_units(c);
        tail.push(c);
    }
    tail.reverse();
    format!("{head}…{}", tail.into_iter().collect::<String>())
}

/// `YYYY-MM-DD HH:mm`.
pub fn fmt_dt(at: DateTime<Local>) -> String {
    at.format("%Y-%m-%d %H:%M").to_string()
}

/// Human file size: `512 B`, `12 KB`, `3.4 MB`.
pub fn fmt_size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * 1024;
    if bytes < KIB {
        format!("{bytes} B")
    } else if bytes < MIB {
        format!("{:.0} KB", bytes as f64 / KIB as f64)
    } else {
        format!("{:.1} MB", bytes as f64 / MIB as f64)
    }
}

/// Zoom factor as a rounded percentage, `1.25` → `125%`.
pub fn fmt_percent(zoom: f32) -> String {
    let pct = if zoom.is_finite() {
        (zoom * 100.0).round()
    } else {
        100.0
    };
    format!("{pct:.0}%")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn truncate_units_counts_cjk_double() {
        assert_eq!(truncate_units("abc", 5), "abc");
        assert_eq!(truncate_units("abcdefgh", 5), "abc…");
        assert_eq!(truncate_units("傳真機器", 6), "傳真…");
        assert_eq!(truncate_units("傳真", 4), "傳真");
    }

    #[test]
    fn truncate_middle_keeps_head_and_tail() {
        let out = truncate_middle("C:/very/long/path/to/file.pdf", 16);
        assert!(out.contains('…'));
        assert!(out.ends_with(".pdf"));
        assert_eq!(truncate_middle("short", 16), "short");
    }

    #[test]
    fn formats() {
        let at = Local.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
        assert_eq!(fmt_dt(at), "2026-01-02 03:04");
        assert_eq!(fmt_size(10), "10 B");
        assert_eq!(fmt_size(2048), "2 KB");
        assert_eq!(fmt_size(3 * 1024 * 1024 + 400_000), "3.4 MB");
        assert_eq!(fmt_percent(1.25), "125%");
        assert_eq!(fmt_percent(f32::NAN), "100%");
    }

    #[test]
    fn helpers_build_nodes() {
        assert!(matches!(badge("x", Tone::Info), WidgetNode::Row(_)));
        assert!(matches!(title("x"), WidgetNode::Label(_)));
        assert!(matches!(
            page_header("t", Some("s".into()), vec![]),
            WidgetNode::Column(_)
        ));
        assert!(matches!(
            empty_state(glyph::NAV_INBOX, "t", "b", None),
            WidgetNode::Column(_)
        ));
    }
}
