//! End-to-end tests of the public API against generated PDF and TIFF fixtures.

use super::test_support::{
    TiffPage, build_encrypted_pdf, build_pdf, checker_rows, dict, g4_encode, gray_reference,
    image_stream, mh_encode, name, png_filter_row, tiff_bytes, write_file,
};
use super::{PreviewError, page_count, render_page};
use crate::model::PageImage;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::tiff::TiffEncoder;
use image::{ExtendedColorType, ImageEncoder};
use lopdf::{Object, Stream};
use std::io::Cursor;
use std::path::Path;

fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("temp dir")
}

/// Red channel of each pixel; for gray images this is the gray level.
fn red_channel(page: &PageImage) -> Vec<u8> {
    page.rgba.chunks_exact(4).map(|px| px[0]).collect()
}

fn render_gray(path: &Path, page: u32) -> PageImage {
    render_page(path, page, u32::MAX).expect("page renders")
}

fn is_unsupported(error: &PreviewError) -> bool {
    matches!(error, PreviewError::Unsupported(_))
}

fn gray_image(
    width: i64,
    height: i64,
    extra: Vec<(&'static str, Object)>,
    content: Vec<u8>,
) -> Stream {
    let mut all = vec![
        ("ColorSpace", name("DeviceGray")),
        ("BitsPerComponent", 8.into()),
    ];
    all.extend(extra);
    image_stream(width, height, all, content)
}

#[test]
fn two_page_pdf_reports_two_pages() {
    let dir = temp_dir();
    let path = write_file(dir.path(), "two.pdf", &build_pdf(vec![None, None], false));
    assert_eq!(page_count(&path).expect("counts"), 2);
}

#[test]
fn jpeg_page_renders_with_expected_size_and_colours() {
    let (width, height) = (64u32, 48u32);
    let mut pixels = Vec::new();
    for _ in 0..height {
        for x in 0..width {
            pixels.extend_from_slice(if x < width / 2 {
                &[255, 0, 0]
            } else {
                &[0, 0, 255]
            });
        }
    }
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, 95)
        .encode(&pixels, width, height, ExtendedColorType::Rgb8)
        .expect("jpeg encodes");
    let stream = image_stream(
        i64::from(width),
        i64::from(height),
        vec![
            ("ColorSpace", name("DeviceRGB")),
            ("BitsPerComponent", 8.into()),
            ("Filter", name("DCTDecode")),
        ],
        jpeg,
    );
    let dir = temp_dir();
    let path = write_file(
        dir.path(),
        "jpeg.pdf",
        &build_pdf(vec![Some(stream)], false),
    );
    let page = render_gray(&path, 0);
    assert_eq!((page.width, page.height), (width, height));
    let left = (24 * width as usize + 8) * 4;
    let right = (24 * width as usize + 56) * 4;
    let left_px = &page.rgba[left..left + 3];
    let right_px = &page.rgba[right..right + 3];
    assert!(
        left_px[0] > 200 && left_px[1] < 60 && left_px[2] < 60,
        "left {left_px:?}"
    );
    assert!(
        right_px[2] > 200 && right_px[0] < 60 && right_px[1] < 60,
        "right {right_px:?}"
    );
}

