use super::*;

use chrono::TimeZone;
use serde_json::json;

/// Build a `Forecast` from a JSON literal —
/// exercises the live parser, so fixtures can't
/// drift from the wire contract.
fn forecast(json: &serde_json::Value) -> Forecast {
    Forecast::from_raw_json(&json.to_string()).expect("valid forecast fixture")
}

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

/// Produce 72 hourly timestamps starting at
/// `start`, returning a JSON value that mimics a
/// Windy response with the usual `*-surface`
/// series fully populated from the supplied
/// constants. Tests that want outlier samples
/// edit the JSON after construction.
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
            "windGust-surface": "m/s",
            "rh-surface": "%",
        },
        "temp-surface": temp_series,
        "clouds-surface": cloud_series,
        "precip-surface": precip_series,
        "wind_u-surface": vec![3.0_f64; 72],
        "wind_v-surface": vec![4.0_f64; 72],
        "windGust-surface": vec![6.0_f64; 72],
        "rh-surface": vec![55.0_f64; 72],
    })
}

#[test]
fn happy_path_fills_current_today_and_three_days() {
    let start = utc(1_776_470_400_000); // 2026-04-18T00:00Z
    let json = hourly_72h(start, 283.15, 10.0, 0.0);
    let f = forecast(&json);
    let now = utc(1_776_423_600_000); // 2026-04-17T11:00Z
    let model = build_model(&f, ctx_utc(now));

    let current = model.current.expect("current built");
    assert!((current.temp_c - 10.0).abs() < 1e-9);
    assert_eq!(current.condition, Condition::Sunny);
    assert!((current.wind_kmh - 18.0).abs() < 1e-9);
    assert_eq!(current.wind_compass, Compass8::SW);
    let gust = current.gust_kmh.expect("gust populated");
    assert!((gust - 6.0 * MS_TO_KMH).abs() < 1e-9);
    let rh = current.humidity_pct.expect("rh populated");
    assert!((rh - 55.0).abs() < 1e-9);
    // 10 °C with 18 km/h wind is at the upper
    // edge of the wind-chill branch (temp_c ≤ 10
    // and wind > 4.8 km/h) so feels_like dips
    // below the raw 10 °C.
    assert!(
        current.feels_like_c < 10.0,
        "expected wind-chill dip, got {}",
        current.feels_like_c,
    );

    // Today populated but sparse: forecast starts
    // at tomorrow 00:00Z, so there are no samples
    // whose UTC date matches today's UTC date.
    assert!(model.today.is_none(), "today: {:?}", model.today);

    // Three forecast tiles populated.
    for (i, slot) in model.days.iter().enumerate() {
        let s = slot.as_ref().unwrap_or_else(|| panic!("day {i}: None"));
        assert_eq!(s.high_c, Some(10));
        assert_eq!(s.low_c, Some(10));
        assert_eq!(s.condition, Condition::Sunny);
    }

    // No telemetry passed → no battery.
    assert_eq!(model.battery_pct, None);
}

#[test]
fn battery_from_context_telemetry() {
    let start = utc(1_776_470_400_000);
    let json = hourly_72h(start, 283.15, 10.0, 0.0);
    let f = forecast(&json);
    let now = utc(1_776_423_600_000);
    let mut ctx = ctx_utc(now);
    ctx.telemetry = DeviceTelemetry {
        battery_voltage: Some(3.75), // midpoint
    };
    let model = build_model(&f, ctx);
    assert_eq!(model.battery_pct, Some(50));
}

