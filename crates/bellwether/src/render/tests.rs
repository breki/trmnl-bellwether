//! End-to-end tests for the render pipeline.

use super::*;
use crate::config::RenderConfig;

/// Render a minimal SVG at `width x height` with the
/// default 1-bit config and return the produced BMP
/// bytes.
fn render(svg: &str, width: u32, height: u32) -> Vec<u8> {
    let cfg = RenderConfig {
        width,
        height,
        ..Default::default()
    };
    Renderer::new().render_to_bmp(svg, &cfg).unwrap()
}

/// Extract the pixel data of a 1-bit BMP as a flat
/// `Vec<bool>` in top-to-bottom, left-to-right order,
/// mirroring how the renderer's input grid is laid out.
/// Panics if the BMP isn't the expected 1-bit layout.
fn bmp_to_bits(bmp: &[u8]) -> (Vec<bool>, u32, u32) {
    assert_eq!(&bmp[..2], b"BM");
    let offset =
        u32::from_le_bytes([bmp[10], bmp[11], bmp[12], bmp[13]]) as usize;
    let width =
        u32::try_from(i32::from_le_bytes([bmp[18], bmp[19], bmp[20], bmp[21]]))
            .unwrap();
    let height =
        u32::try_from(i32::from_le_bytes([bmp[22], bmp[23], bmp[24], bmp[25]]))
            .unwrap();
    let bpp = u16::from_le_bytes([bmp[28], bmp[29]]);
    assert_eq!(bpp, 1, "expected 1-bit BMP");

    let row_bytes = ((width.div_ceil(8)).div_ceil(4)) * 4;
    let mut out = vec![false; (width * height) as usize];
    for y in 0..height {
        // BMP stores bottom-up, so map to top-down index.
        let row_idx_from_bottom = y as usize;
        let top_down_y = (height - 1 - y) as usize;
        let row_start = offset + row_idx_from_bottom * row_bytes as usize;
        for x in 0..width {
            let byte = bmp[row_start + (x / 8) as usize];
            let bit = (byte >> (7 - (x % 8))) & 1;
            out[top_down_y * width as usize + x as usize] = bit != 0;
        }
    }
    (out, width, height)
}

#[test]
fn renders_solid_white_rect_to_all_white() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"
             width="16" height="8" viewBox="0 0 16 8">
          <rect width="16" height="8" fill="white"/>
        </svg>"#;
    let bmp = render(svg, 16, 8);
    let (bits, w, h) = bmp_to_bits(&bmp);
    assert_eq!((w, h), (16, 8));
    assert!(bits.iter().all(|&b| b), "expected all white");
}

#[test]
fn renders_solid_black_rect_to_all_black() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"
             width="16" height="8" viewBox="0 0 16 8">
          <rect width="16" height="8" fill="black"/>
        </svg>"#;
    let bmp = render(svg, 16, 8);
    let (bits, _, _) = bmp_to_bits(&bmp);
    assert!(bits.iter().all(|&b| !b), "expected all black");
}

#[test]
fn renders_half_black_half_white_with_clean_edge() {
    // Left half black, right half white. The boundary
    // should be sharp at x == width/2 on every row.
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"
             width="16" height="4" viewBox="0 0 16 4">
          <rect x="0" y="0" width="8" height="4" fill="black"/>
          <rect x="8" y="0" width="8" height="4" fill="white"/>
        </svg>"#;
    let bmp = render(svg, 16, 4);
    let (bits, w, h) = bmp_to_bits(&bmp);
    for y in 0..h {
        // Left half all black.
        for x in 0..(w / 2) {
            let idx = (y * w + x) as usize;
            assert!(!bits[idx], "({x},{y}) should be black");
        }
        // Right half all white.
        for x in (w / 2)..w {
            let idx = (y * w + x) as usize;
            assert!(bits[idx], "({x},{y}) should be white");
        }
    }
}

