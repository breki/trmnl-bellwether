//! SVG → 1-bit BMP rendering pipeline.
//!
//! 1. Parse an SVG string into a `usvg::Tree`.
//! 2. Rasterize to an RGBA pixmap at the configured
//!    resolution via `resvg` / `tiny-skia`.
//! 3. Convert RGBA to 8-bit grayscale using the Rec. 601
//!    luma coefficients, compositing transparent regions
//!    over white.
//! 4. Floyd–Steinberg dither to 1-bit.
//! 5. Encode as a monochrome BMP with the palette
//!    ordering that TRMNL OG firmware calls `"standart"`
//!    (palette[0] = black, palette[1] = white; bit 1
//!    renders white). Verified against
//!    `usetrmnl/firmware` `lib/trmnl/src/bmp.cpp` and
//!    matches `ImageMagick` / Pillow default output.
//!
//! Text rendering requires at least one font to be
//! loaded via [`Renderer::load_font_data`]. Without any
//! fonts, SVGs that contain `<text>` elements rasterize
//! with glyphs dropped.
//!
//! ## Caller responsibilities
//!
//! - Cap `svg_text.len()` at the caller. The pipeline
//!   itself has no input-size limit; a multi-megabyte
//!   SVG with deeply nested groups can starve the
//!   rasterizer. Web consumers should enforce a byte
//!   limit (~1 MiB is generous for a dashboard).
//! - Only load fonts from trusted sources. Font parsing
//!   (via `ttf-parser` / `rustybuzz`) is `#[forbid(unsafe_code)]`
//!   but adversarial fonts can still cause panics or
//!   long shape times. Do not pass in user-uploaded
//!   font blobs without sandboxing.

mod bmp;
mod dither;

#[cfg(test)]
mod tests;

use resvg::usvg;

use crate::config::{BitDepth, RenderConfig};

/// Bundled Source Sans 3 Semibold font bytes (weight
/// 600). Exposed as a `&[u8]` so callers that need the
/// raw data (tests, future alt renderers) can skip a
/// [`Renderer`] round-trip.
///
/// Source Sans 3 is Adobe's humanist sans-serif; the
/// Semibold weight has enough stroke mass to survive
/// Floyd-Steinberg dithering to 1-bit e-ink while
/// remaining legible at both display sizes (~180 px for
/// the big current temperature) and label sizes
/// (~28-36 px for wind, day labels). Distinguished from
/// the previously bundled Atkinson Hyperlegible by
/// using a dotted (non-slashed) zero. Distributed under
/// the SIL Open Font Licence (see
/// `crates/bellwether/src/render/fonts/README.md`).
///
/// Treat this as a trust-controlled bundled asset per
/// the font trust boundary documented on
/// [`Renderer::load_font_data`]; do not substitute a
/// runtime-provided blob.
///
/// Paired with [`SOURCE_SANS_3_FAMILY`] and
/// [`SOURCE_SANS_3_WEIGHT`]: callers emitting `<text>`
/// into an SVG must use those in the `font-family` and
/// `font-weight` attributes so fontdb matches this face.
pub const SOURCE_SANS_3_SEMIBOLD_TTF: &[u8] =
    include_bytes!("fonts/SourceSans3-Semibold.ttf");

/// Typographic family name advertised by
/// [`SOURCE_SANS_3_SEMIBOLD_TTF`] (name table ID 16).
/// SVG `font-family` must match this string for
/// fontdb to resolve the bundled face.
///
/// The bundled TTF is a single weight (600), so this
/// constant plus [`SOURCE_SANS_3_WEIGHT`] together
/// identify the face. Keeping them next to the bytes
/// means a future font swap touches one module rather
/// than two.
pub const SOURCE_SANS_3_FAMILY: &str = "Source Sans 3";

/// Weight (CSS numeric) of the bundled
/// [`SOURCE_SANS_3_SEMIBOLD_TTF`] face. SVG
/// `font-weight` must match so fontdb's best-match
/// lookup doesn't silently pull in a different
/// face if a second weight is ever added.
pub const SOURCE_SANS_3_WEIGHT: u16 = 600;

/// Upper bound on the SVG-to-pixmap scale factor. Any
/// SVG whose viewport is so small that scaling to the
/// target pixmap would exceed this is rejected; the
/// rasterizer can otherwise spend seconds on degenerate
/// input.
const MAX_SCALE: f32 = 8192.0;

