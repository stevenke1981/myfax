//! Generic retained-input runtime for AcmeUI-Native `WidgetNode` trees.
//!
//! Owner: runtime worker. Public API below is the orchestrator contract —
//! signatures must not change without the orchestrator.
//!
//! Supported widget subset (views must only use these):
//! `Row`, `Column`, `Stack`, `Card`, `Label`, `Button`, `Separator`, `TextInput`.
//! Unsupported widgets are skipped (debug log only).
//!
//! # Conventions
//! * Clickable: enabled `Button` (its `activate()` message), any `Row`/`Column`/
//!   `Stack` with `on_click` message, `TextInput` (focuses it, then emits its
//!   message if set). Click = press and release over the same target. A click
//!   on a keyed clickable container focuses it; a click on an unkeyed container
//!   or empty space clears focus.
//! * Pass-through: containers without a message create no hit region; pointer
//!   input falls through them to whatever is painted beneath (e.g. a full-window
//!   transparent `Stack` layer holding only a toast).
//! * Modal: a `Row`/`Column`/`Stack` whose key starts with `modal:` absorbs
//!   pointer press/release/hover/scroll over its rect for everything painted
//!   *before* it. Its children (painted after it) work normally. While one
//!   exists, Tab focus, Enter/Space, typing and IME input are restricted to
//!   focusables inside the last-started `modal:` container. When a modal first
//!   appears (or changes), focus moves to its first focusable, or is cleared if
//!   it has none. A modal without `on_click` emits nothing itself.
//! * Scroll region: a keyed `Column`/`Row` with style `overflow_scroll()`.
//!   Offsets (x and y) persist per key across frames and are clamped to the
//!   content size on each axis. Wheel: a vertical delta scrolls y when the
//!   region overflows vertically, otherwise x; a horizontal delta scrolls x.
//!   The innermost unblocked region under the pointer whose offset can still
//!   move takes the wheel, so a region at its limit lets the enclosing one
//!   scroll. Give scroll regions an explicit height (or `flex_shrink(1.0)`
//!   inside a bounded parent) so they have a viewport.
//! * Image slot: a container whose key starts with `img:`; the image with the same
//!   key in [`FrameInput::images`] is drawn "contain"-fitted into its rect.
//! * Later siblings paint above earlier ones and win hit-tests (overlays).
//! * Culling: nodes that cannot be seen through the clip (the window, or an
//!   enclosing scroll viewport) are neither painted nor shaped, and produce no
//!   hit regions. Keyed scroll offsets, focus and container identities are kept.
//!
//! # Visual states
//! * Button hover/pressed/disabled colours come from `Button::resolve_style`.
//! * Clickable containers (`on_click`) get a hover overlay `ghost_hover`, and
//!   `ghost_pressed` while pressed, rounded to the container's `border_radius`
//!   (default 0). Views do not need to style hover themselves.
//! * Keyboard focus ring: a 2px ring in the theme `ring` colour is drawn around
//!   the focused `Button`, `TextInput` or keyed clickable container, but only
//!   while focus is *focus-visible*. Focus is focus-visible after Tab, Shift+Tab
//!   or an arrow key (arrows only while a button or container is focused); a
//!   pointer press clears it. A focused text input always shows its border
//!   (caret). Tab / Shift+Tab cycle focusables in paint order.
//! * Button width: in a `Row` or `Stack` a button is intrinsic (label width plus
//!   10px per side, Taffy text leaf). In a `Column` it stretches across the row
//!   (cross-axis stretch), so wrap buttons in a `Row` to keep them compact.
//! * `TextInput` is controlled: the app must store each `TextChanged` value and
//!   pass it back as `value`. A different widget value replaces the local buffer
//!   on the next render; the caret keeps its position, clamped.
//! * `TextInput::label` / `description` / `validation_message` are not drawn; use
//!   a `Label` next to the input.
//!
//! # Keys and Escape
//! * Enter in a text input submits; Space and Enter on a focused button or
//!   container activate it.
//! * Escape while an IME composition is open cancels only the composition.
//!   Escape in a focused text input blurs it and emits nothing. Escape with no
//!   input focused (or with a button or container focused) clears focus and
//!   emits [`UiEvent::Cancel`].
//! * Paste (Ctrl+V) and IME commits are single-line: `\r\n`, `\r`, `\n` and `\t`
//!   become one space each, and other control characters are dropped.
//!
//! # Text cache
//! Shaped text is cached across frames, keyed by text, style, wrap, width and
//! scale (bounded; the cache is cleared when full). Text inputs are shaped by
//! `acme-textinput` and are not cached. Layout (`acme-layout`) still measures
//! every text leaf on each frame; that pass is outside this module.
//!
//! # 變更歷史
//! | 版本 | 日期 | 內容 | 影響範圍 |
//! |------|------|------|----------|
//! | UI-FIX-R | 2026-11-11 | 文字塑形快取、剔除不可見節點、鍵控容器可聚焦與 focus-visible、modal 焦點與輸入隔離、貼上/IME 單行化、Escape 規則、雙軸捲動 | `src/ui/runtime/**` |

