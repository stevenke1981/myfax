//! Text shaping and glyph conversion into [`DrawCommand::Text`].

use acme_core::{
    AtlasUpload, Color, DrawCommand, GlyphDraw, GlyphFormat, Point, Scene, TextPrimitive,
};
use acme_text::{
    AtlasFormat, FontSystem, GlyphAtlas, TextConstraints, TextLayout, TextStyle, TextWrap,
};
use acme_theme::ThemeColor;

const ELLIPSIS: &str = "\u{2026}";

pub(super) fn text_style(font_size: f32, line_height: f32) -> TextStyle {
    TextStyle {
        font_size,
        line_height,
        ..TextStyle::default()
    }
}

pub(super) fn shape(
    fonts: &mut FontSystem,
    text: &str,
    style: &TextStyle,
    max_width: Option<f32>,
    wrap: TextWrap,
    scale: f32,
) -> TextLayout {
    fonts.shape(text, style, TextConstraints { max_width, wrap }, scale)
}

/// Single-line shaping that ends in an ellipsis when the text is wider than `max_width`.
pub(super) fn fit_line(
    fonts: &mut FontSystem,
    text: &str,
    style: &TextStyle,
    max_width: f32,
    scale: f32,
) -> TextLayout {
    let full = shape(fonts, text, style, None, TextWrap::None, scale);
    if full.width <= max_width || text.is_empty() {
        return full;
    }
    let starts: Vec<usize> = text.char_indices().map(|(index, _)| index).collect();
    let (mut low, mut high) = (0usize, starts.len());
    while low < high {
        let mid = (low + high).div_ceil(2);
        let end = starts.get(mid).copied().unwrap_or(text.len());
        let candidate = format!("{}{ELLIPSIS}", text.get(..end).unwrap_or(""));
        let width = shape(fonts, &candidate, style, None, TextWrap::None, scale).width;
        if width <= max_width {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    let end = starts.get(low).copied().unwrap_or(text.len());
    let shown = format!("{}{ELLIPSIS}", text.get(..end).unwrap_or(""));
    shape(fonts, &shown, style, None, TextWrap::None, scale)
}

fn glyph_format(format: AtlasFormat) -> GlyphFormat {
    match format {
        AtlasFormat::Alpha8 => GlyphFormat::Alpha8,
        AtlasFormat::Rgba8 => GlyphFormat::Rgba8,
    }
}

/// Rasterizes `layout` into the atlas and appends a text draw. `origin` is the
/// first baseline (the text box top plus [`TextLayout::baseline`]).
pub(super) fn push_text(
    fonts: &mut FontSystem,
    atlas: &mut GlyphAtlas,
    out: &mut Scene,
    layout: &TextLayout,
    origin: [f32; 2],
    color: ThemeColor,
) {
    if layout.glyphs.is_empty() {
        return;
    }
    let prepared = fonts.prepare(layout, atlas);
    let glyphs = prepared
        .glyphs
        .iter()
        .map(|glyph| GlyphDraw {
            x: glyph.x as f32,
            y: glyph.y as f32,
            width: glyph.width as f32,
            height: glyph.height as f32,
            atlas_x: glyph.atlas_x,
            atlas_y: glyph.atlas_y,
            atlas_page: glyph.atlas_page,
            format: glyph_format(glyph.format),
        })
        .collect();
    let uploads = prepared
        .uploads
        .into_iter()
        .map(|upload| AtlasUpload {
            page: upload.page,
            origin: [upload.x, upload.y],
            size: [upload.width, upload.height],
            format: glyph_format(upload.format),
            pixels: upload.pixels,
        })
        .collect();
    out.push(DrawCommand::Text(TextPrimitive {
        origin: Point::new(origin[0], origin[1]),
        color: Color::rgba(color.red, color.green, color.blue, color.alpha),
        glyphs,
        uploads,
    }));
}
