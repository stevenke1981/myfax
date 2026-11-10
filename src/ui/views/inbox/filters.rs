//! Inbox filter column (UI_SPEC §3.1): folders, date presets and range, date tree, tags.

use super::{filters_active, ink, ink_muted, select_row, text_builder};
use crate::app::state::{MonthNode, YearNode};
use crate::app::{AppState, DatePreset, Field, Msg};
use crate::i18n::{Lang, t};
use crate::model::{FaxQuery, Folder, FolderCounts};
use crate::ui::views::kit::{self, FILTER_W, PANE_PAD, glyph};
use acme_widgets::style::ColorToken;
use acme_widgets::style::prelude::Styled;
use acme_widgets::{ButtonSize, ButtonVariant, WidgetNode, button, column, row, text_input};
use chrono::{Datelike, NaiveDate};

/// Tags shown in the filter column (most used first).
const TAG_LIMIT: usize = 30;
/// Width of one date preset chip button.
const CHIP_W: f32 = 102.0;
/// Width of the year chevron button.
const CHEVRON_W: f32 = 28.0;

/// Left filter pane (`w224`, Surface): header, then the scrolling filter sections.
pub(super) fn filter_column(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let header = row::<Msg>()
        .h(kit::px(40.0))
        .px(PANE_PAD)
        .gap(8.0)
        .child(kit::section(t(lang, "inbox.filters")))
        .child(kit::spacer())
        .child(
            button("btn.clear_filters", t(lang, "inbox.clear_filters"))
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::XS)
                .disabled(!filters_active(&state.query))
                .on_click(Msg::ClearFilters),
        )
        .build();
    let body = column::<Msg>()
        .key("scroll.filters")
        .flex_1()
        .flex_shrink(1.0)
        .min_h(crate::ui::views::kit::px(0.0))
        .overflow_scroll()
        .p(PANE_PAD)
        .gap(16.0)
        .child(folders_block(state))
        .child(date_block(state))
        .child(tree_block(state))
        .child(tags_block(state))
        .build();
    column::<Msg>()
        .w(kit::px(FILTER_W))
        .h(crate::ui::views::kit::full())
        .bg(ColorToken::Surface)
        .child(header)
        .child(kit::hdivider())
        .child(body)
        .build()
}

fn folder_slug(folder: Folder) -> &'static str {
    match folder {
        Folder::All => "all",
        Folder::Unread => "unread",
        Folder::Starred => "starred",
        Folder::Trash => "trash",
    }
}

fn folder_name(lang: Lang, folder: Folder) -> &'static str {
    match folder {
        Folder::All => t(lang, "inbox.folder_all"),
        Folder::Unread => t(lang, "inbox.folder_unread"),
        Folder::Starred => t(lang, "inbox.folder_starred"),
        Folder::Trash => t(lang, "inbox.folder_trash"),
    }
}

fn folder_count(counts: &FolderCounts, folder: Folder) -> u64 {
    match folder {
        Folder::All => counts.all,
        Folder::Unread => counts.unread,
        Folder::Starred => counts.starred,
        Folder::Trash => counts.trash,
    }
}

fn folders_block(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let mut block = column::<Msg>()
        .gap(4.0)
        .child(kit::caption(t(lang, "inbox.section_folders")));
    for folder in [Folder::All, Folder::Unread, Folder::Starred, Folder::Trash] {
        let active = state.query.folder == folder;
        let count = folder_count(&state.counts, folder).to_string();
        let content = row::<Msg>()
            .flex_1()
            .flex_shrink(1.0)
            .min_h(crate::ui::views::kit::px(0.0))
            .h(crate::ui::views::kit::full())
            .px(8.0)
            .gap(8.0)
            .child(text_builder(folder_name(lang, folder), 14.0, false, ink(active)).build())
            .child(kit::spacer())
            .child(text_builder(&count, 12.0, false, ink_muted(active)).build())
            .build();
        let key = format!("folder.{}", folder_slug(folder));
        block =
            block.child(select_row(key, active, 32.0, Msg::SelectFolder(folder), content).build());
    }
    block.build()
}

