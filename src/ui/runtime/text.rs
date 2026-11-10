//! Glyph conversion into [`DrawCommand::Text`] and text style helpers. Shaping
//! itself lives in `cache` so that results can be reused across frames.

use acme_core::{
    AtlasUpload, Color, DrawCommand, GlyphDraw, GlyphFormat, Point, Scene, TextPrimitive,
};
use acme_text::{AtlasFormat, FontSystem, GlyphAtlas, TextLayout, TextStyle};
use acme_theme::ThemeColor;

pub(super) fn text_style(font_size: f32, line_height: f32) -> TextStyle {
    TextStyle {
        font_size,
        line_height,
        ..TextStyle::default()
    }
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
