//! i18n fragment `log` — rows are (key, zh-TW, en). Owner: see docs/team/myfax/BRIEF.md.

pub const ENTRIES: &[(&str, &str, &str)] = &[
    ("log.title", "活動記錄", "Activity log"),
    ("log.count", "共 {count} 筆", "Entries: {count}"),
    ("log.clear", "清除記錄", "Clear log"),
    ("log.empty", "還沒有活動記錄", "No log entries yet"),
    ("log.level_info", "資訊", "Info"),
    ("log.level_warn", "警告", "Warning"),
    ("log.level_error", "錯誤", "Error"),
    (
        "log.truncated",
        "僅顯示最新 {count} 筆",
        "Showing the latest {count}",
    ),
];
