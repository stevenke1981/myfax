//! Inbox list column (UI_SPEC §3.2, §3.4): toolbar, result rows, empty states, pager, bottom blocks.

use super::{filters_active, ink, ink_muted, select_row, star_glyph, text_builder};
use crate::app::{AppState, Field, Msg, Route};
use crate::i18n::{Lang, t, tf};
use crate::model::{FaxRecord, Folder, SortOrder};
use crate::ui::views::kit::{self, LIST_MIN_W, PANE_PAD, ROW_H, SENDER_MAX_UNITS, Tone, glyph};
use acme_widgets::style::ColorToken;
use acme_widgets::style::prelude::Styled;
use acme_widgets::{
    ButtonSize, ButtonVariant, WidgetNode, button, column, label_builder, row, text_input,
};

/// Centre list column (`flex_1`, min 280, Background): toolbar, scrolling rows, pager, bottom.
pub(super) fn list_column(state: &AppState) -> WidgetNode<Msg> {
    let rows = if state.results.items.is_empty() {
        empty_state(state)
    } else {
        rows_block(state)
    };
    let scroll = column::<Msg>()
        .key("scroll.list")
        .flex_1()
        .overflow_scroll()
        .child(rows)
        .build();
    column::<Msg>()
        .flex_1()
        .min_w(kit::px(LIST_MIN_W))
        .h_full()
        .bg(ColorToken::Background)
        .child(toolbar(state))
        .child(kit::hdivider())
        .child(scroll)
        .child(kit::hdivider())
        .child(pager(state))
        .child(kit::hdivider())
        .child(super::bottom_block(state))
        .build()
}

fn toolbar(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let search = text_input(Field::Search.key())
        .placeholder(t(lang, "inbox.search_ph"))
        .value(state.field(Field::Search))
        .w_full()
        .build();
    column::<Msg>()
        .p(PANE_PAD)
        .gap(8.0)
        .child(search)
        .child(sort_row(state))
        .child(meta_row(state))
        .build()
}

fn sort_slug(sort: SortOrder) -> &'static str {
    match sort {
        SortOrder::NewestFirst => "newest",
        SortOrder::OldestFirst => "oldest",
        SortOrder::SenderAsc => "sender",
    }
}

fn sort_label(lang: Lang, sort: SortOrder) -> &'static str {
    match sort {
        SortOrder::NewestFirst => t(lang, "inbox.sort_newest"),
        SortOrder::OldestFirst => t(lang, "inbox.sort_oldest"),
        SortOrder::SenderAsc => t(lang, "inbox.sort_sender"),
    }
}

fn sort_row(state: &AppState) -> WidgetNode<Msg> {
    let sorts = [
        SortOrder::NewestFirst,
        SortOrder::OldestFirst,
        SortOrder::SenderAsc,
    ];
    let mut line = row::<Msg>().gap(8.0);
    for sort in sorts {
        let variant = if state.query.sort == sort {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let chip = button(
            format!("btn.sort.{}", sort_slug(sort)),
            sort_label(state.lang, sort),
        )
        .variant(variant)
        .size(ButtonSize::Small)
        .on_click(Msg::SetSort(sort));
        line = line.child(chip);
    }
    line.build()
}

/// Result count on the left; "Empty trash" (Trash with items) or "Mark all read" on the right.
fn meta_row(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let total = state.results.total.to_string();
    let count_text = tf(lang, "inbox.result_count", &[("count", total.as_str())]);
    let action = if state.query.folder == Folder::Trash && state.counts.trash > 0 {
        button("btn.empty_trash", t(lang, "inbox.empty_trash_btn"))
            .variant(ButtonVariant::Danger)
            .size(ButtonSize::XS)
            .on_click(Msg::AskEmptyTrash)
    } else {
        button("btn.mark_all_read", t(lang, "inbox.mark_all_read"))
            .variant(ButtonVariant::Ghost)
            .size(ButtonSize::XS)
            .disabled(state.counts.unread == 0)
            .on_click(Msg::MarkAllRead)
    };
    row::<Msg>()
        .h(kit::px(24.0))
        .gap(8.0)
        .child(kit::caption(&count_text))
        .child(kit::spacer())
        .child(action)
        .build()
}

fn rows_block(state: &AppState) -> WidgetNode<Msg> {
    let mut block = column::<Msg>();
    for fax in &state.results.items {
        block = block.child(fax_row(state, fax));
    }
    block.build()
}

/// One fax row (UI_SPEC §3.2 `faxrow`): unread dot, star, sender, pages, date and tags.
fn fax_row(state: &AppState, fax: &FaxRecord) -> WidgetNode<Msg> {
    let lang = state.lang;
    let active = state.selected.as_ref().is_some_and(|s| s.id == fax.id);
    let sender = fax.display_sender().map_or_else(
        || t(lang, "inbox.unknown_sender").to_string(),
        |name| kit::truncate_units(name, SENDER_MAX_UNITS),
    );
    let dot = if fax.is_read { "" } else { glyph::DOT };
    let dot_label = label_builder::<Msg>(dot.to_string())
        .font_size(14.0)
        .text_color(ColorToken::Primary)
        .w(kit::px(12.0))
        .build();
    let star = button(format!("btn.star.{}", fax.id), star_glyph(fax.is_starred))
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::XS)
        .on_click(Msg::ToggleStar(fax.id));
    let pages = tf(
        lang,
        "common.pages_short",
        &[("pages", fax.pages.to_string().as_str())],
    );
    let line_one = row::<Msg>()
        .gap(8.0)
        .child(dot_label)
        .child(star)
        .child(
            text_builder(&sender, 14.0, !fax.is_read, ink(active))
                .flex_1()
                .build(),
        )
        .child(text_builder(&pages, 12.0, false, ink_muted(active)).build())
        .build();
    let line_two = date_and_tags(state, fax, active);
    let content = column::<Msg>()
        .flex_1()
        .px(PANE_PAD)
        .py(8.0)
        .gap(2.0)
        .child(line_one)
        .child(line_two)
        .build();
    select_row(
        format!("row.fax.{}", fax.id),
        active,
        ROW_H,
        Msg::SelectFax(fax.id),
        content,
    )
    .build()
}

