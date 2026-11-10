# MyFax architecture

This document is for developers. It describes the crate layout, the module responsibilities, the
data flow, the threading model, and the workarounds used against the pinned AcmeUI-Native revision.
User-facing material is in [README.md](../README.md) and [EPSON_SETUP.md](EPSON_SETUP.md).

## Crate layout

- One package, `myfax`, with a library target (`src/lib.rs`) and a binary target (`src/main.rs`).
  The binary is the AcmeUI-Native `Application` glue; everything else lives in the library.
- `#![forbid(unsafe_code)]` is set in both `lib.rs` and `main.rs`.
- Edition 2024, `rust-version = "1.88"`.
- Main dependencies: AcmeUI-Native crates (`acme-core`, `acme-layout`, `acme-platform`,
  `acme-render-wgpu`, `acme-text`, `acme-theme`, `acme-widgets`, `acme-style`, `acme-textinput`),
  `rusqlite` (bundled SQLite), `lopdf`, `image`, `fax` (CCITT decoder), `sha2`, `chrono`, `serde`,
  `serde_json`, `dirs`, `rfd` (native dialogs), `tracing`.
- AcmeUI-Native is a git dependency pinned to revision `1f23c910f05d7c975094df1e3b2be65ee2437ec9`.
  Do not move the pin without an explicit upgrade task.
- `examples/layout_dump.rs` and `examples/scroll_probe.rs` are developer probes for layout and
  scrolling. They have no doc comments.

## Module map

| Path | Responsibility |
|---|---|
| `src/main.rs` | Entry point. `MyFaxApp` implements `acme_platform::Application`: it forwards platform events to `UiRuntime`, dispatches UI events to `Controller`, renders `views::root`, sets the window title (unread count), and runs the background tick every 750 ms. |
| `src/lib.rs` | Module declarations: `app`, `archive`, `config`, `i18n`, `model`, `pdf`, `store`, `system`, `ui`. |
| `src/model.rs` | Domain types: `FaxRecord`, `NewFax`, `FaxSource` (`watch` / `manual`), `Folder`, `SortOrder`, `FaxQuery`, `SearchPage`, `Contact`, `SentFax`, `LogEntry`, `Stats`, `PageImage`. |
| `src/config.rs` | `Settings` (serde, `#[serde(default)]`), `sanitized()` clamping, data paths, atomic save and load. |
| `src/i18n/` | Translation lookup (`t`, `tf`) and the `Lang` enum (`ZhTw` default, `En`). `catalog/*.rs` holds one fragment per area. |
| `src/store/` | SQLite access: connection setup, migrations, faxes, tags, contacts, sent faxes, activity log, statistics, search queries, CSV codec. |
| `src/archive/` | Import pipeline (`ingest.rs`), filename parsing (`filename.rs`), folder polling thread (`watcher.rs`, `watcher/poller.rs`). |
| `src/pdf/` | Document inspection and page rendering for PDF and TIFF, including the decoders. |
| `src/system/` | OS integration: shell verbs, printer listing, native pickers, file copy. |
| `src/app/` | Application layer: `Msg`, `AppState`, `Controller`, and the `Platform` trait. |
| `src/ui/runtime/` | Generic retained-input runtime for AcmeUI `WidgetNode` trees: layout, paint, hit regions, focus, scroll, text editing. |
| `src/ui/views/` | Pure view functions: `AppState` to `WidgetNode<Msg>`. `kit.rs` holds shared helpers and glyphs. |
| `src/ui/theme.rs` | Maps `ThemeChoice` to the AcmeUI theme. |

### `src/app/`

- `msg.rs`: the `Msg` enum (every user intent), `Route`, `Field` (every editable text field, with a
  stable widget key `field.<name>`), `DatePreset`, `StatsRange`, `BoolSetting`, `ConfirmAction`.
