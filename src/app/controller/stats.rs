//! Statistics window and the activity log (clear).

use super::Controller;
use super::date_range::{days_before, today};
use crate::app::msg::StatsRange;
use chrono::NaiveDate;

/// Inclusive `(from, to)` window ending today for a statistics range.
pub(super) fn stats_window(range: StatsRange, today: NaiveDate) -> (NaiveDate, NaiveDate) {
    let days: u64 = match range {
        StatsRange::Last7Days => 7,
        StatsRange::Last30Days => 30,
        StatsRange::Last365Days => 365,
    };
    (days_before(today, days - 1), today)
}

impl Controller {
    pub(super) fn set_stats_range(&mut self, range: StatsRange) {
        self.state.stats_range = range;
        self.refresh_stats();
    }

    pub(super) fn refresh_stats(&mut self) {
        let (from, to) = stats_window(self.state.stats_range, today());
        if let Some(stats) = self.or_report(self.store.stats(from, to)) {
            self.state.stats = stats;
        }
    }

    /// Confirmed: remove every activity-log line.
    pub(super) fn clear_log(&mut self) {
        if self.or_report(self.store.clear_logs()).is_some() {
            self.refresh_logs();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_span_the_requested_number_of_days() {
        let today = NaiveDate::from_ymd_opt(2026, 3, 10).expect("valid date");
        assert_eq!(
            stats_window(StatsRange::Last7Days, today),
            (NaiveDate::from_ymd_opt(2026, 3, 4).expect("valid"), today)
        );
        assert_eq!(
            stats_window(StatsRange::Last30Days, today).0,
            NaiveDate::from_ymd_opt(2026, 2, 9).expect("valid")
        );
        assert_eq!(
            stats_window(StatsRange::Last365Days, today).0,
            NaiveDate::from_ymd_opt(2025, 3, 11).expect("valid")
        );
    }
}
