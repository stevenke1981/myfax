//! Page preview: JPEG and 1-bit fax pages render, broken or missing files report
//! "unavailable", page stepping clamps, and zoom stays on quarter steps.

use super::PREVIEW_IMAGE_KEY;
use super::tests::{Harness, fax_name, zh};
use crate::app::msg::Msg;
use crate::app::state::PreviewStatus;
use crate::model::FaxId;
use image::ExtendedColorType;
use image::codecs::jpeg::JpegEncoder;
use lopdf::{Dictionary, Document, Object, Stream, dictionary};

/// Vertical stripes as a grey JPEG.
fn grey_jpeg(width: u32, height: u32) -> Vec<u8> {
    let pixels: Vec<u8> = (0..width * height)
        .map(|index| {
            if (index / width).is_multiple_of(2) {
                0
            } else {
                255
            }
        })
        .collect();
    let mut out = Vec::new();
    JpegEncoder::new(&mut out)
        .encode(&pixels, width, height, ExtendedColorType::L8)
        .expect("encode test jpeg");
    out
}

/// One-bit grey image rows, unfiltered.
fn one_bit_rows(width: u32, height: u32) -> Vec<u8> {
    let row_bytes = width.div_ceil(8) as usize;
    (0..height)
        .flat_map(|row| {
            let fill = if row % 2 == 0 { 0xF0 } else { 0x0F };
            std::iter::repeat_n(fill, row_bytes)
        })
        .collect()
}

/// Fax-like PDF: every page draws the same grey image over the whole page.
fn image_pdf(
    pages: u32,
    width: u32,
    height: u32,
    bits: i64,
    filter: Option<&str>,
    data: Vec<u8>,
) -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut image_dict = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Image",
        "Width" => i64::from(width),
        "Height" => i64::from(height),
        "ColorSpace" => "DeviceGray",
        "BitsPerComponent" => bits,
    };
    if let Some(filter) = filter {
        image_dict.set("Filter", filter);
    }
    let image_id = doc.add_object(Stream::new(image_dict, data));
    let mut kids: Vec<Object> = Vec::new();
    for _ in 0..pages {
        let content_id = doc.add_object(Stream::new(
            Dictionary::new(),
            b"q 612 0 0 792 0 0 cm /Im0 Do Q".to_vec(),
        ));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => content_id,
            "Resources" => dictionary! { "XObject" => dictionary! { "Im0" => image_id } },
        });
        kids.push(page_id.into());
    }
    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => kids,
        "Count" => i64::from(pages),
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    let mut out = Vec::new();
    doc.save_to(&mut out).expect("serialise test pdf");
    out
}

/// Import `bytes` as the only fax (which selects and previews it).
fn import_bytes(h: &mut Harness, bytes: &[u8]) -> FaxId {
    let name = fax_name(2026, 3, 5, 9, 0, "0912345678");
    let path = h.write(&format!("incoming/{name}"), bytes);
    h.controller.files_dropped(vec![path]);
    h.id_of(&name)
}

fn preview_status(h: &Harness) -> PreviewStatus {
    h.controller.state.preview.status.clone()
}

#[test]
fn jpeg_fax_page_renders_into_the_preview_slot() {
    let mut h = Harness::new();
    let bytes = image_pdf(2, 64, 64, 8, Some("DCTDecode"), grey_jpeg(64, 64));
    import_bytes(&mut h, &bytes);

    assert_eq!(preview_status(&h), PreviewStatus::Ready);
    let preview = &h.controller.state.preview;
    assert_eq!((preview.page, preview.page_count), (0, 2));
    assert_eq!((preview.image_width, preview.image_height), (64, 64));
    let image = h
        .controller
        .images()
        .get(PREVIEW_IMAGE_KEY)
        .expect("preview image");
    assert_eq!((image.width(), image.height()), (64, 64));
}

#[test]
fn one_bit_fax_page_renders_too() {
    let mut h = Harness::new();
    let bytes = image_pdf(1, 16, 16, 1, None, one_bit_rows(16, 16));
    import_bytes(&mut h, &bytes);
    assert_eq!(preview_status(&h), PreviewStatus::Ready);
    assert_eq!(h.controller.state.preview.page_count, 1);
    assert!(h.controller.images().contains_key(PREVIEW_IMAGE_KEY));
}

#[test]
fn broken_pdf_reports_the_preview_unavailable_without_an_image() {
    let mut h = Harness::new();
    import_bytes(&mut h, b"%PDF-1.4 this is not a real document");
    assert_eq!(
        preview_status(&h),
        PreviewStatus::Unavailable(zh("app.preview_unavailable").to_owned())
    );
    assert!(!h.controller.images().contains_key(PREVIEW_IMAGE_KEY));
}

#[test]
fn missing_archive_file_is_reported_on_reselect() {
    let mut h = Harness::new();
    let bytes = image_pdf(1, 16, 16, 1, None, one_bit_rows(16, 16));
    let id = import_bytes(&mut h, &bytes);
    std::fs::remove_file(h.fax(id).file_path).expect("remove archive file");
    h.controller.update(Msg::CloseDetail);
    assert_eq!(preview_status(&h), PreviewStatus::Empty);
    assert!(!h.controller.images().contains_key(PREVIEW_IMAGE_KEY));

    h.controller.update(Msg::SelectFax(id));
    assert_eq!(
        preview_status(&h),
        PreviewStatus::Unavailable(zh("app.preview_file_missing").to_owned())
    );
}

#[test]
fn page_stepping_clamps_to_the_document() {
    let mut h = Harness::new();
    let bytes = image_pdf(3, 16, 16, 1, None, one_bit_rows(16, 16));
    import_bytes(&mut h, &bytes);

    h.controller.update(Msg::PreviewPrevPage);
    assert_eq!(h.controller.state.preview.page, 0);
    h.controller.update(Msg::PreviewNextPage);
    assert_eq!(h.controller.state.preview.page, 1);
    for _ in 0..5 {
        h.controller.update(Msg::PreviewNextPage);
    }
    assert_eq!(h.controller.state.preview.page, 2);
    assert_eq!(preview_status(&h), PreviewStatus::Ready);
}

#[test]
fn zoom_moves_in_quarter_steps_within_half_to_triple() {
    let mut h = Harness::new();
    assert_eq!(h.controller.state.preview.zoom, 1.0);
    h.controller.update(Msg::ZoomIn);
    assert_eq!(h.controller.state.preview.zoom, 1.25);
    for _ in 0..20 {
        h.controller.update(Msg::ZoomIn);
    }
    assert_eq!(h.controller.state.preview.zoom, 3.0);
    for _ in 0..30 {
        h.controller.update(Msg::ZoomOut);
    }
    assert_eq!(h.controller.state.preview.zoom, 0.5);
    h.controller.update(Msg::ZoomReset);
    assert_eq!(h.controller.state.preview.zoom, 1.0);
}

#[test]
fn zoom_survives_selecting_another_fax() {
    let mut h = Harness::new();
    let bytes = image_pdf(1, 16, 16, 1, None, one_bit_rows(16, 16));
    let id = import_bytes(&mut h, &bytes);
    h.controller.update(Msg::ZoomIn);
    h.controller.update(Msg::CloseDetail);
    h.controller.update(Msg::SelectFax(id));
    assert_eq!(h.controller.state.preview.zoom, 1.25);
}
