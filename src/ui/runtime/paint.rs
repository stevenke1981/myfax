//! Tree walk: containers, hover and focus overlays, modal spans, two-axis scroll
//! viewports and image slots. Leaf widgets are painted by `widgets`.
//!
//! A node whose painted extent (its own rect, or for containers the union of
//! its subtree) cannot be seen through the current clip is neither painted nor
//! shaped. Its bookkeeping (container ids, sequence numbers, scroll offsets,
//! focus, modal spans) is still recorded so that frames stay consistent.

use std::collections::{HashMap, HashSet};

use acme_core::{Color, DrawCommand, ImagePrimitive, Rgba8Image, Scene, WidgetKey};
use acme_layout::{Edges, LayoutNode, LayoutSnapshot, Overflow};
use acme_style::Style;
use acme_text::{FontSystem, GlyphAtlas};
use acme_theme::{Theme, ThemeColor};
use acme_widgets::{Card, CardVariant, WidgetNode};

use super::cache::TextCache;
use super::edit::Focus;
use super::geom::{
    Ctx, FOCUS_RING_WIDTH, Offset2, Rect4, clamp_offset, color, finite_non_negative, intersect,
    is_finite_rect, join, quad, to_rect, visible_in,
};
use super::hit::{FrameState, ModalSpan, Region, ScrollRegion, Target};

const IMAGE_SLOT_PREFIX: &str = "img:";
const MODAL_PREFIX: &str = "modal:";
const SCROLLBAR_WIDTH: f32 = 4.0;
const SCROLLBAR_INSET: f32 = 2.0;
const SCROLLBAR_MIN_THUMB: f32 = 24.0;
/// Overflow below this (layout rounding) does not make an axis scrollable.
const OVERFLOW_SLACK: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Axis {
    Row,
    Column,
}

/// Paint state for one frame. Fields are owned by `UiRuntime` and borrowed here.
pub(super) struct Painter<'a, M> {
    pub fonts: &'a mut FontSystem,
    pub atlas: &'a mut GlyphAtlas,
    pub text: &'a mut TextCache,
    pub theme: &'a Theme,
    pub scale: f32,
    pub images: &'a HashMap<String, Rgba8Image>,
    pub snapshot: &'a LayoutSnapshot,
    pub scroll_offsets: &'a mut HashMap<String, Offset2>,
    pub seen_scroll: HashSet<String>,
    pub focus: &'a mut Option<Focus>,
    pub focus_found: bool,
    pub focus_visible: bool,
    pub hover: Option<Target>,
    pub pressed: Option<Target>,
    pub out: Scene,
    pub frame: FrameState<M>,
    pub container_seq: usize,
    pub seq: usize,
}

struct Group<'w, M> {
    key: Option<&'w str>,
    style: &'w Style,
    message: Option<&'w M>,
    children: &'w [WidgetNode<M>],
    axis: Option<Axis>,
    card: Option<&'w Card<M>>,
    padding: &'w Edges,
}

/// Surface defaults a widget supplies before the `Style` overrides apply.
#[derive(Clone, Copy)]
pub(super) struct BoxDefaults {
    pub fill: Option<ThemeColor>,
    pub border: Option<ThemeColor>,
    pub radius: f32,
    pub border_width: f32,
}

impl BoxDefaults {
    pub(super) const NONE: Self = Self {
        fill: None,
        border: None,
        radius: 0.0,
        border_width: 0.0,
    };
}

fn key_str(key: &Option<WidgetKey>) -> Option<&str> {
    key.as_ref().map(WidgetKey::as_str)
}

