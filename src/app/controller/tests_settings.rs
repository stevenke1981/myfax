//! Settings page and chrome: validation and persistence, watch folders, toggles,
//! language switch, toast expiry, start-up with corrupt settings, pickers.

use super::Controller;
use super::tests::{FakePlatform, Harness, zh};
use crate::app::msg::{BoolSetting, Field, Msg, Route};
use crate::app::state::ToastKind;
use crate::config::{self, Settings, ThemeChoice};
use crate::i18n::{Lang, t, tf};
use std::time::Duration;

/// Every field valid except the one under test.
fn invalid_case(field: Field, value: &str) -> (Field, String) {
    (field, value.to_owned())
}

#[test]
fn each_invalid_settings_field_is_refused_and_nothing_is_written() {
    let cases = [
        (
            invalid_case(Field::ArchiveRoot, "   "),
            "app.settings_invalid_archive",
        ),
        (
            invalid_case(Field::PollInterval, "1"),
            "app.settings_invalid_poll",
        ),
        (
            invalid_case(Field::PollInterval, "3601"),
            "app.settings_invalid_poll",
        ),
        (
            invalid_case(Field::PollInterval, "soon"),
            "app.settings_invalid_poll",
        ),
        (
            invalid_case(Field::TrashRetention, "3651"),
            "app.settings_invalid_trash",
        ),
        (
            invalid_case(Field::TrashRetention, "-1"),
            "app.settings_invalid_trash",
        ),
        (
            invalid_case(Field::PageSize, "9"),
            "app.settings_invalid_page",
        ),
        (
            invalid_case(Field::PageSize, "501"),
            "app.settings_invalid_page",
        ),
    ];
    for ((field, value), key) in cases {
        let mut h = Harness::new();
        h.controller.set_field(field, value.clone());
        h.controller.update(Msg::SaveSettings);
        let reason = t(Lang::ZhTw, key);
        assert_eq!(
            h.toast(),
            Some((
                ToastKind::Error,
                tf(Lang::ZhTw, "app.settings_invalid", &[("reason", reason)])
            )),
            "field {field:?} value {value:?}"
        );
        assert!(
            !h.path("settings.json").exists(),
            "nothing saved for {field:?}={value:?}"
        );
    }
}

#[test]
fn valid_settings_are_persisted_applied_and_reprimed() {
    let mut h = Harness::new();
    let forward = h.path("copies");
    h.controller.set_field(Field::PollInterval, "30".to_owned());
    h.controller
        .set_field(Field::TrashRetention, "7".to_owned());
    h.controller.set_field(Field::PageSize, "25".to_owned());
    h.controller
        .set_field(Field::ForwardFolder, forward.display().to_string());
    h.controller.update(Msg::SaveSettings);

    assert_eq!(h.toast_text(), zh("app.settings_saved"));
    let saved = config::load_settings(&h.path("settings.json")).expect("saved file");
    assert_eq!(saved.poll_interval_secs, 30);
    assert_eq!(saved.trash_retention_days, 7);
    assert_eq!(saved.page_size, 25);
    assert_eq!(saved.forward_copy_folder, Some(forward));
    assert_eq!(h.controller.state.query.page_size, 25);
    assert_eq!(h.controller.state.field(Field::PageSize), "25");
}

#[test]
fn blank_forward_folder_saves_as_none() {
    let mut h = Harness::new();
    h.controller
        .set_field(Field::ForwardFolder, "  ".to_owned());
    h.controller.update(Msg::SaveSettings);
    let saved = config::load_settings(&h.path("settings.json")).expect("saved file");
    assert_eq!(saved.forward_copy_folder, None);
}

#[test]
fn watch_folders_are_validated_added_removed_and_persisted() {
    let mut h = Harness::new();
    let inbox = h.path("inbox");
    std::fs::create_dir_all(&inbox).expect("mkdir");

    let nowhere = h.path("nowhere").display().to_string();
    h.controller
        .set_field(Field::WatchFolderInput, nowhere.clone());
    h.controller.update(Msg::AddWatchFolder);
    assert_eq!(
        h.toast_text(),
        tf(Lang::ZhTw, "app.watch_missing", &[("path", &nowhere)])
    );
    assert!(h.controller.state.settings.watch_folders.is_empty());

    let inbox_text = inbox.display().to_string();
    h.controller
        .set_field(Field::WatchFolderInput, inbox_text.clone());
    h.controller.update(Msg::AddWatchFolder);
    assert_eq!(
        h.controller.state.settings.watch_folders,
        vec![inbox.clone()]
    );
    assert_eq!(h.controller.state.field(Field::WatchFolderInput), "");

    h.controller.set_field(Field::WatchFolderInput, inbox_text);
    h.controller.update(Msg::AddWatchFolder);
    assert_eq!(h.toast_text(), zh("app.watch_duplicate"));
    assert_eq!(h.controller.state.settings.watch_folders.len(), 1);

    h.controller.update(Msg::RemoveWatchFolder(5));
    assert_eq!(
        h.controller.state.settings.watch_folders.len(),
        1,
        "out of range is ignored"
    );

    h.controller.update(Msg::RemoveWatchFolder(0));
    assert!(h.controller.state.settings.watch_folders.is_empty());
    let saved = config::load_settings(&h.path("settings.json")).expect("saved file");
    assert!(saved.watch_folders.is_empty());
}

