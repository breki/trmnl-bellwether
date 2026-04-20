use super::{
    DashboardModel, MIN_SAMPLES_PER_DAY, ModelContext, build_model,
    group_sample_indices_by_date,
};

use chrono::{DateTime, NaiveDate, TimeZone, Utc, Weekday};
use chrono_tz::Tz;

use crate::dashboard::astro::GeoPoint;
use crate::dashboard::classify::{Compass8, ConditionCategory};
use crate::telemetry::DeviceTelemetry;
use crate::weather::{WeatherSnapshot, WeatherSnapshotBuilder};

// `MIN_SAMPLES_PER_DAY` is pulled in so downstream
// tests that check the threshold compile; it isn't
// referenced directly by any test today. Remove when
// the first threshold-specific test lands.
#[allow(dead_code)]
const _: usize = MIN_SAMPLES_PER_DAY;

// `DashboardModel` pulled in for completeness; the
// individual fields are inspected via field access
// which keeps the symbol live.
#[allow(dead_code)]
type _DashboardModelAlias = DashboardModel;

fn utc(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap()
}

const TEST_LOCATION: GeoPoint = GeoPoint {
    lat_deg: 46.05,
    lon_deg: 14.51,
};

/// Default `ModelContext` for a UTC-anchored
/// test. Overwritten as needed.
fn ctx_utc(now: DateTime<Utc>) -> ModelContext {
    ModelContext {
        tz: chrono_tz::UTC,
        location: TEST_LOCATION,
        now,
        telemetry: DeviceTelemetry::default(),
    }
}

fn ctx_with_tz(tz: Tz, now: DateTime<Utc>) -> ModelContext {
    ModelContext {
        tz,
        location: TEST_LOCATION,
        now,
        telemetry: DeviceTelemetry::default(),
    }
}

/// Hourly timestamps starting at `start`, count
/// steps long.
fn ts_hourly(start: DateTime<Utc>, count: usize) -> Vec<DateTime<Utc>> {
    (0..count)
        .map(|h| {
            let secs = i64::try_from(h).expect("fixture is small") * 3600;
            start + chrono::Duration::seconds(secs)
        })
        .collect()
}

/// 216.87° is the compass from-direction a SW wind
/// (u=3, v=4 m/s) produces.
const DEFAULT_WIND_DIR_DEG: f64 = 216.87;

/// hypot(3, 4) * 3.6 = 18 km/h.
const DEFAULT_WIND_KMH: f64 = 18.0;

fn built(builder: WeatherSnapshotBuilder) -> WeatherSnapshot {
    builder.build().expect("valid snapshot")
}

/// Build a 72-hour snapshot with constant temp /
/// clouds / precip plus sensible defaults.
fn snapshot_72h(
    start: DateTime<Utc>,
    temp_c: f64,
    cloud_pct: f64,
    precip_mm: f64,
) -> WeatherSnapshot {
    let n: usize = 72;
    built(WeatherSnapshotBuilder {
        timestamps: ts_hourly(start, n),
        temperature_c: vec![Some(temp_c); n],
        humidity_pct: vec![Some(55.0); n],
        wind_kmh: vec![Some(DEFAULT_WIND_KMH); n],
        wind_dir_deg: vec![Some(DEFAULT_WIND_DIR_DEG); n],
        gust_kmh: vec![Some(21.6); n],
        cloud_cover_pct: vec![Some(cloud_pct); n],
        precip_mm: vec![Some(precip_mm); n],
        weather_code: vec![None; n],
        warning: None,
    })
}

/// Constant-length snapshot filled with `None` for
/// every series.
fn all_none_snapshot(timestamps: Vec<DateTime<Utc>>) -> WeatherSnapshot {
    let n = timestamps.len();
    built(WeatherSnapshotBuilder {
        timestamps,
        temperature_c: vec![None; n],
        humidity_pct: vec![None; n],
        wind_kmh: vec![None; n],
        wind_dir_deg: vec![None; n],
        gust_kmh: vec![None; n],
        cloud_cover_pct: vec![None; n],
        precip_mm: vec![None; n],
        weather_code: vec![None; n],
        warning: None,
    })
}

