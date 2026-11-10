//! Shared fixtures for the controller tests: a recording fake platform, a controller
//! over a temporary archive, and fax PDF builders. Tests never touch real folders,
//! printers or dialogs.

use super::Controller;
use crate::app::platform::Platform;
use crate::app::state::ToastKind;
use crate::config::Settings;
use crate::i18n::{Lang, t};
use crate::model::{FaxId, FaxRecord};
use crate::store::Store;
use anyhow::anyhow;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use tempfile::TempDir;

/// Calls the controller made on the platform, in order (e.g. `print_to:<file>:<printer>`).
pub(super) type Calls = Rc<RefCell<Vec<String>>>;

/// Scripted answers for the native pickers.
#[derive(Default)]
pub(super) struct Picks {
    pub(super) folder: Option<PathBuf>,
    pub(super) file: Option<PathBuf>,
}

/// Test-side handles to the platform that the controller owns.
pub(super) struct Probe {
    pub(super) calls: Calls,
    pub(super) picks: Rc<RefCell<Picks>>,
}

impl Probe {
    /// Recorded calls whose text starts with `prefix`.
    pub(super) fn calls_with(&self, prefix: &str) -> Vec<String> {
        self.calls
            .borrow()
            .iter()
            .filter(|call| call.starts_with(prefix))
            .cloned()
            .collect()
    }
}

/// Recording platform: no program is launched and no dialog is shown.
pub(super) struct FakePlatform {
    calls: Calls,
    picks: Rc<RefCell<Picks>>,
    printers: Vec<String>,
    fail_print_to: bool,
}

impl FakePlatform {
    pub(super) fn boxed(printers: Vec<String>, fail_print_to: bool) -> (Box<dyn Platform>, Probe) {
        let calls: Calls = Rc::new(RefCell::new(Vec::new()));
        let picks = Rc::new(RefCell::new(Picks::default()));
        let platform = FakePlatform {
            calls: Rc::clone(&calls),
            picks: Rc::clone(&picks),
            printers,
            fail_print_to,
        };
        (Box::new(platform), Probe { calls, picks })
    }

    fn record(&self, entry: String) {
        self.calls.borrow_mut().push(entry);
    }
}

impl Platform for FakePlatform {
    fn open_path(&self, path: &Path) -> anyhow::Result<()> {
        self.record(format!("open:{}", path.display()));
        Ok(())
    }

    fn reveal_in_folder(&self, path: &Path) -> anyhow::Result<()> {
        self.record(format!("reveal:{}", path.display()));
        Ok(())
    }

    fn print_file(&self, path: &Path) -> anyhow::Result<()> {
        self.record(format!("print:{}", path.display()));
        Ok(())
    }

    fn print_to(&self, path: &Path, printer: &str) -> anyhow::Result<()> {
        self.record(format!("print_to:{}:{printer}", path.display()));
        if self.fail_print_to {
            return Err(anyhow!("driver offline"));
        }
        Ok(())
    }

    fn list_printers(&self) -> anyhow::Result<Vec<String>> {
        Ok(self.printers.clone())
    }

    fn pick_folder(&self, _start: Option<&Path>) -> Option<PathBuf> {
        self.record("pick_folder".to_owned());
        self.picks.borrow_mut().folder.take()
    }

    fn pick_file(&self, _start: Option<&Path>, extensions: &[&str]) -> Option<PathBuf> {
        self.record(format!("pick_file:{}", extensions.join(",")));
        self.picks.borrow_mut().file.take()
    }

    fn save_file(
        &self,
        _start: Option<&Path>,
        _default_name: &str,
        _extensions: &[&str],
    ) -> Option<PathBuf> {
        None
    }
}

/// Settings whose archive lives inside `dir`.
pub(super) fn settings_in(dir: &Path) -> Settings {
    Settings {
        archive_root: dir.join("archive"),
        ..Settings::default()
    }
}

/// A controller over a temporary directory. `dir` is removed when dropped.
pub(super) struct Harness {
    pub(super) dir: TempDir,
    pub(super) controller: Controller,
    pub(super) probe: Probe,
}

impl Harness {
    pub(super) fn new() -> Self {
        Self::with_platform(Vec::new(), false)
    }

