//! Inbox filters: folders, date presets and tree, tags, sort and paging.

use super::Controller;
use super::date_range::{
    format_date, month_bounds, parse_date_field, preset_range, today, year_bounds,
};
use crate::app::msg::{DatePreset, Field};
use crate::i18n::t;
use crate::model::{Folder, SortOrder};
use chrono::{Datelike, NaiveDate};

impl Controller {
    /// Switch mailbox folder; resets paging and clears the selection.
    pub(super) fn select_folder(&mut self, folder: Folder) {
        self.state.query.folder = folder;
        self.state.query.page = 0;
        self.clear_selection();
        self.refresh_results();
    }

    pub(super) fn set_date_preset(&mut self, preset: DatePreset) {
        let (from, to) = preset_range(preset, today());
        self.apply_range(preset, from, to);
    }

    /// Apply the typed `DateFrom`..`DateTo` range. Invalid text or an inverted range
    /// reports an error and leaves the query unchanged.
    pub(super) fn apply_custom_range(&mut self) {
        let from = parse_date_field(self.state.field(Field::DateFrom));
        let to = parse_date_field(self.state.field(Field::DateTo));
        let (Ok(from), Ok(to)) = (from, to) else {
            let text = t(self.state.lang, "app.date_invalid").to_owned();
            self.report_error(text);
            return;
        };
        if matches!((from, to), (Some(start), Some(end)) if start > end) {
            let text = t(self.state.lang, "app.date_order").to_owned();
            self.report_error(text);
            return;
        }
        let preset = if from.is_none() && to.is_none() {
            DatePreset::AnyTime
        } else {
            DatePreset::Custom
        };
        self.apply_range(preset, from, to);
    }

    /// Set the date range in the query and the date fields, then re-query from page 0.
    fn apply_range(&mut self, preset: DatePreset, from: Option<NaiveDate>, to: Option<NaiveDate>) {
        self.state.date_preset = preset;
        self.state.query.date_from = from;
        self.state.query.date_to = to;
        self.state.fields.insert(Field::DateFrom, format_date(from));
        self.state.fields.insert(Field::DateTo, format_date(to));
        self.state.query.page = 0;
        self.refresh_results();
    }

    fn apply_bounds(&mut self, bounds: Option<(NaiveDate, NaiveDate)>) {
        if let Some((from, to)) = bounds {
            self.apply_range(DatePreset::Custom, Some(from), Some(to));
        }
    }

    /// Flip a year node between expanded and collapsed (remembered across refreshes).
    pub(super) fn toggle_year(&mut self, year: i32) {
        let current = self
            .year_expanded
            .get(&year)
            .copied()
            .unwrap_or(year == today().year());
        let next = !current;
        self.year_expanded.insert(year, next);
        for node in self
            .state
            .date_tree
            .iter_mut()
            .filter(|node| node.year == year)
        {
            node.expanded = next;
        }
    }

    pub(super) fn select_year(&mut self, year: i32) {
        self.apply_bounds(year_bounds(year));
    }

    pub(super) fn select_month(&mut self, year: i32, month: u32) {
        let Some(bounds) = month_bounds(year, month) else {
            return;
        };
        self.state.expanded_month = Some((year, month));
        self.apply_bounds(Some(bounds));
    }

    pub(super) fn select_day(&mut self, day: NaiveDate) {
        self.apply_range(DatePreset::Custom, Some(day), Some(day));
    }

    pub(super) fn set_tag_filter(&mut self, tag: Option<String>) {
        self.state.query.tag = tag
            .map(|value| value.trim().to_lowercase())
            .filter(|value| !value.is_empty());
        self.state.query.page = 0;
        self.refresh_results();
    }

    pub(super) fn set_sort(&mut self, sort: SortOrder) {
        self.state.query.sort = sort;
        self.state.query.page = 0;
        self.refresh_results();
    }

    /// Reset text, dates, tag, sort and preset. The folder is kept.
    pub(super) fn clear_filters(&mut self) {
        self.state.query.text.clear();
        self.state.query.date_from = None;
        self.state.query.date_to = None;
        self.state.query.tag = None;
        self.state.query.sort = SortOrder::default();
        self.state.query.page = 0;
        self.state.date_preset = DatePreset::AnyTime;
        for field in [Field::Search, Field::DateFrom, Field::DateTo] {
            self.state.fields.insert(field, String::new());
        }
        self.refresh_results();
    }

    pub(super) fn prev_page(&mut self) {
        self.state.query.page = self.state.query.page.saturating_sub(1);
        self.refresh_results();
    }

    pub(super) fn next_page(&mut self) {
        let last = self.state.page_count().saturating_sub(1);
        self.state.query.page = self.state.query.page.saturating_add(1).min(last);
        self.refresh_results();
    }
}
