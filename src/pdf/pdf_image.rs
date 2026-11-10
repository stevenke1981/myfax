//! Decoding of a PDF image XObject into RGBA: filter chain, PNG predictors,
//! JPEG and CCITT hand-off, and raw 1-bit / 8-bit samples.

use super::ccitt::{self, CcittParams, Coding};
use super::pdf_doc::{get, get_bool, get_i64, resolve};
use super::raster::{self, RawLayout};
use super::{PreviewError, unsupported};
use crate::model::PageImage;
use image::ImageFormat;
use lopdf::{Dictionary, Document, Object, Stream};
use std::borrow::Cow;
use std::io::Cursor;

/// PDF default for `Columns` in CCITTFaxDecode parameters.
const DEFAULT_CCITT_COLUMNS: i64 = 1728;
/// Largest accepted `Colors` and `Columns` values in predictor parameters.
const MAX_PREDICTOR_COLORS: usize = 4;
const MAX_PREDICTOR_COLUMNS: usize = 65_535;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FilterKind {
    Flate,
    Dct,
    Ccitt,
}

struct FilterStage<'a> {
    kind: FilterKind,
    parms: Option<&'a Dictionary>,
}

/// Decodes one image XObject to a full-resolution RGBA page bitmap.
pub(super) fn decode_image(
    document: &Document,
    stream: &Stream,
) -> Result<PageImage, PreviewError> {
    let dict = &stream.dict;
    let width = dimension(document, dict, "Width")?;
    let height = dimension(document, dict, "Height")?;
    raster::check_dims(width, height)?;
    let is_mask = get_bool(document, dict, b"ImageMask").unwrap_or(false);
    let stages = filter_stages(document, dict)?;

    let mut data: Cow<[u8]> = Cow::Borrowed(&stream.content);
    let mut forced_bits = None;
    for (index, stage) in stages.iter().enumerate() {
        let is_last = index + 1 == stages.len();
        match stage.kind {
            FilterKind::Flate => {
                data = Cow::Owned(undo_predictor(inflate(&data)?, document, stage.parms)?);
            }
            FilterKind::Dct => {
                require_last(is_last)?;
                return decode_jpeg(document, dict, &data, is_mask);
            }
            FilterKind::Ccitt => {
                require_last(is_last)?;
                data = Cow::Owned(decode_ccitt(document, stage.parms, &data, width, height)?);
                forced_bits = Some(1);
            }
        }
    }
    let layout = raw_layout(document, dict, width, height, is_mask, forced_bits)?;
    raster::raw_to_rgba(&layout, &data)
}

fn dimension(document: &Document, dict: &Dictionary, key: &str) -> Result<u32, PreviewError> {
    let value = get_i64(document, dict, key.as_bytes())
        .ok_or_else(|| unsupported(format!("image has no {key}")))?;
    u32::try_from(value).map_err(|_| unsupported(format!("invalid image {key} {value}")))
}

fn require_last(is_last: bool) -> Result<(), PreviewError> {
    if is_last {
        Ok(())
    } else {
        Err(unsupported("image filter must be the last in its chain"))
    }
}

/// Pairs each filter name with its DecodeParms entry (dictionary, array, or none).
fn filter_stages<'a>(
    document: &'a Document,
    dict: &'a Dictionary,
) -> Result<Vec<FilterStage<'a>>, PreviewError> {
    let names: Vec<&[u8]> = match get(document, dict, b"Filter") {
        None | Some(Object::Null) => Vec::new(),
        Some(Object::Name(name)) => vec![name.as_slice()],
        Some(Object::Array(items)) => items
            .iter()
            .map(|item| resolve(document, item).and_then(|o| o.as_name().ok()))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| unsupported("invalid Filter array"))?,
        Some(_) => return Err(unsupported("invalid Filter entry")),
    };
    let parms = decode_parms(document, dict, names.len())?;
    names
        .into_iter()
        .zip(parms)
        .map(|(name, parms)| {
            Ok(FilterStage {
                kind: filter_kind(name)?,
                parms,
            })
        })
        .collect()
}

fn decode_parms<'a>(
    document: &'a Document,
    dict: &'a Dictionary,
    filter_count: usize,
) -> Result<Vec<Option<&'a Dictionary>>, PreviewError> {
    let mut parms: Vec<Option<&Dictionary>> = match get(document, dict, b"DecodeParms") {
        None | Some(Object::Null) => Vec::new(),
        Some(Object::Dictionary(single)) => vec![Some(single)],
        Some(Object::Array(items)) => items
            .iter()
            .map(|item| resolve(document, item).and_then(|o| o.as_dict().ok()))
            .collect(),
        Some(_) => return Err(unsupported("invalid DecodeParms entry")),
    };
    parms.resize(filter_count, None);
    Ok(parms)
}

