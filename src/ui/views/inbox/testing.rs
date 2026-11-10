//! Test helpers for the inbox and detail views: a tree walker that collects keys, labels and
//! reachable messages (a disabled button is not reachable), plus sample domain values.

use crate::app::{AppState, Msg};
use crate::config::Settings;
use crate::model::{FaxId, FaxRecord, FaxSource};
use acme_widgets::{ButtonVariant, WidgetNode};
use chrono::{Local, TimeZone};
use std::path::PathBuf;

/// Everything reachable in one rendered tree.
#[derive(Default)]
pub(crate) struct Walk {
    pub keys: Vec<String>,
    pub labels: Vec<String>,
    pub reachable: Vec<Msg>,
    pub variants: Vec<(String, ButtonVariant)>,
}

impl Walk {
    pub(crate) fn has_key(&self, key: &str) -> bool {
        self.keys.iter().any(|k| k == key)
    }

    pub(crate) fn has_key_prefix(&self, prefix: &str) -> bool {
        self.keys.iter().any(|k| k.starts_with(prefix))
    }

    pub(crate) fn can(&self, msg: &Msg) -> bool {
        self.reachable.iter().any(|m| m == msg)
    }

    pub(crate) fn has_label(&self, text: &str) -> bool {
        self.labels.iter().any(|l| l == text)
    }

    pub(crate) fn label_count(&self, text: &str) -> usize {
        self.labels.iter().filter(|l| l.as_str() == text).count()
    }

    pub(crate) fn variant(&self, key: &str) -> Option<ButtonVariant> {
        self.variants
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| *v)
    }
}

/// Walk a rendered tree and collect keys, label texts and reachable messages.
pub(crate) fn walk(root: &WidgetNode<Msg>) -> Walk {
    let mut out = Walk::default();
    visit(root, &mut out);
    out
}

fn visit(node: &WidgetNode<Msg>, out: &mut Walk) {
    match node {
        WidgetNode::Row(c) | WidgetNode::Column(c) | WidgetNode::Stack(c) => {
            if let Some(key) = &c.key {
                out.keys.push(key.as_str().to_string());
            }
            if let Some(msg) = &c.message {
                out.reachable.push(msg.clone());
            }
            for child in &c.children {
                visit(child, out);
            }
        }
        WidgetNode::Card(c) => {
            if let Some(key) = &c.key {
                out.keys.push(key.as_str().to_string());
            }
            for child in &c.children {
                visit(child, out);
            }
        }
        WidgetNode::Button(b) => {
            out.keys.push(b.key.as_str().to_string());
            out.variants.push((b.key.as_str().to_string(), b.variant));
            if let Some(msg) = b.activate() {
                out.reachable.push(msg.clone());
            }
        }
        WidgetNode::TextInput(input) => out.keys.push(input.key.as_str().to_string()),
        WidgetNode::Label(label) => out.labels.push(label.text.clone()),
        _ => {}
    }
}

/// Fax with a fixed received time (2026-03-05 14:07 local).
pub(crate) fn sample_fax(
    id: FaxId,
    sender: Option<&str>,
    is_read: bool,
    starred: bool,
) -> FaxRecord {
    let at = Local
        .with_ymd_and_hms(2026, 3, 5, 14, 7, 0)
        .single()
        .unwrap_or_else(Local::now);
    FaxRecord {
        id,
        received_at: at,
        sender_number: Some("+886212345678".to_string()),
        sender_name: sender.map(str::to_string),
        pages: 2,
        file_path: PathBuf::from(format!("archive/2026/fax-{id}.pdf")),
        file_size: 2048,
        sha256: String::new(),
        original_name: format!("fax-{id}.pdf"),
        source: FaxSource::WatchFolder,
        is_read,
        is_starred: starred,
        tags: vec!["invoice".to_string()],
        note: String::new(),
        deleted_at: None,
        imported_at: at,
    }
}

/// State with default settings and no data loaded.
pub(crate) fn empty_state() -> AppState {
    AppState::new(Settings::default())
}
