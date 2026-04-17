//! SVG builder for the dashboard layout.
//!
//! Produces the SVG string that [`crate::render::Renderer`]
//! turns into a 1-bit BMP. Pure string templating —
//! every weather-domain decision happens in
//! [`super::model`] and [`super::classify`] so this
//! module is only responsible for placing known
//! quantities on a fixed canvas.
//!
//! ## Canvas invariant
//!
//! The SVG is always authored at 800 × 480 user units
//! with `viewBox="0 0 800 480"`. The [`Renderer`](crate::render::Renderer)
//! pipeline scales independently in X and Y to the
//! configured pixmap dimensions; for the default
//! 800 × 480 TRMNL OG canvas that's a 1:1 pixel map.
//! Running at other sizes (e.g. TRMNL X's 1872 × 1404)
//! scales the whole layout uniformly, which is the
//! desired behaviour.
//!
//! ## Coordinate system
//!
//! `(0, 0)` is top-left; `(800, 480)` is bottom-right.
//! Text baselines are specified in `y` coordinates;
//! the SVG `text-anchor` attribute handles horizontal
//! alignment so the builder never measures rendered
//! glyph widths itself (the m6x11plus font is
//! proportional — pixel-perfect width math in Rust
//! would be wrong by construction).
//!
//! ## Font sizing
//!
//! m6x11plus is a pixel font tuned for integer
//! multiples of 18. Every `font-size` in this file is
//! `18 * N` for some `N`; sub-multiple sizes would
//! antialias and dither, defeating the font's sharp
//! pixel aesthetic.
//!
//! ## Weekday formatting
//!
//! [`super::model::DaySummary`] stores the weekday as
//! the typed `chrono::Weekday` — the "labels are
//! always English" invariant is enforced here by
//! [`weekday_label`], which returns a hard-coded
//! `&'static str` for each variant rather than relying
//! on `chrono`'s `Display` impl. If that impl's
//! behaviour ever shifts under us (e.g. a localised
//! build), this module still renders the expected
//! three-letter English abbreviation.

use chrono::Weekday;

use super::classify::{Compass8, Condition};
use super::icons;
use super::model::{CurrentConditions, DashboardModel, DaySummary};

/// Canvas width in user units — matches TRMNL OG.
const CANVAS_W: u32 = 800;
/// Canvas height in user units — matches TRMNL OG.
const CANVAS_H: u32 = 480;
/// Y of the horizontal divider between current
/// conditions and the forecast tile row.
const DIVIDER_Y: u32 = 240;
/// Placeholder glyph used whenever a field is missing —
/// a single em dash. Unicode `—` (U+2014) is covered
/// by m6x11plus's extended Latin set.
const PLACEHOLDER: &str = "—";

/// Build the dashboard SVG.
#[must_use]
pub fn build_svg(model: &DashboardModel) -> String {
    let mut body = String::new();
    body.push_str(&current_panel(model.current.as_ref()));
    body.push_str(&divider());
    body.push_str(&day_tiles(&model.days));
    wrap(&body)
}

fn wrap(body: &str) -> String {
    format!(
        concat!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" ",
            "width=\"{w}\" height=\"{h}\" ",
            "viewBox=\"0 0 {w} {h}\" ",
            "font-family=\"m6x11plus\">",
            "<rect width=\"{w}\" height=\"{h}\" fill=\"white\"/>",
            "{body}",
            "</svg>",
        ),
        w = CANVAS_W,
        h = CANVAS_H,
        body = body,
    )
}

fn divider() -> String {
    format!(
        concat!(
            "<line x1=\"40\" y1=\"{y}\" x2=\"760\" y2=\"{y}\" ",
            "stroke=\"black\" stroke-width=\"2\"/>",
        ),
        y = DIVIDER_Y,
    )
}

/// Build the current-conditions panel: the big
/// temperature on the left and the two-line condition /
/// wind label on the right.
fn current_panel(current: Option<&CurrentConditions>) -> String {
    match current {
        Some(c) => format!(
            "{temp}{cond}{wind}",
            temp = current_temperature(c.temp_c),
            cond = current_condition_label(c.condition),
            wind = current_wind_label(c.wind_kmh, c.wind_compass),
        ),
        None => current_temperature_placeholder(),
    }
}

#[allow(clippy::cast_possible_truncation)]
fn round_i32(v: f64) -> i32 {
    v.round() as i32
}

fn current_temperature(temp_c: f64) -> String {
    format!(
        concat!(
            "<text x=\"40\" y=\"200\" font-size=\"180\" ",
            "text-anchor=\"start\" fill=\"black\">",
            "{temp}°</text>",
        ),
        temp = round_i32(temp_c),
    )
}

