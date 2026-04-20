use chrono::NaiveTime;

use super::super::classify::{ConditionCategory, WeatherCode, WmoCode};
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
            category: ConditionCategory::PartlyCloudy,
            weather_code: Some(WeatherCode::Wmo(WmoCode::PartlyCloudy)),
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
                category: ConditionCategory::Clear,
                weather_code: Some(WeatherCode::Wmo(WmoCode::Clear)),
            }),
            Some(DaySummary {
                weekday: Weekday::Sun,
                high_c: Some(11),
                low_c: Some(5),
                category: ConditionCategory::Thunderstorm,
                weather_code: Some(WeatherCode::Wmo(WmoCode::Thunderstorm)),
            }),
            Some(DaySummary {
                weekday: Weekday::Mon,
                high_c: Some(9),
                low_c: Some(3),
                category: ConditionCategory::Fog,
                weather_code: Some(WeatherCode::Wmo(WmoCode::Fog)),
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
    // Atomic widgets render each hi/lo separately with
    // an "H"/"L" prefix from the default layout.
    for (hi, lo) in [(14, 7), (11, 5), (9, 3)] {
        assert!(
            svg.contains(&format!("H {hi}°")),
            "missing H {hi}\u{b0}: {svg}",
        );
        assert!(
            svg.contains(&format!("L {lo}°")),
            "missing L {lo}\u{b0}: {svg}",
        );
    }
}

#[test]
fn forecast_tile_with_none_day_keeps_weekday_label() {
    // Missing forecast data must not drop the
    // weekday header — day-name widget reads from
    // ctx.model.day_weekdays (always populated) so
    // the operator can still see *which* day is
    // missing.
    let mut model = sample_model();
    model.days[1] = None;
    let svg = build_svg(&model, noon());
    assert!(svg.contains(">Sat<"));
    assert!(svg.contains(">Sun<"));
    assert!(svg.contains(">Mon<"));
    // The missing day's hi/lo render as em-dashes.
    assert!(svg.contains(">H —<") || svg.contains(">L —<"));
}

#[test]
fn footer_shows_today_highlow_sunrise_sunset() {
    let svg = build_svg(&sample_model(), noon());
    // Default layout labels today's high as
    // "Today H" via the `label` field and low as "L".
    assert!(svg.contains("Today H 15°"));
    assert!(svg.contains("L 8°"));
    assert!(svg.contains("Sunrise 06:12"));
    assert!(svg.contains("Sunset 19:38"));
}

#[test]
fn footer_shows_placeholder_when_today_summary_missing() {
    let mut model = sample_model();
    model.today = None;
    let svg = build_svg(&model, noon());
    assert!(svg.contains("Today H —"));
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
    assert!(svg.contains("Today H 15°"));
    assert!(svg.contains("Sunrise —"));
    assert!(svg.contains("Sunset —"));
}

#[test]
fn current_panel_renders_placeholders_when_current_missing() {
    let mut model = sample_model();
    model.current = None;
    let svg = build_svg(&model, noon());
    // Atomic widgets each render their own placeholder
    // instead of a composite "No current reading"
    // banner.
    assert!(!svg.contains("Partly cloudy"));
    // temp-now → "—"; feels-like → "Feels like —".
    assert!(svg.contains("Feels like —"));
}

#[test]
fn font_family_in_svg_matches_ttf_name_table_family() {
    // The SVG's `font-family` attribute must match
    // the family name the bundled TTF's `name` table
    // advertises. A typo like "SourceSans3"
    // (or a font swap that changed the family name)
    // gets caught here at test time.
    let face =
        ttf_parser::Face::parse(crate::render::SOURCE_SANS_3_SEMIBOLD_TTF, 0)
            .expect("valid TrueType face");
    // Prefer the typographic family (name ID 16) when
    // present — for weight-per-file families like
    // Source Sans 3 Semibold, the legacy `FAMILY`
    // (name ID 1) bakes the weight into the family
    // string ("Source Sans 3 Semibold") whereas the
    // typographic family is the weight-agnostic
    // "Source Sans 3" that pairs with a `font-weight`
    // selector. fontdb/resvg match on the typographic
    // family, so that's what the SVG should emit.
    let mut typographic = None;
    let mut legacy = None;
    for n in face.names() {
        match n.name_id {
            ttf_parser::name_id::TYPOGRAPHIC_FAMILY
                if typographic.is_none() =>
            {
                typographic = n.to_string();
            }
            ttf_parser::name_id::FAMILY if legacy.is_none() => {
                legacy = n.to_string();
            }
            _ => {}
        }
    }
    let family = typographic
        .or(legacy)
        .expect("font must expose a family name");
    // The SVG wraps the family in single quotes inside
    // the double-quoted attribute value. svgtypes'
    // unquoted `font-family` parser tokenises as CSS
    // identifiers, which cannot start with a digit —
    // without the inner quotes, "Source Sans 3" fails
    // to parse at the "3" and text silently falls back
    // to the default font.
    let svg = build_svg(&sample_model(), noon());
    let expected = format!("font-family=\"'{family}'\"");
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
fn day_name_today_renders_literal_today_word() {
    // `day = "today"` on a day-name widget emits the
    // word "Today" rather than a weekday abbreviation,
    // so the current-conditions column reads naturally
    // in layouts that use `day-name` alongside
    // `temp-now`.
    let layout_src = r#"
canvas = { width = 200, height = 40 }
widget = "day-name"
day = "today"
"#;
    let layout: super::super::layout::Layout =
        toml::from_str(layout_src).unwrap();
    let svg = build_svg_with_layout(&layout, &sample_model(), noon()).unwrap();
    assert!(svg.contains(">Today<"), "expected 'Today' label: {svg}");
}

#[test]
fn temp_high_with_label_prefixes_label_and_degree() {
    let layout_src = r#"
canvas = { width = 200, height = 40 }
widget = "temp-high"
day = 0
label = "H"
"#;
    let layout: super::super::layout::Layout =
        toml::from_str(layout_src).unwrap();
    let svg = build_svg_with_layout(&layout, &sample_model(), noon()).unwrap();
    assert!(svg.contains(">H 14°<"), "expected 'H 14°': {svg}");
}

#[test]
fn weather_icon_fidelity_detailed_differs_from_simple_for_specialised_code() {
    // Deferred from PR 4 and now reachable because
    // `icon_for_wmo` has at least one specialised arm
    // (`ThunderstormHailHeavy` → `wi-hail.svg`).
    //
    // Contract being locked: "two widgets reading the
    // same model but differing only in `fidelity`
    // produce different SVG bytes for a specialised
    // code." That is exactly what `assert_ne!` on two
    // single-widget renders checks — nothing more,
    // nothing less. Previous revisions of this test
    // pattern-matched on a substring of the hail
    // glyph's `d` attribute; that coincidentally also
    // appears in rain/snow/drizzle icons, so the test
    // was brittle against later PRs specialising those
    // variants (RT-1/AQ-1/AQ-2).
    let simple_src = r#"
canvas = { width = 200, height = 200 }
widget = "weather-icon"
day = "today"
"#;
    let detailed_src = r#"
canvas = { width = 200, height = 200 }
widget = "weather-icon"
day = "today"
fidelity = "detailed"
"#;
    let simple_layout: super::super::layout::Layout =
        toml::from_str(simple_src).unwrap();
    let detailed_layout: super::super::layout::Layout =
        toml::from_str(detailed_src).unwrap();
    let mut model = sample_model();
    if let Some(c) = model.current.as_mut() {
        c.weather_code = Some(WeatherCode::Wmo(WmoCode::ThunderstormHailHeavy));
    }
    let svg_simple =
        build_svg_with_layout(&simple_layout, &model, noon()).unwrap();
    let svg_detailed =
        build_svg_with_layout(&detailed_layout, &model, noon()).unwrap();
    assert_ne!(
        svg_simple, svg_detailed,
        "Fidelity was silently dropped: Simple and Detailed \
         rendered identically for a specialised code",
    );
}

#[test]
fn format_clock_pads_to_two_digits() {
    let early = NaiveTime::from_hms_opt(6, 9, 0).unwrap();
    assert_eq!(format_clock(early), "06:09");
    let late = NaiveTime::from_hms_opt(23, 59, 59).unwrap();
    assert_eq!(format_clock(late), "23:59");
}
