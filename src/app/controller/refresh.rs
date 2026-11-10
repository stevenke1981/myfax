//! Re-reads store data into `AppState`. Every mutation ends in one of these.

use super::Controller;
use super::date_range::today;
use super::date_tree;
use crate::app::msg::Field;
use chrono::Datelike;

/// Activity-log lines shown on the log page.
const LOG_LIMIT: u32 = 200;
/// Sent-fax history rows kept in state.
const SENT_LIMIT: u32 = 100;

/// Zero-based index of the last result page (at least 0).
fn last_page_index(total: u64, page_size: u32) -> u32 {
    let size = u64::from(page_size.max(1));
    let pages = total.div_ceil(size).max(1);
    u32::try_from(pages - 1).unwrap_or(u32::MAX)
}

impl Controller {
    /// Refresh every list, count and tree in the view-model.
    pub(super) fn refresh_all(&mut self) {
        self.refresh_results();
        self.refresh_tree();
        self.refresh_contacts();
        self.refresh_logs();
        self.refresh_sent();
        self.refresh_stats();
    }

    /// Re-run the inbox query, clamping the page, then refresh counts and selection.
    pub(super) fn refresh_results(&mut self) {
        let Some(mut results) = self.or_report(self.store.search(&self.state.query)) else {
            return;
        };
        let last = last_page_index(results.total, self.state.query.page_size);
        if self.state.query.page > last {
            self.state.query.page = last;
            let Some(again) = self.or_report(self.store.search(&self.state.query)) else {
                return;
            };
            results = again;
        }
        self.state.results = results;
        self.refresh_counts();
        self.refresh_selected();
    }

    pub(super) fn refresh_counts(&mut self) {
        if let Some(counts) = self.or_report(self.store.folder_counts()) {
            self.state.counts = counts;
        }
    }

    /// Re-fetch the selected fax by id; clears the selection when it no longer exists.
    fn refresh_selected(&mut self) {
        let Some(id) = self.state.selected.as_ref().map(|record| record.id) else {
            return;
        };
        match self.or_report(self.store.get_fax(id)) {
            Some(Some(record)) => self.state.selected = Some(record),
            Some(None) => self.clear_selection(),
            None => {}
        }
    }

    /// Date tree (year/month/day) and tag cloud.
    pub(super) fn refresh_tree(&mut self) {
        let buckets = self.or_report(self.store.date_buckets());
        let tags = self.or_report(self.store.tag_counts());
        if let Some(buckets) = buckets {
            self.state.date_tree = date_tree::build(&buckets, today().year(), &self.year_expanded);
        }
        if let Some(tags) = tags {
            self.state.all_tags = tags;
        }
    }

    /// Address book, filtered by the contact search field.
    pub(super) fn refresh_contacts(&mut self) {
        let filter = self.state.field(Field::ContactSearch).to_owned();
        if let Some(contacts) = self.or_report(self.store.list_contacts(&filter)) {
            self.state.contacts = contacts;
        }
    }

    pub(super) fn refresh_logs(&mut self) {
        if let Some(logs) = self.or_report(self.store.recent_logs(LOG_LIMIT)) {
            self.state.logs = logs;
        }
    }

    pub(super) fn refresh_sent(&mut self) {
        if let Some(sent) = self.or_report(self.store.list_sent(SENT_LIMIT)) {
            self.state.sent = sent;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::last_page_index;

    #[test]
    fn last_page_index_handles_empty_and_exact_pages() {
        assert_eq!(last_page_index(0, 10), 0);
        assert_eq!(last_page_index(10, 10), 0);
        assert_eq!(last_page_index(11, 10), 1);
        assert_eq!(last_page_index(101, 50), 2);
        assert_eq!(last_page_index(5, 0), 4);
    }
}
