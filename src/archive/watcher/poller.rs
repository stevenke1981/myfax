//! One poll cycle over the watch folders, free of threads so it can be tested
//! directly. The watcher thread owns a `Poller` and calls `poll_once` in a loop.

use crate::archive::ingest::{IngestOutcome, ingest_file, is_supported, sha256_file};
use crate::archive::watcher::WatchEvent;
use crate::config::Settings;
use crate::i18n::tf;
use crate::model::FaxSource;
use crate::store::Store;
use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

/// A file is retried at most this many times at the same size/mtime.
const MAX_FAILURES: u32 = 3;
/// Failure counts that produce an `Error` event (first and last retry).
const REPORT_FAILURES: [u32; 2] = [1, MAX_FAILURES];
/// Minimum age of a file's modification time before it is ingested. Guards against
/// SMB metadata caching that shows a size and mtime before the data is complete.
const STABLE_MIN_AGE: Duration = Duration::from_secs(5);
/// Unchanged polls after which a file is ingested even if it still looks incomplete.
const FORCE_AFTER_STABLE_POLLS: u32 = 10;
/// Bytes read from the end of a PDF when looking for its end-of-file marker.
const PDF_TAIL_BYTES: u64 = 1024;

/// Observed size and modification time of one file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stamp {
    size: u64,
    mtime: Option<SystemTime>,
}

/// Per-file bookkeeping between polls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileState {
    /// Seen with this stamp; `unchanged` counts the polls since with the same stamp.
    Observed { stamp: Stamp, unchanged: u32 },
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
    /// Set by the watcher handle. Checked before each file so `stop()` returns promptly.
    stop: Arc<AtomicBool>,
    /// Minimum modification-time age before a file counts as settled. The default
    /// (zero) disables the age check; the watcher uses `STABLE_MIN_AGE` via `new`.
    min_age: Duration,
    /// Artificial per-file delay that lets tests hold a poll open.
    #[cfg(test)]
    visit_delay: Duration,
}

impl Poller {
    /// A poller that stops early once `stop` is set, with the production age gate.
    pub(super) fn new(stop: Arc<AtomicBool>) -> Self {
        Self {
            stop,
            min_age: STABLE_MIN_AGE,
            ..Self::default()
        }
    }

    /// Run one cycle over every watch folder. The returned list always ends with
    /// `ScanCompleted`. After a stop request the cycle ends early and keeps its
    /// bookkeeping untouched.
    pub(super) fn poll_once(&mut self, settings: &Settings, store: &Store) -> Vec<WatchEvent> {
        let settings = settings.sanitized();
        let first_pass = !self.started;
        self.started = true;
        let mut events = Vec::new();
        let mut present = HashSet::new();
        for folder in &settings.watch_folders {
            if self.stop_requested() {
                break;
            }
            self.scan_folder(
                folder,
                &settings,
                store,
                first_pass,
                &mut present,
                &mut events,
            );
        }
        if !self.stop_requested() {
            self.forget_missing(&present);
        }
        events.push(WatchEvent::ScanCompleted);
        events
    }

    fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
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
            if self.stop_requested() {
                return;
            }
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
        #[cfg(test)]
        std::thread::sleep(self.visit_delay);
        if self.stop_requested() {
            return;
        }
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
            Some(FileState::Failed { stamp: seen, .. }) if seen == stamp => {}
            Some(FileState::Observed {
                stamp: seen,
                unchanged,
            }) if seen == stamp => {
                let stable_polls = unchanged + 1;
                let forced = stable_polls >= FORCE_AFTER_STABLE_POLLS;
                if !forced && !looks_complete(path, stamp, self.min_age) {
                    self.files.insert(
                        path.to_path_buf(),
                        FileState::Observed {
                            stamp,
                            unchanged: stable_polls,
                        },
                    );
                    return;
                }
            }
            None if first_pass && already_indexed(path, store) => {
                self.files
                    .insert(path.to_path_buf(), FileState::Done(stamp));
                return;
            }
            _ => {
                self.files.insert(
                    path.to_path_buf(),
                    FileState::Observed {
                        stamp,
                        unchanged: 0,
                    },
                );
                return;
            }
        }
        // Still being written by another process if it cannot be opened.
        if std::fs::File::open(path).is_err() || self.stop_requested() {
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

/// Whether a file that kept its stamp looks completely written: old enough, and
/// structurally complete for its type. A failing check only delays ingestion.
fn looks_complete(path: &Path, stamp: Stamp, min_age: Duration) -> bool {
    is_old_enough(stamp.mtime, min_age, SystemTime::now()) && content_is_complete(path)
}

/// A missing modification time cannot prove a file is young, so it passes.
fn is_old_enough(mtime: Option<SystemTime>, min_age: Duration, now: SystemTime) -> bool {
    match mtime {
        None => true,
        Some(mtime) => now.duration_since(mtime).is_ok_and(|age| age >= min_age),
    }
}

/// PDFs must end with `%%EOF` (trailing whitespace allowed); TIFFs must start with
/// a byte-order header. Other types are not checked.
fn content_is_complete(path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("pdf") => pdf_ends_with_marker(path),
        Some("tif" | "tiff") => tiff_has_header(path),
        _ => true,
    }
}

fn pdf_ends_with_marker(path: &Path) -> bool {
    tail_bytes(path, PDF_TAIL_BYTES).is_ok_and(|tail| tail.trim_ascii_end().ends_with(b"%%EOF"))
}

