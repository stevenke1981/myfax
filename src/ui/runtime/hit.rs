//! Hit regions, focus targets and wheel routing, recorded in paint order.

use std::collections::HashMap;

use super::edit::InputSpec;
use super::geom::{Offset2, Rect4, clamp_offset, region_contains};

/// Pixels per wheel line in the runtime contract (the value a notch should move).
pub(super) const WHEEL_LINE_PX: f32 = 40.0;
/// acme-platform already scales `LineDelta` by 32 px per line before the event
/// reaches the runtime, so line deltas are rescaled from that convention.
pub(super) const PLATFORM_LINE_PX: f32 = 32.0;

/// Identity of a pointer or focus target between frames.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Target {
    Button(String),
    Input(String),
    /// A keyed clickable container: focusable and identified by its key.
    Keyed(String),
    /// An unkeyed clickable or modal container: index among such containers in
    /// paint order.
    Container(usize),
}

/// A clickable or focusable area recorded during paint.
pub(super) struct Region<M> {
    pub target: Target,
    pub rect: Rect4,
    pub clip: Option<Rect4>,
    pub message: Option<M>,
    pub input: Option<InputSpec>,
    /// `true` for `modal:` containers: they absorb pointer input over their
    /// rect for everything painted before them.
    pub blocks: bool,
    /// Paint sequence number shared with scroll regions.
    pub seq: usize,
}

/// A scroll viewport recorded during paint.
pub(super) struct ScrollRegion {
    pub key: String,
    pub viewport: Rect4,
    pub clip: Option<Rect4>,
    /// Largest offset per axis (zero when that axis does not overflow).
    pub max: Offset2,
    pub seq: usize,
}

/// Paint-sequence span `[start, end)` of a `modal:` container.
pub(super) struct ModalSpan {
    pub start: usize,
    pub end: usize,
    pub key: String,
}

/// Everything recorded by the last render that input handling needs.
pub(super) struct FrameState<M> {
    pub regions: Vec<Region<M>>,
    pub scroll: Vec<ScrollRegion>,
    pub ime_caret: Option<Rect4>,
    /// The last-started `modal:` container, if any.
    pub modal: Option<ModalSpan>,
}

impl<M> Default for FrameState<M> {
    fn default() -> Self {
        Self {
            regions: Vec::new(),
            scroll: Vec::new(),
            ime_caret: None,
            modal: None,
        }
    }
}

/// Topmost region under the point. Later paints win; `modal:` containers
/// return themselves for points inside them, hiding anything painted earlier.
pub(super) fn hit_index<M>(regions: &[Region<M>], x: f32, y: f32) -> Option<usize> {
    regions
        .iter()
        .rposition(|region| region_contains(region.rect, region.clip, x, y))
}

/// Target under the point, if any.
pub(super) fn target_at<M>(regions: &[Region<M>], x: f32, y: f32) -> Option<Target> {
    let index = hit_index(regions, x, y)?;
    regions.get(index).map(|region| region.target.clone())
}

/// Converts a wheel event to pixels: `(dx, dy)`, positive `dy` scrolls content up.
pub(super) fn wheel_pixels(dx: f32, dy: f32, lines: bool) -> (f32, f32) {
    if lines {
        (
            dx * WHEEL_LINE_PX / PLATFORM_LINE_PX,
            dy * WHEEL_LINE_PX / PLATFORM_LINE_PX,
        )
    } else {
        (dx, dy)
    }
}

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

/// Offset after one wheel delta. A vertical delta moves y when the region
/// overflows vertically; otherwise it moves x. A horizontal delta always moves x.
pub(super) fn scrolled(current: Offset2, max: Offset2, px: f32, py: f32) -> Offset2 {
    let (x_delta, y_delta) = if max.y > 0.0 {
        (px, py)
    } else if px != 0.0 {
        (px, 0.0)
    } else {
        (py, 0.0)
    };
    Offset2 {
        x: clamp_offset(current.x - x_delta, max.x),
        y: clamp_offset(current.y - y_delta, max.y),
    }
}

/// Scrolls the innermost unblocked scroll region under the pointer whose offset
/// can still move. Returns `true` when an offset changed.
pub(super) fn scroll_at<M>(
    frame: &FrameState<M>,
    offsets: &mut HashMap<String, Offset2>,
    dx: f32,
    dy: f32,
    lines: bool,
    x: f32,
    y: f32,
) -> bool {
    let (px, py) = wheel_pixels(dx, dy, lines);
    let (px, py) = (finite_or_zero(px), finite_or_zero(py));
    let blocking_seq = frame
        .regions
        .iter()
        .filter(|region| region.blocks && region_contains(region.rect, region.clip, x, y))
        .map(|region| region.seq)
        .max();
    for region in frame.scroll.iter().rev() {
        if blocking_seq.is_some_and(|blocker| region.seq < blocker) {
            continue;
        }
        if !region_contains(region.viewport, region.clip, x, y) {
            continue;
        }
        let current = offsets
            .get(region.key.as_str())
            .copied()
            .unwrap_or_default();
        let next = scrolled(current, region.max, px, py);
        if next != current {
            offsets.insert(region.key.clone(), next);
            return true;
        }
    }
    false
}