#[test]
fn happy_path_fills_current_today_and_three_days() {
    let start = utc(1_776_470_400_000); // 2026-04-18T00:00Z
    let snap = snapshot_72h(start, 10.0, 10.0, 0.0);
    let now = utc(1_776_423_600_000); // 2026-04-17T11:00Z
    let model = build_model(&snap, ctx_utc(now));

    let current = model.current.expect("current built");
    assert!((current.temp_c - 10.0).abs() < 1e-9);
    assert_eq!(current.category, ConditionCategory::Clear);
    assert!((current.wind_kmh - DEFAULT_WIND_KMH).abs() < 1e-9);
    assert_eq!(current.wind_compass, Compass8::SW);
    let gust = current.gust_kmh.expect("gust populated");
    assert!((gust - 21.6).abs() < 1e-9);
    let rh = current.humidity_pct.expect("rh populated");
    assert!((rh - 55.0).abs() < 1e-9);
    assert!(
        current.feels_like_c < 10.0,
        "expected wind-chill dip, got {}",
        current.feels_like_c,
    );

    assert!(model.today.is_none(), "today: {:?}", model.today);

    for (i, slot) in model.days.iter().enumerate() {
        let s = slot.as_ref().unwrap_or_else(|| panic!("day {i}: None"));
        assert_eq!(s.high_c, Some(10));
        assert_eq!(s.low_c, Some(10));
        assert_eq!(s.category, ConditionCategory::Clear);
    }

    assert_eq!(model.battery_pct, None);
}

#[test]
fn battery_from_context_telemetry() {
    let start = utc(1_776_470_400_000);
    let snap = snapshot_72h(start, 10.0, 10.0, 0.0);
    let now = utc(1_776_423_600_000);
    let mut ctx = ctx_utc(now);
    ctx.telemetry = DeviceTelemetry {
        battery_voltage: Some(3.75),
    };
    let model = build_model(&snap, ctx);
    assert_eq!(model.battery_pct, Some(50));
}

#[test]
fn today_summary_covers_samples_on_local_today() {
    let now = utc(1_776_423_600_000); // 2026-04-17T11:00Z
    let start = now - chrono::Duration::hours(11); // 2026-04-17T00:00Z
    let n: usize = 48;
    let temps: Vec<Option<f64>> = (0..n)
        .map(|h| {
            #[allow(clippy::cast_precision_loss)]
            let hour_f = (h % 24) as f64;
            let diurnal = ((hour_f - 12.0) / 12.0).abs();
            Some(15.0 + (1.0 - diurnal) * 5.0)
        })
        .collect();
    let snap = built(WeatherSnapshotBuilder {
        timestamps: ts_hourly(start, n),
        temperature_c: temps,
        humidity_pct: vec![Some(55.0); n],
        wind_kmh: vec![Some(0.0); n],
        wind_dir_deg: vec![Some(0.0); n],
        gust_kmh: vec![None; n],
        cloud_cover_pct: vec![Some(10.0); n],
        precip_mm: vec![Some(0.0); n],
        weather_code: vec![None; n],
        warning: None,
    });
    let model = build_model(&snap, ctx_utc(now));
    let today = model.today.expect("today populated");
    assert_eq!(today.high_c, Some(20));
    assert_eq!(today.low_c, Some(15));
    assert!(today.sunrise_local.is_some());
    assert!(today.sunset_local.is_some());
}

#[test]
fn missing_temperature_collapses_current_panel() {
    let start = utc(1_776_470_400_000);
    let snap = built(WeatherSnapshotBuilder {
        timestamps: ts_hourly(start, 1),
        temperature_c: vec![None],
        humidity_pct: vec![None],
        wind_kmh: vec![Some(DEFAULT_WIND_KMH)],
        wind_dir_deg: vec![Some(DEFAULT_WIND_DIR_DEG)],
        gust_kmh: vec![None],
        cloud_cover_pct: vec![Some(10.0)],
        precip_mm: vec![None],
        weather_code: vec![None; 1],
        warning: None,
    });
    let now = utc(1_776_423_600_000);
    let model = build_model(&snap, ctx_utc(now));
    assert!(model.current.is_none());
}

