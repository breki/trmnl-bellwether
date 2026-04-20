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
    DaySelector, Direction, Layout, LayoutError, PlacedDivider, Rect,
    WidgetKind,
};
use super::model::DashboardModel;

/// A unified view of "the day referenced by a
/// `DaySelector`". Centralises the per-day projection
/// so the renderer stops repeating the same
/// match-on-selector skeleton for every field.
#[derive(Debug, Clone, Copy)]
struct DayView {
    condition: Option<Condition>,
    high_c: Option<i32>,
    low_c: Option<i32>,
}

fn resolve_day(day: DaySelector, model: &DashboardModel) -> DayView {
    match day {
        DaySelector::Today => DayView {
            condition: model.current.as_ref().map(|c| c.condition),
            high_c: model.today.as_ref().and_then(|t| t.high_c),
            low_c: model.today.as_ref().and_then(|t| t.low_c),
        },
        DaySelector::Offset(n) => {
            let day = model.days.get(usize::from(n)).and_then(Option::as_ref);
            DayView {
                condition: day.map(|d| d.condition),
                high_c: day.and_then(|d| d.high_c),
                low_c: day.and_then(|d| d.low_c),
            }
        }
    }
}

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

/// Font size for each meteorology-strip cell.
const METEO_CELL_PX: u32 = 28;

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
        WidgetKind::WeatherIcon { day } => {
            render_weather_icon(bounds, resolve_day(*day, ctx.model).condition)
        }
        WidgetKind::TempNow { .. } => {
            render_temp_now(bounds, current.map(|c| c.temp_c))
        }
        WidgetKind::Condition { day } => {
            render_condition(bounds, resolve_day(*day, ctx.model).condition)
        }
        WidgetKind::FeelsLike { .. } => {
            render_feels_like(bounds, current.map(|c| c.feels_like_c))
        }
        WidgetKind::DayName { day } => render_day_name(bounds, ctx, *day),
        WidgetKind::TempHigh { day, label } => render_labelled_temp(
            bounds,
            resolve_day(*day, ctx.model).high_c,
            label.as_deref(),
        ),
        WidgetKind::TempLow { day, label } => render_labelled_temp(
            bounds,
            resolve_day(*day, ctx.model).low_c,
            label.as_deref(),
        ),
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

