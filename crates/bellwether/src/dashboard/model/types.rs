//! Presentation-layer types produced by
//! [`super::build_model`] and consumed by the SVG
//! builder. Split out of the module root so the
//! builder logic reads as plain functions and the
//! types are easy to eyeball in isolation.

use chrono::{DateTime, NaiveTime, Utc, Weekday};
use chrono_tz::Tz;

use super::super::astro::GeoPoint;
use super::super::classify::{Compass8, ConditionCategory, WeatherCode};
use crate::telemetry::DeviceTelemetry;

/// Minimum number of hourly samples a day must
/// contain for its tile to show data instead of a
/// placeholder. The first and last days in a
/// forecast window are often partial; with fewer
/// than 6 samples the "high" temperature is a
/// non-representative snapshot, not a daily peak.
pub const MIN_SAMPLES_PER_DAY: usize = 6;

/// Number of day tiles the dashboard renders along
/// its bottom row.
pub const DAY_TILE_COUNT: usize = 3;

/// Everything [`super::build_model`] needs beyond
/// the snapshot itself — the configured timezone for
/// day bucketing, the geographic location for
/// astronomical calculations, a reference "current
/// time" so tests can inject a fixed clock, and the
/// last-reported device telemetry for the battery
/// indicator.
#[derive(Debug, Clone, Copy)]
pub struct ModelContext {
    /// IANA timezone for all local-date bucketing
    /// and sunrise/sunset display.
    pub tz: Tz,
    /// Geographic point of interest. Forwarded to
    /// the astro module for sunrise/sunset.
    pub location: GeoPoint,
    /// Reference time used for "nearest sample"
    /// selection, day bucketing, and astro
    /// calculations.
    pub now: DateTime<Utc>,
    /// Latest device telemetry, or
    /// `DeviceTelemetry::default()` if no device has
    /// posted yet.
    pub telemetry: DeviceTelemetry,
}

/// Everything the dashboard SVG template needs,
/// already normalised into display units. An
/// `Option`-heavy shape because the renderer must
/// tolerate partial data — a snapshot missing
/// temperature renders with the current-conditions
/// panel collapsed rather than crashing the publish
/// loop.
#[derive(Debug, Clone, PartialEq)]
pub struct DashboardModel {
    /// Current-conditions panel data. `None` when
    /// the snapshot didn't include a usable surface
    /// temperature for the sample closest to the
    /// supplied reference time.
    pub current: Option<CurrentConditions>,
    /// Summary of today's weather and astronomical
    /// events. `None` when the snapshot didn't
    /// cover the local "today" at all.
    pub today: Option<TodaySummary>,
    /// Three day summaries, one per tile along the
    /// bottom of the layout. Positions are fixed
    /// (index 0 = leftmost tile). An entry is `None`
    /// when the snapshot didn't cover that day with
    /// at least [`MIN_SAMPLES_PER_DAY`] samples — the
    /// SVG builder renders a placeholder using the
    /// corresponding [`Self::day_weekdays`] entry so
    /// the user can see *which* day is missing.
    pub days: [Option<DaySummary>; DAY_TILE_COUNT],
    /// Weekday labels for each forecast tile,
    /// independent of whether the tile itself has
    /// data. Always populated from `ctx.now` by
    /// [`super::build_model`] so a missing forecast
    /// tile still carries a "Sat", "Sun", … label
    /// into the SVG.
    pub day_weekdays: [Weekday; DAY_TILE_COUNT],
    /// Battery percentage derived from the most
    /// recent device telemetry. `None` when no
    /// device has posted yet or the last post didn't
    /// include a battery voltage.
    pub battery_pct: Option<u8>,
}

/// Snapshot of the "now" sample's weather.
#[derive(Debug, Clone, PartialEq)]
pub struct CurrentConditions {
    /// Temperature in degrees Celsius. Not
    /// pre-rounded — the SVG builder decides display
    /// precision.
    pub temp_c: f64,
    /// Apparent ("feels like") temperature in
    /// Celsius. Equal to `temp_c` when neither the
    /// heat-index nor wind-chill branch applies (see
    /// `feels_like` module).
    pub feels_like_c: f64,
    /// Weather category, used by the simple-fidelity
    /// icon dispatch and the condition label. Produced
    /// by [`classify_category`](super::super::classify::classify_category)
    /// from the provider's WMO code when available, or
    /// from the cloud+precip heuristic otherwise — a
    /// single unified field so the renderer never has
    /// to pick between two possibly-inconsistent
    /// classifications.
    pub category: ConditionCategory,
    /// Raw provider-supplied WMO 4677 code for the
    /// sample, retained so `fidelity = "detailed"`
    /// widgets can dispatch to a specialised glyph.
    /// `None` means the snapshot had no code for this
    /// hour; detailed fidelity then falls back to the
    /// same coarse icon as [`Self::category`].
    pub weather_code: Option<WeatherCode>,
    /// Wind speed in km/h.
    pub wind_kmh: f64,
    /// Compass octant the wind is blowing *from*.
    pub wind_compass: Compass8,
    /// Wind gust speed in km/h. `None` when the
    /// snapshot's gust series had no datum for the
    /// sample.
    pub gust_kmh: Option<f64>,
    /// Relative humidity percentage (0–100). `None`
    /// when the snapshot's humidity series had no
    /// datum for the sample.
    pub humidity_pct: Option<f64>,
}

/// Summary panel for today: high/low temperatures
/// derived from every snapshot sample whose local
/// date matches today, plus the sunrise/sunset times
/// from the NOAA algorithm in
/// [`super::super::astro`].
#[derive(Debug, Clone, PartialEq)]
pub struct TodaySummary {
    /// Rounded daily high temperature in Celsius.
    /// `None` when every sample for today was null
    /// (or the temp series was empty).
    pub high_c: Option<i32>,
    /// Rounded daily low temperature in Celsius.
    /// `None` when every sample for today was null.
    pub low_c: Option<i32>,
    /// Local-time sunrise. `None` during polar night
    /// or if the astro calculation yields a
    /// non-finite result.
    pub sunrise_local: Option<NaiveTime>,
    /// Local-time sunset. `None` during polar day.
    pub sunset_local: Option<NaiveTime>,
}

/// Per-day forecast tile contents: weekday, rounded
/// daily high / low temperatures, and the
/// qualitative condition that drives the icon
/// choice.
#[derive(Debug, Clone, PartialEq)]
pub struct DaySummary {
    /// Local-time weekday the tile represents.
    pub weekday: Weekday,
    /// Rounded daily high temperature in Celsius, or
    /// `None` when the day's temperature series was
    /// entirely null.
    pub high_c: Option<i32>,
    /// Rounded daily low temperature in Celsius, or
    /// `None` when the day's temperature series was
    /// entirely null.
    pub low_c: Option<i32>,
    /// Daily weather category. Prefers the
    /// representative WMO code (first `Some` entry in
    /// the day's hours) when present; otherwise derives
    /// from the cloud-mean + any-rainy-hour heuristic —
    /// same rules as the old `Condition`-based path, now
    /// expressed directly in the richer taxonomy.
    pub category: ConditionCategory,
    /// Representative WMO 4677 code for the day (the
    /// first `Some` entry across the day's samples, if
    /// any). `None` when no hour in the day carried a
    /// provider code; detailed-fidelity dispatch then
    /// coarsens through [`Self::category`].
    pub weather_code: Option<WeatherCode>,
}
