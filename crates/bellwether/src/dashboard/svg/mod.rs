//! SVG builder for the dense v0.11 dashboard layout.
//!
//! Produces the SVG string that
//! [`crate::render::Renderer`] turns into a 1-bit
//! BMP. Pure string templating — every weather-domain
//! decision happens in [`super::model`] and
//! [`super::classify`] so this module is only
//! responsible for placing known quantities on a
//! fixed canvas.
//!
//! ## Canvas invariant
//!
//! The SVG is always authored at 800 × 480 user units
//! with `viewBox="0 0 800 480"`. The
//! [`Renderer`](crate::render::Renderer) pipeline
//! scales independently in X and Y to the configured
//! pixmap dimensions; for the default 800 × 480 TRMNL
//! OG canvas that's a 1:1 pixel map.
//!
//! ## Layout
//!
//! Five horizontal bands, separated by 2-px dividers:
//!
//! 1. **Header** (`0..=50`) — TRMNL brand, "Weather
//!    Report" title, clock, battery indicator.
//! 2. **Current conditions** (`50..=190`) — big
//!    condition icon on the left, big temperature,
//!    condition word and "feels like" line on the
//!    right.
//! 3. **Meteorology strip** (`190..=240`) — three
//!    cells: wind, gust, humidity. Missing-data
//!    cells render the em-dash placeholder.
//! 4. **Forecast row** (`240..=430`) — three tiles
//!    for the next three days, each with weekday,
//!    icon, and "H ..° L ..°" high/low line.
//! 5. **Footer** (`430..=480`) — today's high/low,
//!    sunrise, sunset.
//!
//! ## Weekday and clock formatting
//!
//! Weekdays are rendered via
//! [`super::model::DaySummary::weekday`] converted by
//! the private [`weekday_label`] function to a
//! three-letter English abbreviation. The header clock
//! is received as a [`chrono::NaiveTime`] passed
//! separately to [`build_svg`] (not stored on the
//! model — the forecast data has no clock concept).
//!
//! ## Missing-data convention
//!
//! Every optional field renders an em-dash ("—")
//! placeholder when `None`, never a fake default.
//! Matches the project's "never show fake numbers"
//! philosophy.

use std::fmt::Write as _;

use chrono::{NaiveTime, Timelike, Weekday};

use super::classify::{Compass8, Condition};
use super::icons;
use super::model::{
    CurrentConditions, DAY_TILE_COUNT, DashboardModel, DaySummary, TodaySummary,
};

/// Canvas width in user units — matches TRMNL OG.
const CANVAS_W: u32 = 800;
/// Canvas height in user units — matches TRMNL OG.
const CANVAS_H: u32 = 480;

/// Y of the divider between the header and the
/// current-conditions band.
const HEADER_DIVIDER_Y: u32 = 50;
/// Y of the divider between the current-conditions
/// band and the meteorology strip.
const CURRENT_DIVIDER_Y: u32 = 190;
/// Y of the divider between the meteorology strip and
/// the forecast row.
const METEO_DIVIDER_Y: u32 = 240;
/// Y of the divider between the forecast row and the
/// footer.
const FORECAST_DIVIDER_Y: u32 = 430;

/// Horizontal padding applied to divider lines so they
/// don't run edge-to-edge.
const DIVIDER_INSET_X: u32 = 20;

/// Placeholder glyph used wherever a field is missing
/// — a single em dash. Unicode `—` (U+2014) is covered
/// by the bundled font.
const PLACEHOLDER: &str = "—";

// ─── Font sizes ──────────────────────────────────────

/// Font size for the "TRMNL" brand label on the
/// header's left.
const BRAND_PX: u32 = 24;
/// Font size for the "Weather Report" header title.
const HEADER_TITLE_PX: u32 = 28;
/// Font size for the header clock.
const CLOCK_PX: u32 = 28;
/// Font size for the battery percentage next to the
/// battery icon.
const BATTERY_PCT_PX: u32 = 22;

/// Font size for the big current-conditions
/// temperature.
const CURRENT_TEMP_PX: u32 = 120;
/// Font size for the condition label
/// ("Partly cloudy", "Sunny", …).
const CONDITION_LABEL_PX: u32 = 44;
/// Font size for the "Feels like 8°" line beneath
/// the condition label.
const FEELS_LIKE_PX: u32 = 26;