fn filter_kind(name: &[u8]) -> Result<FilterKind, PreviewError> {
    match name {
        b"FlateDecode" | b"Fl" => Ok(FilterKind::Flate),
        b"DCTDecode" | b"DCT" => Ok(FilterKind::Dct),
        b"CCITTFaxDecode" | b"CCF" => Ok(FilterKind::Ccitt),
        other => Err(unsupported(format!(
            "image filter {} is not supported",
            String::from_utf8_lossy(other)
        ))),
    }
}

/// Zlib-inflates `data` using lopdf's decompressor on a synthetic stream
/// without DecodeParms, so predictors are applied by [`undo_predictor`] only.
fn inflate(data: &[u8]) -> Result<Vec<u8>, PreviewError> {
    let mut dict = Dictionary::new();
    dict.set("Filter", Object::Name(b"FlateDecode".to_vec()));
    let stream = Stream::new(dict, data.to_vec());
    stream
        .decompressed_content()
        .map_err(|err| unsupported(format!("Flate data: {err}")))
}

fn undo_predictor(
    data: Vec<u8>,
    document: &Document,
    parms: Option<&Dictionary>,
) -> Result<Vec<u8>, PreviewError> {
    let int_parm = |key: &[u8], default: i64| {
        parms
            .and_then(|p| get_i64(document, p, key))
            .unwrap_or(default)
    };
    match int_parm(b"Predictor", 1) {
        1 => Ok(data),
        predictor @ 10..=15 => {
            let colors = checked_parm(int_parm(b"Colors", 1), 1..=MAX_PREDICTOR_COLORS, "Colors")?;
            let bits = checked_parm(int_parm(b"BitsPerComponent", 8), 1..=16, "BitsPerComponent")?;
            let columns = checked_parm(
                int_parm(b"Columns", 1),
                1..=MAX_PREDICTOR_COLUMNS,
                "Columns",
            )?;
            if !matches!(bits, 1 | 2 | 4 | 8 | 16) {
                return Err(unsupported(format!(
                    "predictor bit depth {bits} is invalid"
                )));
            }
            png_unfilter(&data, colors, bits, columns)
                .map_err(|err| unsupported(format!("predictor {predictor}: {err}")))
        }
        other => Err(unsupported(format!("predictor {other} is not supported"))),
    }
}

fn checked_parm(
    value: i64,
    range: std::ops::RangeInclusive<usize>,
    name: &str,
) -> Result<usize, PreviewError> {
    usize::try_from(value)
        .ok()
        .filter(|v| range.contains(v))
        .ok_or_else(|| unsupported(format!("predictor {name} {value} is out of range")))
}

/// Reverses PNG row filters (None, Sub, Up, Average, Paeth). Each row is
/// prefixed by its filter-type byte; a trailing partial row is ignored and
/// the caller's length check reports any resulting shortfall.
fn png_unfilter(
    data: &[u8],
    colors: usize,
    bits: usize,
    columns: usize,
) -> Result<Vec<u8>, PreviewError> {
    let row_len = colors
        .checked_mul(bits)
        .and_then(|v| v.checked_mul(columns))
        .map(|v| v.div_ceil(8))
        .ok_or_else(|| unsupported("predictor row size overflow"))?;
    let bytes_per_pixel = (colors * bits).div_ceil(8).max(1);
    let mut out = Vec::with_capacity(data.len());
    let mut previous = vec![0u8; row_len];
    for chunk in data.chunks_exact(row_len + 1) {
        let Some((&tag, encoded)) = chunk.split_first() else {
            continue;
        };
        let mut current = encoded.to_vec();
        unfilter_row(tag, bytes_per_pixel, &previous, &mut current)?;
        out.extend_from_slice(&current);
        previous = current;
    }
    Ok(out)
}

/// Undoes one PNG filter in place. `previous` is the already-reconstructed row above.
fn unfilter_row(tag: u8, bpp: usize, previous: &[u8], row: &mut [u8]) -> Result<(), PreviewError> {
    match tag {
        0 => {}
        1 => {
            for i in bpp..row.len() {
                row[i] = row[i].wrapping_add(row[i - bpp]);
            }
        }
        2 => {
            for (cur, up) in row.iter_mut().zip(previous) {
                *cur = cur.wrapping_add(*up);
            }
        }
        3 => {
            for i in 0..row.len() {
                let left = if i >= bpp { u16::from(row[i - bpp]) } else { 0 };
                let average = ((left + u16::from(previous[i])) / 2) as u8;
                row[i] = row[i].wrapping_add(average);
            }
        }
        4 => {
            for i in 0..row.len() {
                let (left, up_left) = if i >= bpp {
                    (row[i - bpp], previous[i - bpp])
                } else {
                    (0, 0)
                };
                row[i] = row[i].wrapping_add(paeth(left, previous[i], up_left));
            }
        }
        other => return Err(unsupported(format!("invalid PNG filter type {other}"))),
    }
    Ok(())
}

