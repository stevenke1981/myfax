//! i18n fragment `settings` — rows are (key, zh-TW, en). Owner: see docs/team/myfax/BRIEF.md.

pub const ENTRIES: &[(&str, &str, &str)] = &[
    ("settings.title", "設定", "Settings"),
    (
        "settings.section_general",
        "外觀與語言",
        "Appearance & language",
    ),
    ("settings.language", "語言", "Language"),
    ("settings.theme", "主題", "Theme"),
    ("settings.theme_light", "淺色", "Light"),
    ("settings.theme_dark", "深色", "Dark"),
    ("settings.section_receive", "接收", "Receiving"),
    ("settings.watch_folders", "監看資料夾", "Watch folders"),
    (
        "settings.watch_empty",
        "尚未設定監看資料夾",
        "No watch folders yet",
    ),
    (
        "settings.watch_ph",
        "要新增的監看資料夾路徑",
        "Folder path to watch",
    ),
    (
        "settings.poll_interval",
        "掃描間隔（秒）",
        "Scan interval (seconds)",
    ),
    ("settings.poll_hint", "2 至 3600 秒", "2 to 3600 seconds"),
    (
        "settings.opt_delete_source",
        "封存後刪除來源檔案（移動而非複製）",
        "Delete the source file after archiving (move instead of copy)",
    ),
    (
        "settings.opt_accept_tiff",
        "同時接受 TIFF 傳真（.tif / .tiff）",
        "Also accept TIFF faxes (.tif / .tiff)",
    ),
    (
        "settings.opt_auto_print",
        "收到新傳真時自動列印",
        "Print new faxes automatically",
    ),
    (
        "settings.opt_notify",
        "收到新傳真時顯示通知橫幅",
        "Show a banner when a new fax arrives",
    ),
    (
        "settings.forward_folder",
        "額外複製到資料夾（選填）",
        "Also copy new faxes to (optional)",
    ),
    (
        "settings.forward_ph",
        "每份新傳真的複本會放到這裡",
        "A copy of every new fax is placed here",
    ),
    (
        "settings.section_storage",
        "封存與保留",
        "Archive & retention",
    ),
    ("settings.archive_root", "封存根目錄", "Archive folder"),
    (
        "settings.archive_hint",
        "依「年／年-月／年-月-日」資料夾整理",
        "Organized as year / year-month / year-month-day folders",
    ),
    (
        "settings.open_archive",
        "開啟封存資料夾",
        "Open archive folder",
    ),
    (
        "settings.trash_retention",
        "垃圾桶保留天數",
        "Keep trash for (days)",
    ),
    (
        "settings.trash_hint",
        "0 表示不自動清除",
        "0 = never purge automatically",
    ),
    ("settings.page_size", "每頁顯示筆數", "Faxes per page"),
    ("settings.page_hint", "10 至 500", "10 to 500"),
    ("settings.save_btn", "儲存設定", "Save settings"),
    ("settings.unsaved", "有尚未儲存的變更", "Unsaved changes"),
    (
        "settings.apply_note",
        "語言、主題與開關會立即生效；路徑與數字欄位需按「儲存設定」。",
        "Language, theme and switches apply immediately; path and number fields need Save settings.",
    ),
    (
        "settings.err_archive",
        "請輸入封存資料夾路徑",
        "Enter an archive folder path",
    ),
    (
        "settings.err_poll",
        "請輸入 2 至 3600 的整數",
        "Enter a whole number from 2 to 3600",
    ),
    (
        "settings.err_trash",
        "請輸入 0 至 3650 的整數",
        "Enter a whole number from 0 to 3650",
    ),
    (
        "settings.err_page",
        "請輸入 10 至 500 的整數",
        "Enter a whole number from 10 to 500",
    ),
    (
        "settings.help_title",
        "Epson 印表機設定說明",
        "Epson printer setup",
    ),
    (
        "settings.help_intro",
        "MyFax 透過印表機把收到的傳真轉存到網路資料夾來接收，請先在印表機上設定一次：",
        "MyFax receives faxes that your printer saves to a network folder. Set this up once on the printer:",
    ),
    (
        "settings.help_step1",
        "1. 在瀏覽器輸入印表機的 IP 位址開啟 Web Config（或使用印表機操作面板），進入傳真的「傳真輸出」設定。",
        "1. Open the printer's Web Config by entering its IP address in a browser (or use the control panel), then go to the Fax Output settings.",
    ),
    (
        "settings.help_step2",
        "2. 將「傳真輸出」設為「儲存到網路資料夾」（SMB），路徑填入這台電腦上的共用資料夾，例如 \\\\PC-NAME\\FaxInbox。",
        "2. Set Fax Output to Save to Network Folder (SMB) and enter a folder shared from this PC, for example \\\\PC-NAME\\FaxInbox.",
    ),
    (
        "settings.help_step3",
        "3. 檔案格式建議選 PDF；若選 TIFF，請開啟上方的「同時接受 TIFF 傳真」。",
        "3. Choose PDF as the file format (recommended). If you pick TIFF, turn on \"Also accept TIFF faxes\" above.",
    ),
    (
        "settings.help_step4",
        "4. 把同一個資料夾加入上方的「監看資料夾」，再按狀態列的「立即掃描」測試。",
        "4. Add the same folder to Watch folders above, then press Scan now in the status bar to test.",
    ),
    (
        "settings.help_note",
        "選單名稱依機型而異，請找「Fax Output」或「儲存到網路資料夾」。請勿讓 EPSON FAX Utility 與 MyFax 同時監看同一個資料夾。",
        "Menu names vary by model; look for Fax Output or Save to Network Folder. Do not let EPSON FAX Utility and MyFax watch the same folder.",
    ),
];
