//! Shell launches: open, reveal, print and printto. Command construction is kept
//! separate from spawning so tests can inspect commands without starting anything.
//!
//! Windows file actions go through `powershell -EncodedCommand` running a
//! `ProcessStartInfo` with `UseShellExecute`. The path is embedded as a PowerShell
//! literal, so spaces, `&`, `^`, `%`, `'` and CJK characters need no shell quoting.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use super::command::{new_command, spawn_detached};
use super::printers::validate_printer_name;
use super::ps::{literal, powershell_command};

/// Open with the default application (no wait).
pub fn open(path: &Path) -> Result<()> {
    ensure_exists(path)?;
    // Not hidden: ShellExecute with SW_HIDE can start the app with no visible window.
    let cmd = open_command(path)?;
    spawn_detached(cmd, "open", true)
}

/// Reveal the file in Explorer (or open the folder itself).
pub fn reveal(path: &Path) -> Result<()> {
    ensure_exists(path)?;
    let cmd = reveal_command(path)?;
    // explorer.exe commonly exits with 1 even on success.
    spawn_detached(cmd, "reveal", false)
}

/// Shell "print" verb with the default printer (no wait).
pub fn print(path: &Path) -> Result<()> {
    ensure_exists(path)?;
    let cmd = print_command(path)?;
    spawn_detached(cmd, "print", true)
}

/// Shell "printto" verb with a named printer (no wait).
pub fn print_to(path: &Path, printer: &str) -> Result<()> {
    let printer = validate_printer_name(printer)?;
    ensure_exists(path)?;
    let cmd = print_to_command(path, printer)?;
    spawn_detached(cmd, "print_to", true)
}

pub(super) fn open_command(path: &Path) -> Result<Command> {
    if cfg!(windows) {
        shell_exec_command(path, ShellAction::default())
    } else {
        let mut cmd = new_command(unix_opener());
        cmd.arg(absolute(path)?);
        Ok(cmd)
    }
}

pub(super) fn reveal_command(path: &Path) -> Result<Command> {
    if cfg!(windows) {
        windows_reveal_command(path)
    } else if cfg!(target_os = "macos") {
        let mut cmd = new_command("open");
        cmd.arg("-R").arg(absolute(path)?);
        Ok(cmd)
    } else {
        let folder: &Path = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };
        let mut cmd = new_command("xdg-open");
        cmd.arg(absolute(folder)?);
        Ok(cmd)
    }
}

pub(super) fn print_command(path: &Path) -> Result<Command> {
    if cfg!(windows) {
        let action = ShellAction {
            verb: Some("print"),
            arguments: None,
            hidden: true,
        };
        shell_exec_command(path, action)
    } else {
        let mut cmd = new_command("lp");
        cmd.arg(absolute(path)?);
        Ok(cmd)
    }
}

pub(super) fn print_to_command(path: &Path, printer: &str) -> Result<Command> {
    let printer = validate_printer_name(printer)?;
    if cfg!(windows) {
        let action = ShellAction {
            verb: Some("printto"),
            arguments: Some(printto_arguments(printer)),
            hidden: true,
        };
        shell_exec_command(path, action)
    } else {
        let mut cmd = new_command("lp");
        cmd.arg("-d").arg(printer).arg(absolute(path)?);
        Ok(cmd)
    }
}

/// Shell verb arguments for a printer: the printer name in double quotes.
pub(super) fn printto_arguments(printer: &str) -> String {
    format!("\"{printer}\"")
}

/// Explorer `/select` argument. Explorer takes it verbatim, so the path is quoted
/// here and must not contain `"` (not allowed in Windows file names anyway).
pub(super) fn select_argument(path: &str) -> Result<String> {
    if path.contains('"') {
        bail!("path contains a double quote and cannot be passed to Explorer");
    }
    Ok(format!("/select,\"{path}\""))
}

/// Parameters for one ShellExecute-based PowerShell launch.
#[derive(Default)]
pub(super) struct ShellAction<'a> {
    pub verb: Option<&'a str>,
    pub arguments: Option<String>,
    pub hidden: bool,
}

fn shell_exec_command(path: &Path, action: ShellAction<'_>) -> Result<Command> {
    let text = path
        .to_str()
        .with_context(|| format!("path is not valid Unicode: {}", path.display()))?;
    powershell_command(&shell_exec_script(text, &action))
}

/// PowerShell script that starts `path` via ProcessStartInfo. `$ErrorActionPreference`
/// turns a failed start into a non-zero exit so the watcher can log it.
pub(super) fn shell_exec_script(path: &str, action: &ShellAction<'_>) -> String {
    let mut lines = vec![
        "$ErrorActionPreference = 'Stop'".to_owned(),
        "$info = New-Object System.Diagnostics.ProcessStartInfo".to_owned(),
        format!("$info.FileName = {}", literal(path)),
        "$info.UseShellExecute = $true".to_owned(),
    ];
    if action.hidden {
        lines.push("$info.WindowStyle = 'Hidden'".to_owned());
    }
    if let Some(verb) = action.verb {
        lines.push(format!("$info.Verb = {}", literal(verb)));
    }
    if let Some(arguments) = &action.arguments {
        lines.push(format!("$info.Arguments = {}", literal(arguments)));
    }
    lines.push("[void][System.Diagnostics.Process]::Start($info)".to_owned());
    lines.join("\n")
}