use std::collections::{HashMap, HashSet};

use acme_core::{Color, NodeId, Rgba8Image, Scene};
use acme_layout::{LayoutEngine, LayoutNode, LayoutSnapshot, Length, WidgetLayoutContext};
use acme_platform::{Clipboard, PlatformKey};
use acme_text::{FontSystem, GlyphAtlas};
use acme_theme::Theme;
use acme_widgets::WidgetNode;

mod cache;
mod edit;
mod geom;
mod hit;
mod paint;
mod text;
mod widgets;

#[cfg(test)]
mod tests;

use cache::TextCache;
use edit::Focus;
use geom::{Ctx, Offset2, Rect4, color, finite_non_negative};
use hit::{FrameState, Target, scroll_at, target_at};
use paint::Painter;

/// Result of input handling that the application must react to.
#[derive(Clone, Debug, PartialEq)]
pub enum UiEvent<M> {
    /// A clickable widget was activated (mouse click or Enter/Space on focus).
    Message(M),
    /// The focused text input's full new value (after an edit / paste / IME commit).
    TextChanged { key: String, value: String },
    /// Enter pressed in a text input.
    Submit { key: String },
    /// Escape pressed with no text input focused (focus is also cleared).
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

/// Layout-pass inputs shared by every paint pass of one frame.
struct PassEnv<'a> {
    theme: &'a Theme,
    scale: f32,
    images: &'a HashMap<String, Rgba8Image>,
    clear: Color,
    /// The window rectangle; the root clip.
    window: Rect4,
}

/// Output of one paint pass.
struct Pass<M> {
    out: Scene,
    frame: FrameState<M>,
    seen_scroll: HashSet<String>,
    focus_found: bool,
}

/// Layout + paint + hit-test + focus + scrolling + text editing.
pub struct UiRuntime<M> {
    fonts: FontSystem,
    atlas: GlyphAtlas,
    text: TextCache,
    layout: LayoutEngine,
    frame: FrameState<M>,
    focus: Option<Focus>,
    /// Whether the focus ring is shown (keyboard-driven focus).
    focus_visible: bool,
    /// Key of the modal open in the previous frame, for detecting appearance.
    last_modal: Option<String>,
    hover: Option<Target>,
    pressed: Option<Target>,
    scroll_offsets: HashMap<String, Offset2>,
    scale: f32,
    clipboard: Option<Clipboard>,
    clipboard_tried: bool,
}

impl<M: Clone + 'static> Default for UiRuntime<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: Clone + 'static> UiRuntime<M> {
    pub fn new() -> Self {
        Self {
            fonts: FontSystem::new(),
            atlas: GlyphAtlas::default(),
            text: TextCache::default(),
            layout: LayoutEngine::new(),
            frame: FrameState::default(),
            focus: None,
            focus_visible: false,
            last_modal: None,
            hover: None,
            pressed: None,
            scroll_offsets: HashMap::new(),
            scale: 1.0,
            clipboard: None,
            clipboard_tried: false,
        }
    }

