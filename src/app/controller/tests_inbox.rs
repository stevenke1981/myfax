//! Inbox behaviour: import summary, selection and read marking, search, date filters,
//! date tree, folders, paging and clearing filters.

use super::tests::{Harness, fax_name, zh};
use crate::app::msg::{DatePreset, Field, Msg};
use crate::app::state::ToastKind;
use crate::i18n::{Lang, tf};
use crate::model::{Folder, SortOrder};
use chrono::{Datelike, Days, Local, NaiveDate};

fn day(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid test date")
}

/// Filename of a fax received on `date` at 09:`minute`.
fn fax_on(date: NaiveDate, minute: u32, number: &str) -> String {
    fax_name(date.year(), date.month(), date.day(), 9, minute, number)
}

#[test]
fn single_import_is_summarised_selected_and_marked_read() {
    let mut h = Harness::new();
    let name = fax_name(2026, 3, 5, 14, 7, "0912345678");
    h.import_fax(&name, 2);

    let summary = tf(
        Lang::ZhTw,
        "app.import_summary",
        &[
            ("imported", "1"),
            ("duplicates", "0"),
            ("skipped", "0"),
            ("failed", "0"),
        ],
    );
    assert_eq!(h.toast(), Some((ToastKind::Success, summary)));
    let selected = h.controller.state.selected.clone().expect("fax selected");
    assert_eq!(selected.original_name, name);
    assert_eq!(selected.pages, 2);
    assert!(selected.is_read, "selecting an imported fax marks it read");
    assert_eq!(h.controller.state.counts.unread, 0);
    assert_eq!(h.controller.state.counts.all, 1);
}

#[test]
fn folder_import_counts_duplicates_and_keeps_selection_empty() {
    let mut h = Harness::new();
    let a = fax_on(day(2026, 3, 5), 1, "0912345678");
    let b = fax_on(day(2026, 3, 5), 2, "0223456789");
    h.write(&format!("drop/{a}"), &super::tests::fax_pdf(1, &a));
    h.write(&format!("drop/{b}"), &super::tests::fax_pdf(1, &b));
    let folder = h.path("drop");

    h.controller.files_dropped(vec![folder.clone()]);
    assert!(h.controller.state.selected.is_none());
    assert_eq!(h.controller.state.counts.unread, 2);
    assert_eq!(h.toast().map(|t| t.0), Some(ToastKind::Success));

    h.controller.files_dropped(vec![folder]);
    let expected = tf(
        Lang::ZhTw,
        "app.import_summary",
        &[
            ("imported", "0"),
            ("duplicates", "2"),
            ("skipped", "0"),
            ("failed", "0"),
        ],
    );
    assert_eq!(h.toast(), Some((ToastKind::Info, expected)));
}

#[test]
fn empty_import_path_and_missing_path_are_reported() {
    let mut h = Harness::new();
    h.controller.update(Msg::ImportFromField);
    assert_eq!(
        h.toast(),
        Some((ToastKind::Error, zh("app.import_no_path").to_owned()))
    );

    let missing = h.path("nope/missing.pdf");
    h.controller
        .set_field(Field::ImportPath, missing.display().to_string());
    h.controller.update(Msg::ImportFromField);
    let (kind, _text) = h.toast().expect("toast");
    assert_eq!(
        kind,
        ToastKind::Error,
        "a failed path makes the summary an error"
    );
    assert_eq!(h.controller.state.counts.all, 0);
}

#[test]
fn search_text_requeries_live_and_trims() {
    let mut h = Harness::new();
    h.import_fax(&fax_on(day(2026, 3, 5), 1, "0912345678"), 1);
    h.import_fax(&fax_on(day(2026, 3, 5), 2, "0223456789"), 1);
    h.import_fax(&fax_on(day(2026, 3, 5), 3, "0987654321"), 1);
    assert_eq!(h.controller.state.results.total, 3);

    h.controller.set_field(Field::Search, "  0223  ".to_owned());
    assert_eq!(h.controller.state.query.text, "0223");
    assert_eq!(h.controller.state.results.total, 1);

    h.controller.set_field(Field::Search, "zzzz".to_owned());
    assert_eq!(h.controller.state.results.total, 0);

    h.controller.set_field(Field::Search, String::new());
    assert_eq!(h.controller.state.results.total, 3);
}

