//! SQLite index of archived faxes, contacts, sent log and activity log.
//! Owner: store worker. Public API = orchestrator contract.
//!
//! One `Store` per thread (the watcher thread opens its own on the same file).
//! The database uses WAL mode and `busy_timeout` so two connections coexist.

mod codec;
mod contacts;
mod faxes;
mod logs;
mod query;
mod schema;
mod stats;

#[cfg(test)]
mod tests_faxes;
#[cfg(test)]
mod tests_misc;
#[cfg(test)]
mod tests_trash;

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
    conn: rusqlite::Connection,
}

impl Store {
    /// Open (creating parent dirs and schema / running migrations).
    pub fn open(path: &Path) -> StoreResult<Self> {
        Ok(Self {
            conn: schema::open(path)?,
        })
    }
    pub fn open_in_memory() -> StoreResult<Self> {
        Ok(Self {
            conn: schema::open_in_memory()?,
        })
    }

    // ── faxes ──
    pub fn insert_fax(&self, new: &NewFax) -> StoreResult<FaxId> {
        faxes::insert_fax(&self.conn, new)
    }
    /// Any fax (including trashed) with this content hash.
    pub fn find_by_hash(&self, sha256: &str) -> StoreResult<Option<FaxId>> {
        faxes::find_by_hash(&self.conn, sha256)
    }
    pub fn get_fax(&self, id: FaxId) -> StoreResult<Option<FaxRecord>> {
        faxes::get_fax(&self.conn, id)
    }
    pub fn search(&self, query: &FaxQuery) -> StoreResult<SearchPage> {
        query::search(&self.conn, query)
    }
    /// Every match of `query` ignoring paging (for export).
    pub fn search_all(&self, query: &FaxQuery) -> StoreResult<Vec<FaxRecord>> {
        query::search_all(&self.conn, query)
    }
    pub fn set_read(&self, id: FaxId, read: bool) -> StoreResult<()> {
        faxes::set_read(&self.conn, id, read)
    }
    /// Marks every non-deleted fax read; returns rows changed.
    pub fn mark_all_read(&self) -> StoreResult<u64> {
        faxes::mark_all_read(&self.conn)
    }
    pub fn set_starred(&self, id: FaxId, starred: bool) -> StoreResult<()> {
        faxes::set_starred(&self.conn, id, starred)
    }
    /// Replace tags (normalised with `model::normalize_tags`).
    pub fn set_tags(&self, id: FaxId, tags: &[String]) -> StoreResult<()> {
        faxes::set_tags(&self.conn, id, tags)
    }
    pub fn set_note(&self, id: FaxId, note: &str) -> StoreResult<()> {
        faxes::set_note(&self.conn, id, note)
    }
    /// Empty / whitespace name stores NULL.
    pub fn set_sender_name(&self, id: FaxId, name: Option<&str>) -> StoreResult<()> {
        faxes::set_sender_name(&self.conn, id, name)
    }
    pub fn set_file_path(&self, id: FaxId, path: &Path) -> StoreResult<()> {
        faxes::set_file_path(&self.conn, id, path)
    }
    pub fn move_to_trash(&self, id: FaxId, at: DateTime<Local>) -> StoreResult<()> {
        faxes::move_to_trash(&self.conn, id, at)
    }
    pub fn restore(&self, id: FaxId) -> StoreResult<()> {
        faxes::restore(&self.conn, id)
    }
    /// Deletes the row and returns the record (caller deletes the file).
    pub fn delete_permanently(&self, id: FaxId) -> StoreResult<FaxRecord> {
        faxes::delete_permanently(&self.conn, id)
    }
    /// Trashed records with `deleted_at < cutoff` (caller deletes files + rows).
    pub fn trashed_before(&self, cutoff: DateTime<Local>) -> StoreResult<Vec<FaxRecord>> {
        faxes::trashed_before(&self.conn, cutoff)
    }
    /// All trashed records.
    pub fn trashed(&self) -> StoreResult<Vec<FaxRecord>> {
        faxes::trashed(&self.conn)
    }
    /// Per-day counts of non-deleted faxes, ascending by date.
    pub fn date_buckets(&self) -> StoreResult<Vec<DateBucket>> {
        stats::date_buckets(&self.conn)
    }
    pub fn folder_counts(&self) -> StoreResult<FolderCounts> {
        stats::folder_counts(&self.conn)
    }
    /// (tag, count) over non-deleted faxes, count desc then tag asc.
    pub fn tag_counts(&self) -> StoreResult<Vec<(String, u64)>> {
        stats::tag_counts(&self.conn)
    }
    /// Inclusive local-date range.
    pub fn stats(&self, from: NaiveDate, to: NaiveDate) -> StoreResult<Stats> {
        stats::stats(&self.conn, from, to)
    }
    /// Set `sender_name` on every fax whose `sender_number` equals `number`
    /// and whose name is NULL; returns rows changed.
    pub fn fill_sender_names(&self, number: &str, name: &str) -> StoreResult<u64> {
        faxes::fill_sender_names(&self.conn, number, name)
    }

    // ── contacts ──
    /// Case-insensitive substring filter over name/company/number/email; "" = all.
    /// Sorted by name (case-insensitive).
    pub fn list_contacts(&self, filter: &str) -> StoreResult<Vec<Contact>> {
        contacts::list_contacts(&self.conn, filter)
    }
    pub fn get_contact(&self, id: ContactId) -> StoreResult<Option<Contact>> {
        contacts::get_contact(&self.conn, id)
    }
    /// `id == 0` inserts, otherwise updates. Returns the id. Name must be non-empty
    /// (`StoreError::Invalid`). Fax number is stored normalised.
    pub fn upsert_contact(&self, contact: &Contact) -> StoreResult<ContactId> {
        contacts::upsert_contact(&self.conn, contact)
    }
    pub fn delete_contact(&self, id: ContactId) -> StoreResult<()> {
        contacts::delete_contact(&self.conn, id)
    }
    /// Exact match on normalised fax number.
    pub fn contact_by_number(&self, number: &str) -> StoreResult<Option<Contact>> {
        contacts::contact_by_number(&self.conn, number)
    }

    // ── sent log ──
    pub fn insert_sent(
        &self,
        file: &Path,
        printer: &str,
        recipient: &str,
        note: &str,
    ) -> StoreResult<i64> {
        logs::insert_sent(&self.conn, file, printer, recipient, note)
    }
    /// Newest first.
    pub fn list_sent(&self, limit: u32) -> StoreResult<Vec<SentFax>> {
        logs::list_sent(&self.conn, limit)
    }

    // ── activity log ──
    pub fn append_log(&self, level: LogLevel, message: &str) -> StoreResult<()> {
        logs::append_log(&self.conn, level, message)
    }
    /// Newest first.
    pub fn recent_logs(&self, limit: u32) -> StoreResult<Vec<LogEntry>> {
        logs::recent_logs(&self.conn, limit)
    }
    pub fn clear_logs(&self) -> StoreResult<()> {
        logs::clear_logs(&self.conn)
    }
}