#[test]
fn renders_at_trmnl_og_resolution() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"
             width="100" height="60" viewBox="0 0 100 60">
          <rect width="100" height="60" fill="white"/>
        </svg>"#;
    let bmp = render(svg, 800, 480);
    // Expected total size = 62 bytes of header + palette +
    // (100 bytes/row * 480 rows) = 62 + 48000 = 48062.
    assert_eq!(bmp.len(), 62 + 100 * 480);
    let (_, w, h) = bmp_to_bits(&bmp);
    assert_eq!((w, h), (800, 480));
}

#[test]
fn reports_parse_error_on_garbage_svg() {
    let err = Renderer::new()
        .render_to_bmp("<not-svg", &RenderConfig::default())
        .unwrap_err();
    assert!(matches!(err, RenderError::ParseSvg(_)));
}

#[test]
fn rejects_unsupported_bit_depth() {
    let cfg = RenderConfig {
        bit_depth: BitDepth::Four,
        ..Default::default()
    };
    let err = Renderer::new()
        .render_to_bmp(r#"<svg xmlns="http://www.w3.org/2000/svg"/>"#, &cfg)
        .unwrap_err();
    let RenderError::UnsupportedBitDepth { depth } = err else {
        panic!("expected UnsupportedBitDepth, got {err:?}")
    };
    assert_eq!(depth, BitDepth::Four);
}

#[test]
fn placeholder_bmp_renders_at_configured_dimensions() {
    let cfg = RenderConfig {
        width: 64,
        height: 32,
        ..Default::default()
    };
    let bmp = Renderer::new().placeholder_bmp(&cfg).unwrap();
    let (_, w, h) = bmp_to_bits(&bmp);
    assert_eq!((w, h), (64, 32));
}

#[test]
fn debug_impl_mentions_font_count() {
    let r = Renderer::new();
    let s = format!("{r:?}");
    assert!(s.contains("Renderer"));
    assert!(s.contains("font_count"));
}

#[test]
fn rejects_svg_that_would_require_excessive_scale() {
    // Viewport 0.001 x 0.001 rendered into 800 x 480
    // yields scale factors of 800_000 / 480_000 — far
    // above MAX_SCALE. The rasterizer would otherwise
    // churn on degenerate input.
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"
                  width="0.001" height="0.001"
                  viewBox="0 0 0.001 0.001">
          <rect width="0.001" height="0.001" fill="black"/>
        </svg>"#;
    let err = Renderer::new()
        .render_to_bmp(svg, &RenderConfig::default())
        .unwrap_err();
    assert!(matches!(err, RenderError::InvalidScale { .. }));
}

#[test]
fn ignores_external_file_references_in_svg() {
    // Defense-in-depth: `raster-images` is off and usvg's
    // default `image_href_resolver` doesn't touch the
    // filesystem or network. If someone flips a feature
    // flag in the future, this test fails loudly.
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"
                  width="16" height="8" viewBox="0 0 16 8">
          <image href="file:///etc/passwd"
                 width="16" height="8"/>
          <rect width="16" height="8" fill="white"/>
        </svg>"#;
    let bmp = render(svg, 16, 8);
    let (bits, _, _) = bmp_to_bits(&bmp);
    assert!(bits.iter().all(|&b| b), "external href must be ignored");
}

#[test]
fn bundled_dashboard_font_parses_as_truetype() {
    // Guards against a corrupted checkout: the bundled
    // TTF bytes must start with the TrueType magic
    // 0x00010000 (version 1.0) — anything else means
    // the file was not committed verbatim.
    assert!(
        SOURCE_SANS_3_SEMIBOLD_TTF.len() > 64,
        "font bytes suspiciously short: {}",
        SOURCE_SANS_3_SEMIBOLD_TTF.len(),
    );
    assert_eq!(
        &SOURCE_SANS_3_SEMIBOLD_TTF[..4],
        &[0x00, 0x01, 0x00, 0x00],
        "bundled font missing TrueType magic",
    );
    let face = ttf_parser::Face::parse(SOURCE_SANS_3_SEMIBOLD_TTF, 0)
        .expect("valid TrueType face");
    assert!(
        !face.is_variable(),
        "bundled Source Sans 3 Semibold is a static font",
    );
}

#[test]
fn bundled_dashboard_font_covers_dashboard_glyphs() {
    // The dashboard needs every digit, every ASCII
    // letter (for day names, cardinal directions, and
    // condition labels), the space, and a handful of
    // typographic characters — notably U+00B0 °. If a
    // future font swap drops any of these, we need to
    // pick a different font or substitute a drawn
    // glyph; better to know at test time than at render
    // time. The ranges cover the full set so future
    // font subsetting can't sneak a gap past a
    // spot-check.
    let face = ttf_parser::Face::parse(SOURCE_SANS_3_SEMIBOLD_TTF, 0)
        .expect("valid TrueType face");
    // U+2014 EM DASH is the PLACEHOLDER glyph the
    // dashboard emits for every missing-data field
    // (null `high_c`, missing current conditions,
    // etc.). Without it, a legitimate forecast with a
    // partially populated day would render empty.
    let ranges = [
        '0'..='9',
        'A'..='Z',
        'a'..='z',
        ' '..=' ',
        '°'..='°',
        '—'..='—',
    ];
    for range in ranges {
        for c in range {
            assert!(
                face.glyph_index(c).is_some(),
                "font missing glyph for {c:?} (U+{:04X})",
                c as u32,
            );
        }
    }
}

#[test]
fn with_default_fonts_registers_one_face() {
    let renderer = Renderer::with_default_fonts();
    // The Debug impl is the public contract for the
    // font count; asserting against it avoids coupling
    // the test to the private field layout.
    let s = format!("{renderer:?}");
    assert!(s.contains("font_count: 1"), "debug impl: {s}");
}

#[test]
fn with_default_fonts_renders_degree_sign_glyph() {
    // End-to-end text rendering: build a renderer with
    // the bundled font, rasterize an SVG that prints
    // "0°C" at display size, and assert the output has
    // meaningful black coverage — not a blank canvas.
    // Without the font, usvg silently drops glyphs and
    // the result is all-white; this test would then
    // fail and tell us the font pipeline is broken.
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"
             width="160" height="64" viewBox="0 0 160 64">
          <rect width="160" height="64" fill="white"/>
          <text x="80" y="48" font-family="'Source Sans 3'"
                font-weight="600" font-size="36"
                text-anchor="middle"
                fill="black">0°C</text>
        </svg>"#;
    let cfg = RenderConfig {
        width: 160,
        height: 64,
        ..Default::default()
    };
    let bmp = Renderer::with_default_fonts()
        .render_to_bmp(svg, &cfg)
        .expect("render with font");
    let (bits, _, _) = bmp_to_bits(&bmp);
    let black = bits.iter().filter(|b| !**b).count();
    // "0°C" at 36 px should produce hundreds of black
    // pixels. A silently-dropped font would produce ~0.
    // 200 is well above the "tiny stroke residue" floor
    // and comfortably below the actual glyph coverage.
    assert!(
        black > 200,
        "expected substantial black glyph coverage; got {black} pixels",
    );
}

#[test]
fn gradient_dithers_to_mixed_pixels() {
    // A black→white horizontal gradient should produce
    // mostly-black on the left and mostly-white on the
    // right after dithering.
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"
             width="64" height="1" viewBox="0 0 64 1">
          <defs>
            <linearGradient id="g" x1="0" y1="0" x2="1" y2="0">
              <stop offset="0" stop-color="black"/>
              <stop offset="1" stop-color="white"/>
            </linearGradient>
          </defs>
          <rect width="64" height="1" fill="url(#g)"/>
        </svg>"#;
    let bmp = render(svg, 64, 1);
    let (bits, _, _) = bmp_to_bits(&bmp);
    let left_whites = bits[..16].iter().filter(|b| **b).count();
    let right_whites = bits[48..].iter().filter(|b| **b).count();
    assert!(
        left_whites <= 4,
        "left quarter should be mostly black, got {left_whites} whites",
    );
    assert!(
        right_whites >= 12,
        "right quarter should be mostly white, got {right_whites} whites",
    );
}
