//! Inbox detail pane (UI_SPEC §3.3): info card, actions, edit card and page preview.
//! Owner: views worker (inbox + detail).

use super::inbox::text_builder;
use super::kit::{self, DETAIL_W, PANE_PAD, PREVIEW_MAX_H, glyph};
use crate::app::controller::PREVIEW_IMAGE_KEY;
use crate::app::state::PreviewStatus;
use crate::app::{AppState, Field, Msg};
use crate::i18n::{Lang, t, tf};
use crate::model::{FaxId, FaxRecord, normalize_number};
use acme_widgets::style::ColorToken;
use acme_widgets::style::prelude::Styled;
use acme_widgets::{
    ButtonSize, ButtonVariant, CardVariant, LabelWrap, WidgetNode, button, card, column, row,
    text_input,
};

/// Aspect ratio used for a preview slot before the page size is known (A4, height / width).
const A4_RATIO: f32 = 1.414;
/// Border of the scrolling preview viewport (drawn inside its box, 1 px per side).
const PREVIEW_BORDER: f32 = 1.0;
/// Width available for the page: pane minus its padding and the viewport border.
const PREVIEW_INNER_W: f32 = DETAIL_W - 2.0 * PANE_PAD - 2.0 * PREVIEW_BORDER;
/// Zoom range, matching `PreviewState::zoom` (0.5..=3.0).
const ZOOM_MIN: f32 = 0.5;
const ZOOM_MAX: f32 = 3.0;

/// Detail pane (DETAIL_W wide) for `state.selected` (empty state when none).
pub fn pane(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let base = column::<Msg>()
        .w(kit::px(DETAIL_W))
        .h(crate::ui::views::kit::full())
        .bg(ColorToken::Surface);
    let Some(fax) = state.selected.as_ref() else {
        return base
            .child(kit::empty_state(
                glyph::NAV_INBOX,
                t(lang, "detail.empty_title"),
                t(lang, "detail.empty_hint"),
                None,
            ))
            .build();
    };
    let head = row::<Msg>()
        .h(kit::px(40.0))
        .px(PANE_PAD)
        .gap(8.0)
        .child(kit::section(t(lang, "detail.title")))
        .child(kit::spacer())
        .child(
            button("btn.close_detail", glyph::CLOSE)
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::XS)
                .on_click(Msg::CloseDetail),
        )
        .build();
    let body = column::<Msg>()
        .key("scroll.detail")
        .flex_1()
        .flex_shrink(1.0)
        .min_h(crate::ui::views::kit::px(0.0))
        .overflow_scroll()
        .p(PANE_PAD)
        .gap(12.0)
        .child(info_card(state, fax))
        .child(actions(state, fax))
        .child(preview_block(state, fax.id))
        .child(edit_card(state, fax))
        .build();
    base.child(head).child(kit::hdivider()).child(body).build()
}

/// Label/value row of the info card; the label is a fixed 84 px column.
fn kv(label_text: &str, value: &str, wrap: LabelWrap) -> WidgetNode<Msg> {
    row::<Msg>()
        .gap(8.0)
        .child(
            text_builder(label_text, 12.0, false, ColorToken::MutedForeground)
                .wrap(LabelWrap::Word)
                .w(kit::px(84.0))
                .build(),
        )
        .child(
            text_builder(value, 14.0, false, ColorToken::Foreground)
                .wrap(wrap)
                .flex_1()
                .flex_shrink(1.0)
                // A text leaf's min-content width is its single-line width;
                // allow shrinking so long names/paths wrap inside the card.
                .min_w(kit::px(0.0))
                .build(),
        )
        .build()
}

