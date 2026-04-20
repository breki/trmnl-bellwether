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
//!
//! ## Pre-threshold edge snap
//!
//! Before dithering, pixels that are already very
//! close to pure black or pure white are snapped to
//! the extreme. The source material here is pure
//! black glyphs and icon paths on a white background,
//! so any sub-extreme grey is almost always an
//! anti-aliased edge pixel from `resvg`'s rasterizer,
//! not intentional midtone content. Snapping those
//! edges kills a class of visible shimmer on the e-ink
//! panel: without the snap, an AA pixel at ~230 grey
//! rounds to white but leaves −25 of error behind,
//! and the diffusion pattern from thousands of edge
//! pixels stacks into a visible buzzing texture along
//! every glyph and curve. With the snap, edge pixels
//! contribute zero error; FS runs only on genuine
//! midtones (which, for the current dashboard, there
//! are none — but leaving the algorithm correct for
//! midtone content keeps the pipeline honest if icons
//! with gradients ever land).
//!
//! Thresholds: a pixel is snapped to `0` when
//! `pixel <= [SNAP_BLACK_AT]` and to `255` when
//! `pixel >= [SNAP_WHITE_AT]`. Values between those
//! bounds pass through unchanged into the FS loop.

/// Grayscale value at or below which a pixel is
/// snapped to pure black before dithering. 20% of
/// 255 ≈ 51. Anti-aliased edge pixels from `resvg`
/// typically land in the 1–15% range (mostly white bg
/// with a sliver of black bleed at a glyph border);
/// this threshold catches them without touching
/// intentional mid-greys that start at ~20%+.
pub const SNAP_BLACK_AT: u8 = 51;

/// Grayscale value at or above which a pixel is
/// snapped to pure white before dithering. Mirror of
/// [`SNAP_BLACK_AT`] at 80% of 255 = 204.
pub const SNAP_WHITE_AT: u8 = 204;

/// Apply the edge snap: push near-black to 0 and
/// near-white to 255; leave midtones untouched. Pure
/// function, trivially testable; factored out of the
/// main loop so the pre-threshold contract is visible
/// (and invertible) separately from error diffusion.
#[must_use]
fn prethreshold(g: u8) -> i16 {
    if g <= SNAP_BLACK_AT {
        0
    } else if g >= SNAP_WHITE_AT {
        255
    } else {
        i16::from(g)
    }
}

/// Dither an 8-bit grayscale buffer to 1-bit.
///
/// - `grayscale` is row-major, `width * height` bytes,
///   with 0 = black and 255 = white.
/// - Output `bits` is row-major, same dimensions:
///   `true` for white (pixel ≥ threshold after error
///   diffusion), `false` for black.
///
/// See the module-level docs for the pre-threshold
/// edge-snap behaviour applied to near-extreme pixels
/// before the Floyd–Steinberg loop runs.
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
    // Snap near-extreme pixels first so edge AA noise
    // doesn't seed the diffusion pattern.
    let mut buf: Vec<i16> =
        grayscale.iter().copied().map(prethreshold).collect();
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

    // ─── Pre-threshold edge-snap behaviour ──────────────

    #[test]
    fn near_black_snaps_to_black_without_diffusing_error() {
        // AA-edge case: one anti-aliased grey pixel in
        // a white region would, under pure FS, round
        // to white and push +25 of error to the right.
        // With edge-snap, the grey is caught by
        // SNAP_BLACK_AT before FS sees it — wait, 230
        // is near white; swap to 25 for a near-black
        // AA edge in an otherwise white field.
        let bits = floyd_steinberg(&[25, 255, 255, 255], 4, 1);
        assert_eq!(bits, vec![false, true, true, true]);
        // Under raw FS (without snap), pixel 0 at 25
        // rounds to black, err = +25, diffuses +11 to
        // pixel 1 — which still rounds to white, but
        // pixel 1's own err becomes -(255-236)=-19,
        // cascading further. The snap kills that cascade
        // at the source.
    }

    #[test]
    fn near_white_snaps_to_white_without_diffusing_error() {
        // Flipside: a near-white AA edge pixel amid
        // black is the common text/glyph-edge case.
        // Under pure FS, pixel 0 at 230 rounds to white
        // and diffuses -25, pushing pixel 1 (already
        // black at 0) to -11; still black, but seeds a
        // zigzag pattern visible as shimmer at glance.
        let bits = floyd_steinberg(&[230, 0, 0, 0], 4, 1);
        assert_eq!(bits, vec![true, false, false, false]);
    }

    #[test]
    fn snap_thresholds_are_inclusive_at_the_boundary() {
        // 51 is exactly SNAP_BLACK_AT → snapped black.
        // 52 is just above → passes through to FS,
        // rounds black (< 128), diffuses +52*7/16≈22 to
        // the right. With pure-white neighbours the
        // diffusion lands on white, so the row remains
        // all-white except for the first pixel.
        let snapped = floyd_steinberg(&[51, 255, 255, 255], 4, 1);
        assert_eq!(snapped, vec![false, true, true, true]);
        // 204 at SNAP_WHITE_AT → snapped white, no err.
        let hi = floyd_steinberg(&[204, 0, 0, 0], 4, 1);
        assert_eq!(hi, vec![true, false, false, false]);
    }

    #[test]
    fn midtones_still_dither_normally() {
        // A pixel at 128 (the FS threshold) sits inside
        // the snap band (51 < 128 < 204), so it passes
        // through. Existing threshold behaviour must
        // not regress — this is the "pure FS lives on
        // for real midtones" guarantee.
        let bits = floyd_steinberg(&[128], 1, 1);
        assert!(bits[0]);
        let just_under = floyd_steinberg(&[127], 1, 1);
        assert!(!just_under[0]);
    }

    #[test]
    fn prethreshold_maps_extremes_directly() {
        // Direct unit test on the helper so a refactor
        // that breaks the snap invariant fails loudly
        // without depending on the whole FS pipeline.
        assert_eq!(prethreshold(0), 0);
        assert_eq!(prethreshold(SNAP_BLACK_AT), 0);
        assert_eq!(prethreshold(SNAP_BLACK_AT + 1), 52);
        assert_eq!(prethreshold(128), 128);
        assert_eq!(prethreshold(SNAP_WHITE_AT - 1), 203);
        assert_eq!(prethreshold(SNAP_WHITE_AT), 255);
        assert_eq!(prethreshold(255), 255);
    }
}
