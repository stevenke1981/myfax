//! Tests for the detail pane: empty state, actions, trash strip, preview states and sizing math.

use super::{pane, preview_size, preview_viewport_h};
use crate::app::controller::PREVIEW_IMAGE_KEY;
use crate::app::state::{PreviewState, PreviewStatus};
use crate::app::{AppState, Msg};
use crate::model::{Contact, FaxRecord};
use crate::ui::views::inbox::testing::{empty_state, sample_fax, walk};
use crate::ui::views::kit::PREVIEW_MAX_H;
use chrono::Local;

fn selected_state(fax: FaxRecord) -> AppState {
    let mut state = empty_state();
    state.selected = Some(fax);
    state
}

fn ready_preview(page: u32, page_count: u32, zoom: f32) -> PreviewState {
    PreviewState {
        page,
        page_count,
        zoom,
        status: PreviewStatus::Ready,
        image_width: 1000,
        image_height: 1000,
    }
}

#[test]
fn empty_pane_without_selection_has_no_actions() {
    let state = empty_state();
    let tree = walk(&pane(&state));
    assert!(!tree.has_key_prefix("btn."));
    assert!(!tree.can(&Msg::CloseDetail));
    assert!(tree.has_label("選擇一份傳真"));
}

#[test]
fn live_fax_offers_read_star_open_and_trash() {
    let state = selected_state(sample_fax(1, Some("Alpha"), false, false));
    let tree = walk(&pane(&state));
    assert!(tree.can(&Msg::ToggleRead(1)));
    assert!(tree.can(&Msg::ToggleStar(1)));
    assert!(tree.can(&Msg::OpenExternal(1)));
    assert!(tree.can(&Msg::Print(1)));
    assert!(tree.can(&Msg::RevealInFolder(1)));
    assert!(tree.can(&Msg::MoveToTrash(1)));
    assert!(tree.can(&Msg::SaveFaxEdits(1)));
    assert!(tree.can(&Msg::CloseDetail));
    assert!(!tree.can(&Msg::Restore(1)));
    assert!(!tree.can(&Msg::AskDeletePermanently(1)));
}

#[test]
fn trashed_fax_offers_restore_and_delete_permanently() {
    let mut fax = sample_fax(4, Some("Gamma"), true, false);
    fax.deleted_at = Some(Local::now());
    let state = selected_state(fax);
    let tree = walk(&pane(&state));
    assert!(tree.can(&Msg::Restore(4)));
    assert!(tree.can(&Msg::AskDeletePermanently(4)));
    assert!(!tree.can(&Msg::MoveToTrash(4)));
    assert!(tree.has_key("btn.restore"));
    assert!(tree.has_key("btn.delete_forever"));
}

#[test]
fn known_contact_hides_add_to_contacts_button() {
    let fax = sample_fax(5, None, true, false);
    let number = fax.sender_number.clone().unwrap_or_default();
    let mut state = selected_state(fax);
    assert!(walk(&pane(&state)).can(&Msg::AddSenderToContacts(5)));

    state.contacts = vec![Contact {
        id: 1,
        fax_number: number,
        ..Contact::default()
    }];
    let tree = walk(&pane(&state));
    assert!(!tree.has_key("btn.add_contact"));
    assert!(tree.has_label("已在聯絡人中"));
}

#[test]
fn ready_preview_creates_image_slot_and_page_navigation() {
    let mut state = selected_state(sample_fax(1, Some("Alpha"), true, false));
    state.preview = ready_preview(0, 3, 1.0);
    let tree = walk(&pane(&state));
    assert!(tree.has_key(PREVIEW_IMAGE_KEY));
    assert!(tree.has_key("scroll.preview"));
    assert!(tree.can(&Msg::PreviewNextPage));
    assert!(!tree.can(&Msg::PreviewPrevPage));

    state.preview.page = 2;
    let last = walk(&pane(&state));
    assert!(last.can(&Msg::PreviewPrevPage));
    assert!(!last.can(&Msg::PreviewNextPage));
}

#[test]
fn zoom_buttons_disable_at_their_bounds() {
    let mut state = selected_state(sample_fax(1, Some("Alpha"), true, false));
    state.preview = ready_preview(0, 1, 3.0);
    let max = walk(&pane(&state));
    assert!(!max.can(&Msg::ZoomIn));
    assert!(max.can(&Msg::ZoomOut));

    state.preview.zoom = 0.5;
    let min = walk(&pane(&state));
    assert!(!min.can(&Msg::ZoomOut));
    assert!(min.can(&Msg::ZoomIn));
    assert!(min.can(&Msg::ZoomReset));
}

#[test]
fn unavailable_preview_shows_message_and_open_button() {
    let mut state = selected_state(sample_fax(7, Some("Delta"), true, false));
    state.preview = PreviewState {
        status: PreviewStatus::Unavailable("這個 PDF 已加密，無法預覽".to_string()),
        ..PreviewState::default()
    };
    let tree = walk(&pane(&state));
    assert!(tree.has_label("這個 PDF 已加密，無法預覽"));
    assert!(tree.can(&Msg::OpenExternal(7)));
    assert!(!tree.has_key(PREVIEW_IMAGE_KEY));
}

#[test]
fn empty_preview_with_selection_shows_loading_caption() {
    let state = selected_state(sample_fax(1, Some("Alpha"), true, false));
    let tree = walk(&pane(&state));
    assert!(tree.has_label("正在產生預覽…"));
    assert!(!tree.has_key(PREVIEW_IMAGE_KEY));
}

#[test]
fn preview_size_fits_pane_width_at_zoom_one() {
    assert_eq!(preview_size(328.0, 1000, 1000, 1.0), (328.0, 328.0));
}

#[test]
fn preview_size_scales_with_zoom_and_keeps_aspect() {
    assert_eq!(preview_size(328.0, 1000, 1000, 1.25), (410.0, 410.0));
    assert_eq!(preview_size(328.0, 1000, 1000, 0.5), (164.0, 164.0));
    // 2:1 landscape page: height is half the width.
    assert_eq!(preview_size(328.0, 2000, 1000, 1.0), (328.0, 164.0));
}

#[test]
fn preview_size_uses_a4_ratio_when_image_size_is_unknown() {
    assert_eq!(preview_size(328.0, 0, 0, 1.0), (328.0, 464.0));
    assert_eq!(preview_size(328.0, 1000, 0, 1.0), (328.0, 464.0));
}

#[test]
fn preview_size_handles_zero_and_bad_inputs() {
    assert_eq!(preview_size(0.0, 1000, 1000, 1.0), (0.0, 0.0));
    assert_eq!(preview_size(-5.0, 1000, 1000, 1.0), (0.0, 0.0));
    assert_eq!(preview_size(f32::NAN, 1000, 1000, 1.0), (0.0, 0.0));
    assert_eq!(preview_size(328.0, 1000, 1000, f32::NAN), (328.0, 328.0));
    assert_eq!(preview_size(328.0, 1000, 1000, 0.0), (164.0, 164.0));
    assert_eq!(preview_size(328.0, 1000, 1000, 99.0), (984.0, 984.0));
}

#[test]
fn preview_viewport_is_capped_so_the_pane_stays_reachable() {
    assert_eq!(preview_viewport_h(300.0), 300.0);
    assert_eq!(preview_viewport_h(PREVIEW_MAX_H + 200.0), PREVIEW_MAX_H);
    assert_eq!(preview_viewport_h(f32::NAN), 0.0);
    assert_eq!(preview_viewport_h(-1.0), 0.0);
}
