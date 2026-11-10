//! OS integration: open / print / reveal, printers, native pickers, file copy.
//! Owner: system worker. Public API = orchestrator contract.
//! Windows is the primary target; other OSes compile with best-effort fallbacks.

use std::path::{Path, PathBuf};

/// Open with the default application.
#[allow(unused_variables)]
pub fn open_path(path: &Path) -> anyhow::Result<()> {
    todo!()
}

/// Open Explorer with the file selected (folders: open the folder).
#[allow(unused_variables)]
pub fn reveal_in_folder(path: &Path) -> anyhow::Result<()> {
    todo!()
}

/// Print with the default printer through the shell "print" verb (non-blocking).
#[allow(unused_variables)]
pub fn print_file(path: &Path) -> anyhow::Result<()> {
    todo!()
}

/// Print to a named printer through the shell "printto" verb — used to hand a
/// document to the Epson FAX driver, which then asks for the recipient.
#[allow(unused_variables)]
pub fn print_to(path: &Path, printer: &str) -> anyhow::Result<()> {
    todo!()
}

/// Installed printer names, sorted.
pub fn list_printers() -> anyhow::Result<Vec<String>> {
    todo!()
}

/// Printers whose name contains "fax" (case-insensitive).
pub fn fax_printers(all: &[String]) -> Vec<String> {
    all.iter()
        .filter(|p| p.to_lowercase().contains("fax"))
        .cloned()
        .collect()
}

/// Native folder picker (blocking, modal). `None` when cancelled or unavailable.
#[allow(unused_variables)]
pub fn pick_folder(start: Option<&Path>) -> Option<PathBuf> {
    todo!()
}

/// Native open-file picker filtered by extensions (e.g. `["pdf","tif","tiff"]`).
#[allow(unused_variables)]
pub fn pick_file(start: Option<&Path>, extensions: &[&str]) -> Option<PathBuf> {
    todo!()
}

/// Native save-file picker.
#[allow(unused_variables)]
pub fn save_file(start: Option<&Path>, default_name: &str, extensions: &[&str]) -> Option<PathBuf> {
    todo!()
}

/// Copy `src` into `dest_dir` keeping its file name; append ` (2)`, ` (3)`… before
/// the extension on collision. Creates `dest_dir`. Returns the written path.
#[allow(unused_variables)]
pub fn copy_into_dir(src: &Path, dest_dir: &Path) -> std::io::Result<PathBuf> {
    todo!()
}
