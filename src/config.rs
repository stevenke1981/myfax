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

/// Environment variable that overrides the data directory (portable / test use).
pub const DATA_DIR_ENV: &str = "MYFAX_DATA_DIR";

/// `%APPDATA%\MyFax` (or the platform equivalent); created on demand by callers.
/// A non-empty `MYFAX_DATA_DIR` environment variable takes precedence.
pub fn app_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os(DATA_DIR_ENV).filter(|v| !v.is_empty()) {
        return PathBuf::from(dir);
    }
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

/// Upper bound on numbered backup names tried before giving up.
const MAX_BACKUP_ATTEMPTS: u32 = 100;

/// Load settings field by field. Returns `(settings, invalid_field_names, error)`:
/// - a missing file gives defaults;
/// - a field whose value does not deserialise is reset to its default and named in
///   the list (the other fields are kept);
/// - a document that is not a JSON object (syntax error) gives full defaults and an
///   error, after the original is copied to `settings.json.bak-<timestamp>`;
/// - an I/O error gives defaults and an error.
pub fn load_settings_lenient(path: &Path) -> (Settings, Vec<String>, Option<anyhow::Error>) {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return (Settings::default(), Vec::new(), None);
        }
        Err(e) => {
            let error = anyhow::anyhow!("cannot read {}: {e}", path.display());
            return (Settings::default(), Vec::new(), Some(error));
        }
    };
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(serde_json::Value::Object(object)) => lenient_from_object(object),
        Ok(_) => corrupt_file_defaults(path, "the document is not a JSON object"),
        Err(e) => corrupt_file_defaults(path, &e.to_string()),
    }
}

/// Overlay every known field of `object` that deserialises onto the defaults.
fn lenient_from_object(
    object: serde_json::Map<String, serde_json::Value>,
) -> (Settings, Vec<String>, Option<anyhow::Error>) {
    let Ok(serde_json::Value::Object(mut accepted)) = serde_json::to_value(Settings::default())
    else {
        return (Settings::default(), Vec::new(), None);
    };
    let mut invalid = Vec::new();
    for (key, value) in object {
        // Unknown keys are ignored, exactly as the strict loader does.
        if !accepted.contains_key(&key) {
            continue;
        }
        let mut trial = accepted.clone();
        trial.insert(key.clone(), value);
        if serde_json::from_value::<Settings>(serde_json::Value::Object(trial.clone())).is_ok() {
            accepted = trial;
        } else {
            invalid.push(key);
        }
    }
    // Every accepted state deserialised on its own, so this final step cannot fail;
    // the default is only a defensive fallback.
    let settings = serde_json::from_value::<Settings>(serde_json::Value::Object(accepted))
        .unwrap_or_default()
        .sanitized();
    (settings, invalid, None)
}

/// Back up the corrupt file first, then return full defaults with an error that
/// names the backup (or the backup failure).
fn corrupt_file_defaults(
    path: &Path,
    reason: &str,
) -> (Settings, Vec<String>, Option<anyhow::Error>) {
    let note = match backup_settings_file(path) {
        Ok(backup) => format!("; the original was copied to {}", backup.display()),
        Err(error) => format!("; backup failed: {error}"),
    };
    let error = anyhow::anyhow!("invalid settings file {}: {reason}{note}", path.display());
    (Settings::default(), Vec::new(), Some(error))
}

/// Copy `path` to `<name>.bak-<YYYYMMDD-HHMMSS>` (with `-n` appended on a clash). Never
/// overwrites an existing backup.
pub fn backup_settings_file(path: &Path) -> std::io::Result<PathBuf> {
    use std::io::Write;
    let name = path.file_name().map_or_else(
        || "settings.json".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let bytes = std::fs::read(path)?;
    for attempt in 0..MAX_BACKUP_ATTEMPTS {
        let suffix = if attempt == 0 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let target = path.with_file_name(format!("{name}.bak-{stamp}{suffix}"));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(mut file) => {
                file.write_all(&bytes)?;
                return Ok(target);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "no free backup file name",
    ))
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
        let s = Settings {
            language: Lang::En,
            poll_interval_secs: 0, // clamped
            watch_folders: vec!["a".into(), "a".into(), "".into(), "b".into()],
            ..Settings::default()
        };
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
    fn lenient_load_keeps_valid_fields_and_resets_only_invalid_ones() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.json");
        std::fs::write(
            &path,
            r#"{"auto_print": true, "language": "fr", "poll_interval_secs": 42, "watch_folders": ["D:/in"]}"#,
        )
        .unwrap();
        let (settings, invalid, error) = load_settings_lenient(&path);
        assert!(error.is_none(), "a bad field is not a load error");
        assert_eq!(invalid, vec!["language".to_string()]);
        assert!(settings.auto_print);
        assert_eq!(settings.poll_interval_secs, 42);
        assert_eq!(settings.watch_folders, vec![PathBuf::from("D:/in")]);
        assert_eq!(settings.language, Lang::default());
        assert!(
            std::fs::read_dir(dir.path()).unwrap().count() == 1,
            "no backup for a field-level problem"
        );
    }

    #[test]
    fn lenient_load_of_a_valid_or_missing_file_reports_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (defaults, invalid, error) = load_settings_lenient(&dir.path().join("none.json"));
        assert_eq!(defaults, Settings::default().sanitized());
        assert!(invalid.is_empty() && error.is_none());

        let path = dir.path().join("s.json");
        let s = Settings {
            language: Lang::En,
            ..Settings::default()
        };
        save_settings(&path, &s).unwrap();
        let (back, invalid, error) = load_settings_lenient(&path);
        assert_eq!(back, s);
        assert!(invalid.is_empty() && error.is_none());
    }

    #[test]
    fn lenient_load_backs_up_a_syntax_error_before_falling_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{not json").unwrap();
        let (settings, invalid, error) = load_settings_lenient(&path);
        assert_eq!(settings, Settings::default().sanitized());
        assert!(invalid.is_empty());
        let error = error.expect("syntax error is reported");
        assert!(error.to_string().contains("settings.json.bak-"), "{error}");

        let backups: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with("settings.json.bak-"))
            .collect();
        assert_eq!(backups.len(), 1, "exactly one backup: {backups:?}");
        let copy = std::fs::read_to_string(dir.path().join(&backups[0])).unwrap();
        assert_eq!(copy, "{not json", "backup holds the original bytes");
    }

    #[test]
    fn lenient_load_treats_a_non_object_document_as_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "[1, 2, 3]").unwrap();
        let (settings, _, error) = load_settings_lenient(&path);
        assert_eq!(settings, Settings::default().sanitized());
        assert!(error.is_some());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn corrupt_file_is_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.json");
        std::fs::write(&path, "{not json").unwrap();
        assert!(load_settings(&path).is_err());
    }
}
