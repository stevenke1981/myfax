//! Contacts CSV import/export (owner: store worker).
//! Format: UTF-8, header `name,company,fax,phone,email,note`, RFC 4180 quoting.

use crate::model::{Contact, normalize_number};
use anyhow::{Context, bail};
use std::borrow::Cow;
use std::path::Path;

/// Header written by [`write_contacts`], in column order.
const HEADER: [&str; 6] = ["name", "company", "fax", "phone", "email", "note"];

/// Parse contacts from CSV text. Header row required (case-insensitive column
/// names `name,company,fax,phone,email,note`, any order, extra columns ignored;
/// missing `name` column is an error). Rows without a name are skipped.
/// Fax numbers are normalised (`model::normalize_number`, empty if none). `id` is 0.
pub fn parse_contacts(text: &str) -> anyhow::Result<Vec<Contact>> {
    let text = text.trim_start_matches('\u{feff}');
    let mut records = parse_records(text)?
        .into_iter()
        .filter(|record| record.iter().any(|field| !field.trim().is_empty()));

    let header = records.next().context("CSV is empty: header row missing")?;
    let columns = Columns::from_header(&header)?;
    Ok(records
        .map(|record| columns.contact(&record))
        .filter(|c| !c.name.is_empty())
        .collect())
}

/// Serialise contacts with the standard header; fields quoted when they contain
/// `,` `"` CR or LF. Lines end with `\r\n`.
pub fn write_contacts(contacts: &[Contact]) -> String {
    let mut out = String::new();
    push_row(&mut out, HEADER.iter().copied());
    for contact in contacts {
        push_row(
            &mut out,
            [
                contact.name.as_str(),
                contact.company.as_str(),
                contact.fax_number.as_str(),
                contact.phone.as_str(),
                contact.email.as_str(),
                contact.note.as_str(),
            ],
        );
    }
    out
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

/// Index of each known column in the header; `None` when the file lacks it.
struct Columns {
    name: usize,
    company: Option<usize>,
    fax: Option<usize>,
    phone: Option<usize>,
    email: Option<usize>,
    note: Option<usize>,
}

impl Columns {
    fn from_header(header: &[String]) -> anyhow::Result<Self> {
        let find = |wanted: &str| {
            header
                .iter()
                .position(|cell| cell.trim().to_lowercase() == wanted)
        };
        let Some(name) = find("name") else {
            bail!("CSV header has no `name` column");
        };
        Ok(Self {
            name,
            company: find("company"),
            fax: find("fax"),
            phone: find("phone"),
            email: find("email"),
            note: find("note"),
        })
    }

    fn contact(&self, record: &[String]) -> Contact {
        let field = |index: Option<usize>| {
            index
                .and_then(|i| record.get(i))
                .map(|value| value.trim().to_string())
                .unwrap_or_default()
        };
        Contact {
            id: 0,
            name: field(Some(self.name)),
            company: field(self.company),
            fax_number: normalize_number(&field(self.fax)).unwrap_or_default(),
            phone: field(self.phone),
            email: field(self.email),
            note: field(self.note),
        }
    }
}

/// Split CSV text into records of fields (RFC 4180). Accepts CRLF or LF line
/// ends; quoted fields may contain commas, line breaks and `""` escapes.
fn parse_records(text: &str) -> anyhow::Result<Vec<Vec<String>>> {
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            match ch {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => in_quotes = false,
                _ => field.push(ch),
            }
            continue;
        }
        match ch {
            '"' if field.is_empty() => in_quotes = true,
            ',' => record.push(std::mem::take(&mut field)),
            '\r' if chars.peek() == Some(&'\n') => {}
            '\n' => {
                record.push(std::mem::take(&mut field));
                records.push(std::mem::take(&mut record));
            }
            _ => field.push(ch),
        }
    }
    if in_quotes {
        bail!("CSV ends inside a quoted field");
    }
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        records.push(record);
    }
    Ok(records)
}

fn push_row<'a>(out: &mut String, fields: impl IntoIterator<Item = &'a str>) {
    let mut first = true;
    for field in fields {
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str(&escape_field(field));
    }
    out.push_str("\r\n");
}

