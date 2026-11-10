//! Copy/move one fax file into the archive and index it.

use crate::config::Settings;
use crate::model::{FaxId, FaxSource};
use crate::store::Store;
use chrono::{DateTime, Local};
use std::path::{Path, PathBuf};

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
#[allow(unused_variables)]
pub fn is_supported(path: &Path, settings: &Settings) -> bool {
    todo!()
}

/// Archive path: `<root>/YYYY/YYYY-MM/YYYY-MM-DD/YYYYMMDD-HHMMSS_<sender|unknown>.<ext>`
/// (ext lower-case). On collision append `_2`, `_3`, … before the extension.
/// Never returns an existing path.
#[allow(unused_variables)]
pub fn archive_path_for(
    root: &Path,
    received_at: DateTime<Local>,
    sender: Option<&str>,
    ext: &str,
) -> PathBuf {
    todo!()
}

/// Full pipeline: support check → SHA-256 → duplicate check → parse name
/// (fallback: file mtime) → page count (pdf module; 0 if unreadable, still
/// archived) → copy to archive path (create dirs) → verify copied size →
/// insert row (sender name from `contact_by_number`) → delete source if
/// configured → copy to `forward_copy_folder` if set (failure there is logged
/// via `store.append_log`, not fatal). On insert failure the copied file is
/// removed. Also appends an Info log line on success.
#[allow(unused_variables)]
pub fn ingest_file(
    path: &Path,
    source: FaxSource,
    settings: &Settings,
    store: &Store,
) -> anyhow::Result<IngestOutcome> {
    todo!()
}
