//! Tests for [`super::PublishLoop`].
//!
//! `tick_once` is exercised deterministically against a
//! `MockSink`. The HTTP-touching path uses `wiremock`
//! fixtures for happy-path and error coverage.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::clients::windy::{Client as WindyClient, FetchRequest};
use crate::config::{RenderConfig, WindyParameter};
use crate::render::Renderer;

use super::*;

#[derive(Debug, Default)]
struct MockSink {
    calls: Mutex<Vec<(String, usize)>>,
    scripted_results: Mutex<Vec<Result<(), SinkError>>>,
}

impl MockSink {
    fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn record(&self) -> Vec<(String, usize)> {
        self.calls.lock().unwrap().clone()
    }

    /// Queue a sequence of responses. `publish_image`
    /// returns them in order; unscripted calls return
    /// `Ok(())`.
    fn set_scripted(&self, results: Vec<Result<(), SinkError>>) {
        *self.scripted_results.lock().unwrap() = results;
    }
}

impl ImageSink for Arc<MockSink> {
    fn publish_image(
        &self,
        filename: String,
        bytes: Vec<u8>,
    ) -> Result<(), SinkError> {
        self.calls.lock().unwrap().push((filename, bytes.len()));
        let mut scripted = self.scripted_results.lock().unwrap();
        if scripted.is_empty() {
            Ok(())
        } else {
            scripted.remove(0)
        }
    }
}

fn ok_request() -> FetchRequest {
    FetchRequest {
        api_key: "test".into(),
        lat: 46.05,
        lon: 14.51,
        model: "gfs".into(),
        parameters: vec![WindyParameter::Temp, WindyParameter::Wind],
    }
}

fn forecast_fixture() -> serde_json::Value {
    json!({
        "ts": [1_700_000_000_000_i64, 1_700_003_600_000_i64],
        "units": { "temp-surface": "K" },
        "temp-surface": [293.15, 294.25],
    })
}

async fn stubbed_windy(body: serde_json::Value) -> (MockServer, WindyClient) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/point-forecast/v2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    let client = WindyClient::with_base_url(server.uri());
    (server, client)
}

fn test_render_cfg() -> RenderConfig {
    RenderConfig {
        width: 64,
        height: 32,
        ..Default::default()
    }
}

#[test]
fn build_dashboard_svg_with_temp_includes_filled_bar() {
    let forecast = Forecast::from_raw_json(
        r#"{"ts":[1700000000000],"units":{},"temp-surface":[293.15]}"#,
    )
    .unwrap();
    let cfg = RenderConfig {
        width: 800,
        height: 480,
        ..Default::default()
    };
    let svg = build_dashboard_svg(&forecast, &cfg);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("width=\"800\""));
    assert!(svg.contains("height=\"480\""));
    // Temp bar uses fill=black.
    assert!(svg.contains("fill=\"black\""));
    // No X overlay when temp is present.
    assert!(!svg.contains("<line"));
}

#[test]
fn build_dashboard_svg_without_temp_renders_x_overlay() {
    let forecast = Forecast::from_raw_json(
        r#"{"ts":[1700000000000],"units":{},"wind_u-surface":[1.2]}"#,
    )
    .unwrap();
    let cfg = RenderConfig {
        width: 800,
        height: 480,
        ..Default::default()
    };
    let svg = build_dashboard_svg(&forecast, &cfg);
    // Two diagonal lines forming the X overlay.
    assert_eq!(svg.matches("<line").count(), 2);
    // No filled temp bar.
    assert!(!svg.contains("fill=\"black\""));
}

#[tokio::test]
async fn next_filename_is_monotonic_and_url_safe() {
    let (_srv, windy) = stubbed_windy(forecast_fixture()).await;
    let sink = MockSink::new();
    let loop_ = PublishLoop::new(
        windy,
        ok_request(),
        Renderer::new(),
        test_render_cfg(),
        sink,
        Duration::from_secs(60),
    );
    let a = loop_.next_filename();
    let b = loop_.next_filename();
    let c = loop_.next_filename();
    assert_eq!(a, "dash-00000000.bmp");
    assert_eq!(b, "dash-00000001.bmp");
    assert_eq!(c, "dash-00000002.bmp");
    for name in [&a, &b, &c] {
        for ch in name.chars() {
            assert!(
                ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'),
                "{ch:?} not url-safe",
            );
        }
        assert!(!name.starts_with('.'));
        assert!(!name.starts_with('-'));
    }
}

