//! Unit tests for [`super`]. Live in a sibling file so
//! the production module stays at a readable size.

use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;

fn forecast_fixture() -> serde_json::Value {
    json!({
        "ts": [1_700_000_000_000_i64, 1_700_003_600_000_i64],
        "units": {
            "temp-surface": "K",
            "wind_u-surface": "m>s-1",
            "wind_v-surface": "m>s-1",
        },
        "temp-surface": [293.15, 294.25],
        "wind_u-surface": [4.2, 5.1],
        "wind_v-surface": [1.1, 1.2],
    })
}

fn ok_request() -> FetchRequest {
    FetchRequest {
        api_key: "test-key".into(),
        lat: 46.05,
        lon: 14.51,
        model: "gfs".into(),
        parameters: vec![WindyParameter::Temp, WindyParameter::Wind],
    }
}

#[tokio::test]
async fn fetch_parses_timestamps_units_and_series() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(forecast_fixture()),
        )
        .mount(&server)
        .await;

    let client = Client::with_base_url(server.uri());
    let forecast = client.fetch(ok_request()).await.unwrap();

    assert_eq!(forecast.timestamps.len(), 2);
    assert_eq!(
        forecast.values(WindyParameter::Temp).unwrap(),
        &[Some(293.15), Some(294.25)],
    );
    assert_eq!(
        forecast.units.get("temp-surface").map(String::as_str),
        Some("K"),
    );
    assert!(forecast.warning.is_none());
}

#[tokio::test]
async fn fetch_sends_expected_request_body() {
    let server = MockServer::start().await;
    // body_json matcher rejects mismatched bodies with
    // a 404 default; fetch() then returns WindyError::Api
    // and the .unwrap() below panics. That's how we
    // assert the outgoing body shape.
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .and(header("content-type", "application/json"))
        .and(body_json(json!({
            "lat": 46.05,
            "lon": 14.51,
            "model": "gfs",
            "parameters": ["temp", "windGust"],
            "key": "secret-key",
            "levels": ["surface"],
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(forecast_fixture()),
        )
        .mount(&server)
        .await;

    let client = Client::with_base_url(server.uri());
    client
        .fetch(FetchRequest {
            api_key: "secret-key".into(),
            lat: 46.05,
            lon: 14.51,
            model: "gfs".into(),
            parameters: vec![WindyParameter::Temp, WindyParameter::WindGust],
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn fetch_surfaces_api_error_with_redacted_body() {
    let server = MockServer::start().await;
    // Simulate a proxy/server that echoes the request
    // body (including the api key) in its error page.
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(401).set_body_string(
            r#"{"error":"bad key","received_key":"leak-bait-key-xxxxx"}"#,
        ))
        .mount(&server)
        .await;

    let client = Client::with_base_url(server.uri());
    let err = client
        .fetch(FetchRequest {
            api_key: "leak-bait-key-xxxxx".into(),
            lat: 0.0,
            lon: 0.0,
            model: "gfs".into(),
            parameters: vec![WindyParameter::Temp],
        })
        .await
        .unwrap_err();
    let WindyError::Api { status, body } = err else {
        panic!("expected Api, got {err:?}")
    };
    assert_eq!(status, 401);
    assert!(body.contains("<redacted>"));
    assert!(!body.contains("leak-bait-key-xxxxx"));
}

#[tokio::test]
async fn fetch_reports_parse_error_on_malformed_json() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_string("{not json"))
        .mount(&server)
        .await;

    let client = Client::with_base_url(server.uri());
    let err = client.fetch(ok_request()).await.unwrap_err();
    assert!(matches!(err, WindyError::Parse(_)));
}

#[tokio::test]
async fn fetch_propagates_warning_field() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ts": [1_700_000_000_000_i64],
            "units": {},
            "warning": "rate limit approaching",
        })))
        .mount(&server)
        .await;

    let client = Client::with_base_url(server.uri());
    let forecast = client
        .fetch(FetchRequest {
            api_key: "k".into(),
            lat: 0.0,
            lon: 0.0,
            model: "gfs".into(),
            parameters: vec![WindyParameter::Temp],
        })
        .await
        .unwrap();
    assert_eq!(forecast.warning.as_deref(), Some("rate limit approaching"),);
}

