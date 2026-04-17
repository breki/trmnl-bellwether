//! [`DashboardModel`] — presentation-ready view of a
//! [`Forecast`].
//!
//! Folds the raw Windy output into what the SVG
//! template actually needs: current conditions (big
//! panel at the top of the dashboard) and three
//! summaries for the next three calendar days (tiles
//! along the bottom). All Windy → human conversions
//! (Kelvin → Celsius, wind-vector → compass, etc.)
//! happen in [`build_model`] so the SVG builder is
//! pure string templating.

use std::collections::HashMap;

use chrono::{DateTime, Datelike, Days, NaiveDate, Utc, Weekday};
use chrono_tz::Tz;

use super::classify::{Compass8, Condition, classify_weather, wind_to_compass};
use crate::clients::windy::Forecast;
use crate::config::WindyParameter;

/// Minimum number of hourly samples a day must contain
/// for its tile to show data instead of a placeholder.
/// Windy's first and last days in a forecast window are
/// often partial (only the hours inside the forecast
/// horizon are covered); with fewer than 6 samples the
/// "high" temperature is a non-representative snapshot,
/// not a daily peak.
pub const MIN_SAMPLES_PER_DAY: usize = 6;

/// Number of day tiles the dashboard renders along its
/// bottom row.
pub const DAY_TILE_COUNT: usize = 3;

/// Conversion constant: Kelvin to Celsius. Hoisted so
/// the temperature math reads in one place and tests
/// can depend on the same constant the code uses.
const KELVIN_TO_CELSIUS: f64 = 273.15;

/// Everything the dashboard SVG template needs, already
/// normalised from Windy's wire format into display
/// units. An `Option`-heavy shape because the renderer
/// must tolerate partial data — a forecast missing
/// temperature renders with the current-conditions
/// panel collapsed rather than crashing the publish
/// loop.
#[derive(Debug, Clone, PartialEq)]
pub struct DashboardModel {
    /// Current-conditions panel data. `None` when the
    /// forecast didn't include a usable surface
    /// temperature for the sample closest to the
    /// supplied reference time.
    pub current: Option<CurrentConditions>,
    /// Three day summaries, one per tile along the
    /// bottom of the layout. Positions are fixed
    /// (index 0 = leftmost tile). An entry is `None`
    /// when the forecast didn't cover that day with
    /// at least [`MIN_SAMPLES_PER_DAY`] samples.
    pub days: [Option<DaySummary>; DAY_TILE_COUNT],
}

/// Snapshot of the "now" sample's weather: the big
/// temperature reading and wind label that sit at the
/// top of the dashboard.
#[derive(Debug, Clone, PartialEq)]
pub struct CurrentConditions {
    /// Temperature in degrees Celsius. Not pre-rounded
    /// — the SVG builder decides display precision.
    pub temp_c: f64,
    /// Qualitative condition, used to pick the icon
    /// and the one-word label ("Sunny", "Cloudy", …).
    pub condition: Condition,
    /// Wind speed in km/h, converted from Windy's m/s.
    pub wind_kmh: f64,
    /// Compass octant the wind is blowing *from*.
    pub wind_compass: Compass8,
}

/// Per-day forecast tile contents: weekday, rounded
/// daily high temperature (if the temperature series
/// was populated for that day), and the qualitative
/// condition that drives the icon choice.
#[derive(Debug, Clone, PartialEq)]
pub struct DaySummary {
    /// Local-time weekday the tile represents. Kept as
    /// the typed [`Weekday`] rather than a pre-rendered
    /// string so the dashboard's "labels are always
    /// English" invariant lives in exactly one place
    /// (the SVG builder's formatter) rather than being
    /// implicit in whatever `format!("{}", w)` did at
    /// `build_model` time.
    pub weekday: Weekday,
    /// Rounded daily high temperature in Celsius, or
    /// `None` when the forecast had enough samples to
    /// populate the tile (per [`MIN_SAMPLES_PER_DAY`])
    /// but every temperature value for the day was
    /// null. The SVG renders this as a placeholder
    /// glyph.
    pub high_c: Option<i32>,
    /// Daily condition: [`Condition::Rain`] if any
    /// sample reached the precipitation threshold,
    /// otherwise classified from the day's mean cloud
    /// cover.
    pub condition: Condition,
}

