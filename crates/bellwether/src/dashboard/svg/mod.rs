//! SVG builder for the dashboard.
//!
//! Produces the SVG string that
//! [`crate::render::Renderer`] turns into a 1-bit BMP.
//! Layout is data-driven: the tree in
//! `crates/bellwether/assets/layout.toml` (parsed into
//! [`Layout`]) is walked by [`Layout::resolve`] to
//! assign each widget a [`Rect`], and this module
//! dispatches each placement to a bounds-relative
//! widget renderer. Weather-domain decisions happen in
//! [`super::model`] and [`super::classify`]; this
//! module only places known quantities on the canvas.
//!
//! ## Canvas
//!
//! Canvas dimensions come from the layout's
//! `canvas = { width, height }` header; the default
//! matches the TRMNL OG's 800 × 480 panel. The
//! [`Renderer`](crate::render::Renderer) pipeline
//! scales to the configured pixmap dimensions.
//!
//! ## Vertical placement
//!
//! Widget Y coordinates are expressed relative to the
//! widget's assigned bounds, not the canvas. Resizing
//! a band in `layout.toml` moves the widget with it.
//!
//! ## Missing-data convention
//!
//! Every optional field renders an em-dash ("—")
//! placeholder when `None`, never a fake default.

use chrono::{NaiveTime, Timelike, Weekday};

use super::classify::{Compass8, Condition};
use super::icons;
use super::layout::{
    Direction, Layout, LayoutError, PlacedDivider, Rect, WidgetKind,
};
use super::model::{
    CurrentConditions, DashboardModel, DaySummary, TodaySummary,
};

/// Horizontal padding applied to divider lines so they
/// don't run edge-to-edge.
const DIVIDER_INSET_X: u32 = 20;

/// Stroke width for the inter-band and column
/// divider lines.
const DIVIDER_STROKE: u32 = 2;

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

/// Shared renderer for every SVG `<text>` element in
/// the dashboard. Consolidates the opening-tag
/// boilerplate so individual cells only carry their
/// own positional and textual specifics.
///
/// `content` is **XML-escaped** via [`escape_xml`]
/// before interpolation, so any caller (including
/// user-supplied strings from `layout.toml`, e.g. a
/// [`WidgetKind::HeaderTitle`] with `&` or `<` in its
/// text) produces well-formed SVG.
fn text(
    x: u32,
    y: u32,
    size: u32,
    anchor: &str,
    extra_attrs: &str,
    content: &str,
) -> String {
    format!(
        "<text x=\"{x}\" y=\"{y}\" font-size=\"{size}\" \
         text-anchor=\"{anchor}\" fill=\"black\"{extra_attrs}>\
         {content}</text>",
        content = escape_xml(content),
    )
}

/// Escape the five XML-predefined entities so arbitrary
/// text (including values pulled from a user-supplied
/// `layout.toml`) can flow into the SVG without
/// producing malformed output.
fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Context threaded through every widget renderer.
/// Bundles the weather-domain model and the wall-clock
/// time so the dispatcher can stay a simple function of
/// `(widget, bounds, ctx)`.
struct RenderContext<'a> {
    model: &'a DashboardModel,
    now_local: NaiveTime,
}

/// Build the dashboard SVG using the embedded default
/// layout.
///
/// `now_local` is the wall-clock time the header
/// clock should display. Passed separately so the
/// model stays purely forecast-derived — a rendered
/// `DashboardModel` doesn't go stale the instant the
/// caller holds it; only the SVG does.
///
/// Panics only if the embedded `layout.toml` is
/// malformed — a condition guaranteed not to happen at
/// runtime by
/// [`tests::embedded_layout_parses_and_resolves`].
#[must_use]
pub fn build_svg(model: &DashboardModel, now_local: NaiveTime) -> String {
    build_svg_with_layout(Layout::embedded_default(), model, now_local)
        .expect("embedded default layout must resolve (test-guaranteed)")
}