    /// Lay out and paint `root`; remembers geometry for subsequent input calls.
    /// Never panics on odd trees; unsupported widgets are skipped.
    pub fn render(&mut self, root: &WidgetNode<M>, input: FrameInput<'_>) -> Scene {
        let width = finite_non_negative(input.logical_width);
        let height = finite_non_negative(input.logical_height);
        let scale = if input.scale_factor.is_finite() && input.scale_factor > 0.0 {
            input.scale_factor
        } else {
            1.0
        };
        self.scale = scale;
        let theme = input.theme;
        let clear = color(theme.colors.background);
        let context = WidgetLayoutContext {
            body_font_size: theme.typography.body,
            body_line_height: theme.typography.body * theme.typography.line_height,
            label_font_size: theme.typography.label,
            control_height: theme.control_height(3),
            scale_factor: scale,
        };
        let mut tree = root.to_layout_with_context(NodeId::new(1), &context);
        // The root fills the window unless it sets an explicit size itself.
        if matches!(tree.style.width, Length::Auto) {
            tree.style.width = Length::px(width);
        }
        if matches!(tree.style.height, Length::Auto) {
            tree.style.height = Length::px(height);
        }
        let snapshot =
            match self
                .layout
                .compute_with_text(&tree, (width, height), &mut self.fonts, scale)
            {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    tracing::warn!(%error, "ui layout failed; frame skipped");
                    self.frame = FrameState::default();
                    return Scene::with_clear(clear);
                }
            };
        let env = PassEnv {
            theme,
            scale,
            images: input.images,
            clear,
            window: [0.0, 0.0, width, height],
        };
        let mut pass = self.paint_pass(root, &tree, &snapshot, &env);
        self.frame = std::mem::take(&mut pass.frame);
        // A modal that just appeared takes focus; repaint so the ring and
        // caret reflect it.
        if self.sync_modal_focus() {
            pass = self.paint_pass(root, &tree, &snapshot, &env);
            self.frame = std::mem::take(&mut pass.frame);
        }
        if !pass.focus_found {
            self.focus = None;
        }
        self.scroll_offsets
            .retain(|key, _| pass.seen_scroll.contains(key));
        pass.out
    }

    fn paint_pass(
        &mut self,
        root: &WidgetNode<M>,
        tree: &LayoutNode,
        snapshot: &LayoutSnapshot,
        env: &PassEnv<'_>,
    ) -> Pass<M> {
        let mut painter = Painter {
            fonts: &mut self.fonts,
            atlas: &mut self.atlas,
            text: &mut self.text,
            theme: env.theme,
            scale: env.scale,
            images: env.images,
            snapshot,
            scroll_offsets: &mut self.scroll_offsets,
            seen_scroll: HashSet::new(),
            focus: &mut self.focus,
            focus_found: false,
            focus_visible: self.focus_visible,
            hover: self.hover.clone(),
            pressed: self.pressed.clone(),
            out: Scene::with_clear(env.clear),
            frame: FrameState::default(),
            container_seq: 0,
            seq: 0,
        };
        painter.paint(
            root,
            tree,
            Ctx {
                dx: 0.0,
                dy: 0.0,
                clip: Some(env.window),
            },
        );
        let Painter {
            out,
            frame,
            seen_scroll,
            focus_found,
            ..
        } = painter;
        Pass {
            out,
            frame,
            seen_scroll,
            focus_found,
        }
    }

    /// Returns true when hover state changed (redraw needed).
    pub fn pointer_moved(&mut self, x: f32, y: f32) -> bool {
        let target = target_at(&self.frame.regions, x, y);
        if target == self.hover {
            return false;
        }
        self.hover = target;
        true
    }

    pub fn pointer_left(&mut self) -> bool {
        self.hover.take().is_some()
    }

    /// Left button (`button == 0`) press/release. A click fires on release over
    /// the same target that was pressed.
    pub fn pointer_button(&mut self, pressed: bool, x: f32, y: f32) -> Vec<UiEvent<M>> {
        if pressed {
            self.pointer_down(x, y);
            Vec::new()
        } else {
            self.pointer_up(x, y)
        }
    }

    /// Wheel / touchpad scroll at (x, y). Returns true when an offset changed.
    ///
    /// `lines == true` means `dy` is a line delta as reported by acme-platform
    /// (already scaled by 32 px per line); it is converted to 40 px per line.
    /// Positive `dy` scrolls content up, as in the platform convention. `dx`
    /// scrolls horizontally; the caller maps Shift+wheel to `dx`.
    pub fn scroll(&mut self, dx: f32, dy: f32, lines: bool, x: f32, y: f32) -> bool {
        scroll_at(&self.frame, &mut self.scroll_offsets, dx, dy, lines, x, y)
    }

    /// Key press/release. `text` is the produced character text, if any.
    pub fn key(
        &mut self,
        key: PlatformKey,
        pressed: bool,
        mods: Modifiers,
        text: Option<&str>,
    ) -> Vec<UiEvent<M>> {
        if !pressed {
            return Vec::new();
        }
        self.dispatch_key(key, mods, text)
    }

    /// IME composition text (shown inline, not yet committed). Returns redraw flag.
    pub fn ime_preedit(&mut self, text: &str) -> bool {
        self.ime_preedit_text(text)
    }

    pub fn ime_commit(&mut self, text: &str) -> Vec<UiEvent<M>> {
        self.ime_commit_text(text)
    }

    /// Key of the focused text input, if any.
    pub fn focused_input(&self) -> Option<&str> {
        match &self.focus {
            Some(Focus::Input { key, .. }) => Some(key.as_str()),
            _ => None,
        }
    }

    /// Caret rectangle `[x, y, w, h]` (logical) of the focused input, for IME placement.
    pub fn ime_cursor_area(&self) -> Option<[f32; 4]> {
        self.frame.ime_caret
    }

    /// Reset a scroll region to the top-left (e.g. after changing page).
    pub fn reset_scroll(&mut self, key: &str) {
        self.scroll_offsets
            .insert(key.to_owned(), Offset2::default());
    }

    /// Drop GPU-side caches (glyph atlas) after device loss.
    pub fn clear_gpu_caches(&mut self) {
        self.atlas.clear();
    }
}
