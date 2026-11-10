//! One poll cycle over the watch folders, free of threads so it can be tested
//! directly. The watcher thread owns a `Poller` and calls `poll_once` in a loop.

use crate::archive::ingest::{IngestOutcome, ingest_file, is_supported, sha256_file};
use crate::archive::watcher::WatchEvent;
use crate::config::Settings;
use crate::i18n::tf;
use crate::model::FaxSource;
use crate::store::Store;
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A file is retried at most this many times at the same size/mtime.
const MAX_FAILURES: u32 = 3;
/// Failure counts that produce an `Error` event (first and last retry).
const REPORT_FAILURES: [u32; 2] = [1, MAX_FAILURES];

/// Observed size and modification time of one file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stamp {
    size: u64,
    mtime: Option<SystemTime>,
}

/// Per-file bookkeeping between polls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileState {
    /// Seen once with this stamp; ingested once the next poll shows it unchanged.
    Observed(Stamp),
    /// Ingested or recognised as already indexed; ignored while unchanged.
    Done(Stamp),
    /// Ingestion failed `failures` times at this stamp.
    Failed { stamp: Stamp, failures: u32 },
}

#[derive(Debug, Default)]
pub(super) struct Poller {
    files: HashMap<PathBuf, FileState>,
    /// Folders that could not be listed in the previous poll.
    unavailable: HashSet<PathBuf>,
    /// True after the first poll; the first poll treats files already indexed
    /// by content hash as settled (no events).
    started: bool,
}

impl Poller {
    /// Run one cycle over every watch folder. The returned list always ends with
    /// `ScanCompleted`.
    pub(super) fn poll_once(&mut self, settings: &Settings, store: &Store) -> Vec<WatchEvent> {
        let settings = settings.sanitized();
        let first_pass = !self.started;
        self.started = true;
        let mut events = Vec::new();
        let mut present = HashSet::new();
        for folder in &settings.watch_folders {
            self.scan_folder(
                folder,
                &settings,
                store,
                first_pass,
                &mut present,
                &mut events,
            );
        }
        self.forget_missing(&present);
        events.push(WatchEvent::ScanCompleted);
        events
    }

    fn scan_folder(
        &mut self,
        folder: &Path,
        settings: &Settings,
        store: &Store,
        first_pass: bool,
        present: &mut HashSet<PathBuf>,
        events: &mut Vec<WatchEvent>,
    ) {
        let entries = match std::fs::read_dir(folder) {
            Ok(entries) => entries,
            Err(err) => {
                self.report_unavailable(folder, &err, settings, events);
                return;
            }
        };
        self.unavailable.remove(folder);
        for entry in entries {
            // An entry that cannot be read is transient; the next poll lists it again.
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            if !path.is_file() || !is_supported(&path, settings) {
                continue;
            }
            present.insert(path.clone());
            self.visit_file(&path, settings, store, first_pass, events);
        }
    }

    fn report_unavailable(
        &mut self,
        folder: &Path,
        err: &io::Error,
        settings: &Settings,
        events: &mut Vec<WatchEvent>,
    ) {
        // One event per change of state: only when the folder was not already unavailable.
        if self.unavailable.insert(folder.to_path_buf()) {
            let error = tf(
                settings.language,
                "archive.watch_folder_unavailable",
                &[
                    ("path", folder.display().to_string().as_str()),
                    ("error", err.to_string().as_str()),
                ],
            );
            events.push(WatchEvent::Error {
                path: folder.to_path_buf(),
                error,
            });
        }
    }