/// Render the dashboard to SVG using an explicit layout.
///
/// Returns [`LayoutError`] if the layout fails to
/// resolve — use this entry point when the layout was
/// loaded from user input (e.g. `[dashboard]` in the
/// main config TOML).
///
/// # Errors
///
/// Propagates any [`LayoutError`] produced by
/// [`Layout::resolve`] (empty splits, overflow,
/// arithmetic overflow).
pub fn build_svg_with_layout(
    layout: &Layout,
    model: &DashboardModel,
    now_local: NaiveTime,
) -> Result<String, LayoutError> {
    let resolved = layout.resolve()?;
    let ctx = RenderContext { model, now_local };

    let mut body = String::new();
    for p in &resolved.widgets {
        body.push_str(&render_widget(p.widget, p.bounds, &ctx));
    }
    for d in &resolved.dividers {
        body.push_str(&render_divider(layout.canvas.width, *d));
    }
    Ok(wrap(layout.canvas.width, layout.canvas.height, &body))
}

/// Render a single widget at its assigned bounds.
fn render_widget(
    kind: &WidgetKind,
    bounds: Rect,
    ctx: &RenderContext<'_>,
) -> String {
    let current = ctx.model.current.as_ref();
    let today = ctx.model.today.as_ref();
    match kind {
        WidgetKind::Brand => render_brand(bounds),
        WidgetKind::HeaderTitle { text } => render_header_title(bounds, text),
        WidgetKind::Clock => render_clock(bounds, ctx.now_local),
        WidgetKind::Battery => render_battery(bounds, ctx.model.battery_pct),
        WidgetKind::CurrentConditions => {
            render_current_conditions(bounds, current)
        }
        WidgetKind::Wind => render_meteo_cell(
            bounds,
            &current.map_or_else(
                || format_missing_cell("Wind"),
                |c| format_wind_cell(c.wind_kmh, c.wind_compass),
            ),
        ),
        WidgetKind::Gust => render_meteo_cell(
            bounds,
            &current.map_or_else(
                || format_missing_cell("Gust"),
                |c| format_gust_cell(c.gust_kmh),
            ),
        ),
        WidgetKind::Humidity => render_meteo_cell(
            bounds,
            &current.map_or_else(
                || format_missing_cell("Humidity"),
                |c| format_humidity_cell(c.humidity_pct),
            ),
        ),
        WidgetKind::ForecastDay { offset } => {
            let idx = usize::from(*offset);
            // Out-of-range offsets render a "—" tile
            // rather than panic — the `days` /
            // `day_weekdays` arrays are fixed-length by
            // DashboardModel contract, but the offset
            // is a u8 loaded from user TOML.
            match (ctx.model.days.get(idx), ctx.model.day_weekdays.get(idx)) {
                (Some(day), Some(weekday)) => {
                    forecast_tile(bounds, day.as_ref(), *weekday)
                }
                _ => forecast_tile_out_of_range(bounds),
            }
        }
        WidgetKind::TodayHiLo => render_today_hi_lo(bounds, today),
        WidgetKind::Sunrise => render_footer_astro(
            bounds,
            "Sunrise",
            today.and_then(|t| t.sunrise_local),
        ),
        WidgetKind::Sunset => render_footer_astro(
            bounds,
            "Sunset",
            today.and_then(|t| t.sunset_local),
        ),
    }
}

fn wrap(canvas_w: u32, canvas_h: u32, body: &str) -> String {
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
        w = canvas_w,
        h = canvas_h,
        body = body,
    )
}

/// Render a single [`PlacedDivider`] as a 2-px line.
/// Horizontal-split dividers become vertical lines
/// running full bounds height; vertical-split dividers
/// become horizontal lines inset from the canvas edges
/// so they don't run edge-to-edge.
fn render_divider(canvas_w: u32, d: PlacedDivider) -> String {
    match d.orientation {
        Direction::Vertical => {
            // Split is vertical → children stack top→bottom
            // → divider line runs horizontally between
            // them. Inset from the left/right edges.
            let y = d.bounds.y + d.bounds.h / 2;
            format!(
                "<line x1=\"{x1}\" y1=\"{y}\" x2=\"{x2}\" y2=\"{y}\" \
                 stroke=\"black\" stroke-width=\"{s}\"/>",
                x1 = DIVIDER_INSET_X,
                x2 = canvas_w.saturating_sub(DIVIDER_INSET_X),
                y = y,
                s = DIVIDER_STROKE,
            )
        }
        Direction::Horizontal => {
            // Split is horizontal → children stack
            // left→right → divider line runs vertically.
            let x = d.bounds.x + d.bounds.w / 2;
            format!(
                "<line x1=\"{x}\" y1=\"{y1}\" x2=\"{x}\" y2=\"{y2}\" \
                 stroke=\"black\" stroke-width=\"{s}\"/>",
                x = x,
                y1 = d.bounds.y,
                y2 = d.bounds.y + d.bounds.h,
                s = DIVIDER_STROKE,
            )
        }
    }
}

