//! Sent-fax log and activity log. Both list newest first by insertion id.

use super::StoreResult;
use super::codec::{format_ts, parse_ts, path_text, query_vec};
use crate::model::{LogEntry, LogLevel, SentFax};
use chrono::Local;
use rusqlite::types::Value;
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};

pub(super) fn insert_sent(
    conn: &Connection,
    file: &Path,
    printer: &str,
    recipient: &str,
    note: &str,
) -> StoreResult<i64> {
    let file_path = path_text(file)?;
    conn.execute(
        "INSERT INTO sent_faxes (sent_at, file_path, printer, recipient, note) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            format_ts(&Local::now()),
            file_path,
            printer,
            recipient,
            note
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub(super) fn list_sent(conn: &Connection, limit: u32) -> StoreResult<Vec<SentFax>> {
    query_vec(
        conn,
        "SELECT id, sent_at, file_path, printer, recipient, note FROM sent_faxes \
         ORDER BY id DESC LIMIT ?1",
        &[Value::Integer(i64::from(limit))],
        |row| {
            let sent_at: String = row.get("sent_at")?;
            let file_path: String = row.get("file_path")?;
            Ok(SentFax {
                id: row.get("id")?,
                at: parse_ts(&sent_at)?,
                file_path: PathBuf::from(file_path),
                printer: row.get("printer")?,
                recipient: row.get("recipient")?,
                note: row.get("note")?,
            })
        },
    )
}

pub(super) fn append_log(conn: &Connection, level: LogLevel, message: &str) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO activity_log (at, level, message) VALUES (?1, ?2, ?3)",
        params![format_ts(&Local::now()), level.as_db(), message],
    )?;
    Ok(())
}

pub(super) fn recent_logs(conn: &Connection, limit: u32) -> StoreResult<Vec<LogEntry>> {
    query_vec(
        conn,
        "SELECT id, at, level, message FROM activity_log ORDER BY id DESC LIMIT ?1",
        &[Value::Integer(i64::from(limit))],
        |row| {
            let at: String = row.get("at")?;
            let level: String = row.get("level")?;
            Ok(LogEntry {
                id: row.get("id")?,
                at: parse_ts(&at)?,
                level: LogLevel::from_db(&level),
                message: row.get("message")?,
            })
        },
    )
}

pub(super) fn clear_logs(conn: &Connection) -> StoreResult<()> {
    conn.execute("DELETE FROM activity_log", [])?;
    Ok(())
}