fn group_of<M>(widget: &WidgetNode<M>) -> Option<Group<'_, M>> {
    match widget {
        WidgetNode::Row(container) => Some(Group {
            key: key_str(&container.key),
            style: &container.style,
            message: container.message.as_ref(),
            children: &container.children,
            axis: Some(Axis::Row),
            card: None,
            padding: &container.padding,
        }),
        WidgetNode::Column(container) => Some(Group {
            key: key_str(&container.key),
            style: &container.style,
            message: container.message.as_ref(),
            children: &container.children,
            axis: Some(Axis::Column),
            card: None,
            padding: &container.padding,
        }),
        WidgetNode::Stack(container) => Some(Group {
            key: key_str(&container.key),
            style: &container.style,
            message: container.message.as_ref(),
            children: &container.children,
            axis: None,
            card: None,
            padding: &container.padding,
        }),
        WidgetNode::Card(card) => Some(Group {
            key: key_str(&card.key),
            style: &card.style,
            message: None,
            children: &card.children,
            axis: None,
            card: Some(card),
            padding: &card.padding,
        }),
        _ => None,
    }
}

/// Containers and wrappers whose painted extent includes their children.
fn spans_children<M>(widget: &WidgetNode<M>) -> bool {
    matches!(
        widget,
        WidgetNode::Row(_)
            | WidgetNode::Column(_)
            | WidgetNode::Stack(_)
            | WidgetNode::Card(_)
            | WidgetNode::Tooltip(_)
            | WidgetNode::Popover(_)
    )
}

/// Key of a `modal:` container.
fn modal_key(key: Option<&str>) -> Option<&str> {
    key.filter(|key| key.starts_with(MODAL_PREFIX))
}

/// A keyed clickable container that is not a modal can take keyboard focus.
fn is_focusable<M>(group: &Group<'_, M>) -> bool {
    group.message.is_some() && group.key.is_some() && modal_key(group.key).is_none()
}

/// Key of a scroll region: a keyed `Row`/`Column` with `overflow_scroll()`.
fn scroll_key<'w, M>(group: &Group<'w, M>) -> Option<&'w str> {
    match (group.axis, group.key, group.style.overflow) {
        (Some(_), Some(key), Some(Overflow::Scroll)) => Some(key),
        _ => None,
    }
}

/// Background, shadow and border for a box, with `style` overriding `defaults`.
pub(super) fn push_box(
    out: &mut Scene,
    rect: Rect4,
    style: &Style,
    defaults: BoxDefaults,
    theme: &Theme,
) {
    if let Some(shadow) = &style.shadow {
        let shadow_color = shadow.color.resolve(theme);
        let shifted = [
            rect[0] + shadow.offset_x,
            rect[1] + shadow.offset_y,
            rect[2],
            rect[3],
        ];
        out.push(quad(
            shifted,
            color(shadow_color),
            shadow.blur,
            0.0,
            Color::TRANSPARENT,
        ));
    }
    let fill = style
        .background
        .as_ref()
        .map(|token| token.resolve(theme))
        .or(defaults.fill);
    let border = style
        .border_color
        .as_ref()
        .map(|token| token.resolve(theme))
        .or(defaults.border);
    let border_width = style.border_width.unwrap_or(if border.is_some() {
        defaults.border_width
    } else {
        0.0
    });
    if fill.is_none() && border_width <= 0.0 {
        return;
    }
    let radius = style.border_radius.unwrap_or(defaults.radius);
    out.push(quad(
        rect,
        fill.map_or(Color::TRANSPARENT, color),
        radius,
        border_width,
        border.map_or(Color::TRANSPARENT, color),
    ));
}

fn trailing_overflow(end: f32, viewport_end: f32, trailing: f32) -> f32 {
    if !end.is_finite() {
        return 0.0;
    }
    let overflow = end + finite_non_negative(trailing) - viewport_end;
    if overflow > OVERFLOW_SLACK {
        overflow
    } else {
        0.0
    }
}