/// Errors returned by [`Renderer::render_to_bmp`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RenderError {
    /// The SVG input failed to parse.
    ///
    /// The inner `String` is a flattened `usvg::Error`
    /// message for human consumption. Matching on parse
    /// subcategories (malformed gzip vs syntax error vs
    /// size limit) is not supported; if that becomes
    /// necessary, expose the typed error via `#[source]`
    /// rather than parsing the string.
    #[error("parsing SVG: {0}")]
    ParseSvg(String),
    /// Rasterization could not allocate a pixmap at the
    /// requested size.
    #[error("rasterization failed for {width}x{height} pixmap")]
    RasterFailed {
        /// Requested width in pixels.
        width: u32,
        /// Requested height in pixels.
        height: u32,
    },
    /// The SVG's viewport would require a scale factor
    /// outside the supported range. Triggers on crafted
    /// SVGs with near-zero or non-finite viewport
    /// dimensions.
    #[error(
        "SVG scale {scale_x}x{scale_y} outside supported \
         range (0, {MAX_SCALE}]"
    )]
    InvalidScale {
        /// Computed X scale factor.
        scale_x: f32,
        /// Computed Y scale factor.
        scale_y: f32,
    },
    /// The render config requested a bit depth the
    /// renderer doesn't implement yet. Only
    /// [`BitDepth::One`] is implemented in v1; 4-bit
    /// grayscale for the TRMNL X will follow.
    #[error(
        "unsupported bit depth {depth:?}; renderer \
         currently implements only 1-bit output"
    )]
    UnsupportedBitDepth {
        /// The rejected depth variant.
        depth: BitDepth,
    },
}

/// Server-side renderer. Holds the `usvg::Options` with
/// its font database.
///
/// Not `Clone`: usvg's `FontResolver` trait object isn't
/// `Clone`, and we'd rather not lose the loaded fonts on
/// an implicit copy. Construct once per process and pass
/// `&Renderer` around (or put it behind an `Arc`).
pub struct Renderer {
    options: usvg::Options<'static>,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Renderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Renderer")
            .field("font_count", &self.options.fontdb.len())
            .finish()
    }
}

impl Renderer {
    /// Build a renderer with an empty font database.
    /// SVGs containing `<text>` will rasterize with the
    /// glyphs missing unless [`Self::load_font_data`] is
    /// called first.
    #[must_use]
    pub fn new() -> Self {
        Self {
            options: usvg::Options::default(),
        }
    }

    /// Build a renderer with the bundled
    /// [`SOURCE_SANS_3_SEMIBOLD_TTF`] font pre-loaded.
    /// Use this in production code that needs to render
    /// dashboard text; [`Self::new`] stays available
    /// for test code that wants an empty fontdb or
    /// callers that prefer to load fonts themselves.
    #[must_use]
    pub fn with_default_fonts() -> Self {
        let mut renderer = Self::new();
        // fontdb stores the font bytes for its own
        // lifetime, so its API takes owned `Vec<u8>`
        // rather than `&[u8]` (see AQ-034). One
        // per-process copy is unavoidable; the
        // `Renderer` doc tells callers to construct
        // once and share.
        renderer.load_font_data(SOURCE_SANS_3_SEMIBOLD_TTF.to_vec());
        renderer
    }

    /// Load a font from raw bytes (TTF/OTF). Multi-face
    /// collections (TTC/OTC) are supported.
    ///
    /// Takes ownership (`Vec<u8>`) because
    /// `fontdb::Database::load_font_data` stores the
    /// bytes for the lifetime of the database without
    /// copying — passing a `&[u8]` would force an
    /// internal clone.
    ///
    /// ## Trust boundary
    ///
    /// Load only fonts from trusted sources. Font
    /// parsing libraries (`ttf-parser`, `rustybuzz`)
    /// forbid unsafe code, but malformed fonts can
    /// still cause panics or pathological shaping
    /// times. Never feed in user-uploaded font blobs
    /// unsandboxed.
    pub fn load_font_data(&mut self, data: Vec<u8>) {
        // `usvg::Options::fontdb` is an `Arc` so many
        // trees can share a font set; `make_mut` clones
        // on-write only if other owners exist (which
        // they won't unless this Renderer is ever wired
        // into an Arc graph).
        std::sync::Arc::make_mut(&mut self.options.fontdb).load_font_data(data);
    }

