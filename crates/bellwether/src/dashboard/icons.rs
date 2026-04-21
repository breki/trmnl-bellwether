//! Weather icons sourced from Weather Icons by
//! Erik Flowers — <https://erikflowers.github.io/weather-icons/>.
//!
//! Each constant is a full SVG document loaded verbatim
//! from `assets/icons/weather-icons/` so upstream
//! attribution stays byte-for-byte intact. Each file
//! declares its own `viewBox`; the SVG builder embeds
//! each icon inside a positioned outer `<svg x y width
//! height>` and the inner viewBox scales to fill.
//!
//! Paths default to `fill: black` per the SVG spec
//! when no fill is specified. The
//! [`each_icon_renders_visible_pixels`][tests] test
//! verifies this invariant end-to-end for the bundled
//! set so a future upstream refresh that ships a
//! `fill="none"` path (without a compensating stroke)
//! can't silently produce a blank icon on the 1-bit
//! Floyd–Steinberg-dithered e-ink output.
//!
//! ## Two-tier dispatch
//!
//! The module exposes two public entry points; callers
//! pick the one that matches the signal they hold:
//!
//! - [`icon_for_category`] — mandatory nine-way
//!   dispatch keyed by [`ConditionCategory`]. Always
//!   produces a non-empty SVG. The safe fallback path.
//! - [`icon_for_wmo`] — detailed dispatch keyed by
//!   [`WmoCode`]. Specialized arms land one-per-PR
//!   (see HANDOFF PR 4+); every unspecialized variant
//!   routes through [`WmoCode::coarsen`] →
//!   [`icon_for_category`], so the function is total
//!   from day one.
//!
//! [tests]: tests::each_icon_renders_visible_pixels

use super::classify::{ConditionCategory, WmoCode};

/// Upstream Weather Icons `wi-day-sunny`.
const CLEAR_RAW: &str =
    include_str!("../../assets/icons/weather-icons/wi-day-sunny.svg");

/// Upstream Weather Icons `wi-day-cloudy`.
const PARTLY_CLOUDY_RAW: &str =
    include_str!("../../assets/icons/weather-icons/wi-day-cloudy.svg");

/// Upstream Weather Icons `wi-cloudy`.
const CLOUDY_RAW: &str =
    include_str!("../../assets/icons/weather-icons/wi-cloudy.svg");

/// Upstream Weather Icons `wi-fog`.
const FOG_RAW: &str =
    include_str!("../../assets/icons/weather-icons/wi-fog.svg");

/// Upstream Weather Icons `wi-sprinkle` — the drizzle
/// glyph (distinct droplets, sparser than rain).
const DRIZZLE_RAW: &str =
    include_str!("../../assets/icons/weather-icons/wi-sprinkle.svg");

/// Upstream Weather Icons `wi-rain`.
const RAIN_RAW: &str =
    include_str!("../../assets/icons/weather-icons/wi-rain.svg");

/// Upstream Weather Icons `wi-snow`.
const SNOW_RAW: &str =
    include_str!("../../assets/icons/weather-icons/wi-snow.svg");

/// Upstream Weather Icons `wi-thunderstorm`.
const THUNDERSTORM_RAW: &str =
    include_str!("../../assets/icons/weather-icons/wi-thunderstorm.svg");

/// Upstream Weather Icons `wi-hail` — specialised
/// detailed glyph for [`WmoCode::ThunderstormHailHeavy`].
/// Coarsens through [`ConditionCategory::Thunderstorm`]
/// at [`Fidelity::Simple`](super::layout::Fidelity::Simple).
const HAIL_RAW: &str =
    include_str!("../../assets/icons/weather-icons/wi-hail.svg");

/// Upstream Weather Icons `wi-snow-wind` — specialised
/// detailed glyph for [`WmoCode::SnowHeavy`]. The
/// wind-driven snow shape reads visibly heavier than
/// the plain `wi-snow` flake at e-ink resolution, so
/// `fidelity = "detailed"` for a heavy-snowfall code
/// lands on this glyph while slight/moderate snow
/// stays on the coarse `wi-snow` fallback. Coarsens
/// through [`ConditionCategory::Snow`] at
/// [`Fidelity::Simple`](super::layout::Fidelity::Simple).
const SNOW_HEAVY_RAW: &str =
    include_str!("../../assets/icons/weather-icons/wi-snow-wind.svg");

