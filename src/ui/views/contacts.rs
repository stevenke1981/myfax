//! Contacts page (UI_SPEC §4): searchable list on the left, editor on the right,
//! CSV import/export block at the bottom of the list column.

use super::kit::{self, glyph, px};
use crate::app::{AppState, Field, Msg};
use crate::i18n::{Lang, t, tf};
use crate::model::{Contact, ContactId};
use acme_widgets::style::prelude::*;
use acme_widgets::style::{ColorToken, FontWeight};
use acme_widgets::{
    ButtonSize, ButtonVariant, WidgetNode, button, column, label_builder, row, text_input,
};

/// Id used by the controller for "creating a new contact" (`editing_contact == Some(0)`).
const NEW_CONTACT_ID: ContactId = 0;
const LIST_MIN_W: f32 = 360.0;
const EDITOR_W: f32 = 380.0;
const ROW_H: f32 = 56.0;
const BAR_W: f32 = 3.0;
const NAME_UNITS: usize = 28;
const DETAIL_UNITS: usize = 40;

/// Whole page body (right of the nav rail).
pub fn view(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let count = state.contacts.len().to_string();
    let header = kit::page_header(
        t(lang, "contacts.title"),
        Some(tf(lang, "contacts.count", &[("count", count.as_str())])),
        vec![
            button("btn.new_contact", t(lang, "contacts.new"))
                .variant(ButtonVariant::Primary)
                .size(ButtonSize::Small)
                .on_click(Msg::NewContact),
        ],
    );
    let body = row::<Msg>()
        .key("contactsbody")
        .flex_1()
        .child(list_column(state))
        .child(kit::vdivider())
        .child(editor_column(state))
        .build();
    column::<Msg>().flex_1().child(header).child(body).build()
}

fn list_column(state: &AppState) -> WidgetNode<Msg> {
    column::<Msg>()
        .flex_1()
        .min_w(px(LIST_MIN_W))
        .h_full()
        .bg(ColorToken::Background)
        .child(search_bar(state))
        .child(kit::hdivider())
        .child(contact_list(state))
        .child(kit::hdivider())
        .child(csv_block(state))
        .build()
}

fn search_bar(state: &AppState) -> WidgetNode<Msg> {
    let input = text_input::<Msg>(Field::ContactSearch.key())
        .placeholder(t(state.lang, "contacts.search_ph"))
        .value(state.field(Field::ContactSearch))
        .build();
    column::<Msg>().p(12.0).child(input).build()
}

fn contact_list(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let mut list = column::<Msg>()
        .key("scroll.contacts")
        .flex_1()
        .overflow_scroll();
    if state.contacts.is_empty() {
        list = list.child(kit::empty_state(
            glyph::NAV_CONTACTS,
            t(lang, "contacts.empty_title"),
            t(lang, "contacts.empty_hint"),
            None,
        ));
    } else {
        let shown = filter_contacts(&state.contacts, state.field(Field::ContactSearch));
        if shown.is_empty() {
            list = list.child(kit::empty_state(
                glyph::NAV_CONTACTS,
                t(lang, "contacts.no_match"),
                "",
                None,
            ));
        }
        for contact in shown {
            let selected = state.editing_contact == Some(contact.id);
            list = list.child(contact_row(lang, contact, selected));
        }
    }
    list.build()
}

/// One clickable contact row: bar, name + detail caption, quick "send" button.
fn contact_row(lang: Lang, contact: &Contact, selected: bool) -> WidgetNode<Msg> {
    let mut bar = column::<Msg>().w(px(BAR_W)).h_full();
    let mut line = row::<Msg>()
        .key(format!("row.contact.{}", contact.id))
        .h(px(ROW_H))
        .rounded_md()
        .on_click(Msg::EditContact(contact.id));
    if selected {
        bar = bar.bg(ColorToken::Primary);
        line = line.bg(ColorToken::Accent);
    }
    let detail = join_non_empty(
        &[contact.company.as_str(), contact.fax_number.as_str()],
        " · ",
    );
    let text_col = column::<Msg>()
        .flex_1()
        .px(12.0)
        .py(8.0)
        .gap(2.0)
        .child(name_label(&contact.name, selected))
        .child(detail_label(&detail, selected));
    let send = button(
        format!("btn.contact_send.{}", contact.id),
        t(lang, "contacts.send_to"),
    )
    .variant(ButtonVariant::Ghost)
    .size(ButtonSize::XS)
    .mr(12.0)
    .on_click(Msg::UseContactForSend(contact.id));
    line.child(bar.build())
        .child(text_col.build())
        .child(send)
        .build()
}