/// Build a [`DashboardModel`] from a [`Forecast`] and
/// the configured timezone.
///
/// `now` is the reference "current time" used for day
/// bucketing and for picking which forecast sample
/// backs the current-conditions panel — passed
/// explicitly rather than calling `Utc::now()`
/// internally so tests can inject a fixed reference
/// and the publish loop can reuse a single timestamp
/// across fetch/render/publish.
///
/// Day selection is **skip today, then next 3**: if
/// `now` is Tuesday local, the tiles label Wednesday,
/// Thursday, Friday. Rationale — at 23:55 the current
/// day's max is a stale fact about a day that's nearly
/// over; forward-looking tiles stay stable across the
/// midnight boundary.
#[must_use]
pub fn build_model(
    forecast: &Forecast,
    tz: Tz,
    now: DateTime<Utc>,
) -> DashboardModel {
    warn_on_missing_condition_series(forecast);
    let current = build_current(forecast, now);
    let days = build_days(forecast, tz, now);
    DashboardModel { current, days }
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

/// Build [`CurrentConditions`] from the forecast sample
/// whose timestamp sits closest to `now`. If temperature
/// is missing at that sample we collapse the whole
/// panel rather than showing a tile with a real wind
/// reading and a question-mark for temp — the
/// temperature is the visual anchor and an empty panel
/// reads more honestly than a partially-filled one.
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
    Some(CurrentConditions {
        temp_c,
        condition,
        wind_kmh,
        wind_compass,
    })
}

/// Find the sample whose timestamp is nearest to `now`
/// (smallest `|ts - now|`). Windy returns model-grid
/// timestamps in monotonically increasing order but we
/// can't assume the first step is close to wall-clock:
/// a model run from a few hours ago may have `ts[0]`
/// in the past, a just-released run may have `ts[0]` a
/// few hours in the future.
fn nearest_sample_index(
    forecast: &Forecast,
    now: DateTime<Utc>,
) -> Option<usize> {
    forecast
        .timestamps
        .iter()
        .enumerate()
        .min_by_key(|(_, ts)| (**ts - now).num_seconds().abs())
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
    // Cloud missing but no rain → fall back to Cloudy.
    // build_model already emitted a one-shot warn if the
    // whole series is absent, so we don't spam per
    // sample.
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
    // doesn't follow the `{wire_name}-surface` pattern
    // Forecast::values expects — Windy splits it into
    // u/v component series. Go through the raw series
    // map to pick them up.
    let u = series_value_at(&forecast.series, "wind_u-surface", idx)?;
    let v = series_value_at(&forecast.series, "wind_v-surface", idx)?;
    Some((u, v))
}

/// Look up `series_map[key][idx]`, treating "missing
/// series", "index out of bounds", and "Windy null at
/// index" as equivalent absences. Centralises the
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
        condition: day_condition(indices, clouds, precip),
    }
}