#[test]
fn missing_gust_and_humidity_populate_as_none() {
    let start = utc(1_776_470_400_000);
    let snap = built(WeatherSnapshotBuilder {
        timestamps: ts_hourly(start, 1),
        temperature_c: vec![Some(10.0)],
        humidity_pct: vec![None],
        wind_kmh: vec![Some(DEFAULT_WIND_KMH)],
        wind_dir_deg: vec![Some(DEFAULT_WIND_DIR_DEG)],
        gust_kmh: vec![None],
        cloud_cover_pct: vec![Some(10.0)],
        precip_mm: vec![Some(0.0)],
        weather_code: vec![None; 1],
        warning: None,
    });
    let now = utc(1_776_423_600_000);
    let model = build_model(&snap, ctx_utc(now));
    let current = model.current.expect("current built");
    assert_eq!(current.gust_kmh, None);
    assert_eq!(current.humidity_pct, None);
}

#[test]
fn missing_cloud_and_precip_defaults_day_to_cloudy() {
    let start = utc(1_776_470_400_000);
    let n: usize = 72;
    let snap = built(WeatherSnapshotBuilder {
        timestamps: ts_hourly(start, n),
        temperature_c: vec![Some(10.0); n],
        humidity_pct: vec![Some(55.0); n],
        wind_kmh: vec![Some(DEFAULT_WIND_KMH); n],
        wind_dir_deg: vec![Some(DEFAULT_WIND_DIR_DEG); n],
        gust_kmh: vec![None; n],
        cloud_cover_pct: vec![None; n],
        precip_mm: vec![None; n],
        weather_code: vec![None; n],
        warning: None,
    });
    let now = utc(1_776_423_600_000);
    let model = build_model(&snap, ctx_utc(now));

    let current = model.current.expect("current built");
    assert_eq!(current.category, ConditionCategory::Cloudy);
    for slot in &model.days {
        let s = slot.as_ref().expect("day populated");
        assert_eq!(s.category, ConditionCategory::Cloudy);
    }
}

#[test]
fn partial_day_under_threshold_drops_tile() {
    let start = utc(1_776_470_400_000);
    let mut timestamps: Vec<DateTime<Utc>> = Vec::new();
    for h in 0..5 {
        timestamps.push(start + chrono::Duration::hours(h));
    }
    for h in 24..48 {
        timestamps.push(start + chrono::Duration::hours(h));
    }
    let n = timestamps.len();
    let snap = built(WeatherSnapshotBuilder {
        timestamps,
        temperature_c: vec![Some(10.0); n],
        humidity_pct: vec![Some(55.0); n],
        wind_kmh: vec![Some(DEFAULT_WIND_KMH); n],
        wind_dir_deg: vec![Some(DEFAULT_WIND_DIR_DEG); n],
        gust_kmh: vec![None; n],
        cloud_cover_pct: vec![Some(10.0); n],
        precip_mm: vec![Some(0.0); n],
        weather_code: vec![None; n],
        warning: None,
    });
    let now = utc(1_776_423_600_000);
    let model = build_model(&snap, ctx_utc(now));
    assert!(model.days[0].is_none(), "18th: {:?}", model.days[0]);
    assert!(model.days[1].is_some(), "19th: {:?}", model.days[1]);
    assert!(model.days[2].is_none(), "20th: {:?}", model.days[2]);
}

#[test]
fn day_with_all_null_temp_returns_high_and_low_none() {
    let start = utc(1_776_470_400_000);
    let n: usize = 24;
    let snap = built(WeatherSnapshotBuilder {
        timestamps: ts_hourly(start, n),
        temperature_c: vec![None; n],
        humidity_pct: vec![None; n],
        wind_kmh: vec![Some(0.0); n],
        wind_dir_deg: vec![Some(0.0); n],
        gust_kmh: vec![None; n],
        cloud_cover_pct: vec![Some(30.0); n],
        precip_mm: vec![Some(0.0); n],
        weather_code: vec![None; n],
        warning: None,
    });
    let now = utc(1_776_423_600_000);
    let model = build_model(&snap, ctx_utc(now));
    let tile = model.days[0].as_ref().expect("18th tile");
    assert_eq!(tile.high_c, None);
    assert_eq!(tile.low_c, None);
    assert_eq!(tile.category, ConditionCategory::PartlyCloudy);
}