- `state.rs`: `AppState`, the view model. Views read it; only the controller writes it.
- `controller.rs` plus `controller/*.rs`: `Controller` owns the `Store`, the `Platform`, the image
  cache, the watcher handle and the preview revision. Submodules handle one area each: `inbox`,
  `nav`, `fax_actions`, `preview`, `import`, `contacts`, `send`, `stats`, `settings`, `watch`,
  `confirm`, `refresh`, `feedback`, `fields`, `date_tree`, `date_range`. `tests*.rs` are controller tests.
- `platform.rs`: the `Platform` trait (open, reveal, print, print-to, list printers, pickers). The
  controller depends on the trait, so tests never launch real programs. `SystemPlatform` forwards
  to `crate::system`.

## Data flow

```
 Epson printer
      │  "save to network folder" (SMB)
      ▼
 watch folder on disk ──────────────────────────────┐
      │ poll every poll_interval_secs               │
      ▼                                             │
 archive::watcher thread (own Store connection)     │
   stable on 2 polls? readable? retry <= 3          │
   archive::ingest::ingest_file                     │
     support check -> SHA-256 -> duplicate check    │
     filename::parse (fallback: file mtime)         │
     pdf::page_count (0 on failure)                 │
     copy -> <root>/YYYY/YYYY-MM/YYYY-MM-DD/        │
     store.insert_fax (SQLite, WAL)                 │
      │ WatchEvent over mpsc + Notify (waker) ─────┘
      ▼
 UI thread: MyFaxApp
   background tick -> Controller::poll_background / tick
      │ reads/writes AppState; store queries
      ▼
 views::root(&AppState) -> WidgetNode<Msg>
      │
      ▼
 UiRuntime::render: layout -> paint -> hit regions -> Scene -> wgpu
      │ pointer / key / IME / file-drop events -> UiEvent -> Msg
      ▼
 Controller::on_ui_event(Msg)
      ├─ store (SQLite)
      ├─ Platform trait -> system (PowerShell, rfd, file copy)
      └─ toast / dialog / log lines
```

## Threading model

- The UI thread runs the AcmeUI event loop. `MyFaxApp::background_tick` runs every 750 ms
  (`BACKGROUND_POLL`) and calls `Controller::poll_background` and `Controller::tick`.
- The watcher runs on its own thread, `myfax-archive-watcher`. It opens its own `Store` at the same
  database path. It communicates through two `mpsc` channels (control in, events out) and an
  `AtomicBool` stop flag. After queuing events it calls a `Notify` callback; the UI installs a waker
  that wakes the event loop.
- `WatchEvent` variants: `Imported`, `Duplicate`, `Error`, `ScanCompleted`.

## Store

- `src/store/schema.rs` opens the file-backed database in WAL mode and sets `foreign_keys`.
  `src/store/mod.rs` sets `busy_timeout` so the UI connection and the watcher connection coexist.
- Migrations are ordered in `MIGRATIONS` (`MIGRATIONS[i]` upgrades version `i` to `i + 1`). The
  schema version is stored in `PRAGMA user_version`. A database with a newer version than the code
  knows is rejected. The current list has one migration, `SCHEMA_V1`.
- Tables (schema v1):
  - `faxes`: `id`, `received_at`, `received_ts`, `received_date`, `sender_number`, `sender_name`,
    `pages`, `file_path`, `file_size`, `sha256` (UNIQUE), `original_name`, `source`, `is_read`,
    `is_starred`, `note`, `deleted_at`, `imported_at`.
  - `fax_tags`: `fax_id`, `tag`.
  - `contacts`: `name`, `company`, `fax_number`, `phone`, `email`, `note`.
  - `sent_faxes`: send history.
  - `activity_log`: level, message, timestamp.
- Indexes: `received_date`, `received_ts`, `sender_number`, `deleted_at` on `faxes`, and `tag` on
  `fax_tags`.
- Trash is soft-deleted: `deleted_at` is set. Purging removes the file and the row.
- Search (`src/store/query.rs`) uses `LIKE` with an escape character over `sender_name`,
  `sender_number`, `note`, `original_name` and `fax_tags.tag`. It is not an FTS index.
