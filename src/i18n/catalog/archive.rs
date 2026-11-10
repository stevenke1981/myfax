//! i18n fragment `archive` — rows are (key, zh-TW, en). Owner: see docs/team/myfax/BRIEF.md.

pub const ENTRIES: &[(&str, &str, &str)] = &[
    (
        "archive.log_imported",
        "收到傳真：{sender}（{name}）",
        "Fax received from {sender} ({name})",
    ),
    (
        "archive.log_forward_failed",
        "轉寄副本失敗（{name}）：{error}",
        "Forward copy failed ({name}): {error}",
    ),
    (
        "archive.log_delete_failed",
        "無法刪除來源檔（{name}）：{error}",
        "Could not delete the source file ({name}): {error}",
    ),
    (
        "archive.log_pages_unknown",
        "無法讀取頁數，已以 0 頁歸檔（{name}）",
        "Page count unreadable; archived as 0 pages ({name})",
    ),
    ("archive.unknown_sender", "未知寄件者", "Unknown sender"),
    (
        "archive.err_read",
        "無法讀取來源檔：{path}",
        "Cannot read the source file: {path}",
    ),
    (
        "archive.err_mkdir",
        "無法建立歸檔資料夾：{path}",
        "Cannot create the archive folder: {path}",
    ),
    (
        "archive.err_copy",
        "無法複製到歸檔位置：{path}",
        "Cannot copy into the archive: {path}",
    ),
    (
        "archive.err_size",
        "歸檔檔案大小與來源不符：{path}",
        "Archived file size does not match the source: {path}",
    ),
    (
        "archive.err_index",
        "無法寫入索引：{path}",
        "Cannot write the index entry: {path}",
    ),
    (
        "archive.ingest_failed",
        "匯入失敗：{error}",
        "Import failed: {error}",
    ),
    (
        "archive.watch_folder_unavailable",
        "監看資料夾無法使用：{path}（{error}）",
        "Watch folder unavailable: {path} ({error})",
    ),
    (
        "archive.store_open_failed",
        "無法開啟資料庫，監看已停止：{error}",
        "Cannot open the database; watching stopped: {error}",
    ),
];
