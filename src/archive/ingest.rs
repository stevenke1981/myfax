//! Copy/move one fax file into the archive and index it.

use crate::archive::filename;
use crate::config::Settings;
use crate::i18n::{Lang, t, tf};
use crate::model::{FaxId, FaxSource, LogLevel, NewFax};
use crate::store::Store;
use anyhow::{Context, bail};
use chrono::{DateTime, Local};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};

/// Streaming read buffer for hashing.
const HASH_BUFFER_BYTES: usize = 64 * 1024;
/// File-name component used when no sender digits are available.
const UNKNOWN_SENDER_FILE: &str = "unknown";

#[derive(Clone, Debug, PartialEq)]
pub enum IngestOutcome {
    Imported {
        id: FaxId,
        archived: PathBuf,
    },
    /// Same SHA-256 already indexed. The source is removed only when
    /// `delete_source_after_import` is set; otherwise left untouched.
    Duplicate {
        existing: FaxId,
    },
    /// Not a supported fax file (extension / `accept_tiff`) — ignored.
    Skipped,
}

/// Supported extensions (case-insensitive): pdf always; tif/tiff when `accept_tiff`.
pub fn is_supported(path: &Path, settings: &Settings) -> bool {
    match extension_lower(path).as_deref() {
        Some("pdf") => true,
        Some("tif" | "tiff") => settings.accept_tiff,
        _ => false,
    }
}

/// Archive path: `<root>/YYYY/YYYY-MM/YYYY-MM-DD/YYYYMMDD-HHMMSS_<sender|unknown>.<ext>`
/// (ext lower-case). On collision append `_2`, `_3`, … before the extension.
/// Never returns an existing path.
pub fn archive_path_for(
    root: &Path,
    received_at: DateTime<Local>,
    sender: Option<&str>,
    ext: &str,
) -> PathBuf {
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    let day_dir = root
        .join(received_at.format("%Y").to_string())
        .join(received_at.format("%Y-%m").to_string())
        .join(received_at.format("%Y-%m-%d").to_string());
    let stem = format!(
        "{}-{}_{}",
        received_at.format("%Y%m%d"),
        received_at.format("%H%M%S"),
        sanitize_sender(sender)
    );
    let mut candidate = day_dir.join(file_name(&stem, None, &ext));
    let mut counter = 2u64;
    // An I/O error while checking counts as occupied, so no existing file is overwritten.
    while candidate.try_exists().unwrap_or(true) {
        candidate = day_dir.join(file_name(&stem, Some(counter), &ext));
        counter += 1;
    }
    candidate
}

/// Full pipeline: support check → SHA-256 → duplicate check → parse name
/// (fallback: file mtime) → page count (pdf module; 0 if unreadable, still
/// archived) → copy to archive path (create dirs) → verify copied size →
/// insert row (sender name from `contact_by_number`) → delete source if
/// configured → copy to `forward_copy_folder` if set (failure there is logged
/// via `store.append_log`, not fatal). On insert failure the copied file is
/// removed. Also appends an Info log line on success.
pub fn ingest_file(
    path: &Path,
    source: FaxSource,
    settings: &Settings,
    store: &Store,
) -> anyhow::Result<IngestOutcome> {
    if !is_supported(path, settings) {
        return Ok(IngestOutcome::Skipped);
    }
    let lang = settings.language;
    let (sha256, size) = sha256_file(path).with_context(|| {
        tf(
            lang,
            "archive.err_read",
            &[("path", display(path).as_str())],
        )
    })?;
    if let Some(existing) = store.find_by_hash(&sha256)? {
        remove_source_if_configured(path, settings, store);
        return Ok(IngestOutcome::Duplicate { existing });
    }

    let name = file_name_text(path);
    let parsed = filename::parse(&name, modified_time(path));
    let pages = page_count_or_zero(path, &name, settings, store);
    let sender_name = match parsed.sender_number.as_deref() {
        Some(number) => store.contact_by_number(number)?.map(|contact| contact.name),
        None => None,
    };
    let dest = archive_path_for(
        &settings.archive_root,
        parsed.received_at,
        parsed.sender_number.as_deref(),
        &extension_lower(path).unwrap_or_default(),
    );
    copy_into_archive(path, &dest, size, lang)?;

    let new = NewFax {
        received_at: parsed.received_at,
        sender_number: parsed.sender_number.clone(),
        sender_name: sender_name.clone(),
        pages,
        file_path: dest.clone(),
        file_size: size,
        sha256: sha256.clone(),
        original_name: name.clone(),
        source,
    };
    let id = match store.insert_fax(&new) {
        Ok(id) => id,
        Err(err) => {
            remove_quietly(&dest);
            // A concurrent import of the same content wins the UNIQUE(sha256) race.
            if let Ok(Some(existing)) = store.find_by_hash(&sha256) {
                remove_source_if_configured(path, settings, store);
                return Ok(IngestOutcome::Duplicate { existing });
            }
            return Err(anyhow::Error::from(err).context(tf(
                lang,
                "archive.err_index",
                &[("path", display(&dest).as_str())],
            )));
        }
    };

    if let Some(folder) = &settings.forward_copy_folder {
        forward_copy(&dest, folder, settings, store, &name);
    }
    remove_source_if_configured(path, settings, store);
    let sender_label = sender_label(
        lang,
        sender_name.as_deref(),
        parsed.sender_number.as_deref(),
    );
    log_line(
        store,
        LogLevel::Info,
        &tf(
            lang,
            "archive.log_imported",
            &[("sender", sender_label.as_str()), ("name", name.as_str())],
        ),
    );
    Ok(IngestOutcome::Imported { id, archived: dest })
}

