//! Unit tests for [`super`].

use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use crate::dashboard::classify::{WeatherCode, WmoCode};

fn simple_body() -> serde_json::Value {
    json!({
        "hourly": {
            "time": ["2026-04-19T00:00", "2026-04-19T01:00"],
            "temperature_2m": [10.0, 11.5],
            "relative_humidity_2m": [80.0, 82.0],
            "precipitation": [0.0, 0.1],
            "cloud_cover": [20.0, 25.0],
            "wind_speed_10m": [5.5, 6.0],
            "wind_direction_10m": [180.0, 190.0],
            "wind_gusts_10m": [10.0, 12.0],
            "weather_code": [0, 61]
        }
    })
}

fn ok_request() -> FetchRequest {
    FetchRequest {
        lat: 46.55,
        lon: 15.64,
        model: "icon_eu".into(),
    }
}

#[tokio::test]
async fn fetch_parses_timestamps_and_all_series() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(simple_body()))
        .mount(&server)
        .await;

    let client = Client::with_base_url(server.uri());
    let snap = client.fetch(&ok_request()).await.unwrap();

    assert_eq!(snap.timestamps().len(), 2);
    assert_eq!(snap.temperature_c(), &[Some(10.0), Some(11.5)]);
    assert_eq!(snap.humidity_pct(), &[Some(80.0), Some(82.0)]);
    assert_eq!(snap.precip_mm(), &[Some(0.0), Some(0.1)]);
    assert_eq!(snap.cloud_cover_pct(), &[Some(20.0), Some(25.0)]);
    assert_eq!(snap.wind_kmh(), &[Some(5.5), Some(6.0)]);
    assert_eq!(snap.wind_dir_deg(), &[Some(180.0), Some(190.0)]);
    assert_eq!(snap.gust_kmh(), &[Some(10.0), Some(12.0)]);
    assert_eq!(
        snap.weather_code(),
        &[
            Some(WeatherCode::Wmo(WmoCode::Clear)),
            Some(WeatherCode::Wmo(WmoCode::RainSlight)),
        ],
    );
}

#[tokio::test]
async fn fetch_treats_absent_weather_code_series_as_all_none() {
    // Provider may omit the field for models that
    // don't expose WMO codes; fallback path must not
    // error.
    let body = json!({
        "hourly": {
            "time": ["2026-04-19T00:00", "2026-04-19T01:00"],
            "temperature_2m": [10.0, 11.5]
        }
    });
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    let client = Client::with_base_url(server.uri());
    let snap = client.fetch(&ok_request()).await.unwrap();
    assert_eq!(snap.weather_code(), &[None, None]);
}

#[tokio::test]
async fn fetch_partitions_weather_codes_into_three_outcomes() {
    // WMO 4677 is not a contiguous range — it's a
    // sparse set. The boundary produces three distinct
    // outcomes:
    //   - documented codes (0, 99) → Some(Wmo(_))
    //   - in-byte-range gap codes (4) →
    //     Some(Unrecognised(byte)) so the display can
    //     surface ConditionCategory::Unknown
    //   - out-of-byte (-1, 300) and non-integer (50.5)
    //     values → None (wire noise, not a code).
    let body = json!({
        "hourly": {
            "time": [
                "2026-04-19T00:00", "2026-04-19T01:00",
                "2026-04-19T02:00", "2026-04-19T03:00",
                "2026-04-19T04:00", "2026-04-19T05:00"
            ],
            "weather_code": [0, 99, 4, 50.5, -1, 300]
        }
    });
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    let client = Client::with_base_url(server.uri());
    let snap = client.fetch(&ok_request()).await.unwrap();
    assert_eq!(
        snap.weather_code(),
        &[
            Some(WeatherCode::Wmo(WmoCode::Clear)),
            Some(WeatherCode::Wmo(WmoCode::ThunderstormHailHeavy)),
            Some(WeatherCode::Unrecognised(4)),
            None,
            None,
            None,
        ],
    );
}