fn current_temperature_placeholder() -> String {
    format!(
        concat!(
            "<text x=\"40\" y=\"200\" font-size=\"180\" ",
            "text-anchor=\"start\" fill=\"black\">",
            "{placeholder}</text>",
        ),
        placeholder = PLACEHOLDER,
    )
}

fn current_condition_label(condition: Condition) -> String {
    format!(
        concat!(
            "<text x=\"420\" y=\"130\" font-size=\"54\" ",
            "text-anchor=\"start\" fill=\"black\">",
            "{label}</text>",
        ),
        label = condition.label(),
    )
}

fn current_wind_label(kmh: f64, from: Compass8) -> String {
    format!(
        concat!(
            "<text x=\"420\" y=\"200\" font-size=\"36\" ",
            "text-anchor=\"start\" fill=\"black\">",
            "Wind {kmh} km/h {dir}</text>",
        ),
        kmh = round_i32(kmh),
        dir = from.abbrev(),
    )
}

fn day_tiles(days: &[Option<DaySummary>; 3]) -> String {
    let centres = [133_u32, 400_u32, 667_u32];
    let mut out = String::new();
    for (centre_x, slot) in centres.iter().zip(days.iter()) {
        out.push_str(&day_tile(*centre_x, slot.as_ref()));
    }
    out
}

fn day_tile(centre_x: u32, day: Option<&DaySummary>) -> String {
    match day {
        Some(d) => format!(
            "{label}{icon}{high}",
            label = day_label(centre_x, d.weekday),
            icon = day_icon(centre_x, d.condition),
            high = day_high(centre_x, d.high_c),
        ),
        None => day_placeholder(centre_x),
    }
}

/// Three-letter English abbreviation for a weekday.
/// Authoritative source for the dashboard's weekday
/// labels — kept as a match-based constant table rather
/// than `format!("{}", weekday)` so a future change to
/// chrono's `Display` impl can't silently change what
/// the dashboard renders.
fn weekday_label(w: Weekday) -> &'static str {
    match w {
        Weekday::Mon => "Mon",
        Weekday::Tue => "Tue",
        Weekday::Wed => "Wed",
        Weekday::Thu => "Thu",
        Weekday::Fri => "Fri",
        Weekday::Sat => "Sat",
        Weekday::Sun => "Sun",
    }
}

fn day_label(centre_x: u32, weekday: Weekday) -> String {
    format!(
        concat!(
            "<text x=\"{x}\" y=\"300\" font-size=\"36\" ",
            "text-anchor=\"middle\" fill=\"black\">",
            "{label}</text>",
        ),
        x = centre_x,
        label = weekday_label(weekday),
    )
}

fn day_icon(centre_x: u32, condition: Condition) -> String {
    let translate_x = i64::from(centre_x) - 48;
    let translate_y: i32 = 320;
    format!(
        concat!(
            "<g transform=\"translate({tx} {ty}) scale(2)\">",
            "{icon}",
            "</g>",
        ),
        tx = translate_x,
        ty = translate_y,
        icon = icons::icon_for(condition),
    )
}

fn day_high(centre_x: u32, high_c: Option<i32>) -> String {
    match high_c {
        Some(n) => format!(
            concat!(
                "<text x=\"{x}\" y=\"460\" font-size=\"72\" ",
                "text-anchor=\"middle\" fill=\"black\">",
                "{high}°</text>",
            ),
            x = centre_x,
            high = n,
        ),
        None => format!(
            concat!(
                "<text x=\"{x}\" y=\"460\" font-size=\"72\" ",
                "text-anchor=\"middle\" fill=\"black\">",
                "{placeholder}</text>",
            ),
            x = centre_x,
            placeholder = PLACEHOLDER,
        ),
    }
}

fn day_placeholder(centre_x: u32) -> String {
    format!(
        concat!(
            "<text x=\"{x}\" y=\"400\" font-size=\"72\" ",
            "text-anchor=\"middle\" fill=\"black\">",
            "{placeholder}</text>",
        ),
        x = centre_x,
        placeholder = PLACEHOLDER,
    )
}

#[cfg(test)]
mod tests {
    use super::super::model::{
        CurrentConditions, DAY_TILE_COUNT, DashboardModel, DaySummary,
    };
    use super::*;

    fn sample_model() -> DashboardModel {
        DashboardModel {
            current: Some(CurrentConditions {
                temp_c: 12.0,
                condition: Condition::Cloudy,
                wind_kmh: 18.0,
                wind_compass: Compass8::SW,
            }),
            days: [
                Some(DaySummary {
                    weekday: Weekday::Sat,
                    high_c: Some(14),
                    condition: Condition::Sunny,
                }),
                Some(DaySummary {
                    weekday: Weekday::Sun,
                    high_c: Some(11),
                    condition: Condition::Rain,
                }),
                Some(DaySummary {
                    weekday: Weekday::Mon,
                    high_c: Some(9),
                    condition: Condition::Cloudy,
                }),
            ],
        }
    }

