//! Build a [`DashboardModel`] from a
//! [`WeatherSnapshot`] and a [`ModelContext`].
//!
//! Pure functions over already-normalised data (°C,
//! km/h, compass degrees). Unit conversion lives in
//! the provider adapter; see `crate::clients::open_meteo`.

use std::collections::HashMap;

use chrono::{DateTime, Datelike, Days, NaiveDate, Utc, Weekday};
use chrono_tz::Tz;

use super::super::astro;
use super::super::classify::{
    Compass8, ConditionCategory, RAIN_THRESHOLD_MMH, WeatherCode,
    classify_category,
};
use super::super::feels_like::apparent_temperature_c;
use super::types::{
    CurrentConditions, DAY_TILE_COUNT, DashboardModel, DaySummary,
    MIN_SAMPLES_PER_DAY, ModelContext, TodaySummary,
};
use crate::telemetry::battery_voltage_to_pct;
use crate::weather::WeatherSnapshot;

/// Build a [`DashboardModel`] from a [`WeatherSnapshot`]
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
pub fn build_model(
    snapshot: &WeatherSnapshot,
    ctx: ModelContext,
) -> DashboardModel {
    warn_on_missing_condition_series(snapshot);
    let current = build_current(snapshot, ctx.now);
    let today = build_today(snapshot, ctx);
    let days = build_days(snapshot, ctx.tz, ctx.now);
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

fn forecast_tile_weekdays(
    tz: Tz,
    now: DateTime<Utc>,
) -> [Weekday; DAY_TILE_COUNT] {
    let today_local = now.with_timezone(&tz).date_naive();
    let mut out = [Weekday::Mon; DAY_TILE_COUNT];
    for (offset, slot) in out.iter_mut().enumerate() {
        if let Some(date) =
            today_local.checked_add_days(Days::new(offset as u64 + 1))
        {
            *slot = date.weekday();
        }
    }
    out
}

fn warn_on_missing_condition_series(snapshot: &WeatherSnapshot) {
    let clouds_absent = snapshot.cloud_cover_pct().iter().all(Option::is_none);
    let precip_absent = snapshot.precip_mm().iter().all(Option::is_none);
    if clouds_absent && precip_absent {
        tracing::warn!(
            "weather snapshot missing both clouds and precip series; \
             dashboard conditions will default to Cloudy",
        );
    }
}

fn build_current(
    snapshot: &WeatherSnapshot,
    now: DateTime<Utc>,
) -> Option<CurrentConditions> {
    let idx = nearest_sample_index(snapshot, now)?;
    let temp_c = sample_at(snapshot.temperature_c(), idx)?;
    let weather_code = snapshot.weather_code().get(idx).copied().flatten();
    // `classify_category` treats cloud=100 / precip=0 as
    // the "no data" fallback sink, which lands on
    // `Cloudy` — matching the pre-refactor convention
    // where missing cloud data defaulted to Cloudy.
    let cloud_pct = sample_at(snapshot.cloud_cover_pct(), idx).unwrap_or(100.0);
    let precip_mmh = sample_at(snapshot.precip_mm(), idx).unwrap_or(0.0);
    let category = classify_category(weather_code, cloud_pct, precip_mmh);
    let wind_kmh = sample_at(snapshot.wind_kmh(), idx).unwrap_or(0.0);
    let wind_compass = sample_at(snapshot.wind_dir_deg(), idx)
        .map_or(Compass8::N, Compass8::from_degrees);
    let gust_kmh = sample_at(snapshot.gust_kmh(), idx);
    let humidity_pct = sample_at(snapshot.humidity_pct(), idx);
    let feels_like_c = apparent_temperature_c(temp_c, humidity_pct, wind_kmh);
    Some(CurrentConditions {
        temp_c,
        feels_like_c,
        category,
        weather_code,
        wind_kmh,
        wind_compass,
        gust_kmh,
        humidity_pct,
    })
}

fn build_today(
    snapshot: &WeatherSnapshot,
    ctx: ModelContext,
) -> Option<TodaySummary> {
    let today_local = ctx.now.with_timezone(&ctx.tz).date_naive();
    let samples_by_date = group_sample_indices_by_date(snapshot, ctx.tz);
    let indices = samples_by_date.get(&today_local)?;
    let high_c = day_high_celsius(indices, snapshot.temperature_c());
    let low_c = day_low_celsius(indices, snapshot.temperature_c());
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
/// `now`. Uses raw Unix seconds with saturating
/// arithmetic because chrono's `DateTime - DateTime`
/// panics on `TimeDelta` overflow, and a malformed
/// timestamp near `i64::MIN`/`i64::MAX` must not
/// crash the publish loop.
fn nearest_sample_index(
    snapshot: &WeatherSnapshot,
    now: DateTime<Utc>,
) -> Option<usize> {
    let now_s = now.timestamp();
    snapshot
        .timestamps()
        .iter()
        .enumerate()
        .min_by_key(|(_, ts)| {
            ts.timestamp().saturating_sub(now_s).saturating_abs()
        })
        .map(|(i, _)| i)
}

/// Look up `series[idx]`, treating "index out of
/// bounds" and "None at index" as equivalent
/// absences.
fn sample_at(series: &[Option<f64>], idx: usize) -> Option<f64> {
    series.get(idx).copied().flatten()
}

fn build_days(
    snapshot: &WeatherSnapshot,
    tz: Tz,
    now: DateTime<Utc>,
) -> [Option<DaySummary>; DAY_TILE_COUNT] {
    let today_local = now.with_timezone(&tz).date_naive();
    let samples_by_date = group_sample_indices_by_date(snapshot, tz);

    let mut out: [Option<DaySummary>; DAY_TILE_COUNT] = Default::default();
    for (offset, slot) in out.iter_mut().enumerate() {
        let target = today_local.checked_add_days(Days::new(offset as u64 + 1));
        let Some(date) = target else { continue };
        let Some(indices) = samples_by_date.get(&date) else {
            continue;
        };
        if indices.len() < MIN_SAMPLES_PER_DAY {
            continue;
        }
        *slot = Some(summarize_day(
            date,
            indices,
            snapshot.temperature_c(),
            snapshot.cloud_cover_pct(),
            snapshot.precip_mm(),
            snapshot.weather_code(),
        ));
    }
    out
}

pub(crate) fn group_sample_indices_by_date(
    snapshot: &WeatherSnapshot,
    tz: Tz,
) -> HashMap<NaiveDate, Vec<usize>> {
    let mut by_date: HashMap<NaiveDate, Vec<usize>> = HashMap::new();
    for (i, ts) in snapshot.timestamps().iter().enumerate() {
        let d = ts.with_timezone(&tz).date_naive();
        by_date.entry(d).or_default().push(i);
    }
    by_date
}

fn summarize_day(
    date: NaiveDate,
    indices: &[usize],
    temps: &[Option<f64>],
    clouds: &[Option<f64>],
    precip: &[Option<f64>],
    codes: &[Option<WeatherCode>],
) -> DaySummary {
    // Representative code: the first `Some` entry across
    // the day's hours. Good-enough default — most days
    // carry a consistent code; detailed glyph quality
    // only matters once at least one hour has one.
    let weather_code = indices
        .iter()
        .find_map(|&i| codes.get(i).copied().flatten());
    DaySummary {
        weekday: date.weekday(),
        high_c: day_high_celsius(indices, temps),
        low_c: day_low_celsius(indices, temps),
        category: day_category(indices, clouds, precip, weather_code),
        weather_code,
    }
}

fn day_high_celsius(indices: &[usize], temps: &[Option<f64>]) -> Option<i32> {
    day_extreme_celsius(indices, temps, Extreme::High)
}

fn day_low_celsius(indices: &[usize], temps: &[Option<f64>]) -> Option<i32> {
    day_extreme_celsius(indices, temps, Extreme::Low)
}

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

fn day_extreme_celsius(
    indices: &[usize],
    temps: &[Option<f64>],
    which: Extreme,
) -> Option<i32> {
    let extreme_c = indices
        .iter()
        .filter_map(|&i| temps.get(i).copied().flatten())
        .fold(which.identity(), |acc, x| which.reduce(acc, x));
    round_celsius(extreme_c)
}

fn round_celsius(c: f64) -> Option<i32> {
    if !c.is_finite() {
        return None;
    }
    #[allow(clippy::cast_possible_truncation)]
    let rounded = c.round() as i32;
    Some(rounded)
}

/// Day-level [`ConditionCategory`] producer. Preserves
/// the pre-refactor precedence:
///
/// 1. If the day has a representative WMO code,
///    [`classify_category`] lets the code dominate (its
///    coarsening is the category).
/// 2. Else any hour above the rain threshold promotes to
///    `Rain` (via `effective_precip = threshold`).
/// 3. Else the cloud mean picks between `Clear` /
///    `PartlyCloudy` / `Cloudy`; `None`-cloud days land
///    on `Cloudy` (same as the old `classify_sample`
///    default).
fn day_category(
    indices: &[usize],
    clouds: &[Option<f64>],
    precip: &[Option<f64>],
    weather_code: Option<WeatherCode>,
) -> ConditionCategory {
    let rainy = indices.iter().any(|&i| {
        precip
            .get(i)
            .copied()
            .flatten()
            .is_some_and(|v| v >= RAIN_THRESHOLD_MMH)
    });
    let cloud_mean = mean(
        indices
            .iter()
            .filter_map(|&i| clouds.get(i).copied().flatten()),
    )
    .unwrap_or(100.0);
    let effective_precip = if rainy { RAIN_THRESHOLD_MMH } else { 0.0 };
    classify_category(weather_code, cloud_mean, effective_precip)
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
