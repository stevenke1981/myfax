//! Contacts CSV import/export (owner: store worker).
//! Format: UTF-8, header `name,company,fax,phone,email,note`, RFC 4180 quoting.

use crate::model::Contact;
use std::path::Path;

/// Parse contacts from CSV text. Header row required (case-insensitive column
/// names `name,company,fax,phone,email,note`, any order, extra columns ignored;
/// missing `name` column is an error). Rows without a name are skipped.
/// Fax numbers are normalised (`model::normalize_number`, empty if none). `id` is 0.
#[allow(unused_variables)]
pub fn parse_contacts(text: &str) -> anyhow::Result<Vec<Contact>> {
    todo!()
}

/// Serialise contacts with the standard header; fields quoted when they contain
/// `,` `"` CR or LF. Lines end with `\r\n`.
#[allow(unused_variables)]
pub fn write_contacts(contacts: &[Contact]) -> String {
    todo!()
}

pub fn read_file(path: &Path) -> anyhow::Result<Vec<Contact>> {
    let text = std::fs::read_to_string(path)?;
    parse_contacts(text.trim_start_matches('\u{feff}'))
}

/// Writes with a UTF-8 BOM so Excel opens Chinese text correctly.
pub fn write_file(path: &Path, contacts: &[Contact]) -> anyhow::Result<()> {
    std::fs::write(path, format!("\u{feff}{}", write_contacts(contacts)))?;
    Ok(())
}