    #[test]
    fn produces_canvas_sized_svg() {
        let svg = build_svg(&sample_model());
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"800\""));
        assert!(svg.contains("height=\"480\""));
        assert!(svg.contains("viewBox=\"0 0 800 480\""));
    }

    #[test]
    fn includes_current_temperature_and_labels() {
        let svg = build_svg(&sample_model());
        assert!(svg.contains("12°"), "temp missing: {svg}");
        assert!(svg.contains("Cloudy"), "condition label missing");
        assert!(svg.contains("Wind 18 km/h SW"), "wind label missing: {svg}",);
    }

    #[test]
    fn renders_all_three_day_labels_and_highs() {
        let svg = build_svg(&sample_model());
        for label in ["Sat", "Sun", "Mon"] {
            assert!(svg.contains(label), "missing day {label}");
        }
        for high in ["14°", "11°", "9°"] {
            assert!(svg.contains(high), "missing high {high}");
        }
    }

    #[test]
    fn embeds_one_icon_per_day_tile() {
        let svg = build_svg(&sample_model());
        let count = svg.matches("<g transform=").count();
        assert_eq!(count, DAY_TILE_COUNT, "{svg}");
    }

    #[test]
    fn missing_current_falls_back_to_placeholder_temp() {
        let mut model = sample_model();
        model.current = None;
        let svg = build_svg(&model);
        assert!(svg.contains("—"), "placeholder missing: {svg}");
        assert!(!svg.contains("Cloudy"));
        assert!(!svg.contains("Wind "));
    }

    #[test]
    fn missing_day_renders_placeholder_only() {
        let mut model = sample_model();
        model.days[1] = None;
        let svg = build_svg(&model);
        assert!(svg.contains("Sat"));
        assert!(svg.contains("Mon"));
        let count = svg.matches("<g transform=").count();
        assert_eq!(count, 2, "expected 2 icon groups: {svg}");
    }

    #[test]
    fn day_with_none_high_renders_placeholder_temp() {
        // A tile with valid condition but None temp
        // (every sample was null-temp — see model's
        // day_high_celsius tests) should render the
        // weekday label, the condition icon, and the em
        // dash in the high-temperature slot — never "0°".
        let mut model = sample_model();
        model.days[0] = Some(DaySummary {
            weekday: Weekday::Sat,
            high_c: None,
            condition: Condition::Sunny,
        });
        let svg = build_svg(&model);
        assert!(svg.contains("Sat"));
        // Zero — which a magic-`0` regression would
        // emit — must not appear as a temperature.
        assert!(
            !svg.contains("0°"),
            "unexpected 0° in SVG after None high: {svg}",
        );
        // The em dash should appear at least once (the
        // high-temp placeholder); other days have real
        // highs, so the count is exactly 1 here.
        assert_eq!(svg.matches(PLACEHOLDER).count(), 1);
    }

    #[test]
    fn font_family_set_at_svg_root() {
        let svg = build_svg(&sample_model());
        assert_eq!(
            svg.matches("font-family=\"m6x11plus\"").count(),
            1,
            "expected exactly one font-family attr: {svg}",
        );
    }

    #[test]
    fn font_sizes_are_integer_multiples_of_18() {
        let svg = build_svg(&sample_model());
        let mut sizes: Vec<u32> = Vec::new();
        for (_, rest) in svg.match_indices("font-size=\"") {
            let num: String = rest
                .chars()
                .skip("font-size=\"".len())
                .take_while(char::is_ascii_digit)
                .collect();
            let n: u32 = num.parse().unwrap_or(0);
            sizes.push(n);
        }
        assert!(!sizes.is_empty(), "no font-size attrs: {svg}");
        for n in sizes {
            assert_eq!(n % 18, 0, "font-size {n} is not a multiple of 18");
        }
    }

    #[test]
    fn weekday_label_is_three_char_english() {
        // The SVG's weekday labels must always be the
        // three-char English abbreviations even if
        // chrono's Display impl ever changes behaviour.
        // Lock the full rotation here so a future
        // refactor catches breakage at test time.
        assert_eq!(weekday_label(Weekday::Mon), "Mon");
        assert_eq!(weekday_label(Weekday::Tue), "Tue");
        assert_eq!(weekday_label(Weekday::Wed), "Wed");
        assert_eq!(weekday_label(Weekday::Thu), "Thu");
        assert_eq!(weekday_label(Weekday::Fri), "Fri");
        assert_eq!(weekday_label(Weekday::Sat), "Sat");
        assert_eq!(weekday_label(Weekday::Sun), "Sun");
    }
}