/// Font size for each meteorology-strip cell.
const METEO_CELL_PX: u32 = 28;

/// Font size for each forecast tile's weekday
/// abbreviation.
const DAY_LABEL_PX: u32 = 32;
/// Font size for each forecast tile's "H .. L .."
/// high/low line.
const DAY_HIGH_LOW_PX: u32 = 28;

/// Font size for the footer items.
const FOOTER_PX: u32 = 22;

/// Centre X of each column in the 3-column grid
/// shared by the meteorology strip and the forecast
/// row. Hoisted so the two bands can't silently drift
/// out of alignment.
const TRIPLE_COLUMN_CENTRES: [u32; 3] = [133, 400, 667];

/// Shared renderer for every SVG `<text>` element in
/// the dashboard. Consolidates the opening-tag
/// boilerplate so individual cells only carry their
/// own positional and textual specifics.
///
/// `content` is interpolated **raw** into the SVG.
/// All call sites today pass compile-time literals,
/// `&'static str` returns from enum methods
/// (`Condition::label()`, `Compass8::abbrev()`,
/// `weekday_label`), or output of numeric formatters
/// — none of which can carry `<`, `>`, `&` or other
/// XML-special characters. If a future refactor
/// lets a user- or forecast-supplied string flow
/// into `content`, add an XML-escape at the call
/// site or here.
fn text(
    x: u32,
    y: u32,
    size: u32,
    anchor: &str,
    extra_attrs: &str,
    content: &str,
) -> String {
    debug_assert!(
        !content.contains('<') && !content.contains('&'),
        "SVG text content contains XML-special char: {content:?}",
    );
    format!(
        "<text x=\"{x}\" y=\"{y}\" font-size=\"{size}\" \
         text-anchor=\"{anchor}\" fill=\"black\"{extra_attrs}>\
         {content}</text>",
    )
}

/// Build the dashboard SVG.
///
/// `now_local` is the wall-clock time the header
/// clock should display. Passed separately so the
/// model stays purely forecast-derived — a rendered
/// `DashboardModel` doesn't go stale the instant the
/// caller holds it; only the SVG does.
#[must_use]
pub fn build_svg(model: &DashboardModel, now_local: NaiveTime) -> String {
    let mut body = String::new();
    body.push_str(&header_band(now_local, model.battery_pct));
    body.push_str(&current_band(model.current.as_ref()));
    body.push_str(&meteo_band(model.current.as_ref()));
    body.push_str(&forecast_band(&model.days, model.day_weekdays));
    body.push_str(&footer_band(model.today.as_ref()));
    body.push_str(&section_dividers());
    wrap(&body)
}

fn wrap(body: &str) -> String {
    format!(
        concat!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" ",
            "width=\"{w}\" height=\"{h}\" ",
            "viewBox=\"0 0 {w} {h}\" ",
            "font-family=\"Atkinson Hyperlegible\">",
            "<rect width=\"{w}\" height=\"{h}\" fill=\"white\"/>",
            "{body}",
            "</svg>",
        ),
        w = CANVAS_W,
        h = CANVAS_H,
        body = body,
    )
}

fn section_dividers() -> String {
    let mut out = String::new();
    for y in [
        HEADER_DIVIDER_Y,
        CURRENT_DIVIDER_Y,
        METEO_DIVIDER_Y,
        FORECAST_DIVIDER_Y,
    ] {
        let _ = write!(
            out,
            concat!(
                "<line x1=\"{x1}\" y1=\"{y}\" x2=\"{x2}\" y2=\"{y}\" ",
                "stroke=\"black\" stroke-width=\"2\"/>",
            ),
            x1 = DIVIDER_INSET_X,
            x2 = CANVAS_W - DIVIDER_INSET_X,
            y = y,
        );
    }
    out
}

// ─── Header band ─────────────────────────────────────

fn header_band(now_local: NaiveTime, battery_pct: Option<u8>) -> String {
    let brand = header_brand();
    let title = header_title();
    let clock = header_clock(now_local);
    let battery = battery_indicator(battery_pct);
    format!("{brand}{title}{clock}{battery}")
}