fn tail_bytes(path: &Path, max: u64) -> io::Result<Vec<u8>> {
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(max)))?;
    let mut tail = Vec::new();
    file.take(max).read_to_end(&mut tail)?;
    Ok(tail)
}

fn tiff_has_header(path: &Path) -> bool {
    let mut magic = [0u8; 4];
    let read = std::fs::File::open(path).and_then(|mut file| file.read_exact(&mut magic));
    read.is_ok() && (magic == *b"II*\0" || magic == *b"MM\0*")
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

        // A trailing newline changes the size but keeps the PDF end marker valid.
        std::fs::write(&path, [pdf_bytes(1), b"\n".to_vec()].concat()).unwrap();
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

    /// 1-based number of the first poll that imports something, if within `max` polls.
    fn first_import_poll(
        poller: &mut Poller,
        settings: &Settings,
        store: &Store,
        max: usize,
    ) -> Option<usize> {
        (1..=max).find(|_| imported(&poller.poll_once(settings, store)) > 0)
    }

    #[test]
    fn pdf_without_end_marker_waits_until_forced_after_ten_stable_polls() {
        let dir = tempfile::tempdir().unwrap();
        let (settings, watch) = setup(dir.path());
        let store = Store::open_in_memory().unwrap();
        let bytes = pdf_bytes(1);
        let end_marker = bytes
            .windows(5)
            .rposition(|window| window == b"%%EOF")
            .expect("generated pdf has an end marker");
        std::fs::write(watch.join(FAX_NAME), &bytes[..end_marker]).unwrap();
        let mut poller = Poller::default();

        assert_eq!(
            first_import_poll(&mut poller, &settings, &store, 15),
            Some(11)
        );
    }

    #[test]
    fn pdf_with_trailing_newlines_after_end_marker_is_ready_on_second_poll() {
        let dir = tempfile::tempdir().unwrap();
        let (settings, watch) = setup(dir.path());
        let store = Store::open_in_memory().unwrap();
        let bytes = [pdf_bytes(1), b"\r\n\r\n".to_vec()].concat();
        std::fs::write(watch.join(FAX_NAME), bytes).unwrap();
        let mut poller = Poller::default();

        assert_eq!(
            first_import_poll(&mut poller, &settings, &store, 15),
            Some(2)
        );
    }

    #[test]
    fn tiff_without_header_waits_until_forced_after_ten_stable_polls() {
        let dir = tempfile::tempdir().unwrap();
        let (mut settings, watch) = setup(dir.path());
        settings.accept_tiff = true;
        let store = Store::open_in_memory().unwrap();
        std::fs::write(
            watch.join("FAX_20261110_103045_0912345678.tif"),
            b"not a tiff header",
        )
        .unwrap();
        let mut poller = Poller::default();

        assert_eq!(
            first_import_poll(&mut poller, &settings, &store, 15),
            Some(11)
        );
    }

    #[test]
    fn tiff_with_header_is_ready_on_second_poll() {
        let dir = tempfile::tempdir().unwrap();
        let (mut settings, watch) = setup(dir.path());
        settings.accept_tiff = true;
        let store = Store::open_in_memory().unwrap();
        std::fs::write(
            watch.join("FAX_20261110_103045_0912345678.tif"),
            b"II*\0 rest of a tiff",
        )
        .unwrap();
        let mut poller = Poller::default();

        assert_eq!(
            first_import_poll(&mut poller, &settings, &store, 15),
            Some(2)
        );
    }

    #[test]
    fn young_file_waits_for_minimum_age_until_forced() {
        let dir = tempfile::tempdir().unwrap();
        let (settings, watch) = setup(dir.path());
        let store = Store::open_in_memory().unwrap();
        std::fs::write(watch.join(FAX_NAME), pdf_bytes(1)).unwrap();
        let mut poller = Poller {
            min_age: Duration::from_secs(3600),
            ..Poller::default()
        };

        assert_eq!(
            first_import_poll(&mut poller, &settings, &store, 15),
            Some(11)
        );
    }

    #[test]
    fn stop_request_ends_a_poll_over_many_files_promptly() {
        let dir = tempfile::tempdir().unwrap();
        let (settings, watch) = setup(dir.path());
        let store = Store::open_in_memory().unwrap();
        let bytes = pdf_bytes(1);
        for index in 0..200 {
            std::fs::write(watch.join(format!("scan_{index:03}.pdf")), &bytes).unwrap();
        }
        let stop = Arc::new(AtomicBool::new(false));
        let mut poller = Poller {
            stop: Arc::clone(&stop),
            ..Poller::default()
        };
        // The first poll records every file; the second ingests them, slowed per file.
        poller.poll_once(&settings, &store);
        poller.visit_delay = Duration::from_millis(10);

        let requester = Arc::clone(&stop);
        let stopper = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            requester.store(true, Ordering::SeqCst);
            std::time::Instant::now()
        });
        let events = poller.poll_once(&settings, &store);
        let returned = std::time::Instant::now();
        let requested_at = stopper.join().expect("stopper thread");

        assert!(
            returned.duration_since(requested_at) < Duration::from_secs(2),
            "poll kept running after the stop request"
        );
        assert!(
            imported(&events) < 200,
            "poll was not interrupted: {events:?}"
        );
    }
}
