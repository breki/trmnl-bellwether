//! [`DashboardModel`] — presentation-ready view of a
//! [`Forecast`].
//!
//! Folds the raw Windy output plus device telemetry
//! into what the SVG template needs: current
//! conditions, a summary for today, and three
//! forecast tiles for the next three calendar days.
//! All Windy → human conversions (Kelvin → Celsius,
//! wind-vector → compass, m/s → km/h, feels-like
//! math, NOAA sunrise/sunset) happen here so the
//! SVG builder is pure string templating.

use std::collections::HashMap;

use chrono::{DateTime, Datelike, Days, NaiveDate, NaiveTime, Utc, Weekday};
use chrono_tz::Tz;

use super::astro::{self, GeoPoint};
use super::classify::{Compass8, Condition, classify_weather, wind_to_compass};
use super::feels_like::apparent_temperature_c;
use crate::clients::windy::Forecast;
use crate::config::WindyParameter;
use crate::telemetry::{DeviceTelemetry, battery_voltage_to_pct};

/// Minimum number of hourly samples a day must
/// contain for its tile to show data instead of a
/// placeholder. Windy's first and last days in a
/// forecast window are often partial; with fewer
/// than 6 samples the "high" temperature is a
/// non-representative snapshot, not a daily peak.
pub const MIN_SAMPLES_PER_DAY: usize = 6;

/// Number of day tiles the dashboard renders along
/// its bottom row.
pub const DAY_TILE_COUNT: usize = 3;

/// Conversion constant: Kelvin to Celsius. Hoisted
/// so the temperature math reads in one place and
/// tests can depend on the same constant the code
/// uses.
const KELVIN_TO_CELSIUS: f64 = 273.15;

/// Conversion constant: metres-per-second to km/h.
const MS_TO_KMH: f64 = 3.6;

/// Everything [`build_model`] needs beyond the
/// forecast itself — the configured timezone for
/// day bucketing, the geographic location for
/// astronomical calculations, a reference "current
/// time" so tests can inject a fixed clock, and the
/// last-reported device telemetry for the battery
/// indicator.
///
/// Copy-able so call sites can pass by value without
/// lifetime juggling and tests can spread it across
/// fixtures cheaply.
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
/// already normalised from Windy's wire format into
/// display units. An `Option`-heavy shape because
/// the renderer must tolerate partial data — a
/// forecast missing temperature renders with the
/// current-conditions panel collapsed rather than
/// crashing the publish loop.
#[derive(Debug, Clone, PartialEq)]
pub struct DashboardModel {
    /// Current-conditions panel data. `None` when
    /// the forecast didn't include a usable surface
    /// temperature for the sample closest to the
    /// supplied reference time.
    pub current: Option<CurrentConditions>,
    /// Summary of today's weather and astronomical
    /// events. `None` when the forecast didn't
    /// cover the local "today" at all.
    pub today: Option<TodaySummary>,
    /// Three day summaries, one per tile along the
    /// bottom of the layout. Positions are fixed
    /// (index 0 = leftmost tile). An entry is `None`
    /// when the forecast didn't cover that day with
    /// at least [`MIN_SAMPLES_PER_DAY`] samples — the
    /// SVG builder renders a placeholder using the
    /// corresponding [`Self::day_weekdays`] entry so
    /// the user can see *which* day is missing.
    pub days: [Option<DaySummary>; DAY_TILE_COUNT],
    /// Weekday labels for each forecast tile,
    /// independent of whether the tile itself has
    /// data. Always populated from `ctx.now` by
    /// [`build_model`] so a missing forecast tile
    /// still carries a "Sat", "Sun", … label into
    /// the SVG.
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
    /// Qualitative condition, used to pick the icon
    /// and the one-word label ("Sunny", "Cloudy",
    /// …).
    pub condition: Condition,
    /// Wind speed in km/h, converted from Windy's
    /// m/s.
    pub wind_kmh: f64,
    /// Compass octant the wind is blowing *from*.
    pub wind_compass: Compass8,
    /// Wind gust speed in km/h. `None` when Windy
    /// didn't populate the gust series for the
    /// sample.
    pub gust_kmh: Option<f64>,
    /// Relative humidity percentage (0–100). `None`
    /// when Windy didn't populate the RH series for
    /// the sample.
    pub humidity_pct: Option<f64>,
}

