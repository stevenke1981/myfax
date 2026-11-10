//! Background polling watcher for inbox folders.

use crate::config::Settings;
use crate::model::FaxId;
use std::path::PathBuf;

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

pub struct WatcherHandle {
    _private: (),
}

#[allow(unused_variables)]
impl WatcherHandle {
    /// Spawn the polling thread. It opens its own `Store` at `db_path`.
    /// A file is ingested only after its size and mtime are unchanged across
    /// two consecutive polls (printer still writing otherwise) and it can be
    /// opened for reading. Files that failed are retried at most 3 times, then
    /// skipped until their size/mtime change. Only top-level files of each
    /// folder are read. Missing folders produce one `Error` per change of state.
    pub fn spawn(settings: Settings, db_path: PathBuf, notify: Notify) -> std::io::Result<Self> {
        todo!()
    }
    /// Drain queued events (non-blocking).
    pub fn try_events(&self) -> Vec<WatchEvent> {
        todo!()
    }
    /// Request an immediate poll.
    pub fn scan_now(&self) {
        todo!()
    }
    /// Stop and join the thread (also done on Drop).
    pub fn stop(&mut self) {
        todo!()
    }
}
