//! SQLite index of archived faxes, contacts, sent log and activity log.
//! Owner: store worker. Public API = orchestrator contract.
//!
//! One `Store` per thread (the watcher thread opens its own on the same file).
//! The database uses WAL mode and `busy_timeout` so two connections coexist.

pub mod csv;

use crate::model::{
    Contact, ContactId, DateBucket, FaxId, FaxQuery, FaxRecord, FolderCounts, LogEntry, LogLevel,
    NewFax, SearchPage, SentFax, Stats,
};
use chrono::{DateTime, Local, NaiveDate};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not found")]
    NotFound,
    #[error("invalid data: {0}")]
    Invalid(String),
}

pub type StoreResult<T> = Result<T, StoreError>;

pub struct Store {
    _conn: rusqlite::Connection,
}

#[allow(unused_variables)]
impl Store {
    /// Open (creating parent dirs and schema / running migrations).
    pub fn open(path: &Path) -> StoreResult<Self> {
        todo!()
    }
    pub fn open_in_memory() -> StoreResult<Self> {
        todo!()
    }

    // ── faxes ──
    pub fn insert_fax(&self, new: &NewFax) -> StoreResult<FaxId> {
        todo!()
    }
    /// Any fax (including trashed) with this content hash.
    pub fn find_by_hash(&self, sha256: &str) -> StoreResult<Option<FaxId>> {
        todo!()
    }
    pub fn get_fax(&self, id: FaxId) -> StoreResult<Option<FaxRecord>> {
        todo!()
    }
    pub fn search(&self, query: &FaxQuery) -> StoreResult<SearchPage> {
        todo!()
    }
    /// Every match of `query` ignoring paging (for export).
    pub fn search_all(&self, query: &FaxQuery) -> StoreResult<Vec<FaxRecord>> {
        todo!()
    }
    pub fn set_read(&self, id: FaxId, read: bool) -> StoreResult<()> {
        todo!()
    }
    /// Marks every non-deleted fax read; returns rows changed.
    pub fn mark_all_read(&self) -> StoreResult<u64> {
        todo!()
    }
    pub fn set_starred(&self, id: FaxId, starred: bool) -> StoreResult<()> {
        todo!()
    }
    /// Replace tags (normalised with `model::normalize_tags`).
    pub fn set_tags(&self, id: FaxId, tags: &[String]) -> StoreResult<()> {
        todo!()
    }
    pub fn set_note(&self, id: FaxId, note: &str) -> StoreResult<()> {
        todo!()
    }
    /// Empty / whitespace name stores NULL.
    pub fn set_sender_name(&self, id: FaxId, name: Option<&str>) -> StoreResult<()> {
        todo!()
    }
    pub fn set_file_path(&self, id: FaxId, path: &Path) -> StoreResult<()> {
        todo!()
    }
    pub fn move_to_trash(&self, id: FaxId, at: DateTime<Local>) -> StoreResult<()> {
        todo!()
    }
    pub fn restore(&self, id: FaxId) -> StoreResult<()> {
        todo!()
    }
    /// Deletes the row and returns the record (caller deletes the file).
    pub fn delete_permanently(&self, id: FaxId) -> StoreResult<FaxRecord> {
        todo!()
    }
    /// Trashed records with `deleted_at < cutoff` (caller deletes files + rows).
    pub fn trashed_before(&self, cutoff: DateTime<Local>) -> StoreResult<Vec<FaxRecord>> {
        todo!()
    }
    /// All trashed records.
    pub fn trashed(&self) -> StoreResult<Vec<FaxRecord>> {
        todo!()
    }
    /// Per-day counts of non-deleted faxes, ascending by date.
    pub fn date_buckets(&self) -> StoreResult<Vec<DateBucket>> {
        todo!()
    }
    pub fn folder_counts(&self) -> StoreResult<FolderCounts> {
        todo!()
    }
    /// (tag, count) over non-deleted faxes, count desc then tag asc.
    pub fn tag_counts(&self) -> StoreResult<Vec<(String, u64)>> {
        todo!()
    }
    /// Inclusive local-date range.
    pub fn stats(&self, from: NaiveDate, to: NaiveDate) -> StoreResult<Stats> {
        todo!()
    }
    /// Set `sender_name` on every fax whose `sender_number` equals `number`
    /// and whose name is NULL; returns rows changed.
    pub fn fill_sender_names(&self, number: &str, name: &str) -> StoreResult<u64> {
        todo!()
    }

    // ── contacts ──
    /// Case-insensitive substring filter over name/company/number/email; "" = all.
    /// Sorted by name (case-insensitive).
    pub fn list_contacts(&self, filter: &str) -> StoreResult<Vec<Contact>> {
        todo!()
    }
    pub fn get_contact(&self, id: ContactId) -> StoreResult<Option<Contact>> {
        todo!()
    }
    /// `id == 0` inserts, otherwise updates. Returns the id. Name must be non-empty
    /// (`StoreError::Invalid`). Fax number is stored normalised.
    pub fn upsert_contact(&self, contact: &Contact) -> StoreResult<ContactId> {
        todo!()
    }
    pub fn delete_contact(&self, id: ContactId) -> StoreResult<()> {
        todo!()
    }
    /// Exact match on normalised fax number.
    pub fn contact_by_number(&self, number: &str) -> StoreResult<Option<Contact>> {
        todo!()
    }

    // ── sent log ──
    pub fn insert_sent(
        &self,
        file: &Path,
        printer: &str,
        recipient: &str,
        note: &str,
    ) -> StoreResult<i64> {
        todo!()
    }
    /// Newest first.
    pub fn list_sent(&self, limit: u32) -> StoreResult<Vec<SentFax>> {
        todo!()
    }

    // ── activity log ──
    pub fn append_log(&self, level: LogLevel, message: &str) -> StoreResult<()> {
        todo!()
    }
    /// Newest first.
    pub fn recent_logs(&self, limit: u32) -> StoreResult<Vec<LogEntry>> {
        todo!()
    }
    pub fn clear_logs(&self) -> StoreResult<()> {
        todo!()
    }
}
