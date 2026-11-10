//! Process helpers: every child process is created without a console window,
//! detached launches are watched on a background thread (never blocking the UI),
//! and captured runs return their stdout as text.

use std::ffi::OsStr;
use std::io::Read;
use std::process::{Child, Command, Stdio};

use anyhow::{Context, Result, bail};

/// `CREATE_NO_WINDOW`: console programs get no visible console window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Build a `Command` that never flashes a console window on Windows.
pub fn new_command(program: impl AsRef<OsStr>) -> Command {
    let mut cmd = Command::new(program);
    apply_no_window(&mut cmd);
    cmd
}

#[cfg(windows)]
fn apply_no_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn apply_no_window(_cmd: &mut Command) {}

/// Start a process and return immediately. A background thread reads stderr and
/// waits for the exit status so no zombie is left behind; failures are logged.
/// `check_status` = false for launchers that report success with a non-zero code.
pub fn spawn_detached(mut cmd: Command, label: &'static str, check_status: bool) -> Result<()> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let child = cmd
        .spawn()
        .with_context(|| format!("failed to start {label}"))?;
    std::thread::Builder::new()
        .name(format!("myfax-launch-{label}"))
        .spawn(move || watch_child(child, label, check_status))
        .with_context(|| format!("failed to watch {label}"))?;
    Ok(())
}

fn watch_child(mut child: Child, label: &'static str, check_status: bool) {
    let mut stderr_bytes = Vec::new();
    if let Some(mut pipe) = child.stderr.take()
        && let Err(err) = pipe.read_to_end(&mut stderr_bytes)
    {
        tracing::debug!(launcher = label, error = %err, "could not read launcher stderr");
    }
    let stderr = String::from_utf8_lossy(&stderr_bytes);
    match child.wait() {
        Ok(status) if status.success() || !check_status => {
            tracing::debug!(launcher = label, %status, "system launcher finished");
        }
        Ok(status) => {
            tracing::warn!(
                launcher = label,
                %status,
                stderr = stderr.trim(),
                "system launcher reported failure"
            );
        }
        Err(err) => {
            tracing::warn!(launcher = label, error = %err, "failed to wait for system launcher");
        }
    }
}

/// Run a command to completion and return its stdout (lossy UTF-8).
pub fn run_capture(mut cmd: Command, label: &str) -> Result<String> {
    cmd.stdin(Stdio::null());
    let output = cmd
        .output()
        .with_context(|| format!("failed to run {label}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("{label} exited with {}: {}", output.status, stderr.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
