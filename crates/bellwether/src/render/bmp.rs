//! 1-bit monochrome BMP encoder.
//!
//! The TRMNL OG firmware fetches `image_url` and feeds
//! the bytes straight to the e-paper driver. It expects a
//! plain BITMAPFILEHEADER + BITMAPINFOHEADER file with
//! `biBitCount = 1` and a 2-entry black/white palette.
//! We hand-roll the encoder rather than pulling the
//! `image` crate because its BMP codec's 1-bit mode
//! requires indexed pixel buffers we'd have to construct
//! anyway, and the format is ~50 lines of byte writes.

/// BMP file header size in bytes.
const FILE_HEADER_SIZE: u32 = 14;

/// BITMAPINFOHEADER size in bytes.
const INFO_HEADER_SIZE: u32 = 40;

/// Palette size: 2 entries × 4 bytes (BGRX).
const PALETTE_SIZE: u32 = 8;

/// Offset from file start to the pixel data.
const PIXEL_DATA_OFFSET: u32 =
    FILE_HEADER_SIZE + INFO_HEADER_SIZE + PALETTE_SIZE;

/// 72 DPI in pixels-per-metre, the Windows default.
const PELS_PER_METRE: i32 = 2835;

/// Encode a 1-bit-per-pixel image as a monochrome BMP.
///
/// - `bits` is row-major: `bits[y * width + x]`. `false`
///   means black (palette index 0), `true` means white
///   (palette index 1). The image has `width * height`
///   entries. This matches the TRMNL OG firmware's
///   `"standart"` palette path and `ImageMagick` / Pillow
///   default 1-bit output.
/// - Output is a complete BMP file: headers, palette,
///   and pixel data. Rows are stored bottom-up per BMP
///   convention and padded to a 4-byte boundary.
///
/// # Panics
///
/// Panics if `bits.len() != width * height`, if
/// `width == 0 || height == 0`, or if `width > i32::MAX`
/// / `height > i32::MAX` (BMP dimensions are i32). The
/// latter is unreachable in practice: render-config
/// bounds keep both ≤ 4096.
#[must_use]
pub fn encode_1bit_bmp(bits: &[bool], width: u32, height: u32) -> Vec<u8> {
    assert!(width > 0 && height > 0, "zero-sized image");
    assert!(
        i32::try_from(width).is_ok() && i32::try_from(height).is_ok(),
        "dimensions exceed BMP i32 field capacity",
    );
    assert_eq!(
        bits.len() as u64,
        u64::from(width) * u64::from(height),
        "bits length must equal width * height",
    );

    let row_bytes = padded_row_bytes(width);
    let pixel_data_size = row_bytes * height;
    let total_size = PIXEL_DATA_OFFSET + pixel_data_size;

    let mut out = Vec::with_capacity(total_size as usize);

    // BITMAPFILEHEADER
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&total_size.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&PIXEL_DATA_OFFSET.to_le_bytes());

    // BITMAPINFOHEADER
    out.extend_from_slice(&INFO_HEADER_SIZE.to_le_bytes());
    out.extend_from_slice(&i32::try_from(width).unwrap().to_le_bytes());
    out.extend_from_slice(&i32::try_from(height).unwrap().to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    out.extend_from_slice(&1u16.to_le_bytes()); // biBitCount
    out.extend_from_slice(&0u32.to_le_bytes()); // biCompression (BI_RGB)
    out.extend_from_slice(&pixel_data_size.to_le_bytes());
    out.extend_from_slice(&PELS_PER_METRE.to_le_bytes());
    out.extend_from_slice(&PELS_PER_METRE.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes()); // biClrUsed
    out.extend_from_slice(&2u32.to_le_bytes()); // biClrImportant

    // Palette: index 0 = black, index 1 = white (BGRX)
    out.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    out.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0x00]);

    // Pixel data, bottom-up.
    for y in (0..height).rev() {
        let row_start = (y * width) as usize;
        write_row(&mut out, &bits[row_start..row_start + width as usize]);
        let written = unpadded_row_bytes(width);
        out.extend(std::iter::repeat_n(0_u8, (row_bytes - written) as usize));
    }

    debug_assert_eq!(out.len(), total_size as usize);
    out
}

/// Row size in bytes before 4-byte alignment padding.
fn unpadded_row_bytes(width: u32) -> u32 {
    width.div_ceil(8)
}

/// Row size in bytes after padding to a 4-byte boundary.
fn padded_row_bytes(width: u32) -> u32 {
    unpadded_row_bytes(width).div_ceil(4) * 4
}

