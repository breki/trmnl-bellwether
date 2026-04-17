//! Weather icons as SVG element fragments.
//!
//! Each constant is a set of SVG primitives (circles,
//! rectangles, paths) in a 48 × 48 user-unit coordinate
//! system with `(0, 0)` at top-left. The fragments are
//! bare elements — no `<svg>` or `<g>` wrapper — so the
//! layout builder can drop them inside a `<g
//! transform="translate(x y) scale(k)">` at the final
//! dashboard position.
//!
//! The designs favour big, solid shapes: a 1-bit
//! Floyd–Steinberg dither handles high-contrast fills
//! cleanly (confirmed by the render module's tests)
//! but is unfriendly to thin strokes and gradients.
//! At the intended ~96 × 96 display size, pixel-level
//! cleanliness matters more than realism.

use super::classify::Condition;

/// 48×48 sun icon: a filled disc with eight radial rays.
pub const SUNNY_48: &str = concat!(
    r#"<circle cx="24" cy="24" r="10" fill="black"/>"#,
    r#"<rect x="22" y="2" width="4" height="7" fill="black"/>"#,
    r#"<rect x="22" y="39" width="4" height="7" fill="black"/>"#,
    r#"<rect x="2" y="22" width="7" height="4" fill="black"/>"#,
    r#"<rect x="39" y="22" width="7" height="4" fill="black"/>"#,
    r#"<rect x="7" y="7" width="5" height="5" fill="black"/>"#,
    r#"<rect x="36" y="7" width="5" height="5" fill="black"/>"#,
    r#"<rect x="7" y="36" width="5" height="5" fill="black"/>"#,
    r#"<rect x="36" y="36" width="5" height="5" fill="black"/>"#,
);

/// 48×48 partly-cloudy icon: a small sun in the upper
/// left, mostly obscured by a cloud on the lower right.
pub const PARTLY_CLOUDY_48: &str = concat!(
    // Small sun, unobstructed half.
    r#"<circle cx="14" cy="14" r="7" fill="black"/>"#,
    r#"<rect x="12" y="1" width="4" height="5" fill="black"/>"#,
    r#"<rect x="1" y="12" width="5" height="4" fill="black"/>"#,
    r#"<rect x="3" y="3" width="4" height="4" fill="black"/>"#,
    // Cloud: three circles + base rectangle.
    r#"<circle cx="18" cy="32" r="8" fill="black"/>"#,
    r#"<circle cx="30" cy="26" r="10" fill="black"/>"#,
    r#"<circle cx="40" cy="32" r="7" fill="black"/>"#,
    r#"<rect x="14" y="30" width="28" height="12" fill="black"/>"#,
);

/// 48×48 cloudy icon: a big cloud filling most of the
/// frame.
pub const CLOUDY_48: &str = concat!(
    r#"<circle cx="14" cy="26" r="10" fill="black"/>"#,
    r#"<circle cx="28" cy="20" r="12" fill="black"/>"#,
    r#"<circle cx="40" cy="26" r="8" fill="black"/>"#,
    r#"<rect x="10" y="24" width="32" height="14" fill="black"/>"#,
);

/// 48×48 rain icon: a cloud up top, three raindrops
/// below.
pub const RAIN_48: &str = concat!(
    r#"<circle cx="14" cy="16" r="8" fill="black"/>"#,
    r#"<circle cx="26" cy="12" r="10" fill="black"/>"#,
    r#"<circle cx="38" cy="16" r="6" fill="black"/>"#,
    r#"<rect x="10" y="14" width="28" height="12" fill="black"/>"#,
    // Three vertical raindrop slashes.
    r#"<rect x="14" y="32" width="3" height="8" fill="black"/>"#,
    r#"<rect x="24" y="34" width="3" height="10" fill="black"/>"#,
    r#"<rect x="32" y="32" width="3" height="8" fill="black"/>"#,
);

/// Look up the 48-user-unit icon fragment for a given
/// [`Condition`]. Used by the SVG builder when
/// rendering both the current-conditions panel and the
/// forecast day tiles.
#[must_use]
pub fn icon_for(condition: Condition) -> &'static str {
    match condition {
        Condition::Sunny => SUNNY_48,
        Condition::PartlyCloudy => PARTLY_CLOUDY_48,
        Condition::Cloudy => CLOUDY_48,
        Condition::Rain => RAIN_48,
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
            assert!(svg.contains("fill=\"black\""), "{c:?}: no fill");
        }
    }

    #[test]
    fn icons_contain_no_svg_or_g_wrappers() {
        // The layout builder wraps each icon in its own
        // <g transform> element, so the fragments must
        // not include their own. A stray <svg> would
        // also break the nested-SVG semantics of the
        // parent document.
        for svg in [SUNNY_48, PARTLY_CLOUDY_48, CLOUDY_48, RAIN_48] {
            assert!(!svg.contains("<svg"), "fragment has <svg>: {svg}");
            assert!(!svg.contains("<g "), "fragment has <g>: {svg}");
        }
    }
}
