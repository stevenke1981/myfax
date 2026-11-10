//! Connection setup and schema migrations (`PRAGMA user_version`).

use super::{StoreError, StoreResult};
use rusqlite::Connection;
use std::path::Path;
use std::time::Duration;

/// How long a connection waits for another writer (the watcher thread) before failing.
const BUSY_TIMEOUT: Duration = Duration::from_millis(5000);

/// Ordered migrations: `MIGRATIONS[i]` upgrades the schema from version `i` to `i + 1`.
/// Never edit a shipped entry; append a new one instead.
const MIGRATIONS: &[&str] = &[SCHEMA_V1];

const SCHEMA_V1: &str = "
CREATE TABLE faxes (
    id              INTEGER PRIMARY KEY,
    received_at     TEXT NOT NULL,
    received_date   TEXT NOT NULL,
    sender_number   TEXT,
    sender_name     TEXT,
    pages           INTEGER NOT NULL,
    file_path       TEXT NOT NULL,
    file_size       INTEGER NOT NULL,
    sha256          TEXT NOT NULL UNIQUE,
    original_name   TEXT NOT NULL,
    source          TEXT NOT NULL,
    is_read         INTEGER NOT NULL DEFAULT 0,
    is_starred      INTEGER NOT NULL DEFAULT 0,
    note            TEXT NOT NULL DEFAULT '',
    deleted_at      TEXT,
    imported_at     TEXT NOT NULL
);
CREATE TABLE fax_tags (
    fax_id  INTEGER NOT NULL REFERENCES faxes(id) ON DELETE CASCADE,
    tag     TEXT NOT NULL,
    PRIMARY KEY (fax_id, tag)
);
CREATE TABLE contacts (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    company     TEXT NOT NULL DEFAULT '',
    fax_number  TEXT NOT NULL DEFAULT '',
    phone       TEXT NOT NULL DEFAULT '',
    email       TEXT NOT NULL DEFAULT '',
    note        TEXT NOT NULL DEFAULT ''
);
CREATE TABLE sent_faxes (
    id          INTEGER PRIMARY KEY,
    sent_at     TEXT NOT NULL,
    file_path   TEXT NOT NULL,
    printer     TEXT NOT NULL,
    recipient   TEXT NOT NULL,
    note        TEXT NOT NULL
);
CREATE TABLE activity_log (
    id          INTEGER PRIMARY KEY,
    at          TEXT NOT NULL,
    level       TEXT NOT NULL,
    message     TEXT NOT NULL
);
CREATE INDEX idx_faxes_received_date ON faxes(received_date);
CREATE INDEX idx_faxes_sha256 ON faxes(sha256);
CREATE INDEX idx_faxes_sender_number ON faxes(sender_number);
CREATE INDEX idx_faxes_deleted_at ON faxes(deleted_at);
CREATE INDEX idx_fax_tags_tag ON fax_tags(tag);
";

/// Open (creating parent directories) a file-backed database in WAL mode.
pub(super) fn open(path: &Path) -> StoreResult<Connection> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut conn = Connection::open(path)?;
    configure(&conn)?;
    // `journal_mode` answers with a row (the new mode); the value is not needed.
    let _mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    migrate(&mut conn)?;
    Ok(conn)
}

/// Open a throw-away database with the same schema (tests and previews).
pub(super) fn open_in_memory() -> StoreResult<Connection> {
    let mut conn = Connection::open_in_memory()?;
    configure(&conn)?;
    migrate(&mut conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> StoreResult<()> {
    conn.busy_timeout(BUSY_TIMEOUT)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    Ok(())
}

fn read_user_version(conn: &Connection) -> StoreResult<usize> {
    let raw: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    usize::try_from(raw).map_err(|_| StoreError::Invalid(format!("negative schema version {raw}")))
}

fn migrate(conn: &mut Connection) -> StoreResult<()> {
    let current = read_user_version(conn)?;
    if current > MIGRATIONS.len() {
        return Err(StoreError::Invalid(format!(
            "schema version {current} is newer than supported {}",
            MIGRATIONS.len()
        )));
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let version = index + 1;
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }
    Ok(())
}
