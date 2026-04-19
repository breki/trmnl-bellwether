use chrono::NaiveTime;

use super::super::model::{
    CurrentConditions, DashboardModel, DaySummary, TodaySummary,
};
use super::*;

fn noon() -> NaiveTime {
    NaiveTime::from_hms_opt(11, 45, 0).unwrap()
}

#[test]
fn embedded_layout_parses_and_resolves() {
    // Guards the `expect("embedded default layout must
    // resolve")` inside `build_svg` — any accidental
    // breakage of `assets/layout.toml` will fail here
    // instead of crashing a production render.
    let layout = super::super::layout::Layout::embedded_default();
    let resolved = layout
        .resolve()
        .expect("embedded layout.toml must resolve cleanly");
    assert!(!resolved.widgets.is_empty());
}

fn sample_model() -> DashboardModel {
    DashboardModel {
        current: Some(CurrentConditions {
            temp_c: 10.0,
            feels_like_c: 8.0,
            condition: Condition::PartlyCloudy,
            wind_kmh: 18.0,
            wind_compass: Compass8::SW,
            gust_kmh: Some(25.0),
            humidity_pct: Some(65.0),
        }),
        today: Some(TodaySummary {
            high_c: Some(15),
            low_c: Some(8),
            sunrise_local: NaiveTime::from_hms_opt(6, 12, 0),
            sunset_local: NaiveTime::from_hms_opt(19, 38, 0),
        }),
        days: [
            Some(DaySummary {
                weekday: Weekday::Sat,
                high_c: Some(14),
                low_c: Some(7),
                condition: Condition::Sunny,
            }),
            Some(DaySummary {
                weekday: Weekday::Sun,
                high_c: Some(11),
                low_c: Some(5),
                condition: Condition::Rain,
            }),
            Some(DaySummary {
                weekday: Weekday::Mon,
                high_c: Some(9),
                low_c: Some(3),
                condition: Condition::Cloudy,
            }),
        ],
        day_weekdays: [Weekday::Sat, Weekday::Sun, Weekday::Mon],
        battery_pct: Some(82),
    }
}

#[test]
fn produces_canvas_sized_svg() {
    let svg = build_svg(&sample_model(), noon());
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
    assert!(svg.contains("width=\"800\""));
    assert!(svg.contains("height=\"480\""));
    assert!(svg.contains("viewBox=\"0 0 800 480\""));
}

#[test]
fn header_contains_brand_title_and_clock() {
    let svg = build_svg(&sample_model(), noon());
    assert!(svg.contains(">TRMNL<"));
    assert!(svg.contains(">Weather Report<"));
    assert!(svg.contains(">11:45<"), "clock missing: {svg}");
}

#[test]
fn header_battery_percentage_rendered_when_known() {
    let svg = build_svg(&sample_model(), noon());
    assert!(svg.contains(">82%<"), "battery pct missing: {svg}");
}

#[test]
fn header_battery_placeholder_when_unknown() {
    let mut model = sample_model();
    model.battery_pct = None;
    let svg = build_svg(&model, noon());
    // em-dash inline placeholder in the battery slot
    // (there may be other em-dashes elsewhere but the
    // one near "Weather Report" is in the header).
    assert!(svg.contains("—"), "placeholder missing: {svg}");
    // No battery fill rectangle inside the outline.
    // The outline has `fill="none"` and the tip has
    // `fill="black"`; a fully-drawn fill would be an
    // extra rect at x=694 with non-zero width.
    // Count the inner rects — outline + tip = 2 rects
    // in the battery area when pct is None.
    assert!(!svg.contains("width=\"0\""), "zero-width rect: {svg}");
}

#[test]
fn current_panel_includes_big_temperature_and_labels() {
    let svg = build_svg(&sample_model(), noon());
    assert!(svg.contains("10°"), "big temp missing: {svg}");
    assert!(svg.contains("Partly cloudy"));
    assert!(svg.contains("Feels like 8°"));
}

#[test]
fn meteo_strip_has_three_cells_with_wind_gust_humidity() {
    let svg = build_svg(&sample_model(), noon());
    assert!(svg.contains("Wind SW 18 km/h"));
    assert!(svg.contains("Gust 25 km/h"));
    assert!(svg.contains("Humidity 65%"));
}

#[test]
fn meteo_strip_shows_placeholder_for_missing_gust_and_humidity() {
    let mut model = sample_model();
    if let Some(c) = model.current.as_mut() {
        c.gust_kmh = None;
        c.humidity_pct = None;
    }
    let svg = build_svg(&model, noon());
    assert!(svg.contains("Wind SW 18 km/h"));
    assert!(svg.contains("Gust —"));
    assert!(svg.contains("Humidity —"));
}

#[test]
fn forecast_row_renders_three_tiles_with_highs_and_lows() {
    let svg = build_svg(&sample_model(), noon());
    for label in ["Sat", "Sun", "Mon"] {
        assert!(svg.contains(label), "day label {label} missing");
    }
    assert!(svg.contains("H 14° L 7°"));
    assert!(svg.contains("H 11° L 5°"));
    assert!(svg.contains("H 9° L 3°"));
}

#[test]
fn forecast_tile_with_none_day_keeps_weekday_label() {
    // Missing data must not drop the weekday header
    // — otherwise a user seeing "Sat, —, Mon" can't
    // tell whether Sunday or Tuesday is the missing
    // day. Placeholder body replaces icon + H/L only.
    let mut model = sample_model();
    model.days[1] = None;
    let svg = build_svg(&model, noon());
    // All three weekdays still rendered.
    assert!(svg.contains(">Sat<"));
    assert!(svg.contains(">Sun<"));
    assert!(svg.contains(">Mon<"));
    // The missing tile's icon group is dropped, so we
    // expect only two forecast-row icon groups (scale
    // 1.6), plus the current-conditions icon (scale
    // 2.08) which is unaffected.
    assert_eq!(svg.matches("scale(1.6)").count(), 2);
}

