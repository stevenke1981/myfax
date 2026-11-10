//! Pixel-format conversion and downscaling for decoded page bitmaps.

use super::{MAX_PIXELS, MAX_SIDE_PX, PreviewError, unsupported};
use crate::model::PageImage;
use image::imageops::{self, FilterType};
use image::{DynamicImage, ImageBuffer, Rgba};

/// Rejects empty, oversized or implausibly large page bitmaps before any allocation.
pub(super) fn check_dims(width: u32, height: u32) -> Result<(), PreviewError> {
    if width == 0 || height == 0 {
        return Err(unsupported("image has no pixels"));
    }
    if width > MAX_SIDE_PX || height > MAX_SIDE_PX {
        return Err(unsupported(format!("image {width}x{height} is too large")));
    }
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(unsupported(format!(
            "image {width}x{height} exceeds the pixel limit"
        )));
    }
    Ok(())
}

/// Describes packed samples before conversion to RGBA.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RawLayout {
    pub width: u32,
    pub height: u32,
    /// 1 (DeviceGray) or 3 (DeviceRGB) colour components.
    pub components: u8,
    /// 1 or 8 bits per component.
    pub bits: u8,
    /// `Decode [1 0]`: every sample value is inverted.
    pub invert: bool,
}

/// Converts packed raw samples (rows padded to whole bytes) into RGBA8.
/// One-bit samples map 0 → black and 1 → white before the optional inversion.
pub(super) fn raw_to_rgba(layout: &RawLayout, data: &[u8]) -> Result<PageImage, PreviewError> {
    check_dims(layout.width, layout.height)?;
    validate_sample_format(layout)?;
    let width = layout.width as usize;
    let height = layout.height as usize;
    let row_bits = width
        .checked_mul(usize::from(layout.components))
        .and_then(|v| v.checked_mul(usize::from(layout.bits)))
        .ok_or_else(|| unsupported("row size overflow"))?;
    let stride = row_bits.div_ceil(8);
    let needed = stride
        .checked_mul(height)
        .ok_or_else(|| unsupported("image size overflow"))?;
    let source = data
        .get(..needed)
        .ok_or_else(|| unsupported("image data is shorter than its dimensions require"))?;
    let mut rgba = vec![0u8; rgba_len(layout.width, layout.height)?];
    let out_stride = width * 4;
    for (row, out_row) in source
        .chunks_exact(stride)
        .zip(rgba.chunks_exact_mut(out_stride))
    {
        for (x, pixel) in out_row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let [r, g, b] = sample_rgb(row, x, layout);
            *pixel = [r, g, b, 255];
        }
    }
    Ok(PageImage {
        width: layout.width,
        height: layout.height,
        rgba,
    })
}

fn validate_sample_format(layout: &RawLayout) -> Result<(), PreviewError> {
    match (layout.bits, layout.components) {
        (8, 1) | (8, 3) => Ok(()),
        (1, 1) => Ok(()),
        (bits, components) => Err(unsupported(format!(
            "{bits}-bit samples with {components} components are not supported"
        ))),
    }
}

fn sample_rgb(row: &[u8], x: usize, layout: &RawLayout) -> [u8; 3] {
    let rgb = match (layout.bits, layout.components) {
        (1, _) => {
            let byte = row.get(x / 8).copied().unwrap_or(0);
            let bit = (byte >> (7 - x % 8)) & 1;
            [bit * 255; 3]
        }
        (_, 3) => {
            let base = x * 3;
            [
                row.get(base).copied().unwrap_or(0),
                row.get(base + 1).copied().unwrap_or(0),
                row.get(base + 2).copied().unwrap_or(0),
            ]
        }
        _ => [row.get(x).copied().unwrap_or(0); 3],
    };
    if layout.invert {
        rgb.map(|value| 255 - value)
    } else {
        rgb
    }
}

/// Byte length of an RGBA8 bitmap, with overflow checked.
pub(super) fn rgba_len(width: u32, height: u32) -> Result<usize, PreviewError> {
    usize::try_from(u64::from(width) * u64::from(height) * 4)
        .map_err(|_| unsupported("image size overflow"))
}

/// Converts a decoder result from the `image` crate into a page bitmap.
pub(super) fn from_dynamic(image: DynamicImage) -> PageImage {
    let rgba = image.into_rgba8();
    PageImage {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
    }
}

/// Downscales (never upscales) so that the longer side is at most `max_side`.
pub(super) fn fit_within(image: PageImage, max_side: u32) -> Result<PageImage, PreviewError> {
    let longer = image.width.max(image.height);
    if longer <= max_side {
        return Ok(image);
    }
    let width = scaled_side(image.width, longer, max_side);
    let height = scaled_side(image.height, longer, max_side);
    let source = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(image.width, image.height, image.rgba)
        .ok_or_else(|| unsupported("bitmap buffer size mismatch"))?;
    let resized = imageops::resize(&source, width, height, FilterType::Triangle);
    Ok(PageImage {
        width,
        height: resized.height(),
        rgba: resized.into_raw(),
    })
}

/// `side * max_side / longer` in integer arithmetic, at least one pixel.
fn scaled_side(side: u32, longer: u32, max_side: u32) -> u32 {
    let scaled = u64::from(side) * u64::from(max_side) / u64::from(longer);
    u32::try_from(scaled.max(1)).unwrap_or(max_side)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_bit_samples_map_zero_to_black() {
        let layout = RawLayout {
            width: 4,
            height: 1,
            components: 1,
            bits: 1,
            invert: false,
        };
        let page = raw_to_rgba(&layout, &[0b1010_0000]).expect("decodes");
        let reds: Vec<u8> = page
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|px| px[0])
            .collect();
        assert_eq!(reds, vec![255, 0, 255, 0]);
        assert_eq!(page.rgba[3], 255);
    }

    #[test]
    fn invert_flips_every_channel() {
        let layout = RawLayout {
            width: 1,
            height: 1,
            components: 3,
            bits: 8,
            invert: true,
        };
        let page = raw_to_rgba(&layout, &[10, 20, 30]).expect("decodes");
        assert_eq!(page.rgba, vec![245, 235, 225, 255]);
    }

    #[test]
    fn short_data_is_rejected() {
        let layout = RawLayout {
            width: 8,
            height: 2,
            components: 1,
            bits: 8,
            invert: false,
        };
        assert!(raw_to_rgba(&layout, &[0; 15]).is_err());
    }

    #[test]
    fn oversized_dimensions_are_rejected() {
        assert!(check_dims(MAX_SIDE_PX + 1, 1).is_err());
        assert!(check_dims(0, 10).is_err());
        assert!(check_dims(12_000, 12_000).is_err());
        assert!(check_dims(1728, 2339).is_ok());
    }

    #[test]
    fn fit_within_keeps_aspect_ratio_without_upscaling() {
        let page = PageImage {
            width: 2000,
            height: 1000,
            rgba: vec![128; 2000 * 1000 * 4],
        };
        let small = fit_within(page.clone(), 500).expect("fits");
        assert_eq!((small.width, small.height), (500, 250));
        assert_eq!(small.rgba.len(), 500 * 250 * 4);
        let same = fit_within(page, 5000).expect("no change");
        assert_eq!((same.width, same.height), (2000, 1000));
    }
}