#[tokio::test]
async fn fetch_preserves_null_values_in_series() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ts": [1_700_000_000_000_i64, 1_700_003_600_000_i64, 1_700_007_200_000_i64],
            "units": {"temp-surface": "K"},
            "temp-surface": [293.15, null, 294.25],
        })))
        .mount(&server)
        .await;

    let client = Client::with_base_url(server.uri());
    let forecast = client.fetch(ok_request()).await.unwrap();
    assert_eq!(
        forecast.values(WindyParameter::Temp).unwrap(),
        &[Some(293.15), None, Some(294.25)],
    );
}

#[tokio::test]
async fn fetch_rejects_empty_ts() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ts": [],
            "units": {},
        })))
        .mount(&server)
        .await;

    let client = Client::with_base_url(server.uri());
    let err = client.fetch(ok_request()).await.unwrap_err();
    assert!(matches!(err, WindyError::EmptyForecast));
}

#[tokio::test]
async fn fetch_rejects_series_length_mismatch() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ts": [1_700_000_000_000_i64, 1_700_003_600_000_i64],
            "temp-surface": [293.15],
        })))
        .mount(&server)
        .await;

    let client = Client::with_base_url(server.uri());
    let err = client.fetch(ok_request()).await.unwrap_err();
    let WindyError::SeriesLengthMismatch { key, expected, got } = err else {
        panic!("expected SeriesLengthMismatch, got {err:?}")
    };
    assert_eq!(key, "temp-surface");
    assert_eq!(expected, 2);
    assert_eq!(got, 1);
}

#[tokio::test]
async fn fetch_ignores_unknown_non_numeric_fields() {
    // Forward-compat: Windy adding a scalar metadata
    // field should not break parsing.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ts": [1_700_000_000_000_i64],
            "units": {},
            "elevation": 402,
            "model_id": "gfs-0p25",
            "metadata": { "run": "18z" },
            "temp-surface": [293.15],
        })))
        .mount(&server)
        .await;

    let client = Client::with_base_url(server.uri()).clone();
    let forecast = client.fetch(ok_request()).await.unwrap();
    assert_eq!(forecast.series.len(), 1);
    assert!(forecast.series.contains_key("temp-surface"));
}

#[tokio::test]
async fn fetch_rejects_empty_parameters_early() {
    // No server needed — we short-circuit before the HTTP
    // call.
    let client = Client::new();
    let err = client
        .fetch(FetchRequest {
            api_key: "k".into(),
            lat: 0.0,
            lon: 0.0,
            model: "gfs".into(),
            parameters: vec![],
        })
        .await
        .unwrap_err();
    assert!(matches!(err, WindyError::NoParameters));
}

#[tokio::test]
async fn fetch_rejects_oversized_response_via_content_length() {
    // Client has a tight cap; wiremock's auto-set
    // content-length exceeds it and we short-circuit.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .respond_with(
            ResponseTemplate::new(200).set_body_string("x".repeat(200)),
        )
        .mount(&server)
        .await;

    let client =
        Client::with_base_url(server.uri()).with_max_response_bytes(100);
    let err = client.fetch(ok_request()).await.unwrap_err();
    assert!(matches!(err, WindyError::ResponseTooLarge { limit: 100 },));
}

#[tokio::test]
async fn fetch_rejects_oversized_error_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .respond_with(
            ResponseTemplate::new(500).set_body_string("x".repeat(200)),
        )
        .mount(&server)
        .await;

    let client =
        Client::with_base_url(server.uri()).with_max_error_body_bytes(100);
    let err = client.fetch(ok_request()).await.unwrap_err();
    assert!(matches!(err, WindyError::ResponseTooLarge { limit: 100 },));
}

#[tokio::test]
async fn fetch_does_not_follow_redirects() {
    // A redirect would normally re-POST the body
    // (including the api key) to the target. Our policy
    // is `Policy::none()`, so reqwest surfaces the 302
    // as a non-success status instead.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .respond_with(
            ResponseTemplate::new(302)
                .append_header("location", "https://evil.example/"),
        )
        .mount(&server)
        .await;

    let client = Client::with_base_url(server.uri());
    let err = client.fetch(ok_request()).await.unwrap_err();
    let WindyError::Api { status, .. } = err else {
        panic!("expected Api, got {err:?}")
    };
    assert_eq!(status, 302);
}