/// Summary panel for today: high/low temperatures
/// derived from every forecast sample whose local
/// date matches today, plus the sunrise/sunset times
/// from the NOAA algorithm in [`super::astro`].
#[derive(Debug, Clone, PartialEq)]
pub struct TodaySummary {
    /// Rounded daily high temperature in Celsius.
    /// `None` when every sample for today was null
    /// (or the temp series was absent).
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
    /// Stored as the typed [`Weekday`] so the
    /// "labels are always English" invariant lives
    /// in exactly one place (the SVG builder's
    /// formatter) rather than being implicit in
    /// whatever `format!("{}", w)` did at
    /// `build_model` time.
    pub weekday: Weekday,
    /// Rounded daily high temperature in Celsius, or
    /// `None` when the day's temperature series was
    /// entirely null.
    pub high_c: Option<i32>,
    /// Rounded daily low temperature in Celsius, or
    /// `None` when the day's temperature series was
    /// entirely null.
    pub low_c: Option<i32>,
    /// Daily condition: [`Condition::Rain`] if any
    /// sample reached the precipitation threshold,
    /// otherwise classified from the day's mean
    /// cloud cover.
    pub condition: Condition,
}

/// Build a [`DashboardModel`] from a [`Forecast`]
/// and a [`ModelContext`].
///
/// Day selection for the forecast-tile row is
/// **skip today, then next 3**: at 23:55 local time
/// the current day's max is a stale fact about a
/// day that's nearly over; forward-looking tiles
/// stay stable across the midnight boundary. The
/// separate `today` summary covers the current
/// local date.
#[must_use]
pub fn build_model(forecast: &Forecast, ctx: ModelContext) -> DashboardModel {
    warn_on_missing_condition_series(forecast);
    let current = build_current(forecast, ctx.now);
    let today = build_today(forecast, ctx);
    let days = build_days(forecast, ctx.tz, ctx.now);
    let day_weekdays = forecast_tile_weekdays(ctx.tz, ctx.now);
    let battery_pct = ctx
        .telemetry
        .battery_voltage
        .and_then(battery_voltage_to_pct);
    DashboardModel {
        current,
        today,
        days,
        day_weekdays,
        battery_pct,
    }
}

/// Compute the weekday labels for the three forecast
/// tiles, independent of whether `forecast` has data
/// for each target date. Follows the same
/// "skip-today, next 3" rule as [`build_days`]. Used
/// so a missing day tile still renders its weekday
/// header in the SVG.
fn forecast_tile_weekdays(
    tz: Tz,
    now: DateTime<Utc>,
) -> [Weekday; DAY_TILE_COUNT] {
    let today_local = now.with_timezone(&tz).date_naive();
    let mut out = [Weekday::Mon; DAY_TILE_COUNT];
    for (offset, slot) in out.iter_mut().enumerate() {
        // `offset + 1` gives "skip today, then next 3".
        if let Some(date) =
            today_local.checked_add_days(Days::new(offset as u64 + 1))
        {
            *slot = date.weekday();
        }
    }
    out
}

fn warn_on_missing_condition_series(forecast: &Forecast) {
    let has_clouds = forecast.series.contains_key("clouds-surface");
    let has_precip = forecast.series.contains_key("precip-surface");
    if !has_clouds && !has_precip {
        tracing::warn!(
            "Windy forecast missing both clouds and precip series; \
             dashboard conditions will default to Cloudy",
        );
    }
}

/// Build [`CurrentConditions`] from the forecast
/// sample whose timestamp sits closest to `now`. If
/// temperature is missing at that sample we collapse
/// the whole panel rather than showing a tile with a
/// real wind reading and a question-mark for temp —
/// the temperature is the visual anchor and an empty
/// panel reads more honestly than a partially-filled
/// one. Humidity and gust are optional; their
/// absence doesn't collapse the panel.
fn build_current(
    forecast: &Forecast,
    now: DateTime<Utc>,
) -> Option<CurrentConditions> {
    let idx = nearest_sample_index(forecast, now)?;
    let temp_k = sample_value(forecast, WindyParameter::Temp, idx)?;
    let temp_c = temp_k - KELVIN_TO_CELSIUS;
    let condition = classify_at(forecast, idx);
    let (wind_kmh, wind_compass) = wind_components_at(forecast, idx)
        .map_or((0.0, Compass8::N), |(u, v)| wind_to_compass(u, v));
    let gust_kmh = sample_value(forecast, WindyParameter::WindGust, idx)
        .map(|ms| ms * MS_TO_KMH);
    // Clamp humidity into the physically meaningful
    // `[0, 100]` range before feeding it downstream.
    // Windy glitches have been seen emitting >100
    // values; the Rothfusz heat-index formula extrapolates
    // wildly when fed them.
    let humidity_pct = sample_value(forecast, WindyParameter::Rh, idx)
        .map(|rh| rh.clamp(0.0, 100.0));
    let feels_like_c = apparent_temperature_c(temp_c, humidity_pct, wind_kmh);
    Some(CurrentConditions {
        temp_c,
        feels_like_c,
        condition,
        wind_kmh,
        wind_compass,
        gust_kmh,
        humidity_pct,
    })
}

