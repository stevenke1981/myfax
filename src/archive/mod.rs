//! Receiving pipeline: filename parsing, ingest into the date archive, folder watcher.
//! Owner: archive worker. Public API = orchestrator contract.

pub mod filename;
pub mod ingest;
pub mod watcher;
