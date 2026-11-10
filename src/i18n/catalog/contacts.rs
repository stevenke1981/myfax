//! i18n fragment `contacts` — rows are (key, zh-TW, en). Owner: see docs/team/myfax/BRIEF.md.

pub const ENTRIES: &[(&str, &str, &str)] = &[
    ("contacts.title", "聯絡人", "Contacts"),
    ("contacts.count", "共 {count} 位聯絡人", "Contacts: {count}"),
    (
        "contacts.search_ph",
        "搜尋姓名、公司、號碼",
        "Search name, company, number",
    ),
    ("contacts.new", "新增聯絡人", "New contact"),
    ("contacts.empty_title", "還沒有聯絡人", "No contacts yet"),
    (
        "contacts.empty_hint",
        "新增聯絡人，或從 CSV 檔匯入。",
        "Add a contact or import from a CSV file.",
    ),
    (
        "contacts.no_match",
        "找不到符合的聯絡人",
        "No matching contacts",
    ),
    ("contacts.send_to", "傳送", "Send"),
    ("contacts.editor_new_title", "新增聯絡人", "New contact"),
    ("contacts.editor_edit_title", "編輯聯絡人", "Edit contact"),
    (
        "contacts.editor_empty",
        "選擇一位聯絡人來編輯，或新增一位。",
        "Select a contact to edit, or create a new one.",
    ),
    ("contacts.field_name", "姓名", "Name"),
    ("contacts.field_company", "公司", "Company"),
    ("contacts.field_fax", "傳真號碼", "Fax number"),
    ("contacts.field_phone", "電話", "Phone"),
    ("contacts.field_email", "電子郵件", "Email"),
    ("contacts.field_note", "備註", "Note"),
    (
        "contacts.fax_hint",
        "只會保留數字與開頭的 +。",
        "Only digits and a leading + are kept.",
    ),
    (
        "contacts.csv_section",
        "匯入／匯出 CSV",
        "Import / export CSV",
    ),
    ("contacts.csv_ph", "CSV 檔案路徑", "CSV file path"),
    ("contacts.csv_import", "匯入 CSV", "Import CSV"),
    ("contacts.csv_export", "匯出 CSV", "Export CSV"),
    (
        "contacts.csv_format",
        "欄位順序：name,company,fax,phone,email,note（UTF-8 編碼）",
        "Column order: name,company,fax,phone,email,note (UTF-8)",
    ),
];
