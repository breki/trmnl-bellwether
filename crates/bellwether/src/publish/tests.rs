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

/// Render config matching the TRMNL OG display. Used by
/// the end-to-end test below so the generated BMP is a
/// real dashboard image we could in principle hand to a
/// device.
fn trmnl_og_render_cfg() -> RenderConfig {
    RenderConfig {
        width: 800,
        height: 480,
        ..Default::default()
    }
}

/// Rich 72-hour forecast fixture: every `*-surface`
/// series populated with plausible weather. Used by the
/// end-to-end test that exercises the full model →
/// SVG → renderer pipeline.
///
/// Takes an explicit `start: DateTime<Utc>` so the
/// fixture is deterministic across test runs — the
/// assertion test pins a fixed reference day that
/// produces a full three-tile dashboard regardless of
/// wall-clock. A caller who wants "now" can pass
/// `Utc::now() + 30m`.
fn rich_forecast_fixture_at(
    start: chrono::DateTime<chrono::Utc>,
) -> serde_json::Value {
    let start_ms = start.timestamp_millis();
    let mut ts: Vec<i64> = Vec::with_capacity(72);
    let mut temp: Vec<f64> = Vec::with_capacity(72);
    let mut clouds: Vec<f64> = Vec::with_capacity(72);
    let mut precip: Vec<f64> = Vec::with_capacity(72);
    let mut wind_u: Vec<f64> = Vec::with_capacity(72);
    let mut wind_v: Vec<f64> = Vec::with_capacity(72);
    for h in 0..72_i64 {
        ts.push(start_ms + h * 3_600_000);
        // Temperature wobbles around 10 °C (283.15 K)
        // with a diurnal swing — not realistic, but
        // enough that daily highs differ from lows.
        #[allow(clippy::cast_precision_loss)]
        let diurnal = ((h % 24) as f64 - 12.0).abs() / 12.0;
        temp.push(283.15 + (1.0 - diurnal) * 6.0);
        clouds.push(40.0);
        // One rainy hour on day 2 (index 30 = roughly
        // midday of the second full day) so one tile
        // ends up labelled Rain.
        precip.push(if h == 30 { 1.2 } else { 0.0 });
        wind_u.push(3.0);
        wind_v.push(4.0);
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
        "temp-surface": temp,
        "clouds-surface": clouds,
        "precip-surface": precip,
        "wind_u-surface": wind_u,
        "wind_v-surface": wind_v,
    })
}

#[tokio::test]
async fn tick_once_renders_plausible_trmnl_og_bmp() {
    // End-to-end sanity: feed a 72-hour forecast through
    // the real model → SVG → Renderer::with_default_fonts
    // pipeline at TRMNL OG resolution. Assert the output
    // is a syntactically valid 1-bit BMP of the exact
    // expected size (62-byte header+palette +
    // 100-byte-per-row × 480 rows = 48062) and contains
    // enough black pixels that text + icons are
    // contributing — a silently-broken font pipeline
    // would leave the layout mostly white.
    //
    // The fixture's start time is irrelevant to this
    // assertion — `tick_once` calls `Utc::now()` itself
    // for the day-bucketing. The pipeline produces a
    // full BMP in every case (tiles just render as
    // placeholders when they don't match the forecast
    // window), so the size/coverage checks are
    // wall-clock-stable.
    let start = chrono::Utc::now() + chrono::Duration::minutes(30);
    let (_server, windy) = stubbed_windy(rich_forecast_fixture_at(start)).await;
    // Wrap the sink so `tick_once` can publish through
    // it but this test can still see every byte it
    // delivered.
    let capture = CapturingSink::default();
    let loop_ = PublishLoop::new(
        windy,
        ok_request(),
        Renderer::with_default_fonts(),
        trmnl_og_render_cfg(),
        capture.clone(),
        Duration::from_secs(60),
    );

    let filename = loop_.tick_once().await.unwrap();
    let (published_name, bmp) = capture.take().expect("one publish");
    assert_eq!(published_name, filename);

    // Size: 62 (header + 2-colour palette) + 100 bytes
    // per row × 480 rows = 48062 exactly. The renderer's
    // own tests lock this math for arbitrary dimensions;
    // pin it here too so a publish-loop change that
    // inadvertently swaps the encoder fails visibly.
    assert_eq!(bmp.len(), 62 + 100 * 480);

    // Black-pixel count sanity: the layout has three
    // icons (lots of filled shapes) and several text
    // labels. Expect well over a thousand black pixels
    // on a 800×480 canvas; a silently broken pipeline
    // would be nearly all white.
    let black = count_black_pixels(&bmp);
    assert!(
        black > 2000,
        "expected substantial black coverage; got {black}",
    );
}