fn info_card(state: &AppState, fax: &FaxRecord) -> WidgetNode<Msg> {
    let lang = state.lang;
    let sender = fax
        .display_sender()
        .unwrap_or_else(|| t(lang, "inbox.unknown_sender"));
    let mut card_builder = card::<Msg>()
        .variant(CardVariant::Outlined)
        .padding(12.0)
        .gap(8.0)
        .child(
            text_builder(sender, 20.0, true, ColorToken::Foreground)
                .wrap(LabelWrap::Word)
                .build(),
        );
    let has_name = fax
        .sender_name
        .as_deref()
        .is_some_and(|name| !name.trim().is_empty());
    if let Some(number) = fax.sender_number.as_deref().filter(|_| has_name) {
        card_builder = card_builder.child(kit::caption_wrap(number));
    }
    if let Some(at) = fax.deleted_at {
        let text = tf(
            lang,
            "detail.in_trash",
            &[("time", kit::fmt_dt(at).as_str())],
        );
        let strip = row::<Msg>()
            .px(8.0)
            .py(4.0)
            .rounded_sm()
            .bg(ColorToken::WarningSoft)
            .child(text_builder(&text, 12.0, false, ColorToken::Warning).build())
            .build();
        card_builder = card_builder.child(strip);
    }
    let number = fax.sender_number.as_deref().unwrap_or("—");
    card_builder
        .child(kv(
            t(lang, "detail.field_received"),
            &kit::fmt_dt(fax.received_at),
            LabelWrap::Word,
        ))
        .child(kv(t(lang, "detail.field_number"), number, LabelWrap::Word))
        .child(kv(
            t(lang, "detail.field_pages"),
            &fax.pages.to_string(),
            LabelWrap::Word,
        ))
        .child(kv(
            t(lang, "detail.field_size"),
            &kit::fmt_size(fax.file_size),
            LabelWrap::Word,
        ))
        .child(kv(
            t(lang, "detail.field_original"),
            &fax.original_name,
            LabelWrap::Character,
        ))
        .child(kv(
            t(lang, "detail.field_path"),
            &fax.file_path.display().to_string(),
            LabelWrap::Character,
        ))
        .build()
}

/// Horizontal group of buttons with 8 px gaps (buttons keep their intrinsic width).
fn button_row(children: Vec<WidgetNode<Msg>>) -> WidgetNode<Msg> {
    let mut out = row::<Msg>().gap(8.0);
    for child in children {
        out = out.child(child);
    }
    out.build()
}

fn small(key: &str, label: &str, variant: ButtonVariant, msg: Msg) -> WidgetNode<Msg> {
    button(key.to_string(), label.to_string())
        .variant(variant)
        .size(ButtonSize::Small)
        .on_click(msg)
}

/// True when a contact already uses the sender number (normalised comparison).
fn is_known_contact(state: &AppState, number: &str) -> bool {
    let Some(normalized) = normalize_number(number) else {
        return false;
    };
    state.contacts.iter().any(|c| c.fax_number == normalized)
}

fn actions(state: &AppState, fax: &FaxRecord) -> WidgetNode<Msg> {
    let lang = state.lang;
    let id = fax.id;
    let read_label = if fax.is_read {
        t(lang, "detail.mark_unread")
    } else {
        t(lang, "detail.mark_read")
    };
    let star_label = if fax.is_starred {
        format!("{} {}", glyph::STAR, t(lang, "detail.unstar"))
    } else {
        format!("{} {}", glyph::STAR_OFF, t(lang, "detail.star"))
    };
    let mut block = column::<Msg>().gap(8.0).child(button_row(vec![
        small(
            "btn.toggle_read",
            read_label,
            ButtonVariant::Secondary,
            Msg::ToggleRead(id),
        ),
        small(
            "btn.toggle_star",
            &star_label,
            ButtonVariant::Secondary,
            Msg::ToggleStar(id),
        ),
    ]));
    block = block.child(button_row(vec![
        small(
            "btn.open",
            t(lang, "detail.open"),
            ButtonVariant::Primary,
            Msg::OpenExternal(id),
        ),
        small(
            "btn.print",
            t(lang, "detail.print"),
            ButtonVariant::Secondary,
            Msg::Print(id),
        ),
        small(
            "btn.reveal",
            t(lang, "detail.reveal"),
            ButtonVariant::Secondary,
            Msg::RevealInFolder(id),
        ),
    ]));
    if let Some(number) = fax.sender_number.as_deref() {
        let contact = if is_known_contact(state, number) {
            kit::caption(t(lang, "detail.in_contacts"))
        } else {
            small(
                "btn.add_contact",
                t(lang, "detail.add_contact"),
                ButtonVariant::Secondary,
                Msg::AddSenderToContacts(id),
            )
        };
        block = block.child(button_row(vec![contact]));
    }
    let lifecycle = if fax.deleted_at.is_none() {
        vec![small(
            "btn.trash",
            t(lang, "detail.trash"),
            ButtonVariant::Danger,
            Msg::MoveToTrash(id),
        )]
    } else {
        vec![
            small(
                "btn.restore",
                t(lang, "detail.restore"),
                ButtonVariant::Secondary,
                Msg::Restore(id),
            ),
            small(
                "btn.delete_forever",
                t(lang, "detail.delete_forever"),
                ButtonVariant::Danger,
                Msg::AskDeletePermanently(id),
            ),
        ]
    };
    block.child(button_row(lifecycle)).build()
}

