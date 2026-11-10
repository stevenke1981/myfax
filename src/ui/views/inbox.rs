//! Inbox page (UI_SPEC §3): filter column, list column and the detail pane.
//! Owner: views worker (inbox + detail). Sub-blocks live in `inbox/`.

mod filters;
mod list;
#[cfg(test)]
pub(crate) mod testing;
#[cfg(test)]
mod tests;

use super::kit::{self, PANE_PAD, glyph};
use crate::app::{AppState, Field, Msg};
use crate::i18n::t;
use crate::model::FaxQuery;
use acme_widgets::style::prelude::Styled;
use acme_widgets::style::{ColorToken, FontWeight};
use acme_widgets::{
    ButtonSize, ButtonVariant, ContainerBuilder, LabelBuilder, WidgetNode, button, column,
    label_builder, row, text_input,
};

/// Whole page body (right of the nav rail): filters | list | detail.
pub fn view(state: &AppState) -> WidgetNode<Msg> {
    row::<Msg>()
        .flex_1()
        .h_full()
        .child(filters::filter_column(state))
        .child(kit::vdivider())
        .child(list::list_column(state))
        .child(kit::vdivider())
        .child(super::detail::pane(state))
        .build()
}

/// True when any filter other than the folder is active (UI_SPEC §3.1).
fn filters_active(query: &FaxQuery) -> bool {
    !query.text.is_empty()
        || query.date_from.is_some()
        || query.date_to.is_some()
        || query.tag.is_some()
}

/// Text colour of a row label: all text is `AccentForeground` on the active row.
fn ink(active: bool) -> ColorToken {
    if active {
        ColorToken::AccentForeground
    } else {
        ColorToken::Foreground
    }
}

/// Secondary text colour of a row: `AccentForeground` on the active row.
fn ink_muted(active: bool) -> ColorToken {
    if active {
        ColorToken::AccentForeground
    } else {
        ColorToken::MutedForeground
    }
}

/// Text label builder with size, optional SemiBold weight and colour.
pub(super) fn text_builder(
    value: &str,
    size: f32,
    semibold: bool,
    color: ColorToken,
) -> LabelBuilder<Msg> {
    let builder = label_builder::<Msg>(value.to_string())
        .font_size(size)
        .text_color(color);
    if semibold {
        builder.font_weight(FontWeight::SemiBold)
    } else {
        builder
    }
}

/// Selectable row with the left-bar trick (UI_SPEC §1): a 3 px bar that is
/// `Primary` when active, then `content`. Active rows get the `Accent` background.
fn select_row(
    key: String,
    active: bool,
    height: f32,
    msg: Msg,
    content: WidgetNode<Msg>,
) -> ContainerBuilder<Msg> {
    let bar = column::<Msg>().w(kit::px(3.0)).h_full().rounded_full();
    let bar = if active {
        bar.bg(ColorToken::Primary)
    } else {
        bar
    };
    let mut out = row::<Msg>()
        .key(key)
        .h(kit::px(height))
        .rounded_md()
        .on_click(msg)
        .child(bar.build())
        .child(content);
    if active {
        out = out.bg(ColorToken::Accent);
    }
    out
}

/// Horizontal group of buttons or fields with 8 px gaps.
fn line(children: Vec<WidgetNode<Msg>>) -> WidgetNode<Msg> {
    let mut out = row::<Msg>().gap(8.0);
    for child in children {
        out = out.child(child);
    }
    out.build()
}

/// Import and export rows pinned to the bottom of the list column (UI_SPEC §3.2).
fn bottom_block(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let import_row = line(vec![
        text_input(Field::ImportPath.key())
            .placeholder(t(lang, "inbox.import_ph"))
            .value(state.field(Field::ImportPath))
            .flex_1()
            .build(),
        button("btn.browse.import_path", t(lang, "common.browse_short"))
            .variant(ButtonVariant::Secondary)
            .size(ButtonSize::Small)
            .w(kit::px(36.0))
            .on_click(Msg::Browse(Field::ImportPath)),
        button("btn.import", t(lang, "common.import"))
            .variant(ButtonVariant::Primary)
            .size(ButtonSize::Small)
            .on_click(Msg::ImportFromField),
    ]);
    let export_row = line(vec![
        text_input(Field::ExportFolder.key())
            .placeholder(t(lang, "inbox.export_ph"))
            .value(state.field(Field::ExportFolder))
            .flex_1()
            .build(),
        button("btn.browse.export_folder", t(lang, "common.browse_short"))
            .variant(ButtonVariant::Secondary)
            .size(ButtonSize::Small)
            .w(kit::px(36.0))
            .on_click(Msg::Browse(Field::ExportFolder)),
        button("btn.export", t(lang, "inbox.export_btn"))
            .variant(ButtonVariant::Secondary)
            .size(ButtonSize::Small)
            .disabled(state.results.total == 0)
            .on_click(Msg::ExportFiltered),
    ]);
    column::<Msg>()
        .p(PANE_PAD)
        .gap(8.0)
        .bg(ColorToken::Surface)
        .child(import_row)
        .child(kit::caption_wrap(t(lang, "inbox.import_hint")))
        .child(export_row)
        .build()
}

/// Glyph used for a fax's star button (kept here so list and tests agree).
fn star_glyph(starred: bool) -> &'static str {
    if starred {
        glyph::STAR
    } else {
        glyph::STAR_OFF
    }
}
