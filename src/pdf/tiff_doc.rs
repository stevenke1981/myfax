//! TIFF page access: manual IFD walk, CCITT pages via `ccitt`, other pages via `image`.

use super::ccitt::{self, CcittParams, Coding};
use super::raster::{self, RawLayout};
use super::{PreviewError, unsupported};
use crate::model::PageImage;
use image::{ImageFormat, ImageReader};
use std::collections::HashSet;
use std::io::Cursor;

const TAG_IMAGE_WIDTH: u16 = 256;
const TAG_IMAGE_LENGTH: u16 = 257;
const TAG_COMPRESSION: u16 = 259;
const TAG_PHOTOMETRIC: u16 = 262;
const TAG_FILL_ORDER: u16 = 266;
const TAG_STRIP_OFFSETS: u16 = 273;
const TAG_ROWS_PER_STRIP: u16 = 278;
const TAG_STRIP_BYTE_COUNTS: u16 = 279;
const TAG_T4_OPTIONS: u16 = 292;

const COMPRESSION_CCITT_T4: u32 = 3;
const COMPRESSION_CCITT_T6: u32 = 4;
/// Most image directories followed in one file.
const MAX_IFDS: usize = 65_536;
/// Size of one IFD entry on disk.
const ENTRY_LEN: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ByteOrder {
    Little,
    Big,
}

/// Parsed header plus the offset of every image file directory (IFD).
struct TiffFile<'a> {
    data: &'a [u8],
    order: ByteOrder,
    ifds: Vec<usize>,
}

/// One IFD entry: tag, field type, value count and the raw 4-byte value field.
struct Entry {
    tag: u16,
    field_type: u16,
    count: u32,
    field: [u8; 4],
}

struct Ifd {
    entries: Vec<Entry>,
}

pub(super) fn page_count(data: &[u8]) -> Result<u32, PreviewError> {
    let file = TiffFile::parse(data)?;
    u32::try_from(file.ifds.len()).map_err(|_| unsupported("too many TIFF pages"))
}

pub(super) fn render_page(data: &[u8], page: u32) -> Result<PageImage, PreviewError> {
    let file = TiffFile::parse(data)?;
    let index = usize::try_from(page)
        .ok()
        .filter(|index| *index < file.ifds.len())
        .ok_or(PreviewError::PageOutOfRange(page))?;
    let ifd = file.ifd(index)?;
    match ifd.first_u32(&file, TAG_COMPRESSION)?.unwrap_or(1) {
        COMPRESSION_CCITT_T4 | COMPRESSION_CCITT_T6 => render_ccitt_page(&file, &ifd),
        _ if index == 0 => decode_with_image_crate(data),
        _ => Err(unsupported(
            "only the first TIFF page can be previewed for this compression",
        )),
    }
}

impl<'a> TiffFile<'a> {
    fn parse(data: &'a [u8]) -> Result<Self, PreviewError> {
        let order = match data.get(..2) {
            Some(b"II") => ByteOrder::Little,
            Some(b"MM") => ByteOrder::Big,
            _ => return Err(unsupported("not a TIFF file")),
        };
        if u16_at(data, 2, order)? != 42 {
            return Err(unsupported("only classic TIFF (magic 42) is supported"));
        }
        let mut next = to_usize(u32_at(data, 4, order)?);
        let mut ifds = Vec::new();
        let mut seen = HashSet::new();
        while next != 0 {
            if !seen.insert(next) {
                return Err(unsupported("TIFF directory chain loops"));
            }
            if ifds.len() >= MAX_IFDS {
                return Err(unsupported("too many TIFF directories"));
            }
            let count = usize::from(u16_at(data, next, order)?);
            let following = count
                .checked_mul(ENTRY_LEN)
                .and_then(|len| len.checked_add(next.checked_add(2)?))
                .ok_or_else(|| unsupported("TIFF directory offset overflow"))?;
            let after = u32_at(data, following, order)?;
            ifds.push(next);
            next = to_usize(after);
        }
        if ifds.is_empty() {
            return Err(unsupported("TIFF has no image directories"));
        }
        Ok(Self { data, order, ifds })
    }

    fn ifd(&self, index: usize) -> Result<Ifd, PreviewError> {
        let offset = *self
            .ifds
            .get(index)
            .ok_or(PreviewError::PageOutOfRange(index as u32))?;
        let count = usize::from(u16_at(self.data, offset, self.order)?);
        let entries = (0..count)
            .map(|i| self.entry_at(offset + 2 + i * ENTRY_LEN))
            .collect::<Result<Vec<_>, PreviewError>>()?;
        Ok(Ifd { entries })
    }