/// Label of a date preset chip. `Custom` never appears as a chip; it shows as "any".
fn preset_label(lang: Lang, preset: DatePreset) -> &'static str {
    match preset {
        DatePreset::AnyTime | DatePreset::Custom => t(lang, "inbox.preset_any"),
        DatePreset::Today => t(lang, "inbox.preset_today"),
        DatePreset::Yesterday => t(lang, "inbox.preset_yesterday"),
        DatePreset::Last7Days => t(lang, "inbox.preset_7d"),
        DatePreset::Last30Days => t(lang, "inbox.preset_30d"),
        DatePreset::ThisMonth => t(lang, "inbox.preset_month"),
        DatePreset::ThisYear => t(lang, "inbox.preset_year"),
    }
}

fn preset_slug(preset: DatePreset) -> &'static str {
    match preset {
        DatePreset::AnyTime | DatePreset::Custom => "any",
        DatePreset::Today => "today",
        DatePreset::Yesterday => "yesterday",
        DatePreset::Last7Days => "7d",
        DatePreset::Last30Days => "30d",
        DatePreset::ThisMonth => "month",
        DatePreset::ThisYear => "year",
    }
}

fn preset_chip(state: &AppState, preset: DatePreset) -> WidgetNode<Msg> {
    let variant = if state.date_preset == preset {
        ButtonVariant::Primary
    } else {
        ButtonVariant::Secondary
    };
    button(
        format!("btn.preset.{}", preset_slug(preset)),
        preset_label(state.lang, preset),
    )
    .variant(variant)
    .size(ButtonSize::Small)
    .w(kit::px(CHIP_W))
    .on_click(Msg::SetDatePreset(preset))
}

fn date_block(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let mut block = column::<Msg>()
        .gap(8.0)
        .child(kit::caption(t(lang, "inbox.section_date")));
    for pair in DatePreset::CHIPS.chunks(2) {
        let mut line = row::<Msg>().gap(8.0);
        for preset in pair {
            line = line.child(preset_chip(state, *preset));
        }
        block = block.child(line.build());
    }
    block
        .child(
            text_input(Field::DateFrom.key())
                .placeholder(t(lang, "inbox.date_from_ph"))
                .value(state.field(Field::DateFrom))
                .w(crate::ui::views::kit::full())
                .build(),
        )
        .child(
            text_input(Field::DateTo.key())
                .placeholder(t(lang, "inbox.date_to_ph"))
                .value(state.field(Field::DateTo))
                .w(crate::ui::views::kit::full())
                .build(),
        )
        .child(
            row::<Msg>()
                .child(
                    button("btn.apply_range", t(lang, "inbox.date_apply"))
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Small)
                        .on_click(Msg::ApplyCustomRange),
                )
                .build(),
        )
        .build()
}

/// First and last day of a calendar month, `None` for impossible dates.
fn month_bounds(year: i32, month: u32) -> Option<(NaiveDate, NaiveDate)> {
    let first = NaiveDate::from_ymd_opt(year, month, 1)?;
    let (next_year, next_month) = if month >= 12 {
        (year.checked_add(1)?, 1)
    } else {
        (year, month + 1)
    };
    let last = NaiveDate::from_ymd_opt(next_year, next_month, 1)?.pred_opt()?;
    Some((first, last))
}

/// Year node is active when the range is exactly Jan 1 .. Dec 31 of that year.
pub(super) fn year_active(query: &FaxQuery, year: i32) -> bool {
    let (Some(first), Some(last)) = (
        NaiveDate::from_ymd_opt(year, 1, 1),
        NaiveDate::from_ymd_opt(year, 12, 31),
    ) else {
        return false;
    };
    query.date_from == Some(first) && query.date_to == Some(last)
}

/// Month node is active when the range is exactly the first..last day of that month.
pub(super) fn month_active(query: &FaxQuery, year: i32, month: u32) -> bool {
    let Some((first, last)) = month_bounds(year, month) else {
        return false;
    };
    query.date_from == Some(first) && query.date_to == Some(last)
}

/// Day node is active when the range is exactly that single day.
pub(super) fn day_active(query: &FaxQuery, date: NaiveDate) -> bool {
    query.date_from == Some(date) && query.date_to == Some(date)
}

fn tree_block(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let mut block = column::<Msg>()
        .gap(2.0)
        .child(kit::caption(t(lang, "inbox.section_tree")));
    if state.date_tree.is_empty() {
        block = block.child(kit::caption(t(lang, "inbox.tree_empty")));
    }
    for year in &state.date_tree {
        block = block.child(year_block(state, year));
    }
    block.build()
}