#[test]
fn one_bit_uncompressed_image_honours_decode_inversion() {
    // Row pattern (16 px): white x4, black x8, white x4. Four identical rows.
    let row = [0b1111_0000u8, 0b0000_1111u8];
    let content: Vec<u8> = row.iter().copied().cycle().take(8).collect();
    let plain = image_stream(
        16,
        4,
        vec![
            ("ColorSpace", name("DeviceGray")),
            ("BitsPerComponent", 1.into()),
        ],
        content.clone(),
    );
    let inverted = image_stream(
        16,
        4,
        vec![
            ("ColorSpace", name("DeviceGray")),
            ("BitsPerComponent", 1.into()),
            ("Decode", Object::Array(vec![1.into(), 0.into()])),
        ],
        content,
    );
    let dir = temp_dir();
    let bytes = build_pdf(vec![Some(plain), Some(inverted)], false);
    let path = write_file(dir.path(), "bits.pdf", &bytes);

    let expected: Vec<u8> = (0..16)
        .map(|x| if (4..12).contains(&x) { 0 } else { 255 })
        .collect();
    let first_row = |page: &PageImage| red_channel(page)[..16].to_vec();
    assert_eq!(first_row(&render_gray(&path, 0)), expected);
    let inverted_expected: Vec<u8> = expected.iter().map(|v| 255 - v).collect();
    assert_eq!(first_row(&render_gray(&path, 1)), inverted_expected);
}

#[test]
fn flate_compressed_gray_image_round_trips() {
    let (width, height) = (200usize, 100usize);
    let pixels: Vec<u8> = (0..height)
        .flat_map(|y| (0..width).map(move |x| ((x + y) % 256) as u8))
        .collect();
    let mut stream = gray_image(width as i64, height as i64, vec![], pixels.clone());
    stream.compress().expect("compresses");
    assert!(
        stream.dict.get(b"Filter").is_ok(),
        "fixture must actually be compressed"
    );
    let dir = temp_dir();
    let path = write_file(
        dir.path(),
        "flate.pdf",
        &build_pdf(vec![Some(stream)], false),
    );
    assert_eq!(red_channel(&render_gray(&path, 0)), pixels);
}

#[test]
fn flate_png_predictor_rows_are_unfiltered() {
    let (width, height, bpp) = (50usize, 40usize, 1usize);
    let original: Vec<Vec<u8>> = (0..height)
        .map(|y| (0..width).map(|x| (x * 3 + y * 5) as u8).collect())
        .collect();
    let mut encoded = Vec::new();
    let mut previous = vec![0u8; width];
    for (y, row) in original.iter().enumerate() {
        let tag = (y % 5) as u8;
        encoded.push(tag);
        encoded.extend(png_filter_row(tag, bpp, &previous, row));
        previous.clone_from(row);
    }
    let mut stream = gray_image(width as i64, height as i64, vec![], encoded);
    stream.dict.set(
        "DecodeParms",
        dict(vec![
            ("Predictor", 15.into()),
            ("Colors", 1.into()),
            ("BitsPerComponent", 8.into()),
            ("Columns", (width as i64).into()),
        ]),
    );
    stream.compress().expect("compresses");
    assert!(stream.dict.get(b"Filter").is_ok());
    let dir = temp_dir();
    let path = write_file(dir.path(), "png.pdf", &build_pdf(vec![Some(stream)], false));
    let expected: Vec<u8> = original.concat();
    assert_eq!(red_channel(&render_gray(&path, 0)), expected);
}

#[test]
fn ccitt_g4_page_matches_source_pixels() {
    let rows = checker_rows(120, 30);
    let stream = image_stream(
        120,
        30,
        vec![
            ("ColorSpace", name("DeviceGray")),
            ("BitsPerComponent", 1.into()),
            ("Filter", name("CCITTFaxDecode")),
            (
                "DecodeParms",
                dict(vec![("K", (-1).into()), ("Columns", 120.into())]),
            ),
        ],
        g4_encode(&rows),
    );
    let dir = temp_dir();
    let path = write_file(dir.path(), "g4.pdf", &build_pdf(vec![Some(stream)], false));
    assert_eq!(red_channel(&render_gray(&path, 0)), gray_reference(&rows));
}

