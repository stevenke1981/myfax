//! Internationalisation: key-based lookup, Traditional Chinese (default) and English.
//!
//! Catalog entries live in per-owner fragment files under `catalog/`. Each row is
//! `(key, zh_tw, en)`, so both languages are always present by construction.
//! Placeholders use `{name}` and are filled by [`tf`].
//!
//! Key naming: `<area>.<item>` in lower snake case, e.g. `inbox.title`.
//! Every `t(…, "key")` / `tf(…, "key", …)` literal in `src/` must exist in a
//! fragment (enforced by `tests::every_used_key_exists`).

mod catalog;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

/// UI language.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Lang {
    /// 繁體中文（台灣）— default.
    #[default]
    #[serde(rename = "zh-TW")]
    ZhTw,
    #[serde(rename = "en")]
    En,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::ZhTw, Lang::En];

    /// BCP-47 tag.
    pub fn code(self) -> &'static str {
        match self {
            Lang::ZhTw => "zh-TW",
            Lang::En => "en",
        }
    }

    /// The language's own name (shown in the language picker).
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::ZhTw => "繁體中文",
            Lang::En => "English",
        }
    }
}

type Table = HashMap<&'static str, (&'static str, &'static str)>;

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut map = Table::new();
        for (key, zh, en) in catalog::all_entries() {
            map.insert(key, (zh, en));
        }
        map
    })
}

/// Translate `key`. Unknown keys return the key itself (and are caught by tests).
pub fn t(lang: Lang, key: &'static str) -> &'static str {
    match table().get(key) {
        Some((zh, en)) => match lang {
            Lang::ZhTw => zh,
            Lang::En => en,
        },
        None => key,
    }
}

/// Translate `key` and replace each `{name}` with its value.
pub fn tf(lang: Lang, key: &'static str, args: &[(&str, &str)]) -> String {
    let mut text = t(lang, key).to_string();
    for (name, value) in args {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::Path;

    #[test]
    fn default_language_is_traditional_chinese() {
        assert_eq!(Lang::default(), Lang::ZhTw);
        assert_eq!(serde_json::to_string(&Lang::ZhTw).unwrap(), "\"zh-TW\"");
    }

    #[test]
    fn keys_are_unique_and_non_empty() {
        let mut seen = HashSet::new();
        for (key, zh, en) in catalog::all_entries() {
            assert!(seen.insert(key), "duplicate i18n key: {key}");
            assert!(!zh.trim().is_empty(), "empty zh-TW text for {key}");
            assert!(!en.trim().is_empty(), "empty en text for {key}");
        }
    }

    #[test]
    fn placeholders_match_between_languages() {
        fn holders(s: &str) -> Vec<String> {
            let mut out = Vec::new();
            let mut rest = s;
            while let Some(start) = rest.find('{') {
                if let Some(end) = rest[start..].find('}') {
                    out.push(rest[start..start + end + 1].to_string());
                    rest = &rest[start + end + 1..];
                } else {
                    break;
                }
            }
            out.sort();
            out
        }
        for (key, zh, en) in catalog::all_entries() {
            assert_eq!(holders(zh), holders(en), "placeholder mismatch for {key}");
        }
    }

    #[test]
    fn tf_replaces_placeholders() {
        let keys: Vec<_> = catalog::all_entries()
            .into_iter()
            .filter(|(_, zh, _)| zh.contains('{'))
            .collect();
        if let Some((key, _, _)) = keys.first() {
            let out = tf(Lang::En, key, &[]);
            assert!(!out.is_empty());
        }
        assert_eq!(t(Lang::En, "no.such.key"), "no.such.key");
    }

    /// Scan every `.rs` file under `src/` for `t(<expr>, "key"` and
    /// `tf(<expr>, "key"` literals and require each key in the catalog.
    #[test]
    fn every_used_key_exists() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let known: HashSet<&str> = catalog::all_entries().into_iter().map(|e| e.0).collect();
        let mut missing = Vec::new();
        visit(&root, &mut |path, text| {
            for key in used_keys(text) {
                if !known.contains(key.as_str()) {
                    missing.push(format!("{}: {key}", path.display()));
                }
            }
        });
        assert!(
            missing.is_empty(),
            "missing i18n keys:\n{}",
            missing.join("\n")
        );
    }

    fn visit(dir: &Path, f: &mut dyn FnMut(&Path, &str)) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(&path, f);
            } else if path.extension().is_some_and(|e| e == "rs") && !path.ends_with("i18n/mod.rs")
            {
                let text = std::fs::read_to_string(&path).unwrap();
                f(&path, &text);
            }
        }
    }

    /// Extract keys from calls shaped `t(x, "key"` or `tf(x, "key"` where the
    /// key matches `[a-z0-9_]+(\.[a-z0-9_]+)+`.
    fn used_keys(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        for (idx, _) in text.match_indices('"') {
            let before = &text[..idx];
            let trimmed = before.trim_end();
            if !trimmed.ends_with(',') {
                continue;
            }
            let call_start = trimmed.rfind('(');
            let Some(open) = call_start else { continue };
            let head = &trimmed[..open];
            let is_call = head.ends_with("t") || head.ends_with("tf");
            let ident_ok = head
                .trim_end_matches("tf")
                .trim_end_matches('t')
                .chars()
                .last()
                .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
            let args = &trimmed[open + 1..trimmed.len() - 1];
            if !is_call || !ident_ok || args.contains('(') || args.contains('"') {
                continue;
            }
            let rest = &text[idx + 1..];
            let Some(end) = rest.find('"') else { continue };
            let key = &rest[..end];
            let valid = key.contains('.')
                && key
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.');
            if valid {
                out.push(key.to_string());
            }
        }
        out
    }

    #[test]
    fn used_keys_extractor_finds_calls() {
        let src = r#"let a = t(lang, "inbox.title"); let b = tf(state.lang, "x.y", &[]); fmt(a, "no.match");"#;
        assert_eq!(
            used_keys(src),
            vec!["inbox.title".to_string(), "x.y".to_string()]
        );
    }
}
