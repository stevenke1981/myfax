//! Send page (UI_SPEC §5): hand a document to a printer's fax driver and keep a
//! local record. Fax printers are listed first; contacts offer quick recipients.

use super::kit::{self, Tone, glyph, px, truncate_units};
use crate::app::{AppState, Field, Msg};
use crate::i18n::{Lang, t, tf};
use crate::model::{Contact, SentFax};
use crate::system;
use acme_widgets::style::ColorToken;
use acme_widgets::style::prelude::*;
use acme_widgets::{
    ButtonSize, ButtonVariant, CardVariant, WidgetNode, button, card, column, label_builder, row,
    text_input,
};
use std::path::Path;

const FORM_MIN_W: f32 = 420.0;
const HISTORY_W: f32 = 360.0;
const PRINTER_ROW_H: f32 = 36.0;
const PRINTER_NAME_UNITS: usize = 48;
const CONTACT_PICK_LIMIT: usize = 8;
const CONTACT_LIST_H: f32 = 132.0;
const HISTORY_LIMIT: usize = 100;
const HISTORY_FILE_UNITS: usize = 36;
const BAR_W: f32 = 3.0;

/// Whole page body (right of the nav rail).
pub fn view(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let header = kit::page_header(t(lang, "send.title"), None, Vec::new());
    let body = row::<Msg>()
        .key("sendbody")
        .flex_1()
        .flex_shrink(1.0)
        .min_h(crate::ui::views::kit::px(0.0))
        .child(form_column(state))
        .child(kit::vdivider())
        .child(history_column(state))
        .build();
    column::<Msg>()
        .key("page.send")
        .flex_1()
        .flex_shrink(1.0)
        .min_h(crate::ui::views::kit::px(0.0))
        .child(header)
        .child(body)
        .build()
}

fn form_column(state: &AppState) -> WidgetNode<Msg> {
    let printers = order_printers(&state.printers);
    column::<Msg>()
        .key("scroll.send_form")
        .flex_1()
        .flex_shrink(1.0)
        .min_h(crate::ui::views::kit::px(0.0))
        .min_w(px(FORM_MIN_W))
        .overflow_scroll()
        .p(24.0)
        .gap(16.0)
        .child(explain_card(state.lang))
        .child(printer_section(state, &printers))
        .child(file_section(state))
        .child(recipient_section(state))
        .child(go_section(state))
        .build()
}

fn explain_card(lang: Lang) -> WidgetNode<Msg> {
    card::<Msg>()
        .key("send.explain")
        .variant(CardVariant::Outlined)
        .padding(16.0)
        .gap(8.0)
        .bg(ColorToken::InfoSoft)
        .child(kit::section(t(lang, "send.explain_title")))
        .child(kit::body_wrap(t(lang, "send.explain_body")))
        .build()
}

fn printer_section(state: &AppState, printers: &[String]) -> WidgetNode<Msg> {
    let lang = state.lang;
    let heading = row::<Msg>()
        .gap(8.0)
        .child(kit::section(t(lang, "send.section_printer")))
        .child(kit::spacer())
        .child(
            button("btn.refresh_printers", t(lang, "common.refresh"))
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Small)
                .on_click(Msg::RefreshPrinters),
        )
        .build();
    let mut section = column::<Msg>().gap(8.0).child(heading);
    if printers.is_empty() {
        let hint = card::<Msg>()
            .variant(CardVariant::Muted)
            .padding(12.0)
            .child(kit::caption_wrap(t(lang, "send.printers_empty")))
            .build();
        section = section.child(hint);
    }
    for (index, name) in printers.iter().enumerate() {
        section = section.child(printer_row(state, index, name));
    }
    section.build()
}