/// SHA-256 (lower-case hex) and byte count of `path`, streamed in 64 KiB chunks.
pub(crate) fn sha256_file(path: &Path) -> io::Result<(String, u64)> {
    let file = File::open(path)?;
    let mut reader = BufReader::with_capacity(HASH_BUFFER_BYTES, file);
    let mut hasher = Sha256::new();
    let size = io::copy(&mut reader, &mut hasher)?;
    Ok((format!("{:x}", hasher.finalize()), size))
}

fn sanitize_sender(sender: Option<&str>) -> String {
    let cleaned: String = sender
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect();
    if cleaned.chars().any(|c| c.is_ascii_digit()) {
        cleaned
    } else {
        UNKNOWN_SENDER_FILE.to_owned()
    }
}

fn file_name(stem: &str, suffix: Option<u64>, ext: &str) -> String {
    let mut name = stem.to_owned();
    if let Some(number) = suffix {
        name.push_str(&format!("_{number}"));
    }
    if !ext.is_empty() {
        name.push('.');
        name.push_str(ext);
    }
    name
}

fn extension_lower(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}

fn file_name_text(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn display(path: &Path) -> String {
    path.display().to_string()
}

/// File modification time, or now when the platform does not report one.
fn modified_time(path: &Path) -> DateTime<Local> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .map(DateTime::<Local>::from)
        .unwrap_or_else(|_| Local::now())
}

fn page_count_or_zero(path: &Path, name: &str, settings: &Settings, store: &Store) -> u32 {
    match crate::pdf::page_count(path) {
        Ok(pages) => pages,
        Err(err) => {
            tracing::debug!(error = %err, "page count unavailable");
            log_line(
                store,
                LogLevel::Warn,
                &tf(
                    settings.language,
                    "archive.log_pages_unknown",
                    &[("name", name)],
                ),
            );
            0
        }
    }
}

/// Copy `src` to `dest` (creating parent folders), then verify the byte count.
/// On any failure the partial destination is removed.
fn copy_into_archive(src: &Path, dest: &Path, size: u64, lang: Lang) -> anyhow::Result<()> {
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).with_context(|| {
            tf(
                lang,
                "archive.err_mkdir",
                &[("path", display(dir).as_str())],
            )
        })?;
    }
    let copied = match std::fs::copy(src, dest) {
        Ok(copied) => copied,
        Err(err) => {
            remove_quietly(dest);
            return Err(anyhow::Error::from(err).context(tf(
                lang,
                "archive.err_copy",
                &[("path", display(dest).as_str())],
            )));
        }
    };
    let written = std::fs::metadata(dest).map(|meta| meta.len());
    match written {
        Ok(len) if copied == size && len == size => Ok(()),
        _ => {
            remove_quietly(dest);
            bail!(
                "{}",
                tf(
                    lang,
                    "archive.err_size",
                    &[("path", display(dest).as_str())]
                )
            )
        }
    }
}

fn forward_copy(archived: &Path, folder: &Path, settings: &Settings, store: &Store, name: &str) {
    if let Err(err) = crate::system::copy_into_dir(archived, folder) {
        log_line(
            store,
            LogLevel::Warn,
            &tf(
                settings.language,
                "archive.log_forward_failed",
                &[("name", name), ("error", err.to_string().as_str())],
            ),
        );
    }
}

fn remove_source_if_configured(path: &Path, settings: &Settings, store: &Store) {
    if !settings.delete_source_after_import {
        return;
    }
    if let Err(err) = std::fs::remove_file(path) {
        log_line(
            store,
            LogLevel::Warn,
            &tf(
                settings.language,
                "archive.log_delete_failed",
                &[
                    ("name", file_name_text(path).as_str()),
                    ("error", err.to_string().as_str()),
                ],
            ),
        );
    }
}

