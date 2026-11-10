//! UI → controller messages and text-field identities (orchestrator-owned contract).

use crate::config::ThemeChoice;
use crate::i18n::Lang;
use crate::model::{ContactId, FaxId, Folder, SortOrder};
use chrono::NaiveDate;

/// Top-level page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Route {
    #[default]
    Inbox,
    Contacts,
    Send,
    Stats,
    Log,
    Settings,
}

impl Route {
    pub const ALL: [Route; 6] = [
        Route::Inbox,
        Route::Contacts,
        Route::Send,
        Route::Stats,
        Route::Log,
        Route::Settings,
    ];
}

/// Quick date filter for the inbox.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DatePreset {
    #[default]
    AnyTime,
    Today,
    Yesterday,
    Last7Days,
    Last30Days,
    ThisMonth,
    ThisYear,
    /// Range typed into `Field::DateFrom` / `Field::DateTo`, or picked in the date tree.
    Custom,
}

impl DatePreset {
    /// Presets shown as chips (Custom is implicit).
    pub const CHIPS: [DatePreset; 7] = [
        DatePreset::AnyTime,
        DatePreset::Today,
        DatePreset::Yesterday,
        DatePreset::Last7Days,
        DatePreset::Last30Days,
        DatePreset::ThisMonth,
        DatePreset::ThisYear,
    ];
}

/// Range for the statistics page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StatsRange {
    Last7Days,
    #[default]
    Last30Days,
    Last365Days,
}

/// Boolean settings toggled directly from the settings page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BoolSetting {
    DeleteSourceAfterImport,
    AcceptTiff,
    AutoPrint,
    NotifyNewFax,
}

/// Every editable text field. The `TextInput` widget key MUST be `field.key()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Field {
    Search,
    DateFrom,
    DateTo,
    ImportPath,
    ExportFolder,
    FaxSenderName,
    FaxTags,
    FaxNote,
    ContactSearch,
    ContactName,
    ContactCompany,
    ContactFax,
    ContactPhone,
    ContactEmail,
    ContactNote,
    CsvPath,
    SendFile,
    SendRecipient,
    SendNote,
    WatchFolderInput,
    ArchiveRoot,
    PollInterval,
    TrashRetention,
    PageSize,
    ForwardFolder,
}

impl Field {
    pub const ALL: [Field; 25] = [
        Field::Search,
        Field::DateFrom,
        Field::DateTo,
        Field::ImportPath,
        Field::ExportFolder,
        Field::FaxSenderName,
        Field::FaxTags,
        Field::FaxNote,
        Field::ContactSearch,
        Field::ContactName,
        Field::ContactCompany,
        Field::ContactFax,
        Field::ContactPhone,
        Field::ContactEmail,
        Field::ContactNote,
        Field::CsvPath,
        Field::SendFile,
        Field::SendRecipient,
        Field::SendNote,
        Field::WatchFolderInput,
        Field::ArchiveRoot,
        Field::PollInterval,
        Field::TrashRetention,
        Field::PageSize,
        Field::ForwardFolder,
    ];

    /// Stable widget key, `field.<snake_name>`.
    pub fn key(self) -> &'static str {
        match self {
            Field::Search => "field.search",
            Field::DateFrom => "field.date_from",
            Field::DateTo => "field.date_to",
            Field::ImportPath => "field.import_path",
            Field::ExportFolder => "field.export_folder",
            Field::FaxSenderName => "field.fax_sender_name",
            Field::FaxTags => "field.fax_tags",
            Field::FaxNote => "field.fax_note",
            Field::ContactSearch => "field.contact_search",
            Field::ContactName => "field.contact_name",
            Field::ContactCompany => "field.contact_company",
            Field::ContactFax => "field.contact_fax",
            Field::ContactPhone => "field.contact_phone",
            Field::ContactEmail => "field.contact_email",
            Field::ContactNote => "field.contact_note",
            Field::CsvPath => "field.csv_path",
            Field::SendFile => "field.send_file",
            Field::SendRecipient => "field.send_recipient",
            Field::SendNote => "field.send_note",
            Field::WatchFolderInput => "field.watch_folder_input",
            Field::ArchiveRoot => "field.archive_root",
            Field::PollInterval => "field.poll_interval",
            Field::TrashRetention => "field.trash_retention",
            Field::PageSize => "field.page_size",
            Field::ForwardFolder => "field.forward_folder",
        }
    }

    pub fn from_key(key: &str) -> Option<Field> {
        Field::ALL.into_iter().find(|f| f.key() == key)
    }

    /// Fields that hold a filesystem path and offer a "Browse…" picker.
    pub fn is_path(self) -> bool {
        matches!(
            self,
            Field::ImportPath
                | Field::ExportFolder
                | Field::CsvPath
                | Field::SendFile
                | Field::WatchFolderInput
                | Field::ArchiveRoot
                | Field::ForwardFolder
        )
    }

    /// Path fields that expect a folder (others expect a file).
    pub fn picks_folder(self) -> bool {
        matches!(
            self,
            Field::ExportFolder
                | Field::WatchFolderInput
                | Field::ArchiveRoot
                | Field::ForwardFolder
        )
    }
}

