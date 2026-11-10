//! Page preview of the selected fax: render, page stepping and zoom.

use super::{Controller, PREVIEW_IMAGE_KEY};
use crate::app::state::{PreviewState, PreviewStatus};
use crate::i18n::{Lang, t, tf};
use crate::model::PageImage;
use crate::pdf::{self, PreviewError};
use acme_core::{ImageKey, Rgba8Image};
use std::io::ErrorKind;
use std::path::Path;

/// Longest side of the rendered page bitmap, in pixels.
const PREVIEW_MAX_SIDE_PX: u32 = 1800;
/// Zoom changes per click.
const ZOOM_STEP: f32 = 0.25;
const ZOOM_MIN: f32 = 0.5;
const ZOOM_MAX: f32 = 3.0;
/// Image identity inside the preview slot; the revision changes per render.
const PREVIEW_IMAGE_IDENTITY: u64 = 1;

/// Localised explanation for a failed preview.
fn preview_error_text(lang: Lang, error: &PreviewError) -> String {
    match error {
        // The pdf module reports encryption as `Unsupported("encrypted PDF")`.
        PreviewError::Unsupported(cause) if cause.contains("encrypted") => {
            t(lang, "app.preview_encrypted").to_owned()
        }
        PreviewError::Unsupported(_) => t(lang, "app.preview_unavailable").to_owned(),
        PreviewError::Io(io) if io.kind() == ErrorKind::NotFound => {
            t(lang, "app.preview_file_missing").to_owned()
        }
        PreviewError::PageOutOfRange(_) | PreviewError::Io(_) => {
            tf(lang, "app.preview_failed", &[("error", &error.to_string())])
        }
    }
}

/// Zoom snapped to quarter steps and kept inside the allowed range.
fn snap_zoom(zoom: f32) -> f32 {
    ((zoom / ZOOM_STEP).round() * ZOOM_STEP).clamp(ZOOM_MIN, ZOOM_MAX)
}

impl Controller {
    /// Render page `page` of the selected fax into the preview slot.
    pub(super) fn load_preview_page(&mut self, page: u32) {
        let Some(path) = self
            .state
            .selected
            .as_ref()
            .map(|record| record.file_path.clone())
        else {
            self.clear_preview();
            return;
        };
        let lang = self.state.lang;
        if !path.is_file() {
            let text = t(lang, "app.preview_file_missing").to_owned();
            self.set_preview_unavailable(text);
            return;
        }
        let count = match pdf::page_count(&path) {
            Ok(0) => {
                let text = t(lang, "app.preview_unavailable").to_owned();
                self.set_preview_unavailable(text);
                return;
            }
            Ok(count) => count,
            Err(error) => {
                self.preview_failed(&path, &error);
                return;
            }
        };
        let page = page.min(count - 1);
        match pdf::render_page(&path, page, PREVIEW_MAX_SIDE_PX) {
            Ok(image) => self.show_preview(page, count, image),
            Err(error) => self.preview_failed(&path, &error),
        }
    }

    fn preview_failed(&mut self, path: &Path, error: &PreviewError) {
        tracing::warn!(path = %path.display(), %error, "fax preview unavailable");
        let text = preview_error_text(self.state.lang, error);
        self.set_preview_unavailable(text);
    }

    fn set_preview_unavailable(&mut self, text: String) {
        self.images.remove(PREVIEW_IMAGE_KEY);
        self.state.preview = PreviewState {
            page: 0,
            page_count: 0,
            zoom: self.state.preview.zoom,
            status: PreviewStatus::Unavailable(text),
            image_width: 0,
            image_height: 0,
        };
    }

    fn show_preview(&mut self, page: u32, count: u32, image: PageImage) {
        let lang = self.state.lang;
        let (width, height) = (image.width, image.height);
        self.preview_revision += 1;
        match Rgba8Image::new(
            ImageKey::new(PREVIEW_IMAGE_IDENTITY),
            self.preview_revision,
            width,
            height,
            image.rgba,
        ) {
            Ok(pixels) => {
                self.images.insert(PREVIEW_IMAGE_KEY.to_owned(), pixels);
                self.state.preview = PreviewState {
                    page,
                    page_count: count,
                    zoom: self.state.preview.zoom,
                    status: PreviewStatus::Ready,
                    image_width: width,
                    image_height: height,
                };
            }
            Err(error) => {
                let text = tf(lang, "app.preview_failed", &[("error", &error.to_string())]);
                self.set_preview_unavailable(text);
            }
        }
    }

    /// Move to the previous (`-1`) or next (`1`) page, clamped to the document.
    pub(super) fn preview_step(&mut self, delta: i32) {
        let count = self.state.preview.page_count;
        if count == 0 {
            return;
        }
        let target = i64::from(self.state.preview.page) + i64::from(delta);
        let page = target.clamp(0, i64::from(count) - 1);
        let page = u32::try_from(page).unwrap_or(0);
        self.load_preview_page(page);
    }

    /// Zoom in (`1`) or out (`-1`) by one step within 0.5..=3.0.
    pub(super) fn zoom_step(&mut self, direction: i32) {
        let zoom = self.state.preview.zoom + ZOOM_STEP * direction as f32;
        self.state.preview.zoom = snap_zoom(zoom);
    }

    pub(super) fn reset_zoom(&mut self) {
        self.state.preview.zoom = 1.0;
    }
}

#[cfg(test)]
mod tests {
    use super::snap_zoom;

    #[test]
    fn zoom_stays_in_range_and_on_quarter_steps() {
        assert_eq!(snap_zoom(1.1), 1.0);
        assert_eq!(snap_zoom(0.1), 0.5);
        assert_eq!(snap_zoom(9.0), 3.0);
        assert_eq!(snap_zoom(1.25 + 0.25), 1.5);
    }
}