fn header_brand() -> String {
    text(30, 34, BRAND_PX, "start", " font-weight=\"bold\"", "TRMNL")
}

fn header_title() -> String {
    text(400, 34, HEADER_TITLE_PX, "middle", "", "Weather Report")
}

fn header_clock(now_local: NaiveTime) -> String {
    // Right-aligned against the battery indicator.
    // Battery sits at x=690..=760; clock ends at
    // x=660 with a gap.
    text(650, 34, CLOCK_PX, "end", "", &format_clock(now_local))
}

fn format_clock(t: NaiveTime) -> String {
    // 24-hour HH:MM. We skip seconds — the dashboard
    // only refreshes every ~15 min so sub-minute
    // precision would be a lie.
    format!("{:02}:{:02}", t.hour(), t.minute())
}

// ─── Battery indicator ──────────────────────────────

/// Battery outline position. Top-left corner.
const BATTERY_X: u32 = 690;
const BATTERY_Y: u32 = 14;
const BATTERY_W: u32 = 60;
const BATTERY_H: u32 = 24;
/// Stroke thickness of the outline.
const BATTERY_STROKE: u32 = 2;
/// Battery-tip rectangle on the right. Positioned to
/// the right of the outline so the full indicator is
/// visually a capped rectangle.
const BATTERY_TIP_W: u32 = 4;
const BATTERY_TIP_H: u32 = 10;
/// Font-size for the percentage text right of the
/// outline. Actually drawn *left* of the outline
/// because the outline sits at the far-right of the
/// canvas — the percentage reads more naturally to
/// its left.
const BATTERY_PCT_X: u32 = 685;
const BATTERY_PCT_Y: u32 = 34;
/// Inside-fill padding — the fill rectangle sits
/// strictly inside the stroke.
const BATTERY_FILL_INSET: u32 = 4;

fn battery_indicator(pct: Option<u8>) -> String {
    let outline = format!(
        concat!(
            "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" ",
            "fill=\"none\" stroke=\"black\" stroke-width=\"{s}\"/>",
            "<rect x=\"{tip_x}\" y=\"{tip_y}\" ",
            "width=\"{tw}\" height=\"{th}\" fill=\"black\"/>",
        ),
        x = BATTERY_X,
        y = BATTERY_Y,
        w = BATTERY_W,
        h = BATTERY_H,
        s = BATTERY_STROKE,
        tip_x = BATTERY_X + BATTERY_W,
        tip_y = BATTERY_Y + (BATTERY_H - BATTERY_TIP_H) / 2,
        tw = BATTERY_TIP_W,
        th = BATTERY_TIP_H,
    );
    let label_content = match pct {
        Some(p) => format!("{p}%"),
        None => PLACEHOLDER.to_owned(),
    };
    let label = text(
        BATTERY_PCT_X,
        BATTERY_PCT_Y,
        BATTERY_PCT_PX,
        "end",
        "",
        &label_content,
    );
    let fill = pct.map_or_else(String::new, battery_fill_rect);
    format!("{label}{outline}{fill}")
}

fn battery_fill_rect(pct: u8) -> String {
    // Upstream `battery_voltage_to_pct` already clamps
    // to 0..=100; keep the debug-assert so a broken
    // path surfaces in tests rather than silently
    // renders an oversized fill.
    debug_assert!(pct <= 100, "battery pct out of range: {pct}");
    let pct_clamped = pct.min(100);
    let inner_max = BATTERY_W.saturating_sub(2 * BATTERY_FILL_INSET);
    // Round-half-to-nearest so 99 % renders as 99 %
    // of the inner width, not 98 % (the old `* / 100`
    // truncation loss).
    let width = (inner_max * u32::from(pct_clamped) + 50) / 100;
    // Skip a zero-width rect so `pct = 0..=0` produces
    // a clean empty outline instead of a valid-but-
    // useless `<rect width="0">`.
    if width == 0 {
        return String::new();
    }
    format!(
        concat!(
            "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" ",
            "fill=\"black\"/>",
        ),
        x = BATTERY_X + BATTERY_FILL_INSET,
        y = BATTERY_Y + BATTERY_FILL_INSET,
        w = width,
        h = BATTERY_H - 2 * BATTERY_FILL_INSET,
    )
}

// ─── Current-conditions band ────────────────────────