/// Action awaiting confirmation in the modal dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfirmAction {
    DeletePermanently(FaxId),
    EmptyTrash,
    DeleteContact(ContactId),
    ClearLog,
}

/// Every user intent. Views only construct these; the controller handles them.
#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    // ── navigation ──
    Navigate(Route),
    SelectFolder(Folder),
    SelectFax(FaxId),
    CloseDetail,

    // ── inbox filters ──
    SetDatePreset(DatePreset),
    /// Apply `Field::DateFrom`..`Field::DateTo` (YYYY-MM-DD) as a custom range.
    ApplyCustomRange,
    ToggleYear(i32),
    SelectYear(i32),
    SelectMonth {
        year: i32,
        month: u32,
    },
    SelectDay(NaiveDate),
    SetTagFilter(Option<String>),
    SetSort(SortOrder),
    ClearFilters,
    PrevPage,
    NextPage,

    // ── fax actions ──
    ToggleRead(FaxId),
    ToggleStar(FaxId),
    MarkAllRead,
    OpenExternal(FaxId),
    Print(FaxId),
    RevealInFolder(FaxId),
    MoveToTrash(FaxId),
    Restore(FaxId),
    /// Opens the confirm dialog.
    AskDeletePermanently(FaxId),
    /// Opens the confirm dialog.
    AskEmptyTrash,
    /// Save `Field::FaxSenderName`, `Field::FaxTags` (comma separated), `Field::FaxNote`.
    SaveFaxEdits(FaxId),
    AddSenderToContacts(FaxId),
    /// Copy every fax matching the current filter (all pages) to `Field::ExportFolder`.
    ExportFiltered,
    PreviewPrevPage,
    PreviewNextPage,
    ZoomIn,
    ZoomOut,
    ZoomReset,

    // ── import / receive ──
    /// Import the file or folder typed in `Field::ImportPath`.
    ImportFromField,
    /// Poll all watch folders immediately.
    ScanNow,

    // ── contacts ──
    NewContact,
    EditContact(ContactId),
    SaveContact,
    CancelContactEdit,
    AskDeleteContact(ContactId),
    /// Import contacts from the CSV in `Field::CsvPath`.
    ImportContactsCsv,
    /// Export contacts to the CSV path in `Field::CsvPath`.
    ExportContactsCsv,

    // ── send (via printer fax driver) ──
    RefreshPrinters,
    SelectPrinter(String),
    UseContactForSend(ContactId),
    SendFax,

    // ── statistics / log ──
    SetStatsRange(StatsRange),
    AskClearLog,

    // ── settings ──
    SetLanguage(Lang),
    SetTheme(ThemeChoice),
    AddWatchFolder,
    RemoveWatchFolder(usize),
    ToggleSetting(BoolSetting),
    /// Validate and persist the text-field settings (archive root, interval, …).
    SaveSettings,
    OpenArchiveFolder,

    // ── generic ──
    /// Open a native picker for a path field and store the result in it.
    Browse(Field),
    ConfirmYes,
    ConfirmNo,
    DismissToast,
    DismissBanner,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn field_keys_are_unique_and_roundtrip() {
        let mut seen = HashSet::new();
        for field in Field::ALL {
            assert!(seen.insert(field.key()));
            assert_eq!(Field::from_key(field.key()), Some(field));
        }
        assert_eq!(Field::from_key("nope"), None);
    }
}
