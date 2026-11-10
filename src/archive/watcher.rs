//! Background polling watcher for inbox folders.

mod poller;

use crate::config::Settings;
use crate::i18n::tf;
use crate::model::FaxId;
use crate::store::Store;
use poller::Poller;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Events sent from the watcher thread to the UI thread.
#[derive(Clone, Debug, PartialEq)]
pub enum WatchEvent {
    /// A fax was archived and indexed.
    Imported {
        id: FaxId,
        source: PathBuf,
    },
    Duplicate {
        source: PathBuf,
    },
    /// Non-fatal error for one file or folder (path, error text).
    Error {
        path: PathBuf,
        error: String,
    },
    /// One full poll cycle completed.
    ScanCompleted,
}

/// Callback invoked (from the watcher thread) after events were queued — used
/// to wake the UI event loop.
pub type Notify = Box<dyn Fn() + Send + 'static>;

/// Requests from the handle to the watcher thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Control {
    ScanNow,
    Stop,
}

pub struct WatcherHandle {
    control: Sender<Control>,
    events: Receiver<WatchEvent>,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl WatcherHandle {
    /// Spawn the polling thread. It opens its own `Store` at `db_path`.
    /// A file is ingested only after its size and mtime are unchanged across
    /// two consecutive polls (printer still writing otherwise) and it can be
    /// opened for reading. Files that failed are retried at most 3 times, then
    /// skipped until their size/mtime change. Only top-level files of each
    /// folder are read. Missing folders produce one `Error` per change of state.
    pub fn spawn(settings: Settings, db_path: PathBuf, notify: Notify) -> std::io::Result<Self> {
        let settings = settings.sanitized();
        let (control, control_rx) = mpsc::channel();
        let (events_tx, events) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let join = thread::Builder::new()
            .name("myfax-archive-watcher".to_owned())
            .spawn(move || {
                run(
                    settings,
                    db_path,
                    Thread {
                        control: control_rx,
                        events: events_tx,
                        notify,
                        stop: thread_stop,
                    },
                )
            })?;
        Ok(Self {
            control,
            events,
            stop,
            join: Some(join),
        })
    }

    /// Drain queued events (non-blocking).
    pub fn try_events(&self) -> Vec<WatchEvent> {
        self.events.try_iter().collect()
    }

    /// Request an immediate poll. No-op once the thread has exited.
    pub fn scan_now(&self) {
        // The send fails only after the thread exited; there is nothing to wake then.
        let _ = self.control.send(Control::ScanNow);
    }

    /// Stop and join the thread (also done on Drop).
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = self.control.send(Control::Stop);
        if let Some(Err(_)) = self.join.take().map(JoinHandle::join) {
            tracing::error!("archive watcher thread panicked");
        }
    }
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Channels and flags the watcher thread owns.
struct Thread {
    control: Receiver<Control>,
    events: Sender<WatchEvent>,
    notify: Notify,
    stop: Arc<AtomicBool>,
}

fn run(settings: Settings, db_path: PathBuf, thread: Thread) {
    let store = match Store::open(&db_path) {
        Ok(store) => store,
        Err(err) => {
            let error = tf(
                settings.language,
                "archive.store_open_failed",
                &[("error", err.to_string().as_str())],
            );
            deliver(
                &thread,
                vec![WatchEvent::Error {
                    path: db_path,
                    error,
                }],
            );
            return;
        }
    };
    let interval = Duration::from_secs(settings.poll_interval_secs);
    let mut poller = Poller::new(Arc::clone(&thread.stop));
    loop {
        if thread.stop.load(Ordering::SeqCst) {
            break;
        }
        let batch = poller.poll_once(&settings, &store);
        if !deliver(&thread, batch) {
            break;
        }
        match thread.control.recv_timeout(interval) {
            Ok(Control::ScanNow) | Err(RecvTimeoutError::Timeout) => {}
            Ok(Control::Stop) | Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// Send one cycle's events, then notify once if any non-`ScanCompleted` event
/// was queued. Returns false when the handle side has gone away.
fn deliver(thread: &Thread, batch: Vec<WatchEvent>) -> bool {
    let wake = batch
        .iter()
        .any(|event| !matches!(event, WatchEvent::ScanCompleted));
    for event in batch {
        if thread.events.send(event).is_err() {
            return false;
        }
    }
    if wake {
        (thread.notify)();
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::ingest::testutil::pdf_bytes;
    use std::sync::atomic::AtomicUsize;
    use std::time::Instant;

    const WAIT: Duration = Duration::from_secs(15);

    fn settings_for(dir: &std::path::Path, watch: &[PathBuf]) -> Settings {
        Settings {
            archive_root: dir.join("archive"),
            watch_folders: watch.to_vec(),
            poll_interval_secs: 2,
            ..Settings::default()
        }
    }

    fn counting_notify() -> (Notify, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        let inner = Arc::clone(&count);
        let notify: Notify = Box::new(move || {
            inner.fetch_add(1, Ordering::SeqCst);
        });
        (notify, count)
    }

    /// Poll the handle until `done` accepts the collected events or the deadline passes.
    fn wait_for(
        handle: &WatcherHandle,
        mut done: impl FnMut(&[WatchEvent]) -> bool,
    ) -> Vec<WatchEvent> {
        let deadline = Instant::now() + WAIT;
        let mut all = Vec::new();
        while Instant::now() < deadline {
            all.extend(handle.try_events());
            if done(&all) {
                return all;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        all
    }

    #[test]
    fn threaded_scan_now_ingests_once_and_stop_joins() {
        let dir = tempfile::tempdir().unwrap();
        let watch = dir.path().join("in");
        std::fs::create_dir_all(&watch).unwrap();
        std::fs::write(
            watch.join("FAX_20261110_103045_0912345678.pdf"),
            pdf_bytes(2),
        )
        .unwrap();
        let (notify, notify_count) = counting_notify();
        let settings = settings_for(dir.path(), std::slice::from_ref(&watch));
        let mut handle =
            WatcherHandle::spawn(settings, dir.path().join("myfax.db"), notify).unwrap();

        handle.scan_now();
        let events = wait_for(&handle, |events| {
            events
                .iter()
                .any(|e| matches!(e, WatchEvent::Imported { .. }))
        });
        let imported = events
            .iter()
            .filter(|e| matches!(e, WatchEvent::Imported { .. }))
            .count();
        assert_eq!(imported, 1, "events: {events:?}");

        handle.scan_now();
        let later = wait_for(&handle, |events| {
            events
                .iter()
                .any(|e| matches!(e, WatchEvent::ScanCompleted))
        });
        assert!(
            !later
                .iter()
                .any(|e| matches!(e, WatchEvent::Imported { .. })),
            "file imported twice: {later:?}"
        );
        assert!(notify_count.load(Ordering::SeqCst) >= 1);

        let (done_tx, done_rx) = mpsc::channel();
        let stopper = thread::spawn(move || {
            handle.stop();
            let _ = done_tx.send(());
        });
        assert!(
            done_rx.recv_timeout(WAIT).is_ok(),
            "stop() did not return in time"
        );
        stopper.join().unwrap();
    }

    #[test]
    fn unopenable_database_reports_one_error_and_thread_exits() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, b"file, not a folder").unwrap();
        let (notify, _count) = counting_notify();
        let settings = settings_for(dir.path(), &[]);
        let mut handle = WatcherHandle::spawn(settings, blocker.join("myfax.db"), notify).unwrap();
        let events = wait_for(&handle, |events| !events.is_empty());
        assert!(
            events.iter().any(|e| matches!(e, WatchEvent::Error { .. })),
            "expected Error event, got {events:?}"
        );
        handle.stop();
    }

    #[test]
    fn dropping_handle_stops_thread() {
        let dir = tempfile::tempdir().unwrap();
        let (notify, _count) = counting_notify();
        let settings = settings_for(dir.path(), &[]);
        let handle = WatcherHandle::spawn(settings, dir.path().join("myfax.db"), notify).unwrap();
        let (done_tx, done_rx) = mpsc::channel();
        let dropper = thread::spawn(move || {
            drop(handle);
            let _ = done_tx.send(());
        });
        assert!(done_rx.recv_timeout(WAIT).is_ok(), "drop did not join");
        dropper.join().unwrap();
    }
}