impl<M: Clone> Painter<'_, M> {
    /// Window-space rectangle of a laid-out node after scroll translation.
    pub(super) fn place(&self, layout: &LayoutNode, ctx: Ctx) -> Option<Rect4> {
        let snapped = self.snapshot.get(layout.id)?;
        let rect = [
            snapped.x - ctx.dx,
            snapped.y - ctx.dy,
            snapped.width,
            snapped.height,
        ];
        is_finite_rect(rect).then_some(rect)
    }

    pub(super) fn bump_seq(&mut self) -> usize {
        let seq = self.seq;
        self.seq += 1;
        seq
    }

    /// Adds a region; its paint sequence number is assigned here.
    pub(super) fn push_region(&mut self, mut region: Region<M>) {
        region.seq = self.bump_seq();
        self.frame.regions.push(region);
    }

    /// Bounding box of a node and all of its laid-out descendants.
    fn subtree_bounds(&self, layout: &LayoutNode, ctx: Ctx) -> Option<Rect4> {
        let own = self.place(layout, ctx);
        layout
            .children
            .iter()
            .fold(own, |acc, child| match self.subtree_bounds(child, ctx) {
                Some(rect) => Some(join(acc, rect)),
                None => acc,
            })
    }

    /// Whether anything of `widget` can be seen through the current clip.
    fn may_show(&self, widget: &WidgetNode<M>, layout: &LayoutNode, ctx: Ctx) -> bool {
        let bounds = if spans_children(widget) {
            self.subtree_bounds(layout, ctx)
        } else {
            self.place(layout, ctx)
        };
        bounds.is_some_and(|rect| visible_in(rect, ctx.clip))
    }

    pub(super) fn paint(&mut self, widget: &WidgetNode<M>, layout: &LayoutNode, ctx: Ctx) {
        if !self.may_show(widget, layout, ctx) {
            self.skip(widget, layout);
            return;
        }
        if let Some(group) = group_of(widget) {
            self.paint_group(&group, layout, ctx);
            return;
        }
        match widget {
            WidgetNode::Label(label) => self.paint_label(label, layout, ctx),
            WidgetNode::Button(button) => self.paint_button(button, layout, ctx),
            WidgetNode::Separator(_) => self.paint_separator(layout, ctx),
            WidgetNode::TextInput(input) => self.paint_text_input(input, layout, ctx),
            WidgetNode::Tooltip(tooltip) => self.paint(&tooltip.child, layout, ctx),
            WidgetNode::Popover(popover) => {
                if let Some(first) = popover.children.first() {
                    self.paint(first, layout, ctx);
                }
            }
            _ => tracing::debug!("ui runtime: unsupported widget skipped"),
        }
    }

    /// Records the bookkeeping of a node that cannot be seen, without painting,
    /// shaping or emitting regions. Mirrors `paint` so ids and sequences match.
    fn skip(&mut self, widget: &WidgetNode<M>, layout: &LayoutNode) {
        if let Some(group) = group_of(widget) {
            self.skip_group(&group, layout);
            return;
        }
        match widget {
            WidgetNode::Tooltip(tooltip) => self.skip(&tooltip.child, layout),
            WidgetNode::Popover(popover) => {
                if let Some(first) = popover.children.first() {
                    self.skip(first, layout);
                }
            }
            WidgetNode::Button(button) if !button.disabled => {
                self.bump_seq();
                if self.button_has_focus(button.key.as_str()) {
                    self.focus_found = true;
                }
            }
            WidgetNode::TextInput(input) if !input.disabled => {
                self.bump_seq();
                if self.input_has_focus(input.key.as_str()) {
                    self.focus_found = true;
                }
            }
            _ => {}
        }
    }

    fn skip_group(&mut self, group: &Group<'_, M>, layout: &LayoutNode) {
        let modal = modal_key(group.key);
        let clickable = group.message.is_some() || modal.is_some();
        let focusable = is_focusable(group);
        if clickable {
            self.claim_container(group.key, focusable);
        }
        if focusable && group.key.is_some_and(|key| self.container_has_focus(key)) {
            self.focus_found = true;
        }
        let span_start = self.seq;
        if clickable {
            self.bump_seq();
        }
        if let Some(key) = scroll_key(group) {
            self.seen_scroll.insert(key.to_owned());
            self.bump_seq();
        }
        for (child, child_layout) in group.children.iter().zip(layout.children.iter()) {
            self.skip(child, child_layout);
        }
        if let Some(key) = modal {
            self.close_modal(span_start, key);
        }
    }

    fn card_defaults(&self, card: Option<&Card<M>>) -> BoxDefaults {
        let Some(card) = card else {
            return BoxDefaults::NONE;
        };
        let surface = match card.variant {
            CardVariant::Muted => self.theme.colors.muted,
            _ => self.theme.colors.surface,
        };
        BoxDefaults {
            fill: Some(card.background_color.unwrap_or(surface)),
            border: Some(self.theme.colors.border),
            radius: card.border_radius.unwrap_or(self.theme.radii.md),
            border_width: 1.0,
        }
    }

    /// Whether the button with `key` holds keyboard focus.
    pub(super) fn button_has_focus(&self, key: &str) -> bool {
        matches!(self.focus.as_ref(), Some(Focus::Button { key: k }) if k.as_str() == key)
    }

    /// Whether the text input with `key` holds focus.
    pub(super) fn input_has_focus(&self, key: &str) -> bool {
        matches!(self.focus.as_ref(), Some(Focus::Input { key: k, .. }) if k.as_str() == key)
    }

    fn container_has_focus(&self, key: &str) -> bool {
        matches!(self.focus.as_ref(), Some(Focus::Container { key: k }) if k.as_str() == key)
    }

    fn paint_group(&mut self, group: &Group<'_, M>, layout: &LayoutNode, ctx: Ctx) {
        let Some(rect) = self.place(layout, ctx) else {
            return;
        };
        let defaults = self.card_defaults(group.card);
        push_box(&mut self.out, rect, group.style, defaults, self.theme);
        let modal = modal_key(group.key);
        let clickable = group.message.is_some() || modal.is_some();
        let focusable = is_focusable(group);
        let radius = group.style.border_radius.unwrap_or(defaults.radius);
        let target = clickable.then(|| self.claim_container(group.key, focusable));
        let focused = focusable && group.key.is_some_and(|key| self.container_has_focus(key));
        if let (Some(target), Some(_)) = (&target, group.message) {
            self.paint_container_hover(target, rect, radius);
        }
        self.paint_image_slot(group.key, rect);
        let span_start = self.seq;
        if let Some(target) = &target {
            self.push_region(Region {
                target: target.clone(),
                rect,
                clip: ctx.clip,
                message: group.message.cloned(),
                input: None,
                blocks: modal.is_some(),
                seq: 0,
            });
        }
        match scroll_key(group) {
            Some(key) => self.paint_scroll(group, key, layout, rect, ctx),
            None => self.paint_children(group.children, layout, ctx),
        }
        if focused {
            self.focus_found = true;
            if self.focus_visible {
                self.out.push(quad(
                    rect,
                    Color::TRANSPARENT,
                    radius,
                    FOCUS_RING_WIDTH,
                    color(self.theme.colors.ring),
                ));
            }
        }
        if let Some(key) = modal {
            self.close_modal(span_start, key);
        }
    }

    /// Records the paint span of a `modal:` container. The last-started modal
    /// restricts Tab focus and receives keyboard input.
    fn close_modal(&mut self, start: usize, key: &str) {
        let end = self.seq;
        if self
            .frame
            .modal
            .as_ref()
            .is_none_or(|known| start > known.start)
        {
            self.frame.modal = Some(ModalSpan {
                start,
                end,
                key: key.to_owned(),
            });
        }
    }

    /// Allocates the next container id. A keyed focusable container is
    /// identified by its key; every other container by its paint-order index.
    fn claim_container(&mut self, key: Option<&str>, focusable: bool) -> Target {
        let index = self.container_seq;
        self.container_seq += 1;
        match key {
            Some(key) if focusable => Target::Keyed(key.to_owned()),
            _ => Target::Container(index),
        }
    }

    fn paint_container_hover(&mut self, target: &Target, rect: Rect4, radius: f32) {
        if self.hover.as_ref() != Some(target) {
            return;
        }
        let fill = if self.pressed.as_ref() == Some(target) {
            self.theme.colors.ghost_pressed
        } else {
            self.theme.colors.ghost_hover
        };
        self.out
            .push(quad(rect, color(fill), radius, 0.0, Color::TRANSPARENT));
    }

    fn paint_image_slot(&mut self, key: Option<&str>, rect: Rect4) {
        let Some(key) = key.filter(|key| key.starts_with(IMAGE_SLOT_PREFIX)) else {
            return;
        };
        let Some(image) = self.images.get(key) else {
            return;
        };
        self.out.push(DrawCommand::Image(ImagePrimitive::contain(
            image.clone(),
            to_rect(rect),
        )));
    }

    fn paint_children(&mut self, children: &[WidgetNode<M>], layout: &LayoutNode, ctx: Ctx) {
        for (child, child_layout) in children.iter().zip(layout.children.iter()) {
            self.paint(child, child_layout, ctx);
        }
    }

    fn paint_scroll(
        &mut self,
        group: &Group<'_, M>,
        key: &str,
        layout: &LayoutNode,
        rect: Rect4,
        ctx: Ctx,
    ) {
        let base = [rect[0] + ctx.dx, rect[1] + ctx.dy, rect[2], rect[3]];
        let max = self.overflow_extent(layout, base, group.padding);
        let stored = self.scroll_offsets.get(key).copied().unwrap_or_default();
        let offset = Offset2 {
            x: clamp_offset(stored.x, max.x),
            y: clamp_offset(stored.y, max.y),
        };
        self.scroll_offsets.insert(key.to_owned(), offset);
        self.seen_scroll.insert(key.to_owned());
        let seq = self.bump_seq();
        self.frame.scroll.push(ScrollRegion {
            key: key.to_owned(),
            viewport: rect,
            clip: ctx.clip,
            max,
            seq,
        });
        let child_ctx = Ctx {
            dx: ctx.dx + offset.x,
            dy: ctx.dy + offset.y,
            clip: Some(intersect(ctx.clip, rect)),
        };
        self.out.push(DrawCommand::PushClip(to_rect(rect)));
        self.paint_children(group.children, layout, child_ctx);
        self.out.push(DrawCommand::PopClip);
        self.paint_scrollbar(rect, true, offset.x, max.x);
        self.paint_scrollbar(rect, false, offset.y, max.y);
    }

    /// Distance each axis's content extends past the viewport's trailing edge
    /// (plus the trailing padding), in snapshot space.
    fn overflow_extent(&self, layout: &LayoutNode, base: Rect4, padding: &Edges) -> Offset2 {
        let (mut end_x, mut end_y) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
        for child in &layout.children {
            let Some(rect) = self.snapshot.get(child.id) else {
                continue;
            };
            let (right, bottom) = (rect.x + rect.width, rect.y + rect.height);
            if right.is_finite() {
                end_x = end_x.max(right);
            }
            if bottom.is_finite() {
                end_y = end_y.max(bottom);
            }
        }
        Offset2 {
            x: trailing_overflow(end_x, base[0] + base[2], padding.right),
            y: trailing_overflow(end_y, base[1] + base[3], padding.bottom),
        }
    }

    fn paint_scrollbar(&mut self, rect: Rect4, horizontal: bool, offset: f32, max: f32) {
        if max <= 0.0 {
            return;
        }
        let viewport = if horizontal { rect[2] } else { rect[3] };
        if viewport <= 0.0 || !viewport.is_finite() {
            return;
        }
        let thumb = (viewport * viewport / (viewport + max))
            .clamp(SCROLLBAR_MIN_THUMB.min(viewport), viewport);
        let travel = viewport - thumb;
        let position = offset / max * travel;
        let muted = self.theme.colors.muted_foreground;
        let fill = ThemeColor::rgba(muted.red, muted.green, muted.blue, 0.5);
        let thumb_rect = if horizontal {
            [
                rect[0] + position,
                rect[1] + rect[3] - SCROLLBAR_WIDTH - SCROLLBAR_INSET,
                thumb,
                SCROLLBAR_WIDTH,
            ]
        } else {
            [
                rect[0] + rect[2] - SCROLLBAR_WIDTH - SCROLLBAR_INSET,
                rect[1] + position,
                SCROLLBAR_WIDTH,
                thumb,
            ]
        };
        self.out.push(quad(
            thumb_rect,
            color(fill),
            SCROLLBAR_WIDTH / 2.0,
            0.0,
            Color::TRANSPARENT,
        ));
    }
}
