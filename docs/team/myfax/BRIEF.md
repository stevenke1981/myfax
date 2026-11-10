# MyFax — shared team brief (read before any work)

## Goal
A native Rust desktop app (Windows first) that replaces the Epson FAX Utility
"receive to PC" workflow. Epson (and other) printers can forward every received
fax as a PDF/TIFF file to a network folder (SMB "Save to Network Folder" /
"Fax Output → Forward"). MyFax watches those folders, archives every fax into a
date-organised tree (`<root>/YYYY/YYYY-MM/YYYY-MM-DD/`), indexes it in SQLite and
offers inbox, search, date navigation, preview, print, export, contacts,
statistics, activity log and "send via printer fax driver". UI: AcmeUI-Native.
Languages: zh-TW (default) and English.

## Read-only references
- AcmeUI-Native checkout (pinned rev `1f23c910`): cargo git checkout under
  `~/.cargo/git/checkouts/acmeui-native-*/1f23c91/` and a clone at
  a local clone of https://github.com/stevenke1981/AcmeUI-Native at the same revision.
  Useful: `apps/gallery/src/render/*` (painter, hit test), `examples/sdk-consumer/src/main.rs`,
  `crates/acme-widgets/src/*`, `crates/acme-platform/src/lib.rs`, `crates/acme-layout`,
  `crates/acme-text`, `crates/acme-textinput`, `crates/acme-core/src/scene.rs`.
- Do NOT modify the AcmeUI-Native checkout.

## Shared contracts (orchestrator-owned — do not edit; ask in your report if a change is needed)
- `src/model.rs` domain types, `src/config.rs` settings, `src/i18n/mod.rs` lookup API,
  `src/i18n/catalog/mod.rs`, `src/app/msg.rs` (Msg, Field, Route…), `src/app/state.rs`
  (AppState), `src/app/platform.rs`, `src/lib.rs`, `Cargo.toml`, every `mod.rs`
  that only declares modules.
- The public signatures already present in your owned files are the contract.
  Keep them exactly; you may add private helpers, private submodules (declare them
  inside your own file/dir) and tests.

## i18n rules
- Call `crate::i18n::t(lang, "area.key")` / `tf(lang, "area.key", &[("name", value)])`
  with a plain variable (or field access like `state.lang`) as first arg and a
  string literal key — a test scans sources for these literals.
- Add your keys to YOUR fragment `src/i18n/catalog/<area>.rs` as rows
  `("area.key", "繁體中文", "English")`. Natural Taiwanese wording (例：傳真、收件匣、
  寄件者、已讀、加星號、垃圾桶、列印、匯出、聯絡人、設定). No Simplified Chinese.
- Never hard-code user-visible text outside the catalog.

## Hard rules
1. Only edit files you own (listed in your task). Never touch other files.
2. No `git add/commit/stash/checkout/reset`. No new dependencies (Cargo.toml is fixed:
   acme-*, anyhow, thiserror, tracing, serde(+json), chrono, sha2, rusqlite(bundled),
   lopdf 0.34, image 0.25 (jpeg/png/tiff), fax 0.2, dirs 5, rfd 0.15; dev: tempfile).
3. `#![forbid(unsafe_code)]` is crate-wide. No `unwrap()`/`expect()` outside tests,
   no panics on bad input, no `todo!()` left in your files.
4. Tests: write unit tests in your files (`#[cfg(test)] mod tests`), run only
   `cargo test --lib <your_module_path>` and `cargo clippy --all-targets -- -D warnings`
   limited to judging your files. Tests must not touch real user folders, printers,
   or launch programs — use `tempfile`.
5. Format with `cargo fmt -- <your files>` (or `rustfmt --edition 2024 <file>`).
6. Text cursor positions are char indices, never byte offsets.

## Report format (final message, ≤ 40 lines)
- Files changed; public API kept (yes/no + any deviation).
- Commands run with exit codes and test counts (PASS/FAIL/NOT_RUN — be truthful).
- i18n keys added (count) and fragment file.
- Known limitations / open questions for the orchestrator.
