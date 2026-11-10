//! Filtered, sorted and paged search over `faxes`. Every user value is bound as
//! a parameter; only fixed SQL fragments are ever formatted into the statement.

use super::StoreResult;
use super::codec::{FAX_COLUMNS, contains_pattern, fetch_with_tags, format_date, from_db};
use crate::model::{FaxQuery, FaxRecord, Folder, SearchPage, SortOrder, normalize_tags};
use rusqlite::Connection;
use rusqlite::params_from_iter;
use rusqlite::types::Value;

/// Page size used when the query asks for 0 items per page.
const DEFAULT_PAGE_SIZE: u32 = 50;

/// Number of `?` placeholders in [`WORD_CLAUSE`].
const WORD_PARAMS: usize = 5;

/// One search word must match at least one of these fields (or a tag).
const WORD_CLAUSE: &str = concat!(
    r"(sender_name LIKE ? ESCAPE '\' OR sender_number LIKE ? ESCAPE '\'",
    r" OR note LIKE ? ESCAPE '\' OR original_name LIKE ? ESCAPE '\'",
    r" OR EXISTS (SELECT 1 FROM fax_tags t WHERE t.fax_id = faxes.id AND t.tag LIKE ? ESCAPE '\'))",
);

/// WHERE fragment plus its positional parameters, in placeholder order.
struct Filter {
    clause: String,
    params: Vec<Value>,
}

impl Filter {
    fn new(query: &FaxQuery) -> Self {
        let mut clauses = vec![format!("({})", folder_clause(query.folder))];
        let mut params = Vec::new();

        for word in query.text.split_whitespace() {
            clauses.push(WORD_CLAUSE.to_string());
            let pattern = contains_pattern(word);
            params.extend((0..WORD_PARAMS).map(|_| Value::Text(pattern.clone())));
        }
        if let Some(from) = query.date_from {
            clauses.push("received_date >= ?".to_string());
            params.push(Value::Text(format_date(from)));
        }
        if let Some(to) = query.date_to {
            clauses.push("received_date <= ?".to_string());
            params.push(Value::Text(format_date(to)));
        }
        if let Some(tag) = query.tag.as_deref().and_then(first_tag) {
            clauses.push(
                "EXISTS (SELECT 1 FROM fax_tags t WHERE t.fax_id = faxes.id AND t.tag = ?)"
                    .to_string(),
            );
            params.push(Value::Text(tag));
        }

        Self {
            clause: clauses.join(" AND "),
            params,
        }
    }
}

fn folder_clause(folder: Folder) -> &'static str {
    match folder {
        Folder::All => "deleted_at IS NULL",
        Folder::Unread => "deleted_at IS NULL AND is_read = 0",
        Folder::Starred => "deleted_at IS NULL AND is_starred = 1",
        Folder::Trash => "deleted_at IS NOT NULL",
    }
}

fn order_clause(sort: SortOrder) -> &'static str {
    match sort {
        SortOrder::NewestFirst => "received_at DESC, id DESC",
        SortOrder::OldestFirst => "received_at ASC, id ASC",
        SortOrder::SenderAsc => {
            "COALESCE(NULLIF(sender_name, ''), sender_number, '') COLLATE NOCASE ASC, \
             received_at DESC, id DESC"
        }
    }
}

fn first_tag(raw: &str) -> Option<String> {
    normalize_tags([raw]).into_iter().next()
}

fn count(conn: &Connection, filter: &Filter) -> StoreResult<u64> {
    let sql = format!("SELECT COUNT(*) FROM faxes WHERE {}", filter.clause);
    let total: i64 = conn.query_row(&sql, params_from_iter(filter.params.iter()), |row| {
        row.get(0)
    })?;
    from_db(total, "match count")
}

fn select_sql(filter: &Filter, sort: SortOrder, paged: bool) -> String {
    let limit = if paged { " LIMIT ? OFFSET ?" } else { "" };
    format!(
        "SELECT {FAX_COLUMNS} FROM faxes WHERE {} ORDER BY {}{limit}",
        filter.clause,
        order_clause(sort)
    )
}

/// One page of matches plus the total number of matches.
pub(super) fn search(conn: &Connection, query: &FaxQuery) -> StoreResult<SearchPage> {
    let filter = Filter::new(query);
    let total = count(conn, &filter)?;

    let page_size = match query.page_size {
        0 => DEFAULT_PAGE_SIZE,
        size => size,
    };
    let offset = i64::from(query.page) * i64::from(page_size);
    let sql = select_sql(&filter, query.sort, true);
    let mut params = filter.params;
    params.push(Value::Integer(i64::from(page_size)));
    params.push(Value::Integer(offset));

    let items = fetch_with_tags(conn, &sql, &params)?;
    Ok(SearchPage { items, total })
}

/// Every match of `query`, ignoring paging.
pub(super) fn search_all(conn: &Connection, query: &FaxQuery) -> StoreResult<Vec<FaxRecord>> {
    let filter = Filter::new(query);
    let sql = select_sql(&filter, query.sort, false);
    fetch_with_tags(conn, &sql, &filter.params)
}