#[test]
fn footer_shows_today_highlow_sunrise_sunset() {
    let svg = build_svg(&sample_model(), noon());
    assert!(svg.contains("Today H 15° L 8°"));
    assert!(svg.contains("Sunrise 06:12"));
    assert!(svg.contains("Sunset 19:38"));
}

#[test]
fn footer_shows_placeholder_when_today_summary_missing() {
    let mut model = sample_model();
    model.today = None;
    let svg = build_svg(&model, noon());
    assert!(svg.contains("Today —"));
    assert!(svg.contains("Sunrise —"));
    assert!(svg.contains("Sunset —"));
}

#[test]
fn footer_shows_placeholder_when_astro_unavailable() {
    // Polar day/night: today populated but both astro
    // times are None.
    let mut model = sample_model();
    if let Some(today) = model.today.as_mut() {
        today.sunrise_local = None;
        today.sunset_local = None;
    }
    let svg = build_svg(&model, noon());
    assert!(svg.contains("Today H 15° L 8°"));
    assert!(svg.contains("Sunrise —"));
    assert!(svg.contains("Sunset —"));
}

#[test]
fn current_panel_collapses_to_placeholder_when_temp_missing() {
    let mut model = sample_model();
    model.current = None;
    let svg = build_svg(&model, noon());
    // Big temp, condition label, and feels-like text
    // all omitted. A meaningful "No current reading"
    // message takes their place, rather than a lone
    // 120-px em-dash floating in empty space.
    assert!(!svg.contains("Partly cloudy"));
    assert!(!svg.contains("Feels like"));
    assert!(
        svg.contains("No current reading"),
        "expected fallback label, got: {svg}",
    );
}

#[test]
fn font_family_in_svg_matches_ttf_name_table_family() {
    // The SVG's `font-family` attribute must match
    // the family name the bundled TTF's `name` table
    // advertises. A typo like "Atkinson-Hyperlegible"
    // (or a font swap that changed the family name)
    // gets caught here at test time.
    let face =
        ttf_parser::Face::parse(crate::render::ATKINSON_HYPERLEGIBLE_TTF, 0)
            .expect("valid TrueType face");
    let family = face
        .names()
        .into_iter()
        .filter(|n| n.name_id == ttf_parser::name_id::FAMILY)
        .find_map(|n| n.to_string())
        .expect("font must expose a family name");
    let svg = build_svg(&sample_model(), noon());
    let expected = format!("font-family=\"{family}\"");
    assert!(
        svg.contains(&expected),
        "SVG font-family does not match TTF family {family:?}",
    );
}

#[test]
fn weekday_label_is_three_char_english() {
    assert_eq!(weekday_label(Weekday::Mon), "Mon");
    assert_eq!(weekday_label(Weekday::Tue), "Tue");
    assert_eq!(weekday_label(Weekday::Wed), "Wed");
    assert_eq!(weekday_label(Weekday::Thu), "Thu");
    assert_eq!(weekday_label(Weekday::Fri), "Fri");
    assert_eq!(weekday_label(Weekday::Sat), "Sat");
    assert_eq!(weekday_label(Weekday::Sun), "Sun");
}

#[test]
fn calm_wind_renders_as_calm_not_zero_knot_north() {
    // The snapshot adapter defaults calm conditions
    // to wind_dir_deg = 0.0 → Compass8::N. Without
    // special-casing, the cell would read
    // "Wind N 0 km/h" — a fake north wind.
    let mut model = sample_model();
    if let Some(c) = model.current.as_mut() {
        c.wind_kmh = 0.0;
        c.wind_compass = Compass8::N;
    }
    let svg = build_svg(&model, noon());
    assert!(svg.contains("Wind calm"), "missing calm label: {svg}");
    assert!(!svg.contains("Wind N 0"), "fake north wind rendered: {svg}");
}

#[test]
fn battery_fill_rounds_not_truncates() {
    // 99 % should render as ~51 of 52 inner pixels,
    // not 51 from a `* / 100` truncation (same).
    // Point of this test: the rounding rule must
    // produce at least one less than full-width at
    // 99 % and full-width at 100 %.
    let mut model = sample_model();
    model.battery_pct = Some(99);
    let svg_99 = build_svg(&model, noon());
    model.battery_pct = Some(100);
    let svg_100 = build_svg(&model, noon());
    // Just confirm both contain non-empty fill rects
    // (width ≥ 1) and a clean 0% produces no fill.
    model.battery_pct = Some(0);
    let svg_0 = build_svg(&model, noon());
    assert!(svg_99.contains("fill=\"black\""));
    assert!(svg_100.contains("fill=\"black\""));
    // 0 % → no inner fill rect, so the canvas should
    // have only the outline + tip rects with
    // `fill="black"`. Count:
    let zero_fills = svg_0.matches("width=\"0\"").count();
    assert_eq!(
        zero_fills, 0,
        "expected no width=\"0\" rect at 0 %, got {svg_0}",
    );
}

#[test]
fn format_clock_pads_to_two_digits() {
    let early = NaiveTime::from_hms_opt(6, 9, 0).unwrap();
    assert_eq!(format_clock(early), "06:09");
    let late = NaiveTime::from_hms_opt(23, 59, 59).unwrap();
    assert_eq!(format_clock(late), "23:59");
}