#[test]
fn date_presets_filter_by_day_and_write_the_fields() {
    let mut h = Harness::new();
    let today = Local::now().date_naive();
    let three_days_ago = today.checked_sub_days(Days::new(3)).expect("date");
    let ten_days_ago = today.checked_sub_days(Days::new(10)).expect("date");
    h.import_fax(&fax_on(today, 1, "0911111111"), 1);
    h.import_fax(&fax_on(three_days_ago, 2, "0922222222"), 1);
    h.import_fax(&fax_on(ten_days_ago, 3, "0933333333"), 1);

    h.controller.update(Msg::SetDatePreset(DatePreset::Today));
    assert_eq!(h.controller.state.results.total, 1);
    assert_eq!(h.controller.state.query.date_from, Some(today));
    assert_eq!(
        h.controller.state.field(Field::DateFrom),
        today.format("%Y-%m-%d").to_string()
    );

    h.controller
        .update(Msg::SetDatePreset(DatePreset::Yesterday));
    assert_eq!(h.controller.state.results.total, 0);

    h.controller
        .update(Msg::SetDatePreset(DatePreset::Last7Days));
    assert_eq!(h.controller.state.results.total, 2);
    assert_eq!(
        h.controller.state.query.date_from,
        Some(today.checked_sub_days(Days::new(6)).expect("date"))
    );

    h.controller.update(Msg::SetDatePreset(DatePreset::AnyTime));
    assert_eq!(h.controller.state.results.total, 3);
    assert_eq!(h.controller.state.field(Field::DateFrom), "");
    assert_eq!(h.controller.state.field(Field::DateTo), "");
}

#[test]
fn custom_range_validation_leaves_the_query_unchanged() {
    let mut h = Harness::new();
    h.import_fax(&fax_name(2026, 3, 5, 9, 0, "0912345678"), 1);

    h.controller
        .set_field(Field::DateFrom, "2026-13-01".to_owned());
    h.controller.update(Msg::ApplyCustomRange);
    assert_eq!(
        h.toast(),
        Some((ToastKind::Error, zh("app.date_invalid").to_owned()))
    );
    assert_eq!(h.controller.state.query.date_from, None);
    assert_eq!(h.controller.state.date_preset, DatePreset::AnyTime);

    h.controller
        .set_field(Field::DateFrom, "2026-03-10".to_owned());
    h.controller
        .set_field(Field::DateTo, "2026-03-01".to_owned());
    h.controller.update(Msg::ApplyCustomRange);
    assert_eq!(
        h.toast(),
        Some((ToastKind::Error, zh("app.date_order").to_owned()))
    );
    assert_eq!(h.controller.state.query.date_to, None);

    h.controller
        .set_field(Field::DateFrom, "2026-03-01".to_owned());
    h.controller
        .set_field(Field::DateTo, "2026-03-31".to_owned());
    h.controller.update(Msg::ApplyCustomRange);
    assert_eq!(h.controller.state.date_preset, DatePreset::Custom);
    assert_eq!(h.controller.state.query.date_from, Some(day(2026, 3, 1)));
    assert_eq!(h.controller.state.query.date_to, Some(day(2026, 3, 31)));
    assert_eq!(h.controller.state.results.total, 1);
}

#[test]
fn month_selection_uses_real_month_ends_including_leap_february() {
    let mut h = Harness::new();
    h.controller.update(Msg::SelectMonth {
        year: 2024,
        month: 2,
    });
    assert_eq!(h.controller.state.query.date_from, Some(day(2024, 2, 1)));
    assert_eq!(h.controller.state.query.date_to, Some(day(2024, 2, 29)));
    assert_eq!(h.controller.state.expanded_month, Some((2024, 2)));
    assert_eq!(h.controller.state.date_preset, DatePreset::Custom);

    h.controller.update(Msg::SelectMonth {
        year: 2025,
        month: 2,
    });
    assert_eq!(h.controller.state.query.date_to, Some(day(2025, 2, 28)));

    h.controller.update(Msg::SelectMonth {
        year: 2026,
        month: 12,
    });
    assert_eq!(h.controller.state.query.date_to, Some(day(2026, 12, 31)));

    h.controller.update(Msg::SelectYear(2026));
    assert_eq!(h.controller.state.query.date_from, Some(day(2026, 1, 1)));
    assert_eq!(h.controller.state.query.date_to, Some(day(2026, 12, 31)));

    let picked = day(2026, 3, 5);
    h.controller.update(Msg::SelectDay(picked));
    assert_eq!(h.controller.state.query.date_from, Some(picked));
    assert_eq!(h.controller.state.query.date_to, Some(picked));
}

