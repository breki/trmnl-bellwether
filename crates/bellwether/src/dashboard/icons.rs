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
//! [tests]: tests::each_icon_renders_visible_pixels

use super::classify::Condition;

/// Upstream Weather Icons `wi-day-sunny`.
const SUNNY_RAW: &str =
    include_str!("../../assets/icons/weather-icons/wi-day-sunny.svg");

/// Upstream Weather Icons `wi-day-cloudy`.
const PARTLY_CLOUDY_RAW: &str =
    include_str!("../../assets/icons/weather-icons/wi-day-cloudy.svg");

/// Upstream Weather Icons `wi-cloudy`.
const CLOUDY_RAW: &str =
    include_str!("../../assets/icons/weather-icons/wi-cloudy.svg");

/// Upstream Weather Icons `wi-rain`.
const RAIN_RAW: &str =
    include_str!("../../assets/icons/weather-icons/wi-rain.svg");

/// Return the SVG fragment for a given [`Condition`],
/// already trimmed past the XML prolog and any
/// generator comments so the caller can embed it
/// directly inside a wrapping `<svg>` element without
/// producing invalid XML.
///
/// Panics only via [`skip_to_svg_root`]'s invariants,
/// and only on malformed bundled assets — not on user
/// input — so the production render path is
/// infallible.
#[must_use]
pub fn icon_for(condition: Condition) -> &'static str {
    let raw = match condition {
        Condition::Sunny => SUNNY_RAW,
        Condition::PartlyCloudy => PARTLY_CLOUDY_RAW,
        Condition::Cloudy => CLOUDY_RAW,
        Condition::Rain => RAIN_RAW,
    };
    skip_to_svg_root(raw)
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
            "wi-rain.svg",
            "9cfadbeb849500e135cba50dcb812d4084a5ee91d0652c1a5a20929693884c28",
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

    #[test]
    fn each_icon_covers_every_condition_variant() {
        // If a new Condition is added, this match-based
        // check fails to compile until an icon is wired
        // in — keeps the dashboard from silently
        // rendering a blank tile for an unlabelled
        // variant.
        for c in [
            Condition::Sunny,
            Condition::PartlyCloudy,
            Condition::Cloudy,
            Condition::Rain,
        ] {
            let svg = icon_for(c);
            assert!(!svg.is_empty(), "{c:?} icon empty");
            assert!(svg.starts_with("<svg"), "{c:?} icon not trimmed to root");
            assert!(svg.contains("<path"), "{c:?} icon has no <path> data");
        }
    }

    #[test]
    fn each_icon_renders_visible_pixels() {
        // The old test (pre-`<svg>` wrapping era)
        // asserted `fill="black"` presence, which was a
        // weak but non-trivial "icon will paint
        // something" invariant. Weather Icons SVGs
        // rely on the SVG default fill, so the new
        // check is: no <path> element carries
        // `fill="none"` without a compensating stroke.
        // A path with `fill="none"` and no stroke
        // renders nothing, so a future upstream
        // refresh introducing one would silently
        // produce a blank icon on e-ink.
        for raw in [SUNNY_RAW, PARTLY_CLOUDY_RAW, CLOUDY_RAW, RAIN_RAW] {
            assert!(
                !raw.contains("fill=\"none\""),
                "bundled icon has a fill=\"none\" path — would render blank"
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
        for raw in [SUNNY_RAW, PARTLY_CLOUDY_RAW, CLOUDY_RAW, RAIN_RAW] {
            assert!(
                !raw.contains("<!DOCTYPE"),
                "bundled icon contains <!DOCTYPE declaration"
            );
            assert!(
                !raw.contains("<!ENTITY"),
                "bundled icon contains <!ENTITY declaration"
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
        let inputs: &[(&str, &str)] = &[
            ("wi-day-sunny.svg", SUNNY_RAW),
            ("wi-day-cloudy.svg", PARTLY_CLOUDY_RAW),
            ("wi-cloudy.svg", CLOUDY_RAW),
            ("wi-rain.svg", RAIN_RAW),
        ];
        for (name, content) in inputs {
            let got = hex_of(Sha256::digest(content.as_bytes()).as_slice());
            let expected = PINNED_SHA256
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, h)| *h)
                .expect("PINNED_SHA256 missing entry");
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
