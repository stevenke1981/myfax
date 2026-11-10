//! Rectangle, colour and clip helpers shared by the runtime submodules.
//!
//! All rectangles are `[x, y, width, height]` in logical pixels, window space.

use acme_core::{Color, DrawCommand, Logical, QuadPrimitive, Rect};
use acme_theme::ThemeColor;

/// `[x, y, width, height]` in logical pixels.
pub(super) type Rect4 = [f32; 4];

/// Width of the keyboard focus ring (theme `ring` colour) on focusable widgets.
pub(super) const FOCUS_RING_WIDTH: f32 = 2.0;

/// Scroll offsets along each axis, in logical pixels (always `>= 0`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Offset2 {
    pub x: f32,
    pub y: f32,
}

/// Paint-time translation accumulated from enclosing scroll regions, plus the
/// clip that applies to painting and hit-testing inside them.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Ctx {
    pub dx: f32,
    pub dy: f32,
    pub clip: Option<Rect4>,
}

pub(super) fn is_finite_rect(rect: Rect4) -> bool {
    rect.iter().all(|value| value.is_finite())
}

/// Half-open point-in-rect test.
pub(super) fn contains(rect: Rect4, x: f32, y: f32) -> bool {
    x >= rect[0] && x < rect[0] + rect[2] && y >= rect[1] && y < rect[1] + rect[3]
}

/// Point hit test that honours an optional clip rectangle.
pub(super) fn region_contains(rect: Rect4, clip: Option<Rect4>, x: f32, y: f32) -> bool {
    contains(rect, x, y) && clip.is_none_or(|clip| contains(clip, x, y))
}

/// Intersects `rect` with the optional `clip`; a disjoint pair yields a
/// zero-sized rectangle, so nothing inside it is hit.
pub(super) fn intersect(clip: Option<Rect4>, rect: Rect4) -> Rect4 {
    let Some(clip) = clip else {
        return rect;
    };
    let x0 = clip[0].max(rect[0]);
    let y0 = clip[1].max(rect[1]);
    let x1 = (clip[0] + clip[2]).min(rect[0] + rect[2]);
    let y1 = (clip[1] + clip[3]).min(rect[1] + rect[3]);
    [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)]
}

/// Whether anything of `rect` can be seen through `clip`: both have positive
/// area and overlap. No clip means unclipped.
pub(super) fn visible_in(rect: Rect4, clip: Option<Rect4>) -> bool {
    let Some(clip) = clip else {
        return true;
    };
    let has_area = |r: Rect4| r[2] > 0.0 && r[3] > 0.0;
    has_area(rect)
        && has_area(clip)
        && rect[0] < clip[0] + clip[2]
        && rect[0] + rect[2] > clip[0]
        && rect[1] < clip[1] + clip[3]
        && rect[1] + rect[3] > clip[1]
}

/// Bounding box of `acc` (when present) and `rect`.
pub(super) fn join(acc: Option<Rect4>, rect: Rect4) -> Rect4 {
    let Some(a) = acc else {
        return rect;
    };
    let x0 = a[0].min(rect[0]);
    let y0 = a[1].min(rect[1]);
    let x1 = (a[0] + a[2]).max(rect[0] + rect[2]);
    let y1 = (a[1] + a[3]).max(rect[1] + rect[3]);
    [x0, y0, x1 - x0, y1 - y0]
}

pub(super) fn to_rect(rect: Rect4) -> Rect<Logical> {
    Rect::new(rect[0], rect[1], rect[2], rect[3])
}

pub(super) fn color(value: ThemeColor) -> Color {
    Color::rgba(value.red, value.green, value.blue, value.alpha)
}

pub(super) fn finite_non_negative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

/// Clamps a scroll offset into `[0, max]`; NaN-safe.
pub(super) fn clamp_offset(value: f32, max: f32) -> f32 {
    let max = finite_non_negative(max);
    if value.is_finite() {
        value.clamp(0.0, max)
    } else {
        0.0
    }
}

pub(super) fn quad(
    rect: Rect4,
    fill: Color,
    radius: f32,
    border_width: f32,
    border: Color,
) -> DrawCommand {
    DrawCommand::Quad(QuadPrimitive {
        rect: to_rect(rect),
        color: fill,
        radius,
        border_width,
        border_color: border,
    })
}
