//! Aggregates all i18n fragments. Orchestrator-owned: add a fragment here only.

mod app;
mod archive;
mod common;
mod contacts;
mod detail;
mod inbox;
mod log;
mod send;
mod settings;
mod shell;
mod stats;

/// Every catalog row from every fragment.
pub fn all_entries() -> Vec<(&'static str, &'static str, &'static str)> {
    [
        common::ENTRIES,
        shell::ENTRIES,
        inbox::ENTRIES,
        detail::ENTRIES,
        contacts::ENTRIES,
        send::ENTRIES,
        stats::ENTRIES,
        log::ENTRIES,
        settings::ENTRIES,
        app::ENTRIES,
        archive::ENTRIES,
    ]
    .iter()
    .flat_map(|rows| rows.iter().copied())
    .collect()
}
