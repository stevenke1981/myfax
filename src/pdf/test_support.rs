//! Fixture builders for the PDF, TIFF and CCITT tests. Nothing here is written
//! to the repository: test files are generated into temporary directories.

use fax::encoder::Encoder;
use fax::maps::{black, white};
use fax::{BitWriter, Bits, Color, VecWriter};
use lopdf::{Dictionary, Document, Object, Stream};
use std::path::{Path, PathBuf};

/// Test pattern: true = black. Checkerboard blocks plus a diagonal stroke.
pub fn checker_rows(width: usize, height: usize) -> Vec<Vec<bool>> {
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| ((x / 5) + (y / 3)) % 2 == 0 || x == y)
                .collect()
        })
        .collect()
}

/// T.6 (Group 4) encoding of the rows, with EOFB.
pub fn g4_encode(rows: &[Vec<bool>]) -> Vec<u8> {
    let width = u16::try_from(rows[0].len()).expect("width fits u16");
    let mut encoder = Encoder::new(VecWriter::new());
    for row in rows {
        let pels = row
            .iter()
            .map(|&p| if p { Color::Black } else { Color::White });
        encoder.encode_line(pels, width).expect("infallible writer");
    }
    encoder.finish().expect("infallible writer").finish()
}

/// T.4 one-dimensional encoding. `eol` writes an end-of-line before every row;
/// `align` pads every row to a byte boundary (PDF `EncodedByteAlign`).
pub fn mh_encode(rows: &[Vec<bool>], eol: bool, align: bool) -> Vec<u8> {
    let mut writer = VecWriter::new();
    for row in rows {
        if eol {
            put(&mut writer, Bits { data: 1, len: 12 });
        }
        for (index, length) in runs(row).into_iter().enumerate() {
            write_run(&mut writer, length, index % 2 == 1);
        }
        if align {
            writer.pad();
        }
    }
    writer.finish()
}

/// Packed 1-bit reference: `black_is_1` selects whether black pixels are 1 bits.
pub fn packed_reference(rows: &[Vec<bool>], black_is_1: bool) -> Vec<u8> {
    let (white_bit, black_bit) = if black_is_1 { (0u8, 1u8) } else { (1, 0) };
    // Padding bits after the last pixel are white, as the decoder leaves them.
    let white_byte = if white_bit == 1 { 0xFF } else { 0x00 };
    let mut out = Vec::new();
    for row in rows {
        let mut bytes = vec![white_byte; row.len().div_ceil(8)];
        for (x, &is_black) in row.iter().enumerate() {
            let mask = 0x80 >> (x % 8);
            let bit = if is_black { black_bit } else { white_bit };
            if bit == 1 {
                bytes[x / 8] |= mask;
            } else {
                bytes[x / 8] &= !mask;
            }
        }
        out.extend_from_slice(&bytes);
    }
    out
}

/// Expected 8-bit gray values: black → 0, white → 255.
pub fn gray_reference(rows: &[Vec<bool>]) -> Vec<u8> {
    rows.iter()
        .flatten()
        .map(|&is_black| if is_black { 0 } else { 255 })
        .collect()
}

/// Run lengths of a row, starting with a white run (possibly zero).
fn runs(row: &[bool]) -> Vec<usize> {
    let mut runs = Vec::new();
    let mut colour = false;
    let mut length = 0;
    for &pixel in row {
        if pixel != colour {
            runs.push(length);
            length = 0;
            colour = pixel;
        }
        length += 1;
    }
    runs.push(length);
    runs
}

fn write_run(writer: &mut VecWriter, mut length: usize, is_black: bool) {
    while length >= 2560 {
        put(writer, code(2560, is_black));
        length -= 2560;
    }
    if length >= 64 {
        let makeup = length / 64 * 64;
        put(writer, code(makeup, is_black));
        length -= makeup;
    }
    put(writer, code(length, is_black));
}

fn code(value: usize, is_black: bool) -> Bits {
    let value = u16::try_from(value).expect("run fits u16");
    let bits = if is_black {
        black::encode(value)
    } else {
        white::encode(value)
    };
    bits.expect("run length has a code")
}

fn put(writer: &mut VecWriter, bits: Bits) {
    writer.write(bits).expect("infallible writer");
}

/// PDF name object.
pub fn name(value: &str) -> Object {
    Object::Name(value.as_bytes().to_vec())
}