/// Build today's summary from every forecast sample
/// whose local date matches `now`'s local date.
/// High/low derived from the full set — including
/// already-past samples — so the panel stays stable
/// across the day and is a single-glance summary of
/// "today's weather" rather than a forward-only
/// forecast.
fn build_today(forecast: &Forecast, ctx: ModelContext) -> Option<TodaySummary> {
    let today_local = ctx.now.with_timezone(&ctx.tz).date_naive();
    let samples_by_date = group_sample_indices_by_date(forecast, ctx.tz);
    let indices = samples_by_date.get(&today_local)?;
    let temps = forecast.values(WindyParameter::Temp);
    let high_c = day_high_celsius(indices, temps);
    let low_c = day_low_celsius(indices, temps);
    let (sunrise_local, sunset_local) =
        astro::sunrise_sunset(today_local, ctx.location, ctx.tz);
    Some(TodaySummary {
        high_c,
        low_c,
        sunrise_local,
        sunset_local,
    })
}

/// Find the sample whose timestamp is nearest to
/// `now` (smallest `|ts - now|`). Windy returns
/// model-grid timestamps in monotonically increasing
/// order but we can't assume the first step is close
/// to wall-clock: a model run from a few hours ago
/// may have `ts[0]` in the past, a just-released run
/// may have `ts[0]` a few hours in the future.
fn nearest_sample_index(
    forecast: &Forecast,
    now: DateTime<Utc>,
) -> Option<usize> {
    forecast
        .timestamps
        .iter()
        .enumerate()
        // `saturating_abs` rather than `abs` so a
        // malformed timestamp near `i64::MIN` doesn't
        // panic the publish loop.
        .min_by_key(|(_, ts)| (**ts - now).num_seconds().saturating_abs())
        .map(|(i, _)| i)
}

fn classify_at(forecast: &Forecast, idx: usize) -> Condition {
    let cloud = sample_value(forecast, WindyParameter::Clouds, idx);
    let precip = sample_value(forecast, WindyParameter::Precip, idx);
    classify_sample(cloud, precip)
}

fn classify_sample(
    cloud_pct: Option<f64>,
    precip_mmh: Option<f64>,
) -> Condition {
    if let Some(p) = precip_mmh
        && p >= super::classify::RAIN_THRESHOLD_MMH
    {
        return Condition::Rain;
    }
    // Cloud missing but no rain → fall back to
    // Cloudy. build_model already emitted a one-shot
    // warn if the whole series is absent, so we
    // don't spam per sample.
    cloud_pct.map_or(Condition::Cloudy, |c| {
        classify_weather(c, precip_mmh.unwrap_or(0.0))
    })
}

fn sample_value(
    forecast: &Forecast,
    param: WindyParameter,
    idx: usize,
) -> Option<f64> {
    forecast.values(param)?.get(idx).copied().flatten()
}

fn wind_components_at(forecast: &Forecast, idx: usize) -> Option<(f64, f64)> {
    // Wind is the one parameter whose response key
    // doesn't follow the `{wire_name}-surface`
    // pattern Forecast::values expects — Windy
    // splits it into u/v component series. Go
    // through the raw series map to pick them up.
    let u = series_value_at(&forecast.series, "wind_u-surface", idx)?;
    let v = series_value_at(&forecast.series, "wind_v-surface", idx)?;
    Some((u, v))
}

/// Look up `series_map[key][idx]`, treating "missing
/// series", "index out of bounds", and "Windy null
/// at index" as equivalent absences. Centralises the
/// option chain so every caller reads the same shape
/// instead of nested `and_then` / `flatten` calls.
fn series_value_at(
    series_map: &HashMap<String, Vec<Option<f64>>>,
    key: &str,
    idx: usize,
) -> Option<f64> {
    series_map.get(key)?.get(idx).copied().flatten()
}