    fn visit_file(
        &mut self,
        path: &Path,
        settings: &Settings,
        store: &Store,
        first_pass: bool,
        events: &mut Vec<WatchEvent>,
    ) {
        // A file that vanished between listing and stat is simply not there this poll.
        let Some(stamp) = stamp_of(path) else { return };
        if stamp.size == 0 {
            return;
        }
        let state = self.files.get(path).copied();
        match state {
            Some(FileState::Done(seen)) if seen == stamp => return,
            Some(FileState::Failed {
                stamp: seen,
                failures,
            }) if seen == stamp && failures >= MAX_FAILURES => {
                return;
            }
            Some(FileState::Observed(seen) | FileState::Failed { stamp: seen, .. })
                if seen == stamp => {}
            None if first_pass && already_indexed(path, store) => {
                self.files
                    .insert(path.to_path_buf(), FileState::Done(stamp));
                return;
            }
            _ => {
                self.files
                    .insert(path.to_path_buf(), FileState::Observed(stamp));
                return;
            }
        }
        // Stable across two polls. Still being written by another process if it cannot be opened.
        if std::fs::File::open(path).is_err() {
            return;
        }
        let prior_failures = match state {
            Some(FileState::Failed {
                stamp: seen,
                failures,
            }) if seen == stamp => failures,
            _ => 0,
        };
        self.ingest(path, stamp, prior_failures, settings, store, events);
    }

    fn ingest(
        &mut self,
        path: &Path,
        stamp: Stamp,
        prior_failures: u32,
        settings: &Settings,
        store: &Store,
        events: &mut Vec<WatchEvent>,
    ) {
        match ingest_file(path, FaxSource::WatchFolder, settings, store) {
            Ok(IngestOutcome::Imported { id, .. }) => {
                self.files
                    .insert(path.to_path_buf(), FileState::Done(stamp));
                events.push(WatchEvent::Imported {
                    id,
                    source: path.to_path_buf(),
                });
            }
            Ok(IngestOutcome::Duplicate { .. }) => {
                self.files
                    .insert(path.to_path_buf(), FileState::Done(stamp));
                events.push(WatchEvent::Duplicate {
                    source: path.to_path_buf(),
                });
            }
            Ok(IngestOutcome::Skipped) => {
                self.files
                    .insert(path.to_path_buf(), FileState::Done(stamp));
            }
            Err(err) => {
                let failures = prior_failures + 1;
                self.files
                    .insert(path.to_path_buf(), FileState::Failed { stamp, failures });
                if REPORT_FAILURES.contains(&failures) {
                    let error = tf(
                        settings.language,
                        "archive.ingest_failed",
                        &[("error", format!("{err:#}").as_str())],
                    );
                    events.push(WatchEvent::Error {
                        path: path.to_path_buf(),
                        error,
                    });
                }
            }
        }
    }

    /// Drop bookkeeping for files that no longer exist, keeping entries of folders
    /// that are merely unavailable right now.
    fn forget_missing(&mut self, present: &HashSet<PathBuf>) {
        let unavailable = &self.unavailable;
        self.files.retain(|path, _| {
            present.contains(path) || path.parent().is_some_and(|dir| unavailable.contains(dir))
        });
    }
}

fn stamp_of(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    Some(Stamp {
        size: meta.len(),
        mtime: meta.modified().ok(),
    })
}

