//! Extract receive time and sender number from printer-generated file names.

use chrono::{DateTime, Local, NaiveDate, NaiveTime, TimeZone};
use std::ops::Range;

#[derive(Clone, Debug, PartialEq)]
pub struct ParsedName {
    pub received_at: DateTime<Local>,
    pub sender_number: Option<String>,
}

const SENDER_MIN_DIGITS: usize = 6;
const SENDER_MAX_DIGITS: usize = 20;
/// `YYYYMMDD` (compact date).
const COMPACT_DATE_LEN: usize = 8;
/// `HHMMSS` (compact time).
const COMPACT_TIME_LEN: usize = 6;
/// `YYYYMMDDHHMMSS`.
const COMPACT_SECONDS_LEN: usize = 14;
/// `YYYYMMDDHHMM`.
const COMPACT_MINUTES_LEN: usize = 12;

/// Layout of `YYYY-MM-DD[_ T]HH[-.:]MM[-.:]SS`: (digit width, allowed separator
/// bytes before the field). The first field has no separator.
const DASHED_LAYOUT: [(usize, &[u8]); 6] = [
    (4, b""),
    (2, b"-"),
    (2, b"-"),
    (2, b"_ T"),
    (2, b"-.:"),
    (2, b"-.:"),
];

/// One recognised date-time shape. `at` is `None` when the shape matched but the
/// calendar values are invalid or do not exist in the local time zone.
struct Shape {
    span: Range<usize>,
    at: Option<DateTime<Local>>,
}

/// Parse `name` (file name only). Recognised date-times: `YYYYMMDD[_-]?HHMMSS`,
/// `YYYY-MM-DD[_ T]HH[-.:]MM[-.:]SS`, `YYYYMMDDHHMM`. Sender: the longest digit
/// run of 6..=20 digits (optionally `+`-prefixed) that is not part of the
/// date-time, normalised. Falls back to `fallback` (file modified time) when
/// no valid date-time is found (invalid calendar values count as not found).
///
/// Notes: the `YYYYMMDD[_-]HHMMSS` pair and the dashed form are excluded from the
/// sender even when their calendar values are invalid; a lone 12- or 14-digit run
/// is excluded only when it is a valid date-time, so an invalid 12-digit
/// international sender number stays eligible. Spaced sender numbers such as
/// `+886 2 2345 6789` are not joined, so their sender is `None`.
pub fn parse(name: &str, fallback: DateTime<Local>) -> ParsedName {
    let bytes = name.as_bytes();
    let runs = digit_runs(bytes);
    let mut shapes = compact_shapes(bytes, &runs);
    shapes.extend(dashed_shapes(bytes));
    shapes.sort_by_key(|shape| shape.span.start);
    let received_at = shapes.iter().find_map(|shape| shape.at).unwrap_or(fallback);
    ParsedName {
        received_at,
        sender_number: sender_in(bytes, &runs, &shapes),
    }
}