// ─── Header-band widgets ────────────────────────────

/// Baseline of header-row text as a fraction of the
/// band's height — 68 % places it visually just below
/// the centre, matching the prior hardcoded y=34 on a
/// 50-px band.
fn header_baseline_y(bounds: Rect) -> u32 {
    bounds.y + bounds.h * 68 / 100
}

fn render_brand(bounds: Rect) -> String {
    text(
        bounds.x + 30,
        header_baseline_y(bounds),
        BRAND_PX,
        "start",
        " font-weight=\"bold\"",
        "TRMNL",
    )
}

fn render_header_title(bounds: Rect, title_text: &str) -> String {
    text(
        bounds.x + bounds.w / 2,
        header_baseline_y(bounds),
        HEADER_TITLE_PX,
        "middle",
        "",
        title_text,
    )
}

fn render_clock(bounds: Rect, now_local: NaiveTime) -> String {
    // End-anchored at the bounds' right edge. The
    // header layout sizes the clock cell so this
    // lands visually clear of the battery indicator.
    text(
        bounds.x + bounds.w,
        header_baseline_y(bounds),
        CLOCK_PX,
        "end",
        "",
        &format_clock(now_local),
    )
}

fn format_clock(t: NaiveTime) -> String {
    // 24-hour HH:MM. We skip seconds — the dashboard
    // only refreshes every ~15 min so sub-minute
    // precision would be a lie.
    format!("{:02}:{:02}", t.hour(), t.minute())
}

// ─── Battery indicator ──────────────────────────────

/// Battery outline width in pixels.
const BATTERY_W: u32 = 60;
/// Battery outline height in pixels.
const BATTERY_H: u32 = 24;
/// Stroke thickness of the outline.
const BATTERY_STROKE: u32 = 2;
/// Battery-tip rectangle on the right. Positioned to
/// the right of the outline so the full indicator is
/// visually a capped rectangle.
const BATTERY_TIP_W: u32 = 4;
const BATTERY_TIP_H: u32 = 10;
/// Inside-fill padding — the fill rectangle sits
/// strictly inside the stroke.
const BATTERY_FILL_INSET: u32 = 4;
/// Left pad of the battery outline inside its widget
/// bounds — leaves room for the percentage label.
const BATTERY_LEFT_PAD: u32 = 40;

fn render_battery(bounds: Rect, pct: Option<u8>) -> String {
    // Centre the outline vertically within the bounds.
    // Falls back to 0 if bounds.h somehow ends up
    // smaller than the outline height.
    let outline_y = bounds.y + bounds.h.saturating_sub(BATTERY_H) / 2;
    let label_y = header_baseline_y(bounds);
    // Left-pad the outline inside the bounds so there's
    // room for the percentage label on its left.
    let outline_x = bounds.x + BATTERY_LEFT_PAD;
    let label_x = outline_x.saturating_sub(5);
    let outline = format!(
        concat!(
            "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" ",
            "fill=\"none\" stroke=\"black\" stroke-width=\"{s}\"/>",
            "<rect x=\"{tip_x}\" y=\"{tip_y}\" ",
            "width=\"{tw}\" height=\"{th}\" fill=\"black\"/>",
        ),
        x = outline_x,
        y = outline_y,
        w = BATTERY_W,
        h = BATTERY_H,
        s = BATTERY_STROKE,
        tip_x = outline_x + BATTERY_W,
        tip_y = outline_y + (BATTERY_H - BATTERY_TIP_H) / 2,
        tw = BATTERY_TIP_W,
        th = BATTERY_TIP_H,
    );
    let label_content = match pct {
        Some(p) => format!("{p}%"),
        None => PLACEHOLDER.to_owned(),
    };
    let label =
        text(label_x, label_y, BATTERY_PCT_PX, "end", "", &label_content);
    let fill = pct.map_or_else(String::new, |p| {
        battery_fill_rect(outline_x, outline_y, p)
    });
    format!("{label}{outline}{fill}")
}

