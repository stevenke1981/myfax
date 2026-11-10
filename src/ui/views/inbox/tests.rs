//! Tests for the inbox page: rows, pager, trash, date tree, tags, empty states and truncation.

use super::filters::{day_active, month_active, year_active};
use super::testing::{empty_state, sample_fax, walk};
use super::view;
use crate::app::state::{DayNode, MonthNode, YearNode};
use crate::app::{AppState, DatePreset, Msg, Route};
use crate::model::{FaxQuery, Folder, SortOrder};
use crate::ui::views::kit;
use acme_widgets::ButtonVariant;
use chrono::NaiveDate;

fn three_faxes() -> AppState {
    let mut state = empty_state();
    state.results.items = vec![
        sample_fax(1, Some("Alpha"), false, false),
        sample_fax(2, Some("Beta"), true, true),
        sample_fax(3, None, false, false),
    ];
    state.results.total = 3;
    state.counts.all = 3;
    state.counts.unread = 2;
    state
}

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid test date")
}

#[test]
fn rows_select_each_fax_by_id() {
    let state = three_faxes();
    let tree = walk(&view(&state));
    for id in [1, 2, 3] {
        assert!(tree.has_key(&format!("row.fax.{id}")), "missing row {id}");
        assert!(tree.can(&Msg::SelectFax(id)), "row {id} not clickable");
    }
}

#[test]
fn unread_rows_show_dot_and_read_rows_do_not() {
    let state = three_faxes();
    let tree = walk(&view(&state));
    assert_eq!(tree.label_count("●"), 2);
}

#[test]
fn star_button_toggles_star_for_its_fax() {
    let state = three_faxes();
    let tree = walk(&view(&state));
    assert!(tree.can(&Msg::ToggleStar(1)));
    assert!(tree.can(&Msg::ToggleStar(2)));
}

#[test]
fn missing_sender_shows_unknown_label() {
    let mut fax = sample_fax(8, None, true, false);
    fax.sender_number = None;
    let mut state = empty_state();
    state.results.items = vec![fax];
    state.results.total = 1;
    let tree = walk(&view(&state));
    assert!(tree.has_label("未知寄件者"));
}

#[test]
fn long_sender_is_truncated_to_budget() {
    let mut state = empty_state();
    let long = "傳真".repeat(30);
    state.results.items = vec![sample_fax(9, Some(long.as_str()), true, false)];
    state.results.total = 1;
    let tree = walk(&view(&state));
    let sender = tree
        .labels
        .iter()
        .find(|l| l.ends_with('…'))
        .expect("truncated sender label");
    let units: usize = sender
        .chars()
        .map(|c| if c.is_ascii() { 1 } else { 2 })
        .sum();
    assert!(units <= crate::ui::views::kit::SENDER_MAX_UNITS);
    assert!(kit::truncate_units(&long, kit::SENDER_MAX_UNITS).ends_with('…'));
}

#[test]
fn single_page_disables_both_pager_buttons() {
    let state = three_faxes();
    assert_eq!(state.page_count(), 1);
    let tree = walk(&view(&state));
    assert!(!tree.can(&Msg::PrevPage));
    assert!(!tree.can(&Msg::NextPage));
}

#[test]
fn middle_page_enables_both_and_last_page_disables_next() {
    let mut state = three_faxes();
    state.query.page_size = 50;
    state.results.total = 150;
    state.query.page = 1;
    let tree = walk(&view(&state));
    assert!(tree.can(&Msg::PrevPage));
    assert!(tree.can(&Msg::NextPage));

    state.query.page = 2;
    let tree = walk(&view(&state));
    assert!(tree.can(&Msg::PrevPage));
    assert!(!tree.can(&Msg::NextPage));
}

#[test]
fn trash_with_items_offers_empty_trash_instead_of_mark_all_read() {
    let mut state = three_faxes();
    state.query.folder = Folder::Trash;
    state.counts.trash = 3;
    let tree = walk(&view(&state));
    assert!(tree.can(&Msg::AskEmptyTrash));
    assert!(!tree.has_key("btn.mark_all_read"));
}

