//! Third-party license texts embedded into the
//! compiled binary for runtime display.
//!
//! Shipping the bellwether binary — whether via
//! `cargo xtask deploy` to an `RPi` or any future
//! packaging route — redistributes the bundled
//! [Weather Icons] SVG bytes and the
//! [Source Sans 3] font bytes. Both are
//! [SIL OFL 1.1]-licensed; §2 of the license requires
//! each copy of the Font Software to "contain the
//! above copyright notice and this license. These
//! can be included either as stand-alone text files,
//! human-readable headers or **in the appropriate
//! machine-readable metadata fields within text or
//! binary files as long as those fields can be easily
//! viewed by the user**."
//!
//! Source-distribution compliance is satisfied by the
//! `LICENSE` files living next to each asset tree in
//! the repository. **Binary-distribution compliance**
//! is satisfied by the consts in this module plus the
//! `GET /licenses` endpoint on the web server, which
//! serves them as `text/plain` for a human user of a
//! deployed bellwether instance to view without
//! needing the source tree.
//!
//! Adding a new bundled font or icon set? Add its
//! license text here as a `&'static str` const, wire
//! it into [`ALL`] so the `/licenses` endpoint
//! surfaces it, and assert the addition at test time
//! so a future bundle that forgets the license step
//! fails at build.
//!
//! [Weather Icons]: https://erikflowers.github.io/weather-icons/
//! [Source Sans 3]: https://fonts.adobe.com/fonts/source-sans
//! [SIL OFL 1.1]: https://openfontlicense.org

/// SIL OFL 1.1 text bundled with the Weather Icons
/// SVGs under `crates/bellwether/assets/icons/weather-icons/`.
/// Served at `/licenses` so binary-only redistribution
/// still carries the license with it.
pub const WEATHER_ICONS_OFL: &str =
    include_str!("../assets/icons/weather-icons/LICENSE");

/// SIL OFL 1.1 text for the bundled Source Sans 3
/// Semibold TTF. Points at the README that accompanies
/// the font bytes in `crates/bellwether/src/render/fonts/`.
///
/// The bundled TTF's `name` table already carries the
/// copyright and license URL per the Adobe Originals
/// licence agreement, but §2 still prefers a
/// "stand-alone text file[…] easily viewed by the
/// user", so we surface the README here.
pub const SOURCE_SANS_3_OFL: &str = include_str!("render/fonts/README.md");

/// Named `(label, text)` pairs exposed by the
/// `/licenses` endpoint. Ordering is stable — keep new
/// entries at the bottom so the landing page layout
/// doesn't shift when a bundle is added.
pub const ALL: &[(&str, &str)] = &[
    (
        "Weather Icons (Erik Flowers, SIL OFL 1.1)",
        WEATHER_ICONS_OFL,
    ),
    (
        "Source Sans 3 Semibold (Adobe, SIL OFL 1.1)",
        SOURCE_SANS_3_OFL,
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundle_has_a_non_empty_license_entry() {
        // An empty or whitespace-only entry would
        // satisfy the `include_str!` macro but fail
        // the SIL OFL §2 "easily viewed by the user"
        // criterion at runtime. Catch it at build.
        for (label, text) in ALL {
            assert!(
                !text.trim().is_empty(),
                "license entry {label:?} is empty — \
                 did the file move?"
            );
        }
    }

    #[test]
    fn weather_icons_license_contains_ofl_header() {
        // A file present but lacking the actual
        // license body would be catastrophically
        // incorrect — compliance-wise this looks like
        // we bundled the license when we didn't.
        assert!(
            WEATHER_ICONS_OFL.contains("SIL OPEN FONT LICENSE"),
            "Weather Icons LICENSE file does not look \
             like an OFL text"
        );
    }
}