#[test]
fn ccitt_black_is_one_flips_the_rendered_pixels() {
    let rows = checker_rows(64, 16);
    let stream = image_stream(
        64,
        16,
        vec![
            ("ColorSpace", name("DeviceGray")),
            ("BitsPerComponent", 1.into()),
            ("Filter", name("CCITTFaxDecode")),
            (
                "DecodeParms",
                dict(vec![
                    ("K", (-1).into()),
                    ("Columns", 64.into()),
                    ("BlackIs1", true.into()),
                ]),
            ),
        ],
        g4_encode(&rows),
    );
    let dir = temp_dir();
    let path = write_file(
        dir.path(),
        "g4inv.pdf",
        &build_pdf(vec![Some(stream)], false),
    );
    // Black pixels become 1 bits, which DeviceGray shows as white.
    let expected: Vec<u8> = gray_reference(&rows).iter().map(|v| 255 - v).collect();
    assert_eq!(red_channel(&render_gray(&path, 0)), expected);
}

#[test]
fn ccitt_g3_one_dimensional_pages_with_eol_and_byte_alignment() {
    let rows = checker_rows(150, 20);
    let with_eol = image_stream(
        150,
        20,
        vec![
            ("ColorSpace", name("DeviceGray")),
            ("BitsPerComponent", 1.into()),
            ("Filter", name("CCITTFaxDecode")),
            (
                "DecodeParms",
                dict(vec![
                    ("K", 0.into()),
                    ("Columns", 150.into()),
                    ("EndOfLine", true.into()),
                ]),
            ),
        ],
        mh_encode(&rows, true, false),
    );
    let aligned = image_stream(
        150,
        20,
        vec![
            ("ColorSpace", name("DeviceGray")),
            ("BitsPerComponent", 1.into()),
            ("Filter", name("CCITTFaxDecode")),
            (
                "DecodeParms",
                dict(vec![
                    ("K", 0.into()),
                    ("Columns", 150.into()),
                    ("EncodedByteAlign", true.into()),
                ]),
            ),
        ],
        mh_encode(&rows, false, true),
    );
    let dir = temp_dir();
    let bytes = build_pdf(vec![Some(with_eol), Some(aligned)], false);
    let path = write_file(dir.path(), "g3.pdf", &bytes);
    let expected = gray_reference(&rows);
    assert_eq!(red_channel(&render_gray(&path, 0)), expected);
    assert_eq!(red_channel(&render_gray(&path, 1)), expected);
}

#[test]
fn ccitt_two_dimensional_k_above_zero_is_unsupported() {
    let stream = image_stream(
        8,
        2,
        vec![
            ("ColorSpace", name("DeviceGray")),
            ("BitsPerComponent", 1.into()),
            ("Filter", name("CCITTFaxDecode")),
            (
                "DecodeParms",
                dict(vec![("K", 4.into()), ("Columns", 8.into())]),
            ),
        ],
        vec![0xFF; 8],
    );
    let dir = temp_dir();
    let path = write_file(dir.path(), "k4.pdf", &build_pdf(vec![Some(stream)], false));
    let error = render_page(&path, 0, 100).expect_err("K>0 rejected");
    assert!(is_unsupported(&error));
}

#[test]
fn inherited_resources_on_parent_pages_node_are_found() {
    let stream = gray_image(4, 4, vec![], (0..16).map(|v| (v * 16) as u8).collect());
    let dir = temp_dir();
    let path = write_file(
        dir.path(),
        "inherit.pdf",
        &build_pdf(vec![Some(stream)], true),
    );
    let expected: Vec<u8> = (0..16).map(|v| (v * 16) as u8).collect();
    assert_eq!(red_channel(&render_gray(&path, 0)), expected);
}

#[test]
fn page_out_of_range_and_imageless_pages_are_reported() {
    let dir = temp_dir();
    let path = write_file(dir.path(), "none.pdf", &build_pdf(vec![None, None], false));
    assert!(matches!(
        render_page(&path, 5, 100),
        Err(PreviewError::PageOutOfRange(5))
    ));
    assert!(is_unsupported(
        &render_page(&path, 0, 100).expect_err("no image")
    ));
}