/// Pack one row of `width` bits (MSB first) into `out`.
fn write_row(out: &mut Vec<u8>, row: &[bool]) {
    let mut byte: u8 = 0;
    let mut bits_in_byte: u8 = 0;
    for &bit in row {
        byte = (byte << 1) | u8::from(bit);
        bits_in_byte += 1;
        if bits_in_byte == 8 {
            out.push(byte);
            byte = 0;
            bits_in_byte = 0;
        }
    }
    // This branch only fires when width % 8 != 0: any
    // whole multiple of 8 bits is flushed inside the
    // loop. Kept for the general-width case.
    if bits_in_byte > 0 {
        byte <<= 8 - bits_in_byte;
        out.push(byte);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_sizes_match_bmp_rules() {
        // width 1 -> 1 data byte, padded to 4.
        assert_eq!(unpadded_row_bytes(1), 1);
        assert_eq!(padded_row_bytes(1), 4);
        // width 8 -> 1 data byte, padded to 4.
        assert_eq!(unpadded_row_bytes(8), 1);
        assert_eq!(padded_row_bytes(8), 4);
        // width 9 -> 2 data bytes, padded to 4.
        assert_eq!(unpadded_row_bytes(9), 2);
        assert_eq!(padded_row_bytes(9), 4);
        // width 32 -> 4 data bytes, no padding.
        assert_eq!(unpadded_row_bytes(32), 4);
        assert_eq!(padded_row_bytes(32), 4);
        // width 33 -> 5 data bytes, padded to 8.
        assert_eq!(unpadded_row_bytes(33), 5);
        assert_eq!(padded_row_bytes(33), 8);
        // TRMNL OG width 800 -> 100 data bytes, already aligned.
        assert_eq!(unpadded_row_bytes(800), 100);
        assert_eq!(padded_row_bytes(800), 100);
    }

    #[test]
    fn encodes_all_white_8x1() {
        let bits = vec![true; 8];
        let bmp = encode_1bit_bmp(&bits, 8, 1);
        // Header (14) + info (40) + palette (8) + row (4) = 66.
        assert_eq!(bmp.len(), 66);
        assert_eq!(&bmp[..2], b"BM");
        let total_size = u32::from_le_bytes([bmp[2], bmp[3], bmp[4], bmp[5]]);
        assert_eq!(total_size, 66);
        // Pixel data: one byte 0xFF followed by 3 padding bytes.
        assert_eq!(bmp[62..66], [0xFF, 0, 0, 0]);
    }

    #[test]
    fn encodes_all_black_8x1() {
        let bits = vec![false; 8];
        let bmp = encode_1bit_bmp(&bits, 8, 1);
        assert_eq!(bmp[62..66], [0x00, 0, 0, 0]);
    }

    #[test]
    fn packs_bits_msb_first() {
        // Pattern: 1010_1010 -> 0xAA
        let bits = vec![true, false, true, false, true, false, true, false];
        let bmp = encode_1bit_bmp(&bits, 8, 1);
        assert_eq!(bmp[62], 0xAA);
    }

    #[test]
    fn zero_pads_trailing_bits_in_final_byte() {
        // 5 bits (10100) -> 0b10100_000 = 0xA0
        let bits = vec![true, false, true, false, false];
        let bmp = encode_1bit_bmp(&bits, 5, 1);
        assert_eq!(bmp[62], 0xA0);
    }

    #[test]
    fn rows_stored_bottom_up() {
        // 8x2 image: row 0 = all white, row 1 = all black.
        // BMP stores row 1 first, then row 0.
        let mut bits = Vec::new();
        bits.extend(std::iter::repeat_n(true, 8)); // row 0 top
        bits.extend(std::iter::repeat_n(false, 8)); // row 1 bottom
        let bmp = encode_1bit_bmp(&bits, 8, 2);
        // Expect: row 1 (black) first, then row 0 (white).
        assert_eq!(bmp[62], 0x00);
        assert_eq!(bmp[66], 0xFF);
    }

    #[test]
    fn header_dimensions_match_input() {
        let bits = vec![false; 800 * 480];
        let bmp = encode_1bit_bmp(&bits, 800, 480);
        let width = i32::from_le_bytes([bmp[18], bmp[19], bmp[20], bmp[21]]);
        let height = i32::from_le_bytes([bmp[22], bmp[23], bmp[24], bmp[25]]);
        let bits_per_pixel = u16::from_le_bytes([bmp[28], bmp[29]]);
        assert_eq!(width, 800);
        assert_eq!(height, 480);
        assert_eq!(bits_per_pixel, 1);
        // Full file size = 62 + (100 * 480) = 48062 bytes.
        assert_eq!(bmp.len(), 62 + 100 * 480);
    }

    #[test]
    fn palette_is_black_then_white() {
        let bits = vec![false; 8];
        let bmp = encode_1bit_bmp(&bits, 8, 1);
        assert_eq!(bmp[54..58], [0x00, 0x00, 0x00, 0x00]);
        assert_eq!(bmp[58..62], [0xFF, 0xFF, 0xFF, 0x00]);
    }

    #[test]
    #[should_panic(expected = "zero-sized image")]
    fn zero_width_panics() {
        let _ = encode_1bit_bmp(&[], 0, 1);
    }

    #[test]
    #[should_panic(expected = "bits length")]
    fn mismatched_length_panics() {
        let _ = encode_1bit_bmp(&[true; 7], 8, 1);
    }
}