fn day_high_celsius(
    indices: &[usize],
    temps: Option<&[Option<f64>]>,
) -> Option<i32> {
    let high_k = indices
        .iter()
        .filter_map(|&i| temps?.get(i).copied().flatten())
        .fold(f64::NEG_INFINITY, f64::max);
    if !high_k.is_finite() {
        // Every temperature sample for the day was
        // null, or the temp series is entirely absent.
        // The caller renders a placeholder glyph rather
        // than a misleading "0°".
        return None;
    }
    #[allow(clippy::cast_possible_truncation)]
    let c = (high_k - KELVIN_TO_CELSIUS).round() as i32;
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
mod tests {
    use super::*;

    use chrono::TimeZone;
    use serde_json::json;

    /// Build a `Forecast` from a JSON literal —
    /// exercises the live parser, so fixtures can't
    /// drift from the wire contract.
    fn forecast(json: &serde_json::Value) -> Forecast {
        Forecast::from_raw_json(&json.to_string())
            .expect("valid forecast fixture")
    }

    fn utc(ms: i64) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(ms).single().unwrap()
    }

    /// Produce 72 hourly timestamps starting at `start`,
    /// returning a JSON value that mimics a Windy
    /// response with the usual four `*-surface` series
    /// fully populated from the supplied constants.
    /// Tests that want outlier samples edit the JSON
    /// after construction.
    fn hourly_72h(
        start: DateTime<Utc>,
        temp_k: f64,
        cloud: f64,
        precip: f64,
    ) -> serde_json::Value {
        let mut ts: Vec<i64> = Vec::with_capacity(72);
        let mut temp_series: Vec<f64> = Vec::with_capacity(72);
        let mut cloud_series: Vec<f64> = Vec::with_capacity(72);
        let mut precip_series: Vec<f64> = Vec::with_capacity(72);
        for hour in 0..72 {
            ts.push(start.timestamp_millis() + i64::from(hour) * 3_600_000);
            temp_series.push(temp_k);
            cloud_series.push(cloud);
            precip_series.push(precip);
        }
        json!({
            "ts": ts,
            "units": {
                "temp-surface": "K",
                "clouds-surface": "%",
                "precip-surface": "mm/h",
                "wind_u-surface": "m/s",
                "wind_v-surface": "m/s",
            },
            "temp-surface": temp_series,
            "clouds-surface": cloud_series,
            "precip-surface": precip_series,
            "wind_u-surface": vec![3.0_f64; 72],
            "wind_v-surface": vec![4.0_f64; 72],
        })
    }

    #[test]
    fn happy_path_fills_current_and_three_days() {
        // Start the forecast at 2026-04-18 00:00 UTC.
        // With UTC timezone and now=2026-04-17 23:00,
        // "today" is the 17th and the three tiles land
        // on the 18th (Sat), 19th (Sun), 20th (Mon).
        let start = utc(1_776_470_400_000); // 2026-04-18T00:00Z
        let json = hourly_72h(start, 283.15, 10.0, 0.0);
        let f = forecast(&json);
        let now = utc(1_776_423_600_000); // 2026-04-17T11:00Z
        let model = build_model(&f, chrono_tz::UTC, now);

        let current = model.current.expect("current built");
        assert!((current.temp_c - 10.0).abs() < 1e-9);
        assert_eq!(current.condition, Condition::Sunny);
        // wind (u=3, v=4) → speed = 5 m/s = 18 km/h; from SW.
        assert!((current.wind_kmh - 18.0).abs() < 1e-9);
        assert_eq!(current.wind_compass, Compass8::SW);

        for (i, slot) in model.days.iter().enumerate() {
            let s = slot.as_ref().unwrap_or_else(|| panic!("day {i}: None"));
            assert_eq!(s.high_c, Some(10));
            assert_eq!(s.condition, Condition::Sunny);
        }
        assert_eq!(model.days[0].as_ref().unwrap().weekday, Weekday::Sat);
        assert_eq!(model.days[1].as_ref().unwrap().weekday, Weekday::Sun);
        assert_eq!(model.days[2].as_ref().unwrap().weekday, Weekday::Mon);
    }

    #[test]
    fn current_picks_sample_nearest_to_now_not_index_zero() {
        // Forecast starts 6h before now; `now` falls
        // between samples 6 and 7. The nearest-to-now
        // sample should be 6 (exactly aligned) with
        // temp=280 K, not index 0 which sits 6 hours in
        // the past with a cooler temp.
        let start = utc(1_776_402_000_000); // 2026-04-17T05:00Z
        let mut temps = Vec::new();
        let mut ts = Vec::new();
        let mut clouds = Vec::new();
        let mut precip = Vec::new();
        for h in 0..12_i64 {
            ts.push(start.timestamp_millis() + h * 3_600_000);
            // Index 0: 270 K, index 6: 280 K, so a
            // "nearest-to-now" picker lands on the
            // warmer reading.
            #[allow(clippy::cast_precision_loss)]
            let offset_k = h as f64;
            temps.push(270.0 + offset_k);
            clouds.push(10.0);
            precip.push(0.0);
        }
        let json = json!({
            "ts": ts,
            "units": {},
            "temp-surface": temps,
            "clouds-surface": clouds,
            "precip-surface": precip,
            "wind_u-surface": vec![3.0_f64; 12],
            "wind_v-surface": vec![4.0_f64; 12],
        });
        let f = forecast(&json);
        let now = utc(1_776_423_600_000); // 2026-04-17T11:00Z
        let model = build_model(&f, chrono_tz::UTC, now);
        let c = model.current.expect("current built");
        // Sample 6 at 11:00Z has temp 276 K (270+6)
        // → 2.85 °C. Index 0 at 05:00Z would be
        // 270 K → -3.15 °C. Check we picked the right
        // one.
        assert!((c.temp_c - 2.85).abs() < 1e-9, "temp_c={}", c.temp_c);
    }

    #[test]
    fn missing_temperature_collapses_current_panel() {
        let start = utc(1_776_470_400_000);
        let json = json!({
            "ts": [start.timestamp_millis()],
            "units": {},
            "clouds-surface": [10.0],
            "wind_u-surface": [3.0],
            "wind_v-surface": [4.0],
        });
        let f = forecast(&json);
        let now = utc(1_776_423_600_000);
        let model = build_model(&f, chrono_tz::UTC, now);
        assert!(model.current.is_none());
    }

    #[test]
    fn missing_cloud_and_precip_defaults_day_to_cloudy() {
        let start = utc(1_776_470_400_000);
        let mut ts = Vec::new();
        let mut temps = Vec::new();
        for h in 0..72 {
            ts.push(start.timestamp_millis() + i64::from(h) * 3_600_000);
            temps.push(283.15);
        }
        let json = json!({
            "ts": ts,
            "units": {},
            "temp-surface": temps,
            "wind_u-surface": vec![3.0_f64; 72],
            "wind_v-surface": vec![4.0_f64; 72],
        });
        let f = forecast(&json);
        let now = utc(1_776_423_600_000);
        let model = build_model(&f, chrono_tz::UTC, now);

        let current = model.current.expect("current built");
        assert_eq!(current.condition, Condition::Cloudy);
        for slot in &model.days {
            let s = slot.as_ref().expect("day populated");
            assert_eq!(s.condition, Condition::Cloudy);
        }
    }

    #[test]
    fn partial_day_under_threshold_drops_tile() {
        let start = utc(1_776_470_400_000); // 2026-04-18T00:00Z
        let mut ts = Vec::new();
        let mut temps = Vec::new();
        let mut clouds = Vec::new();
        let mut precips = Vec::new();
        for h in 0..5 {
            ts.push(start.timestamp_millis() + i64::from(h) * 3_600_000);
            temps.push(283.15);
            clouds.push(10.0);
            precips.push(0.0);
        }
        for h in 24..48 {
            ts.push(start.timestamp_millis() + i64::from(h) * 3_600_000);
            temps.push(283.15);
            clouds.push(10.0);
            precips.push(0.0);
        }
        let wind_u = vec![3.0_f64; ts.len()];
        let wind_v = vec![4.0_f64; ts.len()];
        let json = json!({
            "ts": ts,
            "units": {},
            "temp-surface": temps,
            "clouds-surface": clouds,
            "precip-surface": precips,
            "wind_u-surface": wind_u,
            "wind_v-surface": wind_v,
        });
        let f = forecast(&json);
        let now = utc(1_776_423_600_000);
        let model = build_model(&f, chrono_tz::UTC, now);
        assert!(model.days[0].is_none(), "18th: {:?}", model.days[0]);
        assert!(model.days[1].is_some(), "19th: {:?}", model.days[1]);
        assert!(model.days[2].is_none(), "20th: {:?}", model.days[2]);
    }

    #[test]
    fn day_with_6_indices_but_all_null_temp_returns_high_none() {
        // 24 hourly samples on 2026-04-18, clouds and
        // precip fully populated, but every temperature
        // entry is null. Day tile must exist (sample
        // count passes the gate) and have `high_c = None`
        // so the SVG renders a placeholder rather than
        // "0°".
        let start = utc(1_776_470_400_000);
        let mut ts = Vec::new();
        let mut temps: Vec<serde_json::Value> = Vec::new();
        let mut clouds = Vec::new();
        let mut precips = Vec::new();
        for h in 0..24 {
            ts.push(start.timestamp_millis() + i64::from(h) * 3_600_000);
            temps.push(serde_json::Value::Null);
            clouds.push(30.0);
            precips.push(0.0);
        }
        let json = json!({
            "ts": ts,
            "units": {},
            "temp-surface": temps,
            "clouds-surface": clouds,
            "precip-surface": precips,
            "wind_u-surface": vec![0.0_f64; 24],
            "wind_v-surface": vec![0.0_f64; 24],
        });
        let f = forecast(&json);
        let now = utc(1_776_423_600_000);
        let model = build_model(&f, chrono_tz::UTC, now);
        let tile = model.days[0].as_ref().expect("18th tile");
        assert_eq!(tile.high_c, None);
        assert_eq!(tile.condition, Condition::PartlyCloudy);
    }

    #[test]
    fn any_rainy_hour_makes_the_whole_day_rain() {
        let start = utc(1_776_470_400_000);
        let mut ts = Vec::new();
        let mut temps = Vec::new();
        let mut clouds = Vec::new();
        let mut precips = Vec::new();
        for h in 0..24 {
            ts.push(start.timestamp_millis() + i64::from(h) * 3_600_000);
            temps.push(283.15);
            clouds.push(5.0);
            precips.push(if h == 12 { 1.0 } else { 0.0 });
        }
        let json = json!({
            "ts": ts,
            "units": {},
            "temp-surface": temps,
            "clouds-surface": clouds,
            "precip-surface": precips,
            "wind_u-surface": vec![0.0_f64; 24],
            "wind_v-surface": vec![0.0_f64; 24],
        });
        let f = forecast(&json);
        let now = utc(1_776_423_600_000);
        let model = build_model(&f, chrono_tz::UTC, now);
        let tile = model.days[0].as_ref().expect("18th covered");
        assert_eq!(tile.condition, Condition::Rain);
    }

    #[test]
    fn timezone_buckets_samples_by_local_date() {
        // London is BST (UTC+1) from late March to late
        // October. Start the forecast at 2026-03-30
        // 22:00 UTC = 2026-03-30 23:00 London. With
        // now=2026-03-29, skip-today-next-3 targets the
        // 30th, 31st, and 1st April (London).
        let start = utc(1_774_044_000_000); // 2026-03-30T22:00Z
        let json = hourly_72h(start, 283.15, 10.0, 0.0);
        let f = forecast(&json);
        let now = utc(1_773_921_600_000); // 2026-03-29T12:00Z
        let london = chrono_tz::Europe::London;
        let model = build_model(&f, london, now);
        // 30th London: only 1 UTC hour (22:00→23:00 London)
        // before midnight → < 6 → None.
        assert!(
            model.days[0].is_none(),
            "30th London has 1 sample: {:?}",
            model.days[0],
        );
        let d31 = model.days[1].as_ref().expect("31st populated");
        assert!(matches!(d31.high_c, Some(h) if (9..=11).contains(&h)));
        assert!(model.days[2].is_some(), "1 April populated");
    }

    #[test]
    fn samples_straddling_spring_forward_bucket_into_same_local_date() {
        // UK DST transition: 2026-03-29 01:00 UTC →
        // 02:00 local (clocks spring forward to BST).
        // Two samples, one 30 min before the transition
        // and one 30 min after, must both land on the
        // 29th London local date. The Tz implementation
        // handles this via the "nonexistent local time"
        // case — this test pins behaviour so future
        // refactors of group_sample_indices_by_date
        // don't silently regress.
        let before = utc(1_774_745_400_000); // 2026-03-29T00:30Z
        let after = utc(1_774_749_000_000); // 2026-03-29T01:30Z
        let json = json!({
            "ts": [before.timestamp_millis(), after.timestamp_millis()],
            "units": {},
            "temp-surface": [283.15, 284.15],
            "clouds-surface": [10.0, 10.0],
            "precip-surface": [0.0, 0.0],
            "wind_u-surface": [0.0, 0.0],
            "wind_v-surface": [0.0, 0.0],
        });
        let f = forecast(&json);
        let london = chrono_tz::Europe::London;
        let groups = group_sample_indices_by_date(&f, london);
        // Both samples land on the 29th local:
        // before = 00:30 GMT = 29th; after = 02:30 BST
        // = 29th.
        let expected = NaiveDate::from_ymd_opt(2026, 3, 29).unwrap();
        assert_eq!(
            groups.get(&expected).map(Vec::len),
            Some(2),
            "both samples should land on 29th local, got {groups:?}",
        );
    }

    #[test]
    fn weekday_label_matches_target_calendar_day() {
        // 2026-04-18 is a Saturday; with now on Friday
        // the 17th, the three tiles are Sat/Sun/Mon.
        let start = utc(1_776_470_400_000);
        let json = hourly_72h(start, 283.15, 10.0, 0.0);
        let f = forecast(&json);
        let now = utc(1_776_423_600_000);
        let model = build_model(&f, chrono_tz::UTC, now);
        let weekdays: Vec<Weekday> = model
            .days
            .iter()
            .map(|d| d.as_ref().unwrap().weekday)
            .collect();
        assert_eq!(weekdays, vec![Weekday::Sat, Weekday::Sun, Weekday::Mon]);
    }

    #[test]
    fn zero_wind_returns_calm_from_north_in_current() {
        let start = utc(1_776_470_400_000);
        let json = json!({
            "ts": [start.timestamp_millis()],
            "units": {},
            "temp-surface": [283.15],
            "clouds-surface": [0.0],
            "precip-surface": [0.0],
            "wind_u-surface": [0.0],
            "wind_v-surface": [0.0],
        });
        let f = forecast(&json);
        let now = utc(1_776_423_600_000);
        let model = build_model(&f, chrono_tz::UTC, now);
        let c = model.current.expect("current built");
        assert!(c.wind_kmh.abs() < 1e-9);
        assert_eq!(c.wind_compass, Compass8::N);
    }
}