fn date_and_tags(state: &AppState, fax: &FaxRecord, active: bool) -> WidgetNode<Msg> {
    let lang = state.lang;
    let mut line = row::<Msg>().gap(8.0).child(
        text_builder(
            &kit::fmt_dt(fax.received_at),
            12.0,
            false,
            ink_muted(active),
        )
        .build(),
    );
    for tag in fax.tags.iter().take(2) {
        let label = format!("#{}", kit::truncate_units(tag, 10));
        line = line.child(kit::chip(&label, Tone::Info));
    }
    if fax.tags.len() > 2 {
        let more = (fax.tags.len() - 2).to_string();
        let text = tf(lang, "inbox.tag_more", &[("count", more.as_str())]);
        line = line.child(kit::chip(&text, Tone::Neutral));
    }
    line.build()
}

/// Empty-state variant of the list (UI_SPEC §3.4, first matching row wins).
fn empty_state(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    if state.query.folder == Folder::Trash {
        return kit::empty_state(glyph::NAV_INBOX, t(lang, "inbox.empty_trash"), "", None);
    }
    if filters_active(&state.query) || state.query.folder != Folder::All {
        let clear = button("btn.empty_clear_filters", t(lang, "inbox.clear_filters"))
            .variant(ButtonVariant::Secondary)
            .size(ButtonSize::Small)
            .on_click(Msg::ClearFilters);
        return kit::empty_state(
            glyph::NAV_INBOX,
            t(lang, "inbox.empty_title"),
            t(lang, "inbox.empty_filtered"),
            Some(clear),
        );
    }
    if state.settings.watch_folders.is_empty() {
        let settings = button("btn.go_settings", t(lang, "inbox.go_settings"))
            .variant(ButtonVariant::Primary)
            .size(ButtonSize::Small)
            .on_click(Msg::Navigate(Route::Settings));
        return kit::empty_state(
            glyph::NAV_INBOX,
            t(lang, "inbox.empty_title"),
            t(lang, "inbox.empty_no_folder"),
            Some(settings),
        );
    }
    kit::empty_state(
        glyph::NAV_INBOX,
        t(lang, "inbox.empty_title"),
        t(lang, "inbox.empty_waiting"),
        None,
    )
}

/// Pager (UI_SPEC §3.2): Previous / "x / y" / Next, bounds disable the buttons.
fn pager(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let page = state.query.page;
    let pages = state.page_count();
    let page_text = tf(
        lang,
        "common.page_x_of_y",
        &[
            ("page", page.saturating_add(1).to_string().as_str()),
            ("pages", pages.to_string().as_str()),
        ],
    );
    let prev_label = format!("{} {}", glyph::PREV, t(lang, "common.prev"));
    let next_label = format!("{} {}", t(lang, "common.next"), glyph::NEXT);
    let at_last = page.saturating_add(1) >= pages;
    row::<Msg>()
        .h(kit::px(40.0))
        .px(PANE_PAD)
        .gap(8.0)
        .child(
            button("btn.prev_page", prev_label)
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Small)
                .disabled(page == 0)
                .on_click(Msg::PrevPage),
        )
        .child(kit::spacer())
        .child(kit::caption(&page_text))
        .child(kit::spacer())
        .child(
            button("btn.next_page", next_label)
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Small)
                .disabled(at_last)
                .on_click(Msg::NextPage),
        )
        .build()
}
