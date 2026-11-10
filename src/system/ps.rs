//! PowerShell helpers: single-quoted literals and `-EncodedCommand` payloads
//! (UTF-16LE then base64). Pure functions, unit-tested on every OS.

use std::process::Command;

use anyhow::{Result, bail};

use super::command::new_command;

/// Upper bound for the encoded script; Windows caps a command line at 32767 chars.
pub const MAX_ENCODED_COMMAND_CHARS: usize = 30_000;

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Characters PowerShell accepts as a single-quote delimiter (ASCII and curly).
fn is_powershell_single_quote(ch: char) -> bool {
    matches!(ch, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}')
}

/// Wrap `value` as a PowerShell single-quoted literal. Every single-quote-like
/// character is doubled, so no part of `value` is ever interpreted as code.
pub fn literal(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('\'');
    for ch in value.chars() {
        if is_powershell_single_quote(ch) {
            out.push(ch);
        }
        out.push(ch);
    }
    out.push('\'');
    out
}

/// Standard base64 (RFC 4648, with padding).
pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk.first().copied().unwrap_or(0) as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(sextet(n >> 18));
        out.push(sextet(n >> 12));
        out.push(if chunk.len() > 1 { sextet(n >> 6) } else { '=' });
        out.push(if chunk.len() > 2 { sextet(n) } else { '=' });
    }
    out
}

fn sextet(bits: u32) -> char {
    let index = (bits & 0x3F) as usize;
    BASE64_ALPHABET.get(index).copied().map_or('=', char::from)
}

/// UTF-16 little-endian bytes, the encoding `-EncodedCommand` expects.
pub fn utf16le_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// Payload for `powershell -EncodedCommand`.
pub fn encoded_command(script: &str) -> String {
    base64_encode(&utf16le_bytes(script))
}

/// `powershell.exe` invocation running `script` hidden and without a profile.
pub fn powershell_command(script: &str) -> Result<Command> {
    let encoded = encoded_command(script);
    if encoded.len() > MAX_ENCODED_COMMAND_CHARS {
        bail!(
            "PowerShell script too long for a command line ({} chars)",
            encoded.len()
        );
    }
    let mut cmd = new_command("powershell.exe");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-WindowStyle",
        "Hidden",
        "-EncodedCommand",
        encoded.as_str(),
    ]);
    Ok(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_wraps_plain_text_in_single_quotes() {
        assert_eq!(literal("abc"), "'abc'");
    }

    #[test]
    fn literal_doubles_apostrophes() {
        assert_eq!(literal("O'Brien.pdf"), "'O''Brien.pdf'");
    }

    #[test]
    fn literal_doubles_curly_single_quotes() {
        assert_eq!(literal("a\u{2019}b"), "'a\u{2019}\u{2019}b'");
        assert_eq!(literal("a\u{2018}b"), "'a\u{2018}\u{2018}b'");
    }

    #[test]
    fn literal_keeps_chinese_spaces_and_metacharacters() {
        let value = "傳真 & ^ % $(x) `n \"q\" [a]*";
        assert_eq!(literal(value), format!("'{value}'"));
    }

    #[test]
    fn base64_matches_rfc4648_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn utf16le_of_ascii_interleaves_zero_bytes() {
        assert_eq!(utf16le_bytes("ab"), vec![0x61, 0x00, 0x62, 0x00]);
    }

    #[test]
    fn utf16le_of_cjk_uses_little_endian_code_unit() {
        // U+50B3 (傳) -> 0xB3 0x50
        assert_eq!(utf16le_bytes("傳"), vec![0xB3, 0x50]);
    }

    #[test]
    fn encoded_command_matches_powershell_vector() {
        // [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes('ab')) == YQBiAA==
        assert_eq!(encoded_command("ab"), "YQBiAA==");
    }

    #[test]
    fn powershell_command_passes_encoded_payload_as_argument() {
        let cmd = powershell_command("ab").expect("short script fits");
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args.last().map(String::as_str), Some("YQBiAA=="));
        assert!(args.iter().any(|a| a == "-EncodedCommand"));
    }

    #[test]
    fn powershell_command_rejects_oversized_script() {
        let script = "x".repeat(MAX_ENCODED_COMMAND_CHARS);
        assert!(powershell_command(&script).is_err());
    }
}
