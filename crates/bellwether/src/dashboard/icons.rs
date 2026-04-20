//! Weather icons sourced from Weather Icons by
//! Erik Flowers — <https://erikflowers.github.io/weather-icons/>.
//!
//! Each constant is a full SVG document (`<svg
//! viewBox="0 0 30 30">…`) loaded verbatim from
//! `assets/icons/weather-icons/` so upstream
//! attribution stays byte-for-byte intact.
//!
//! The renderer embeds each SVG inside a positioned
//! outer `<svg x y width height>`, relying on the
//! inner viewBox for scaling. Paths default to
//! `fill: black` when no fill is specified, matching
//! the 1-bit Floyd–Steinberg dither pipeline used
//! downstream.

use super::classify::Condition;

/// Weather Icons `wi-day-sunny` — clear sky.
pub const SUNNY: &str =
    include_str!("../../assets/icons/weather-icons/wi-day-sunny.svg");

/// Weather Icons `wi-day-cloudy` — sun + cloud.
pub const PARTLY_CLOUDY: &str =
    include_str!("../../assets/icons/weather-icons/wi-day-cloudy.svg");

/// Weather Icons `wi-cloudy` — full overcast.
pub const CLOUDY: &str =
    include_str!("../../assets/icons/weather-icons/wi-cloudy.svg");

/// Weather Icons `wi-rain` — cloud with rainfall.
pub const RAIN: &str =
    include_str!("../../assets/icons/weather-icons/wi-rain.svg");

/// Strip the XML prolog and any leading comments from
/// a Weather Icons SVG so the remaining `<svg …>…`
/// root element can be embedded as a child of an outer
/// `<svg>`. An XML prolog or processing instruction is
/// only legal at document top, so leaving it in place
/// would break strict XML parsers.
#[must_use]
pub fn strip_xml_prolog(svg: &str) -> &str {
    match svg.find("<svg") {
        Some(idx) => &svg[idx..],
        None => svg,
    }
}

/// Look up the SVG document for a given [`Condition`].
/// Used by the SVG builder when rendering both the
/// current-conditions panel and the forecast day
/// tiles.
#[must_use]
pub fn icon_for(condition: Condition) -> &'static str {
    match condition {
        Condition::Sunny => SUNNY,
        Condition::PartlyCloudy => PARTLY_CLOUDY,
        Condition::Cloudy => CLOUDY,
        Condition::Rain => RAIN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            assert!(
                strip_xml_prolog(svg).starts_with("<svg"),
                "{c:?}: not an SVG document"
            );
            assert!(svg.contains("<path"), "{c:?}: no path data");
        }
    }

    #[test]
    fn strip_xml_prolog_removes_processing_instruction() {
        // Upstream SVGs start with `<?xml …?>` and a
        // generator comment. Embedding those inside an
        // outer <svg> is an XML error, so the helper
        // must drop everything before the root <svg>.
        let with_prolog = "<?xml version=\"1.0\"?>\n<!-- gen -->\n<svg>x</svg>";
        assert_eq!(strip_xml_prolog(with_prolog), "<svg>x</svg>");
    }

    #[test]
    fn strip_xml_prolog_is_idempotent_on_bare_svg() {
        let bare = "<svg viewBox=\"0 0 30 30\"><path/></svg>";
        assert_eq!(strip_xml_prolog(bare), bare);
    }
}