    /// Render the built-in placeholder layout — a
    /// geometric font-free pattern — at the configured
    /// dimensions. Intended for servers to seed their
    /// image store with *something* before the first
    /// real render completes.
    pub fn placeholder_bmp(
        &self,
        cfg: &RenderConfig,
    ) -> Result<Vec<u8>, RenderError> {
        self.render_to_bmp(include_str!("placeholder.svg"), cfg)
    }

    /// Render an SVG string to a 1-bit BMP byte vector
    /// sized according to `cfg.width` × `cfg.height`.
    pub fn render_to_bmp(
        &self,
        svg_text: &str,
        cfg: &RenderConfig,
    ) -> Result<Vec<u8>, RenderError> {
        if cfg.bit_depth != BitDepth::One {
            return Err(RenderError::UnsupportedBitDepth {
                depth: cfg.bit_depth,
            });
        }
        let tree = usvg::Tree::from_str(svg_text, &self.options)
            .map_err(|e| RenderError::ParseSvg(e.to_string()))?;

        let mut pixmap = resvg::tiny_skia::Pixmap::new(cfg.width, cfg.height)
            .ok_or(RenderError::RasterFailed {
            width: cfg.width,
            height: cfg.height,
        })?;

        // Scale the SVG's viewport to fill the pixmap.
        // Using independent X/Y factors lets us render a
        // landscape SVG at a non-matching aspect ratio
        // rather than silently letterboxing. `as f32`
        // from `u32` may lose precision above 2^24, but
        // our target resolutions are far below that
        // (TRMNL X tops out at 1872 px).
        let svg_size = tree.size();
        #[allow(clippy::cast_precision_loss)]
        let scale_x = cfg.width as f32 / svg_size.width();
        #[allow(clippy::cast_precision_loss)]
        let scale_y = cfg.height as f32 / svg_size.height();
        if !scale_x.is_finite()
            || !scale_y.is_finite()
            || scale_x <= 0.0
            || scale_y <= 0.0
            || scale_x > MAX_SCALE
            || scale_y > MAX_SCALE
        {
            return Err(RenderError::InvalidScale { scale_x, scale_y });
        }
        let transform =
            resvg::tiny_skia::Transform::from_scale(scale_x, scale_y);
        resvg::render(&tree, transform, &mut pixmap.as_mut());

        let grayscale = rgba_to_luma(pixmap.data());
        let bits = dither::floyd_steinberg(&grayscale, cfg.width, cfg.height);
        Ok(bmp::encode_1bit_bmp(&bits, cfg.width, cfg.height))
    }
}

/// Fully opaque white, used as the compositing
/// background for transparent SVG regions (so unset
/// areas render white on e-ink, not black).
const WHITE_BG: u32 = 255;

/// Convert an RGBA8 buffer (length = 4 * pixels) to an
/// 8-bit grayscale buffer using the Rec. 601 luma
/// coefficients in fixed-point: Y ≈ 0.299R + 0.587G +
/// 0.114B. Alpha is composited against white so
/// transparent regions render as white on the e-ink.
fn rgba_to_luma(rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(rgba.len() / 4);
    for chunk in rgba.chunks_exact(4) {
        let alpha = u32::from(chunk[3]);
        let inv_alpha = 255 - alpha;
        // Composite over white.
        let r = composite(chunk[0], alpha, inv_alpha);
        let g = composite(chunk[1], alpha, inv_alpha);
        let b = composite(chunk[2], alpha, inv_alpha);
        // Fixed-point Rec. 601: (77*R + 150*G + 29*B) / 256.
        // 77 + 150 + 29 = 256 so the mix keeps 0..255
        // exactly — no clamp needed.
        let y = (77 * r + 150 * g + 29 * b) / 256;
        #[allow(clippy::cast_possible_truncation)]
        out.push(y as u8);
    }
    out
}

/// Alpha-composite one channel over the [`WHITE_BG`].
fn composite(channel: u8, alpha: u32, inv_alpha: u32) -> u32 {
    (u32::from(channel) * alpha + WHITE_BG * inv_alpha) / 255
}