fn escape_field(field: &str) -> Cow<'_, str> {
    if field.contains([',', '"', '\r', '\n']) {
        Cow::Owned(format!("\"{}\"", field.replace('"', "\"\"")))
    } else {
        Cow::Borrowed(field)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contact(name: &str, fax: &str, note: &str) -> Contact {
        Contact {
            id: 0,
            name: name.into(),
            company: String::new(),
            fax_number: fax.into(),
            phone: String::new(),
            email: String::new(),
            note: note.into(),
        }
    }

    #[test]
    fn roundtrip_keeps_quotes_commas_newlines_and_chinese() {
        let original = Contact {
            id: 0,
            name: "陳 \"小美\", 經理".into(),
            company: "甲公司\n第二行".into(),
            fax_number: "+886221234567".into(),
            phone: "02-1234".into(),
            email: "a@example.com".into(),
            note: "含, 逗號 與 \"引號\"".into(),
        };
        let text = write_contacts(std::slice::from_ref(&original));
        let parsed = parse_contacts(&text).expect("roundtrip parses");
        assert_eq!(parsed, vec![original]);
    }

    #[test]
    fn writer_uses_crlf_and_standard_header() {
        let text = write_contacts(&[contact("A", "", "")]);
        assert_eq!(text, "name,company,fax,phone,email,note\r\nA,,,,,\r\n");
    }

    #[test]
    fn accepts_lf_and_crlf_line_endings() {
        let lf = "name,fax\nAlice,02-2345 6789\n";
        let crlf = "name,fax\r\nAlice,02-2345 6789\r\n";
        let a = parse_contacts(lf).expect("lf parses");
        let b = parse_contacts(crlf).expect("crlf parses");
        assert_eq!(a, b);
        assert_eq!(a[0].fax_number, "0223456789");
    }

    #[test]
    fn header_is_case_insensitive_and_order_free() {
        let text = "Note , EMAIL,Extra,NAME,Fax\nhi,x@y.z,zz,Bob,+1 800\n";
        let parsed = parse_contacts(text).expect("parses");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "Bob");
        assert_eq!(parsed[0].fax_number, "+1800");
        assert_eq!(parsed[0].email, "x@y.z");
        assert_eq!(parsed[0].note, "hi");
        assert_eq!(parsed[0].company, "");
    }

    #[test]
    fn missing_name_column_is_error() {
        let err = parse_contacts("company,fax\nAcme,123\n").expect_err("must fail");
        assert!(err.to_string().contains("name"));
    }

    #[test]
    fn blank_lines_and_nameless_rows_are_skipped() {
        let text = "name,company\n\n  \nAlice,Acme\n,NoName\n\r\nBob,\n";
        let names: Vec<String> = parse_contacts(text)
            .expect("parses")
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(names, vec!["Alice", "Bob"]);
    }

    #[test]
    fn unterminated_quote_is_error() {
        assert!(parse_contacts("name\n\"open\n").is_err());
    }

    #[test]
    fn empty_input_is_error() {
        assert!(parse_contacts("").is_err());
    }

    #[test]
    fn short_rows_fill_missing_fields_with_empty() {
        let parsed = parse_contacts("name,company,fax\nAlice\n").expect("parses");
        assert_eq!(parsed[0].name, "Alice");
        assert_eq!(parsed[0].fax_number, "");
    }

    #[test]
    fn file_roundtrip_handles_bom() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("contacts.csv");
        let contacts = vec![contact("王小明", "02-2345-6789", "備註")];
        write_file(&path, &contacts).expect("write");

        let bytes = std::fs::read(&path).expect("read bytes");
        assert_eq!(&bytes[..3], &[0xEF, 0xBB, 0xBF], "BOM written");

        let parsed = read_file(&path).expect("read");
        assert_eq!(parsed[0].name, "王小明");
        assert_eq!(parsed[0].fax_number, "0223456789");
        assert_eq!(parsed[0].note, "備註");
    }

    #[test]
    fn read_file_strips_bom_written_by_other_tools() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("excel.csv");
        std::fs::write(&path, "\u{feff}name,fax\r\nAlice,123\r\n").expect("write");
        let parsed = read_file(&path).expect("read");
        assert_eq!(parsed[0].name, "Alice");
    }
}
