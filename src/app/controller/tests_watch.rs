//! A real folder watcher on a temporary inbox: a dropped fax is archived, the UI is
//! notified through `poll_background`, and auto-print runs. The wait is bounded (10 s).

use super::WakeFn;
use super::tests::{Harness, fax_name, fax_pdf};
use crate::app::state::WatcherStatus;
use crate::config::Settings;
use crate::store::Store;
use std::sync::Arc;
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(10);
const STEP: Duration = Duration::from_millis(100);

#[test]
fn running_watcher_archives_a_dropped_fax_and_notifies_the_ui() {
    let dir = tempfile::tempdir().expect("temp dir");
    let inbox = dir.path().join("inbox");
    std::fs::create_dir_all(&inbox).expect("mkdir inbox");
    let db = dir.path().join("myfax.db");
    let settings = Settings {
        archive_root: dir.path().join("archive"),
        watch_folders: vec![inbox.clone()],
        poll_interval_secs: 2,
        auto_print: true,
        ..Settings::default()
    };
    let store = Store::open(&db).expect("file-backed store");
    let mut h = Harness::with_store(settings, store, db);

    let wake: WakeFn = Arc::new(|| {});
    h.controller.start_watcher(wake);
    assert!(matches!(
        h.controller.state.watcher,
        WatcherStatus::Running(1)
    ));

    let name = fax_name(2026, 3, 5, 9, 0, "0912345678");
    std::fs::write(inbox.join(&name), fax_pdf(1, &name)).expect("drop fax");

    let deadline = Instant::now() + WAIT;
    let mut changed = false;
    while Instant::now() < deadline && h.controller.state.counts.all == 0 {
        changed |= h.controller.poll_background();
        std::thread::sleep(STEP);
    }

    assert_eq!(
        h.controller.state.counts.all, 1,
        "fax archived within {WAIT:?}"
    );
    assert!(changed, "poll_background reports the change");
    assert_eq!(h.controller.state.new_fax_banner, 1);
    assert_eq!(
        h.probe.calls_with("print:").len(),
        1,
        "auto-print sent the new fax"
    );

    h.controller.shutdown();
    assert!(matches!(h.controller.state.watcher, WatcherStatus::Stopped));
}