fn build_days(
    forecast: &Forecast,
    tz: Tz,
    now: DateTime<Utc>,
) -> [Option<DaySummary>; DAY_TILE_COUNT] {
    let today_local = now.with_timezone(&tz).date_naive();
    let samples_by_date = group_sample_indices_by_date(forecast, tz);
    let temps = forecast.values(WindyParameter::Temp);
    let clouds = forecast.values(WindyParameter::Clouds);
    let precip = forecast.values(WindyParameter::Precip);

    let mut out: [Option<DaySummary>; DAY_TILE_COUNT] = Default::default();
    for (offset, slot) in out.iter_mut().enumerate() {
        // `offset + 1` gives "skip today, then next 3".
        let target = today_local.checked_add_days(Days::new(offset as u64 + 1));
        let Some(date) = target else { continue };
        let Some(indices) = samples_by_date.get(&date) else {
            continue;
        };
        if indices.len() < MIN_SAMPLES_PER_DAY {
            continue;
        }
        *slot = Some(summarize_day(date, indices, temps, clouds, precip));
    }
    out
}

fn group_sample_indices_by_date(
    forecast: &Forecast,
    tz: Tz,
) -> HashMap<NaiveDate, Vec<usize>> {
    let mut by_date: HashMap<NaiveDate, Vec<usize>> = HashMap::new();
    for (i, ts) in forecast.timestamps.iter().enumerate() {
        let d = ts.with_timezone(&tz).date_naive();
        by_date.entry(d).or_default().push(i);
    }
    by_date
}

fn summarize_day(
    date: NaiveDate,
    indices: &[usize],
    temps: Option<&[Option<f64>]>,
    clouds: Option<&[Option<f64>]>,
    precip: Option<&[Option<f64>]>,
) -> DaySummary {
    DaySummary {
        weekday: date.weekday(),
        high_c: day_high_celsius(indices, temps),
        low_c: day_low_celsius(indices, temps),
        condition: day_condition(indices, clouds, precip),
    }
}

fn day_high_celsius(
    indices: &[usize],
    temps: Option<&[Option<f64>]>,
) -> Option<i32> {
    day_extreme_celsius(indices, temps, Extreme::High)
}

fn day_low_celsius(
    indices: &[usize],
    temps: Option<&[Option<f64>]>,
) -> Option<i32> {
    day_extreme_celsius(indices, temps, Extreme::Low)
}

/// Which end of a day's temperature range to pick.
/// The identity element for the fold (`-∞` for max,
/// `+∞` for min) and the reducer are coupled at the
/// type level rather than accepting two correlated
/// `f64`s from the caller.
#[derive(Clone, Copy)]
enum Extreme {
    High,
    Low,
}

impl Extreme {
    fn identity(self) -> f64 {
        match self {
            Self::High => f64::NEG_INFINITY,
            Self::Low => f64::INFINITY,
        }
    }

    fn reduce(self, acc: f64, x: f64) -> f64 {
        match self {
            Self::High => acc.max(x),
            Self::Low => acc.min(x),
        }
    }
}

/// Shared implementation for `day_high_celsius` /
/// `day_low_celsius`. Filters out early when `temps`
/// is `None` so we don't iterate indices pointlessly.
fn day_extreme_celsius(
    indices: &[usize],
    temps: Option<&[Option<f64>]>,
    which: Extreme,
) -> Option<i32> {
    let series = temps?;
    let extreme_k = indices
        .iter()
        .filter_map(|&i| series.get(i).copied().flatten())
        .fold(which.identity(), |acc, x| which.reduce(acc, x));
    kelvin_to_rounded_celsius(extreme_k)
}

fn kelvin_to_rounded_celsius(k: f64) -> Option<i32> {
    if !k.is_finite() {
        return None;
    }
    #[allow(clippy::cast_possible_truncation)]
    let c = (k - KELVIN_TO_CELSIUS).round() as i32;
    Some(c)
}

fn day_condition(
    indices: &[usize],
    clouds: Option<&[Option<f64>]>,
    precip: Option<&[Option<f64>]>,
) -> Condition {
    // Rain if any sample reaches the threshold.
    if let Some(p) = precip
        && indices.iter().any(|&i| {
            p.get(i)
                .copied()
                .flatten()
                .is_some_and(|v| v >= super::classify::RAIN_THRESHOLD_MMH)
        })
    {
        return Condition::Rain;
    }
    // Otherwise classify on mean cloud cover.
    let cloud_mean = clouds.and_then(|series| {
        mean(
            indices
                .iter()
                .filter_map(|&i| series.get(i).copied().flatten()),
        )
    });
    classify_sample(cloud_mean, None)
}

fn mean<I: IntoIterator<Item = f64>>(values: I) -> Option<f64> {
    let mut sum = 0.0_f64;
    let mut count = 0_usize;
    for v in values {
        sum += v;
        count += 1;
    }
    if count == 0 {
        None
    } else {
        #[allow(clippy::cast_precision_loss)]
        let divisor = count as f64;
        Some(sum / divisor)
    }
}

#[cfg(test)]
mod tests;
