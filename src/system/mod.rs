//! OS integration: open / print / reveal, printers, native pickers, file copy.
//! Owner: system worker. Public API = orchestrator contract.
//! Windows is the primary target; other OSes compile with best-effort fallbacks.

mod command;
mod dialogs;
mod files;
mod launch;
mod printers;
mod ps;

use std::io;
use std::path::{Path, PathBuf};

/// Open with the default application.
pub fn open_path(path: &Path) -> anyhow::Result<()> {
    launch::open(path)
}

/// Open Explorer with the file selected (folders: open the folder).
pub fn reveal_in_folder(path: &Path) -> anyhow::Result<()> {
    launch::reveal(path)
}

/// Print with the default printer through the shell "print" verb (non-blocking).
pub fn print_file(path: &Path) -> anyhow::Result<()> {
    launch::print(path)
}

/// Print to a named printer through the shell "printto" verb — used to hand a
/// document to the Epson FAX driver, which then asks for the recipient.
pub fn print_to(path: &Path, printer: &str) -> anyhow::Result<()> {
    launch::print_to(path, printer)
}

/// Installed printer names, sorted.
pub fn list_printers() -> anyhow::Result<Vec<String>> {
    printers::installed()
}

/// Printers whose name contains "fax" (case-insensitive).
pub fn fax_printers(all: &[String]) -> Vec<String> {
    all.iter()
        .filter(|p| p.to_lowercase().contains("fax"))
        .cloned()
        .collect()
}

/// Native folder picker (blocking, modal). `None` when cancelled or unavailable.
pub fn pick_folder(start: Option<&Path>) -> Option<PathBuf> {
    dialogs::pick_folder(start)
}

/// Native open-file picker filtered by extensions (e.g. `["pdf","tif","tiff"]`).
pub fn pick_file(start: Option<&Path>, extensions: &[&str]) -> Option<PathBuf> {
    dialogs::pick_file(start, extensions)
}

/// Native save-file picker.
pub fn save_file(start: Option<&Path>, default_name: &str, extensions: &[&str]) -> Option<PathBuf> {
    dialogs::save_file(start, default_name, extensions)
}

/// Copy `src` into `dest_dir` keeping its file name; append ` (2)`, ` (3)`… before
/// the extension on collision. Creates `dest_dir`. Returns the written path.
pub fn copy_into_dir(src: &Path, dest_dir: &Path) -> io::Result<PathBuf> {
    files::copy_into_dir(src, dest_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fax_printers_keeps_only_names_containing_fax_case_insensitively() {
        let all = vec![
            "HP LaserJet".to_owned(),
            "EPSON FAX Driver".to_owned(),
            "Canon fax-2".to_owned(),
            "Microsoft Print to PDF".to_owned(),
        ];
        assert_eq!(
            fax_printers(&all),
            vec!["EPSON FAX Driver".to_owned(), "Canon fax-2".to_owned()]
        );
    }

    #[test]
    fn fax_printers_of_empty_list_is_empty() {
        assert!(fax_printers(&[]).is_empty());
    }
}
