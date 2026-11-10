//! Path comparisons for the watch-folder rules. A watch folder may not be the archive
//! root or inside it, nor the copy folder. Comparisons use normalised paths: the
//! nearest existing ancestor is canonicalised (so folders not yet created still
//! compare), `..` is resolved, and case is ignored on Windows.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// True when `child` is `parent` itself or lies inside it (whole path components).
pub(super) fn is_same_or_inside(child: &Path, parent: &Path) -> bool {
    comparable(child).starts_with(comparable(parent))
}

/// Normalised form used for comparisons (see the module docs).
fn comparable(path: &Path) -> PathBuf {
    let mut missing: Vec<&OsStr> = Vec::new();
    let mut current = path;
    let base = loop {
        match std::fs::canonicalize(current) {
            Ok(canonical) => break strip_verbatim(canonical),
            Err(_) => match (current.file_name(), current.parent()) {
                (Some(name), Some(parent)) => {
                    missing.push(name);
                    current = parent;
                }
                _ => break path.to_path_buf(),
            },
        }
    };
    let mut normalised = base;
    for name in missing.iter().rev() {
        normalised.push(name);
    }
    fold_case(normalised)
}

/// Drop the Windows verbatim prefix (`\?\C:\…` → `C:\…`, `\?\UNC\s\h` → `\s\h`)
/// that `canonicalize` adds, so canonical and typed paths compare equal.
fn strip_verbatim(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy().into_owned();
    if let Some(rest) = text.strip_prefix(r"\?\UNC\") {
        PathBuf::from(format!(r"\{rest}"))
    } else if let Some(rest) = text.strip_prefix(r"\?\") {
        PathBuf::from(rest)
    } else {
        path
    }
}

fn fold_case(path: PathBuf) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(path.to_string_lossy().to_lowercase())
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let archive = dir.path().join("archive");
        std::fs::create_dir_all(archive.join("2026")).expect("mkdir");
        (dir, archive)
    }

    #[test]
    fn child_inside_parent_and_the_parent_itself_are_detected() {
        let (_dir, archive) = fixture();
        assert!(is_same_or_inside(&archive.join("2026"), &archive));
        assert!(is_same_or_inside(&archive, &archive));
        assert!(!is_same_or_inside(&archive, &archive.join("2026")));
    }

    #[test]
    fn a_name_that_merely_starts_with_the_parent_is_not_inside() {
        let (dir, archive) = fixture();
        let sibling = dir.path().join("archive2");
        std::fs::create_dir_all(&sibling).expect("mkdir");
        assert!(!is_same_or_inside(&sibling, &archive));
    }

    #[test]
    fn folders_that_do_not_exist_yet_compare_through_their_existing_ancestor() {
        let (_dir, archive) = fixture();
        let future = archive.join("not").join("yet");
        assert!(is_same_or_inside(&future, &archive));
        assert!(!is_same_or_inside(&archive, &future));
    }

    #[test]
    fn dot_dot_segments_are_resolved_before_comparing() {
        let (_dir, archive) = fixture();
        let sneaky = archive.join("..").join("archive").join("2026");
        assert!(is_same_or_inside(&sneaky, &archive));
    }

    #[test]
    fn case_only_differences_match_on_windows_only() {
        let (_dir, archive) = fixture();
        let upper = PathBuf::from(archive.to_string_lossy().to_uppercase());
        assert_eq!(is_same_or_inside(&upper, &archive), cfg!(windows));
    }
}