    fn entry_at(&self, pos: usize) -> Result<Entry, PreviewError> {
        let field: [u8; 4] = bytes_at(self.data, pos + 8, 4)?
            .try_into()
            .map_err(|_| unsupported("truncated TIFF entry"))?;
        Ok(Entry {
            tag: u16_at(self.data, pos, self.order)?,
            field_type: u16_at(self.data, pos + 2, self.order)?,
            count: u32_at(self.data, pos + 4, self.order)?,
            field,
        })
    }

    /// All values of an integer entry (SHORT or LONG) as `u32`.
    fn values(&self, entry: &Entry) -> Result<Vec<u32>, PreviewError> {
        let element = match entry.field_type {
            3 => 2usize,
            4 => 4usize,
            other => {
                return Err(unsupported(format!(
                    "TIFF field type {other} is not supported"
                )));
            }
        };
        let count = usize::try_from(entry.count).map_err(|_| unsupported("TIFF count overflow"))?;
        let total = count
            .checked_mul(element)
            .ok_or_else(|| unsupported("TIFF value size overflow"))?;
        let bytes = if total <= 4 {
            entry
                .field
                .get(..total)
                .ok_or_else(|| unsupported("truncated TIFF value"))?
        } else {
            bytes_at(self.data, to_usize(entry.field_u32(self.order)), total)?
        };
        bytes
            .chunks_exact(element)
            .map(|chunk| read_element(chunk, self.order))
            .collect()
    }
}

impl Entry {
    /// The 4-byte value field read as a LONG in the file's byte order.
    fn field_u32(&self, order: ByteOrder) -> u32 {
        match order {
            ByteOrder::Little => u32::from_le_bytes(self.field),
            ByteOrder::Big => u32::from_be_bytes(self.field),
        }
    }
}

/// Reads one SHORT (2 bytes) or LONG (4 bytes) element in the file's byte order.
fn read_element(chunk: &[u8], order: ByteOrder) -> Result<u32, PreviewError> {
    match (chunk, order) {
        ([a, b], ByteOrder::Little) => Ok(u32::from(u16::from_le_bytes([*a, *b]))),
        ([a, b], ByteOrder::Big) => Ok(u32::from(u16::from_be_bytes([*a, *b]))),
        ([a, b, c, d], ByteOrder::Little) => Ok(u32::from_le_bytes([*a, *b, *c, *d])),
        ([a, b, c, d], ByteOrder::Big) => Ok(u32::from_be_bytes([*a, *b, *c, *d])),
        _ => Err(unsupported("truncated TIFF value")),
    }
}

impl Ifd {
    fn entry(&self, tag: u16) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.tag == tag)
    }

    fn values(&self, file: &TiffFile<'_>, tag: u16) -> Result<Option<Vec<u32>>, PreviewError> {
        self.entry(tag).map(|entry| file.values(entry)).transpose()
    }

    fn first_u32(&self, file: &TiffFile<'_>, tag: u16) -> Result<Option<u32>, PreviewError> {
        Ok(self.values(file, tag)?.and_then(|v| v.first().copied()))
    }

    fn required_u32(&self, file: &TiffFile<'_>, tag: u16) -> Result<u32, PreviewError> {
        self.first_u32(file, tag)?
            .ok_or_else(|| unsupported(format!("TIFF tag {tag} is missing")))
    }
}