- `csv.rs` is the contact CSV codec (see the formula guard and BOM rules in the README).
- `fill_sender_names` updates only rows whose `sender_name IS NULL`.

## Archive pipeline

`ingest_file` (`src/archive/ingest.rs`):

1. Support check: `pdf` always; `tif` and `tiff` only when `accept_tiff` is on.
2. SHA-256 of the source. If the hash exists, the file counts as a duplicate. The source is removed
   only when `delete_source_after_import` is on.
3. `filename::parse` gives the receive time and the sender number. The fallback is the file's
   modification time.
4. `pdf::page_count`. An unreadable file is still archived, with 0 pages.
5. The sender name comes from `contact_by_number`.
6. `archive_path_for` builds `<root>/YYYY/YYYY-MM/YYYY-MM-DD/YYYYMMDD-HHMMSS_<sender|unknown>.<ext>`.
   On collision it appends `_2`, `_3`, and so on. The ext is lower-case.
7. `copy_into_archive` uses `create_new`, so it never overwrites. It syncs and checks the copied size.
   On any error the partial destination is removed.
8. `store.insert_fax`. If the insert fails, the copy is removed. A concurrent import of the same hash
   is reported as a duplicate.
9. Optional forward copy to `forward_copy_folder`. Failure is logged, not fatal.
10. Optional source removal, then an Info log line.

`ingest_file` returns `IngestOutcome::{Imported, Duplicate, Skipped}`.

`watcher/poller.rs` rules (see the `WatcherHandle::spawn` doc comment):

- A file is ingested only after its size and modification time are unchanged across two consecutive
  polls, and it can be opened for reading.
- Zero-byte files are ignored until they have content.
- A file that fails is retried at most `MAX_FAILURES` (3) times at the same size and modification
  time. Failure reports happen at the first and last attempt. Then the file waits for a change.
- Only top-level files are read. Subfolders and unsupported extensions are ignored.
- A missing folder produces one error per change of state, not one per poll.
- Files already indexed by hash before start-up produce no events.

## PDF and TIFF decoding

`src/pdf/mod.rs`:

- The format is detected from the first four bytes (`%PDF`, or a classic TIFF header `II*\0` or
  `MM\0*`), never from the extension.
- `guarded()` wraps every decoder call in `catch_unwind`. A panic inside a parser becomes
  `PreviewError::Unsupported`.
- Limits: `MAX_SIDE_PX` 20000, `MAX_PIXELS` 100,000,000. The controller requests previews with a
  longest side of 1800 px (`PREVIEW_MAX_SIDE_PX`). Pages are never upscaled.

PDF (`pdf_doc.rs`, `pdf_image.rs`, `raster.rs`):

- Encrypted PDFs are rejected (`encrypted PDF`).
- Image filters supported: `FlateDecode` (with PNG predictors), `DCTDecode` (JPEG, via `image`),
  `CCITTFaxDecode` (K < 0 is G4, K = 0 is G3 one-dimensional). `CCITTFaxDecode` with K > 0 is
  rejected. Other filters, such as `JBIG2Decode`, are rejected.
- Raw 1-bit and 8-bit DeviceGray and DeviceRGB samples are supported, honouring `Decode [1 0]` and
  `BlackIs1`. ICCBased colour spaces with N = 1 or 3 are accepted; CMYK and Lab are not.
- Image masks are rejected unless they use the supported samples.

TIFF (`tiff_doc.rs`, `ccitt.rs`):

- A manual IFD walk over classic TIFF (magic 42) only.
- CCITT T.4 (one-dimensional) and T.6 pages are decoded strip by strip by `ccitt.rs`. Two-dimensional
  T.4 (T4Options bit 0) is rejected.
- Other compressions are decoded with the `image` crate, but only for the first page.
  `PageOutOfRange` covers invalid indexes.

## System integration

- `launch.rs`: open, reveal, print and printto. Windows actions are executed through
  `powershell -EncodedCommand` running a generated script. The file path and printer name are passed
  as literal values. Printer names containing a double quote are rejected (`printers.rs`).