#[test]
fn today_summary_covers_samples_on_local_today() {
    // Forecast covers a 48-hour window centred on
    // `now`. Today's samples should populate the
    // TodaySummary with both high and low.
    let now = utc(1_776_423_600_000); // 2026-04-17T11:00Z
    let start = now - chrono::Duration::hours(11); // 2026-04-17T00:00Z
    let mut ts = Vec::new();
    let mut temps = Vec::new();
    for h in 0..48_i64 {
        ts.push(start.timestamp_millis() + h * 3_600_000);
        // Temperatures swing between 5 °C
        // (overnight) and 15 °C (midday).
        #[allow(clippy::cast_precision_loss)]
        let hour_f = (h % 24) as f64;
        let diurnal = ((hour_f - 12.0) / 12.0).abs();
        temps.push(283.15 + (1.0 - diurnal) * 5.0 + 5.0);
    }
    let json = json!({
        "ts": ts,
        "units": {},
        "temp-surface": temps,
        "clouds-surface": vec![10.0_f64; 48],
        "precip-surface": vec![0.0_f64; 48],
        "wind_u-surface": vec![0.0_f64; 48],
        "wind_v-surface": vec![0.0_f64; 48],
    });
    let f = forecast(&json);
    let model = build_model(&f, ctx_utc(now));
    let today = model.today.expect("today populated");
    // Diurnal fixture: midday peaks at 20 °C,
    // midnight floor is 15 °C.
    assert_eq!(today.high_c, Some(20));
    assert_eq!(today.low_c, Some(15));
    // Ljubljana-ish coordinates → April sunrise
    // around 06:30Z, sunset around 18:00Z (UTC).
    assert!(today.sunrise_local.is_some());
    assert!(today.sunset_local.is_some());
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
    let model = build_model(&f, ctx_utc(now));
    assert!(model.current.is_none());
}

#[test]
fn missing_gust_and_humidity_populate_as_none() {
    // Temp + wind + clouds present, but no gust
    // or humidity series. `current` is still
    // rendered (temp is there); the optional
    // fields become `None`.
    let start = utc(1_776_470_400_000);
    let json = json!({
        "ts": [start.timestamp_millis()],
        "units": {},
        "temp-surface": [283.15],
        "clouds-surface": [10.0],
        "wind_u-surface": [3.0],
        "wind_v-surface": [4.0],
    });
    let f = forecast(&json);
    let now = utc(1_776_423_600_000);
    let model = build_model(&f, ctx_utc(now));
    let current = model.current.expect("current built");
    assert_eq!(current.gust_kmh, None);
    assert_eq!(current.humidity_pct, None);
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
    let model = build_model(&f, ctx_utc(now));

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
    let model = build_model(&f, ctx_utc(now));
    assert!(model.days[0].is_none(), "18th: {:?}", model.days[0]);
    assert!(model.days[1].is_some(), "19th: {:?}", model.days[1]);
    assert!(model.days[2].is_none(), "20th: {:?}", model.days[2]);
}

#[test]
fn day_with_all_null_temp_returns_high_and_low_none() {
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
    let model = build_model(&f, ctx_utc(now));
    let tile = model.days[0].as_ref().expect("18th tile");
    assert_eq!(tile.high_c, None);
    assert_eq!(tile.low_c, None);
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
    let model = build_model(&f, ctx_utc(now));
    let tile = model.days[0].as_ref().expect("18th covered");
    assert_eq!(tile.condition, Condition::Rain);
}

#[test]
fn timezone_buckets_samples_by_local_date() {
    let start = utc(1_774_044_000_000); // 2026-03-30T22:00Z
    let json = hourly_72h(start, 283.15, 10.0, 0.0);
    let f = forecast(&json);
    let now = utc(1_773_921_600_000); // 2026-03-29T12:00Z
    let model = build_model(&f, ctx_with_tz(chrono_tz::Europe::London, now));
    assert!(model.days[0].is_none(), "30th: {:?}", model.days[0]);
    let d31 = model.days[1].as_ref().expect("31st populated");
    assert!(matches!(d31.high_c, Some(h) if (9..=11).contains(&h)));
    assert!(model.days[2].is_some(), "1 April populated");
}

#[test]
fn samples_straddling_spring_forward_bucket_into_same_local_date() {
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
    let groups = group_sample_indices_by_date(&f, chrono_tz::Europe::London);
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
    let json = hourly_72h(start, 283.15, 10.0, 0.0);
    let f = forecast(&json);
    let now = utc(1_776_423_600_000);
    let model = build_model(&f, ctx_utc(now));
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
    let model = build_model(&f, ctx_utc(now));
    let c = model.current.expect("current built");
    assert!(c.wind_kmh.abs() < 1e-9);
    assert_eq!(c.wind_compass, Compass8::N);
}
