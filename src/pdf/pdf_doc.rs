//! PDF document access: loading, page tree, inherited resources and image selection.

use super::{PreviewError, pdf_image, unsupported};
use crate::model::PageImage;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use std::collections::HashSet;
use std::path::Path;

/// Deepest page-tree walk followed when looking for inherited resources.
const MAX_PAGE_TREE_DEPTH: usize = 64;
/// Form XObjects are followed this many levels deep (page → form → image).
const MAX_FORM_DEPTH: u8 = 1;

pub(super) fn page_count(path: &Path) -> Result<u32, PreviewError> {
    let document = load(path)?;
    u32::try_from(document.get_pages().len()).map_err(|_| unsupported("too many pages"))
}

pub(super) fn render_page(path: &Path, page: u32) -> Result<PageImage, PreviewError> {
    let document = load(path)?;
    let pages = document.get_pages();
    let page_number = page
        .checked_add(1)
        .ok_or(PreviewError::PageOutOfRange(page))?;
    let page_id = pages
        .get(&page_number)
        .copied()
        .ok_or(PreviewError::PageOutOfRange(page))?;
    let image = largest_image(&document, page_id)
        .ok_or_else(|| unsupported("page has no raster image to preview"))?;
    pdf_image::decode_image(&document, image)
}

/// Loads a PDF from disk, retrying from memory, and rejects encrypted files.
fn load(path: &Path) -> Result<Document, PreviewError> {
    let document = match Document::load(path) {
        Ok(document) => document,
        Err(first) => {
            let bytes = std::fs::read(path)?;
            Document::load_mem(&bytes)
                .map_err(|second| unsupported(format!("cannot parse PDF: {first}; {second}")))?
        }
    };
    if document.is_encrypted() {
        return Err(unsupported("encrypted PDF"));
    }
    Ok(document)
}

/// The image XObject with the largest area among the page's resources.
fn largest_image(document: &Document, page_id: ObjectId) -> Option<&Stream> {
    let resources = inherited_resources(document, page_id)?;
    let mut images = Vec::new();
    collect_images(document, resources, 0, &mut images);
    images
        .into_iter()
        .max_by_key(|stream| image_area(document, &stream.dict))
}

fn image_area(document: &Document, dict: &Dictionary) -> u64 {
    let width = get_i64(document, dict, b"Width").and_then(|v| u64::try_from(v).ok());
    let height = get_i64(document, dict, b"Height").and_then(|v| u64::try_from(v).ok());
    match (width, height) {
        (Some(w), Some(h)) => w.saturating_mul(h),
        _ => 0,
    }
}

/// Resources of the page, or of the nearest ancestor `Pages` node that has them.
fn inherited_resources(document: &Document, page_id: ObjectId) -> Option<&Dictionary> {
    let mut node = page_id;
    let mut seen = HashSet::new();
    for _ in 0..MAX_PAGE_TREE_DEPTH {
        if !seen.insert(node) {
            return None;
        }
        let dict = document.get_dictionary(node).ok()?;
        if let Some(resources) = get(document, dict, b"Resources") {
            return resources.as_dict().ok();
        }
        node = dict.get(b"Parent").ok()?.as_reference().ok()?;
    }
    None
}

/// Collects image XObjects, descending into form XObjects up to `MAX_FORM_DEPTH`.
fn collect_images<'a>(
    document: &'a Document,
    resources: &'a Dictionary,
    depth: u8,
    out: &mut Vec<&'a Stream>,
) {
    let Some(xobjects) = get(document, resources, b"XObject").and_then(|o| o.as_dict().ok()) else {
        return;
    };
    for (_, value) in xobjects.iter() {
        let Some(Object::Stream(stream)) = resolve(document, value) else {
            continue;
        };
        match get_name(document, &stream.dict, b"Subtype") {
            Some(b"Image") => out.push(stream),
            Some(b"Form") if depth < MAX_FORM_DEPTH => {
                let inner =
                    get(document, &stream.dict, b"Resources").and_then(|o| o.as_dict().ok());
                if let Some(inner) = inner {
                    collect_images(document, inner, depth + 1, out);
                }
            }
            _ => {}
        }
    }
}

/// Follows indirect references (bounded by lopdf's reference limit).
pub(super) fn resolve<'a>(document: &'a Document, object: &'a Object) -> Option<&'a Object> {
    document
        .dereference(object)
        .ok()
        .map(|(_, resolved)| resolved)
}

/// Looks up `key` in `dict` and resolves an indirect value.
pub(super) fn get<'a>(
    document: &'a Document,
    dict: &'a Dictionary,
    key: &[u8],
) -> Option<&'a Object> {
    dict.get(key)
        .ok()
        .and_then(|value| resolve(document, value))
}

pub(super) fn get_i64(document: &Document, dict: &Dictionary, key: &[u8]) -> Option<i64> {
    get(document, dict, key)?.as_i64().ok()
}

pub(super) fn get_bool(document: &Document, dict: &Dictionary, key: &[u8]) -> Option<bool> {
    get(document, dict, key)?.as_bool().ok()
}

pub(super) fn get_name<'a>(
    document: &'a Document,
    dict: &'a Dictionary,
    key: &[u8],
) -> Option<&'a [u8]> {
    get(document, dict, key)?.as_name().ok()
}