#[test]
fn empty_trash_without_items_offers_neither_trash_nor_mark_all_read_action() {
    let mut state = three_faxes();
    state.query.folder = Folder::Trash;
    state.counts.trash = 0;
    let tree = walk(&view(&state));
    assert!(!tree.has_key("btn.mark_all_read"));
    assert!(!tree.has_key("btn.empty_trash"));
}

#[test]
fn mark_all_read_is_disabled_without_unread_faxes() {
    let mut state = three_faxes();
    state.counts.unread = 0;
    let tree = walk(&view(&state));
    assert!(!tree.can(&Msg::MarkAllRead));
    assert!(tree.has_key("btn.mark_all_read"));
}

#[test]
fn empty_trash_shows_trash_empty_state_without_actions() {
    let mut state = empty_state();
    state.query.folder = Folder::Trash;
    let tree = walk(&view(&state));
    assert!(tree.has_label("垃圾桶是空的"));
    assert!(!tree.can(&Msg::AskEmptyTrash));
}

#[test]
fn empty_filtered_list_offers_clear_filters() {
    let mut state = empty_state();
    state.query.tag = Some("invoice".to_string());
    let tree = walk(&view(&state));
    assert!(tree.has_label("沒有符合目前篩選條件的傳真。"));
    assert!(tree.can(&Msg::ClearFilters));
}

#[test]
fn empty_list_without_watch_folder_links_to_settings() {
    let mut state = empty_state();
    state.settings.watch_folders.clear();
    let tree = walk(&view(&state));
    assert!(tree.can(&Msg::Navigate(Route::Settings)));
    assert!(tree.has_label("尚未設定監看資料夾，請先到「設定」新增。"));
}

#[test]
fn empty_list_with_watch_folder_waits_for_faxes() {
    let mut state = empty_state();
    state.settings.watch_folders = vec![std::path::PathBuf::from("inbox")];
    let tree = walk(&view(&state));
    assert!(!tree.can(&Msg::Navigate(Route::Settings)));
    assert!(tree.has_label("正在等待新傳真，也可以在下方匯入既有檔案。"));
}

#[test]
fn clear_filters_is_only_enabled_with_an_active_filter() {
    let state = three_faxes();
    assert!(!walk(&view(&state)).can(&Msg::ClearFilters));

    let mut filtered = three_faxes();
    filtered.query.text = "fax".to_string();
    assert!(walk(&view(&filtered)).can(&Msg::ClearFilters));
}

#[test]
fn preset_chips_emit_messages_and_mark_the_active_one() {
    let mut state = three_faxes();
    state.date_preset = DatePreset::Today;
    let tree = walk(&view(&state));
    assert!(tree.can(&Msg::SetDatePreset(DatePreset::Yesterday)));
    assert_eq!(
        tree.variant("btn.preset.today"),
        Some(ButtonVariant::Primary)
    );
    assert_eq!(
        tree.variant("btn.preset.7d"),
        Some(ButtonVariant::Secondary)
    );
}

#[test]
fn sort_buttons_mark_the_active_order() {
    let mut state = three_faxes();
    state.query.sort = SortOrder::OldestFirst;
    let tree = walk(&view(&state));
    assert_eq!(
        tree.variant("btn.sort.oldest"),
        Some(ButtonVariant::Primary)
    );
    assert_eq!(
        tree.variant("btn.sort.newest"),
        Some(ButtonVariant::Secondary)
    );
    assert!(tree.can(&Msg::SetSort(SortOrder::SenderAsc)));
}

#[test]
fn import_and_export_rows_emit_their_messages() {
    let state = three_faxes();
    let tree = walk(&view(&state));
    assert!(tree.can(&Msg::ImportFromField));
    assert!(tree.can(&Msg::Browse(crate::app::Field::ImportPath)));
    assert!(tree.can(&Msg::ExportFiltered));

    let empty = empty_state();
    assert!(!walk(&view(&empty)).can(&Msg::ExportFiltered));
}