fn edit_card(state: &AppState, fax: &FaxRecord) -> WidgetNode<Msg> {
    let lang = state.lang;
    let tags = column::<Msg>()
        .gap(4.0)
        .child(kit::field(
            t(lang, "detail.label_tags"),
            text_input(Field::FaxTags.key())
                .value(state.field(Field::FaxTags))
                .w(crate::ui::views::kit::full())
                .build(),
        ))
        .child(kit::caption(t(lang, "detail.tags_hint")))
        .build();
    let save = row::<Msg>()
        .child(kit::spacer())
        .child(small(
            "btn.save_edits",
            t(lang, "detail.save_edits"),
            ButtonVariant::Primary,
            Msg::SaveFaxEdits(fax.id),
        ))
        .build();
    card::<Msg>()
        .variant(CardVariant::Outlined)
        .padding(12.0)
        .gap(8.0)
        .child(kit::section(t(lang, "detail.section_edit")))
        .child(kit::field(
            t(lang, "detail.label_sender"),
            text_input(Field::FaxSenderName.key())
                .value(state.field(Field::FaxSenderName))
                .w(crate::ui::views::kit::full())
                .build(),
        ))
        .child(tags)
        .child(kit::field(
            t(lang, "detail.label_note"),
            text_input(Field::FaxNote.key())
                .value(state.field(Field::FaxNote))
                .w(crate::ui::views::kit::full())
                .build(),
        ))
        .child(save)
        .build()
}

fn preview_block(state: &AppState, fax_id: FaxId) -> WidgetNode<Msg> {
    let lang = state.lang;
    let body = match &state.preview.status {
        PreviewStatus::Empty => loading_card(lang),
        PreviewStatus::Unavailable(text) => unavailable_card(lang, text, fax_id),
        PreviewStatus::Ready => ready_view(state),
    };
    column::<Msg>()
        .gap(8.0)
        .child(kit::section(t(lang, "detail.section_preview")))
        .child(body)
        .build()
}

/// Placeholder while the preview is being rendered (Muted card, 120 px high).
fn loading_card(lang: Lang) -> WidgetNode<Msg> {
    let text = row::<Msg>()
        .child(kit::spacer())
        .child(
            text_builder(
                t(lang, "detail.preview_loading"),
                12.0,
                false,
                ColorToken::Foreground,
            )
            .build(),
        )
        .child(kit::spacer())
        .build();
    card::<Msg>()
        .variant(CardVariant::Muted)
        .h(kit::px(120.0))
        .child(
            column::<Msg>()
                .flex_1()
                .flex_shrink(1.0)
                .min_h(crate::ui::views::kit::px(0.0))
                .child(kit::spacer())
                .child(text)
                .child(kit::spacer())
                .build(),
        )
        .build()
}

/// Preview could not be rendered: message, hint and an "Open" button.
fn unavailable_card(lang: Lang, message: &str, fax_id: FaxId) -> WidgetNode<Msg> {
    let open = row::<Msg>()
        .child(small(
            "btn.preview_open",
            t(lang, "detail.open"),
            ButtonVariant::Primary,
            Msg::OpenExternal(fax_id),
        ))
        .build();
    card::<Msg>()
        .variant(CardVariant::Muted)
        .padding(12.0)
        .gap(8.0)
        .child(
            text_builder(glyph::WARN, 14.0, true, ColorToken::Warning)
                .w(kit::px(16.0))
                .build(),
        )
        .child(
            text_builder(message, 12.0, false, ColorToken::Foreground)
                .wrap(LabelWrap::Word)
                .build(),
        )
        .child(
            text_builder(
                t(lang, "detail.preview_unavailable_hint"),
                12.0,
                false,
                ColorToken::Foreground,
            )
            .wrap(LabelWrap::Word)
            .build(),
        )
        .child(open)
        .build()
}

