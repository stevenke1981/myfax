//! Fax document inspection and page preview (PDF and TIFF).
//! Owner: pdf worker. Public API = orchestrator contract.

use crate::model::PageImage;
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

/// Number of pages (PDF page tree / TIFF IFD count). Errors for unreadable files.
#[allow(unused_variables)]
pub fn page_count(path: &Path) -> Result<u32, PreviewError> {
    todo!()
}

/// Render zero-based `page` to RGBA, downscaled so the longer side <= `max_side`
/// (never upscaled). Fax PDFs embed one raster image per page: support
/// DCTDecode (JPEG), CCITTFaxDecode (K<0 G4, K=0 G3-1D, K>0 G3-2D),
/// FlateDecode or no filter with 1-bit / 8-bit DeviceGray or DeviceRGB
/// (honour `Decode [1 0]` / `BlackIs1`). TIFF: `image` crate; CCITT-compressed
/// TIFF via the `fax` crate. Anything else → `PreviewError::Unsupported`
/// (UI falls back to "open externally"). Must never panic on malformed input.
#[allow(unused_variables)]
pub fn render_page(path: &Path, page: u32, max_side: u32) -> Result<PageImage, PreviewError> {
    todo!()
}