#[tokio::test]
async fn fetch_sends_expected_query_params() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(ENDPOINT_PATH))
        // Six-decimal formatting avoids scientific
        // notation for subnormal inputs.
        .and(query_param("latitude", "46.550000"))
        .and(query_param("longitude", "15.640000"))
        .and(query_param("forecast_days", "4"))
        .and(query_param("models", "icon_eu"))
        .respond_with(ResponseTemplate::new(200).set_body_json(simple_body()))
        .mount(&server)
        .await;
    let client = Client::with_base_url(server.uri());
    client.fetch(&ok_request()).await.unwrap();
}

#[tokio::test]
async fn fetch_omits_timezone_query_param() {
    // Open-Meteo rejects `timezone=utc` with HTTP 400.
    // Lock the "no timezone param" contract.
    use wiremock::matchers::query_param_is_missing;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(ENDPOINT_PATH))
        .and(query_param_is_missing("timezone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(simple_body()))
        .mount(&server)
        .await;
    let client = Client::with_base_url(server.uri());
    client.fetch(&ok_request()).await.unwrap();
}

#[tokio::test]
async fn fetch_handles_missing_optional_series_as_none() {
    // When Open-Meteo omits a series entirely, every
    // step becomes `None`. Locks the "absent series"
    // branch of `pick_series`.
    let body = json!({
        "hourly": {
            "time": ["2026-04-19T00:00"],
            "temperature_2m": [10.0]
        }
    });
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    let client = Client::with_base_url(server.uri());
    let snap = client.fetch(&ok_request()).await.unwrap();
    assert_eq!(snap.timestamps().len(), 1);
    assert_eq!(snap.temperature_c(), &[Some(10.0)]);
    assert_eq!(snap.humidity_pct(), &[None]);
    assert_eq!(snap.gust_kmh(), &[None]);
}

#[tokio::test]
async fn fetch_rejects_series_with_mismatched_length() {
    // `time.len() == 2` but `temperature_2m.len() == 1`
    // — must error, not silently pad. Protects
    // against the "half-a-forecast" wire bug.
    let body = json!({
        "hourly": {
            "time": ["2026-04-19T00:00", "2026-04-19T01:00"],
            "temperature_2m": [10.0]
        }
    });
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    let client = Client::with_base_url(server.uri());
    let err = client.fetch(&ok_request()).await.unwrap_err();
    let OpenMeteoError::SeriesLengthMismatch {
        series,
        expected,
        got,
    } = err
    else {
        panic!("wrong variant: {err:?}")
    };
    assert_eq!(series, "temperature_c");
    assert_eq!(expected, 2);
    assert_eq!(got, 1);
}

#[tokio::test]
async fn fetch_coerces_non_finite_values_to_none() {
    // serde_json accepts neither NaN nor Infinity in
    // strict mode; simulate by sending a very small
    // value that rounds via f64 parsing. Instead we
    // exercise `sanitise_non_finite` directly in a
    // unit test below — here we just confirm normal
    // sentinel values (e.g. -9999) survive.
    let body = json!({
        "hourly": {
            "time": ["2026-04-19T00:00"],
            "temperature_2m": [-9999.0]
        }
    });
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    let client = Client::with_base_url(server.uri());
    let snap = client.fetch(&ok_request()).await.unwrap();
    // Sentinel is finite, so it passes through. Only
    // IEEE-754 non-finite values are filtered.
    assert_eq!(snap.temperature_c(), &[Some(-9999.0)]);
}

#[test]
fn sanitise_non_finite_maps_nan_and_inf_to_none() {
    let input = vec![
        Some(10.0),
        Some(f64::NAN),
        Some(f64::INFINITY),
        Some(f64::NEG_INFINITY),
        None,
    ];
    let out = sanitise_non_finite(input);
    assert_eq!(out[0], Some(10.0));
    assert_eq!(out[1], None);
    assert_eq!(out[2], None);
    assert_eq!(out[3], None);
    assert_eq!(out[4], None);
}

#[tokio::test]
async fn fetch_preserves_null_entries_in_a_series() {
    let body = json!({
        "hourly": {
            "time": ["2026-04-19T00:00", "2026-04-19T01:00"],
            "temperature_2m": [10.0, null]
        }
    });
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    let client = Client::with_base_url(server.uri());
    let snap = client.fetch(&ok_request()).await.unwrap();
    assert_eq!(snap.temperature_c(), &[Some(10.0), None]);
}

#[tokio::test]
async fn fetch_returns_api_error_for_non_2xx() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(400).set_body_string(
            r#"{"error":true,"reason":"Latitude must be in [-90,90]"}"#,
        ))
        .mount(&server)
        .await;
    let client = Client::with_base_url(server.uri());
    let err = client.fetch(&ok_request()).await.unwrap_err();
    let OpenMeteoError::Api { status, body } = err else {
        panic!("expected Api, got {err:?}")
    };
    assert_eq!(status, 400);
    assert!(body.contains("Latitude"));
}