fn tree_state(expanded_month: Option<(i32, u32)>) -> AppState {
    let mut state = three_faxes();
    state.date_tree = vec![YearNode {
        year: 2026,
        count: 3,
        expanded: true,
        months: vec![MonthNode {
            month: 3,
            count: 3,
            days: vec![DayNode { day: 5, count: 3 }],
        }],
    }];
    state.expanded_month = expanded_month;
    state
}

#[test]
fn day_buttons_appear_only_for_the_expanded_month() {
    let collapsed = walk(&view(&tree_state(None)));
    assert!(!collapsed.has_key_prefix("tree.d."));
    assert!(collapsed.has_key("tree.m.2026-03"));

    let expanded = walk(&view(&tree_state(Some((2026, 3)))));
    assert!(expanded.has_key("tree.d.2026-03-05"));
    assert!(expanded.can(&Msg::SelectDay(date(2026, 3, 5))));
}

#[test]
fn collapsed_year_hides_its_months() {
    let mut state = tree_state(Some((2026, 3)));
    state.date_tree[0].expanded = false;
    let tree = walk(&view(&state));
    assert!(tree.has_key("tree.y.2026"));
    assert!(!tree.has_key_prefix("tree.m."));
    assert!(tree.can(&Msg::ToggleYear(2026)));
    assert!(tree.can(&Msg::SelectYear(2026)));
}

#[test]
fn month_and_year_rows_emit_select_messages() {
    let tree = walk(&view(&tree_state(None)));
    assert!(tree.can(&Msg::SelectMonth {
        year: 2026,
        month: 3
    }));
}

#[test]
fn tag_filter_toggles_between_tag_and_none() {
    let mut state = three_faxes();
    state.all_tags = vec![("invoice".to_string(), 2)];
    let tree = walk(&view(&state));
    assert!(tree.can(&Msg::SetTagFilter(Some("invoice".to_string()))));
    assert_eq!(tree.variant("btn.tag.invoice"), Some(ButtonVariant::Ghost));

    state.query.tag = Some("invoice".to_string());
    let active = walk(&view(&state));
    assert!(active.can(&Msg::SetTagFilter(None)));
    assert_eq!(
        active.variant("btn.tag.invoice"),
        Some(ButtonVariant::Primary)
    );
}

#[test]
fn trash_folder_row_emits_select_folder() {
    let tree = walk(&view(&three_faxes()));
    assert!(tree.can(&Msg::SelectFolder(Folder::Trash)));
    assert!(tree.has_key("folder.unread"));
}

#[test]
fn year_is_active_only_for_full_calendar_year() {
    let mut query = FaxQuery {
        date_from: Some(date(2026, 1, 1)),
        date_to: Some(date(2026, 12, 31)),
        ..FaxQuery::default()
    };
    assert!(year_active(&query, 2026));
    assert!(!year_active(&query, 2025));

    query.date_to = Some(date(2026, 12, 30));
    assert!(!year_active(&query, 2026));
}

#[test]
fn month_is_active_for_first_to_last_day() {
    let query = FaxQuery {
        date_from: Some(date(2024, 2, 1)),
        date_to: Some(date(2024, 2, 29)),
        ..FaxQuery::default()
    };
    assert!(month_active(&query, 2024, 2));
    assert!(!month_active(&query, 2023, 2));
    assert!(!month_active(&query, 2024, 3));
}

#[test]
fn day_is_active_only_for_single_day_range() {
    let mut query = FaxQuery {
        date_from: Some(date(2026, 3, 5)),
        date_to: Some(date(2026, 3, 5)),
        ..FaxQuery::default()
    };
    assert!(day_active(&query, date(2026, 3, 5)));
    query.date_to = Some(date(2026, 3, 6));
    assert!(!day_active(&query, date(2026, 3, 5)));
}
