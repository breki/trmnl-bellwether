//! Floyd–Steinberg dithering (grayscale → 1-bit).
//!
//! The algorithm diffuses the per-pixel rounding error
//! to the 4 neighbours that haven't been visited yet:
//!
//! ```text
//!           *   7
//!       3   5   1     (all divided by 16)
//! ```
//!
//! This yields a balanced 1-bit output that looks much
//! better on e-ink than naïve thresholding, especially
//! for photographic / weather-icon source material.

/// Dither an 8-bit grayscale buffer to 1-bit.
///
/// - `grayscale` is row-major, `width * height` bytes,
///   with 0 = black and 255 = white.
/// - Output `bits` is row-major, same dimensions:
///   `true` for white (pixel ≥ threshold after error
///   diffusion), `false` for black.
///
/// # Panics
///
/// Panics if `grayscale.len() != width * height`.
#[must_use]
pub fn floyd_steinberg(grayscale: &[u8], width: u32, height: u32) -> Vec<bool> {
    assert_eq!(
        grayscale.len() as u64,
        u64::from(width) * u64::from(height),
        "grayscale length must equal width * height",
    );
    let w = width as usize;
    let h = height as usize;

    // Work in i16 so error diffusion can't overflow u8.
    let mut buf: Vec<i16> = grayscale.iter().map(|&g| i16::from(g)).collect();
    let mut bits = vec![false; w * h];

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let old = buf[idx];
            let (new_bit, new_gray) = if old >= 128 {
                (true, 255_i16)
            } else {
                (false, 0_i16)
            };
            bits[idx] = new_bit;
            let err = old - new_gray;
            if x + 1 < w {
                diffuse(&mut buf, y * w + x + 1, err, 7);
            }
            if y + 1 < h {
                if x > 0 {
                    diffuse(&mut buf, (y + 1) * w + x - 1, err, 3);
                }
                diffuse(&mut buf, (y + 1) * w + x, err, 5);
                if x + 1 < w {
                    diffuse(&mut buf, (y + 1) * w + x + 1, err, 1);
                }
            }
        }
    }

    bits
}

fn diffuse(buf: &mut [i16], idx: usize, err: i16, numerator: i16) {
    // Clamp to u8 range after diffusion; otherwise large
    // errors at near-black / near-white would bleed into
    // the next pixel as out-of-range values that skew
    // further comparisons.
    let updated = buf[idx].saturating_add((err * numerator) / 16);
    buf[idx] = updated.clamp(0, 255);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_white_stays_white() {
        let bits = floyd_steinberg(&[255; 64], 8, 8);
        assert!(bits.iter().all(|&b| b));
    }

    #[test]
    fn all_black_stays_black() {
        let bits = floyd_steinberg(&[0; 64], 8, 8);
        assert!(bits.iter().all(|&b| !b));
    }

    #[test]
    fn threshold_exactly_at_128() {
        // A single pixel at 128 should become white
        // (>= 128 branch).
        let bits = floyd_steinberg(&[128], 1, 1);
        assert!(bits[0]);
    }

    #[test]
    fn threshold_just_below_128_goes_black() {
        let bits = floyd_steinberg(&[127], 1, 1);
        assert!(!bits[0]);
    }

    #[test]
    fn midtone_produces_roughly_half_white() {
        // A 50% gray field should dither to ~50% white.
        // We allow a generous window because dither
        // patterns aren't perfectly balanced.
        let input = vec![128; 64 * 64];
        let bits = floyd_steinberg(&input, 64, 64);
        let whites = bits.iter().filter(|b| **b).count();
        // whites/bits.len() are ≤ 4096; f64 is exact.
        #[allow(clippy::cast_precision_loss)]
        let pct = (whites as f64) / (bits.len() as f64);
        assert!(
            (0.40..=0.60).contains(&pct),
            "expected ~50% white, got {:.2}%",
            pct * 100.0,
        );
    }

    #[test]
    fn single_row_error_diffuses_rightward() {
        // [127, 127]: first pixel rounds down (black),
        // diffuses +127*7/16 = ~55 to pixel 2, bringing
        // it from 127 to ~182 -> rounds up (white).
        let bits = floyd_steinberg(&[127, 127], 2, 1);
        assert_eq!(bits, vec![false, true]);
    }

    #[test]
    fn preserves_sharp_edges() {
        // Half black, half white. Dither shouldn't bleed
        // across the boundary noticeably.
        let mut input = Vec::new();
        for _ in 0..8 {
            input.extend(std::iter::repeat_n(0_u8, 4));
            input.extend(std::iter::repeat_n(255_u8, 4));
        }
        let bits = floyd_steinberg(&input, 8, 8);
        for row in 0..8 {
            assert!(!bits[row * 8], "row {row} col 0 should be black");
            assert!(bits[row * 8 + 7], "row {row} col 7 should be white");
        }
    }

    #[test]
    #[should_panic(expected = "grayscale length")]
    fn mismatched_length_panics() {
        let _ = floyd_steinberg(&[0; 7], 8, 1);
    }
}
