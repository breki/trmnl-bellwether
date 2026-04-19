//! Integration tests for the TRMNL BYOS routes.

use axum::body::{Body, Bytes};
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use tower::ServiceExt;

use super::*;

fn test_state() -> TrmnlState {
    TrmnlState::new("http://host.test/images", RefreshInterval::from_secs(900))
        .expect("valid base URL")
}

async fn body_json(resp: Response) -> serde_json::Value {
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn body_bytes(resp: Response) -> Bytes {
    axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap()
}

#[test]
fn validate_filename_accepts_reasonable_names() {
    for ok in &["today.bmp", "a_b-c.1.bmp", "X", "placeholder.bmp"] {
        assert!(validate_filename(ok).is_ok(), "{ok} should be valid");
    }
}

#[test]
fn validate_filename_rejects_bad_chars_and_shapes() {
    use InvalidFilename::{BadChar, Empty, LeadingDot, TooLong};
    assert_eq!(validate_filename(""), Err(Empty));
    assert_eq!(validate_filename(".hidden"), Err(LeadingDot));
    assert_eq!(validate_filename("a/b.bmp"), Err(BadChar('/')));
    assert_eq!(validate_filename("a b.bmp"), Err(BadChar(' ')));
    assert_eq!(validate_filename("a?.bmp"), Err(BadChar('?')));
    assert_eq!(validate_filename("a\nb.bmp"), Err(BadChar('\n')));
    let long = "a".repeat(MAX_FILENAME_LEN + 1);
    assert_eq!(validate_filename(&long), Err(TooLong));
}

#[test]
fn normalize_base_strips_trailing_slash_and_validates_scheme() {
    assert_eq!(
        normalize_base("http://x/images/").unwrap(),
        "http://x/images",
    );
    assert_eq!(
        normalize_base("https://x/images").unwrap(),
        "https://x/images",
    );
    assert_eq!(normalize_base(""), Err(InvalidBaseUrl::Empty));
    assert_eq!(normalize_base("/"), Err(InvalidBaseUrl::Empty));
    assert_eq!(normalize_base("///"), Err(InvalidBaseUrl::Empty));
    assert_eq!(
        normalize_base("malina.local/images"),
        Err(InvalidBaseUrl::NoScheme),
    );
    assert_eq!(
        normalize_base("http://x/images?query=1"),
        Err(InvalidBaseUrl::UnexpectedQuery),
    );
    assert_eq!(
        normalize_base("http://x/images#frag"),
        Err(InvalidBaseUrl::UnexpectedQuery),
    );
}

#[test]
fn trmnl_state_requires_valid_base() {
    assert!(
        TrmnlState::new("not-a-url", RefreshInterval::from_secs(900)).is_err(),
    );
}

#[test]
fn image_store_composite_lock_keeps_latest_and_map_in_sync() {
    let store = ImageStore::new();
    assert!(store.latest_filename().is_none());
    store
        .put_image("a.bmp".into(), Bytes::from_static(b"a"))
        .unwrap();
    store
        .put_image("b.bmp".into(), Bytes::from_static(b"b"))
        .unwrap();
    assert_eq!(store.latest_filename().as_deref(), Some("b.bmp"));
    // Older images remain fetchable so in-flight device
    // polls don't 404.
    assert_eq!(store.get_image("a.bmp").unwrap(), Bytes::from_static(b"a"));
    assert_eq!(store.get_image("b.bmp").unwrap(), Bytes::from_static(b"b"));
    assert!(store.get_image("nope.bmp").is_none());
}

#[test]
fn image_store_evicts_oldest_beyond_retention_cap() {
    // Monotonic filenames like the publish loop produces;
    // after inserting more than MAX_RETAINED_IMAGES, the
    // oldest should be gone and the latest should remain
    // fetchable.
    let store = ImageStore::new();
    for i in 0..(MAX_RETAINED_IMAGES + 3) {
        store
            .put_image(format!("dash-{i:08}.bmp"), Bytes::from_static(b"x"))
            .unwrap();
    }
    for i in 0..3 {
        assert!(
            store.get_image(&format!("dash-{i:08}.bmp")).is_none(),
            "oldest image dash-{i:08}.bmp should have been evicted",
        );
    }
    for i in 3..(MAX_RETAINED_IMAGES + 3) {
        assert!(
            store.get_image(&format!("dash-{i:08}.bmp")).is_some(),
            "recent image dash-{i:08}.bmp should still be present",
        );
    }
    let latest_n = MAX_RETAINED_IMAGES + 2;
    assert_eq!(
        store.latest_filename().as_deref(),
        Some(format!("dash-{latest_n:08}.bmp").as_str()),
    );
}

#[test]
fn image_store_never_evicts_the_current_latest() {
    // Guard against the corner case where a non-monotonic
    // filename is inserted last: evicting the oldest by
    // lexical order must not drop the current `latest`.
    let store = ImageStore::new();
    for i in 0..MAX_RETAINED_IMAGES {
        store
            .put_image(format!("z-{i:03}.bmp"), Bytes::from_static(b"x"))
            .unwrap();
    }
    // `a.bmp` sorts earliest but is the most recent.
    store
        .put_image("a.bmp".into(), Bytes::from_static(b"new"))
        .unwrap();
    assert_eq!(
        store.get_image("a.bmp").as_deref(),
        Some(b"new".as_slice()),
        "the most recently inserted image must never be evicted",
    );
    assert_eq!(store.latest_filename().as_deref(), Some("a.bmp"));
}

#[test]
fn image_store_rejects_bad_filenames() {
    let store = ImageStore::new();
    assert_eq!(
        store.put_image("a/b.bmp".into(), Bytes::from_static(b"x")),
        Err(InvalidFilename::BadChar('/')),
    );
    assert_eq!(
        store.put_image(String::new(), Bytes::from_static(b"x")),
        Err(InvalidFilename::Empty),
    );
    assert_eq!(
        store.put_image(".hidden".into(), Bytes::from_static(b"x")),
        Err(InvalidFilename::LeadingDot),
    );
}

#[test]
fn refresh_interval_serializes_as_u32_seconds() {
    let json = serde_json::to_value(RefreshInterval::from_secs(900)).unwrap();
    assert_eq!(json, serde_json::json!(900));
}

#[tokio::test]
async fn display_returns_503_when_store_empty() {
    let state = test_state();
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/display")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn display_returns_manifest_for_latest_image() {
    let state = test_state();
    state
        .put_image("today-1430.bmp".into(), Bytes::from_static(b"BM..fake"))
        .unwrap();
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/display")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["filename"], "today-1430.bmp");
    assert_eq!(json["image_url"], "http://host.test/images/today-1430.bmp",);
    assert_eq!(json["refresh_rate"], 900);
    assert_eq!(json["status"], 0);
    assert_eq!(json["update_firmware"], false);
    assert_eq!(json["reset_firmware"], false);
    // firmware_url is omitted when None.
    assert!(json.get("firmware_url").is_none());
}

#[tokio::test]
async fn display_points_at_most_recently_put_image() {
    let state = test_state();
    state
        .put_image("old.bmp".into(), Bytes::from_static(b"1"))
        .unwrap();
    state
        .put_image("new.bmp".into(), Bytes::from_static(b"2"))
        .unwrap();
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/display")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(resp).await;
    assert_eq!(json["filename"], "new.bmp");
}

#[tokio::test]
async fn image_route_serves_bmp_bytes_and_sets_content_type() {
    let state = test_state();
    state
        .put_image("foo.bmp".into(), Bytes::from_static(b"BM\x01\x02\x03"))
        .unwrap();
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/images/foo.bmp")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "image/bmp",
    );
    assert_eq!(&body_bytes(resp).await[..], b"BM\x01\x02\x03");
}