/// Maximal runs of ASCII digits, in order.
fn digit_runs(bytes: &[u8]) -> Vec<Range<usize>> {
    let mut runs = Vec::new();
    let mut start = None;
    for (index, byte) in bytes.iter().enumerate() {
        match (byte.is_ascii_digit(), start) {
            (true, None) => start = Some(index),
            (false, Some(begin)) => {
                runs.push(begin..index);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(begin) = start {
        runs.push(begin..bytes.len());
    }
    runs
}

/// Shapes made of whole digit runs: `YYYYMMDD[_-]HHMMSS`, `YYYYMMDDHHMMSS`,
/// `YYYYMMDDHHMM`.
fn compact_shapes(bytes: &[u8], runs: &[Range<usize>]) -> Vec<Shape> {
    let mut shapes = Vec::new();
    let mut index = 0;
    while index < runs.len() {
        let run = &runs[index];
        let pair = runs.get(index + 1).filter(|next| {
            run.len() == COMPACT_DATE_LEN
                && next.len() == COMPACT_TIME_LEN
                && is_pair_gap(&bytes[run.end..next.start])
        });
        if let Some(next) = pair {
            let digits = [&bytes[run.clone()], &bytes[next.clone()]].concat();
            shapes.push(Shape {
                span: run.start..next.end,
                at: compact_local(&digits),
            });
            index += 2;
            continue;
        }
        // A bare 12/14-digit run is a date-time only when valid; an invalid one may
        // be a 12-digit international sender number and stays eligible.
        let lone_shape = run.len() == COMPACT_SECONDS_LEN || run.len() == COMPACT_MINUTES_LEN;
        if lone_shape && let Some(at) = compact_local(&bytes[run.clone()]) {
            shapes.push(Shape {
                span: run.clone(),
                at: Some(at),
            });
        }
        index += 1;
    }
    shapes
}

fn is_pair_gap(gap: &[u8]) -> bool {
    gap == b"_".as_slice() || gap == b"-".as_slice()
}

/// `YYYY-MM-DD[_ T]HH[-.:]MM[-.:]SS` anywhere in the name, bounded by non-digits.
fn dashed_shapes(bytes: &[u8]) -> Vec<Shape> {
    let mut shapes = Vec::new();
    for start in 0..bytes.len() {
        let preceded_by_digit = start > 0 && bytes[start - 1].is_ascii_digit();
        if preceded_by_digit || !bytes[start].is_ascii_digit() {
            continue;
        }
        let Some(end) = dashed_end(bytes, start) else {
            continue;
        };
        if bytes.get(end).is_some_and(u8::is_ascii_digit) {
            continue;
        }
        let digits: Vec<u8> = bytes[start..end]
            .iter()
            .copied()
            .filter(u8::is_ascii_digit)
            .collect();
        shapes.push(Shape {
            span: start..end,
            at: compact_local(&digits),
        });
    }
    shapes
}

/// End offset of a dashed date-time that starts at `start`, if the layout matches.
fn dashed_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut pos = start;
    for (index, &(width, separators)) in DASHED_LAYOUT.iter().enumerate() {
        if index > 0 {
            let separator = *bytes.get(pos)?;
            if !separators.contains(&separator) {
                return None;
            }
            pos += 1;
        }
        let field = bytes.get(pos..pos + width)?;
        if !field.iter().all(u8::is_ascii_digit) {
            return None;
        }
        pos += width;
    }
    Some(pos)
}

/// Build a local date-time from `YYYYMMDDHHMM` or `YYYYMMDDHHMMSS` digits.
/// Ambiguous local times resolve to the earlier instant; nonexistent ones and
/// invalid calendar values yield `None`.
fn compact_local(digits: &[u8]) -> Option<DateTime<Local>> {
    let seconds = match digits.len() {
        COMPACT_SECONDS_LEN => field(digits, 12..14)?,
        COMPACT_MINUTES_LEN => 0,
        _ => return None,
    };
    let date = NaiveDate::from_ymd_opt(
        field(digits, 0..4)? as i32,
        field(digits, 4..6)?,
        field(digits, 6..8)?,
    )?;
    let time = NaiveTime::from_hms_opt(field(digits, 8..10)?, field(digits, 10..12)?, seconds)?;
    Local.from_local_datetime(&date.and_time(time)).earliest()
}

fn field(digits: &[u8], range: Range<usize>) -> Option<u32> {
    std::str::from_utf8(digits.get(range)?).ok()?.parse().ok()
}

/// Longest eligible digit run that does not overlap any date-time shape.
/// Ties keep the earlier run. A directly preceding `+` is kept.
fn sender_in(bytes: &[u8], runs: &[Range<usize>], shapes: &[Shape]) -> Option<String> {
    let mut best: Option<(usize, String)> = None;
    for run in runs {
        let len = run.len();
        if !(SENDER_MIN_DIGITS..=SENDER_MAX_DIGITS).contains(&len) {
            continue;
        }
        if shapes.iter().any(|shape| overlaps(&shape.span, run)) {
            continue;
        }
        if best.as_ref().is_some_and(|(best_len, _)| *best_len >= len) {
            continue;
        }
        let Ok(digits) = std::str::from_utf8(&bytes[run.clone()]) else {
            continue;
        };
        let plus = run.start > 0 && bytes[run.start - 1] == b'+';
        let number = if plus {
            format!("+{digits}")
        } else {
            digits.to_owned()
        };
        best = Some((len, number));
    }
    best.map(|(_, number)| number)
}

fn overlaps(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start < b.end && b.start < a.end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(y, mo, d, h, mi, s)
            .single()
            .expect("test time exists")
    }

    fn fallback() -> DateTime<Local> {
        local(2000, 1, 1, 0, 0, 0)
    }

    fn sender(text: &str) -> Option<String> {
        parse(text, fallback()).sender_number
    }

    #[test]
    fn epson_underscore_name_gives_time_and_sender() {
        let parsed = parse("FAX_20261110_103045_0912345678.pdf", fallback());
        assert_eq!(parsed.received_at, local(2026, 11, 10, 10, 30, 45));
        assert_eq!(parsed.sender_number.as_deref(), Some("0912345678"));
    }

    #[test]
    fn compact_fourteen_digits_without_sender() {
        let parsed = parse("20261110103045.pdf", fallback());
        assert_eq!(parsed.received_at, local(2026, 11, 10, 10, 30, 45));
        assert_eq!(parsed.sender_number, None);
    }

    #[test]
    fn dashed_date_with_spaced_sender_has_no_sender() {
        let parsed = parse(
            "Fax 2026-11-10 10.30.45 from +886 2 2345 6789.pdf",
            fallback(),
        );
        assert_eq!(parsed.received_at, local(2026, 11, 10, 10, 30, 45));
        assert_eq!(parsed.sender_number, None);
    }

    #[test]
    fn twelve_digit_minutes_form_gives_zero_seconds() {
        let parsed = parse("RX_202611101030_0223456789.pdf", fallback());
        assert_eq!(parsed.received_at, local(2026, 11, 10, 10, 30, 0));
        assert_eq!(parsed.sender_number.as_deref(), Some("0223456789"));
    }

    #[test]
    fn short_numeric_name_falls_back() {
        let parsed = parse("scan0001.pdf", fallback());
        assert_eq!(parsed.received_at, fallback());
        assert_eq!(parsed.sender_number, None);
    }

    #[test]
    fn invalid_month_falls_back_and_sender_is_not_the_date_part() {
        let parsed = parse("20261340_250000.pdf", fallback());
        assert_eq!(parsed.received_at, fallback());
        assert_eq!(parsed.sender_number, None);
    }

    #[test]
    fn invalid_hour_in_compact_form_falls_back() {
        assert_eq!(
            parse("20261110250000.pdf", fallback()).received_at,
            fallback()
        );
    }

    #[test]
    fn impossible_calendar_day_falls_back() {
        assert_eq!(
            parse("FAX_20260230_103045.pdf", fallback()).received_at,
            fallback()
        );
    }

    #[test]
    fn plus_prefix_is_kept_on_sender() {
        assert_eq!(
            sender("FAX_20261110_103045_+886223456789.pdf").as_deref(),
            Some("+886223456789")
        );
    }

    #[test]
    fn longest_eligible_run_wins_and_ties_keep_first() {
        assert_eq!(
            sender("FAX_20261110_103045_12345678_0912345678.pdf").as_deref(),
            Some("0912345678")
        );
        assert_eq!(
            sender("FAX_20261110_103045_1234567890_0987654321.pdf").as_deref(),
            Some("1234567890")
        );
    }

    #[test]
    fn runs_over_twenty_digits_or_under_six_are_not_senders() {
        assert_eq!(
            sender("FAX_20261110_103045_123456789012345678901.pdf"),
            None
        );
        assert_eq!(sender("FAX_20261110_103045_12345.pdf"), None);
    }

    #[test]
    fn digits_inside_a_longer_run_are_not_a_date() {
        // 9 leading digits: the 8-digit window is not a whole run.
        let parsed = parse("9202611101030_0912345678.pdf", fallback());
        assert_eq!(parsed.received_at, fallback());
    }

    #[test]
    fn chinese_name_keeps_date_and_sender() {
        let parsed = parse("傳真_20261110_103045_0223456789.pdf", fallback());
        assert_eq!(parsed.received_at, local(2026, 11, 10, 10, 30, 45));
        assert_eq!(parsed.sender_number.as_deref(), Some("0223456789"));
    }
}
