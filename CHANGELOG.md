# Changelog

All notable changes to MyFax are recorded in this file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.0] - 2026-11-11

### Added

- Watched folders with polling. A file is imported only after its size and modification time are
  stable across two polls. Failed files are retried up to three times.
- Date-organised archive: `<archive root>\YYYY\YYYY-MM\YYYY-MM-DD\YYYYMMDD-HHMMSS_<sender>.pdf`.
  Faxes without a sender number use `unknown`.
- Duplicate detection by SHA-256. Identical content is archived once.
- Filename parsing for receive date, time and sender number, with a fallback to the file's
  modification time.
- Support for PDF and, optionally, TIFF (`.tif`, `.tiff`) faxes.
- Inbox with folders All, Unread, Starred and Trash; date presets, a year / month / day date tree,
  tag filter, sorting by newest, oldest or sender, and paging.
- Substring search over sender name, sender number, note, file name and tags.
- Fax details: editable sender name, tags and note; mark read or unread; star; move to trash;
  restore; permanent delete with confirmation; open; print; show in folder; add sender to contacts.
- Page preview for PDF and TIFF with zoom (50% to 300%) and page navigation. Unsupported files can be
  opened with the default app.
- Export of the faxes matching the current filter (all pages) to a folder.
- Import by typed path, or by dragging files and folders onto the window.
- Contacts: add, edit, search and delete; sender names resolved from contacts by fax number.
- Contact CSV import and export (`name,company,fax,phone,email,note`), UTF-8 with BOM on export, and
  a spreadsheet formula guard.
- Send fax through an installed fax printer driver, with send history. MyFax hands the document to
  the driver; it does not dial or confirm delivery.
- Statistics for the last 7 days, 30 days or 1 year: totals, averages, busiest day, daily and monthly
  charts, and top senders.
- Activity log with Info, Warning and Error levels and a confirmed clear action.
- Settings: language (Traditional Chinese, Taiwan, by default; English), light and dark theme, watch
  folders, scan interval, archive folder, trash retention, page size, delete-source option, TIFF
  option, auto-print, new-fax banner, and an extra copy folder.
- Auto-print of faxes received through a watched folder, to the default printer.
- Trash retention: expired trash is purged at start-up (default 30 days; 0 disables purging).
- SQLite storage in WAL mode with schema migrations tracked by `PRAGMA user_version`.
- Data stored under `%APPDATA%\MyFax`, overridable with `MYFAX_DATA_DIR`.
- CI on `windows-latest`: format check, clippy with warnings denied, tests, doc tests, and a release
  build uploaded as the `myfax-windows-x64` artifact.