fn printer_row(state: &AppState, index: usize, name: &str) -> WidgetNode<Msg> {
    let lang = state.lang;
    let selected = state.selected_printer.as_deref() == Some(name);
    let mut bar = column::<Msg>()
        .w(px(BAR_W))
        .h(crate::ui::views::kit::full());
    let mut line = row::<Msg>()
        .key(format!("printer.{index}"))
        .h(px(PRINTER_ROW_H))
        .px(8.0)
        .gap(8.0)
        .rounded_md()
        .on_click(Msg::SelectPrinter(name.to_string()));
    if selected {
        bar = bar.bg(ColorToken::Primary);
        line = line.bg(ColorToken::Accent);
    }
    let radio = if selected {
        glyph::RADIO_ON
    } else {
        glyph::RING
    };
    let radio_label = label_builder::<Msg>(radio)
        .font_size(14.0)
        .text_color(ColorToken::Primary)
        .build();
    let mut name_label = label_builder::<Msg>(truncate_units(name, PRINTER_NAME_UNITS))
        .font_size(14.0)
        .flex_1()
        .flex_shrink(1.0);
    if selected {
        name_label = name_label.text_color(ColorToken::AccentForeground);
    }
    line = line
        .child(bar.build())
        .child(radio_label)
        .child(name_label.build());
    if is_fax(name) {
        line = line.child(kit::badge(t(lang, "send.printer_fax_badge"), Tone::Info));
    }
    line.build()
}

fn file_section(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let input = text_input::<Msg>(Field::SendFile.key())
        .flex_1()
        .flex_shrink(1.0)
        .placeholder(t(lang, "send.file_ph"))
        .value(state.field(Field::SendFile))
        .build();
    let picker = button("btn.browse.send_file", t(lang, "common.browse"))
        .variant(ButtonVariant::Secondary)
        .size(ButtonSize::Small)
        .on_click(Msg::Browse(Field::SendFile));
    column::<Msg>()
        .gap(8.0)
        .child(kit::section(t(lang, "send.section_file")))
        .child(row::<Msg>().gap(8.0).child(input).child(picker).build())
        .build()
}

fn recipient_section(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let recipient = text_input::<Msg>(Field::SendRecipient.key())
        .placeholder(t(lang, "send.recipient_ph"))
        .value(state.field(Field::SendRecipient))
        .build();
    let note = text_input::<Msg>(Field::SendNote.key())
        .placeholder(t(lang, "send.note_ph"))
        .value(state.field(Field::SendNote))
        .build();
    column::<Msg>()
        .gap(8.0)
        .child(kit::section(t(lang, "send.section_recipient")))
        .child(recipient)
        .child(kit::caption(t(lang, "send.pick_contact")))
        .child(contact_pick_list(state))
        .child(note)
        .build()
}

/// Quick-pick list: the first contacts that have a fax number.
fn contact_pick_list(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let picks = quick_pick(&state.contacts);
    let mut list = column::<Msg>()
        .key("scroll.send_contacts")
        .h(px(CONTACT_LIST_H))
        .overflow_scroll()
        .border(1.0, ColorToken::Border)
        .rounded_md();
    if picks.is_empty() {
        let hint = column::<Msg>()
            .p(12.0)
            .child(kit::caption_wrap(t(lang, "send.contacts_empty")))
            .build();
        list = list.child(hint);
    }
    for contact in picks {
        list = list.child(pick_row(contact));
    }
    list.build()
}

fn pick_row(contact: &Contact) -> WidgetNode<Msg> {
    row::<Msg>()
        .key(format!("sc.{}", contact.id))
        .h(px(36.0))
        .px(12.0)
        .gap(8.0)
        .on_click(Msg::UseContactForSend(contact.id))
        .child(
            label_builder::<Msg>(truncate_units(&contact.name, 28))
                .font_size(14.0)
                .flex_1()
                .flex_shrink(1.0)
                .build(),
        )
        .child(kit::caption(&contact.fax_number))
        .build()
}

fn go_section(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let has_printer = state.selected_printer.is_some();
    let has_file = !state.field(Field::SendFile).trim().is_empty();
    let mut section = column::<Msg>().gap(8.0);
    if !has_printer {
        section = section.child(kit::colored(
            t(lang, "send.need_printer"),
            12.0,
            ColorToken::Warning,
        ));
    }
    if !has_file {
        section = section.child(kit::colored(
            t(lang, "send.need_file"),
            12.0,
            ColorToken::Warning,
        ));
    }
    let send = button("btn.send_fax", t(lang, "send.button"))
        .variant(ButtonVariant::Primary)
        .size(ButtonSize::Large)
        .leading_icon(glyph::NAV_SEND)
        .disabled(!(has_printer && has_file))
        .on_click(Msg::SendFax);
    section.child(row::<Msg>().child(send).build()).build()
}

