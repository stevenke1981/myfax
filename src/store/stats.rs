//! Aggregates for the inbox badges, date navigation and the statistics page.
//! All of them ignore trashed faxes except the trash count.

use super::StoreResult;
use super::codec::{format_date, from_db, parse_date, query_vec};
use crate::model::{DateBucket, FolderCounts, Stats};
use chrono::NaiveDate;
use rusqlite::Connection;
use rusqlite::types::Value;

/// Maximum number of senders reported in [`Stats::top_senders`].
const TOP_SENDERS: usize = 10;

const NON_DELETED: &str = "deleted_at IS NULL";

/// Per-day counts for rows matching `where_sql`, ascending by date.
fn daily_counts(
    conn: &Connection,
    where_sql: &str,
    params: &[Value],
) -> StoreResult<Vec<DateBucket>> {
    let sql = format!(
        "SELECT received_date, COUNT(*) FROM faxes WHERE {where_sql} \
         GROUP BY received_date ORDER BY received_date ASC"
    );
    query_vec(conn, &sql, params, |row| {
        let date: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        Ok(DateBucket {
            date: parse_date(&date)?,
            count: from_db(count, "daily count")?,
        })
    })
}

pub(super) fn date_buckets(conn: &Connection) -> StoreResult<Vec<DateBucket>> {
    daily_counts(conn, NON_DELETED, &[])
}

pub(super) fn folder_counts(conn: &Connection) -> StoreResult<FolderCounts> {
    let sql = "SELECT \
         COUNT(CASE WHEN deleted_at IS NULL THEN 1 END), \
         COUNT(CASE WHEN deleted_at IS NULL AND is_read = 0 THEN 1 END), \
         COUNT(CASE WHEN deleted_at IS NULL AND is_starred = 1 THEN 1 END), \
         COUNT(CASE WHEN deleted_at IS NOT NULL THEN 1 END) \
         FROM faxes";
    let (all, unread, starred, trash): (i64, i64, i64, i64) = conn.query_row(sql, [], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    })?;
    Ok(FolderCounts {
        all: from_db(all, "count")?,
        unread: from_db(unread, "count")?,
        starred: from_db(starred, "count")?,
        trash: from_db(trash, "count")?,
    })
}

pub(super) fn tag_counts(conn: &Connection) -> StoreResult<Vec<(String, u64)>> {
    let sql = "SELECT t.tag, COUNT(*) FROM fax_tags t \
         JOIN faxes f ON f.id = t.fax_id \
         WHERE f.deleted_at IS NULL \
         GROUP BY t.tag ORDER BY COUNT(*) DESC, t.tag ASC";
    query_vec(conn, sql, &[], |row| {
        let tag: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        Ok((tag, from_db(count, "tag count")?))
    })
}

/// Totals, per-day counts and top senders for the inclusive range `from..=to`.
pub(super) fn stats(conn: &Connection, from: NaiveDate, to: NaiveDate) -> StoreResult<Stats> {
    let range = "deleted_at IS NULL AND received_date BETWEEN ?1 AND ?2";
    let params = [Value::Text(format_date(from)), Value::Text(format_date(to))];

    let totals_sql = format!("SELECT COUNT(*), COALESCE(SUM(pages), 0) FROM faxes WHERE {range}");
    let (total_faxes, total_pages): (i64, i64) = conn.query_row(
        &totals_sql,
        rusqlite::params_from_iter(params.iter()),
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    // Display sender: trimmed name when present, otherwise the number.
    let senders_sql = format!(
        "SELECT who, COUNT(*) AS n FROM (\
           SELECT NULLIF(COALESCE(NULLIF(TRIM(sender_name), ''), sender_number), '') AS who \
           FROM faxes WHERE {range}\
         ) WHERE who IS NOT NULL \
         GROUP BY who ORDER BY n DESC, who ASC LIMIT {TOP_SENDERS}"
    );
    let top_senders = query_vec(conn, &senders_sql, &params, |row| {
        let who: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        Ok((who, from_db(count, "sender count")?))
    })?;

    Ok(Stats {
        total_faxes: from_db(total_faxes, "fax count")?,
        total_pages: from_db(total_pages, "page count")?,
        per_day: daily_counts(conn, range, &params)?,
        top_senders,
    })
}
