//! Non-overwriting file copy: `name.ext`, then `name (2).ext`, `name (3).ext`, ...

use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

/// Safety valve so a pathological directory cannot loop forever.
const MAX_COLLISION_ATTEMPTS: usize = 9_999;

/// Copy `src` into `dest_dir` keeping its file name, never overwriting. Creates
/// `dest_dir` when missing. Returns the path that was written.
pub fn copy_into_dir(src: &Path, dest_dir: &Path) -> io::Result<PathBuf> {
    let file_name = src
        .file_name()
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "source path has no file name"))?;
    if !fs::metadata(src)?.is_file() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "source is not a regular file",
        ));
    }
    fs::create_dir_all(dest_dir)?;
    let mut source = File::open(src)?;
    for attempt in 1..=MAX_COLLISION_ATTEMPTS {
        let candidate = dest_dir.join(numbered_file_name(file_name, attempt));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut target) => return copy_into_new_file(&mut source, &mut target, &candidate),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    Err(io::Error::new(
        ErrorKind::AlreadyExists,
        "too many file name collisions in destination",
    ))
}

/// Copy the bytes into a file we just created exclusively; on failure remove the
/// partial file so no truncated copy is left behind.
fn copy_into_new_file(
    source: &mut File,
    target: &mut File,
    candidate: &Path,
) -> io::Result<PathBuf> {
    match io::copy(source, target) {
        Ok(_) => Ok(candidate.to_path_buf()),
        Err(err) => {
            if let Err(cleanup) = fs::remove_file(candidate) {
                tracing::warn!(
                    path = %candidate.display(),
                    error = %cleanup,
                    "could not remove partial copy"
                );
            }
            Err(err)
        }
    }
}

/// Attempt 1 keeps the original name; attempt `n >= 2` inserts ` (n)` before the
/// extension. Works on `OsStr` so non-UTF-8 names keep their bytes.
pub(super) fn numbered_file_name(file_name: &OsStr, attempt: usize) -> OsString {
    if attempt <= 1 {
        return file_name.to_os_string();
    }
    let path = Path::new(file_name);
    let stem = path.file_stem().unwrap_or(file_name);
    let mut name = stem.to_os_string();
    name.push(format!(" ({attempt})"));
    if let Some(ext) = path.extension() {
        name.push(".");
        name.push(ext);
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(text: &str, attempt: usize) -> String {
        numbered_file_name(OsStr::new(text), attempt)
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn numbering_inserts_counter_before_extension() {
        assert_eq!(name("a.pdf", 1), "a.pdf");
        assert_eq!(name("a.pdf", 2), "a (2).pdf");
        assert_eq!(name("a.pdf", 3), "a (3).pdf");
    }

    #[test]
    fn numbering_handles_files_without_extension_and_dotfiles() {
        assert_eq!(name("notes", 2), "notes (2)");
        assert_eq!(name(".bashrc", 2), ".bashrc (2)");
    }

    #[test]
    fn numbering_only_splits_the_last_extension() {
        assert_eq!(name("a.tar.gz", 2), "a.tar (2).gz");
    }

    #[test]
    fn copy_names_collisions_a_then_2_then_3_without_overwriting() {
        let root = tempfile::tempdir().expect("tempdir");
        let src_dir = root.path().join("src");
        let dest = root.path().join("inbox");
        fs::create_dir_all(&src_dir).expect("src dir");
        let src = src_dir.join("a.pdf");
        fs::write(&src, b"first").expect("write src");

        let first = copy_into_dir(&src, &dest).expect("first copy");
        fs::write(&src, b"second").expect("rewrite src");
        let second = copy_into_dir(&src, &dest).expect("second copy");
        fs::write(&src, b"third").expect("rewrite src");
        let third = copy_into_dir(&src, &dest).expect("third copy");

        assert_eq!(first, dest.join("a.pdf"));
        assert_eq!(second, dest.join("a (2).pdf"));
        assert_eq!(third, dest.join("a (3).pdf"));
        assert_eq!(fs::read(&first).expect("read first"), b"first");
        assert_eq!(fs::read(&second).expect("read second"), b"second");
        assert_eq!(fs::read(&third).expect("read third"), b"third");
    }

    #[test]
    fn copy_keeps_existing_destination_file_untouched() {
        let root = tempfile::tempdir().expect("tempdir");
        let src = root.path().join("a.pdf");
        fs::write(&src, b"new").expect("write src");
        let dest = root.path().join("out");
        fs::create_dir_all(&dest).expect("dest dir");
        fs::write(dest.join("a.pdf"), b"keep me").expect("existing");

        let written = copy_into_dir(&src, &dest).expect("copy");

        assert_eq!(written, dest.join("a (2).pdf"));
        assert_eq!(fs::read(dest.join("a.pdf")).expect("read"), b"keep me");
    }

    #[test]
    fn copy_handles_file_without_extension() {
        let root = tempfile::tempdir().expect("tempdir");
        let src = root.path().join("notes");
        fs::write(&src, b"x").expect("write src");
        let dest = root.path().join("out");

        let first = copy_into_dir(&src, &dest).expect("first");
        let second = copy_into_dir(&src, &dest).expect("second");

        assert_eq!(first, dest.join("notes"));
        assert_eq!(second, dest.join("notes (2)"));
    }

    #[test]
    fn copy_creates_nested_destination_directory() {
        let root = tempfile::tempdir().expect("tempdir");
        let src = root.path().join("a.pdf");
        fs::write(&src, b"x").expect("write src");
        let dest = root.path().join("2026").join("2026-11").join("2026-11-10");

        let written = copy_into_dir(&src, &dest).expect("copy");

        assert!(dest.is_dir());
        assert_eq!(written, dest.join("a.pdf"));
    }

    #[test]
    fn copy_nonexistent_source_is_not_found() {
        let root = tempfile::tempdir().expect("tempdir");
        let err = copy_into_dir(&root.path().join("missing.pdf"), &root.path().join("out"))
            .expect_err("missing source must fail");
        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert!(!root.path().join("out").exists());
    }

    #[test]
    fn copy_directory_source_is_rejected() {
        let root = tempfile::tempdir().expect("tempdir");
        let folder = root.path().join("folder");
        fs::create_dir_all(&folder).expect("folder");
        let err = copy_into_dir(&folder, &root.path().join("out")).expect_err("dir rejected");
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }

    #[test]
    fn copy_fails_when_destination_is_a_file() {
        let root = tempfile::tempdir().expect("tempdir");
        let src = root.path().join("a.pdf");
        fs::write(&src, b"x").expect("write src");
        let blocker = root.path().join("blocker");
        fs::write(&blocker, b"file").expect("blocker");
        assert!(copy_into_dir(&src, &blocker).is_err());
    }
}
