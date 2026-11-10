//! Unit tests for the window shell (UI_SPEC §2). Child module of `shell`.

use super::test_util::{button_at, find_key, keys, messages, texts};
use super::*;
use crate::config::Settings;
use crate::i18n::{Lang, t};
use chrono::TimeZone;
use std::path::PathBuf;

fn state() -> AppState {
    AppState::new(Settings::default())
}

fn frame_of(state: &AppState) -> WidgetNode<Msg> {
    frame(state, kit::body("page-marker"))
}

fn with_confirm(action: ConfirmAction) -> AppState {
    let mut st = state();
    st.confirm = Some(ConfirmDialog {
        title: "確認".into(),
        body: "內容".into(),
        action,
    });
    st
}

#[test]
fn default_frame_has_shell_nav_and_status_bar_without_overlays() {
    let root = frame_of(&state());
    for key in [
        "root",
        "shell",
        "nav",
        "nav.inbox",
        "nav.settings",
        "statusbar",
        "btn.scan_now",
    ] {
        assert!(find_key(&root, key).is_some(), "missing {key}");
    }
    for key in [
        "banner",
        "toast_layer",
        "modal:scrim",
        "modal:dialog_layer",
        "btn.confirm_yes",
    ] {
        assert!(find_key(&root, key).is_none(), "unexpected {key}");
    }
    assert!(texts(&root).contains(&"page-marker".to_string()));
}

#[test]
fn layers_are_ordered_shell_then_toast_then_dialog() {
    let mut st = with_confirm(ConfirmAction::EmptyTrash);
    st.toast = Some(Toast {
        kind: ToastKind::Info,
        text: "note".into(),
    });
    let root = frame_of(&st);
    let order: Vec<String> = root
        .children()
        .iter()
        .filter_map(|c| c.key().map(|k| k.as_str().to_string()))
        .collect();
    assert_eq!(
        order,
        vec!["shell", "toast_layer", "modal:scrim", "modal:dialog_layer"]
    );
}

#[test]
fn nav_rail_emits_navigate_for_every_route() {
    let msgs = messages(&frame_of(&state()));
    for route in Route::ALL {
        assert!(msgs.contains(&Msg::Navigate(route)), "{route:?}");
    }
}

#[test]
fn every_route_can_be_selected_and_keeps_the_shell() {
    for route in Route::ALL {
        let mut st = state();
        st.route = route;
        let root = frame_of(&st);
        assert!(find_key(&root, "nav").is_some(), "{route:?}");
        assert!(texts(&root).contains(&"page-marker".to_string()));
    }
}

#[test]
fn unread_badge_shows_count_and_caps_at_99_plus() {
    let mut st = state();
    st.counts.unread = 7;
    assert!(texts(&frame_of(&st)).contains(&"7".to_string()));
    st.counts.unread = 150;
    assert!(texts(&frame_of(&st)).contains(&"99+".to_string()));
}

#[test]
fn unread_badge_hidden_when_zero() {
    let st = state();
    assert!(!texts(&frame_of(&st)).contains(&"0".to_string()));
}

#[test]
fn unread_label_caps_above_99() {
    assert_eq!(unread_label(0), "0");
    assert_eq!(unread_label(99), "99");
    assert_eq!(unread_label(100), "99+");
}

#[test]
fn banner_shows_only_with_new_faxes_and_offers_view_outside_inbox() {
    let mut st = state();
    assert!(find_key(&frame_of(&st), "banner").is_none());
    st.new_fax_banner = 3;
    let root = frame_of(&st);
    assert!(find_key(&root, "banner").is_some());
    assert!(messages(&root).contains(&Msg::DismissBanner));
    assert!(find_key(&root, "btn.banner_view").is_none());
    assert!(texts(&root).contains(&tf(Lang::ZhTw, "shell.banner_new_faxes", &[("count", "3")])));
    st.route = Route::Stats;
    let root = frame_of(&st);
    assert!(find_key(&root, "btn.banner_view").is_some());
}

#[test]
fn toast_is_dismissible_and_only_present_when_set() {
    let mut st = state();
    assert!(find_key(&frame_of(&st), "toast_layer").is_none());
    st.toast = Some(Toast {
        kind: ToastKind::Error,
        text: "Import failed".into(),
    });
    let root = frame_of(&st);
    assert!(find_key(&root, "toast_layer").is_some());
    assert!(messages(&root).contains(&Msg::DismissToast));
    assert!(texts(&root).contains(&"Import failed".to_string()));
}

#[test]
fn toast_tone_follows_kind() {
    assert_eq!(toast_look(ToastKind::Info).1, Tone::Info);
    assert_eq!(toast_look(ToastKind::Success).1, Tone::Success);
    assert_eq!(toast_look(ToastKind::Error).1, Tone::Danger);
}