fn wrap(w: u32, h: u32, body: &str) -> String {
    // The family name is single-quoted inside the
    // double-quoted attribute because svgtypes' unquoted
    // `font-family` parser tokenises as CSS identifiers,
    // which can't start with a digit — `Source Sans 3`
    // unquoted would fail to parse at the `3`, families
    // would fall back to the default, and all text would
    // silently drop. The quoted-string branch of the
    // parser treats the whole value as one family name.
    let family = crate::render::SOURCE_SANS_3_FAMILY;
    let weight = crate::render::SOURCE_SANS_3_WEIGHT;
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" \
         width=\"{w}\" height=\"{h}\" \
         viewBox=\"0 0 {w} {h}\" \
         font-family=\"'{family}'\" \
         font-weight=\"{weight}\">\
         <rect width=\"{w}\" height=\"{h}\" fill=\"white\"/>\
         {body}\
         </svg>"
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

// ─── Atomic weather widgets ────────────────────────
//
// Every text widget below auto-sizes its glyph height
// to a fixed fraction of the assigned `Rect`. Visual
// weight is therefore entirely a function of the
// widget's bounds in the layout, which lets
// `layout.toml` tune emphasis without widget-level
// font knobs.

/// Fraction of bounds height used as the font-size for
/// the "big number" widgets (`temp-now`). Leaves room
/// for glyph descenders.
const FONT_FRACTION_BIG: u32 = 85;
/// Fraction of bounds height used for medium text
/// widgets (`day-name`, `temp-high`, `temp-low`,
/// `condition`).
const FONT_FRACTION_MEDIUM: u32 = 60;
/// Fraction of bounds height used for compact text
/// widgets (`feels-like`).
const FONT_FRACTION_SMALL: u32 = 42;
/// Fraction of bounds' *smaller* dimension used to
/// pick the icon scale. Icons are authored at 48 user
/// units, so scale = min(w,h) * fraction / 100 / 48.
const ICON_FRACTION: u32 = 85;

/// Average glyph width as a percentage of font size
/// for the bundled Source Sans 3 Semibold face.
/// Slightly generous so short strings don't overflow
/// edge-case bounds. Used only by [`fit_font_px`] for
/// the cap-by-width heuristic.
const AVG_GLYPH_WIDTH_PCT: u64 = 60;

/// Minimum legible font size in pixels. Acts as a
/// floor on the width-based heuristic so a long label
/// in a narrow cell either shrinks to this size or
/// spills over (rather than rendering at 1-2 px and
/// silently becoming invisible).
const MIN_LEGIBLE_PX: u32 = 8;

/// Pick a font size that fits `content` inside `bounds`
/// on both axes. The height-based candidate is
/// `bounds.h * fraction / 100` (auto-fit to bounds
/// height). The width-based candidate is the largest
/// font whose estimated text width — `n_chars × font ×
/// AVG_GLYPH_WIDTH_PCT%` — still fits `bounds.w`. The
/// smaller of the two wins, then the result is
/// clamped to [`MIN_LEGIBLE_PX`] so an overlong label
/// doesn't reduce to an invisible 1 px glyph. All
/// arithmetic is performed in `u64` and narrowed at
/// the end so pathological (but TOML-trusted) widget
/// geometry can't wrap.
fn fit_font_px(bounds: Rect, h_fraction: u32, content: &str) -> u32 {
    let h = u64::from(bounds.h);
    let w = u64::from(bounds.w);
    let frac = u64::from(h_fraction);
    let n_chars = u64::try_from(content.chars().count())
        .unwrap_or(u64::MAX)
        .max(1);
    let from_h = (h.saturating_mul(frac) / 100).max(1);
    // Width candidate: largest font s.t. n_chars * s *
    // AVG_GLYPH_WIDTH_PCT / 100 <= w
    //   → s <= w * 100 / (n_chars * AVG_GLYPH_WIDTH_PCT).
    let denom = n_chars.saturating_mul(AVG_GLYPH_WIDTH_PCT).max(1);
    let from_w = (w.saturating_mul(100) / denom).max(1);
    let chosen = from_h.min(from_w);
    let narrowed = u32::try_from(chosen).unwrap_or(u32::MAX);
    narrowed.max(MIN_LEGIBLE_PX)
}

fn centered_baseline_y(bounds: Rect, font: u32) -> u32 {
    // Approximate a visually-centred baseline: place
    // the baseline ~75 % down the glyph so the
    // ascender-to-descender span straddles the bounds'
    // midline.
    let centre = bounds.y + bounds.h / 2;
    centre + font * 35 / 100
}

fn render_centered_text(bounds: Rect, size_px: u32, content: &str) -> String {
    text(
        bounds.x + bounds.w / 2,
        centered_baseline_y(bounds, size_px),
        size_px,
        "middle",
        "",
        content,
    )
}

fn render_weather_icon(bounds: Rect, condition: Option<Condition>) -> String {
    let Some(condition) = condition else {
        // No data → emit a centred em-dash placeholder
        // so the cell matches the missing-data
        // convention used by every other widget
        // (documented at module top).
        let size = fit_font_px(bounds, FONT_FRACTION_MEDIUM, PLACEHOLDER);
        return render_centered_text(bounds, size, PLACEHOLDER);
    };
    let min_dim = bounds.w.min(bounds.h);
    // Icons are embedded as full SVG documents with
    // their own viewBox; positioning/sizing is handled
    // by a nested <svg x y width height> wrapper rather
    // than a <g transform>. The inner viewBox scales
    // to fill the wrapper's box, preserving aspect.
    let sz = min_dim * ICON_FRACTION / 100;
    let tx = bounds.x + bounds.w.saturating_sub(sz) / 2;
    let ty = bounds.y + bounds.h.saturating_sub(sz) / 2;
    let icon = icons::icon_for(condition);
    format!(
        "<svg x=\"{tx}\" y=\"{ty}\" width=\"{sz}\" height=\"{sz}\">{icon}</svg>"
    )
}

fn render_temp_now(bounds: Rect, temp_c: Option<f64>) -> String {
    let content = temp_c.map_or_else(
        || PLACEHOLDER.to_owned(),
        |t| format!("{}°", round_i32(t)),
    );
    let size = fit_font_px(bounds, FONT_FRACTION_BIG, &content);
    render_centered_text(bounds, size, &content)
}

fn render_condition(bounds: Rect, condition: Option<Condition>) -> String {
    let content = condition.map_or(PLACEHOLDER, Condition::label);
    let size = fit_font_px(bounds, FONT_FRACTION_MEDIUM, content);
    render_centered_text(bounds, size, content)
}

fn render_feels_like(bounds: Rect, feels_like_c: Option<f64>) -> String {
    let content = feels_like_c.map_or_else(
        || format!("Feels like {PLACEHOLDER}"),
        |t| format!("Feels like {}°", round_i32(t)),
    );
    let size = fit_font_px(bounds, FONT_FRACTION_SMALL, &content);
    render_centered_text(bounds, size, &content)
}

fn render_day_name(
    bounds: Rect,
    ctx: &RenderContext<'_>,
    day: DaySelector,
) -> String {
    let content = match day {
        DaySelector::Today => "Today".to_owned(),
        DaySelector::Offset(n) => {
            ctx.model.day_weekdays.get(usize::from(n)).map_or_else(
                || PLACEHOLDER.to_owned(),
                |w| weekday_label(*w).to_owned(),
            )
        }
    };
    let size = fit_font_px(bounds, FONT_FRACTION_MEDIUM, &content);
    render_centered_text(bounds, size, &content)
}

fn render_labelled_temp(
    bounds: Rect,
    value: Option<i32>,
    label: Option<&str>,
) -> String {
    let number = match value {
        Some(n) => format!("{n}°"),
        None => PLACEHOLDER.to_owned(),
    };
    let content = match label {
        Some(prefix) if !prefix.is_empty() => format!("{prefix} {number}"),
        _ => number,
    };
    let size = fit_font_px(bounds, FONT_FRACTION_MEDIUM, &content);
    render_centered_text(bounds, size, &content)
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

// ─── Footer widgets ────────────────────────────────

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