#[tokio::test]
async fn image_route_returns_404_for_missing_filename() {
    let app = router(test_state());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/images/missing.bmp")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn preview_returns_latest_bmp_bytes_without_token() {
    // `/preview.bmp` must be reachable from an
    // unauthenticated browser on the landing page even
    // when an access token is configured, and must
    // return the bytes of the most recently rendered
    // image so the preview updates between renders.
    let state = test_state().with_access_token("s3cret");
    state
        .put_image("old.bmp".into(), Bytes::from_static(b"old"))
        .unwrap();
    state
        .put_image("new.bmp".into(), Bytes::from_static(b"BMnew"))
        .unwrap();
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/preview.bmp")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "image/bmp",
    );
    assert_eq!(
        resp.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store",
    );
    assert_eq!(&body_bytes(resp).await[..], b"BMnew");
}

#[tokio::test]
async fn preview_returns_404_when_no_image_rendered_yet() {
    // Chosen over 503 so the landing page's `<img>`
    // `onerror` handler fires immediately instead of
    // the browser treating the response as transient
    // and retrying.
    let app = router(test_state());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/preview.bmp")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert!(resp.headers().get(header::CACHE_CONTROL).is_none());
}

#[tokio::test]
async fn log_accepts_known_fields() {
    let app = router(test_state());
    let body = serde_json::json!({
        "battery_voltage": 3.92,
        "rssi": -67,
        "fw_version": "1.4.2"
    });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/log")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn log_persists_battery_voltage_into_state() {
    // After a successful `/api/log` post the state's
    // cached telemetry should include the device's
    // last-reported battery voltage, ready for the
    // publish loop to read on the next tick.
    let state = test_state();
    let app = router(state.clone());
    let body = serde_json::json!({
        "battery_voltage": 4.01,
        "rssi": -67,
    });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/log")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let telemetry = state.telemetry();
    let voltage = telemetry.battery_voltage.expect("voltage persisted");
    assert!(
        (voltage - 4.01).abs() < 1e-3,
        "expected ~4.01 V, got {voltage}",
    );
}

