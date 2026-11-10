//! Side-effect boundary used by the controller so tests never launch real
//! programs, printers or dialogs. Orchestrator-owned contract.

use std::path::{Path, PathBuf};

pub trait Platform {
    fn open_path(&self, path: &Path) -> anyhow::Result<()>;
    fn reveal_in_folder(&self, path: &Path) -> anyhow::Result<()>;
    fn print_file(&self, path: &Path) -> anyhow::Result<()>;
    fn print_to(&self, path: &Path, printer: &str) -> anyhow::Result<()>;
    fn list_printers(&self) -> anyhow::Result<Vec<String>>;
    fn pick_folder(&self, start: Option<&Path>) -> Option<PathBuf>;
    fn pick_file(&self, start: Option<&Path>, extensions: &[&str]) -> Option<PathBuf>;
    fn save_file(
        &self,
        start: Option<&Path>,
        default_name: &str,
        extensions: &[&str],
    ) -> Option<PathBuf>;
}

/// Real implementation backed by [`crate::system`].
pub struct SystemPlatform;

impl Platform for SystemPlatform {
    fn open_path(&self, path: &Path) -> anyhow::Result<()> {
        crate::system::open_path(path)
    }
    fn reveal_in_folder(&self, path: &Path) -> anyhow::Result<()> {
        crate::system::reveal_in_folder(path)
    }
    fn print_file(&self, path: &Path) -> anyhow::Result<()> {
        crate::system::print_file(path)
    }
    fn print_to(&self, path: &Path, printer: &str) -> anyhow::Result<()> {
        crate::system::print_to(path, printer)
    }
    fn list_printers(&self) -> anyhow::Result<Vec<String>> {
        crate::system::list_printers()
    }
    fn pick_folder(&self, start: Option<&Path>) -> Option<PathBuf> {
        crate::system::pick_folder(start)
    }
    fn pick_file(&self, start: Option<&Path>, extensions: &[&str]) -> Option<PathBuf> {
        crate::system::pick_file(start, extensions)
    }
    fn save_file(
        &self,
        start: Option<&Path>,
        default_name: &str,
        extensions: &[&str],
    ) -> Option<PathBuf> {
        crate::system::save_file(start, default_name, extensions)
    }
}