#[test]
fn garbage_bytes_are_unsupported() {
    let dir = temp_dir();
    let path = write_file(dir.path(), "junk.pdf", b"definitely not a document");
    assert!(is_unsupported(&page_count(&path).expect_err("garbage")));
    assert!(is_unsupported(
        &render_page(&path, 0, 100).expect_err("garbage")
    ));
}

#[test]
fn encrypted_pdf_is_unsupported() {
    let dir = temp_dir();
    let path = write_file(dir.path(), "secret.pdf", &build_encrypted_pdf());
    assert!(is_unsupported(
        &render_page(&path, 0, 100).expect_err("encrypted")
    ));
}

#[test]
fn large_image_is_downscaled_without_upscaling() {
    let stream = gray_image(2000, 1000, vec![], vec![128; 2_000_000]);
    let dir = temp_dir();
    let path = write_file(dir.path(), "big.pdf", &build_pdf(vec![Some(stream)], false));
    let small = render_page(&path, 0, 500).expect("renders");
    assert_eq!((small.width, small.height), (500, 250));
    assert_eq!(small.rgba.len(), 500 * 250 * 4);
    let unchanged = render_page(&path, 0, 4000).expect("renders");
    assert_eq!((unchanged.width, unchanged.height), (2000, 1000));
}

#[test]
fn oversized_or_truncated_images_are_rejected() {
    let huge = gray_image(30_000, 30_000, vec![], vec![]);
    let short = gray_image(10, 10, vec![], vec![0; 50]);
    let indexed = image_stream(
        4,
        4,
        vec![
            ("ColorSpace", name("Indexed")),
            ("BitsPerComponent", 8.into()),
        ],
        vec![0; 16],
    );
    let dir = temp_dir();
    let bytes = build_pdf(vec![Some(huge), Some(short), Some(indexed)], false);
    let path = write_file(dir.path(), "bad.pdf", &bytes);
    for page in 0..3 {
        assert!(
            is_unsupported(&render_page(&path, page, 100).expect_err("rejected")),
            "page {page}"
        );
    }
}

#[test]
fn gray_tiff_renders_through_the_image_crate() {
    let (width, height) = (32u32, 8u32);
    let pixels: Vec<u8> = (0..width * height).map(|i| (i * 5 % 256) as u8).collect();
    let mut buffer = Cursor::new(Vec::new());
    TiffEncoder::new(&mut buffer)
        .write_image(&pixels, width, height, ExtendedColorType::L8)
        .expect("tiff encodes");
    let dir = temp_dir();
    // The extension is wrong on purpose: detection must use the magic bytes.
    let path = write_file(dir.path(), "scan.pdf", &buffer.into_inner());
    assert_eq!(page_count(&path).expect("counts"), 1);
    let page = render_gray(&path, 0);
    assert_eq!((page.width, page.height), (width, height));
    assert_eq!(red_channel(&page), pixels);
}

#[test]
fn ccitt_g4_tiff_is_pixel_exact_in_both_byte_orders() {
    let rows = checker_rows(90, 20);
    let page = TiffPage {
        width: 90,
        height: 20,
        compression: 4,
        strip: g4_encode(&rows),
    };
    let dir = temp_dir();
    for little in [true, false] {
        let bytes = tiff_bytes(std::slice::from_ref(&page), little);
        let path = write_file(dir.path(), &format!("g4-{little}.tif"), &bytes);
        assert_eq!(page_count(&path).expect("counts"), 1);
        assert_eq!(
            red_channel(&render_gray(&path, 0)),
            gray_reference(&rows),
            "little={little}"
        );
    }
}