/// Renders a CCITT (T.4 or T.6) page strip by strip; strips are independent.
fn render_ccitt_page(file: &TiffFile<'_>, ifd: &Ifd) -> Result<PageImage, PreviewError> {
    let width = ifd.required_u32(file, TAG_IMAGE_WIDTH)?;
    let height = ifd.required_u32(file, TAG_IMAGE_LENGTH)?;
    raster::check_dims(width, height)?;
    let compression = ifd.required_u32(file, TAG_COMPRESSION)?;
    check_photometric_and_fill_order(file, ifd)?;
    let t4_options = ifd.first_u32(file, TAG_T4_OPTIONS)?.unwrap_or(0);
    if compression == COMPRESSION_CCITT_T4 && t4_options & 1 != 0 {
        return Err(unsupported(
            "two-dimensional T.4 (T4Options bit 0) is not supported",
        ));
    }
    let coding = if compression == COMPRESSION_CCITT_T6 {
        Coding::G4
    } else {
        Coding::G3OneDimensional
    };
    let rows_per_strip = ifd
        .first_u32(file, TAG_ROWS_PER_STRIP)?
        .unwrap_or(height)
        .clamp(1, height);
    let offsets = ifd
        .values(file, TAG_STRIP_OFFSETS)?
        .ok_or_else(|| unsupported("TIFF has no strip offsets"))?;
    let counts = ifd
        .values(file, TAG_STRIP_BYTE_COUNTS)?
        .ok_or_else(|| unsupported("TIFF has no strip byte counts"))?;
    if offsets.len() != counts.len() {
        return Err(unsupported("TIFF strip offsets and byte counts differ"));
    }
    let strips = height.div_ceil(rows_per_strip) as usize;
    if offsets.len() < strips {
        return Err(unsupported("TIFF is missing strips"));
    }

    let stride = (width as usize).div_ceil(8);
    let mut packed = Vec::with_capacity(stride * height as usize);
    let mut rows_total: u64 = 0;
    let strip_refs = offsets.iter().zip(counts.iter()).take(strips);
    for (strip, (&offset, &length)) in strip_refs.enumerate() {
        let first_row = (strip as u32).saturating_mul(rows_per_strip);
        let rows = rows_per_strip.min(height.saturating_sub(first_row));
        let bytes = bytes_at(file.data, to_usize(offset), to_usize(length))?;
        let decoded = ccitt::decode(
            bytes,
            CcittParams {
                coding,
                columns: width,
                rows,
                black_is_1: false,
                byte_align: false,
            },
        )?;
        if decoded.rows_decoded < rows {
            tracing::warn!(
                strip,
                decoded = decoded.rows_decoded,
                rows,
                "CCITT strip ended early; remaining rows are left white"
            );
        }
        rows_total += u64::from(decoded.rows_decoded);
        packed.extend_from_slice(&decoded.packed);
    }
    if rows_total == 0 {
        return Err(unsupported("no CCITT rows could be decoded"));
    }
    let layout = RawLayout {
        width,
        height,
        components: 1,
        bits: 1,
        invert: false,
    };
    raster::raw_to_rgba(&layout, &packed)
}

fn check_photometric_and_fill_order(file: &TiffFile<'_>, ifd: &Ifd) -> Result<(), PreviewError> {
    match ifd.first_u32(file, TAG_PHOTOMETRIC)? {
        None | Some(0) | Some(1) => {}
        Some(other) => {
            return Err(unsupported(format!(
                "CCITT photometric interpretation {other} is not supported"
            )));
        }
    }
    match ifd.first_u32(file, TAG_FILL_ORDER)? {
        None | Some(1) => Ok(()),
        Some(other) => Err(unsupported(format!(
            "TIFF FillOrder {other} is not supported"
        ))),
    }
}

/// Non-CCITT first page: the `image` crate's TIFF decoder.
fn decode_with_image_crate(data: &[u8]) -> Result<PageImage, PreviewError> {
    let (width, height) = ImageReader::with_format(Cursor::new(data), ImageFormat::Tiff)
        .into_dimensions()
        .map_err(unsupported)?;
    raster::check_dims(width, height)?;
    let decoded =
        image::load_from_memory_with_format(data, ImageFormat::Tiff).map_err(unsupported)?;
    Ok(raster::from_dynamic(decoded))
}

fn to_usize(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

/// `len` bytes at `pos`, failing (never panicking) when the range leaves the file.
fn bytes_at(data: &[u8], pos: usize, len: usize) -> Result<&[u8], PreviewError> {
    let end = pos
        .checked_add(len)
        .ok_or_else(|| unsupported("TIFF offset overflow"))?;
    data.get(pos..end)
        .ok_or_else(|| unsupported("TIFF data is truncated"))
}

fn u16_at(data: &[u8], pos: usize, order: ByteOrder) -> Result<u16, PreviewError> {
    let bytes: [u8; 2] = bytes_at(data, pos, 2)?
        .try_into()
        .map_err(|_| unsupported("TIFF data is truncated"))?;
    Ok(match order {
        ByteOrder::Little => u16::from_le_bytes(bytes),
        ByteOrder::Big => u16::from_be_bytes(bytes),
    })
}

fn u32_at(data: &[u8], pos: usize, order: ByteOrder) -> Result<u32, PreviewError> {
    let bytes: [u8; 4] = bytes_at(data, pos, 4)?
        .try_into()
        .map_err(|_| unsupported("TIFF data is truncated"))?;
    Ok(match order {
        ByteOrder::Little => u32::from_le_bytes(bytes),
        ByteOrder::Big => u32::from_be_bytes(bytes),
    })
}
