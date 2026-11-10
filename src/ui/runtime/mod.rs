//! Generic retained-input runtime for AcmeUI-Native `WidgetNode` trees.
//!
//! Owner: runtime worker. Public API below is the orchestrator contract —
//! signatures must not change without the orchestrator.
//!
//! Supported widget subset (views must only use these):
//! `Row`, `Column`, `Stack`, `Card`, `Label`, `Button`, `Separator`, `TextInput`.
//! Conventions:
//! * Clickable: enabled `Button` (its `activate()` message), any container with
//!   `on_click` message, `TextInput` (focuses it, then emits its message if set).
//! * Scroll region: a `Column`/`Row` that has a key AND style `overflow_scroll()`.
//!   Offsets persist per key across frames and are clamped to content size.
//! * Image slot: a container whose key starts with `img:`; the image with the same
//!   key in [`FrameInput::images`] is drawn "contain"-fitted into its rect.
//! * Later siblings paint above earlier ones and win hit-tests (overlays).

use acme_core::{Rgba8Image, Scene};
use acme_platform::PlatformKey;
use acme_theme::Theme;
use acme_widgets::WidgetNode;
use std::collections::HashMap;

/// Result of input handling that the application must react to.
#[derive(Clone, Debug, PartialEq)]
pub enum UiEvent<M> {
    /// A clickable widget was activated (mouse click or Enter/Space on focus).
    Message(M),
    /// The focused text input's full new value (after an edit / paste / IME commit).
    TextChanged { key: String, value: String },
    /// Enter pressed in a text input.
    Submit { key: String },
    /// Escape pressed (focus is also cleared).
    Cancel,
}

/// Per-frame inputs.
pub struct FrameInput<'a> {
    pub logical_width: f32,
    pub logical_height: f32,
    pub scale_factor: f32,
    pub theme: &'a Theme,
    /// Images for `img:*` slots, keyed by the full widget key (e.g. `img:preview`).
    pub images: &'a HashMap<String, Rgba8Image>,
}

/// Keyboard modifiers for [`UiRuntime::key`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// Layout + paint + hit-test + focus + scrolling + text editing.
pub struct UiRuntime<M> {
    _marker: std::marker::PhantomData<M>,
}

impl<M: Clone + 'static> Default for UiRuntime<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: Clone + 'static> UiRuntime<M> {
    pub fn new() -> Self {
        todo!("runtime worker")
    }

    /// Lay out and paint `root`; remembers geometry for subsequent input calls.
    /// Never panics on odd trees; unsupported widgets are skipped.
    pub fn render(&mut self, _root: &WidgetNode<M>, _input: FrameInput<'_>) -> Scene {
        todo!("runtime worker")
    }

    /// Returns true when hover state changed (redraw needed).
    pub fn pointer_moved(&mut self, _x: f32, _y: f32) -> bool {
        todo!("runtime worker")
    }

    pub fn pointer_left(&mut self) -> bool {
        todo!("runtime worker")
    }

    /// Left button (`button == 0`) press/release. A click fires on release over
    /// the same target that was pressed.
    pub fn pointer_button(&mut self, _pressed: bool, _x: f32, _y: f32) -> Vec<UiEvent<M>> {
        todo!("runtime worker")
    }

    /// Wheel / touchpad scroll at (x, y). Returns true when an offset changed.
    pub fn scroll(&mut self, _dx: f32, _dy: f32, _lines: bool, _x: f32, _y: f32) -> bool {
        todo!("runtime worker")
    }

    /// Key press/release. `text` is the produced character text, if any.
    pub fn key(
        &mut self,
        _key: PlatformKey,
        _pressed: bool,
        _mods: Modifiers,
        _text: Option<&str>,
    ) -> Vec<UiEvent<M>> {
        todo!("runtime worker")
    }

    /// IME composition text (shown inline, not yet committed). Returns redraw flag.
    pub fn ime_preedit(&mut self, _text: &str) -> bool {
        todo!("runtime worker")
    }

    pub fn ime_commit(&mut self, _text: &str) -> Vec<UiEvent<M>> {
        todo!("runtime worker")
    }

    /// Key of the focused text input, if any.
    pub fn focused_input(&self) -> Option<&str> {
        todo!("runtime worker")
    }

    /// Caret rectangle `[x, y, w, h]` (logical) of the focused input, for IME placement.
    pub fn ime_cursor_area(&self) -> Option<[f32; 4]> {
        todo!("runtime worker")
    }

    /// Reset a scroll region to the top (e.g. after changing page).
    pub fn reset_scroll(&mut self, _key: &str) {
        todo!("runtime worker")
    }

    /// Drop GPU-side caches (glyph atlas) after device loss.
    pub fn clear_gpu_caches(&mut self) {
        todo!("runtime worker")
    }
}