    pub(super) fn with_platform(printers: Vec<String>, fail_print_to: bool) -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let settings = settings_in(dir.path());
        let store = Store::open_in_memory().expect("in-memory store");
        let db_path = dir.path().join("myfax.db");
        Self::assemble(dir, settings, store, db_path, printers, fail_print_to)
    }

    /// Controller over a file-backed store (needed when a real watcher shares the db).
    pub(super) fn with_store(settings: Settings, store: Store, db_path: PathBuf) -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        Self::assemble(dir, settings, store, db_path, Vec::new(), false)
    }

    fn assemble(
        dir: TempDir,
        settings: Settings,
        store: Store,
        db_path: PathBuf,
        printers: Vec<String>,
        fail_print_to: bool,
    ) -> Self {
        let (platform, probe) = FakePlatform::boxed(printers, fail_print_to);
        let settings_path = dir.path().join("settings.json");
        let controller = Controller::with_store(settings, store, platform, settings_path, db_path);
        Self {
            dir,
            controller,
            probe,
        }
    }

    pub(super) fn path(&self, relative: &str) -> PathBuf {
        self.dir.path().join(relative)
    }

    /// Write bytes to `relative` inside the temp dir (parents created).
    pub(super) fn write(&self, relative: &str, bytes: &[u8]) -> PathBuf {
        let path = self.path(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, bytes).expect("write fixture");
        path
    }

    /// Import one fax PDF named `name` (dropped on the window, like a user would).
    pub(super) fn import_fax(&mut self, name: &str, pages: u32) -> PathBuf {
        let path = self.write(&format!("incoming/{name}"), &fax_pdf(pages, name));
        self.controller.files_dropped(vec![path.clone()]);
        path
    }

    /// Id of the first result row whose original file name is `name`.
    pub(super) fn id_of(&self, name: &str) -> FaxId {
        self.controller
            .state
            .results
            .items
            .iter()
            .find(|fax| fax.original_name == name)
            .map(|fax| fax.id)
            .expect("fax present in results")
    }

    pub(super) fn fax(&self, id: FaxId) -> FaxRecord {
        self.controller
            .store
            .get_fax(id)
            .expect("store readable")
            .expect("fax exists")
    }

    /// Current toast as (kind, text), if any.
    pub(super) fn toast(&self) -> Option<(ToastKind, String)> {
        self.controller
            .state
            .toast
            .as_ref()
            .map(|toast| (toast.kind, toast.text.clone()))
    }

    /// Text of the current toast, panicking when none is shown.
    pub(super) fn toast_text(&self) -> String {
        self.toast().expect("a toast is shown").1
    }

    /// Message texts of the activity log, oldest first.
    pub(super) fn log_messages(&self) -> Vec<String> {
        let mut logs = self
            .controller
            .store
            .recent_logs(500)
            .expect("logs readable");
        logs.reverse();
        logs.into_iter().map(|entry| entry.message).collect()
    }
}

/// Localised text for a catalog key in the default language (zh-TW).
pub(super) fn zh(key: &'static str) -> &'static str {
    t(Lang::ZhTw, key)
}

/// Minimal fax PDF with `pages` empty pages. `tag` makes each file's bytes (and
/// therefore its SHA-256) unique so repeated imports are not treated as duplicates.
pub(super) fn fax_pdf(pages: u32, tag: &str) -> Vec<u8> {
    let mut bytes = crate::archive::ingest::testutil::pdf_bytes(pages);
    // Encode the tag as trailing whitespace (one space/tab per bit) so the file
    // still ends with `%%EOF` plus whitespace — what the watcher treats as complete.
    bytes.push(b'\n');
    for byte in tag.bytes() {
        for bit in 0..8 {
            bytes.push(if byte >> bit & 1 == 1 { b'\t' } else { b' ' });
        }
    }
    bytes.push(b'\n');
    bytes
}

/// Filename in the printer's forwarding style, received on `y-m-d hh:mm:ss`.
pub(super) fn fax_name(y: i32, m: u32, d: u32, hh: u32, mm: u32, number: &str) -> String {
    format!("FAX_{y:04}{m:02}{d:02}_{hh:02}{mm:02}00_{number}.pdf")
}