/// PNG Paeth predictor (RFC 2083 section 6.6).
pub(super) fn paeth(left: u8, up: u8, up_left: u8) -> u8 {
    let (a, b, c) = (i16::from(left), i16::from(up), i16::from(up_left));
    let estimate = a + b - c;
    let (da, db, dc) = (
        (estimate - a).abs(),
        (estimate - b).abs(),
        (estimate - c).abs(),
    );
    if da <= db && da <= dc {
        left
    } else if db <= dc {
        up
    } else {
        up_left
    }
}

fn decode_jpeg(
    document: &Document,
    dict: &Dictionary,
    data: &[u8],
    is_mask: bool,
) -> Result<PageImage, PreviewError> {
    if is_mask {
        return Err(unsupported("JPEG image masks are not supported"));
    }
    let invert = decode_inverted(document, dict)?;
    let (width, height) = image::ImageReader::with_format(Cursor::new(data), ImageFormat::Jpeg)
        .into_dimensions()
        .map_err(unsupported)?;
    raster::check_dims(width, height)?;
    let decoded =
        image::load_from_memory_with_format(data, ImageFormat::Jpeg).map_err(unsupported)?;
    let mut page = raster::from_dynamic(decoded);
    if invert {
        for pixel in page.rgba.chunks_exact_mut(4) {
            for channel in pixel.iter_mut().take(3) {
                *channel = 255 - *channel;
            }
        }
    }
    Ok(page)
}