fn name_label(name: &str, selected: bool) -> WidgetNode<Msg> {
    let label = label_builder::<Msg>(truncate(name, NAME_UNITS))
        .font_size(14.0)
        .font_weight(FontWeight::SemiBold);
    if selected {
        label.text_color(ColorToken::AccentForeground).build()
    } else {
        label.build()
    }
}

fn detail_label(detail: &str, selected: bool) -> WidgetNode<Msg> {
    let label = label_builder::<Msg>(truncate(detail, DETAIL_UNITS)).font_size(12.0);
    if selected {
        label.text_color(ColorToken::AccentForeground).build()
    } else {
        label.text_muted().build()
    }
}

fn editor_column(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let body = match state.editing_contact {
        None => kit::empty_state(
            glyph::NAV_CONTACTS,
            t(lang, "contacts.editor_empty"),
            "",
            Some(
                button("btn.new_contact.empty", t(lang, "contacts.new"))
                    .variant(ButtonVariant::Primary)
                    .size(ButtonSize::Small)
                    .on_click(Msg::NewContact),
            ),
        ),
        Some(id) => editor(state, id),
    };
    column::<Msg>()
        .key("contacteditor")
        .w(px(EDITOR_W))
        .h_full()
        .bg(ColorToken::Surface)
        .child(body)
        .build()
}

/// Editor for `Some(id)`; `id == 0` means a new contact (no delete button).
fn editor(state: &AppState, id: ContactId) -> WidgetNode<Msg> {
    let lang = state.lang;
    let title = if id == NEW_CONTACT_ID {
        t(lang, "contacts.editor_new_title")
    } else {
        t(lang, "contacts.editor_edit_title")
    };
    let header = row::<Msg>()
        .h(px(40.0))
        .px(16.0)
        .child(kit::section(title))
        .build();
    let fields = column::<Msg>()
        .key("scroll.contact_editor")
        .flex_1()
        .overflow_scroll()
        .p(16.0)
        .gap(12.0)
        .child(input_field(
            state,
            t(lang, "contacts.field_name"),
            Field::ContactName,
        ))
        .child(input_field(
            state,
            t(lang, "contacts.field_company"),
            Field::ContactCompany,
        ))
        .child(fax_field(state))
        .child(input_field(
            state,
            t(lang, "contacts.field_phone"),
            Field::ContactPhone,
        ))
        .child(input_field(
            state,
            t(lang, "contacts.field_email"),
            Field::ContactEmail,
        ))
        .child(input_field(
            state,
            t(lang, "contacts.field_note"),
            Field::ContactNote,
        ))
        .build();
    column::<Msg>()
        .flex_1()
        .child(header)
        .child(kit::hdivider())
        .child(fields)
        .child(kit::hdivider())
        .child(editor_buttons(lang, id))
        .build()
}

fn input_field(state: &AppState, label: &str, field: Field) -> WidgetNode<Msg> {
    let input = text_input::<Msg>(field.key())
        .value(state.field(field))
        .build();
    kit::field(label, input)
}

fn fax_field(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let input = text_input::<Msg>(Field::ContactFax.key())
        .value(state.field(Field::ContactFax))
        .build();
    let input_with_hint = column::<Msg>()
        .gap(4.0)
        .child(input)
        .child(kit::caption(t(lang, "contacts.fax_hint")))
        .build();
    kit::field(t(lang, "contacts.field_fax"), input_with_hint)
}