fn current_band(current: Option<&CurrentConditions>) -> String {
    match current {
        Some(c) => {
            let icon = current_icon(c.condition);
            let temp = current_temperature(c.temp_c);
            let condition = current_condition_label(c.condition);
            let feels = current_feels_like(c.feels_like_c);
            format!("{icon}{temp}{condition}{feels}")
        }
        None => current_temperature_placeholder(),
    }
}

fn current_icon(condition: Condition) -> String {
    // Icon fragment is authored at 48 user units;
    // scale to 100 px so it sits comfortably on the
    // left third of the current panel (y=50..=190
    // is 140 px tall; 100 × 100 icon leaves 20-px
    // top/bottom padding).
    format!(
        concat!(
            "<g transform=\"translate(30 70) scale(2.08)\">",
            "{icon}",
            "</g>",
        ),
        icon = icons::icon_for(condition),
    )
}

fn current_temperature(temp_c: f64) -> String {
    text(
        150,
        160,
        CURRENT_TEMP_PX,
        "start",
        "",
        &format!("{}°", round_i32(temp_c)),
    )
}

fn current_temperature_placeholder() -> String {
    // The current band is 140 px tall; a lone 120-px
    // em-dash reads as visual noise. Use a smaller
    // explicit label centred in the band instead, so
    // the operator understands "no current reading"
    // rather than "the system rendered garbage".
    text(
        CANVAS_W / 2,
        130,
        CONDITION_LABEL_PX,
        "middle",
        "",
        "No current reading",
    )
}

fn current_condition_label(condition: Condition) -> String {
    text(420, 115, CONDITION_LABEL_PX, "start", "", condition.label())
}

fn current_feels_like(feels_like_c: f64) -> String {
    text(
        420,
        160,
        FEELS_LIKE_PX,
        "start",
        "",
        &format!("Feels like {}°", round_i32(feels_like_c)),
    )
}

// ─── Meteorology strip ─────────────────────────────

fn meteo_band(current: Option<&CurrentConditions>) -> String {
    let cells = match current {
        Some(c) => [
            format_wind_cell(c.wind_kmh, c.wind_compass),
            format_gust_cell(c.gust_kmh),
            format_humidity_cell(c.humidity_pct),
        ],
        None => [
            format_missing_cell("Wind"),
            format_missing_cell("Gust"),
            format_missing_cell("Humidity"),
        ],
    };
    let mut out = String::new();
    for (x, cell_text) in TRIPLE_COLUMN_CENTRES.iter().zip(cells.iter()) {
        out.push_str(&text(*x, 222, METEO_CELL_PX, "middle", "", cell_text));
    }
    out.push_str(&meteo_separator(266));
    out.push_str(&meteo_separator(533));
    out
}

fn meteo_separator(x: u32) -> String {
    format!(
        concat!(
            "<line x1=\"{x}\" y1=\"198\" x2=\"{x}\" y2=\"232\" ",
            "stroke=\"black\" stroke-width=\"1\"/>",
        ),
        x = x,
    )
}

fn format_wind_cell(kmh: f64, from: Compass8) -> String {
    // `wind_to_compass` returns `Compass8::N` as a
    // placeholder for calm conditions; don't confuse
    // the user by labelling 0 km/h as a north wind.
    if round_i32(kmh) == 0 {
        "Wind calm".to_owned()
    } else {
        format!(
            "Wind {dir} {kmh} km/h",
            dir = from.abbrev(),
            kmh = round_i32(kmh),
        )
    }
}

fn format_gust_cell(gust_kmh: Option<f64>) -> String {
    format_labelled_number(gust_kmh, "Gust", " km/h")
}

fn format_humidity_cell(humidity_pct: Option<f64>) -> String {
    format_labelled_number(humidity_pct, "Humidity", "%")
}

fn format_missing_cell(label: &str) -> String {
    format!("{label} {PLACEHOLDER}")
}

/// Render a labelled numeric cell — `"{label} {n}{unit}"`
/// when `value` is `Some`, `"{label} —"` when `None`.
/// Used by the meteo strip's optional fields so the
/// "label + rounded value + unit + em-dash fallback"
/// shape lives in exactly one place.
fn format_labelled_number(
    value: Option<f64>,
    label: &str,
    unit: &str,
) -> String {
    match value {
        Some(v) => format!("{label} {}{unit}", round_i32(v)),
        None => format!("{label} {PLACEHOLDER}"),
    }
}