fn battery_fill_rect(outline_x: u32, outline_y: u32, pct: u8) -> String {
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
        x = outline_x + BATTERY_FILL_INSET,
        y = outline_y + BATTERY_FILL_INSET,
        w = width,
        h = BATTERY_H - 2 * BATTERY_FILL_INSET,
    )
}

// ─── Current-conditions widget ─────────────────────

fn render_current_conditions(
    bounds: Rect,
    current: Option<&CurrentConditions>,
) -> String {
    match current {
        Some(c) => {
            let icon = current_icon(bounds, c.condition);
            let temp = current_temperature(bounds, c.temp_c);
            let condition = current_condition_label(bounds, c.condition);
            let feels = current_feels_like(bounds, c.feels_like_c);
            format!("{icon}{temp}{condition}{feels}")
        }
        None => current_temperature_placeholder(bounds),
    }
}

fn current_icon(bounds: Rect, condition: Condition) -> String {
    // Icon authored at 48 user units; scale to ≈100 px
    // so it sits comfortably on the band's left third.
    // Translation is offset 30 px right and 20 px down
    // from the band origin.
    format!(
        concat!(
            "<g transform=\"translate({tx} {ty}) scale(2.08)\">",
            "{icon}",
            "</g>",
        ),
        tx = bounds.x + 30,
        ty = bounds.y + 20,
        icon = icons::icon_for(condition),
    )
}

fn current_temperature(bounds: Rect, temp_c: f64) -> String {
    text(
        bounds.x + 150,
        bounds.y + 110,
        CURRENT_TEMP_PX,
        "start",
        "",
        &format!("{}°", round_i32(temp_c)),
    )
}

fn current_temperature_placeholder(bounds: Rect) -> String {
    // The current band is 140 px tall; a lone 120-px
    // em-dash reads as visual noise. Use a smaller
    // explicit label centred in the band instead, so
    // the operator understands "no current reading"
    // rather than "the system rendered garbage".
    text(
        bounds.x + bounds.w / 2,
        bounds.y + 80,
        CONDITION_LABEL_PX,
        "middle",
        "",
        "No current reading",
    )
}

fn current_condition_label(bounds: Rect, condition: Condition) -> String {
    text(
        bounds.x + 420,
        bounds.y + 65,
        CONDITION_LABEL_PX,
        "start",
        "",
        condition.label(),
    )
}

fn current_feels_like(bounds: Rect, feels_like_c: f64) -> String {
    text(
        bounds.x + 420,
        bounds.y + 110,
        FEELS_LIKE_PX,
        "start",
        "",
        &format!("Feels like {}°", round_i32(feels_like_c)),
    )
}

// ─── Meteorology strip ─────────────────────────────

fn render_meteo_cell(bounds: Rect, cell_text: &str) -> String {
    let centre_x = bounds.x + bounds.w / 2;
    // Centre the text vertically within the bounds,
    // nudged down to land on the typographic baseline.
    let baseline_y = bounds.y + bounds.h * 64 / 100;
    text(centre_x, baseline_y, METEO_CELL_PX, "middle", "", cell_text)
}

