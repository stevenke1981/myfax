//! Persistent user settings (contract owned by the orchestrator; load/save
//! bodies owned by the system/integration worker).

use crate::i18n::Lang;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Light / dark appearance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeChoice {
    #[default]
    Light,
    Dark,
}

/// All user-tunable settings. Serialised as JSON in [`settings_path`].
/// Unknown / missing fields fall back to [`Settings::default`] (serde default).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub language: Lang,
    pub theme: ThemeChoice,
    /// Folders polled for new fax files (printer "save to network folder" targets).
    pub watch_folders: Vec<PathBuf>,
    /// Root of the date-organised archive: `<root>/YYYY/YYYY-MM/YYYY-MM-DD/`.
    pub archive_root: PathBuf,
    /// Seconds between folder polls (clamped to 2..=3600 by [`Settings::sanitized`]).
    pub poll_interval_secs: u64,
    /// Remove the source file after it was archived successfully (move instead of copy).
    pub delete_source_after_import: bool,
    /// Also accept TIFF faxes (`.tif`, `.tiff`), not only PDF.
    pub accept_tiff: bool,
    /// Send each newly received fax to the default printer.
    pub auto_print: bool,
    /// Optional extra folder that receives a copy of every new fax.
    pub forward_copy_folder: Option<PathBuf>,
    /// Trash entries older than this many days are purged at start-up (0 = never).
    pub trash_retention_days: u32,
    /// Items per inbox page (clamped to 10..=500).
    pub page_size: u32,
    /// Show the in-app banner when a new fax arrives.
    pub notify_new_fax: bool,
    /// Preferred printer name for the "Send fax" page (an Epson FAX driver queue).
    pub fax_printer: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        let docs = dirs::document_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            language: Lang::default(),
            theme: ThemeChoice::Light,
            watch_folders: Vec::new(),
            archive_root: docs.join("MyFax Archive"),
            poll_interval_secs: 5,
            delete_source_after_import: false,
            accept_tiff: true,
            auto_print: false,
            forward_copy_folder: None,
            trash_retention_days: 30,
            page_size: 50,
            notify_new_fax: true,
            fax_printer: None,
        }
    }
}

impl Settings {
    /// Returns a copy with every numeric field clamped into its valid range
    /// and duplicate / empty watch folders removed (order preserved).
    pub fn sanitized(&self) -> Self {
        let mut out = self.clone();
        out.poll_interval_secs = out.poll_interval_secs.clamp(2, 3600);
        out.page_size = out.page_size.clamp(10, 500);
        let mut seen: Vec<PathBuf> = Vec::new();
        for folder in &self.watch_folders {
            if folder.as_os_str().is_empty() || seen.contains(folder) {
                continue;
            }
            seen.push(folder.clone());
        }
        out.watch_folders = seen;
        out
    }
}

/// `%APPDATA%\MyFax` (or the platform equivalent); created on demand by callers.
pub fn app_data_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("MyFax")
}

/// `<app_data_dir>/settings.json`.
pub fn settings_path() -> PathBuf {
    app_data_dir().join("settings.json")
}

/// `<app_data_dir>/myfax.db`.
pub fn database_path() -> PathBuf {
    app_data_dir().join("myfax.db")
}

/// Load settings from `path`. A missing file yields defaults; a corrupt file
/// yields an error (callers fall back to defaults and log a warning).
pub fn load_settings(path: &Path) -> anyhow::Result<Settings> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let parsed: Settings = serde_json::from_str(&text)
                .map_err(|e| anyhow::anyhow!("invalid settings file {}: {e}", path.display()))?;
            Ok(parsed.sanitized())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(anyhow::anyhow!("cannot read {}: {e}", path.display())),
    }
}

/// Save settings atomically (write `*.tmp`, then rename), creating parent dirs.
pub fn save_settings(path: &Path, settings: &Settings) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(&settings.sanitized())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let s = load_settings(&dir.path().join("none.json")).unwrap();
        assert_eq!(s, Settings::default().sanitized());
    }

    #[test]
    fn roundtrip_and_partial_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.json");
        let mut s = Settings::default();
        s.language = Lang::En;
        s.poll_interval_secs = 0; // clamped
        s.watch_folders = vec!["a".into(), "a".into(), "".into(), "b".into()];
        save_settings(&path, &s).unwrap();
        let back = load_settings(&path).unwrap();
        assert_eq!(back.language, Lang::En);
        assert_eq!(back.poll_interval_secs, 2);
        assert_eq!(
            back.watch_folders,
            vec![PathBuf::from("a"), PathBuf::from("b")]
        );

        std::fs::write(&path, r#"{"auto_print": true}"#).unwrap();
        let partial = load_settings(&path).unwrap();
        assert!(partial.auto_print);
        assert_eq!(partial.page_size, 50);
    }

    #[test]
    fn corrupt_file_is_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.json");
        std::fs::write(&path, "{not json").unwrap();
        assert!(load_settings(&path).is_err());
    }
}