#[test]
fn any_rainy_hour_makes_the_whole_day_rain() {
    let start = utc(1_776_470_400_000);
    let n: usize = 24;
    let precip: Vec<Option<f64>> = (0..n)
        .map(|h| Some(if h == 12 { 1.0 } else { 0.0 }))
        .collect();
    let snap = built(WeatherSnapshotBuilder {
        timestamps: ts_hourly(start, n),
        temperature_c: vec![Some(10.0); n],
        humidity_pct: vec![None; n],
        wind_kmh: vec![Some(0.0); n],
        wind_dir_deg: vec![Some(0.0); n],
        gust_kmh: vec![None; n],
        cloud_cover_pct: vec![Some(5.0); n],
        precip_mm: precip,
        weather_code: vec![None; n],
        warning: None,
    });
    let now = utc(1_776_423_600_000);
    let model = build_model(&snap, ctx_utc(now));
    let tile = model.days[0].as_ref().expect("18th covered");
    assert_eq!(tile.category, ConditionCategory::Rain);
}

#[test]
fn timezone_buckets_samples_by_local_date() {
    let start = utc(1_774_044_000_000); // 2026-03-30T22:00Z
    let snap = snapshot_72h(start, 10.0, 10.0, 0.0);
    let now = utc(1_773_921_600_000); // 2026-03-29T12:00Z
    let model = build_model(&snap, ctx_with_tz(chrono_tz::Europe::London, now));
    assert!(model.days[0].is_none(), "30th: {:?}", model.days[0]);
    let d31 = model.days[1].as_ref().expect("31st populated");
    assert!(matches!(d31.high_c, Some(h) if (9..=11).contains(&h)));
    assert!(model.days[2].is_some(), "1 April populated");
}

#[test]
fn samples_straddling_spring_forward_bucket_into_same_local_date() {
    let before = utc(1_774_745_400_000); // 2026-03-29T00:30Z
    let after = utc(1_774_749_000_000); // 2026-03-29T01:30Z
    let snap = built(WeatherSnapshotBuilder {
        timestamps: vec![before, after],
        temperature_c: vec![Some(10.0), Some(11.0)],
        humidity_pct: vec![None, None],
        wind_kmh: vec![Some(0.0), Some(0.0)],
        wind_dir_deg: vec![Some(0.0), Some(0.0)],
        gust_kmh: vec![None, None],
        cloud_cover_pct: vec![Some(10.0), Some(10.0)],
        precip_mm: vec![Some(0.0), Some(0.0)],
        weather_code: vec![None, None],
        warning: None,
    });
    let groups = group_sample_indices_by_date(&snap, chrono_tz::Europe::London);
    let expected = NaiveDate::from_ymd_opt(2026, 3, 29).unwrap();
    assert_eq!(
        groups.get(&expected).map(Vec::len),
        Some(2),
        "both samples should land on 29th local, got {groups:?}",
    );
}

#[test]
fn weekday_label_matches_target_calendar_day() {
    let start = utc(1_776_470_400_000);
    let snap = snapshot_72h(start, 10.0, 10.0, 0.0);
    let now = utc(1_776_423_600_000);
    let model = build_model(&snap, ctx_utc(now));
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
    let snap = built(WeatherSnapshotBuilder {
        timestamps: ts_hourly(start, 1),
        temperature_c: vec![Some(10.0)],
        humidity_pct: vec![None],
        wind_kmh: vec![Some(0.0)],
        wind_dir_deg: vec![Some(0.0)],
        gust_kmh: vec![None],
        cloud_cover_pct: vec![Some(0.0)],
        precip_mm: vec![Some(0.0)],
        weather_code: vec![None; 1],
        warning: None,
    });
    let now = utc(1_776_423_600_000);
    let model = build_model(&snap, ctx_utc(now));
    let c = model.current.expect("current built");
    assert!(c.wind_kmh.abs() < 1e-9);
    assert_eq!(c.wind_compass, Compass8::N);
}

#[test]
fn forecast_tile_weekdays_populate_even_when_days_empty() {
    let now = utc(1_776_423_600_000);
    let snap = all_none_snapshot(ts_hourly(now, 1));
    let model = build_model(&snap, ctx_utc(now));
    assert_eq!(
        model.day_weekdays,
        [Weekday::Sat, Weekday::Sun, Weekday::Mon]
    );
}
