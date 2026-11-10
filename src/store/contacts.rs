//! Address-book contacts.

use super::codec::{contains_pattern, query_vec};
use super::{StoreError, StoreResult};
use crate::model::{Contact, ContactId, normalize_number};
use rusqlite::types::Value;
use rusqlite::{Connection, Row, params};

const CONTACT_COLUMNS: &str = "id, name, company, fax_number, phone, email, note";

fn read_contact(row: &Row<'_>) -> StoreResult<Contact> {
    Ok(Contact {
        id: row.get("id")?,
        name: row.get("name")?,
        company: row.get("company")?,
        fax_number: row.get("fax_number")?,
        phone: row.get("phone")?,
        email: row.get("email")?,
        note: row.get("note")?,
    })
}

pub(super) fn list_contacts(conn: &Connection, filter: &str) -> StoreResult<Vec<Contact>> {
    let needle = filter.trim();
    if needle.is_empty() {
        let sql =
            format!("SELECT {CONTACT_COLUMNS} FROM contacts ORDER BY name COLLATE NOCASE, id");
        return query_vec(conn, &sql, &[], read_contact);
    }
    // `?1` is referenced four times, so one bound pattern covers every column.
    let sql = format!(
        "SELECT {CONTACT_COLUMNS} FROM contacts \
         WHERE name LIKE ?1 ESCAPE '\\' OR company LIKE ?1 ESCAPE '\\' \
         OR fax_number LIKE ?1 ESCAPE '\\' OR email LIKE ?1 ESCAPE '\\' \
         ORDER BY name COLLATE NOCASE, id"
    );
    query_vec(
        conn,
        &sql,
        &[Value::Text(contains_pattern(needle))],
        read_contact,
    )
}

pub(super) fn get_contact(conn: &Connection, id: ContactId) -> StoreResult<Option<Contact>> {
    let sql = format!("SELECT {CONTACT_COLUMNS} FROM contacts WHERE id = ?1");
    Ok(query_vec(conn, &sql, &[Value::Integer(id)], read_contact)?
        .into_iter()
        .next())
}

/// Trims text fields, requires a name and normalises the fax number.
fn cleaned(contact: &Contact) -> StoreResult<Contact> {
    let name = contact.name.trim();
    if name.is_empty() {
        return Err(StoreError::Invalid("name".to_string()));
    }
    Ok(Contact {
        id: contact.id,
        name: name.to_string(),
        company: contact.company.trim().to_string(),
        fax_number: normalize_number(&contact.fax_number).unwrap_or_default(),
        phone: contact.phone.trim().to_string(),
        email: contact.email.trim().to_string(),
        note: contact.note.trim().to_string(),
    })
}

pub(super) fn upsert_contact(conn: &Connection, contact: &Contact) -> StoreResult<ContactId> {
    let row = cleaned(contact)?;
    if row.id == 0 {
        conn.execute(
            "INSERT INTO contacts (name, company, fax_number, phone, email, note) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                row.name,
                row.company,
                row.fax_number,
                row.phone,
                row.email,
                row.note
            ],
        )?;
        return Ok(conn.last_insert_rowid());
    }
    let changed = conn.execute(
        "UPDATE contacts SET name = ?2, company = ?3, fax_number = ?4, phone = ?5, \
         email = ?6, note = ?7 WHERE id = ?1",
        params![
            row.id,
            row.name,
            row.company,
            row.fax_number,
            row.phone,
            row.email,
            row.note
        ],
    )?;
    if changed == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(row.id)
}

pub(super) fn delete_contact(conn: &Connection, id: ContactId) -> StoreResult<()> {
    let changed = conn.execute("DELETE FROM contacts WHERE id = ?1", [id])?;
    if changed == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

/// Lowest-id contact whose normalised fax number equals `number`'s normalised form.
pub(super) fn contact_by_number(conn: &Connection, number: &str) -> StoreResult<Option<Contact>> {
    let Some(normalised) = normalize_number(number) else {
        return Ok(None);
    };
    let sql =
        format!("SELECT {CONTACT_COLUMNS} FROM contacts WHERE fax_number = ?1 ORDER BY id LIMIT 1");
    Ok(
        query_vec(conn, &sql, &[Value::Text(normalised)], read_contact)?
            .into_iter()
            .next(),
    )
}
