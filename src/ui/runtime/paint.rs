//! Tree walk: containers, hover overlays, modal spans, scroll viewports and
//! image slots. Leaf widgets are painted by `widgets`.

use std::collections::{HashMap, HashSet};

use acme_core::{Color, DrawCommand, ImagePrimitive, Rgba8Image, Scene, WidgetKey};
use acme_layout::{Edges, LayoutNode, LayoutSnapshot, Overflow};
use acme_style::Style;
use acme_text::{FontSystem, GlyphAtlas};
use acme_theme::{Theme, ThemeColor};
use acme_widgets::{Card, CardVariant, WidgetNode};

use super::edit::Focus;
use super::geom::{
    Ctx, Rect4, clamp_offset, color, finite_non_negative, intersect, is_finite_rect, quad, to_rect,
};
use super::hit::{FrameState, Region, ScrollRegion, Target};

const IMAGE_SLOT_PREFIX: &str = "img:";
const MODAL_PREFIX: &str = "modal:";
const SCROLLBAR_WIDTH: f32 = 4.0;
const SCROLLBAR_INSET: f32 = 2.0;
const SCROLLBAR_MIN_THUMB: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Axis {
    Row,
    Column,
}

/// Paint state for one frame. Fields are owned by `UiRuntime` and borrowed here.
pub(super) struct Painter<'a, M> {
    pub fonts: &'a mut FontSystem,
    pub atlas: &'a mut GlyphAtlas,
    pub theme: &'a Theme,
    pub scale: f32,
    pub images: &'a HashMap<String, Rgba8Image>,
    pub snapshot: &'a LayoutSnapshot,
    pub scroll_offsets: &'a mut HashMap<String, f32>,
    pub seen_scroll: HashSet<String>,
    pub focus: &'a mut Option<Focus>,
    pub focus_found: bool,
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

    pub(super) fn paint(&mut self, widget: &WidgetNode<M>, layout: &LayoutNode, ctx: Ctx) {
        match widget {
            WidgetNode::Row(container) => self.paint_group(
                &Group {
                    key: key_str(&container.key),
                    style: &container.style,
                    message: container.message.as_ref(),
                    children: &container.children,
                    axis: Some(Axis::Row),
                    card: None,
                    padding: &container.padding,
                },
                layout,
                ctx,
            ),
            WidgetNode::Column(container) => self.paint_group(
                &Group {
                    key: key_str(&container.key),
                    style: &container.style,
                    message: container.message.as_ref(),
                    children: &container.children,
                    axis: Some(Axis::Column),
                    card: None,
                    padding: &container.padding,
                },
                layout,
                ctx,
            ),
            WidgetNode::Stack(container) => self.paint_group(
                &Group {
                    key: key_str(&container.key),
                    style: &container.style,
                    message: container.message.as_ref(),
                    children: &container.children,
                    axis: None,
                    card: None,
                    padding: &container.padding,
                },
                layout,
                ctx,
            ),
            WidgetNode::Card(card) => self.paint_group(
                &Group {
                    key: key_str(&card.key),
                    style: &card.style,
                    message: None,
                    children: &card.children,
                    axis: None,
                    card: Some(card),
                    padding: &card.padding,
                },
                layout,
                ctx,
            ),
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

    fn paint_group(&mut self, group: &Group<'_, M>, layout: &LayoutNode, ctx: Ctx) {
        let Some(rect) = self.place(layout, ctx) else {
            return;
        };
        let defaults = self.card_defaults(group.card);
        push_box(&mut self.out, rect, group.style, defaults, self.theme);
        let modal = group.key.is_some_and(|key| key.starts_with(MODAL_PREFIX));
        let clickable = group.message.is_some() || modal;
        let container = clickable.then(|| self.alloc_container());
        if let (Some(index), Some(_)) = (container, group.message) {
            let radius = group.style.border_radius.unwrap_or(defaults.radius);
            self.paint_container_hover(index, rect, radius);
        }
        self.paint_image_slot(group.key, rect);
        let span_start = self.seq;
        if let Some(index) = container {
            self.push_region(Region {
                target: Target::Container(index),
                rect,
                clip: ctx.clip,
                message: group.message.cloned(),
                input: None,
                blocks: modal,
                seq: 0,
            });
        }
        match (group.axis, group.key, group.style.overflow) {
            (Some(axis), Some(key), Some(Overflow::Scroll)) => {
                self.paint_scroll(group, key, axis, layout, rect, ctx);
            }
            _ => self.paint_children(group.children, layout, ctx),
        }
        if modal {
            self.close_modal(span_start);
        }
    }

    /// Records the paint span of a `modal:` container. The last-started modal
    /// restricts Tab focus.
    fn close_modal(&mut self, start: usize) {
        let end = self.seq;
        if self.frame.modal.is_none_or(|(known, _)| start > known) {
            self.frame.modal = Some((start, end));
        }
    }

    fn alloc_container(&mut self) -> usize {
        let index = self.container_seq;
        self.container_seq += 1;
        index
    }

    fn paint_container_hover(&mut self, index: usize, rect: Rect4, radius: f32) {
        let hovered = matches!(&self.hover, Some(Target::Container(i)) if *i == index);
        if !hovered {
            return;
        }
        let pressed = matches!(&self.pressed, Some(Target::Container(i)) if *i == index);
        let fill = if pressed {
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
        axis: Axis,
        layout: &LayoutNode,
        rect: Rect4,
        ctx: Ctx,
    ) {
        let horizontal = axis == Axis::Row;
        let base = [rect[0] + ctx.dx, rect[1] + ctx.dy, rect[2], rect[3]];
        let max = self.overflow_extent(layout, base, horizontal, group.padding);
        let stored = self.scroll_offsets.get(key).copied().unwrap_or(0.0);
        let offset = clamp_offset(stored, max);
        self.scroll_offsets.insert(key.to_owned(), offset);
        self.seen_scroll.insert(key.to_owned());
        let seq = self.bump_seq();
        self.frame.scroll.push(ScrollRegion {
            key: key.to_owned(),
            horizontal,
            viewport: rect,
            clip: ctx.clip,
            max,
            seq,
        });
        let (dx, dy) = if horizontal {
            (ctx.dx + offset, ctx.dy)
        } else {
            (ctx.dx, ctx.dy + offset)
        };
        let child_ctx = Ctx {
            dx,
            dy,
            clip: Some(intersect(ctx.clip, rect)),
        };
        self.out.push(DrawCommand::PushClip(to_rect(rect)));
        self.paint_children(group.children, layout, child_ctx);
        self.out.push(DrawCommand::PopClip);
        self.paint_scrollbar(rect, horizontal, offset, max);
    }

    /// Distance the content extends past the viewport's trailing edge (plus
    /// the trailing padding), in snapshot space.
    fn overflow_extent(
        &self,
        layout: &LayoutNode,
        base: Rect4,
        horizontal: bool,
        padding: &Edges,
    ) -> f32 {
        let mut end = f32::NEG_INFINITY;
        for child in &layout.children {
            let Some(child_rect) = self.snapshot.get(child.id) else {
                continue;
            };
            let edge = if horizontal {
                child_rect.x + child_rect.width
            } else {
                child_rect.y + child_rect.height
            };
            if edge.is_finite() {
                end = end.max(edge);
            }
        }
        if !end.is_finite() {
            return 0.0;
        }
        let (viewport_end, trailing) = if horizontal {
            (base[0] + base[2], padding.right)
        } else {
            (base[1] + base[3], padding.bottom)
        };
        (end + finite_non_negative(trailing) - viewport_end).max(0.0)
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
