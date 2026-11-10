//! i18n fragment `shell` — rows are (key, zh-TW, en). Owner: see docs/team/myfax/BRIEF.md.

pub const ENTRIES: &[(&str, &str, &str)] = &[
    ("shell.window_title", "MyFax 傳真中心", "MyFax Fax Center"),
    (
        "shell.window_title_unread",
        "MyFax 傳真中心（{count} 封未讀）",
        "MyFax Fax Center ({count} unread)",
    ),
    ("shell.app_name", "MyFax", "MyFax"),
    ("shell.app_tagline", "傳真收件匣", "Fax inbox"),
    ("shell.nav_inbox", "收件匣", "Inbox"),
    ("shell.nav_contacts", "聯絡人", "Contacts"),
    ("shell.nav_send", "傳送傳真", "Send fax"),
    ("shell.nav_stats", "統計", "Statistics"),
    ("shell.nav_log", "活動記錄", "Activity log"),
    ("shell.nav_settings", "設定", "Settings"),
    ("shell.watch_running_short", "監看中", "Watching"),
    ("shell.watch_stopped_short", "已停止", "Stopped"),
    ("shell.watch_error_short", "發生錯誤", "Error"),
    (
        "shell.status_watching",
        "監看中：{count} 個資料夾",
        "Watching {count} folder(s)",
    ),
    (
        "shell.status_stopped",
        "資料夾監看已停止",
        "Folder watcher stopped",
    ),
    (
        "shell.status_no_folder",
        "尚未設定監看資料夾",
        "No watch folder configured",
    ),
    (
        "shell.status_error",
        "監看發生錯誤：{error}",
        "Watcher error: {error}",
    ),
    ("shell.last_scan", "上次掃描：{time}", "Last scan: {time}"),
    ("shell.no_scan", "尚未掃描", "Not scanned yet"),
    ("shell.archive_path", "封存位置：{path}", "Archive: {path}"),
    ("shell.scan_now", "立即掃描", "Scan now"),
    (
        "shell.banner_new_faxes",
        "收到 {count} 份新傳真",
        "{count} new fax(es) received",
    ),
    ("shell.banner_view", "檢視收件匣", "View inbox"),
    ("shell.confirm_delete", "刪除", "Delete"),
    ("shell.confirm_empty", "清空垃圾桶", "Empty trash"),
    ("shell.confirm_clear", "清除", "Clear"),
];