// ─── Forecast row ───────────────────────────────────

fn forecast_band(
    days: &[Option<DaySummary>; DAY_TILE_COUNT],
    weekdays: [Weekday; DAY_TILE_COUNT],
) -> String {
    TRIPLE_COLUMN_CENTRES
        .iter()
        .zip(days.iter())
        .zip(weekdays.iter())
        .map(|((x, slot), weekday)| forecast_tile(*x, slot.as_ref(), *weekday))
        .collect()
}

fn forecast_tile(
    centre_x: u32,
    day: Option<&DaySummary>,
    weekday: Weekday,
) -> String {
    // The weekday header is always drawn, even when
    // the data row is a placeholder, so an operator
    // can see *which* day is missing instead of a
    // dangling em-dash between two real days.
    let label = day_label(centre_x, weekday);
    let body = match day {
        Some(d) => {
            let icon = day_icon(centre_x, d.condition);
            let high_low = day_high_low(centre_x, d.high_c, d.low_c);
            format!("{icon}{high_low}")
        }
        None => day_placeholder(centre_x),
    };
    format!("{label}{body}")
}

/// Three-letter English abbreviation for a weekday.
/// Authoritative source for the dashboard's weekday
/// labels — kept as a match-based constant table
/// rather than `format!("{}", weekday)` so a future
/// change to chrono's `Display` impl can't silently
/// change what the dashboard renders.
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
    text(
        centre_x,
        280,
        DAY_LABEL_PX,
        "middle",
        "",
        weekday_label(weekday),
    )
}

fn day_icon(centre_x: u32, condition: Condition) -> String {
    // Tile icon at scale 1.6 → 77 × 77 px. Top at
    // y=300, bottom at y=377. Below, the H/L text
    // sits at y=412 (baseline), leaving ~15 px margin.
    let translate_x = i64::from(centre_x) - 38;
    format!(
        "<g transform=\"translate({tx} 300) scale(1.6)\">{icon}</g>",
        tx = translate_x,
        icon = icons::icon_for(condition),
    )
}

fn day_high_low(
    centre_x: u32,
    high_c: Option<i32>,
    low_c: Option<i32>,
) -> String {
    let content = format!(
        "H {high} L {low}",
        high = format_temp(high_c),
        low = format_temp(low_c),
    );
    text(centre_x, 412, DAY_HIGH_LOW_PX, "middle", "", &content)
}

fn day_placeholder(centre_x: u32) -> String {
    text(centre_x, 345, DAY_HIGH_LOW_PX, "middle", "", PLACEHOLDER)
}

fn format_temp(value: Option<i32>) -> String {
    match value {
        Some(n) => format!("{n}°"),
        None => PLACEHOLDER.to_owned(),
    }
}

// ─── Footer band ────────────────────────────────────

fn footer_band(today: Option<&TodaySummary>) -> String {
    let today_content = today.map_or_else(
        || format!("Today {PLACEHOLDER}"),
        |t| {
            format!(
                "Today H {high} L {low}",
                high = format_temp(t.high_c),
                low = format_temp(t.low_c),
            )
        },
    );
    let today_hi_lo = footer_item(30, &today_content);
    let sunrise = footer_item(
        410,
        &astro_label("Sunrise", today.and_then(|t| t.sunrise_local)),
    );
    let sunset = footer_item(
        620,
        &astro_label("Sunset", today.and_then(|t| t.sunset_local)),
    );
    format!("{today_hi_lo}{sunrise}{sunset}")
}

fn footer_item(x: u32, content: &str) -> String {
    text(x, 462, FOOTER_PX, "start", "", content)
}

fn astro_label(label: &str, time: Option<NaiveTime>) -> String {
    match time {
        Some(t) => format!("{label} {}", format_clock(t)),
        None => format!("{label} {PLACEHOLDER}"),
    }
}

// ─── Tiny utilities ────────────────────────────────

#[allow(clippy::cast_possible_truncation)]
fn round_i32(v: f64) -> i32 {
    v.round() as i32
}

#[cfg(test)]
mod tests;