/// Regenerate `target/dashboard-sample.bmp` for eyeball
/// inspection during layout work. Marked `#[ignore]` so
/// it doesn't run by default — `cargo xtask test
/// generate_dashboard_sample_bmp -- --ignored` invokes
/// it on demand. The BMP it writes opens in any
/// Windows image viewer.
#[tokio::test]
#[ignore = "manual tool: writes target/dashboard-sample.bmp for eyeball"]
async fn generate_dashboard_sample_bmp() {
    let now = chrono::Utc::now();
    let start = now + chrono::Duration::minutes(30);
    let (_server, windy) = stubbed_windy(rich_forecast_fixture_at(start)).await;
    let forecast = windy.fetch(&ok_request()).await.expect("fetch");
    let cfg = trmnl_og_render_cfg();
    let ctx = dashboard::ModelContext {
        tz: cfg.timezone,
        location: dashboard::astro::GeoPoint {
            lat_deg: ok_request().lat,
            lon_deg: ok_request().lon,
        },
        now,
        telemetry: DeviceTelemetry::default(),
    };
    let now_local = ctx.now.with_timezone(&ctx.tz).time();
    let model = dashboard::build_model(&forecast, ctx);
    let svg = dashboard::build_svg(&model, now_local);
    let bmp = Renderer::with_default_fonts()
        .render_to_bmp(&svg, &cfg)
        .expect("render");
    let out = std::env::current_dir()
        .expect("cwd")
        .join("target")
        .join("dashboard-sample.bmp");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    std::fs::write(&out, &bmp).unwrap();
    eprintln!("wrote {} ({} bytes)", out.display(), bmp.len());
}

type CapturedPublish = (String, Vec<u8>);

/// [`ImageSink`] that stashes the published bytes so
/// tests can inspect the actual BMP rather than just
/// the filename and size. The production [`MockSink`]
/// stores only `(filename, len)` to keep its assertion
/// surface small; this one is for the end-to-end test
/// where we really do want the bytes.
#[derive(Debug, Default, Clone)]
struct CapturingSink {
    inner: Arc<Mutex<Option<CapturedPublish>>>,
}

impl CapturingSink {
    fn take(&self) -> Option<CapturedPublish> {
        self.inner.lock().unwrap().take()
    }
}

impl ImageSink for CapturingSink {
    fn publish_image(
        &self,
        filename: String,
        bytes: Vec<u8>,
    ) -> Result<(), SinkError> {
        *self.inner.lock().unwrap() = Some((filename, bytes));
        Ok(())
    }
}

/// Count black pixels in a 1-bit BMP. Duplicates some
/// logic from `render::tests::bmp_to_bits`, but the
/// test modules can't cross-reference each other's
/// helpers without exposing them more broadly than
/// they warrant.
fn count_black_pixels(bmp: &[u8]) -> usize {
    let offset =
        u32::from_le_bytes([bmp[10], bmp[11], bmp[12], bmp[13]]) as usize;
    let width =
        u32::try_from(i32::from_le_bytes([bmp[18], bmp[19], bmp[20], bmp[21]]))
            .unwrap();
    let height =
        u32::try_from(i32::from_le_bytes([bmp[22], bmp[23], bmp[24], bmp[25]]))
            .unwrap();
    let row_bytes = ((width.div_ceil(8)).div_ceil(4)) * 4;
    let mut count = 0;
    for y in 0..height {
        let row_start = offset + (y as usize) * row_bytes as usize;
        for x in 0..width {
            let byte = bmp[row_start + (x / 8) as usize];
            let bit = (byte >> (7 - (x % 8))) & 1;
            if bit == 0 {
                count += 1;
            }
        }
    }
    count
}

#[test]
fn image_sink_default_latest_telemetry_is_all_none() {
    // MockSink doesn't override `latest_telemetry`, so
    // the default-method contract is that it returns
    // a fully-empty telemetry.
    let sink = MockSink::new();
    let t = sink.latest_telemetry();
    assert_eq!(t, DeviceTelemetry::default());
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