#[test]
fn multi_page_ccitt_tiff_renders_each_ifd() {
    let rows_a = checker_rows(40, 10);
    let rows_b = checker_rows(64, 12);
    let pages = [
        TiffPage {
            width: 40,
            height: 10,
            compression: 4,
            strip: g4_encode(&rows_a),
        },
        TiffPage {
            width: 64,
            height: 12,
            compression: 3,
            strip: mh_encode(&rows_b, false, false),
        },
    ];
    let dir = temp_dir();
    let path = write_file(dir.path(), "multi.tif", &tiff_bytes(&pages, true));
    assert_eq!(page_count(&path).expect("counts"), 2);
    let first = render_gray(&path, 0);
    assert_eq!((first.width, first.height), (40, 10));
    assert_eq!(red_channel(&first), gray_reference(&rows_a));
    let second = render_gray(&path, 1);
    assert_eq!((second.width, second.height), (64, 12));
    assert_eq!(red_channel(&second), gray_reference(&rows_b));
    assert!(matches!(
        render_page(&path, 2, 100),
        Err(PreviewError::PageOutOfRange(2))
    ));
}

#[test]
fn truncated_tiff_is_an_error_not_a_panic() {
    let rows = checker_rows(50, 10);
    let bytes = tiff_bytes(
        &[TiffPage {
            width: 50,
            height: 10,
            compression: 4,
            strip: g4_encode(&rows),
        }],
        true,
    );
    let dir = temp_dir();
    let inside_ifd = write_file(dir.path(), "cut-ifd.tif", &bytes[..40]);
    assert!(page_count(&inside_ifd).is_err());
    assert!(render_page(&inside_ifd, 0, 100).is_err());
    let inside_strip = write_file(dir.path(), "cut-strip.tif", &bytes[..bytes.len() - 5]);
    assert!(page_count(&inside_strip).is_ok());
    assert!(is_unsupported(
        &render_page(&inside_strip, 0, 100).expect_err("strip cut")
    ));
}

#[test]
fn tiff_directory_loop_is_rejected() {
    let mut bytes = b"II*\0".to_vec();
    bytes.extend_from_slice(&8u32.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&8u32.to_le_bytes());
    let dir = temp_dir();
    let path = write_file(dir.path(), "loop.tif", &bytes);
    assert!(is_unsupported(&page_count(&path).expect_err("loop")));
}

#[test]
fn image_above_forty_megapixels_is_rejected_before_decoding() {
    // 7000 x 6000 is 42 MP: inside the old 100 MP limit, outside the 40 MP limit.
    let stream = gray_image(7000, 6000, vec![], vec![0; 16]);
    let dir = temp_dir();
    let path = write_file(
        dir.path(),
        "wide.pdf",
        &build_pdf(vec![Some(stream)], false),
    );
    let error = render_page(&path, 0, 100).expect_err("rejected");
    assert!(error.to_string().contains("pixel limit"), "{error}");
}

#[test]
fn flate_image_with_oversized_declared_samples_is_rejected_before_inflating() {
    // 6000 x 6000 at 16 bits per RGB sample is about 288 MB decoded, above the 160 MB ceiling.
    let mut stream = image_stream(
        6000,
        6000,
        vec![
            ("ColorSpace", name("DeviceRGB")),
            ("BitsPerComponent", 16.into()),
        ],
        vec![0; 64],
    );
    stream.compress().expect("compresses");
    let dir = temp_dir();
    let path = write_file(
        dir.path(),
        "deep.pdf",
        &build_pdf(vec![Some(stream)], false),
    );
    let error = render_page(&path, 0, 100).expect_err("rejected");
    assert!(error.to_string().contains("decoded size"), "{error}");
}

#[test]
fn flate_data_longer_than_the_declared_image_is_rejected() {
    // A 10 x 10 gray image needs 100 bytes; the stream inflates to 2 MB of padding.
    let mut stream = gray_image(10, 10, vec![], vec![0; 2_000_000]);
    stream.compress().expect("compresses");
    let dir = temp_dir();
    let path = write_file(
        dir.path(),
        "padded.pdf",
        &build_pdf(vec![Some(stream)], false),
    );
    let error = render_page(&path, 0, 100).expect_err("rejected");
    assert!(error.to_string().contains("inflated size"), "{error}");
}