/// True when the file's content hash is already in the index. Errors are logged
/// and treated as "not indexed", so the normal import path decides.
fn already_indexed(path: &Path, store: &Store) -> bool {
    let (hash, _) = match sha256_file(path) {
        Ok(hashed) => hashed,
        Err(err) => {
            tracing::debug!(path = %path.display(), error = %err, "cannot hash existing file");
            return false;
        }
    };
    match store.find_by_hash(&hash) {
        Ok(found) => found.is_some(),
        Err(err) => {
            tracing::warn!(error = %err, "cannot query index for existing file");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::ingest::testutil::pdf_bytes;
    use crate::model::FaxQuery;

    const FAX_NAME: &str = "FAX_20261110_103045_0912345678.pdf";

    fn setup(dir: &Path) -> (Settings, PathBuf) {
        let watch = dir.join("in");
        std::fs::create_dir_all(&watch).unwrap();
        let settings = Settings {
            archive_root: dir.join("archive"),
            watch_folders: vec![watch.clone()],
            ..Settings::default()
        };
        (settings, watch)
    }

    fn count_of(events: &[WatchEvent], pred: impl Fn(&WatchEvent) -> bool) -> usize {
        events.iter().filter(|e| pred(e)).count()
    }

    fn imported(events: &[WatchEvent]) -> usize {
        count_of(events, |e| matches!(e, WatchEvent::Imported { .. }))
    }

    fn duplicates(events: &[WatchEvent]) -> usize {
        count_of(events, |e| matches!(e, WatchEvent::Duplicate { .. }))
    }

    fn errors(events: &[WatchEvent]) -> usize {
        count_of(events, |e| matches!(e, WatchEvent::Error { .. }))
    }

    fn only_scan_completed(events: &[WatchEvent]) -> bool {
        events == [WatchEvent::ScanCompleted]
    }

    #[test]
    fn file_is_ingested_once_after_it_is_stable_on_two_polls() {
        let dir = tempfile::tempdir().unwrap();
        let (settings, watch) = setup(dir.path());
        let store = Store::open_in_memory().unwrap();
        std::fs::write(watch.join(FAX_NAME), pdf_bytes(2)).unwrap();
        let mut poller = Poller::default();

        let first = poller.poll_once(&settings, &store);
        assert!(only_scan_completed(&first), "first poll: {first:?}");
        let second = poller.poll_once(&settings, &store);
        assert_eq!(imported(&second), 1, "second poll: {second:?}");
        let third = poller.poll_once(&settings, &store);
        assert!(only_scan_completed(&third), "third poll: {third:?}");

        let rows = store.search_all(&FaxQuery::default()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].pages, 2);
    }

    #[test]
    fn growing_file_waits_until_its_size_is_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let (settings, watch) = setup(dir.path());
        let store = Store::open_in_memory().unwrap();
        let bytes = pdf_bytes(1);
        let path = watch.join(FAX_NAME);
        let half = bytes.len() / 2;
        std::fs::write(&path, &bytes[..half]).unwrap();
        let mut poller = Poller::default();

        assert!(only_scan_completed(&poller.poll_once(&settings, &store)));
        std::fs::write(&path, &bytes).unwrap();
        let grown = poller.poll_once(&settings, &store);
        assert_eq!(
            imported(&grown),
            0,
            "must wait while size changes: {grown:?}"
        );
        let stable = poller.poll_once(&settings, &store);
        assert_eq!(imported(&stable), 1, "stable poll: {stable:?}");
        assert_eq!(store.search_all(&FaxQuery::default()).unwrap().len(), 1);
    }

    #[test]
    fn zero_byte_file_is_ignored_until_it_has_content() {
        let dir = tempfile::tempdir().unwrap();
        let (settings, watch) = setup(dir.path());
        let store = Store::open_in_memory().unwrap();
        let path = watch.join(FAX_NAME);
        std::fs::write(&path, b"").unwrap();
        let mut poller = Poller::default();

        assert!(only_scan_completed(&poller.poll_once(&settings, &store)));
        assert!(only_scan_completed(&poller.poll_once(&settings, &store)));
        std::fs::write(&path, pdf_bytes(1)).unwrap();
        assert_eq!(imported(&poller.poll_once(&settings, &store)), 0);
        assert_eq!(imported(&poller.poll_once(&settings, &store)), 1);
    }

    #[test]
    fn missing_folder_reports_once_per_change_of_state() {
        let dir = tempfile::tempdir().unwrap();
        let (settings, watch) = setup(dir.path());
        let store = Store::open_in_memory().unwrap();
        std::fs::remove_dir_all(&watch).unwrap();
        let mut poller = Poller::default();

        assert_eq!(errors(&poller.poll_once(&settings, &store)), 1);
        assert_eq!(errors(&poller.poll_once(&settings, &store)), 0);
        std::fs::create_dir_all(&watch).unwrap();
        assert!(only_scan_completed(&poller.poll_once(&settings, &store)));
        std::fs::remove_dir_all(&watch).unwrap();
        assert_eq!(errors(&poller.poll_once(&settings, &store)), 1);
    }

    #[test]
    fn file_already_indexed_before_start_produces_no_event() {
        let dir = tempfile::tempdir().unwrap();
        let (settings, watch) = setup(dir.path());
        let store = Store::open_in_memory().unwrap();
        let path = watch.join(FAX_NAME);
        std::fs::write(&path, pdf_bytes(1)).unwrap();
        // Index it as an earlier session would have (source kept, delete flag off).
        let earlier = crate::archive::ingest::ingest_file(
            &path,
            crate::model::FaxSource::WatchFolder,
            &settings,
            &store,
        )
        .unwrap();
        assert!(matches!(
            earlier,
            crate::archive::ingest::IngestOutcome::Imported { .. }
        ));

        let mut poller = Poller::default();
        let first = poller.poll_once(&settings, &store);
        let second = poller.poll_once(&settings, &store);
        assert!(only_scan_completed(&first), "first: {first:?}");
        assert!(only_scan_completed(&second), "second: {second:?}");
        assert_eq!(store.search_all(&FaxQuery::default()).unwrap().len(), 1);
    }

    #[test]
    fn unknown_file_present_at_start_is_still_imported() {
        let dir = tempfile::tempdir().unwrap();
        let (settings, watch) = setup(dir.path());
        let store = Store::open_in_memory().unwrap();
        std::fs::write(watch.join(FAX_NAME), pdf_bytes(1)).unwrap();
        let mut poller = Poller::default();
        assert_eq!(imported(&poller.poll_once(&settings, &store)), 0);
        assert_eq!(imported(&poller.poll_once(&settings, &store)), 1);
    }

    #[test]
    fn duplicate_content_reports_duplicate_once() {
        let dir = tempfile::tempdir().unwrap();
        let (settings, watch) = setup(dir.path());
        let store = Store::open_in_memory().unwrap();
        let bytes = pdf_bytes(1);
        std::fs::write(watch.join(FAX_NAME), &bytes).unwrap();
        std::fs::write(watch.join("again.pdf"), &bytes).unwrap();
        let mut poller = Poller::default();

        poller.poll_once(&settings, &store);
        let stable = poller.poll_once(&settings, &store);
        assert_eq!(imported(&stable), 1, "{stable:?}");
        assert_eq!(duplicates(&stable), 1, "{stable:?}");
        assert!(only_scan_completed(&poller.poll_once(&settings, &store)));
    }

    #[test]
    fn repeated_failures_report_first_and_last_attempt_then_wait_for_change() {
        let dir = tempfile::tempdir().unwrap();
        let (mut settings, watch) = setup(dir.path());
        // Archive root below a regular file: every archive copy fails.
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, b"not a folder").unwrap();
        settings.archive_root = blocker.join("archive");
        let store = Store::open_in_memory().unwrap();
        let path = watch.join(FAX_NAME);
        std::fs::write(&path, pdf_bytes(1)).unwrap();
        let mut poller = Poller::default();

        let counts: Vec<usize> = (0..6)
            .map(|_| errors(&poller.poll_once(&settings, &store)))
            .collect();
        assert_eq!(counts, vec![0, 1, 0, 1, 0, 0]);

        std::fs::write(&path, [pdf_bytes(1), b"x".to_vec()].concat()).unwrap();
        assert_eq!(errors(&poller.poll_once(&settings, &store)), 0);
        assert_eq!(errors(&poller.poll_once(&settings, &store)), 1);
    }

    #[test]
    fn unsupported_files_and_subfolders_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let (settings, watch) = setup(dir.path());
        let store = Store::open_in_memory().unwrap();
        std::fs::write(watch.join("notes.txt"), b"hello").unwrap();
        let nested = watch.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join(FAX_NAME), pdf_bytes(1)).unwrap();
        let mut poller = Poller::default();
        for _ in 0..3 {
            assert!(only_scan_completed(&poller.poll_once(&settings, &store)));
        }
        assert!(store.search_all(&FaxQuery::default()).unwrap().is_empty());
    }
}
