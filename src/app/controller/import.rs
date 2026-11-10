//! Importing fax files (typed path, drag and drop) and exporting the filtered list.

use super::Controller;
use crate::app::msg::Field;
use crate::app::state::ToastKind;
use crate::archive::ingest::{IngestOutcome, ingest_file};
use crate::i18n::{t, tf};
use crate::model::{FaxId, FaxSource, LogLevel};
use crate::system::copy_into_dir;
use std::path::{Path, PathBuf};

/// Counts gathered while importing one batch of paths.
#[derive(Debug, Default)]
struct ImportSummary {
    imported: u64,
    duplicates: u64,
    skipped: u64,
    failed: u64,
    last_imported: Option<FaxId>,
}

/// File name of a path for messages (full path when there is no name).
pub(super) fn file_label(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Regular files directly inside `dir`, sorted by path.
fn top_level_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    files.sort();
    Ok(files)
}

impl Controller {
    /// Import the file or folder typed in the import field.
    pub(super) fn import_from_field(&mut self) {
        let text = self.state.field(Field::ImportPath).trim().to_owned();
        if text.is_empty() {
            let message = t(self.state.lang, "app.import_no_path").to_owned();
            self.report_error(message);
            return;
        }
        self.import_paths(vec![PathBuf::from(text)]);
    }

    /// Import files (folders contribute their top-level files), summarise, refresh,
    /// and select the fax when exactly one was imported.
    pub(super) fn import_paths(&mut self, paths: Vec<PathBuf>) {
        let mut summary = ImportSummary::default();
        let files = self.collect_files(paths, &mut summary);
        for file in &files {
            self.import_one(file, &mut summary);
        }
        self.announce_import(&summary);
        self.refresh_all();
        if let (1, Some(id)) = (summary.imported, summary.last_imported) {
            self.select_fax(id);
        }
    }

    fn collect_files(&mut self, paths: Vec<PathBuf>, summary: &mut ImportSummary) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for path in paths {
            if path.is_dir() {
                match top_level_files(&path) {
                    Ok(found) => files.extend(found),
                    Err(error) => {
                        summary.failed += 1;
                        self.log_import_error(&path, &error.to_string());
                    }
                }
            } else if path.is_file() {
                files.push(path);
            } else {
                summary.failed += 1;
                let shown = path.display().to_string();
                let reason = tf(self.state.lang, "app.file_missing", &[("path", &shown)]);
                self.log_import_error(&path, &reason);
            }
        }
        files
    }

    fn import_one(&mut self, file: &Path, summary: &mut ImportSummary) {
        let lang = self.state.lang;
        match ingest_file(file, FaxSource::Manual, &self.state.settings, &self.store) {
            Ok(IngestOutcome::Imported { id, .. }) => {
                summary.imported += 1;
                summary.last_imported = Some(id);
            }
            Ok(IngestOutcome::Duplicate { .. }) => {
                summary.duplicates += 1;
                let name = file_label(file);
                self.log(
                    LogLevel::Info,
                    &tf(lang, "app.log_import_duplicate", &[("name", &name)]),
                );
            }
            Ok(IngestOutcome::Skipped) => summary.skipped += 1,
            Err(error) => {
                summary.failed += 1;
                self.log_import_error(file, &format!("{error:#}"));
            }
        }
    }

    fn log_import_error(&self, path: &Path, reason: &str) {
        let name = file_label(path);
        let text = tf(
            self.state.lang,
            "app.log_import_error",
            &[("name", &name), ("error", reason)],
        );
        self.log(LogLevel::Error, &text);
    }

    fn announce_import(&mut self, summary: &ImportSummary) {
        let lang = self.state.lang;
        let imported = summary.imported.to_string();
        let duplicates = summary.duplicates.to_string();
        let skipped = summary.skipped.to_string();
        let failed = summary.failed.to_string();
        let text = tf(
            lang,
            "app.import_summary",
            &[
                ("imported", &imported),
                ("duplicates", &duplicates),
                ("skipped", &skipped),
                ("failed", &failed),
            ],
        );
        let kind = if summary.failed > 0 {
            ToastKind::Error
        } else if summary.imported > 0 {
            ToastKind::Success
        } else {
            ToastKind::Info
        };
        self.toast(kind, text);
    }

    /// Copy every fax matching the current filter (all pages) into the export folder.
    pub(super) fn export_filtered(&mut self) {
        let lang = self.state.lang;
        let folder_text = self.state.field(Field::ExportFolder).trim().to_owned();
        if folder_text.is_empty() {
            let message = t(lang, "app.export_no_folder").to_owned();
            self.report_error(message);
            return;
        }
        let folder = PathBuf::from(folder_text);
        let Some(records) = self.or_report(self.store.search_all(&self.state.query)) else {
            return;
        };
        if records.is_empty() {
            self.toast(ToastKind::Info, t(lang, "app.export_none").to_owned());
            return;
        }
        if let Err(error) = std::fs::create_dir_all(&folder) {
            let text = tf(lang, "app.export_error", &[("error", &error.to_string())]);
            self.report_error(text);
            return;
        }
        let mut copied = 0u64;
        let mut failure: Option<String> = None;
        for record in &records {
            match copy_into_dir(&record.file_path, &folder) {
                Ok(_) => copied += 1,
                Err(error) => failure = Some(error.to_string()),
            }
        }
        if let Some(error) = failure {
            let text = tf(lang, "app.export_error", &[("error", &error)]);
            self.report_error(text);
            return;
        }
        let count = copied.to_string();
        let shown = folder.display().to_string();
        self.log(
            LogLevel::Info,
            &tf(
                lang,
                "app.log_exported",
                &[("count", &count), ("folder", &shown)],
            ),
        );
        self.toast(
            ToastKind::Success,
            tf(
                lang,
                "app.export_done",
                &[("count", &count), ("folder", &shown)],
            ),
        );
    }
}
