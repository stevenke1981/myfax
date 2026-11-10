//! Extract receive time and sender number from printer-generated file names.

use chrono::{DateTime, Local};

#[derive(Clone, Debug, PartialEq)]
pub struct ParsedName {
    pub received_at: DateTime<Local>,
    pub sender_number: Option<String>,
}

/// Parse `name` (file name only). Recognised date-times: `YYYYMMDD[_-]?HHMMSS`,
/// `YYYY-MM-DD[_ T]HH[-.:]MM[-.:]SS`, `YYYYMMDDHHMM`. Sender: the longest digit
/// run of 6..=20 digits (optionally `+`-prefixed) that is not part of the
/// date-time, normalised. Falls back to `fallback` (file modified time) when
/// no valid date-time is found (invalid calendar values count as not found).
#[allow(unused_variables)]
pub fn parse(name: &str, fallback: DateTime<Local>) -> ParsedName {
    todo!()
}