- `printers.rs`: installed printers come from `Get-CimInstance Win32_Printer`. Output is parsed as
  UTF-8 (BOM stripped), de-duplicated and sorted. On other OSes it falls back to `lpstat`.
- `fax_printers` (in `system/mod.rs`) is a case-insensitive name match on "fax". The send page marks
  those printers with a Fax badge and lists them first.
- `dialogs.rs`: native pickers through `rfd` (folder, file, save).
- `files.rs`: `copy_into_dir` keeps the file name and appends ` (2)`, ` (3)` and so on on collision.

## Application flow

- Views only construct `Msg` values. `Controller::on_ui_event` maps a `UiEvent` to a `Msg` and
  handles it, then writes `AppState`.
- Text input: each `TextInput` key is `Field::key()`. Typing arrives as `UiEvent::TextChanged` and is
  stored in `AppState::fields`. Only `Field::Search` also re-queries immediately. Enter in a field
  produces `UiEvent::Submit` with the field's key; the controller maps the key to a `Msg`.
- Modal confirmation: while `AppState::confirm` is set, the controller ignores every `Msg` except
  `ConfirmYes`, `ConfirmNo` and `DismissToast`.
- Settings persistence: `config::save_settings` writes `settings.json.tmp` and renames it. A corrupt
  settings file falls back to defaults and is logged.

## Configuration and data location

- `config::app_data_dir()` is `MYFAX_DATA_DIR` when set and non-empty; otherwise `dirs::config_dir()`
  joined with `MyFax`, which is `%APPDATA%\MyFax` on Windows.
- `settings.json` and `myfax.db` live in that directory.
- Default archive root: `dirs::document_dir()\MyFax Archive`.
- `Settings::sanitized()` clamps `poll_interval_secs` to 2..=3600, `page_size` to 10..=500 and
  removes duplicate and empty watch folders. `trash_retention_days` is 0..=3650 in the settings
  validation.

## UI runtime

`src/ui/runtime/mod.rs` is the generic runtime. Its contract is in its module docs.

- Supported widgets: `Row`, `Column`, `Stack`, `Card`, `Label`, `Button`, `Separator`, `TextInput`.
  Unsupported widgets are skipped with a debug log.
- Frame: layout, then paint, then hit regions. `render` returns a `Scene`.
- Hit regions: enabled buttons and any container with `on_click`. A click is a press and release over
  the same target.
- Modal: a container whose key starts with `modal:` absorbs pointer input for everything painted
  before it, and restricts Tab focus to its children while it exists.
- Scroll regions: a keyed `Column` or `Row` with `overflow_scroll()`. Offsets persist per key and are
  clamped to content size.
- Image slots: a container whose key starts with `img:` draws the matching image from
  `FrameInput::images`, contain-fitted.
- Focus: Tab and Shift+Tab cycle focusable widgets in paint order. Focused buttons and text inputs
  draw a 2 px ring.
- Text editing (`edit.rs`): Enter submits (unless an IME composition is active), Escape cancels the
  composition first and otherwise emits `Cancel`. IME preedit and commit are routed through
  `ime_preedit` and `ime_commit`.
- Text layout (`text.rs`, `widgets.rs`): see the workarounds section below.

## Views

- `views::root(&AppState) -> WidgetNode<Msg>` composes the shell (`views/shell.rs`) and the page
  views: inbox (`inbox.rs`, `inbox/filters.rs`, `inbox/list.rs`), `detail.rs`, `contacts.rs`,
  `send.rs`, `stats.rs`, `log.rs`, `settings.rs`.
- Views are pure functions. They read `AppState` and return a tree. They contain no I/O.
- `kit.rs` holds the shared metrics, text helpers, badges, page header, empty state, formatting, and
  the glyph constants. Glyphs are plain text characters (for example `★`, `☆`, `●`, `○`), not an icon font.

## Internationalisation

