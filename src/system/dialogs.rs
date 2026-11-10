//! Native file and folder dialogs via `rfd` (blocking, modal).

use std::path::{Path, PathBuf};

use rfd::FileDialog;

pub fn pick_folder(start: Option<&Path>) -> Option<PathBuf> {
    with_start_dir(FileDialog::new(), start).pick_folder()
}

pub fn pick_file(start: Option<&Path>, extensions: &[&str]) -> Option<PathBuf> {
    with_filter(with_start_dir(FileDialog::new(), start), extensions).pick_file()
}

pub fn save_file(start: Option<&Path>, default_name: &str, extensions: &[&str]) -> Option<PathBuf> {
    let mut dialog = with_filter(with_start_dir(FileDialog::new(), start), extensions);
    if !default_name.is_empty() {
        dialog = dialog.set_file_name(default_name);
    }
    dialog.save_file()
}

fn with_start_dir(dialog: FileDialog, start: Option<&Path>) -> FileDialog {
    match start.and_then(usable_start_dir) {
        Some(dir) => dialog.set_directory(dir),
        None => dialog,
    }
}

/// A usable starting folder: `start` itself when it is a directory, its parent when
/// it is a file, otherwise `None` (the dialog then uses its own default).
pub(super) fn usable_start_dir(start: &Path) -> Option<PathBuf> {
    if start.is_dir() {
        return Some(start.to_path_buf());
    }
    start
        .parent()
        .filter(|parent| parent.is_dir())
        .map(Path::to_path_buf)
}

fn with_filter(dialog: FileDialog, extensions: &[&str]) -> FileDialog {
    let exts = normalize_extensions(extensions);
    if exts.is_empty() {
        dialog
    } else {
        dialog.add_filter(filter_label(&exts), &exts)
    }
}

/// Drop leading dots and empty entries: `".pdf"` becomes `"pdf"`.
pub(super) fn normalize_extensions<'a>(extensions: &[&'a str]) -> Vec<&'a str> {
    extensions
        .iter()
        .map(|ext| ext.trim().trim_start_matches('.'))
        .filter(|ext| !ext.is_empty())
        .collect()
}

/// Filter label built from the extension list itself (no translatable text).
pub(super) fn filter_label(exts: &[&str]) -> String {
    exts.iter()
        .map(|ext| ext.to_uppercase())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_dir_is_kept_when_it_is_a_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(usable_start_dir(dir.path()), Some(dir.path().to_path_buf()));
    }

    #[test]
    fn start_dir_falls_back_to_parent_for_a_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.pdf");
        std::fs::write(&file, b"x").expect("write");
        assert_eq!(usable_start_dir(&file), Some(dir.path().to_path_buf()));
    }

    #[test]
    fn start_dir_is_none_when_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(usable_start_dir(&dir.path().join("nope").join("x")), None);
    }

    #[test]
    fn extensions_are_normalized_and_labelled_without_translatable_text() {
        let exts = normalize_extensions(&[".pdf", "", "tif", " tiff "]);
        assert_eq!(exts, vec!["pdf", "tif", "tiff"]);
        assert_eq!(filter_label(&exts), "PDF, TIF, TIFF");
    }
}
