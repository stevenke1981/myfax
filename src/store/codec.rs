//! Row and value conversions shared by the store submodules.

use super::{StoreError, StoreResult};
use crate::model::{FaxRecord, FaxSource};
use chrono::{DateTime, Local, NaiveDate};
use rusqlite::types::Value;
use rusqlite::{Connection, Row, params_from_iter};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Column list for `faxes` queries; must stay in step with [`read_record`].
pub(super) const FAX_COLUMNS: &str = "id, received_at, sender_number, sender_name, pages, \
     file_path, file_size, sha256, original_name, source, is_read, is_starred, note, \
     deleted_at, imported_at";

/// Upper bound on bound parameters per `IN (...)` list (well under SQLite's limit).
const ID_CHUNK: usize = 500;

/// Stored form of a timestamp: RFC 3339 with the local offset.
pub(super) fn format_ts(at: &DateTime<Local>) -> String {
    at.to_rfc3339()
}

pub(super) fn parse_ts(text: &str) -> StoreResult<DateTime<Local>> {
    DateTime::parse_from_rfc3339(text)
        .map(|at| at.with_timezone(&Local))
        .map_err(|err| StoreError::Invalid(format!("timestamp {text:?}: {err}")))
}

/// Stored form of a calendar date: `YYYY-MM-DD`.
pub(super) fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub(super) fn parse_date(text: &str) -> StoreResult<NaiveDate> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .map_err(|err| StoreError::Invalid(format!("date {text:?}: {err}")))
}

/// Paths are stored as text; non-UTF-8 paths are rejected rather than mangled.
pub(super) fn path_text(path: &Path) -> StoreResult<&str> {
    path.to_str()
        .ok_or_else(|| StoreError::Invalid(format!("path is not valid UTF-8: {}", path.display())))
}

/// Trimmed text, or `None` when it is empty or whitespace only.
pub(super) fn non_blank(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// Escape `\`, `%` and `_` so they match literally under `LIKE ... ESCAPE '\'`.
pub(super) fn escape_like(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if matches!(ch, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// `%text%` pattern with the user text escaped.
pub(super) fn contains_pattern(text: &str) -> String {
    format!("%{}%", escape_like(text))
}

/// Checked conversion of an unsigned value for storage in an INTEGER column.
pub(super) fn to_db_int(value: u64, what: &str) -> StoreResult<i64> {
    i64::try_from(value)
        .map_err(|_| StoreError::Invalid(format!("{what} too large to store: {value}")))
}

/// Checked conversion of a stored INTEGER into a narrower Rust type.
pub(super) fn from_db<T: TryFrom<i64>>(value: i64, what: &str) -> StoreResult<T> {
    T::try_from(value).map_err(|_| StoreError::Invalid(format!("{what} out of range: {value}")))
}

/// Run `sql` and map every row with `map`.
pub(super) fn query_vec<T>(
    conn: &Connection,
    sql: &str,
    params: &[Value],
    mut map: impl FnMut(&Row<'_>) -> StoreResult<T>,
) -> StoreResult<Vec<T>> {
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query(params_from_iter(params.iter()))?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(map(row)?);
    }
    Ok(out)
}

/// Decode one `faxes` row selected with [`FAX_COLUMNS`]. Tags are left empty.
pub(super) fn read_record(row: &Row<'_>) -> StoreResult<FaxRecord> {
    let received_at: String = row.get("received_at")?;
    let file_path: String = row.get("file_path")?;
    let source: String = row.get("source")?;
    let deleted_at: Option<String> = row.get("deleted_at")?;
    let imported_at: String = row.get("imported_at")?;
    let pages: i64 = row.get("pages")?;
    let file_size: i64 = row.get("file_size")?;
    Ok(FaxRecord {
        id: row.get("id")?,
        received_at: parse_ts(&received_at)?,
        sender_number: row.get("sender_number")?,
        sender_name: row.get("sender_name")?,
        pages: from_db(pages, "pages")?,
        file_path: PathBuf::from(file_path),
        file_size: from_db(file_size, "file_size")?,
        sha256: row.get("sha256")?,
        original_name: row.get("original_name")?,
        source: FaxSource::from_db(&source),
        is_read: row.get("is_read")?,
        is_starred: row.get("is_starred")?,
        tags: Vec::new(),
        note: row.get("note")?,
        deleted_at: deleted_at.as_deref().map(parse_ts).transpose()?,
        imported_at: parse_ts(&imported_at)?,
    })
}

/// Run a `faxes` query (selecting [`FAX_COLUMNS`]) without loading tags.
pub(super) fn fetch_records(
    conn: &Connection,
    sql: &str,
    params: &[Value],
) -> StoreResult<Vec<FaxRecord>> {
    query_vec(conn, sql, params, read_record)
}

/// Run a `faxes` query and fill in each record's tags.
pub(super) fn fetch_with_tags(
    conn: &Connection,
    sql: &str,
    params: &[Value],
) -> StoreResult<Vec<FaxRecord>> {
    let records = fetch_records(conn, sql, params)?;
    attach_tags(conn, records)
}

/// Fill `tags` on every record with one query per chunk of ids.
pub(super) fn attach_tags(
    conn: &Connection,
    mut records: Vec<FaxRecord>,
) -> StoreResult<Vec<FaxRecord>> {
    let ids: Vec<i64> = records.iter().map(|record| record.id).collect();
    let mut tags = load_tags(conn, &ids)?;
    for record in &mut records {
        record.tags = tags.remove(&record.id).unwrap_or_default();
    }
    Ok(records)
}

fn load_tags(conn: &Connection, ids: &[i64]) -> StoreResult<HashMap<i64, Vec<String>>> {
    let mut by_fax: HashMap<i64, Vec<String>> = HashMap::new();
    for chunk in ids.chunks(ID_CHUNK) {
        let placeholders = vec!["?"; chunk.len()].join(", ");
        let sql = format!(
            "SELECT fax_id, tag FROM fax_tags WHERE fax_id IN ({placeholders}) \
             ORDER BY fax_id, tag"
        );
        let pairs = query_vec(conn, &sql, &id_params(chunk), |row| {
            let fax_id: i64 = row.get(0)?;
            let tag: String = row.get(1)?;
            Ok((fax_id, tag))
        })?;
        for (fax_id, tag) in pairs {
            by_fax.entry(fax_id).or_default().push(tag);
        }
    }
    Ok(by_fax)
}

fn id_params(ids: &[i64]) -> Vec<Value> {
    ids.iter().copied().map(Value::Integer).collect()
}

/// Fetch one fax with its tags, if it exists.
pub(super) fn fetch_one(conn: &Connection, id: i64) -> StoreResult<Option<FaxRecord>> {
    let sql = format!("SELECT {FAX_COLUMNS} FROM faxes WHERE id = ?1");
    Ok(fetch_with_tags(conn, &sql, &[Value::Integer(id)])?
        .into_iter()
        .next())
}