/// Decodes CCITT data into packed 1-bit rows (PDF convention, `BlackIs1` honoured).
fn decode_ccitt(
    document: &Document,
    parms: Option<&Dictionary>,
    data: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>, PreviewError> {
    let int_parm = |key: &[u8], default: i64| {
        parms
            .and_then(|p| get_i64(document, p, key))
            .unwrap_or(default)
    };
    let flag_parm = |key: &[u8]| {
        parms
            .and_then(|p| get_bool(document, p, key))
            .unwrap_or(false)
    };
    let columns = u32::try_from(int_parm(b"Columns", DEFAULT_CCITT_COLUMNS))
        .map_err(|_| unsupported("invalid CCITT Columns"))?;
    if columns != width {
        return Err(unsupported("CCITT Columns must equal the image Width"));
    }
    let coding = match int_parm(b"K", 0) {
        k if k < 0 => Coding::G4,
        0 => Coding::G3OneDimensional,
        _ => return Err(unsupported("CCITT two-dimensional K>0 is not supported")),
    };
    let decoded = ccitt::decode(
        data,
        CcittParams {
            coding,
            columns,
            rows: height,
            black_is_1: flag_parm(b"BlackIs1"),
            byte_align: flag_parm(b"EncodedByteAlign"),
        },
    )?;
    if decoded.rows_decoded == 0 {
        return Err(unsupported("no CCITT rows could be decoded"));
    }
    if decoded.rows_decoded < height {
        tracing::warn!(
            decoded = decoded.rows_decoded,
            height,
            "CCITT data ended early; remaining rows are left white"
        );
    }
    Ok(decoded.packed)
}

/// Layout of the raw (unfiltered or post-CCITT) samples.
fn raw_layout(
    document: &Document,
    dict: &Dictionary,
    width: u32,
    height: u32,
    is_mask: bool,
    forced_bits: Option<u8>,
) -> Result<RawLayout, PreviewError> {
    let declared_bits = get_i64(document, dict, b"BitsPerComponent");
    let fixed_one_bit = is_mask || forced_bits.is_some();
    let (components, bits): (u8, u8) = if is_mask {
        (1, 1)
    } else if let Some(forced) = forced_bits {
        (1, forced)
    } else {
        let declared = declared_bits.ok_or_else(|| unsupported("image has no BitsPerComponent"))?;
        let bits = u8::try_from(declared)
            .map_err(|_| unsupported(format!("invalid BitsPerComponent {declared}")))?;
        (colour_components(document, dict)?, bits)
    };
    if fixed_one_bit && declared_bits.is_some_and(|declared| declared != i64::from(bits)) {
        return Err(unsupported(
            "image mask or CCITT data must use 1 bit per component",
        ));
    }
    Ok(RawLayout {
        width,
        height,
        components,
        bits,
        invert: decode_inverted(document, dict)?,
    })
}

fn colour_components(document: &Document, dict: &Dictionary) -> Result<u8, PreviewError> {
    let space =
        get(document, dict, b"ColorSpace").ok_or_else(|| unsupported("image has no ColorSpace"))?;
    match space {
        Object::Name(name) => device_components(name),
        Object::Array(items) => icc_components(document, items),
        _ => Err(unsupported("unsupported ColorSpace")),
    }
}

fn device_components(name: &[u8]) -> Result<u8, PreviewError> {
    match name {
        b"DeviceGray" | b"G" => Ok(1),
        b"DeviceRGB" | b"RGB" => Ok(3),
        other => Err(unsupported(format!(
            "colour space {} is not supported",
            String::from_utf8_lossy(other)
        ))),
    }
}

/// `[/ICCBased stream]` with N = 1 or 3; other profiles (CMYK, Lab, ...) are unsupported.
fn icc_components(document: &Document, items: &[Object]) -> Result<u8, PreviewError> {
    let [Object::Name(kind), profile, ..] = items else {
        return Err(unsupported("unsupported ColorSpace array"));
    };
    if kind.as_slice() != b"ICCBased".as_slice() {
        return Err(unsupported("unsupported ColorSpace array"));
    }
    let Some(Object::Stream(profile)) = resolve(document, profile) else {
        return Err(unsupported("ICCBased profile is missing"));
    };
    match get_i64(document, &profile.dict, b"N") {
        Some(1) => Ok(1),
        Some(3) => Ok(3),
        other => Err(unsupported(format!(
            "ICCBased N {other:?} is not supported"
        ))),
    }
}

/// `Decode [1 0]` (every component) inverts samples; `[0 1]` or absent does not.
/// Any other range is unsupported.
fn decode_inverted(document: &Document, dict: &Dictionary) -> Result<bool, PreviewError> {
    let Some(decode) = get(document, dict, b"Decode") else {
        return Ok(false);
    };
    let items = decode
        .as_array()
        .map_err(|_| unsupported("Decode is not an array"))?;
    let values: Vec<f32> = items
        .iter()
        .map(|item| resolve(document, item).and_then(|o| o.as_float().ok()))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| unsupported("invalid Decode array"))?;
    if values.is_empty() {
        return Ok(false);
    }
    if !values.len().is_multiple_of(2) {
        return Err(unsupported("Decode array has an odd length"));
    }
    let pairs = || values.chunks_exact(2);
    if pairs().all(|p| p[0] == 0.0 && p[1] == 1.0) {
        Ok(false)
    } else if pairs().all(|p| p[0] == 1.0 && p[1] == 0.0) {
        Ok(true)
    } else {
        Err(unsupported(
            "Decode ranges other than [0 1] and [1 0] are not supported",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::test_support::png_filter_row;

    #[test]
    fn png_unfilter_reverses_every_filter_type() {
        let width = 9;
        let bpp = 3;
        let original: Vec<Vec<u8>> = (0..10u8)
            .map(|r| {
                (0..width * bpp)
                    .map(|i| (i as u8).wrapping_mul(37).wrapping_add(r.wrapping_mul(91)))
                    .collect()
            })
            .collect();
        let mut encoded = Vec::new();
        let mut previous = vec![0u8; width * bpp];
        for (row, bytes) in original.iter().enumerate() {
            let tag = (row % 5) as u8;
            encoded.push(tag);
            encoded.extend(png_filter_row(tag, bpp, &previous, bytes));
            previous.clone_from(bytes);
        }
        let decoded = png_unfilter(&encoded, 3, 8, width).expect("decodes");
        assert_eq!(decoded, original.concat());
    }

    #[test]
    fn paeth_prefers_left_then_up_then_upper_left() {
        assert_eq!(paeth(10, 20, 10), 20);
        assert_eq!(paeth(10, 10, 30), 10);
        assert_eq!(paeth(0, 0, 0), 0);
    }

    #[test]
    fn png_unfilter_rejects_unknown_filter_tag() {
        assert!(png_unfilter(&[9, 1, 2, 3], 1, 8, 3).is_err());
    }

    #[test]
    fn device_colour_spaces_map_to_component_counts() {
        assert_eq!(device_components(b"DeviceGray").ok(), Some(1));
        assert_eq!(device_components(b"DeviceRGB").ok(), Some(3));
        assert!(device_components(b"DeviceCMYK").is_err());
    }

    #[test]
    fn filter_kinds_cover_supported_and_reject_others() {
        assert_eq!(filter_kind(b"DCTDecode").ok(), Some(FilterKind::Dct));
        assert_eq!(filter_kind(b"CCITTFaxDecode").ok(), Some(FilterKind::Ccitt));
        assert!(filter_kind(b"JBIG2Decode").is_err());
    }
}