/// Image XObject stream with `extra` entries (ColorSpace, BitsPerComponent, Filter, ...).
pub fn image_stream(
    width: i64,
    height: i64,
    extra: Vec<(&'static str, Object)>,
    content: Vec<u8>,
) -> Stream {
    let mut dict = Dictionary::new();
    dict.set("Type", name("XObject"));
    dict.set("Subtype", name("Image"));
    dict.set("Width", width);
    dict.set("Height", height);
    for (key, value) in extra {
        dict.set(key, value);
    }
    Stream::new(dict, content)
}

/// Dictionary from literal key/value pairs.
pub fn dict(entries: Vec<(&'static str, Object)>) -> Object {
    let mut dictionary = Dictionary::new();
    for (key, value) in entries {
        dictionary.set(key, value);
    }
    Object::Dictionary(dictionary)
}

/// Builds a PDF with one page per entry. `Some(image)` puts the image on the page as
/// `/Im<index>`; `inherit` places the resources on the root Pages node instead of each page.
pub fn build_pdf(pages: Vec<Option<Stream>>, inherit: bool) -> Vec<u8> {
    assemble(pages, inherit, false)
}

/// A one-page PDF whose trailer points at an Encrypt dictionary.
pub fn build_encrypted_pdf() -> Vec<u8> {
    assemble(vec![None], false, true)
}

fn assemble(pages: Vec<Option<Stream>>, inherit: bool, encrypt: bool) -> Vec<u8> {
    let mut document = Document::with_version("1.5");
    let pages_id = document.new_object_id();
    let mut kids = Vec::new();
    let mut shared_xobjects = Dictionary::new();
    for (index, image) in pages.into_iter().enumerate() {
        let mut page = Dictionary::new();
        page.set("Type", name("Page"));
        page.set("Parent", Object::Reference(pages_id));
        page.set(
            "MediaBox",
            Object::Array(vec![0.into(), 0.into(), 612.into(), 792.into()]),
        );
        if let Some(stream) = image {
            let key = format!("Im{index}");
            let image_id = document.add_object(stream);
            if inherit {
                shared_xobjects.set(key, Object::Reference(image_id));
            } else {
                let mut xobjects = Dictionary::new();
                xobjects.set(key, Object::Reference(image_id));
                let mut resources = Dictionary::new();
                resources.set("XObject", Object::Dictionary(xobjects));
                page.set("Resources", Object::Dictionary(resources));
            }
        }
        kids.push(Object::Reference(document.add_object(page)));
    }
    let mut pages_dict = Dictionary::new();
    pages_dict.set("Type", name("Pages"));
    pages_dict.set("Count", kids.len() as i64);
    pages_dict.set("Kids", Object::Array(kids));
    if inherit {
        let mut resources = Dictionary::new();
        resources.set("XObject", Object::Dictionary(shared_xobjects));
        pages_dict.set("Resources", Object::Dictionary(resources));
    }
    document
        .objects
        .insert(pages_id, Object::Dictionary(pages_dict));
    let mut catalog = Dictionary::new();
    catalog.set("Type", name("Catalog"));
    catalog.set("Pages", Object::Reference(pages_id));
    let catalog_id = document.add_object(catalog);
    document.trailer.set("Root", Object::Reference(catalog_id));
    if encrypt {
        let encrypt_id = document.add_object(dict(vec![("Filter", name("Standard"))]));
        document
            .trailer
            .set("Encrypt", Object::Reference(encrypt_id));
    }
    let mut out = Vec::new();
    document.save_to(&mut out).expect("PDF serialises");
    out
}

/// One TIFF page: a single strip holding `strip` bytes of CCITT data.
pub struct TiffPage {
    pub width: u32,
    pub height: u32,
    /// TIFF Compression value (3 = T.4, 4 = T.6).
    pub compression: u16,
    pub strip: Vec<u8>,
}

/// Classic TIFF with one IFD per page, written in the requested byte order.
pub fn tiff_bytes(pages: &[TiffPage], little: bool) -> Vec<u8> {
    const ENTRY_COUNT: u16 = 8;
    let ifd_len = 2 + usize::from(ENTRY_COUNT) * 12 + 4;
    let mut out = Vec::new();
    out.extend_from_slice(if little { b"II" } else { b"MM" });
    out.extend_from_slice(&u16_bytes(42, little));
    out.extend_from_slice(&u32_bytes(8, little));
    let mut ifd_start = 8usize;
    for (index, page) in pages.iter().enumerate() {
        let strip_start = ifd_start + ifd_len;
        let next = if index + 1 < pages.len() {
            strip_start + page.strip.len()
        } else {
            0
        };
        out.extend_from_slice(&u16_bytes(ENTRY_COUNT, little));
        entry(&mut out, 256, 4, page.width, little);
        entry(&mut out, 257, 4, page.height, little);
        entry(&mut out, 258, 3, 1, little);
        entry(&mut out, 259, 3, u32::from(page.compression), little);
        entry(&mut out, 262, 3, 0, little);
        entry(&mut out, 273, 4, strip_start as u32, little);
        entry(&mut out, 278, 4, page.height, little);
        entry(&mut out, 279, 4, page.strip.len() as u32, little);
        out.extend_from_slice(&u32_bytes(next as u32, little));
        out.extend_from_slice(&page.strip);
        ifd_start = next;
    }
    out
}

fn entry(out: &mut Vec<u8>, tag: u16, field_type: u16, value: u32, little: bool) {
    out.extend_from_slice(&u16_bytes(tag, little));
    out.extend_from_slice(&u16_bytes(field_type, little));
    out.extend_from_slice(&u32_bytes(1, little));
    if field_type == 3 {
        out.extend_from_slice(&u16_bytes(value as u16, little));
        out.extend_from_slice(&[0, 0]);
    } else {
        out.extend_from_slice(&u32_bytes(value, little));
    }
}

fn u16_bytes(value: u16, little: bool) -> [u8; 2] {
    if little {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    }
}

fn u32_bytes(value: u32, little: bool) -> [u8; 4] {
    if little {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    }
}

/// Spec-conformant PNG row filter (encoder side). `previous` and `current` are the
/// original (unfiltered) rows; `tag` is the filter type 0..=4.
pub fn png_filter_row(tag: u8, bpp: usize, previous: &[u8], current: &[u8]) -> Vec<u8> {
    use super::pdf_image::paeth;
    (0..current.len())
        .map(|i| {
            let left = if i >= bpp { current[i - bpp] } else { 0 };
            let up = previous[i];
            let up_left = if i >= bpp { previous[i - bpp] } else { 0 };
            let predicted = match tag {
                0 => 0,
                1 => left,
                2 => up,
                3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
                _ => paeth(left, up, up_left),
            };
            current[i].wrapping_sub(predicted)
        })
        .collect()
}

/// Writes `bytes` to `dir/file_name` and returns the path.
pub fn write_file(dir: &Path, file_name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(file_name);
    std::fs::write(&path, bytes).expect("temp file writes");
    path
}