fn history_column(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let mut column_builder = column::<Msg>()
        .key("scroll.send_history")
        .w(px(HISTORY_W))
        .overflow_scroll()
        .p(16.0)
        .gap(8.0)
        .bg(ColorToken::Surface)
        .child(kit::section(t(lang, "send.history_title")));
    if state.sent.is_empty() {
        column_builder = column_builder.child(kit::caption(t(lang, "send.history_empty")));
    }
    for sent in state.sent.iter().take(HISTORY_LIMIT) {
        column_builder = column_builder.child(history_card(lang, sent));
    }
    column_builder.build()
}

fn history_card(lang: Lang, sent: &SentFax) -> WidgetNode<Msg> {
    let file = truncate_units(&file_name(&sent.file_path), HISTORY_FILE_UNITS);
    let via = tf(
        lang,
        "send.history_via",
        &[("printer", sent.printer.as_str())],
    );
    let mut card_builder = card::<Msg>()
        .variant(CardVariant::Outlined)
        .padding(12.0)
        .gap(2.0)
        .child(kit::body_strong(&file))
        .child(kit::caption(&kit::fmt_dt(sent.at)))
        .child(kit::caption(&via));
    if !sent.recipient.trim().is_empty() {
        let to = tf(
            lang,
            "send.history_to",
            &[("recipient", sent.recipient.as_str())],
        );
        card_builder = card_builder.child(kit::caption(&to));
    }
    if !sent.note.trim().is_empty() {
        card_builder = card_builder.child(kit::caption_wrap(&sent.note));
    }
    card_builder.build()
}

/// Whether a printer name looks like a fax driver (case-insensitive "fax").
fn is_fax(name: &str) -> bool {
    name.to_lowercase().contains("fax")
}

/// Fax printers first, then the rest; each group keeps its input order.
fn order_printers(all: &[String]) -> Vec<String> {
    let fax = system::fax_printers(all);
    let rest = all
        .iter()
        .filter(|printer| !fax.contains(*printer))
        .cloned();
    fax.iter().cloned().chain(rest).collect()
}

