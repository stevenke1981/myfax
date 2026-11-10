//! i18n fragment `app` — rows are (key, zh-TW, en). Owner: controller worker
//! (texts from UI_SPEC §11.10; see the controller report for keys added or renamed).

pub const ENTRIES: &[(&str, &str, &str)] = &[
    // ── toasts ──
    (
        "app.import_summary",
        "匯入 {imported} 份、重複 {duplicates} 份、略過 {skipped} 份、失敗 {failed} 份",
        "Imported {imported}, duplicates {duplicates}, skipped {skipped}, failed {failed}",
    ),
    (
        "app.import_no_path",
        "請先輸入要匯入的檔案或資料夾路徑",
        "Enter the file or folder to import first",
    ),
    (
        "app.export_done",
        "已匯出 {count} 份傳真到 {folder}",
        "Exported {count} fax(es) to {folder}",
    ),
    (
        "app.export_none",
        "目前篩選結果沒有可匯出的傳真",
        "No faxes to export for the current filter",
    ),
    (
        "app.export_no_folder",
        "請先指定匯出資料夾",
        "Choose an export folder first",
    ),
    (
        "app.export_error",
        "匯出失敗：{error}",
        "Export failed: {error}",
    ),
    (
        "app.print_sent",
        "已送到印表機：{name}",
        "Sent to the printer: {name}",
    ),
    (
        "app.print_error",
        "列印失敗：{error}",
        "Printing failed: {error}",
    ),
    (
        "app.open_failed",
        "無法開啟檔案：{error}",
        "Could not open the file: {error}",
    ),
    (
        "app.reveal_failed",
        "無法開啟資料夾：{error}",
        "Could not open the folder: {error}",
    ),
    (
        "app.file_missing",
        "找不到封存檔案：{path}",
        "Archived file not found: {path}",
    ),
    ("app.edits_saved", "已儲存變更", "Changes saved"),
    ("app.moved_to_trash", "已移到垃圾桶", "Moved to trash"),
    ("app.restored", "已還原", "Restored"),
    ("app.deleted_forever", "已永久刪除", "Deleted permanently"),
    (
        "app.trash_emptied",
        "已清空垃圾桶（{count} 份）",
        "Trash emptied ({count})",
    ),
    (
        "app.marked_all_read",
        "已將 {count} 份傳真標為已讀",
        "Marked {count} fax(es) as read",
    ),
    (
        "app.scan_done",
        "掃描完成：{count} 份新傳真",
        "Scan finished: {count} new fax(es)",
    ),
    ("app.settings_saved", "設定已儲存", "Settings saved"),
    (
        "app.settings_invalid",
        "設定未儲存：{reason}",
        "Settings not saved: {reason}",
    ),
    (
        "app.settings_invalid_archive",
        "封存位置不可為空",
        "The archive location can't be empty",
    ),
    (
        "app.settings_invalid_poll",
        "輪詢間隔必須是 2 到 3600 之間的整數（秒）",
        "The poll interval must be a whole number from 2 to 3600 seconds",
    ),
    (
        "app.settings_invalid_trash",
        "垃圾桶保留天數必須是 0 到 3650 之間的整數",
        "Trash retention must be a whole number of days from 0 to 3650",
    ),
    (
        "app.settings_invalid_page",
        "每頁筆數必須是 10 到 500 之間的整數",
        "Items per page must be a whole number from 10 to 500",
    ),
    (
        "app.settings_save_failed",
        "無法儲存設定：{error}",
        "Could not save settings: {error}",
    ),
    (
        "app.settings_load_failed",
        "設定檔無法讀取，已改用預設值：{error}",
        "The settings file could not be read; using defaults: {error}",
    ),
    (
        "app.watch_empty",
        "請先輸入資料夾路徑",
        "Enter a folder path first",
    ),
    (
        "app.watch_missing",
        "找不到資料夾：{path}",
        "Folder not found: {path}",
    ),
    (
        "app.watch_duplicate",
        "這個資料夾已在清單中",
        "This folder is already in the list",
    ),
    ("app.contact_saved", "已儲存聯絡人", "Contact saved"),
    ("app.contact_deleted", "已刪除聯絡人", "Contact deleted"),
    ("app.contact_invalid_name", "請輸入姓名", "Enter a name"),
    (
        "app.contact_invalid_fax",
        "請輸入有效的傳真號碼",
        "Enter a valid fax number",
    ),
    (
        "app.contact_exists",
        "這個號碼已在聯絡人中",
        "This number is already in your contacts",
    ),
    (
        "app.no_sender_number",
        "這份傳真沒有寄件者號碼，無法加入聯絡人",
        "This fax has no sender number, so it can't be added to contacts",
    ),
    (
        "app.csv_imported",
        "已匯入 {count} 位聯絡人",
        "Imported {count} contact(s)",
    ),
    (
        "app.csv_exported",
        "已匯出 {count} 位聯絡人到 {path}",
        "Exported {count} contact(s) to {path}",
    ),
    (
        "app.csv_no_path",
        "請先指定 CSV 檔案路徑",
        "Choose a CSV file path first",
    ),
    (
        "app.csv_error",
        "CSV 處理失敗：{error}",
        "CSV error: {error}",
    ),
    (
        "app.date_invalid",
        "日期格式不正確，請輸入 YYYY-MM-DD",
        "Invalid date. Use YYYY-MM-DD.",
    ),
    (
        "app.date_order",
        "起始日不能晚於結束日",
        "The start date can't be after the end date",
    ),
    (
        "app.send_handed",
        "已交給傳真驅動程式「{printer}」，請在跳出的視窗輸入傳真號碼並送出。",
        "Handed to the fax driver \"{printer}\". Enter the fax number in the window that opens and send.",
    ),
    (
        "app.send_no_printer",
        "找不到可用的印表機，請先安裝傳真驅動程式",
        "No printer available. Install the fax driver first.",
    ),
    (
        "app.send_no_file",
        "請先選擇要傳送的檔案",
        "Choose a file to send first",
    ),
    (
        "app.send_file_missing",
        "找不到要傳送的檔案：{path}",
        "File to send not found: {path}",
    ),
    (
        "app.send_error",
        "無法交給傳真驅動程式：{error}",
        "Could not hand the file to the fax driver: {error}",
    ),
    (
        "app.printers_refreshed",
        "找到 {count} 台印表機",
        "Found {count} printer(s)",
    ),
    (
        "app.printers_error",
        "無法讀取印表機清單：{error}",
        "Could not read the printer list: {error}",
    ),
    (
        "app.store_error",
        "資料庫錯誤：{error}",
        "Database error: {error}",
    ),
    (
        "app.file_delete_failed",
        "無法刪除封存檔案：{error}",
        "Could not delete the archived file: {error}",
    ),
    ("app.unknown_sender", "未知寄件者", "Unknown sender"),
    // ── preview and watcher status ──
    (
        "app.preview_unavailable",
        "無法預覽這份傳真",
        "This fax can't be previewed",
    ),
    (
        "app.preview_encrypted",
        "這個 PDF 已加密，無法預覽",
        "This PDF is encrypted and can't be previewed",
    ),
    (
        "app.preview_failed",
        "預覽失敗：{error}",
        "Preview failed: {error}",
    ),
    (
        "app.preview_file_missing",
        "找不到封存檔案，無法預覽",
        "The archived file is missing, can't preview",
    ),
    (
        "app.watch_error_start",
        "無法啟動資料夾監看：{error}",
        "Cannot start the folder watcher: {error}",
    ),
    // ── confirm dialogs ──
    (
        "app.confirm_delete_title",
        "永久刪除這份傳真？",
        "Delete this fax permanently?",
    ),
    (
        "app.confirm_delete_body",
        "「{name}」及其封存檔案將被永久刪除，無法復原。",
        "\"{name}\" and its archived file will be deleted permanently. This cannot be undone.",
    ),
    (
        "app.confirm_empty_title",
        "清空垃圾桶？",
        "Empty the trash?",
    ),
    (
        "app.confirm_empty_body",
        "垃圾桶中的 {count} 份傳真將被永久刪除，無法復原。",
        "The {count} fax(es) in the trash will be deleted permanently. This cannot be undone.",
    ),
    (
        "app.confirm_contact_title",
        "刪除這位聯絡人？",
        "Delete this contact?",
    ),
    (
        "app.confirm_contact_body",
        "「{name}」將從聯絡人中刪除。",
        "\"{name}\" will be removed from your contacts.",
    ),
    (
        "app.confirm_log_title",
        "清除活動記錄？",
        "Clear the activity log?",
    ),
    (
        "app.confirm_log_body",
        "所有活動記錄都會被刪除，無法復原。",
        "All log entries will be deleted. This cannot be undone.",
    ),
    // ── activity log lines ──
    ("app.log_started", "MyFax 已啟動", "MyFax started"),
    (
        "app.log_import_duplicate",
        "略過重複的傳真：{name}",
        "Skipped duplicate fax: {name}",
    ),
    (
        "app.log_import_error",
        "匯入失敗：{name}（{error}）",
        "Import failed: {name} ({error})",
    ),
    (
        "app.log_exported",
        "已匯出 {count} 份傳真到 {folder}",
        "Exported {count} fax(es) to {folder}",
    ),
    ("app.log_printed", "已列印：{name}", "Printed: {name}"),
    (
        "app.log_trashed",
        "已移到垃圾桶：{name}",
        "Moved to trash: {name}",
    ),
    ("app.log_restored", "已還原：{name}", "Restored: {name}"),
    (
        "app.log_deleted",
        "已永久刪除：{name}",
        "Deleted permanently: {name}",
    ),
    (
        "app.log_trash_emptied",
        "已清空垃圾桶（{count} 份）",
        "Emptied the trash ({count})",
    ),
    (
        "app.log_trash_purged",
        "已自動清除垃圾桶中逾期的 {count} 份傳真",
        "Purged {count} expired fax(es) from the trash",
    ),
    ("app.log_settings_saved", "已儲存設定", "Settings saved"),
    (
        "app.log_watch_added",
        "已新增監看資料夾：{path}",
        "Added watch folder: {path}",
    ),
    (
        "app.log_watch_removed",
        "已移除監看資料夾：{path}",
        "Removed watch folder: {path}",
    ),
    (
        "app.log_watcher_started",
        "開始監看 {count} 個資料夾",
        "Watching {count} folder(s)",
    ),
    (
        "app.log_watcher_error",
        "資料夾監看錯誤：{error}",
        "Folder watcher error: {error}",
    ),
    (
        "app.log_scan_manual",
        "手動掃描完成，發現 {count} 份新傳真",
        "Manual scan finished: {count} new fax(es)",
    ),
    (
        "app.log_contact_saved",
        "已儲存聯絡人：{name}",
        "Saved contact: {name}",
    ),
    (
        "app.log_contact_deleted",
        "已刪除聯絡人：{name}",
        "Deleted contact: {name}",
    ),
    (
        "app.log_csv_imported",
        "已從 CSV 匯入 {count} 位聯絡人",
        "Imported {count} contact(s) from CSV",
    ),
    (
        "app.log_csv_exported",
        "已匯出 {count} 位聯絡人到 CSV",
        "Exported {count} contact(s) to CSV",
    ),
    (
        "app.log_send_handed",
        "已交給傳真驅動程式：{file} → {printer}",
        "Handed to fax driver: {file} → {printer}",
    ),
];