#[tokio::test]
async fn fetch_with_config_uses_loaded_api_key() {
    use crate::config::Config;
    use tempfile::TempDir;

    // Build a minimal on-disk config so the api_key gets
    // populated by Config::load.
    let tmp = TempDir::new().unwrap();
    let key_path = tmp.path().join("key.txt");
    std::fs::write(&key_path, "loaded-key\n").unwrap();
    let cfg_path = tmp.path().join("config.toml");
    std::fs::write(
        &cfg_path,
        "[windy]\n\
         api_key_file = \"key.txt\"\n\
         lat = 46.05\n\
         lon = 14.51\n\
         parameters = [\"temp\"]\n\
         [trmnl]\n\
         mode = \"byos\"\n\
         public_image_base = \"http://x/\"\n",
    )
    .unwrap();
    let cfg = Config::load(&cfg_path).unwrap();

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .and(body_json(json!({
            "lat": 46.05,
            "lon": 14.51,
            "model": "gfs",
            "parameters": ["temp"],
            "key": "loaded-key",
            "levels": ["surface"],
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ts": [1_700_000_000_000_i64],
            "units": {},
            "temp-surface": [293.15],
        })))
        .mount(&server)
        .await;

    let client = Client::with_base_url(server.uri());
    let forecast = client.fetch_with_config(&cfg.windy).await.unwrap();
    assert_eq!(forecast.timestamps.len(), 1);
}

#[tokio::test]
async fn fetch_with_config_errors_if_key_not_loaded() {
    use crate::config::Config;

    let cfg = Config::from_toml_str(
        r#"
        [windy]
        api_key_file = "k.txt"
        lat = 0
        lon = 0

        [trmnl]
        mode = "byos"
        public_image_base = "http://x/"
        "#,
    )
    .unwrap();
    let client = Client::new();
    let err = client.fetch_with_config(&cfg.windy).await.unwrap_err();
    assert!(matches!(err, WindyError::MissingApiKey));
}

#[tokio::test]
async fn fetch_surfaces_invalid_timestamp() {
    // i64::MAX ms is ~292M years past 1970 — outside
    // chrono's NaiveDateTime range.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ts": [i64::MAX],
            "units": {},
        })))
        .mount(&server)
        .await;

    let client = Client::with_base_url(server.uri());
    let err = client.fetch(ok_request()).await.unwrap_err();
    let WindyError::InvalidTimestamp { ms } = err else {
        panic!("expected InvalidTimestamp, got {err:?}")
    };
    assert_eq!(ms, i64::MAX);
}

#[test]
fn endpoint_composes_base_and_path() {
    let client = Client::with_base_url("http://host.invalid");
    assert_eq!(
        client.endpoint(),
        "http://host.invalid/api/point-forecast/v2",
    );
}

#[test]
fn default_endpoint_constant_matches_composition() {
    let client = Client::new();
    assert_eq!(client.endpoint(), DEFAULT_ENDPOINT);
}

#[test]
fn truncate_long_utf8_string_cleanly() {
    let s = "a".repeat(600);
    let out = truncate(s, 512);
    assert!(out.ends_with("…(truncated)"));
    assert!(out.len() <= 512 + "…(truncated)".len());
}

#[test]
fn truncate_short_string_unchanged() {
    let s = "hello".to_owned();
    assert_eq!(truncate(s.clone(), 512), s);
}

#[test]
fn redact_secret_replaces_all_occurrences() {
    let s = redact_secret("key=abc and also abc", "abc");
    assert_eq!(s, "key=<redacted> and also <redacted>");
}

#[test]
fn redact_secret_empty_is_noop() {
    assert_eq!(redact_secret("hello", ""), "hello");
}

// Live real-network test. Gated on the `live-tests`
// feature flag so it doesn't compile into regular
// builds. Run manually with:
//
//   BELLWETHER_WINDY_KEY=... \
//   cargo test --features live-tests -p bellwether \
//     -- --ignored live_windy
#[cfg(feature = "live-tests")]
#[tokio::test]
#[ignore = "hits the live Windy API; requires BELLWETHER_WINDY_KEY"]
async fn live_windy() {
    let key = std::env::var("BELLWETHER_WINDY_KEY")
        .expect("BELLWETHER_WINDY_KEY env var");
    let client = Client::new();
    let forecast = client
        .fetch(FetchRequest {
            api_key: key,
            lat: 46.05,
            lon: 14.51,
            model: "gfs".into(),
            parameters: vec![WindyParameter::Temp, WindyParameter::Wind],
        })
        .await
        .expect("live fetch must succeed");
    assert!(!forecast.timestamps.is_empty());
    assert!(
        forecast.values(WindyParameter::Temp).is_some(),
        "expected temperature series"
    );
}
