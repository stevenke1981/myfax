//! Shaped-text cache for the runtime.
//!
//! Shaping (`FontSystem::shape`) is the dominant paint cost. Results are kept
//! across frames, keyed by everything that changes the output: text, style
//! fields, wrap mode, width and device scale. Fit results (single-line text cut
//! with an ellipsis) are cached per width as well. Both maps are bounded: when
//! full they are cleared, which is cheap and self-healing for a UI that
//! re-renders the same content every frame.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use acme_text::{FontSystem, TextConstraints, TextLayout, TextStyle, TextWrap};

const ELLIPSIS: &str = "\u{2026}";

/// Tolerance (logical px) before a single-line label is ellipsised.
const FIT_SLACK: f32 = 1.0;

/// Upper bound per map before the map is cleared.
const MAX_ENTRIES: usize = 4096;

/// Style fields that influence shaping, with floats stored as bits.
#[derive(Clone, PartialEq, Eq, Hash)]
struct StyleKey {
    family: String,
    font_bits: u32,
    line_bits: u32,
    bold: bool,
    italic: bool,
}

impl StyleKey {
    fn of(style: &TextStyle) -> Self {
        Self {
            family: style.family.clone(),
            font_bits: style.font_size.to_bits(),
            line_bits: style.line_height.to_bits(),
            bold: style.bold,
            italic: style.italic,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct ShapeKey {
    text: String,
    style: StyleKey,
    max_width: Option<u32>,
    wrap: u8,
    scale: u32,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct FitKey {
    text: String,
    style: StyleKey,
    width: u32,
    scale: u32,
}

fn wrap_tag(wrap: TextWrap) -> u8 {
    match wrap {
        TextWrap::None => 0,
        TextWrap::Word => 1,
        TextWrap::Glyph => 2,
        TextWrap::WordOrGlyph => 3,
    }
}

fn bounded_insert<K: Eq + Hash, V>(map: &mut HashMap<K, V>, key: K, value: V) {
    if map.len() >= MAX_ENTRIES {
        map.clear();
    }
    map.insert(key, value);
}

#[derive(Default)]
pub(super) struct TextCache {
    shaped: HashMap<ShapeKey, Arc<TextLayout>>,
    fitted: HashMap<FitKey, Arc<TextLayout>>,
    /// Real `FontSystem::shape` calls made by this runtime (test builds only).
    #[cfg(test)]
    pub(super) shape_calls: usize,
}

impl TextCache {
    /// Shapes `text`, reusing an earlier identical result.
    pub(super) fn shape(
        &mut self,
        fonts: &mut FontSystem,
        text: &str,
        style: &TextStyle,
        max_width: Option<f32>,
        wrap: TextWrap,
        scale: f32,
    ) -> Arc<TextLayout> {
        let key = ShapeKey {
            text: text.to_owned(),
            style: StyleKey::of(style),
            max_width: max_width.map(f32::to_bits),
            wrap: wrap_tag(wrap),
            scale: scale.to_bits(),
        };
        if let Some(hit) = self.shaped.get(&key) {
            return Arc::clone(hit);
        }
        let layout = Arc::new(self.shape_uncached(fonts, text, style, max_width, wrap, scale));
        bounded_insert(&mut self.shaped, key, Arc::clone(&layout));
        layout
    }

    fn shape_uncached(
        &mut self,
        fonts: &mut FontSystem,
        text: &str,
        style: &TextStyle,
        max_width: Option<f32>,
        wrap: TextWrap,
        scale: f32,
    ) -> TextLayout {
        #[cfg(test)]
        {
            self.shape_calls += 1;
        }
        fonts.shape(text, style, TextConstraints { max_width, wrap }, scale)
    }

    /// Single-line shaping that ends in an ellipsis when the text is wider than `max_width`.
    pub(super) fn fit_line(
        &mut self,
        fonts: &mut FontSystem,
        text: &str,
        style: &TextStyle,
        max_width: f32,
        scale: f32,
    ) -> Arc<TextLayout> {
        let key = FitKey {
            text: text.to_owned(),
            style: StyleKey::of(style),
            width: max_width.to_bits(),
            scale: scale.to_bits(),
        };
        if let Some(hit) = self.fitted.get(&key) {
            return Arc::clone(hit);
        }
        let layout = self.fit_uncached(fonts, text, style, max_width, scale);
        bounded_insert(&mut self.fitted, key, Arc::clone(&layout));
        layout
    }

    fn fit_uncached(
        &mut self,
        fonts: &mut FontSystem,
        text: &str,
        style: &TextStyle,
        max_width: f32,
        scale: f32,
    ) -> Arc<TextLayout> {
        let full = self.shape(fonts, text, style, None, TextWrap::None, scale);
        // Layout measurement and paint shaping can differ by sub-pixel amounts
        // (hinting at the device scale); a label laid out at exactly its measured
        // width must not be ellipsised.
        if full.width <= max_width + FIT_SLACK || text.is_empty() {
            return full;
        }
        let starts: Vec<usize> = text.char_indices().map(|(index, _)| index).collect();
        let (mut low, mut high) = (0usize, starts.len());
        while low < high {
            let mid = (low + high).div_ceil(2);
            let end = starts.get(mid).copied().unwrap_or(text.len());
            let candidate = format!("{}{ELLIPSIS}", text.get(..end).unwrap_or(""));
            let width = self
                .shape(fonts, &candidate, style, None, TextWrap::None, scale)
                .width;
            if width <= max_width {
                low = mid;
            } else {
                high = mid - 1;
            }
        }
        let end = starts.get(low).copied().unwrap_or(text.len());
        let shown = format!("{}{ELLIPSIS}", text.get(..end).unwrap_or(""));
        self.shape(fonts, &shown, style, None, TextWrap::None, scale)
    }
}