#[tokio::test]
async fn tick_once_publishes_a_bmp_to_the_sink() {
    let (_server, windy) = stubbed_windy(forecast_fixture()).await;
    let sink = MockSink::new();
    let loop_ = PublishLoop::new(
        windy,
        ok_request(),
        Renderer::new(),
        test_render_cfg(),
        sink.clone(),
        Duration::from_secs(60),
    );
    let filename = loop_.tick_once().await.unwrap();
    let calls = sink.record();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, filename);
    assert!(calls[0].1 >= 62);
}

#[tokio::test]
async fn tick_once_filenames_are_strictly_increasing() {
    let (_server, windy) = stubbed_windy(forecast_fixture()).await;
    let sink = MockSink::new();
    let loop_ = PublishLoop::new(
        windy,
        ok_request(),
        Renderer::new(),
        test_render_cfg(),
        sink.clone(),
        Duration::from_secs(60),
    );
    let a = loop_.tick_once().await.unwrap();
    let b = loop_.tick_once().await.unwrap();
    assert_ne!(a, b);
    assert!(b > a, "expected {b} > {a}");
}

#[tokio::test]
async fn tick_once_propagates_sink_errors() {
    let (_server, windy) = stubbed_windy(forecast_fixture()).await;
    let sink = MockSink::new();
    sink.set_scripted(vec![Err("disk full".into())]);
    let loop_ = PublishLoop::new(
        windy,
        ok_request(),
        Renderer::new(),
        test_render_cfg(),
        sink,
        Duration::from_secs(60),
    );
    let err = loop_.tick_once().await.unwrap_err();
    let PublishError::Sink(inner) = err else {
        panic!("expected Sink, got {err:?}")
    };
    assert_eq!(inner.to_string(), "disk full");
}

#[tokio::test]
async fn tick_once_surfaces_windy_api_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/point-forecast/v2"))
        .respond_with(ResponseTemplate::new(500).set_body_string("nope"))
        .mount(&server)
        .await;
    let windy = WindyClient::with_base_url(server.uri());
    let sink = MockSink::new();
    let loop_ = PublishLoop::new(
        windy,
        ok_request(),
        Renderer::new(),
        test_render_cfg(),
        sink.clone(),
        Duration::from_secs(60),
    );
    let err = loop_.tick_once().await.unwrap_err();
    assert!(matches!(err, PublishError::Windy(_)));
    assert!(sink.record().is_empty());
}

#[tokio::test]
async fn run_recovers_after_transient_sink_error() {
    // Test error-swallowing behavior deterministically:
    // script the sink to Err, then Ok. If run() propagates
    // instead of swallowing, the loop never reaches the
    // second publish.
    let (_server, windy) = stubbed_windy(forecast_fixture()).await;
    let sink = MockSink::new();
    sink.set_scripted(vec![Err("transient".into()), Ok(()), Ok(())]);
    let loop_ = PublishLoop::new(
        windy,
        ok_request(),
        Renderer::new(),
        test_render_cfg(),
        sink.clone(),
        Duration::from_millis(5),
    );
    let handle = tokio::spawn(loop_.run());
    tokio::time::sleep(Duration::from_millis(80)).await;
    handle.abort();
    // At least two sink calls observed — one failed,
    // subsequent succeeded. If the loop propagated the
    // first error, we'd only see one call.
    let calls = sink.record();
    assert!(
        calls.len() >= 2,
        "loop should keep ticking after an error; saw {} calls",
        calls.len(),
    );
}

#[tokio::test]
async fn supervise_logs_when_task_ends() {
    // Can't easily capture tracing output in a test
    // without pulling in `tracing_test`. Verify the
    // supervisor does not deadlock and its spawned
    // handle resolves when the inner task ends.
    let handle = super::supervise("test", async {
        tokio::time::sleep(Duration::from_millis(5)).await;
    });
    tokio::time::timeout(Duration::from_millis(200), handle)
        .await
        .expect("supervisor should complete")
        .expect("outer task should not fail");
}
