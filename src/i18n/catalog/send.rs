//! i18n fragment `send` — rows are (key, zh-TW, en). Owner: see docs/team/myfax/BRIEF.md.

pub const ENTRIES: &[(&str, &str, &str)] = &[
    ("send.title", "傳送傳真", "Send a fax"),
    ("send.explain_title", "運作方式", "How it works"),
    (
        "send.explain_body",
        "MyFax 會把文件交給印表機的傳真驅動程式（例如 EPSON FAX）。驅動程式會自己跳出視窗，請在其中輸入傳真號碼並送出。MyFax 無法代為撥號，也無法確認對方是否收到。",
        "MyFax hands your document to the printer's fax driver (for example EPSON FAX). The driver opens its own window where you enter the fax number and send. MyFax cannot dial for you or confirm delivery.",
    ),
    ("send.section_printer", "1. 傳真印表機", "1. Fax printer"),
    ("send.section_file", "2. 要傳送的檔案", "2. File to send"),
    (
        "send.section_recipient",
        "3. 收件人（僅供記錄）",
        "3. Recipient (for your records)",
    ),
    ("send.printer_fax_badge", "傳真", "Fax"),
    (
        "send.printers_empty",
        "找不到印表機。請確認已安裝 EPSON FAX 驅動程式，再按「重新整理」。",
        "No printers found. Make sure the EPSON FAX driver is installed, then press Refresh.",
    ),
    (
        "send.file_ph",
        "要傳送的檔案路徑（PDF、圖片、文件…）",
        "File to send (PDF, image, document...)",
    ),
    (
        "send.recipient_ph",
        "收件人名稱或傳真號碼（選填）",
        "Recipient name or fax number (optional)",
    ),
    ("send.pick_contact", "從聯絡人選擇", "Pick from contacts"),
    (
        "send.contacts_empty",
        "還沒有聯絡人，可到「聯絡人」新增。",
        "No contacts yet. Add some on the Contacts page.",
    ),
    ("send.note_ph", "備註（選填）", "Note (optional)"),
    ("send.button", "傳送傳真", "Send fax"),
    (
        "send.need_printer",
        "請先選擇印表機",
        "Choose a printer first",
    ),
    (
        "send.need_file",
        "請先指定要傳送的檔案",
        "Choose a file to send first",
    ),
    ("send.history_title", "傳送記錄", "Sent history"),
    ("send.history_empty", "還沒有傳送記錄", "Nothing sent yet"),
    ("send.history_via", "經由 {printer}", "via {printer}"),
    ("send.history_to", "收件人：{recipient}", "To: {recipient}"),
];