fn format_wind_cell(kmh: f64, from: Compass8) -> String {
    // The snapshot adapter collapses calm winds to
    // 0 km/h + 0° (→ Compass8::N); don't confuse the
    // user by labelling 0 km/h as a north wind.
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

fn forecast_tile(
    bounds: Rect,
    day: Option<&DaySummary>,
    weekday: Weekday,
) -> String {
    // The weekday header is always drawn, even when
    // the data row is a placeholder, so an operator
    // can see *which* day is missing instead of a
    // dangling em-dash between two real days.
    let centre_x = bounds.x + bounds.w / 2;
    let label = day_label(bounds, centre_x, weekday);
    let body = match day {
        Some(d) => {
            let icon = day_icon(bounds, centre_x, d.condition);
            let high_low = day_high_low(bounds, centre_x, d.high_c, d.low_c);
            format!("{icon}{high_low}")
        }
        None => day_placeholder(bounds, centre_x),
    };
    format!("{label}{body}")
}

fn forecast_tile_out_of_range(bounds: Rect) -> String {
    let centre_x = bounds.x + bounds.w / 2;
    // Centre a placeholder vertically — this path only
    // fires if `layout.toml` specifies an out-of-range
    // `ForecastDay.offset`.
    text(
        centre_x,
        bounds.y + bounds.h / 2,
        DAY_HIGH_LOW_PX,
        "middle",
        "",
        PLACEHOLDER,
    )
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

/// Baseline offsets within a forecast tile, expressed
/// as fractions of the tile's height. The defaults
/// reproduce the old 190-px band layout (label at
/// y=280 → 21 % from band top, icon at y=300 → 32 %,
/// H/L at y=412 → 91 %).
fn day_label(bounds: Rect, centre_x: u32, weekday: Weekday) -> String {
    text(
        centre_x,
        bounds.y + bounds.h * 21 / 100,
        DAY_LABEL_PX,
        "middle",
        "",
        weekday_label(weekday),
    )
}

fn day_icon(bounds: Rect, centre_x: u32, condition: Condition) -> String {
    // Tile icon at scale 1.6 → 77 × 77 px; anchored
    // ~32 % from the band top, centred horizontally on
    // the column.
    let translate_x = i64::from(centre_x) - 38;
    let translate_y = bounds.y + bounds.h * 32 / 100;
    format!(
        "<g transform=\"translate({tx} {ty}) scale(1.6)\">{icon}</g>",
        tx = translate_x,
        ty = translate_y,
        icon = icons::icon_for(condition),
    )
}

fn day_high_low(
    bounds: Rect,
    centre_x: u32,
    high_c: Option<i32>,
    low_c: Option<i32>,
) -> String {
    let content = format!(
        "H {high} L {low}",
        high = format_temp(high_c),
        low = format_temp(low_c),
    );
    text(
        centre_x,
        bounds.y + bounds.h * 91 / 100,
        DAY_HIGH_LOW_PX,
        "middle",
        "",
        &content,
    )
}

fn day_placeholder(bounds: Rect, centre_x: u32) -> String {
    text(
        centre_x,
        bounds.y + bounds.h * 55 / 100,
        DAY_HIGH_LOW_PX,
        "middle",
        "",
        PLACEHOLDER,
    )
}

fn format_temp(value: Option<i32>) -> String {
    match value {
        Some(n) => format!("{n}°"),
        None => PLACEHOLDER.to_owned(),
    }
}

// ─── Footer widgets ────────────────────────────────

fn render_today_hi_lo(bounds: Rect, today: Option<&TodaySummary>) -> String {
    let content = today.map_or_else(
        || format!("Today {PLACEHOLDER}"),
        |t| {
            format!(
                "Today H {high} L {low}",
                high = format_temp(t.high_c),
                low = format_temp(t.low_c),
            )
        },
    );
    footer_item(bounds, &content)
}

fn render_footer_astro(
    bounds: Rect,
    label: &str,
    time: Option<NaiveTime>,
) -> String {
    footer_item(bounds, &astro_label(label, time))
}

/// Left padding applied inside each footer widget's
/// bounds before its text baseline. Keeps the three
/// items visually aligned with a consistent gutter.
const FOOTER_ITEM_LEFT_PAD: u32 = 30;

fn footer_item(bounds: Rect, content: &str) -> String {
    text(
        bounds.x + FOOTER_ITEM_LEFT_PAD,
        bounds.y + bounds.h * 64 / 100,
        FOOTER_PX,
        "start",
        "",
        content,
    )
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
