//! Pure date helpers for inbox filters and the statistics window.

use crate::app::msg::DatePreset;
use chrono::{Datelike, Days, Local, NaiveDate};

/// Wire format of the date text fields.
const DATE_FORMAT: &str = "%Y-%m-%d";

/// Local calendar date right now.
pub(super) fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// `YYYY-MM-DD`, or the empty string for an open bound.
pub(super) fn format_date(date: Option<NaiveDate>) -> String {
    date.map(|day| day.format(DATE_FORMAT).to_string())
        .unwrap_or_default()
}

/// A date text field that is neither empty nor `YYYY-MM-DD`.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct InvalidDate;

/// Parse an optional date field. Empty (after trimming) means an open bound.
pub(super) fn parse_date_field(text: &str) -> Result<Option<NaiveDate>, InvalidDate> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    NaiveDate::parse_from_str(text, DATE_FORMAT)
        .map(Some)
        .map_err(|_| InvalidDate)
}

/// First and last day of a calendar month (leap years included).
pub(super) fn month_bounds(year: i32, month: u32) -> Option<(NaiveDate, NaiveDate)> {
    let first = NaiveDate::from_ymd_opt(year, month, 1)?;
    let next_first = if month == 12 {
        NaiveDate::from_ymd_opt(year.checked_add(1)?, 1, 1)?
    } else {
        NaiveDate::from_ymd_opt(year, month.checked_add(1)?, 1)?
    };
    Some((first, next_first.pred_opt()?))
}

/// January 1 and December 31 of a year.
pub(super) fn year_bounds(year: i32) -> Option<(NaiveDate, NaiveDate)> {
    Some((
        NaiveDate::from_ymd_opt(year, 1, 1)?,
        NaiveDate::from_ymd_opt(year, 12, 31)?,
    ))
}

/// `day` minus `days` calendar days (saturates to `day` on overflow).
pub(super) fn days_before(day: NaiveDate, days: u64) -> NaiveDate {
    day.checked_sub_days(Days::new(days)).unwrap_or(day)
}

/// Inclusive range for a quick preset. `AnyTime` and `Custom` are open.
pub(super) fn preset_range(
    preset: DatePreset,
    today: NaiveDate,
) -> (Option<NaiveDate>, Option<NaiveDate>) {
    let single_day = (Some(today), Some(today));
    match preset {
        DatePreset::AnyTime | DatePreset::Custom => (None, None),
        DatePreset::Today => single_day,
        DatePreset::Yesterday => {
            let day = days_before(today, 1);
            (Some(day), Some(day))
        }
        DatePreset::Last7Days => (Some(days_before(today, 6)), Some(today)),
        DatePreset::Last30Days => (Some(days_before(today, 29)), Some(today)),
        DatePreset::ThisMonth => month_bounds(today.year(), today.month())
            .map_or(single_day, |(from, to)| (Some(from), Some(to))),
        DatePreset::ThisYear => {
            year_bounds(today.year()).map_or(single_day, |(from, to)| (Some(from), Some(to)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid test date")
    }

    #[test]
    fn february_end_respects_leap_years() {
        assert_eq!(
            month_bounds(2024, 2),
            Some((day(2024, 2, 1), day(2024, 2, 29)))
        );
        assert_eq!(
            month_bounds(2025, 2),
            Some((day(2025, 2, 1), day(2025, 2, 28)))
        );
        assert_eq!(
            month_bounds(2026, 12),
            Some((day(2026, 12, 1), day(2026, 12, 31)))
        );
        assert_eq!(month_bounds(2026, 13), None);
    }

    #[test]
    fn year_bounds_cover_the_whole_year() {
        assert_eq!(
            year_bounds(2024),
            Some((day(2024, 1, 1), day(2024, 12, 31)))
        );
    }

    #[test]
    fn date_field_parsing_accepts_empty_and_iso_only() {
        assert_eq!(parse_date_field("  "), Ok(None));
        assert_eq!(parse_date_field(" 2026-03-01 "), Ok(Some(day(2026, 3, 1))));
        assert_eq!(parse_date_field("2026-02-30"), Err(InvalidDate));
        assert_eq!(parse_date_field("03/01/2026"), Err(InvalidDate));
    }

    #[test]
    fn presets_span_the_expected_days() {
        let today = day(2026, 3, 10);
        assert_eq!(
            preset_range(DatePreset::Today, today),
            (Some(today), Some(today))
        );
        assert_eq!(
            preset_range(DatePreset::Yesterday, today),
            (Some(day(2026, 3, 9)), Some(day(2026, 3, 9)))
        );
        assert_eq!(
            preset_range(DatePreset::Last7Days, today),
            (Some(day(2026, 3, 4)), Some(today))
        );
        assert_eq!(
            preset_range(DatePreset::Last30Days, today),
            (Some(day(2026, 2, 9)), Some(today))
        );
        assert_eq!(
            preset_range(DatePreset::ThisMonth, today),
            (Some(day(2026, 3, 1)), Some(day(2026, 3, 31)))
        );
        assert_eq!(preset_range(DatePreset::AnyTime, today), (None, None));
    }

    #[test]
    fn format_date_round_trips() {
        assert_eq!(format_date(Some(day(2026, 3, 1))), "2026-03-01");
        assert_eq!(format_date(None), "");
    }
}