/// First contacts that have a fax number, at most [`CONTACT_PICK_LIMIT`].
fn quick_pick(contacts: &[Contact]) -> Vec<&Contact> {
    contacts
        .iter()
        .filter(|contact| !contact.fax_number.trim().is_empty())
        .take(CONTACT_PICK_LIMIT)
        .collect()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;
    use acme_widgets::{Card, ScrollView};
    use chrono::{Local, TimeZone};
    use std::collections::HashSet;
    use std::path::PathBuf;

    #[derive(Default)]
    struct Walk {
        keys: Vec<String>,
        msgs: Vec<Msg>,
    }

    fn walk(node: &WidgetNode<Msg>, out: &mut Walk) {
        if let Some(key) = node.key() {
            out.keys.push(key.as_str().to_string());
        }
        match node {
            WidgetNode::Row(c) | WidgetNode::Column(c) | WidgetNode::Stack(c) => {
                if let Some(msg) = &c.message {
                    out.msgs.push(msg.clone());
                }
                c.children.iter().for_each(|child| walk(child, out));
            }
            WidgetNode::Card(Card { children, .. }) => {
                children.iter().for_each(|child| walk(child, out));
            }
            WidgetNode::ScrollView(ScrollView { children, .. }) => {
                children.iter().for_each(|child| walk(child, out));
            }
            WidgetNode::Button(b) => {
                if let Some(msg) = b.activate() {
                    out.msgs.push(msg.clone());
                }
            }
            _ => {}
        }
    }

    fn collect(state: &AppState) -> Walk {
        let tree = view(state);
        let mut out = Walk::default();
        walk(&tree, &mut out);
        out
    }

    fn state(printers: &[&str], selected: Option<&str>, file: &str) -> AppState {
        let mut state = AppState::new(Settings::default());
        state.printers = printers.iter().map(|p| p.to_string()).collect();
        state.selected_printer = selected.map(str::to_string);
        state.fields.insert(Field::SendFile, file.to_string());
        state
    }

    fn contact(id: i64, name: &str, fax: &str) -> Contact {
        Contact {
            id,
            name: name.into(),
            fax_number: fax.into(),
            ..Contact::default()
        }
    }

    #[test]
    fn fax_printers_are_ordered_first_and_stable() {
        let all: Vec<String> = ["HP LaserJet", "EPSON FAX", "Canon", "fax-2", "Brother"]
            .iter()
            .map(|p| p.to_string())
            .collect();
        assert_eq!(
            order_printers(&all),
            vec!["EPSON FAX", "fax-2", "HP LaserJet", "Canon", "Brother"]
        );
        assert!(order_printers(&[]).is_empty());
    }

    #[test]
    fn send_fax_is_not_reachable_without_printer() {
        let out = collect(&state(&["EPSON FAX"], None, "C:/a.pdf"));
        assert!(!out.msgs.contains(&Msg::SendFax));
    }

    #[test]
    fn send_fax_is_not_reachable_without_file() {
        let out = collect(&state(&["EPSON FAX"], Some("EPSON FAX"), "   "));
        assert!(!out.msgs.contains(&Msg::SendFax));
    }

    #[test]
    fn send_fax_is_reachable_with_printer_and_file() {
        let out = collect(&state(&["EPSON FAX"], Some("EPSON FAX"), "C:/a.pdf"));
        assert!(out.msgs.contains(&Msg::SendFax));
    }

    #[test]
    fn printer_rows_emit_select_in_fax_first_order() {
        let out = collect(&state(&["HP", "EPSON FAX"], None, ""));
        let selects: Vec<&Msg> = out
            .msgs
            .iter()
            .filter(|m| matches!(m, Msg::SelectPrinter(_)))
            .collect();
        assert_eq!(
            selects,
            vec![
                &Msg::SelectPrinter("EPSON FAX".into()),
                &Msg::SelectPrinter("HP".into())
            ]
        );
        assert!(out.keys.contains(&"printer.0".to_string()));
    }

    #[test]
    fn empty_printer_list_offers_refresh() {
        let out = collect(&state(&[], None, ""));
        assert!(out.msgs.contains(&Msg::RefreshPrinters));
        assert!(!out.msgs.iter().any(|m| matches!(m, Msg::SelectPrinter(_))));
    }

    #[test]
    fn contact_quick_pick_only_lists_contacts_with_numbers() {
        let mut st = state(&[], None, "");
        st.contacts = vec![
            contact(1, "No Fax", "  "),
            contact(2, "Has Fax", "0223456789"),
        ];
        let out = collect(&st);
        assert!(out.msgs.contains(&Msg::UseContactForSend(2)));
        assert!(!out.msgs.contains(&Msg::UseContactForSend(1)));
    }

    #[test]
    fn contact_quick_pick_is_limited_to_eight() {
        let mut st = state(&[], None, "");
        st.contacts = (1..=10).map(|id| contact(id, "X", "0223456789")).collect();
        let picks = quick_pick(&st.contacts);
        assert_eq!(picks.len(), CONTACT_PICK_LIMIT);
        assert_eq!(picks.first().map(|c| c.id), Some(1));
    }

    #[test]
    fn send_history_renders_sent_entries() {
        let mut st = state(&[], None, "");
        st.sent = vec![SentFax {
            id: 1,
            at: Local.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap(),
            file_path: PathBuf::from("C:/docs/offer.pdf"),
            printer: "EPSON FAX".into(),
            recipient: "Bob".into(),
            note: "urgent".into(),
        }];
        let out = collect(&st);
        assert!(out.keys.contains(&"scroll.send_history".to_string()));
        assert!(!out.keys.contains(&"send.history_empty".to_string()));
    }

    #[test]
    fn file_name_takes_last_component() {
        assert_eq!(file_name(Path::new("C:/docs/offer.pdf")), "offer.pdf");
        assert_eq!(file_name(Path::new("")), "");
    }

    #[test]
    fn keys_are_unique_within_one_frame() {
        let mut st = state(&["EPSON FAX", "HP"], Some("HP"), "C:/a.pdf");
        st.contacts = vec![contact(1, "A", "1"), contact(2, "B", "2")];
        let out = collect(&st);
        let mut seen = HashSet::new();
        for key in &out.keys {
            assert!(seen.insert(key.clone()), "duplicate key {key}");
        }
    }

    #[test]
    fn view_builds_in_english() {
        let mut st = state(&["EPSON FAX"], Some("EPSON FAX"), "C:/a.pdf");
        st.lang = Lang::En;
        assert!(!collect(&st).keys.is_empty());
    }
}
