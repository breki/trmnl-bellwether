//! Tests for the provider-neutral weather types.

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};

use super::{
    WeatherError, WeatherProvider, WeatherSnapshot, WeatherSnapshotBuilder,
};
use crate::dashboard::astro::GeoPoint;

fn hourly_timestamps(n: usize) -> Vec<DateTime<Utc>> {
    (0..n)
        .map(|i| {
            let secs =
                i64::try_from(i).expect("test fixture uses small n") * 3600;
            Utc.timestamp_opt(secs, 0).unwrap()
        })
        .collect()
}

fn builder_of_length(n: usize) -> WeatherSnapshotBuilder {
    WeatherSnapshotBuilder {
        timestamps: hourly_timestamps(n),
        temperature_c: vec![None; n],
        humidity_pct: vec![None; n],
        wind_kmh: vec![None; n],
        wind_dir_deg: vec![None; n],
        gust_kmh: vec![None; n],
        cloud_cover_pct: vec![None; n],
        precip_mm: vec![None; n],
        weather_code: vec![None; n],
        warning: None,
    }
}

#[test]
fn build_accepts_matching_lengths() {
    let snap = builder_of_length(3).build().expect("valid snapshot");
    assert_eq!(snap.timestamps().len(), 3);
    assert_eq!(snap.temperature_c().len(), 3);
}

#[test]
fn build_rejects_empty_timestamps() {
    let builder = WeatherSnapshotBuilder::default();
    assert!(matches!(builder.build(), Err(WeatherError::EmptySnapshot)));
}

#[test]
fn build_rejects_mismatched_series_length() {
    let mut builder = builder_of_length(3);
    builder.temperature_c = vec![Some(10.0), Some(11.0)];
    let err = builder.build().unwrap_err();
    let WeatherError::SeriesLengthMismatch {
        series,
        expected,
        got,
    } = err
    else {
        panic!("wrong variant: {err:?}");
    };
    assert_eq!(series, "temperature_c");
    assert_eq!(expected, 3);
    assert_eq!(got, 2);
}

#[test]
fn build_flags_each_series_by_name() {
    // Rotate through every series to confirm the
    // error names the field that was actually wrong.
    let names = [
        "temperature_c",
        "humidity_pct",
        "wind_kmh",
        "wind_dir_deg",
        "gust_kmh",
        "cloud_cover_pct",
        "precip_mm",
        "weather_code",
    ];
    for (index, expected_name) in names.iter().enumerate() {
        let mut b = builder_of_length(3);
        let short: Vec<Option<f64>> = vec![None; 2];
        match index {
            0 => b.temperature_c = short,
            1 => b.humidity_pct = short,
            2 => b.wind_kmh = short,
            3 => b.wind_dir_deg = short,
            4 => b.gust_kmh = short,
            5 => b.cloud_cover_pct = short,
            6 => b.precip_mm = short,
            7 => b.weather_code = vec![None; 2],
            _ => unreachable!(),
        }
        let err = b.build().unwrap_err();
        let WeatherError::SeriesLengthMismatch { series, .. } = err else {
            panic!("wrong variant for {expected_name}: {err:?}");
        };
        assert_eq!(&series, expected_name);
    }
}

#[test]
fn accessors_return_what_builder_stored() {
    let mut b = builder_of_length(2);
    b.temperature_c = vec![Some(10.0), Some(12.0)];
    b.wind_dir_deg = vec![Some(180.0), Some(90.0)];
    b.warning = Some("quota".into());
    let snap = b.build().unwrap();
    assert_eq!(snap.temperature_c(), &[Some(10.0), Some(12.0)]);
    assert_eq!(snap.wind_dir_deg(), &[Some(180.0), Some(90.0)]);
    assert_eq!(snap.warning(), Some("quota"));
}

#[test]
fn error_display_is_informative() {
    let empty = WeatherError::EmptySnapshot;
    assert!(empty.to_string().contains("no timestamps"));

    let mismatch = WeatherError::SeriesLengthMismatch {
        series: "wind_kmh".into(),
        expected: 24,
        got: 23,
    };
    let msg = mismatch.to_string();
    assert!(msg.contains("wind_kmh"));
    assert!(msg.contains("24"));
    assert!(msg.contains("23"));

    let transport = WeatherError::Transport("boom".into());
    // Pass-through message: no "weather transport
    // error:" prefix, just the inner message.
    assert_eq!(transport.to_string(), "boom");

    let provider = WeatherError::Provider("nope".into());
    assert_eq!(provider.to_string(), "nope");
}

#[test]
fn provider_trait_is_object_safe() {
    fn _assert(_p: &dyn WeatherProvider) {}
}

struct ConstProvider {
    location: GeoPoint,
}

#[async_trait]
impl WeatherProvider for ConstProvider {
    fn location(&self) -> GeoPoint {
        self.location
    }

    async fn fetch(&self) -> Result<WeatherSnapshot, WeatherError> {
        Err(WeatherError::EmptySnapshot)
    }
}

#[test]
fn provider_location_returns_configured_point() {
    let p = ConstProvider {
        location: GeoPoint {
            lat_deg: 46.05,
            lon_deg: 14.51,
        },
    };
    assert!((p.location().lat_deg - 46.05).abs() < f64::EPSILON);
    assert!((p.location().lon_deg - 14.51).abs() < f64::EPSILON);
}