/// Ready preview: page bar (prev / x of y / next, zoom controls) and the scrolling image slot.
fn ready_view(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let preview = &state.preview;
    let zoom = preview.zoom;
    let page_text = tf(
        lang,
        "common.page_x_of_y",
        &[
            ("page", preview.page.saturating_add(1).to_string().as_str()),
            ("pages", preview.page_count.to_string().as_str()),
        ],
    );
    let percent = kit::fmt_percent(zoom);
    let zoom_text = tf(
        lang,
        "detail.zoom_percent",
        &[("percent", percent.trim_end_matches('%'))],
    );
    let at_first = preview.page == 0;
    let at_last = preview.page.saturating_add(1) >= preview.page_count;
    let bar = row::<Msg>()
        .gap(4.0)
        .child(
            button("btn.preview_prev", glyph::PREV)
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::XS)
                .disabled(at_first)
                .on_click(Msg::PreviewPrevPage),
        )
        .child(kit::caption(&page_text))
        .child(
            button("btn.preview_next", glyph::NEXT)
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::XS)
                .disabled(at_last)
                .on_click(Msg::PreviewNextPage),
        )
        .child(kit::spacer())
        .child(
            button("btn.zoom_out", glyph::MINUS)
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::XS)
                .disabled(zoom <= ZOOM_MIN)
                .on_click(Msg::ZoomOut),
        )
        .child(kit::caption(&zoom_text))
        .child(
            button("btn.zoom_in", glyph::PLUS)
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::XS)
                .disabled(zoom >= ZOOM_MAX)
                .on_click(Msg::ZoomIn),
        )
        .child(
            button("btn.zoom_reset", t(lang, "detail.zoom_fit"))
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::XS)
                .on_click(Msg::ZoomReset),
        )
        .build();
    let (slot_w, slot_h) = preview_size(
        PREVIEW_INNER_W,
        preview.image_width,
        preview.image_height,
        zoom,
    );
    let slot = column::<Msg>()
        .key(PREVIEW_IMAGE_KEY)
        .w(kit::px(slot_w))
        .h(kit::px(slot_h))
        .build();
    let viewport = column::<Msg>()
        .key("scroll.preview")
        .w(crate::ui::views::kit::full())
        .h(kit::px(preview_viewport_h(slot_h)))
        .overflow_scroll()
        .bg(ColorToken::Muted)
        .border(1.0, ColorToken::Border)
        .rounded_sm()
        .child(slot)
        .build();
    column::<Msg>().gap(8.0).child(bar).child(viewport).build()
}

/// Size of the page image slot for a given zoom (UI_SPEC §3.3).
///
/// Rule: width = round(`inner_w` × zoom), with zoom clamped to 0.5..=3.0 (non-finite zoom = 1.0).
/// Height keeps the image aspect ratio of the rounded width; when the image size is unknown
/// (0 in either axis) the A4 ratio 1.414 is used. The slot itself is never capped, so a zoomed
/// page stays whole and the viewport scrolls it (see [`preview_viewport_h`]).
/// Returns `(0, 0)` for a zero or non-finite pane width.
pub(crate) fn preview_size(inner_w: f32, image_w: u32, image_h: u32, zoom: f32) -> (f32, f32) {
    let zoom = if zoom.is_finite() {
        zoom.clamp(ZOOM_MIN, ZOOM_MAX)
    } else {
        1.0
    };
    let width = if inner_w.is_finite() {
        (inner_w.max(0.0) * zoom).round()
    } else {
        0.0
    };
    let ratio = if image_w > 0 && image_h > 0 {
        image_h as f32 / image_w as f32
    } else {
        A4_RATIO
    };
    (width, (width * ratio).round())
}

/// Height of the scrolling viewport: the slot height capped at `PREVIEW_MAX_H` so the rest of the
/// pane stays reachable. Zero or negative input yields 0.
pub(crate) fn preview_viewport_h(slot_h: f32) -> f32 {
    if slot_h.is_finite() {
        slot_h.clamp(0.0, PREVIEW_MAX_H)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests;