fn editor_buttons(lang: Lang, id: ContactId) -> WidgetNode<Msg> {
    let mut line = row::<Msg>()
        .p(12.0)
        .gap(8.0)
        .child(
            button("btn.contact_save", t(lang, "common.save"))
                .variant(ButtonVariant::Primary)
                .size(ButtonSize::Small)
                .on_click(Msg::SaveContact),
        )
        .child(
            button("btn.contact_cancel", t(lang, "common.cancel"))
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Small)
                .on_click(Msg::CancelContactEdit),
        )
        .child(kit::spacer());
    if id != NEW_CONTACT_ID {
        line = line.child(
            button("btn.contact_delete", t(lang, "common.delete"))
                .variant(ButtonVariant::Danger)
                .size(ButtonSize::Small)
                .on_click(Msg::AskDeleteContact(id)),
        );
    }
    line.build()
}

/// CSV import/export controls for the contacts file at `Field::CsvPath`.
fn csv_block(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let path_row = row::<Msg>()
        .gap(8.0)
        .child(
            text_input::<Msg>(Field::CsvPath.key())
                .flex_1()
                .placeholder(t(lang, "contacts.csv_ph"))
                .value(state.field(Field::CsvPath))
                .build(),
        )
        .child(
            button("btn.browse.csv_path", t(lang, "common.browse"))
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Small)
                .on_click(Msg::Browse(Field::CsvPath)),
        )
        .build();
    let action_row = row::<Msg>()
        .gap(8.0)
        .child(
            button("btn.csv_import", t(lang, "contacts.csv_import"))
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Small)
                .on_click(Msg::ImportContactsCsv),
        )
        .child(
            button("btn.csv_export", t(lang, "contacts.csv_export"))
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Small)
                .on_click(Msg::ExportContactsCsv),
        )
        .build();
    column::<Msg>()
        .key("csvblock")
        .p(12.0)
        .gap(8.0)
        .bg(ColorToken::Surface)
        .child(kit::caption(t(lang, "contacts.csv_section")))
        .child(path_row)
        .child(action_row)
        .child(kit::caption_wrap(t(lang, "contacts.csv_format")))
        .build()
}

/// Contacts whose name, company, fax, phone or email contain every
/// whitespace-separated word of `query` (case-insensitive). Empty query = all.
fn filter_contacts<'a>(contacts: &'a [Contact], query: &str) -> Vec<&'a Contact> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    contacts
        .iter()
        .filter(|contact| {
            words
                .iter()
                .all(|word| haystack(contact).contains(word.as_str()))
        })
        .collect()
}

fn haystack(contact: &Contact) -> String {
    [
        contact.name.as_str(),
        contact.company.as_str(),
        contact.fax_number.as_str(),
        contact.phone.as_str(),
        contact.email.as_str(),
    ]
    .join(" ")
    .to_lowercase()
}

/// Join the non-empty parts with `sep`.
fn join_non_empty(parts: &[&str], sep: &str) -> String {
    parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(sep)
}