fn windows_reveal_command(path: &Path) -> Result<Command> {
    if path.is_dir() {
        let mut cmd = new_command("explorer.exe");
        cmd.arg(path);
        return Ok(cmd);
    }
    let text = path
        .to_str()
        .with_context(|| format!("path is not valid Unicode: {}", path.display()))?;
    let select = select_argument(text)?;
    let mut cmd = new_command("explorer.exe");
    add_raw_argument(&mut cmd, &select);
    Ok(cmd)
}

#[cfg(windows)]
fn add_raw_argument(cmd: &mut Command, arg: &str) {
    use std::os::windows::process::CommandExt;
    cmd.raw_arg(arg);
}

#[cfg(not(windows))]
fn add_raw_argument(cmd: &mut Command, arg: &str) {
    cmd.arg(arg);
}

fn unix_opener() -> &'static str {
    if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    }
}

fn ensure_exists(path: &Path) -> Result<()> {
    match path.try_exists() {
        Ok(true) => Ok(()),
        Ok(false) => bail!("file not found: {}", path.display()),
        Err(err) => Err(err).with_context(|| format!("cannot access {}", path.display())),
    }
}

fn absolute(path: &Path) -> Result<PathBuf> {
    std::path::absolute(path).with_context(|| format!("cannot resolve {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args_of(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    fn file_in(dir: &tempfile::TempDir, name: &str) -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, b"%PDF-1.4").expect("write temp file");
        path
    }

    #[test]
    fn script_embeds_path_with_metacharacters_as_literal() {
        let script = shell_exec_script(r"C:\a&b^c%d\報告 'x'.pdf", &ShellAction::default());
        assert!(
            script.contains(r"$info.FileName = 'C:\a&b^c%d\報告 ''x''.pdf'"),
            "script was: {script}"
        );
    }

    #[test]
    fn open_action_is_not_hidden_and_has_no_verb() {
        let script = shell_exec_script("C:\\x.pdf", &ShellAction::default());
        assert!(!script.contains("WindowStyle"));
        assert!(!script.contains("$info.Verb"));
        assert!(script.contains("$info.UseShellExecute = $true"));
        assert!(script.contains("$ErrorActionPreference = 'Stop'"));
    }

    #[test]
    fn print_action_is_hidden_with_print_verb() {
        let action = ShellAction {
            verb: Some("print"),
            arguments: None,
            hidden: true,
        };
        let script = shell_exec_script("C:\\x.pdf", &action);
        assert!(script.contains("$info.WindowStyle = 'Hidden'"));
        assert!(script.contains("$info.Verb = 'print'"));
        assert!(!script.contains("$info.Arguments"));
    }

    #[test]
    fn printto_arguments_quote_printer_with_spaces_and_parentheses() {
        assert_eq!(
            printto_arguments("EPSON Fax (Network) 2"),
            "\"EPSON Fax (Network) 2\""
        );
    }

    #[test]
    fn printto_script_carries_quoted_printer_as_literal() {
        let action = ShellAction {
            verb: Some("printto"),
            arguments: Some(printto_arguments("O'Neil 傳真 (FAX)")),
            hidden: true,
        };
        let script = shell_exec_script("C:\\x.pdf", &action);
        assert!(
            script.contains("$info.Arguments = '\"O''Neil 傳真 (FAX)\"'"),
            "script was: {script}"
        );
        assert!(script.contains("$info.Verb = 'printto'"));
    }

    #[test]
    fn print_to_rejects_double_quote_in_printer_name() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = file_in(&dir, "a.pdf");
        let err = print_to(&file, "EPSON \"FAX\"").expect_err("quote must be rejected");
        assert!(err.to_string().contains("double quote"));
    }

    #[test]
    fn print_to_rejects_control_characters_and_empty_printer() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = file_in(&dir, "a.pdf");
        assert!(print_to(&file, "bad\nname").is_err());
        assert!(print_to(&file, "  ").is_err());
    }

    #[test]
    fn open_missing_file_errors_without_spawning() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("missing.pdf");
        let err = open(&missing).expect_err("missing file must fail");
        assert!(err.to_string().contains("file not found"));
    }

    #[test]
    fn reveal_print_and_print_to_missing_file_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("missing.pdf");
        assert!(reveal(&missing).is_err());
        assert!(print(&missing).is_err());
        assert!(print_to(&missing, "Printer").is_err());
    }

    #[test]
    fn select_argument_quotes_path_and_rejects_quote() {
        assert_eq!(
            select_argument(r"C:\a b\報告.pdf").expect("plain path"),
            "/select,\"C:\\a b\\報告.pdf\""
        );
        assert!(select_argument("C:\\bad\"name").is_err());
    }

    #[test]
    fn print_to_command_builds_expected_program() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = file_in(&dir, "a.pdf");
        let cmd = print_to_command(&file, "FAX 2").expect("command");
        if cfg!(windows) {
            assert_eq!(cmd.get_program().to_string_lossy(), "powershell.exe");
        } else {
            assert_eq!(cmd.get_program().to_string_lossy(), "lp");
            let args = args_of(&cmd);
            assert_eq!(args.len(), 3);
            assert_eq!(args[0], "-d");
            assert_eq!(args[1], "FAX 2");
        }
    }

    #[test]
    fn reveal_command_for_file_uses_select_argument_on_windows() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = file_in(&dir, "a b.pdf");
        let cmd = reveal_command(&file).expect("command");
        if cfg!(windows) {
            assert_eq!(cmd.get_program().to_string_lossy(), "explorer.exe");
            let args = args_of(&cmd);
            assert_eq!(args.len(), 1);
            assert!(args[0].starts_with("/select,\""), "args: {args:?}");
        }
    }
}