fn remove_quietly(path: &Path) {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => {
            tracing::warn!(path = %path.display(), error = %err, "cannot remove partial archive file");
        }
    }
}

/// Activity-log write; a failure here must not undo a successful import.
fn log_line(store: &Store, level: LogLevel, message: &str) {
    if let Err(err) = store.append_log(level, message) {
        tracing::warn!(error = %err, "cannot write activity log");
    }
}

fn sender_label(lang: Lang, name: Option<&str>, number: Option<&str>) -> String {
    name.or(number)
        .map(str::to_owned)
        .unwrap_or_else(|| t(lang, "archive.unknown_sender").to_owned())
}

/// Test fixtures shared by archive tests.
#[cfg(test)]
pub(crate) mod testutil {
    use lopdf::{Dictionary, Document, Object, Stream, dictionary};

    /// Minimal valid PDF with `pages` empty pages.
    pub(crate) fn pdf_bytes(pages: u32) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let mut kids: Vec<Object> = Vec::new();
        for _ in 0..pages {
            let content_id = doc.add_object(Stream::new(Dictionary::new(), Vec::new()));
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
                "Contents" => content_id,
            });
            kids.push(page_id.into());
        }
        let pages_dict = dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => i64::from(pages),
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages_dict));
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);
        let mut out = Vec::new();
        doc.save_to(&mut out).expect("serialise test pdf");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::pdf_bytes;
    use super::*;
    use crate::model::Contact;
    use chrono::TimeZone;

    fn settings_in(dir: &Path) -> Settings {
        Settings {
            archive_root: dir.join("archive"),
            ..Settings::default()
        }
    }

    fn local(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(y, mo, d, h, mi, s)
            .single()
            .expect("test time exists")
    }

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, bytes).expect("write test file");
        path
    }

    fn sha_hex(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn archived_path(dir: &Path, date: &str, file: &str) -> PathBuf {
        let year = &date[..4];
        let month = &date[..7];
        dir.join("archive")
            .join(year)
            .join(month)
            .join(date)
            .join(file)
    }

    #[test]
    fn supported_extensions_follow_accept_tiff() {
        let dir = tempfile::tempdir().unwrap();
        let on = Settings {
            accept_tiff: true,
            ..settings_in(dir.path())
        };
        let off = Settings {
            accept_tiff: false,
            ..settings_in(dir.path())
        };
        assert!(is_supported(Path::new("a.PDF"), &off));
        assert!(is_supported(Path::new("b.pdf"), &on));
        assert!(is_supported(Path::new("c.tif"), &on));
        assert!(is_supported(Path::new("d.TIFF"), &on));
        assert!(!is_supported(Path::new("c.tif"), &off));
        assert!(!is_supported(Path::new("d.tiff"), &off));
        assert!(!is_supported(Path::new("e.txt"), &on));
        assert!(!is_supported(Path::new("noext"), &on));
    }

    #[test]
    fn archive_path_has_date_layout_and_lowercase_extension() {
        let root = Path::new("R");
        let at = local(2026, 11, 10, 10, 30, 45);
        let path = archive_path_for(root, at, Some("0912345678"), "PDF");
        assert_eq!(
            path,
            root.join("2026")
                .join("2026-11")
                .join("2026-11-10")
                .join("20261110-103045_0912345678.pdf")
        );
    }

    #[test]
    fn archive_path_sanitises_sender_and_defaults_to_unknown() {
        let root = Path::new("R");
        let at = local(2026, 11, 10, 10, 30, 45);
        let spaced = archive_path_for(root, at, Some("+886 2 2345 (x)"), "pdf");
        assert!(spaced.ends_with("20261110-103045_+88622345.pdf"));
        let none = archive_path_for(root, at, None, "pdf");
        assert!(none.ends_with("20261110-103045_unknown.pdf"));
        let letters = archive_path_for(root, at, Some("abc"), "pdf");
        assert!(letters.ends_with("20261110-103045_unknown.pdf"));
    }

    #[test]
    fn archive_path_adds_numeric_suffix_on_collision() {
        let dir = tempfile::tempdir().unwrap();
        let at = local(2026, 11, 10, 10, 30, 45);
        let day = dir.path().join("2026").join("2026-11").join("2026-11-10");
        std::fs::create_dir_all(&day).unwrap();
        let first = archive_path_for(dir.path(), at, None, "pdf");
        std::fs::write(&first, b"x").unwrap();
        let second = archive_path_for(dir.path(), at, None, "pdf");
        assert!(second.ends_with("20261110-103045_unknown_2.pdf"));
        std::fs::write(&second, b"x").unwrap();
        let third = archive_path_for(dir.path(), at, None, "pdf");
        assert!(third.ends_with("20261110-103045_unknown_3.pdf"));
        assert_ne!(first, third);
    }

    #[test]
    fn imports_pdf_into_date_folder_with_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let settings = settings_in(dir.path());
        let store = Store::open_in_memory().unwrap();
        let bytes = pdf_bytes(2);
        let src = write(dir.path(), "FAX_20261110_103045_0912345678.pdf", &bytes);

        let outcome = ingest_file(&src, FaxSource::WatchFolder, &settings, &store).unwrap();
        let IngestOutcome::Imported { id, archived } = outcome else {
            panic!("expected Imported, got {outcome:?}");
        };
        let expected = archived_path(dir.path(), "2026-11-10", "20261110-103045_0912345678.pdf");
        assert_eq!(archived, expected);
        assert_eq!(std::fs::read(&archived).unwrap(), bytes);

        let record = store.get_fax(id).unwrap().expect("row exists");
        assert_eq!(record.pages, 2);
        assert_eq!(record.sha256, sha_hex(&bytes));
        assert_eq!(record.file_size, bytes.len() as u64);
        assert_eq!(record.sender_number.as_deref(), Some("0912345678"));
        assert_eq!(record.sender_name, None);
        assert_eq!(record.original_name, "FAX_20261110_103045_0912345678.pdf");
        assert_eq!(record.received_at, local(2026, 11, 10, 10, 30, 45));
        assert_eq!(record.source, FaxSource::WatchFolder);
        assert!(src.exists(), "source kept when delete flag is off");
    }

    #[test]
    fn sender_name_comes_from_contact_book() {
        let dir = tempfile::tempdir().unwrap();
        let settings = settings_in(dir.path());
        let store = Store::open_in_memory().unwrap();
        store
            .upsert_contact(&Contact {
                name: "王小明".into(),
                fax_number: "02-2345-6789".into(),
                ..Contact::default()
            })
            .unwrap();
        let src = write(dir.path(), "RX_202611101030_0223456789.pdf", &pdf_bytes(1));
        let outcome = ingest_file(&src, FaxSource::Manual, &settings, &store).unwrap();
        let IngestOutcome::Imported { id, .. } = outcome else {
            panic!("expected Imported");
        };
        let record = store.get_fax(id).unwrap().unwrap();
        assert_eq!(record.sender_name.as_deref(), Some("王小明"));
    }

    #[test]
    fn duplicate_content_is_detected_and_not_copied_twice() {
        let dir = tempfile::tempdir().unwrap();
        let settings = settings_in(dir.path());
        let store = Store::open_in_memory().unwrap();
        let bytes = pdf_bytes(1);
        let first = write(dir.path(), "FAX_20261110_103045_0912345678.pdf", &bytes);
        let second = write(dir.path(), "copy_of_fax.pdf", &bytes);
        let IngestOutcome::Imported { id, .. } =
            ingest_file(&first, FaxSource::WatchFolder, &settings, &store).unwrap()
        else {
            panic!("expected Imported");
        };

        let outcome = ingest_file(&second, FaxSource::WatchFolder, &settings, &store).unwrap();
        assert_eq!(outcome, IngestOutcome::Duplicate { existing: id });
        assert!(
            second.exists(),
            "duplicate source kept when delete flag is off"
        );
        let day = dir.path().join("archive/2026/2026-11/2026-11-10");
        assert_eq!(std::fs::read_dir(day).unwrap().count(), 1);
    }

    #[test]
    fn delete_source_on_removes_source_after_import_and_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings {
            delete_source_after_import: true,
            ..settings_in(dir.path())
        };
        let store = Store::open_in_memory().unwrap();
        let bytes = pdf_bytes(1);
        let first = write(dir.path(), "FAX_20261110_103045_0912345678.pdf", &bytes);
        let second = write(dir.path(), "again.pdf", &bytes);
        let outcome = ingest_file(&first, FaxSource::WatchFolder, &settings, &store).unwrap();
        assert!(matches!(outcome, IngestOutcome::Imported { .. }));
        assert!(!first.exists());
        let outcome = ingest_file(&second, FaxSource::WatchFolder, &settings, &store).unwrap();
        assert!(matches!(outcome, IngestOutcome::Duplicate { .. }));
        assert!(!second.exists());
    }

    #[test]
    fn forward_copy_is_written_to_folder() {
        let dir = tempfile::tempdir().unwrap();
        let forward = dir.path().join("forward");
        let settings = Settings {
            forward_copy_folder: Some(forward.clone()),
            ..settings_in(dir.path())
        };
        let store = Store::open_in_memory().unwrap();
        let src = write(
            dir.path(),
            "FAX_20261110_103045_0912345678.pdf",
            &pdf_bytes(1),
        );
        ingest_file(&src, FaxSource::WatchFolder, &settings, &store).unwrap();
        let copied = forward.join("20261110-103045_0912345678.pdf");
        assert!(copied.exists(), "forward copy missing");
        let logs = store.recent_logs(10).unwrap();
        assert!(logs.iter().any(|log| log.level == LogLevel::Info));
    }

    #[test]
    fn forward_failure_is_logged_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = write(dir.path(), "blocker", b"not a folder");
        let settings = Settings {
            forward_copy_folder: Some(blocker.join("sub")),
            ..settings_in(dir.path())
        };
        let store = Store::open_in_memory().unwrap();
        let src = write(
            dir.path(),
            "FAX_20261110_103045_0912345678.pdf",
            &pdf_bytes(1),
        );
        let outcome = ingest_file(&src, FaxSource::WatchFolder, &settings, &store).unwrap();
        assert!(matches!(outcome, IngestOutcome::Imported { .. }));
        let logs = store.recent_logs(10).unwrap();
        assert!(logs.iter().any(|log| log.level == LogLevel::Warn));
    }

    #[test]
    fn corrupt_pdf_is_archived_with_zero_pages() {
        let dir = tempfile::tempdir().unwrap();
        let settings = settings_in(dir.path());
        let store = Store::open_in_memory().unwrap();
        let src = write(
            dir.path(),
            "broken_fax.pdf",
            b"%PDF-1.4 garbage, not really",
        );
        let outcome = ingest_file(&src, FaxSource::WatchFolder, &settings, &store).unwrap();
        let IngestOutcome::Imported { id, archived } = outcome else {
            panic!("expected Imported");
        };
        assert!(archived.exists());
        assert_eq!(store.get_fax(id).unwrap().unwrap().pages, 0);
    }

    #[test]
    fn unsupported_extension_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let settings = settings_in(dir.path());
        let store = Store::open_in_memory().unwrap();
        let src = write(dir.path(), "notes.txt", b"hello");
        let outcome = ingest_file(&src, FaxSource::Manual, &settings, &store).unwrap();
        assert_eq!(outcome, IngestOutcome::Skipped);
        assert!(src.exists());
        assert!(!dir.path().join("archive").exists());
    }

    #[test]
    fn unreadable_source_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let settings = settings_in(dir.path());
        let store = Store::open_in_memory().unwrap();
        let missing = dir.path().join("gone.pdf");
        assert!(ingest_file(&missing, FaxSource::Manual, &settings, &store).is_err());
    }

    #[test]
    fn chinese_file_name_is_preserved_in_original_name() {
        let dir = tempfile::tempdir().unwrap();
        let settings = settings_in(dir.path());
        let store = Store::open_in_memory().unwrap();
        let src = write(
            dir.path(),
            "傳真_20261110_103045_0223456789.pdf",
            &pdf_bytes(1),
        );
        let IngestOutcome::Imported { id, .. } =
            ingest_file(&src, FaxSource::WatchFolder, &settings, &store).unwrap()
        else {
            panic!("expected Imported");
        };
        let record = store.get_fax(id).unwrap().unwrap();
        assert_eq!(record.original_name, "傳真_20261110_103045_0223456789.pdf");
    }

    #[test]
    fn sha256_file_matches_in_memory_digest() {
        let dir = tempfile::tempdir().unwrap();
        let bytes: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let path = write(dir.path(), "big.bin", &bytes);
        let (hex, size) = sha256_file(&path).unwrap();
        assert_eq!(hex, sha_hex(&bytes));
        assert_eq!(size, bytes.len() as u64);
    }

    #[test]
    fn tiff_source_is_archived_with_tif_extension() {
        let dir = tempfile::tempdir().unwrap();
        let settings = settings_in(dir.path());
        let store = Store::open_in_memory().unwrap();
        let src = write(
            dir.path(),
            "FAX_20261110_103045_0912345678.TIFF",
            b"II*\0junk",
        );
        let IngestOutcome::Imported { archived, .. } =
            ingest_file(&src, FaxSource::WatchFolder, &settings, &store).unwrap()
        else {
            panic!("expected Imported");
        };
        assert!(archived.ends_with("20261110-103045_0912345678.tiff"));
    }
}
