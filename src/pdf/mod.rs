//! Fax document inspection and page preview (PDF and TIFF).
//! Owner: pdf worker. Public API = orchestrator contract.
//!
//! The format is detected from the file's magic bytes, never the extension.
//! Every failure is returned as a [`PreviewError`] value; a panic inside a
//! third-party parser is caught at this boundary and reported as
//! `Unsupported` so that malformed input cannot take the UI down.

mod ccitt;
mod pdf_doc;
mod pdf_image;
mod raster;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
mod tiff_doc;

use crate::model::PageImage;
use std::io::Read;
use std::panic::AssertUnwindSafe;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum PreviewError {
    #[error("unsupported or unreadable document: {0}")]
    Unsupported(String),
    #[error("page {0} out of range")]
    PageOutOfRange(u32),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Longest accepted side of a decoded page bitmap, in pixels.
const MAX_SIDE_PX: u32 = 20_000;
/// Largest accepted decoded page area, in pixels. A 600 dpi A4 fax page is about
/// 35 MP, so 40 MP covers real fax pages; as RGBA8 it is about 160 MB.
const MAX_PIXELS: u64 = 40_000_000;

/// Document container recognised from its magic bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DocKind {
    Pdf,
    Tiff,
}

/// Number of pages (PDF page tree / TIFF IFD count). Errors for unreadable files.
pub fn page_count(path: &Path) -> Result<u32, PreviewError> {
    guarded(|| match detect_kind(path)? {
        DocKind::Pdf => pdf_doc::page_count(path),
        DocKind::Tiff => tiff_doc::page_count(&std::fs::read(path)?),
    })
}

/// Render zero-based `page` to RGBA, downscaled so the longer side <= `max_side`
/// (never upscaled). Fax PDFs embed one raster image per page: support
/// DCTDecode (JPEG), CCITTFaxDecode (K<0 G4, K=0 G3-1D, K>0 G3-2D),
/// FlateDecode or no filter with 1-bit / 8-bit DeviceGray or DeviceRGB
/// (honour `Decode [1 0]` / `BlackIs1`). TIFF: `image` crate; CCITT-compressed
/// TIFF via the `fax` crate. Anything else → `PreviewError::Unsupported`
/// (UI falls back to "open externally"). Must never panic on malformed input.
pub fn render_page(path: &Path, page: u32, max_side: u32) -> Result<PageImage, PreviewError> {
    if max_side == 0 {
        return Err(unsupported("max_side must be at least 1"));
    }
    guarded(|| {
        let image = match detect_kind(path)? {
            DocKind::Pdf => pdf_doc::render_page(path, page)?,
            DocKind::Tiff => tiff_doc::render_page(&std::fs::read(path)?, page)?,
        };
        raster::fit_within(image, max_side)
    })
}

/// Reads the first four bytes and classifies the container.
fn detect_kind(path: &Path) -> Result<DocKind, PreviewError> {
    let mut file = std::fs::File::open(path)?;
    let mut magic = Vec::with_capacity(4);
    (&mut file).take(4).read_to_end(&mut magic)?;
    if magic.starts_with(b"%PDF") {
        return Ok(DocKind::Pdf);
    }
    if magic.starts_with(b"II*\0") || magic.starts_with(b"MM\0*") {
        return Ok(DocKind::Tiff);
    }
    Err(unsupported("neither a PDF nor a classic TIFF file"))
}

/// Runs decoder work and converts an unexpected panic into `Unsupported`.
fn guarded<T>(work: impl FnOnce() -> Result<T, PreviewError>) -> Result<T, PreviewError> {
    std::panic::catch_unwind(AssertUnwindSafe(work))
        .unwrap_or_else(|_| Err(unsupported("decoder failed on malformed input")))
}

/// Builds an `Unsupported` error from any displayable cause.
fn unsupported(cause: impl std::fmt::Display) -> PreviewError {
    PreviewError::Unsupported(cause.to_string())
}