#[test]
fn toggles_flip_and_persist_immediately() {
    let mut h = Harness::new();
    assert!(!h.controller.state.settings.auto_print);
    h.controller
        .update(Msg::ToggleSetting(BoolSetting::AutoPrint));
    assert!(h.controller.state.settings.auto_print);
    let saved = config::load_settings(&h.path("settings.json")).expect("saved file");
    assert!(saved.auto_print);
}

#[test]
fn language_switch_changes_the_text_of_later_messages() {
    let mut h = Harness::new();
    h.controller.update(Msg::SetLanguage(Lang::En));
    assert_eq!(h.controller.state.lang, Lang::En);
    h.controller.update(Msg::ImportFromField);
    assert_eq!(h.toast_text(), t(Lang::En, "app.import_no_path"));
    let saved = config::load_settings(&h.path("settings.json")).expect("saved file");
    assert_eq!(saved.language, Lang::En);

    h.controller.update(Msg::SetTheme(ThemeChoice::Dark));
    assert_eq!(h.controller.state.theme, ThemeChoice::Dark);
}

#[test]
fn toasts_expire_after_their_own_time_to_live() {
    let mut h = Harness::new();
    h.controller.update(Msg::ImportFromField);
    let started = h.controller.toast_started.expect("timer started");
    assert!(
        !h.controller.tick_at(started + Duration::from_secs(7)),
        "error toast lives 8 s"
    );
    assert!(h.controller.tick_at(started + Duration::from_secs(8)));
    assert!(h.toast().is_none());

    h.controller.toast(ToastKind::Success, "ok".to_owned());
    let started = h.controller.toast_started.expect("timer started");
    assert!(
        h.controller.tick_at(started + Duration::from_secs(4)),
        "success toast lives 4 s"
    );
    assert!(h.toast().is_none());
    assert!(
        !h.controller.tick_at(started + Duration::from_secs(4)),
        "nothing left to hide"
    );
}

#[test]
fn dismiss_toast_clears_it_at_once() {
    let mut h = Harness::new();
    h.controller.update(Msg::ImportFromField);
    h.controller.update(Msg::DismissToast);
    assert!(h.toast().is_none());
}

#[test]
fn corrupt_settings_fall_back_to_defaults_with_a_warning() {
    let dir = tempfile::tempdir().expect("temp dir");
    let settings_path = dir.path().join("settings.json");
    std::fs::write(&settings_path, "{not json").expect("write");
    let (platform, _probe) = FakePlatform::boxed(Vec::new(), false);
    let controller = Controller::new(settings_path, dir.path().join("myfax.db"), platform)
        .expect("start-up succeeds");
    assert_eq!(controller.state.settings, Settings::default());
    assert_eq!(controller.state.lang, Lang::ZhTw);
    let warned = controller
        .store
        .recent_logs(50)
        .expect("logs")
        .iter()
        .any(|entry| entry.message.contains("設定檔無法讀取，已改用預設值"));
    assert!(warned, "a warning line explains the fallback");
}

#[test]
fn controller_start_up_on_a_fresh_store_has_empty_lists() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (platform, _probe) = FakePlatform::boxed(Vec::new(), false);
    let controller = Controller::new(
        dir.path().join("app").join("settings.json"),
        dir.path().join("app").join("myfax.db"),
        platform,
    )
    .expect("start-up");
    assert_eq!(controller.state.counts.all, 0);
    assert!(controller.state.toast.is_none());
}

#[test]
fn browse_stores_the_picked_path_in_its_field() {
    let mut h = Harness::new();
    let picked = h.path("elsewhere");
    h.probe.picks.borrow_mut().folder = Some(picked.clone());
    h.controller.update(Msg::Browse(Field::ArchiveRoot));
    assert_eq!(
        h.controller.state.field(Field::ArchiveRoot),
        picked.display().to_string()
    );

    h.controller.update(Msg::Browse(Field::ImportPath));
    assert_eq!(h.probe.calls_with("pick_file:pdf,tif,tiff").len(), 1);

    h.controller.update(Msg::Browse(Field::CsvPath));
    assert_eq!(h.probe.calls_with("pick_file:csv").len(), 1);

    h.controller.update(Msg::Browse(Field::Search));
    assert_eq!(
        h.probe.calls.borrow().len(),
        3,
        "non-path fields open no picker"
    );
}

#[test]
fn navigation_clears_the_banner_only_on_inbox() {
    let mut h = Harness::new();
    h.controller.state.new_fax_banner = 3;
    h.controller.update(Msg::Navigate(Route::Settings));
    assert_eq!(h.controller.state.new_fax_banner, 3);
    h.controller.update(Msg::Navigate(Route::Inbox));
    assert_eq!(h.controller.state.new_fax_banner, 0);
}

#[test]
fn open_archive_folder_creates_the_missing_root_first() {
    let mut h = Harness::new();
    let root = h.controller.state.settings.archive_root.clone();
    assert!(!root.exists());
    h.controller.update(Msg::OpenArchiveFolder);
    assert!(root.is_dir());
    assert_eq!(h.probe.calls_with("open:").len(), 1);
}