fn truncate(text: &str, units: usize) -> String {
    kit::truncate_units(text, units)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;
    use acme_widgets::{Card, ScrollView};
    use std::collections::HashSet;

    #[derive(Default)]
    struct Walk {
        keys: Vec<String>,
        msgs: Vec<Msg>,
    }

    /// Collect every widget key and every reachable message of a tree.
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

    fn contact(id: ContactId, name: &str, company: &str, fax: &str) -> Contact {
        Contact {
            id,
            name: name.into(),
            company: company.into(),
            fax_number: fax.into(),
            ..Contact::default()
        }
    }

    fn state_with(contacts: Vec<Contact>, editing: Option<ContactId>) -> AppState {
        let mut state = AppState::new(Settings::default());
        state.contacts = contacts;
        state.editing_contact = editing;
        state
    }

    #[test]
    fn list_rows_emit_edit_contact_for_each_contact() {
        let state = state_with(
            vec![
                contact(5, "Alice", "ACME", "0223456789"),
                contact(9, "Bob", "", ""),
            ],
            None,
        );
        let out = collect(&state);
        assert!(out.msgs.contains(&Msg::EditContact(5)));
        assert!(out.msgs.contains(&Msg::EditContact(9)));
        assert!(out.keys.contains(&"row.contact.5".to_string()));
    }

    #[test]
    fn editor_is_hidden_when_not_editing() {
        let state = state_with(vec![contact(5, "Alice", "", "")], None);
        let out = collect(&state);
        assert!(!out.keys.contains(&"field.contact_name".to_string()));
        assert!(!out.msgs.contains(&Msg::SaveContact));
        assert!(!out.msgs.contains(&Msg::CancelContactEdit));
        assert!(out.msgs.contains(&Msg::NewContact));
    }

    #[test]
    fn editor_shows_fields_and_delete_when_editing_existing_contact() {
        let state = state_with(vec![contact(5, "Alice", "", "")], Some(5));
        let out = collect(&state);
        for key in [
            "field.contact_name",
            "field.contact_company",
            "field.contact_fax",
            "field.contact_phone",
            "field.contact_email",
            "field.contact_note",
        ] {
            assert!(out.keys.contains(&key.to_string()), "missing {key}");
        }
        assert!(out.msgs.contains(&Msg::SaveContact));
        assert!(out.msgs.contains(&Msg::CancelContactEdit));
        assert!(out.msgs.contains(&Msg::AskDeleteContact(5)));
    }

    #[test]
    fn new_contact_has_no_delete_action() {
        let state = state_with(vec![contact(5, "Alice", "", "")], Some(NEW_CONTACT_ID));
        let out = collect(&state);
        assert!(out.keys.contains(&"field.contact_name".to_string()));
        assert!(out.msgs.contains(&Msg::SaveContact));
        assert!(
            !out.msgs
                .iter()
                .any(|m| matches!(m, Msg::AskDeleteContact(_)))
        );
    }

    #[test]
    fn csv_block_emits_import_export_and_browse() {
        let out = collect(&state_with(vec![], None));
        assert!(out.msgs.contains(&Msg::ImportContactsCsv));
        assert!(out.msgs.contains(&Msg::ExportContactsCsv));
        assert!(out.msgs.contains(&Msg::Browse(Field::CsvPath)));
    }

    #[test]
    fn empty_contacts_show_no_rows() {
        let out = collect(&state_with(vec![], None));
        assert!(!out.msgs.iter().any(|m| matches!(m, Msg::EditContact(_))));
        assert!(out.keys.contains(&"scroll.contacts".to_string()));
    }

    #[test]
    fn search_filters_every_word_case_insensitively() {
        let contacts = vec![
            contact(1, "Alice", "ACME Corp", "0223456789"),
            contact(2, "Bob", "Globex", "0287654321"),
        ];
        assert_eq!(filter_contacts(&contacts, "").len(), 2);
        assert_eq!(filter_contacts(&contacts, "acme").len(), 1);
        assert_eq!(filter_contacts(&contacts, "  ALICE   0223 ")[0].id, 1);
        assert!(filter_contacts(&contacts, "alice globex").is_empty());
        assert_eq!(filter_contacts(&contacts, "8765")[0].id, 2);
    }

    #[test]
    fn search_result_drives_visible_rows() {
        let mut state = state_with(
            vec![contact(1, "Alice", "", ""), contact(2, "Bob", "", "")],
            None,
        );
        state.fields.insert(Field::ContactSearch, "bob".into());
        let out = collect(&state);
        assert!(out.msgs.contains(&Msg::EditContact(2)));
        assert!(!out.msgs.contains(&Msg::EditContact(1)));
    }

    #[test]
    fn join_non_empty_skips_blanks() {
        assert_eq!(join_non_empty(&["ACME", "", "  "], " · "), "ACME");
        assert_eq!(join_non_empty(&["ACME", "0223"], " · "), "ACME · 0223");
        assert_eq!(join_non_empty(&[], " · "), "");
    }

    #[test]
    fn keys_are_unique_within_one_frame() {
        let state = state_with(
            vec![contact(1, "A", "", ""), contact(2, "B", "", "")],
            Some(2),
        );
        let out = collect(&state);
        let mut seen = HashSet::new();
        for key in &out.keys {
            assert!(seen.insert(key.clone()), "duplicate key {key}");
        }
    }

    #[test]
    fn view_builds_in_english_too() {
        let mut state = state_with(vec![contact(1, "A", "", "")], Some(1));
        state.lang = Lang::En;
        assert!(!collect(&state).keys.is_empty());
    }
}