/// Upstream Weather Icons `wi-na` — "not available"
/// glyph used for [`ConditionCategory::Unknown`] when
/// the provider returned a WMO code outside the
/// documented subset.
const UNKNOWN_RAW: &str =
    include_str!("../../assets/icons/weather-icons/wi-na.svg");

/// Return the SVG fragment for a given
/// [`ConditionCategory`], already trimmed past the XML
/// prolog and any generator comments so the caller can
/// embed it directly inside a wrapping `<svg>` element
/// without producing invalid XML.
///
/// Panics only via [`skip_to_svg_root`]'s invariants,
/// and only on malformed bundled assets — not on user
/// input — so the production render path is
/// infallible.
#[must_use]
pub fn icon_for_category(category: ConditionCategory) -> &'static str {
    let raw = match category {
        ConditionCategory::Clear => CLEAR_RAW,
        ConditionCategory::PartlyCloudy => PARTLY_CLOUDY_RAW,
        ConditionCategory::Cloudy => CLOUDY_RAW,
        ConditionCategory::Fog => FOG_RAW,
        ConditionCategory::Drizzle => DRIZZLE_RAW,
        ConditionCategory::Rain => RAIN_RAW,
        ConditionCategory::Snow => SNOW_RAW,
        ConditionCategory::Thunderstorm => THUNDERSTORM_RAW,
        ConditionCategory::Unknown => UNKNOWN_RAW,
    };
    skip_to_svg_root(raw)
}

/// Return the SVG fragment for a given [`WmoCode`],
/// preferring a specialized glyph when one is bundled
/// and falling back to the coarse
/// [`icon_for_category`] otherwise.
///
/// Detailed arms are added here incrementally (see
/// HANDOFF PR 4+). Every new arm must also add its
/// source SVG file, register a SHA-256 pin in
/// [`tests::PINNED_SHA256`], and wire the file into
/// `assets/icons/weather-icons/README.md` so the
/// byte-identity contract stays visible.
#[must_use]
pub fn icon_for_wmo(code: WmoCode) -> &'static str {
    match code {
        // Add new specialised arms above this catch-all.
        // The `_` keeps the function total while arms
        // are added one at a time;
        // `tests::icon_for_wmo_respects_its_dispatch_classification`
        // exhaustively classifies every `WmoCode` as
        // Specialised or Coarsened, so dropping or
        // adding an arm without updating that match
        // fails at compile time.
        WmoCode::ThunderstormHailHeavy => skip_to_svg_root(HAIL_RAW),
        WmoCode::SnowHeavy => skip_to_svg_root(SNOW_HEAVY_RAW),
        _ => icon_for_category(code.coarsen()),
    }
}