- `i18n::t(lang, key)` and `i18n::tf(lang, key, &[(name, value)])` look up strings. `tf` replaces
  `{name}` placeholders.
- Catalog rows live in `src/i18n/catalog/<area>.rs` as `("key", "zh-TW", "English")` tuples. Areas:
  `app`, `archive`, `common`, `contacts`, `detail`, `inbox`, `log`, `send`, `settings`, `shell`,
  `stats`. `catalog/mod.rs` concatenates them.
- Default language: `Lang::ZhTw` (serialised as `zh-TW`). The other value is `Lang::En` (`en`).
- Tests in `src/i18n/mod.rs`:
  - keys are unique and non-empty;
  - placeholders match between languages;
  - `every_used_key_exists` scans the source for `t(` and `tf(` calls and checks each key exists;
  - the used-keys extractor is itself tested.
- Adding a string: add a row to the right fragment, use the same placeholders in both columns, and
  reference the key in source. The scanning test catches typos.

## AcmeUI-Native workarounds

These are the workarounds in use at the pinned revision. Each one is tied to code in this repo.

1. **`kit::full()` instead of `w_full()` / `h_full()`.** At the pinned revision `Styled::w_full()`
   is `self.w(Length::Percent(100.0))` (`acme-style/src/styled.rs`). The kit doc comment states that
   Taffy expects a 0..=1 fraction, so `Percent(100.0)` sizes a node at 100 times its parent.
   `kit::full()` returns `Length::Percent(1.0)`.
2. **`flex_shrink` defaults to 0.** At the pinned revision `acme-layout` sets `flex_shrink: 0.0` by
   default. Flexible panes therefore use `.flex_1()` with `.flex_shrink(1.0)` and an explicit
   `.min_w(...)`. This pattern appears throughout `views/contacts.rs`, `views/detail.rs` and
   `views/inbox/filters.rs`.
3. **Text leaf min-content width.** A text leaf's min-content width is its single-line width.
   Long names and paths in a card would overflow, so `views/detail.rs` sets `.min_w(kit::px(0.0))` with
   `flex_1()` and `flex_shrink(1.0)` on value labels, which lets them wrap.
4. **Label wrapping.** Labels default to no wrap. Prose uses `LabelWrap::Word`. Paths and file names
   use `LabelWrap::Character`, and only inside a width-constrained parent. `runtime/widgets.rs` adds
   `WRAP_SLACK` (0.5 px) so a word-wrapped label re-wraps exactly as it was measured.
5. **Single-line fit tolerance.** `runtime/text.rs` uses `FIT_SLACK` (1.0 px). A single-line label laid
   out at its measured width is not ellipsised by sub-pixel differences between layout and paint.
6. **Emoji-presentation code points are avoided.** `kit::glyph` comments say that U+2611, U+2699,
   U+2709 and U+263A draw from the colour emoji font on Windows, which the glyph atlas renders as a
   solid block. Use the non-emoji glyphs in `kit::glyph`.
7. **Stack children are absolutely positioned.** UI_SPEC section 0.2 records that every child of a
   `stack()` is placed at the stack's top-left and sized to its content. Each full-window layer must
   therefore set both width and height to full size, and centring uses flex spacers. This is recorded
   in the UI spec, not verified by a test in this repo.

## Testing

- Unit tests sit beside the code (`#[cfg(test)]` modules, `tests*.rs` files, `testutil` helpers).
  Store tests use `tempfile` databases. Archive and watcher tests use temporary folders and synthetic
  PDFs (`pdf_bytes`).
- `cargo test --locked --all-targets` runs unit and integration-style tests. `cargo test --locked --doc`
  runs doc tests. CI runs both, plus `cargo fmt --check` and `cargo clippy -D warnings`.
- QA gaps are listed in the README under Known limitations.

## Change history

| Version | Date | Change | Scope |
|---|---|---|---|
| v0.1.0 | 2026-11-10 | Initial developer architecture document. | docs/ARCHITECTURE.md |