#[test]
fn confirm_yes_and_no_exist_only_while_confirm_is_open() {
    assert!(find_key(&frame_of(&state()), "btn.confirm_yes").is_none());
    let root = frame_of(&with_confirm(ConfirmAction::DeletePermanently(7)));
    assert!(find_key(&root, "modal:scrim").is_some());
    assert!(find_key(&root, "modal:dialog").is_some());
    let msgs = messages(&root);
    assert!(msgs.contains(&Msg::ConfirmYes));
    assert!(msgs.contains(&Msg::ConfirmNo));
}

#[test]
fn confirm_button_label_follows_action() {
    let cases = [
        (ConfirmAction::DeletePermanently(1), "shell.confirm_delete"),
        (ConfirmAction::DeleteContact(1), "shell.confirm_delete"),
        (ConfirmAction::EmptyTrash, "shell.confirm_empty"),
        (ConfirmAction::ClearLog, "shell.confirm_clear"),
    ];
    for (action, key) in cases {
        let root = frame_of(&with_confirm(action));
        let yes = button_at(&root, "btn.confirm_yes").expect("confirm_yes");
        assert_eq!(yes.label, t(Lang::ZhTw, key));
    }
}

#[test]
fn confirm_cancel_is_first_button_in_tree_order() {
    let root = frame_of(&with_confirm(ConfirmAction::ClearLog));
    let order: Vec<String> = keys(&root)
        .into_iter()
        .filter(|k| k.starts_with("btn.confirm"))
        .collect();
    assert_eq!(order, vec!["btn.confirm_no", "btn.confirm_yes"]);
}

#[test]
fn status_bar_reports_watcher_states() {
    let mut st = state();
    st.watcher = WatcherStatus::Running(2);
    let count = tf(Lang::ZhTw, "shell.status_watching", &[("count", "2")]);
    assert!(texts(&frame_of(&st)).contains(&count));

    st.watcher = WatcherStatus::Stopped;
    assert!(texts(&frame_of(&st)).contains(&t(Lang::ZhTw, "shell.status_no_folder").to_string()));

    st.settings.watch_folders = vec![PathBuf::from("C:/watch/a")];
    assert!(texts(&frame_of(&st)).contains(&t(Lang::ZhTw, "shell.status_stopped").to_string()));

    st.watcher = WatcherStatus::Error("disk gone".into());
    let err = tf(Lang::ZhTw, "shell.status_error", &[("error", "disk gone")]);
    assert!(texts(&frame_of(&st)).contains(&err));
}

#[test]
fn last_scan_shows_time_only_for_today() {
    let now = Local.with_ymd_and_hms(2026, 11, 10, 12, 0, 0).unwrap();
    let today = Local.with_ymd_and_hms(2026, 11, 10, 9, 5, 7).unwrap();
    let earlier = Local.with_ymd_and_hms(2026, 11, 9, 23, 0, 1).unwrap();
    assert_eq!(
        last_scan_text(Lang::ZhTw, Some(today), now),
        tf(Lang::ZhTw, "shell.last_scan", &[("time", "09:05:07")])
    );
    assert_eq!(
        last_scan_text(Lang::ZhTw, Some(earlier), now),
        tf(
            Lang::ZhTw,
            "shell.last_scan",
            &[("time", "2026-11-09 23:00:01")]
        )
    );
    assert_eq!(
        last_scan_text(Lang::En, None, now),
        t(Lang::En, "shell.no_scan")
    );
}

#[test]
fn archive_path_is_truncated_in_status_bar() {
    let mut st = state();
    st.settings.archive_root =
        PathBuf::from(format!("C:/{}/archive", "very-long-segment/".repeat(8)));
    let labels = texts(&frame_of(&st));
    assert!(
        labels
            .iter()
            .any(|s| s.contains('…') && s.starts_with("封存位置："))
    );
}

#[test]
fn reworded_fragments_avoid_placeholder_plurals_and_mainland_nouns() {
    // Keys whose wording was reviewed (fragments are private to `i18n`).
    const REVIEWED: [&str; 10] = [
        "shell.window_title_unread",
        "shell.status_watching",
        "shell.banner_new_faxes",
        "shell.nav_log",
        "common.pages_short",
        "log.title",
        "log.clear",
        "log.empty",
        "send.history_title",
        "settings.archive_hint",
    ];
    for key in REVIEWED {
        let zh = t(Lang::ZhTw, key);
        let en = t(Lang::En, key);
        for banned in ["(s)", "(es)", " pg", "Organised"] {
            assert!(
                !en.contains(banned),
                "{key}: English contains {banned:?}: {en}"
            );
        }
        for banned in ["活動記錄", "傳送記錄", "清除記錄", "封未讀"] {
            assert!(
                !zh.contains(banned),
                "{key}: zh-TW contains {banned:?}: {zh}"
            );
        }
    }
    assert!(t(Lang::ZhTw, "shell.window_title_unread").contains("份未讀"));
    assert_eq!(t(Lang::En, "send.history_title"), "Sent history");
}