#[test]
fn year_nodes_toggle_and_keep_their_state_across_refreshes() {
    let mut h = Harness::new();
    h.import_fax(&fax_name(2026, 3, 5, 9, 0, "0912345678"), 1);
    let year = h.controller.state.date_tree[0].year;
    let before = h.controller.state.date_tree[0].expanded;

    h.controller.update(Msg::ToggleYear(year));
    assert_eq!(h.controller.state.date_tree[0].expanded, !before);

    h.import_fax(&fax_name(2026, 3, 6, 9, 0, "0923456789"), 1);
    assert_eq!(
        h.controller.state.date_tree[0].expanded, !before,
        "toggle survives refresh"
    );
    assert_eq!(h.controller.state.date_tree[0].count, 2);
}

#[test]
fn folder_switch_resets_paging_and_clears_selection() {
    let mut h = Harness::new();
    h.import_fax(&fax_name(2026, 3, 5, 9, 0, "0912345678"), 1);
    assert!(h.controller.state.selected.is_some());

    h.controller.update(Msg::SelectFolder(Folder::Unread));
    assert_eq!(h.controller.state.query.folder, Folder::Unread);
    assert!(h.controller.state.selected.is_none());
    assert_eq!(
        h.controller.state.results.total, 0,
        "imported fax was already read"
    );

    h.controller.update(Msg::SelectFolder(Folder::Trash));
    assert_eq!(h.controller.state.query.folder, Folder::Trash);
    h.controller.update(Msg::SelectFolder(Folder::All));
    assert_eq!(h.controller.state.results.total, 1);
}

#[test]
fn paging_clamps_to_the_first_and_last_page() {
    let mut h = Harness::new();
    for i in 0..25 {
        h.import_fax(&fax_name(2026, 3, 5, 10, i, &format!("09000{i:05}")), 1);
    }
    h.controller.state.query.page_size = 10;
    h.controller.update(Msg::SetSort(SortOrder::NewestFirst));
    assert_eq!(h.controller.state.page_count(), 3);
    assert_eq!(h.controller.state.results.items.len(), 10);

    h.controller.update(Msg::PrevPage);
    assert_eq!(h.controller.state.query.page, 0);

    for _ in 0..5 {
        h.controller.update(Msg::NextPage);
    }
    assert_eq!(h.controller.state.query.page, 2);
    assert_eq!(h.controller.state.results.items.len(), 5);

    h.controller.update(Msg::PrevPage);
    assert_eq!(h.controller.state.query.page, 1);
}

#[test]
fn clear_filters_resets_everything_but_the_folder() {
    let mut h = Harness::new();
    h.import_fax(&fax_name(2026, 3, 5, 9, 0, "0912345678"), 1);
    h.controller.update(Msg::SelectFolder(Folder::Starred));
    h.controller.set_field(Field::Search, "09".to_owned());
    h.controller
        .update(Msg::SetDatePreset(DatePreset::ThisYear));
    h.controller
        .update(Msg::SetTagFilter(Some(" Invoice ".to_owned())));
    h.controller.update(Msg::SetSort(SortOrder::SenderAsc));

    assert_eq!(h.controller.state.query.tag.as_deref(), Some("invoice"));
    h.controller.update(Msg::ClearFilters);

    let query = &h.controller.state.query;
    assert_eq!(query.folder, Folder::Starred);
    assert!(query.text.is_empty());
    assert_eq!(query.date_from, None);
    assert_eq!(query.date_to, None);
    assert_eq!(query.tag, None);
    assert_eq!(query.sort, SortOrder::NewestFirst);
    assert_eq!(h.controller.state.date_preset, DatePreset::AnyTime);
    assert_eq!(h.controller.state.field(Field::Search), "");
}

#[test]
fn tag_filter_toggles_off_with_none() {
    let mut h = Harness::new();
    h.import_fax(&fax_name(2026, 3, 5, 9, 0, "0912345678"), 1);
    let id = h.id_of(&fax_name(2026, 3, 5, 9, 0, "0912345678"));
    h.controller.set_field(Field::FaxTags, "invoice".to_owned());
    h.controller.update(Msg::SaveFaxEdits(id));

    h.controller
        .update(Msg::SetTagFilter(Some("INVOICE".to_owned())));
    assert_eq!(h.controller.state.results.total, 1);
    h.controller.update(Msg::SetTagFilter(None));
    assert_eq!(h.controller.state.query.tag, None);
    assert_eq!(h.controller.state.results.total, 1);
}
