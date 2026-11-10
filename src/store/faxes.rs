//! Fax rows: insert, lookup, single-row edits and the trash lifecycle.

use super::codec::{
    FAX_COLUMNS, fetch_one, fetch_with_tags, format_date, format_ts, non_blank, path_text,
    to_db_int,
};
use super::{StoreError, StoreResult};
use crate::model::{FaxId, FaxRecord, NewFax, normalize_tags};
use chrono::{DateTime, Local};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

pub(super) fn insert_fax(conn: &Connection, new: &NewFax) -> StoreResult<FaxId> {
    let received_at = format_ts(&new.received_at);
    let received_date = format_date(new.received_at.date_naive());
    let file_path = path_text(&new.file_path)?;
    let file_size = to_db_int(new.file_size, "file_size")?;
    let sender_name = new.sender_name.as_deref().and_then(non_blank);
    conn.execute(
        "INSERT INTO faxes (received_at, received_ts, received_date, sender_number, sender_name, pages, \
         file_path, file_size, sha256, original_name, source, imported_at) \
         VALUES (?1, ?12, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            received_at,
            received_date,
            new.sender_number,
            sender_name,
            i64::from(new.pages),
            file_path,
            file_size,
            new.sha256,
            new.original_name,
            new.source.as_db(),
            format_ts(&Local::now()),
            new.received_at.timestamp(),
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub(super) fn find_by_hash(conn: &Connection, sha256: &str) -> StoreResult<Option<FaxId>> {
    let found = conn
        .query_row("SELECT id FROM faxes WHERE sha256 = ?1", [sha256], |row| {
            row.get(0)
        })
        .optional()?;
    Ok(found)
}

pub(super) fn get_fax(conn: &Connection, id: FaxId) -> StoreResult<Option<FaxRecord>> {
    fetch_one(conn, id)
}

pub(super) fn set_read(conn: &Connection, id: FaxId, read: bool) -> StoreResult<()> {
    let changed = conn.execute(
        "UPDATE faxes SET is_read = ?2 WHERE id = ?1",
        params![id, read],
    )?;
    expect_changed(changed)
}

pub(super) fn mark_all_read(conn: &Connection) -> StoreResult<u64> {
    let changed = conn.execute(
        "UPDATE faxes SET is_read = 1 WHERE deleted_at IS NULL AND is_read = 0",
        [],
    )?;
    Ok(changed as u64)
}

pub(super) fn set_starred(conn: &Connection, id: FaxId, starred: bool) -> StoreResult<()> {
    let changed = conn.execute(
        "UPDATE faxes SET is_starred = ?2 WHERE id = ?1",
        params![id, starred],
    )?;
    expect_changed(changed)
}

/// Replaces the tag set atomically; unknown ids fail with `NotFound` before any change.
pub(super) fn set_tags(conn: &Connection, id: FaxId, tags: &[String]) -> StoreResult<()> {
    let tags = normalize_tags(tags);
    let tx = conn.unchecked_transaction()?;
    ensure_exists(&tx, id)?;
    tx.execute("DELETE FROM fax_tags WHERE fax_id = ?1", [id])?;
    for tag in &tags {
        tx.execute(
            "INSERT INTO fax_tags (fax_id, tag) VALUES (?1, ?2)",
            params![id, tag],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub(super) fn set_note(conn: &Connection, id: FaxId, note: &str) -> StoreResult<()> {
    let changed = conn.execute(
        "UPDATE faxes SET note = ?2 WHERE id = ?1",
        params![id, note],
    )?;
    expect_changed(changed)
}

pub(super) fn set_sender_name(conn: &Connection, id: FaxId, name: Option<&str>) -> StoreResult<()> {
    let name = name.and_then(non_blank);
    let changed = conn.execute(
        "UPDATE faxes SET sender_name = ?2 WHERE id = ?1",
        params![id, name],
    )?;
    expect_changed(changed)
}

pub(super) fn set_file_path(conn: &Connection, id: FaxId, path: &Path) -> StoreResult<()> {
    let file_path = path_text(path)?;
    let changed = conn.execute(
        "UPDATE faxes SET file_path = ?2 WHERE id = ?1",
        params![id, file_path],
    )?;
    expect_changed(changed)
}

/// Keeps the original deletion time when the fax is already in the trash.
pub(super) fn move_to_trash(conn: &Connection, id: FaxId, at: DateTime<Local>) -> StoreResult<()> {
    let changed = conn.execute(
        "UPDATE faxes SET deleted_at = COALESCE(deleted_at, ?2) WHERE id = ?1",
        params![id, format_ts(&at)],
    )?;
    expect_changed(changed)
}

pub(super) fn restore(conn: &Connection, id: FaxId) -> StoreResult<()> {
    let changed = conn.execute("UPDATE faxes SET deleted_at = NULL WHERE id = ?1", [id])?;
    expect_changed(changed)
}

/// Deletes the row inside a transaction; tags go with it through `ON DELETE CASCADE`.
pub(super) fn delete_permanently(conn: &Connection, id: FaxId) -> StoreResult<FaxRecord> {
    let tx = conn.unchecked_transaction()?;
    let record = fetch_one(&tx, id)?.ok_or(StoreError::NotFound)?;
    tx.execute("DELETE FROM faxes WHERE id = ?1", [id])?;
    tx.commit()?;
    Ok(record)
}

pub(super) fn trashed_before(
    conn: &Connection,
    cutoff: DateTime<Local>,
) -> StoreResult<Vec<FaxRecord>> {
    // Compared on parsed instants: stored RFC 3339 text does not sort by time
    // across different UTC offsets.
    Ok(trashed(conn)?
        .into_iter()
        .filter(|record| record.deleted_at.is_some_and(|at| at < cutoff))
        .collect())
}

pub(super) fn trashed(conn: &Connection) -> StoreResult<Vec<FaxRecord>> {
    let sql = format!(
        "SELECT {FAX_COLUMNS} FROM faxes WHERE deleted_at IS NOT NULL \
         ORDER BY deleted_at DESC, id DESC"
    );
    fetch_with_tags(conn, &sql, &[])
}

/// Fills `sender_name` on faxes from `number` whose name is still NULL.
pub(super) fn fill_sender_names(conn: &Connection, number: &str, name: &str) -> StoreResult<u64> {
    let Some(name) = non_blank(name) else {
        return Ok(0);
    };
    if number.is_empty() {
        return Ok(0);
    }
    let changed = conn.execute(
        "UPDATE faxes SET sender_name = ?2 WHERE sender_number = ?1 AND sender_name IS NULL",
        params![number, name],
    )?;
    Ok(changed as u64)
}

fn expect_changed(changed: usize) -> StoreResult<()> {
    if changed == 0 {
        Err(StoreError::NotFound)
    } else {
        Ok(())
    }
}

fn ensure_exists(conn: &Connection, id: FaxId) -> StoreResult<()> {
    let found: Option<i64> = conn
        .query_row("SELECT id FROM faxes WHERE id = ?1", [id], |row| row.get(0))
        .optional()?;
    found.map(|_| ()).ok_or(StoreError::NotFound)
}