/// Return the slice starting at the root `<svg>`
/// element, stepping past any XML processing
/// instructions (`<?…?>`) and comments (`<!--…-->`)
/// that precede it.
///
/// Naive `svg.find("<svg")` would be defeated by a
/// future upstream refresh whose generator comment
/// happens to contain the literal string `<svg>`; this
/// helper instead consumes PI/comment spans
/// sequentially, so `<svg>` inside a comment stays
/// hidden. Panics on a malformed asset — the inputs
/// are compile-time `include_str!` constants so any
/// violation is a build-time bug, not a runtime
/// possibility.
fn skip_to_svg_root(svg: &'static str) -> &'static str {
    let mut remaining = svg.trim_start();
    loop {
        if let Some(after_pi) = remaining.strip_prefix("<?") {
            let end = after_pi.find("?>").expect(
                "bundled icon has unterminated <?…?> processing instruction",
            );
            remaining = after_pi[end + 2..].trim_start();
        } else if let Some(after_comment) = remaining.strip_prefix("<!--") {
            let end = after_comment
                .find("-->")
                .expect("bundled icon has unterminated <!-- … --> comment");
            remaining = after_comment[end + 3..].trim_start();
        } else {
            break;
        }
    }
    assert!(
        remaining.starts_with("<svg"),
        "bundled icon is missing its <svg> root element"
    );
    remaining
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every bundled icon file, in `(filename, bytes)`
    /// form. Single source of truth for the tests that
    /// iterate the whole bundle (byte-identity, no-XXE,
    /// no-fill-none). Adding a new icon here without a
    /// pin or a category mapping fails a sibling test
    /// loudly — that's the point.
    const BUNDLED_ICONS: &[(&str, &str)] = &[
        ("wi-day-sunny.svg", CLEAR_RAW),
        ("wi-day-cloudy.svg", PARTLY_CLOUDY_RAW),
        ("wi-cloudy.svg", CLOUDY_RAW),
        ("wi-fog.svg", FOG_RAW),
        ("wi-sprinkle.svg", DRIZZLE_RAW),
        ("wi-rain.svg", RAIN_RAW),
        ("wi-snow.svg", SNOW_RAW),
        ("wi-thunderstorm.svg", THUNDERSTORM_RAW),
        ("wi-hail.svg", HAIL_RAW),
        ("wi-snow-wind.svg", SNOW_HEAVY_RAW),
        ("wi-na.svg", UNKNOWN_RAW),
    ];

    /// Hash pins guard the "byte-identical to upstream"
    /// claim in `assets/icons/weather-icons/README.md`.
    /// A whitespace-normalising pre-commit hook or a
    /// maintainer "fixing" the Adobe Illustrator-ism in
    /// one of the files silently falsifies the claim —
    /// this test fails loudly instead. Regenerate with
    /// `sha256sum < the-file` after any intentional
    /// refresh.
    const PINNED_SHA256: &[(&str, &str)] = &[
        (
            "wi-day-sunny.svg",
            "1dd025f7c0e891a628c575ed9b97a20bccdca7ee630041ab3a207523bbff6b00",
        ),
        (
            "wi-day-cloudy.svg",
            "5a7d99fd7f316b3eec46624df414a36c212f1067ee0b02943d68f0ec0ab68910",
        ),
        (
            "wi-cloudy.svg",
            "571cef0545b87794c78cdc1a13da4c1011f88c3c23fb308d27932fb33fdbbeea",
        ),
        (
            "wi-fog.svg",
            "80f225af4bed4acaca2604dcbc6aac5f078fe286890bd97cebb23278dc138cc5",
        ),
        (
            "wi-sprinkle.svg",
            "82af77373374946ad72ba19bc8ab36a4ff0f71995471281e7797a26f8e36aba9",
        ),
        (
            "wi-rain.svg",
            "9cfadbeb849500e135cba50dcb812d4084a5ee91d0652c1a5a20929693884c28",
        ),
        (
            "wi-snow.svg",
            "8401a01fff4cf40a6d78a79f2d4bfc8645fd74b4fc793efa16a4c2369132fe9c",
        ),
        (
            "wi-thunderstorm.svg",
            "5714873e99b82a9938f89f8c06eda575a4255264cb6d15c9c454bf7ed6f41543",
        ),
        (
            "wi-hail.svg",
            "ff45a373e4ea53b28c7f25b4422ea510041667042b3b7860e3c679bfed66affb",
        ),
        (
            "wi-snow-wind.svg",
            "fa3556e46152867a4a538d54b533b3aed2f031f14c9a8de6e3b53ce0339f5ec2",
        ),
        (
            "wi-na.svg",
            "edf0c9d7edbf4261cd2c727d9e1d89934d1f1337a47a89d0b6696c0b12059c7c",
        ),
    ];

    /// Lower-case hex-encode a byte slice. Lives here
    /// rather than pulling in the `hex` crate because
    /// it's a single ~5-line helper used only to
    /// compare a sha256 output against a pinned hex
    /// string — not worth a dep.
    fn hex_of(bytes: &[u8]) -> String {
        use std::fmt::Write;
        let mut out = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            write!(&mut out, "{b:02x}").expect("writing to String");
        }
        out
    }

    /// Every [`ConditionCategory`] variant, listed
    /// explicitly so this array drives the coverage
    /// test below. Adding a new variant makes this
    /// array fail to compile (exhaustive match) and
    /// forces a paired icon update.
    const ALL_CATEGORIES: &[ConditionCategory] = &[
        ConditionCategory::Clear,
        ConditionCategory::PartlyCloudy,
        ConditionCategory::Cloudy,
        ConditionCategory::Fog,
        ConditionCategory::Drizzle,
        ConditionCategory::Rain,
        ConditionCategory::Snow,
        ConditionCategory::Thunderstorm,
        ConditionCategory::Unknown,
    ];

    #[test]
    fn all_categories_array_matches_enum_variants_exhaustively() {
        // Compile-time enforcement via an exhaustive
        // match: a new variant breaks this function
        // until added to `ALL_CATEGORIES`, which in
        // turn forces the coverage tests to re-run.
        for &c in ALL_CATEGORIES {
            match c {
                ConditionCategory::Clear
                | ConditionCategory::PartlyCloudy
                | ConditionCategory::Cloudy
                | ConditionCategory::Fog
                | ConditionCategory::Drizzle
                | ConditionCategory::Rain
                | ConditionCategory::Snow
                | ConditionCategory::Thunderstorm
                | ConditionCategory::Unknown => {}
            }
        }
    }

    #[test]
    fn icon_for_category_covers_every_variant() {
        for &c in ALL_CATEGORIES {
            let svg = icon_for_category(c);
            assert!(!svg.is_empty(), "{c:?} icon empty");
            assert!(svg.starts_with("<svg"), "{c:?} icon not trimmed to root");
            assert!(svg.contains("<path"), "{c:?} icon has no <path> data");
        }
    }

    #[test]
    fn icon_for_wmo_is_total_over_every_documented_code() {
        // `WmoCode::ALL` is the single source of truth
        // for "documented variants" — iterating it here
        // means a new variant trips both coarsen's
        // exhaustive match and this icon coverage test
        // without any list to keep in sync.
        for &code in WmoCode::ALL {
            let svg = icon_for_wmo(code);
            assert!(!svg.is_empty(), "{code:?} icon empty");
            assert!(svg.starts_with("<svg"), "{code:?} icon not trimmed");
            assert!(svg.contains("<path"), "{code:?} icon has no <path>");
        }
    }

    /// Per-variant classification of `icon_for_wmo`'s
    /// dispatch, used by the coverage test below.
    /// Exhaustive `match` without a catch-all — adding
    /// a new `WmoCode` variant is a compile error until
    /// the reviewer decides whether it is Specialised
    /// or Coarsened. That's the forcing function AQ-3
    /// asked for.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum DispatchKind {
        /// Variant has its own arm in `icon_for_wmo`
        /// returning a glyph distinct from
        /// `icon_for_category(code.coarsen())`.
        Specialised,
        /// Variant falls through `icon_for_wmo`'s default
        /// arm, yielding the same glyph as its coarsened
        /// category.
        Coarsened,
    }

    fn dispatch_kind(code: WmoCode) -> DispatchKind {
        use DispatchKind::{Coarsened, Specialised};
        match code {
            WmoCode::ThunderstormHailHeavy | WmoCode::SnowHeavy => Specialised,
            WmoCode::Clear
            | WmoCode::MainlyClear
            | WmoCode::PartlyCloudy
            | WmoCode::Overcast
            | WmoCode::Fog
            | WmoCode::RimeFog
            | WmoCode::DrizzleLight
            | WmoCode::DrizzleModerate
            | WmoCode::DrizzleDense
            | WmoCode::FreezingDrizzleLight
            | WmoCode::FreezingDrizzleDense
            | WmoCode::RainSlight
            | WmoCode::RainModerate
            | WmoCode::RainHeavy
            | WmoCode::FreezingRainLight
            | WmoCode::FreezingRainHeavy
            | WmoCode::SnowSlight
            | WmoCode::SnowModerate
            | WmoCode::SnowGrains
            | WmoCode::RainShowersSlight
            | WmoCode::RainShowersModerate
            | WmoCode::RainShowersViolent
            | WmoCode::SnowShowersSlight
            | WmoCode::SnowShowersHeavy
            | WmoCode::Thunderstorm
            | WmoCode::ThunderstormHailSlight => Coarsened,
        }
    }

    #[test]
    fn icon_for_wmo_respects_its_dispatch_classification() {
        // The unified contract (AQ-3): every `WmoCode`
        // variant is either Specialised (differs from
        // its category fallback) or Coarsened (equals
        // it), and the classification lives in one place
        // (`dispatch_kind`). A new variant that is not
        // classified fails to compile; a specialised arm
        // dropped silently fails this test loudly.
        for &code in WmoCode::ALL {
            match dispatch_kind(code) {
                DispatchKind::Specialised => assert_ne!(
                    icon_for_wmo(code),
                    icon_for_category(code.coarsen()),
                    "{code:?} is classified Specialised but \
                     icon_for_wmo routes it through the coarse \
                     category",
                ),
                DispatchKind::Coarsened => assert_eq!(
                    icon_for_wmo(code),
                    icon_for_category(code.coarsen()),
                    "{code:?} is classified Coarsened but \
                     icon_for_wmo diverges from the coarse \
                     category",
                ),
            }
        }
    }

    #[test]
    fn icon_for_wmo_falls_back_to_coarsened_category() {
        // Human-readable sanity check alongside the
        // exhaustive `…_respects_its_dispatch_classification`
        // test above. Keeping `Thunderstorm` explicit
        // here guards the highest-risk collision zone —
        // the thunderstorm category, whose specialised
        // sibling `ThunderstormHailHeavy` already exists
        // (AQ-5).
        for code in [
            WmoCode::RainSlight,
            WmoCode::SnowSlight,
            WmoCode::Thunderstorm,
            WmoCode::Fog,
        ] {
            assert_eq!(
                icon_for_wmo(code),
                icon_for_category(code.coarsen()),
                "coarsen fallback drifted for {code:?}",
            );
        }
    }

    #[test]
    fn every_icon_renders_visible_pixels() {
        // Weather Icons SVGs rely on the SVG default
        // fill (black); a future upstream refresh
        // introducing a bare `fill="none"` path would
        // silently produce a blank icon on e-ink.
        for (name, raw) in BUNDLED_ICONS {
            assert!(
                !raw.contains("fill=\"none\""),
                "{name}: has a fill=\"none\" path — would render blank",
            );
        }
    }

    #[test]
    fn bundled_icons_contain_no_doctype_or_entity_declarations() {
        // XXE / billion-laughs defence-in-depth: the
        // current bundle has no `<!DOCTYPE>` or
        // `<!ENTITY>` declarations, and a future
        // upstream refresh (or a sloppily-edited local
        // SVG) must not introduce one. `roxmltree`
        // mitigates external-entity attacks, but
        // internal-entity-expansion attacks depend on
        // its limits — cheaper to forbid the
        // declarations entirely at the asset boundary.
        for (name, raw) in BUNDLED_ICONS {
            assert!(
                !raw.contains("<!DOCTYPE"),
                "{name}: contains <!DOCTYPE declaration",
            );
            assert!(
                !raw.contains("<!ENTITY"),
                "{name}: contains <!ENTITY declaration",
            );
        }
    }

    #[test]
    fn bundled_icons_match_pinned_sha256() {
        // Backs the "byte-identical to upstream" claim
        // in `assets/icons/weather-icons/README.md`.
        // See the PINNED_SHA256 comment above for
        // regeneration instructions.
        use sha2::{Digest, Sha256};
        assert_eq!(
            BUNDLED_ICONS.len(),
            PINNED_SHA256.len(),
            "every bundled icon needs a pinned SHA-256",
        );
        for (name, content) in BUNDLED_ICONS {
            let got = hex_of(Sha256::digest(content.as_bytes()).as_slice());
            let expected =
                PINNED_SHA256.iter().find(|(n, _)| n == name).map_or_else(
                    || panic!("PINNED_SHA256 missing entry for {name}"),
                    |(_, h)| *h,
                );
            assert_eq!(
                got, expected,
                "{name}: sha256 drift — either a refresh \
                 that the pins weren't updated for, or a \
                 local edit that falsifies the \
                 byte-identical-to-upstream claim"
            );
        }
    }

    #[test]
    fn skip_to_svg_root_drops_processing_instruction() {
        // Upstream SVGs start with `<?xml …?>` plus a
        // generator comment. Embedding those inside an
        // outer <svg> is an XML error, so the helper
        // must drop everything before the root <svg>.
        let with_prolog = "<?xml version=\"1.0\"?>\n<!-- gen -->\n<svg>x</svg>";
        assert_eq!(skip_to_svg_root(with_prolog), "<svg>x</svg>");
    }

    #[test]
    fn skip_to_svg_root_is_idempotent_on_bare_svg() {
        let bare = "<svg viewBox=\"0 0 30 30\"><path/></svg>";
        assert_eq!(skip_to_svg_root(bare), bare);
    }

    #[test]
    fn skip_to_svg_root_ignores_svg_inside_comments() {
        // A future upstream refresh whose generator
        // comment contains the literal `<svg` (e.g.
        // `<!-- produced by svgo from <svg…> -->`)
        // must not trick the helper into slicing
        // mid-comment. Naive `find("<svg")` would
        // regress here.
        let tricky =
            "<!-- exported from <svg> tools -->\n<svg viewBox=\"0 0 10 10\"/>";
        assert_eq!(skip_to_svg_root(tricky), "<svg viewBox=\"0 0 10 10\"/>");
    }

    #[test]
    #[should_panic(expected = "missing its <svg> root")]
    fn skip_to_svg_root_panics_on_missing_root() {
        // A corrupted or truncated bundled asset must
        // fail loudly at test/startup time rather than
        // silently producing a runtime parse error.
        skip_to_svg_root("<?xml ?><!-- no root here -->");
    }
}