#[tokio::test]
async fn log_without_battery_voltage_keeps_previous_value() {
    // The TRMNL firmware posts `/api/log` for many
    // reasons (wake-up reports, error reports,
    // keepalives) and not every post includes a
    // battery reading. A merge-semantic cache keeps
    // the last-known voltage until a fresh one
    // arrives; a naive "overwrite" would blink the
    // battery indicator to "unknown" every time a
    // keepalive came in.
    let state = test_state();
    state.update_telemetry(DeviceTelemetry {
        battery_voltage: Some(3.7),
    });
    assert_eq!(state.telemetry().battery_voltage, Some(3.7));

    let app = router(state.clone());
    let body = serde_json::json!({ "rssi": -70 });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/log")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert_eq!(state.telemetry().battery_voltage, Some(3.7));
}

#[test]
fn trmnl_state_as_image_sink_exposes_telemetry() {
    // Via the ImageSink trait extension the publish
    // loop sees the same cached value the `/api/log`
    // handler writes. Goes through the trait to
    // catch any accidental method-name drift.
    let state = test_state();
    state.update_telemetry(DeviceTelemetry {
        battery_voltage: Some(3.8),
    });
    let sink: &dyn ImageSink = &state;
    assert_eq!(sink.latest_telemetry().battery_voltage, Some(3.8));
}

#[tokio::test]
async fn log_rejects_oversized_body() {
    let app = router(test_state());
    let big = "x".repeat(MAX_LOG_BODY_BYTES + 1);
    let body = serde_json::json!({ "fw_version": big });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/log")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn access_token_required_when_configured() {
    let state = test_state().with_access_token("s3cret");
    state
        .put_image("x.bmp".into(), Bytes::from_static(b"x"))
        .unwrap();
    let app = router(state);
    // No Access-Token header.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/display")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    // Wrong token.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/display")
                .header("access-token", "wrong")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    // Correct token.
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/display")
                .header("access-token", "s3cret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn setup_returns_manifest_for_latest_image() {
    let state = test_state();
    state
        .put_image("today-1430.bmp".into(), Bytes::from_static(b"BM..fake"))
        .unwrap();
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/setup")
                .header("ID", "AA:BB:CC:DD:EE:FF")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["status"], 200);
    assert_eq!(json["filename"], "today-1430.bmp");
    assert_eq!(json["image_url"], "http://host.test/images/today-1430.bmp");
    assert_eq!(json["friendly_id"], "DDEEFF");
    assert!(
        json["api_key"].as_str().is_some_and(|s| !s.is_empty()),
        "api_key should be a non-empty string",
    );
}

#[tokio::test]
async fn setup_returns_503_when_store_empty() {
    // Mirrors `/api/display` — if we advertised an
    // `image_url` here with nothing behind it, the
    // device would 404 on its first fetch.
    let state = test_state();
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/setup")
                .header("ID", "01:23:45:67:89:AB")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn setup_requires_id_header() {
    let state = test_state();
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/setup")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn setup_returns_configured_access_token_as_api_key() {
    let state = test_state().with_access_token("s3cret");
    state
        .put_image("x.bmp".into(), Bytes::from_static(b"x"))
        .unwrap();
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/setup")
                .header("ID", "AA:BB:CC:DD:EE:FF")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Setup itself is exempt from the access-token check —
    // a fresh device has no token yet.
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["api_key"], "s3cret");
}

#[tokio::test]
async fn setup_is_exempt_from_access_token_middleware() {
    let state = test_state().with_access_token("s3cret");
    state
        .put_image("x.bmp".into(), Bytes::from_static(b"x"))
        .unwrap();
    let app = router(state);
    // No Access-Token header — should still work.
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/setup")
                .header("ID", "AA:BB:CC:DD:EE:FF")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn setup_returns_default_api_key_when_no_token_configured() {
    let state = test_state();
    state
        .put_image("x.bmp".into(), Bytes::from_static(b"x"))
        .unwrap();
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/setup")
                .header("ID", "AA:BB:CC:DD:EE:FF")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["api_key"], DEFAULT_UNCONFIGURED_API_KEY);
}

#[test]
fn friendly_id_from_mac_takes_last_six_hex_uppercase() {
    assert_eq!(
        FriendlyId::from_mac("AA:BB:CC:DD:EE:FF").to_string(),
        "DDEEFF"
    );
    assert_eq!(
        FriendlyId::from_mac("aa-bb-cc-dd-ee-ff").to_string(),
        "DDEEFF"
    );
    assert_eq!(FriendlyId::from_mac("AABBCCDDEEFF").to_string(), "DDEEFF");
    // Fewer than 6 hex chars → pad with leading zeros so
    // the ID is always a stable 6-char string.
    assert_eq!(FriendlyId::from_mac("AB").to_string(), "0000AB");
    assert_eq!(FriendlyId::from_mac("").to_string(), "000000");
    // Non-hex chars are filtered out.
    assert_eq!(
        FriendlyId::from_mac("xx-01-23-45-67-89-ab").to_string(),
        "6789AB",
    );
}

#[tokio::test]
async fn access_token_empty_string_is_ignored() {
    // An operator who forgets to set BELLWETHER_ACCESS_TOKEN
    // gets "" from `env::var(...).unwrap_or_default()`. We
    // treat that as "no token required" rather than "every
    // request must send an empty token".
    let state = test_state().with_access_token("");
    state
        .put_image("x.bmp".into(), Bytes::from_static(b"x"))
        .unwrap();
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/display")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
