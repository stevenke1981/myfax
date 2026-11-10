//! Printer enumeration, output parsing and printer-name validation.

use std::io::ErrorKind;

use anyhow::{Context, Result, bail};

use super::command::{new_command, run_capture};
use super::ps::powershell_command;

/// Console encoding is forced to UTF-8 first so Chinese printer names survive
/// the pipe; `$ErrorActionPreference` makes a WMI failure a non-zero exit.
const LIST_SCRIPT: &str = "[Console]::OutputEncoding = [Text.Encoding]::UTF8\n\
$ErrorActionPreference = 'Stop'\n\
Get-CimInstance Win32_Printer | ForEach-Object { $_.Name }";

/// Installed printer names, sorted and de-duplicated.
pub fn installed() -> Result<Vec<String>> {
    if cfg!(windows) {
        let stdout = run_capture(powershell_command(LIST_SCRIPT)?, "printer listing")?;
        Ok(parse_printer_names(&stdout))
    } else {
        installed_with_lpstat()
    }
}

/// CUPS `lpstat -e` lists destinations one per line. Without CUPS (or with no
/// destinations) the result is an empty list, not an error.
fn installed_with_lpstat() -> Result<Vec<String>> {
    let mut cmd = new_command("lpstat");
    cmd.arg("-e");
    match cmd.output() {
        Ok(output) if output.status.success() => Ok(parse_printer_names(&String::from_utf8_lossy(
            &output.stdout,
        ))),
        Ok(output) => {
            tracing::debug!(status = %output.status, "lpstat reported no destinations");
            Ok(Vec::new())
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(err).context("failed to run lpstat"),
    }
}

/// Parse one printer name per line: handles CRLF, a UTF-8 BOM, blank lines,
/// surrounding whitespace, duplicates; the result is sorted.
pub fn parse_printer_names(text: &str) -> Vec<String> {
    let mut names: Vec<String> = text
        .trim_start_matches('\u{feff}')
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Reject names that cannot be passed safely inside the `"..."` printto argument.
pub fn validate_printer_name(name: &str) -> Result<&str> {
    if name.trim().is_empty() {
        bail!("printer name is empty");
    }
    if name.contains('"') {
        bail!("printer name contains a double quote");
    }
    if name.chars().any(char::is_control) {
        bail!("printer name contains a control character");
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_handles_crlf_blanks_duplicates_and_sorting() {
        let text = "Zebra\r\n\r\n  Epson FAX  \r\nAlpha\r\nEpson FAX\r\n\r\n";
        assert_eq!(
            parse_printer_names(text),
            vec!["Alpha", "Epson FAX", "Zebra"]
        );
    }

    #[test]
    fn parse_strips_utf8_bom_and_keeps_chinese_names() {
        let text = "\u{feff}傳真機\r\nEPSON 印表機 (FAX)\r\n";
        assert_eq!(
            parse_printer_names(text),
            vec!["EPSON 印表機 (FAX)", "傳真機"]
        );
    }

    #[test]
    fn parse_empty_output_gives_empty_list() {
        assert!(parse_printer_names("").is_empty());
        assert!(parse_printer_names(" \r\n\n").is_empty());
    }

    #[test]
    fn list_script_sets_utf8_console_first() {
        assert!(LIST_SCRIPT.starts_with("[Console]::OutputEncoding"));
        assert!(LIST_SCRIPT.contains("Win32_Printer"));
    }

    #[test]
    fn validate_accepts_spaces_and_parentheses() {
        assert_eq!(
            validate_printer_name("EPSON Fax (Network) 2").ok(),
            Some("EPSON Fax (Network) 2")
        );
    }

    #[test]
    fn validate_rejects_double_quote() {
        assert!(validate_printer_name("bad\"name").is_err());
    }

    #[test]
    fn validate_rejects_control_characters_and_empty_names() {
        assert!(validate_printer_name("line\nbreak").is_err());
        assert!(validate_printer_name("tab\tname").is_err());
        assert!(validate_printer_name("").is_err());
        assert!(validate_printer_name("   ").is_err());
    }
}
