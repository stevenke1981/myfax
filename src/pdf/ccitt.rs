//! CCITT fax decoding (ITU-T T.4 one-dimensional and T.6) into packed rows.
//!
//! Shared by PDF `CCITTFaxDecode` and TIFF Compression 3 / 4. Two-dimensional
//! T.4 (K > 0) is not provided by the `fax` crate and is rejected by callers.

use super::{PreviewError, unsupported};
use fax::decoder::{DecodeStatus, Group4Decoder};
use fax::maps::{black, white};
use fax::{BitReader, slice_reader};
use std::convert::Infallible;

/// Zero bytes appended to the input so fixed-width code lookups near the end
/// of the data still succeed. Zeros cannot form a valid code on their own.
const LOOKAHEAD_PAD: usize = 8;
/// Longest accepted scan line in pixels (the `fax` crate takes the width as `u16`).
const MAX_COLUMNS: u32 = u16::MAX as u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Coding {
    /// T.6 / Group 4: two-dimensional, no end-of-line codes.
    G4,
    /// T.4 one-dimensional: Modified Huffman run lengths, one row at a time.
    G3OneDimensional,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CcittParams {
    pub coding: Coding,
    pub columns: u32,
    pub rows: u32,
    /// `true`: black pixels decode to 1 bits. `false`: black pixels decode to 0 bits (PDF default).
    pub black_is_1: bool,
    /// Every one-dimensional row starts on a byte boundary (PDF `EncodedByteAlign`).
    pub byte_align: bool,
}

#[derive(Debug)]
pub(super) struct Decoded {
    /// `ceil(columns / 8) * rows` bytes, MSB first. Undecoded rows stay white.
    pub packed: Vec<u8>,
    /// Number of leading rows that decoded successfully.
    pub rows_decoded: u32,
}

/// Decodes `params.rows` rows. Malformed data ends decoding early; the rows
/// decoded so far are returned and the remainder is left white.
pub(super) fn decode(input: &[u8], params: CcittParams) -> Result<Decoded, PreviewError> {
    validate(params)?;
    let columns = params.columns;
    let stride = columns.div_ceil(8) as usize;
    let rows = params.rows as usize;
    let total = stride
        .checked_mul(rows)
        .ok_or_else(|| unsupported("CCITT image size overflow"))?;
    let (white_bit, black_bit) = if params.black_is_1 { (0, 1) } else { (1, 0) };
    let white_byte = if white_bit == 1 { 0xFF } else { 0x00 };
    let mut packed = vec![white_byte; total];
    let mut padded = Vec::with_capacity(input.len() + LOOKAHEAD_PAD);
    padded.extend_from_slice(input);
    padded.resize(input.len() + LOOKAHEAD_PAD, 0);

    let rows_decoded = {
        let mut paint = |row: usize, transitions: &[u16]| {
            paint_row(&mut packed, stride, row, transitions, columns, black_bit);
        };
        match params.coding {
            Coding::G4 => decode_g4_rows(&padded, columns, rows, &mut paint),
            Coding::G3OneDimensional => {
                decode_g3_rows(&padded, columns, rows, params.byte_align, &mut paint)
            }
        }
    };
    Ok(Decoded {
        packed,
        rows_decoded: u32::try_from(rows_decoded).unwrap_or(u32::MAX),
    })
}

fn validate(params: CcittParams) -> Result<(), PreviewError> {
    if params.columns == 0 || params.columns > MAX_COLUMNS {
        return Err(unsupported(format!(
            "CCITT line width {} is out of range",
            params.columns
        )));
    }
    if params.coding == Coding::G4 && params.byte_align {
        return Err(unsupported("byte-aligned Group 4 data is not supported"));
    }
    Ok(())
}

/// Unwraps a result whose error type is uninhabited (the in-memory reader cannot fail).
fn infallible<T>(result: Result<T, Infallible>) -> T {
    match result {
        Ok(value) => value,
        Err(never) => match never {},
    }
}

/// Emits each Group 4 row as its list of colour-change positions, starting with white.
fn decode_g4_rows(
    input: &[u8],
    columns: u32,
    rows: usize,
    emit: &mut impl FnMut(usize, &[u16]),
) -> usize {
    let Ok(width) = u16::try_from(columns) else {
        return 0;
    };
    let mut decoder = infallible(Group4Decoder::new(
        input.iter().copied().map(Ok::<u8, Infallible>),
        width,
    ));
    let mut decoded = 0;
    while decoded < rows {
        match decoder.advance() {
            Ok(DecodeStatus::Incomplete) => {
                emit(decoded, decoder.transition());
                decoded += 1;
            }
            Ok(DecodeStatus::End) | Err(_) => break,
        }
    }
    decoded
}

/// Decodes T.4 one-dimensional rows. End-of-line codes are skipped when present.
fn decode_g3_rows(
    input: &[u8],
    columns: u32,
    rows: usize,
    byte_align: bool,
    emit: &mut impl FnMut(usize, &[u16]),
) -> usize {
    let mut reader = slice_reader(input);
    let mut decoded = 0;
    while decoded < rows {
        if !skip_eol(&mut reader) && byte_align {
            let fill = reader.bits_to_byte_boundary();
            infallible(reader.consume(fill));
        }
        let Some(transitions) = read_row_1d(&mut reader, columns) else {
            break;
        };
        emit(decoded, &transitions);
        decoded += 1;
    }
    decoded
}

/// Skips an end-of-line code (eleven zero bits and a one), with optional fill zeros.
/// Returns `false` and consumes nothing when no end-of-line code is next.
fn skip_eol(reader: &mut impl BitReader) -> bool {
    let Some(window) = reader.peek(16) else {
        return false;
    };
    let zeros = window.leading_zeros();
    if window == 0 || zeros < 11 {
        return false;
    }
    reader.consume(zeros as u8 + 1).is_ok()
}

/// Reads one one-dimensional row as colour-change positions, starting with white.
fn read_row_1d(reader: &mut impl BitReader, columns: u32) -> Option<Vec<u16>> {
    let mut transitions = Vec::new();
    let mut position: u32 = 0;
    let mut is_black = false;
    while position < columns {
        let run = read_run(reader, is_black)?;
        position = position.checked_add(run)?;
        let edge = position.min(columns);
        transitions.push(u16::try_from(edge).ok()?);
        is_black = !is_black;
    }
    Some(transitions)
}

/// Reads one run length: make-up codes followed by a terminating code (< 64).
fn read_run(reader: &mut impl BitReader, is_black: bool) -> Option<u32> {
    let mut total: u32 = 0;
    loop {
        let code = if is_black {
            black::decode(reader)?
        } else {
            white::decode(reader)?
        };
        total = total.checked_add(u32::from(code))?;
        if code < 64 {
            return Some(total);
        }
    }
}

/// Writes one row of black runs into `packed`; the row must already be white.
fn paint_row(
    packed: &mut [u8],
    stride: usize,
    row: usize,
    transitions: &[u16],
    columns: u32,
    black_bit: u8,
) {
    let width = columns as usize;
    let Some(start) = row.checked_mul(stride) else {
        return;
    };
    let Some(end) = start.checked_add(stride) else {
        return;
    };
    let Some(dst) = packed.get_mut(start..end) else {
        return;
    };
    let mut position = 0usize;
    let mut is_black = false;
    let edges = transitions
        .iter()
        .map(|&edge| usize::from(edge))
        .chain(std::iter::once(width));
    for edge in edges {
        let stop = edge.min(width);
        if is_black && stop > position {
            paint_span(dst, position, stop, black_bit);
        }
        position = position.max(stop);
        is_black = !is_black;
    }
}

fn paint_span(row: &mut [u8], from: usize, to: usize, black_bit: u8) {
    for x in from..to {
        let Some(byte) = row.get_mut(x / 8) else {
            break;
        };
        let mask = 0x80u8 >> (x % 8);
        if black_bit == 1 {
            *byte |= mask;
        } else {
            *byte &= !mask;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{checker_rows, g4_encode, mh_encode, packed_reference};
    use super::*;

    fn params(coding: Coding, columns: u32, rows: u32) -> CcittParams {
        CcittParams {
            coding,
            columns,
            rows,
            black_is_1: false,
            byte_align: false,
        }
    }

    #[test]
    fn group4_round_trip_matches_source_pixels() {
        let rows = checker_rows(77, 40);
        let encoded = g4_encode(&rows);
        let decoded = decode(&encoded, params(Coding::G4, 77, 40)).expect("decodes");
        assert_eq!(decoded.rows_decoded, 40);
        assert_eq!(decoded.packed, packed_reference(&rows, false));
    }

    #[test]
    fn group3_one_dimensional_round_trip_with_and_without_eol() {
        let rows = checker_rows(130, 25);
        for eol in [false, true] {
            let encoded = mh_encode(&rows, eol, false);
            let decoded =
                decode(&encoded, params(Coding::G3OneDimensional, 130, 25)).expect("decodes");
            assert_eq!(decoded.rows_decoded, 25, "eol={eol}");
            assert_eq!(decoded.packed, packed_reference(&rows, false), "eol={eol}");
        }
    }

    #[test]
    fn group3_byte_aligned_rows_decode() {
        let rows = checker_rows(45, 12);
        let encoded = mh_encode(&rows, false, true);
        let mut p = params(Coding::G3OneDimensional, 45, 12);
        p.byte_align = true;
        let decoded = decode(&encoded, p).expect("decodes");
        assert_eq!(decoded.rows_decoded, 12);
        assert_eq!(decoded.packed, packed_reference(&rows, false));
    }

    #[test]
    fn long_runs_use_make_up_codes() {
        let mut rows = vec![vec![false; 3000]; 2];
        rows[1][..2900].fill(true);
        let encoded = mh_encode(&rows, true, false);
        let decoded = decode(&encoded, params(Coding::G3OneDimensional, 3000, 2)).expect("decodes");
        assert_eq!(decoded.rows_decoded, 2);
        assert_eq!(decoded.packed, packed_reference(&rows, false));
    }

    #[test]
    fn black_is_one_inverts_the_packed_bits() {
        let rows = checker_rows(20, 5);
        let encoded = g4_encode(&rows);
        let mut p = params(Coding::G4, 20, 5);
        p.black_is_1 = true;
        let decoded = decode(&encoded, p).expect("decodes");
        assert_eq!(decoded.packed, packed_reference(&rows, true));
    }

    #[test]
    fn truncated_data_yields_partial_rows_without_panic() {
        let rows = checker_rows(64, 30);
        let encoded = g4_encode(&rows);
        let cut = &encoded[..encoded.len() / 2];
        let decoded = decode(cut, params(Coding::G4, 64, 30)).expect("no config error");
        assert!(decoded.rows_decoded < 30);
        assert_eq!(decoded.packed.len(), 8 * 30);
    }

    #[test]
    fn garbage_input_never_panics() {
        let garbage: Vec<u8> = (0..400u32).map(|i| (i * 131 + 7) as u8).collect();
        for coding in [Coding::G4, Coding::G3OneDimensional] {
            let decoded = decode(&garbage, params(coding, 100, 50)).expect("no config error");
            assert!(decoded.rows_decoded <= 50);
        }
    }

    #[test]
    fn invalid_configuration_is_rejected() {
        assert!(decode(&[], params(Coding::G4, 0, 1)).is_err());
        assert!(decode(&[], params(Coding::G4, 70_000, 1)).is_err());
        let mut p = params(Coding::G4, 10, 1);
        p.byte_align = true;
        assert!(decode(&[], p).is_err());
    }
}