#[tokio::test]
async fn fetch_surfaces_invalid_timestamp() {
    let body = json!({
        "hourly": {
            "time": ["nope"],
            "temperature_2m": [10.0]
        }
    });
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    let client = Client::with_base_url(server.uri());
    let err = client.fetch(&ok_request()).await.unwrap_err();
    let OpenMeteoError::InvalidTimestamp { raw } = err else {
        panic!("expected InvalidTimestamp, got {err:?}")
    };
    assert_eq!(raw, "nope");
}

#[tokio::test]
async fn fetch_surfaces_malformed_json() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;
    let client = Client::with_base_url(server.uri());
    let err = client.fetch(&ok_request()).await.unwrap_err();
    assert!(matches!(err, OpenMeteoError::Json(_)), "got: {err:?}");
}

#[tokio::test]
async fn fetch_enforces_response_size_cap() {
    let huge = "x".repeat(8192);
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_string(huge))
        .mount(&server)
        .await;
    let client =
        Client::with_base_url(server.uri()).with_max_response_bytes(1024);
    let err = client.fetch(&ok_request()).await.unwrap_err();
    assert!(
        matches!(err, OpenMeteoError::ResponseTooLarge { limit: 1024 }),
        "got: {err:?}",
    );
}

#[test]
fn fetch_request_from_parts_extracts_model() {
    let sub = OpenMeteoProviderConfig {
        model: "gfs_global".to_owned(),
    };
    let req = FetchRequest::from_parts(46.55, 15.64, &sub);
    assert!((req.lat - 46.55).abs() < 1e-9);
    assert!((req.lon - 15.64).abs() < 1e-9);
    assert_eq!(req.model, "gfs_global");
}

#[tokio::test]
async fn provider_reports_its_configured_location() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(simple_body()))
        .mount(&server)
        .await;
    let client = Client::with_base_url(server.uri());
    let provider = OpenMeteoProvider::new(client, ok_request());
    let loc = provider.location();
    assert!((loc.lat_deg - 46.55).abs() < 1e-9);
    assert!((loc.lon_deg - 15.64).abs() < 1e-9);
}

#[tokio::test]
async fn provider_trait_round_trips_snapshot() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(simple_body()))
        .mount(&server)
        .await;
    let client = Client::with_base_url(server.uri());
    let provider = OpenMeteoProvider::new(client, ok_request());
    let snap = provider.fetch().await.unwrap();
    assert_eq!(snap.timestamps().len(), 2);
    assert_eq!(snap.temperature_c()[0], Some(10.0));
}

#[tokio::test]
async fn provider_maps_transport_error_to_weather_transport() {
    let client = Client::with_base_url("http://127.0.0.1:1");
    let provider = OpenMeteoProvider::new(client, ok_request());
    let err = provider.fetch().await.unwrap_err();
    assert!(matches!(err, WeatherError::Transport(_)), "got: {err:?}",);
}

#[test]
fn api_error_maps_to_weather_provider() {
    let e = OpenMeteoError::Api {
        status: 500,
        body: "boom".into(),
    };
    assert!(matches!(WeatherError::from(e), WeatherError::Provider(_)));
}
