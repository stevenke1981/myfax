//! Shared domain types (contract owned by the orchestrator).
//!
//! Every module communicates through these types. Changing a field here is a
//! cross-team change: only the orchestrator edits this file.

use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Database identity of an archived fax.
pub type FaxId = i64;
/// Database identity of an address-book contact.
pub type ContactId = i64;

/// Where an archived fax came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaxSource {
    /// Picked up from a watched inbox folder (printer "save to network folder").
    WatchFolder,
    /// Imported manually (file picker path, drag and drop).
    Manual,
}

impl FaxSource {
    pub fn as_db(self) -> &'static str {
        match self {
            Self::WatchFolder => "watch",
            Self::Manual => "manual",
        }
    }
    pub fn from_db(value: &str) -> Self {
        match value {
            "watch" => Self::WatchFolder,
            _ => Self::Manual,
        }
    }
}

/// Values needed to insert a newly archived fax.
#[derive(Clone, Debug, PartialEq)]
pub struct NewFax {
    pub received_at: DateTime<Local>,
    /// Normalised sender number (digits and a leading `+` only), if known.
    pub sender_number: Option<String>,
    /// Display name resolved from the address book or typed by the user.
    pub sender_name: Option<String>,
    pub pages: u32,
    /// Absolute path of the archived file (inside the archive root).
    pub file_path: PathBuf,
    pub file_size: u64,
    /// Lower-case hex SHA-256 of the file contents.
    pub sha256: String,
    /// File name as it arrived (before renaming).
    pub original_name: String,
    pub source: FaxSource,
}

/// One archived fax as stored in the index.
#[derive(Clone, Debug, PartialEq)]
pub struct FaxRecord {
    pub id: FaxId,
    pub received_at: DateTime<Local>,
    pub sender_number: Option<String>,
    pub sender_name: Option<String>,
    pub pages: u32,
    pub file_path: PathBuf,
    pub file_size: u64,
    pub sha256: String,
    pub original_name: String,
    pub source: FaxSource,
    pub is_read: bool,
    pub is_starred: bool,
    /// Lower-case, trimmed, de-duplicated tags, sorted ascending.
    pub tags: Vec<String>,
    pub note: String,
    /// `Some` when the fax is in the trash.
    pub deleted_at: Option<DateTime<Local>>,
    pub imported_at: DateTime<Local>,
}

impl FaxRecord {
    /// Name shown in lists: sender name, else number, else `None` (caller shows "unknown").
    pub fn display_sender(&self) -> Option<&str> {
        self.sender_name
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or(self.sender_number.as_deref())
    }
}

/// Which mailbox view is active.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Folder {
    /// All non-deleted faxes.
    #[default]
    All,
    Unread,
    Starred,
    Trash,
}

/// Sort order for search results.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortOrder {
    #[default]
    NewestFirst,
    OldestFirst,
    SenderAsc,
}

/// Search / filter request. Empty fields mean "no constraint".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FaxQuery {
    pub folder: Folder,
    /// Free text: matched case-insensitively against sender name, sender
    /// number, note, tags and original file name (substring, all words must match).
    pub text: String,
    /// Inclusive local date range on `received_at`.
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    /// Exact tag match (lower-case).
    pub tag: Option<String>,
    pub sort: SortOrder,
    /// Zero-based page index.
    pub page: u32,
    /// Items per page (> 0).
    pub page_size: u32,
}

/// One page of search results.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SearchPage {
    pub items: Vec<FaxRecord>,
    /// Total matches across all pages.
    pub total: u64,
}

/// Fax count for a calendar day (non-deleted only).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateBucket {
    pub date: NaiveDate,
    pub count: u64,
}

/// Mailbox badge counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FolderCounts {
    pub all: u64,
    pub unread: u64,
    pub starred: u64,
    pub trash: u64,
}

/// Aggregates for the statistics page (non-deleted faxes in the range).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stats {
    pub total_faxes: u64,
    pub total_pages: u64,
    /// One entry per day in the range that has faxes, ascending.
    pub per_day: Vec<DateBucket>,
    /// (display sender, count), descending by count then name, at most 10.
    pub top_senders: Vec<(String, u64)>,
}

/// Address-book entry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Contact {
    pub id: ContactId,
    pub name: String,
    pub company: String,
    /// Normalised fax number (see [`normalize_number`]).
    pub fax_number: String,
    pub phone: String,
    pub email: String,
    pub note: String,
}

/// Severity of an activity-log entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
    pub fn from_db(value: &str) -> Self {
        match value {
            "warn" => Self::Warn,
            "error" => Self::Error,
            _ => Self::Info,
        }
    }
}

/// Activity-log entry. `message` is already human-readable (stored in the
/// language active when it was written).
#[derive(Clone, Debug, PartialEq)]
pub struct LogEntry {
    pub id: i64,
    pub at: DateTime<Local>,
    pub level: LogLevel,
    pub message: String,
}

/// A fax handed to a printer fax driver from the "Send" page.
#[derive(Clone, Debug, PartialEq)]
pub struct SentFax {
    pub id: i64,
    pub at: DateTime<Local>,
    pub file_path: PathBuf,
    pub printer: String,
    pub recipient: String,
    pub note: String,
}

/// RGBA8 page bitmap produced by the preview renderer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageImage {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes, row-major, non-premultiplied RGBA.
    pub rgba: Vec<u8>,
}

/// Normalise a phone / fax number: keep ASCII digits, keep one leading `+`.
/// Returns `None` when no digit remains.
///
/// ```
/// assert_eq!(myfax::model::normalize_number(" +886 (2) 1234-5678 "), Some("+886212345678".into()));
/// assert_eq!(myfax::model::normalize_number("ext"), None);
/// ```
pub fn normalize_number(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let plus = trimmed.starts_with('+');
    let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    Some(if plus { format!("+{digits}") } else { digits })
}

/// Normalise a list of tags: trim, lower-case, drop empties, dedupe, sort.
pub fn normalize_tags<I, S>(tags: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut out: Vec<String> = tags
        .into_iter()
        .map(|t| t.as_ref().trim().to_lowercase())
        .filter(|t| !t.is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_number_keeps_plus_and_digits() {
        assert_eq!(normalize_number("02-2345 6789"), Some("0223456789".into()));
        assert_eq!(normalize_number("+1 (800) 555"), Some("+1800555".into()));
        assert_eq!(normalize_number("  "), None);
    }

    #[test]
    fn normalize_tags_dedupes_and_sorts() {
        assert_eq!(
            normalize_tags([" Invoice", "invoice", "", "Urgent "]),
            vec!["invoice".to_string(), "urgent".to_string()]
        );
    }

    #[test]
    fn display_sender_prefers_name() {
        let mut record = FaxRecord {
            id: 1,
            received_at: Local::now(),
            sender_number: Some("123".into()),
            sender_name: Some("  ".into()),
            pages: 1,
            file_path: PathBuf::new(),
            file_size: 0,
            sha256: String::new(),
            original_name: String::new(),
            source: FaxSource::Manual,
            is_read: false,
            is_starred: false,
            tags: vec![],
            note: String::new(),
            deleted_at: None,
            imported_at: Local::now(),
        };
        assert_eq!(record.display_sender(), Some("123"));
        record.sender_name = Some("ACME".into());
        assert_eq!(record.display_sender(), Some("ACME"));
    }
}