fn year_block(state: &AppState, node: &YearNode) -> WidgetNode<Msg> {
    let active = year_active(&state.query, node.year);
    let toggle_glyph = if node.expanded {
        glyph::CHEVRON_DOWN
    } else {
        glyph::CHEVRON_RIGHT
    };
    let toggle = button(format!("btn.toggle_year.{}", node.year), toggle_glyph)
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::XS)
        .w(kit::px(CHEVRON_W))
        .on_click(Msg::ToggleYear(node.year));
    let content = row::<Msg>()
        .flex_1()
        .flex_shrink(1.0)
        .min_h(crate::ui::views::kit::px(0.0))
        .h(crate::ui::views::kit::full())
        .px(8.0)
        .gap(8.0)
        .child(text_builder(&node.year.to_string(), 14.0, true, ink(active)).build())
        .child(kit::spacer())
        .child(text_builder(&node.count.to_string(), 12.0, false, ink_muted(active)).build())
        .build();
    let ysel = select_row(
        format!("tree.ysel.{}", node.year),
        active,
        32.0,
        Msg::SelectYear(node.year),
        content,
    )
    .flex_1()
    .flex_shrink(1.0)
    .min_h(crate::ui::views::kit::px(0.0))
    .build();
    let head = row::<Msg>()
        .key(format!("tree.y.{}", node.year))
        .h(kit::px(32.0))
        .rounded_md()
        .child(toggle)
        .child(ysel)
        .build();
    let mut block = column::<Msg>().child(head);
    if node.expanded {
        for month in &node.months {
            block = block.child(month_block(state, node.year, month));
        }
    }
    block.build()
}

fn month_block(state: &AppState, year: i32, month: &MonthNode) -> WidgetNode<Msg> {
    let active = month_active(&state.query, year, month.month);
    let content = row::<Msg>()
        .flex_1()
        .flex_shrink(1.0)
        .min_h(crate::ui::views::kit::px(0.0))
        .h(crate::ui::views::kit::full())
        .pl(24.0)
        .pr(8.0)
        .gap(8.0)
        .child(text_builder(&format!("{:02}", month.month), 14.0, false, ink(active)).build())
        .child(kit::spacer())
        .child(text_builder(&month.count.to_string(), 12.0, false, ink_muted(active)).build())
        .build();
    let key = format!("tree.m.{year}-{:02}", month.month);
    let msg = Msg::SelectMonth {
        year,
        month: month.month,
    };
    let mut block = column::<Msg>().child(select_row(key, active, 28.0, msg, content).build());
    if state.expanded_month == Some((year, month.month)) {
        for day in &month.days {
            if let Some(date) = NaiveDate::from_ymd_opt(year, month.month, day.day) {
                block = block.child(day_row(state, date, day.count));
            }
        }
    }
    block.build()
}

fn day_row(state: &AppState, date: NaiveDate, count: u64) -> WidgetNode<Msg> {
    let active = day_active(&state.query, date);
    let content = row::<Msg>()
        .flex_1()
        .flex_shrink(1.0)
        .min_h(crate::ui::views::kit::px(0.0))
        .h(crate::ui::views::kit::full())
        .pl(44.0)
        .pr(8.0)
        .gap(8.0)
        .child(text_builder(&format!("{:02}", date.day()), 14.0, false, ink(active)).build())
        .child(kit::spacer())
        .child(text_builder(&count.to_string(), 12.0, false, ink_muted(active)).build())
        .build();
    let key = format!("tree.d.{}", date.format("%Y-%m-%d"));
    select_row(key, active, 28.0, Msg::SelectDay(date), content).build()
}

fn tags_block(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let mut block = column::<Msg>()
        .gap(4.0)
        .child(kit::caption(t(lang, "inbox.section_tags")));
    if state.all_tags.is_empty() {
        block = block.child(kit::caption(t(lang, "inbox.tags_empty")));
    }
    for (tag, count) in state.all_tags.iter().take(TAG_LIMIT) {
        let active = state.query.tag.as_deref() == Some(tag.as_str());
        let variant = if active {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Ghost
        };
        let next = if active { None } else { Some(tag.clone()) };
        let label = format!("#{}  {count}", kit::truncate_units(tag, 16));
        let chip = button(format!("btn.tag.{tag}"), label)
            .variant(variant)
            .size(ButtonSize::Small)
            .on_click(Msg::SetTagFilter(next));
        block = block.child(row::<Msg>().child(chip).build());
    }
    block.build()
}
