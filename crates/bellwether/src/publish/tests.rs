//! Tests for [`super::PublishLoop`].
//!
//! The loop is exercised deterministically against a
//! [`MockSink`] plus an in-memory [`FakeProvider`] that
//! yields a scripted [`WeatherSnapshot`] (or a
//! scripted [`WeatherError`]).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Duration as ChronoDuration, TimeZone, Utc};

use crate::dashboard::astro::GeoPoint;
use crate::render::Renderer;
use crate::weather::{
    WeatherError, WeatherProvider, WeatherSnapshot, WeatherSnapshotBuilder,
};

use super::*;

const TEST_LOCATION: GeoPoint = GeoPoint {
    lat_deg: 46.05,
    lon_deg: 14.51,
};

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

/// In-memory [`WeatherProvider`] used across these
/// tests. Returns either a scripted [`WeatherSnapshot`]
/// on every call, or a fresh [`WeatherError::Provider`]
/// whose message matches what the test constructed it
/// with.
struct FakeProvider {
    response: FakeResponse,
}

enum FakeResponse {
    // Boxed so the enum stays lean — `WeatherSnapshot`
    // is ~240 bytes and makes the error variant pay
    // for it on every allocation.
    Ok(Box<WeatherSnapshot>),
    /// Provider-level error. The message is cloned per
    /// call so the trait object can re-yield a fresh
    /// boxed error (the outer `WeatherError` is not
    /// `Clone`).
    ProviderError(String),
}

impl FakeProvider {
    fn ok(snapshot: WeatherSnapshot) -> Arc<dyn WeatherProvider> {
        Arc::new(Self {
            response: FakeResponse::Ok(Box::new(snapshot)),
        })
    }

    fn provider_error(msg: &str) -> Arc<dyn WeatherProvider> {
        Arc::new(Self {
            response: FakeResponse::ProviderError(msg.to_owned()),
        })
    }
}

#[async_trait]
impl WeatherProvider for FakeProvider {
    fn location(&self) -> GeoPoint {
        TEST_LOCATION
    }

    async fn fetch(&self) -> Result<WeatherSnapshot, WeatherError> {
        match &self.response {
            FakeResponse::Ok(s) => Ok((**s).clone()),
            FakeResponse::ProviderError(msg) => {
                Err(WeatherError::Provider(msg.clone().into()))
            }
        }
    }
}

/// Two-step snapshot matching the older Windy fixture's
/// timing (two consecutive hourly steps around
/// 2023-11-14). Enough to exercise the pipeline; not
/// enough to populate the day tiles.
fn simple_snapshot() -> WeatherSnapshot {
    let t0 = Utc.timestamp_millis_opt(1_700_000_000_000).unwrap();
    let t1 = Utc.timestamp_millis_opt(1_700_003_600_000).unwrap();
    WeatherSnapshotBuilder {
        timestamps: vec![t0, t1],
        temperature_c: vec![Some(20.0), Some(21.1)],
        humidity_pct: vec![None; 2],
        wind_kmh: vec![Some(0.0); 2],
        wind_dir_deg: vec![Some(0.0); 2],
        gust_kmh: vec![None; 2],
        cloud_cover_pct: vec![Some(20.0); 2],
        precip_mm: vec![Some(0.0); 2],
        weather_code: vec![None; 2],
        warning: None,
    }
    .build()
    .expect("valid snapshot")
}

/// Rich 72-hour snapshot with a plausible diurnal
/// temperature swing, steady 40% cloud, and one rainy
/// hour on day 2 so one of the forecast tiles renders
/// as `Rain`. Used by the end-to-end render test.
fn rich_snapshot_at(start: DateTime<Utc>) -> WeatherSnapshot {
    use crate::dashboard::classify::{WeatherCode, WmoCode};
    let n: usize = 72;
    let mut timestamps: Vec<DateTime<Utc>> = Vec::with_capacity(n);
    let mut temperature_c: Vec<Option<f64>> = Vec::with_capacity(n);
    let mut precip_mm: Vec<Option<f64>> = Vec::with_capacity(n);
    let mut weather_code: Vec<Option<WeatherCode>> = Vec::with_capacity(n);
    for h in 0..n {
        let secs = i64::try_from(h).expect("small h") * 3600;
        timestamps.push(start + ChronoDuration::seconds(secs));
        #[allow(clippy::cast_precision_loss)]
        let hour_f = (h % 24) as f64;
        let diurnal = (hour_f - 12.0).abs() / 12.0;
        // 10 °C floor, 16 °C ceiling.
        temperature_c.push(Some(10.0 + (1.0 - diurnal) * 6.0));
        // One rainy hour on day 2 (index 30).
        precip_mm.push(Some(if h == 30 { 1.2 } else { 0.0 }));
        // Salt each forecast day with a distinct WMO
        // code so the preview exercises multiple icon
        // arms (Clear now, Fog tomorrow, Thunderstorm
        // the day after). Once `icon_for_wmo` grows
        // specialised arms, `fidelity = "detailed"`
        // widgets will render different glyphs here
        // without needing further test-data changes.
        let code = match h / 24 {
            0 => WmoCode::PartlyCloudy,
            1 => WmoCode::Fog,
            _ => WmoCode::Thunderstorm,
        };
        weather_code.push(Some(WeatherCode::Wmo(code)));
    }
    WeatherSnapshotBuilder {
        timestamps,
        temperature_c,
        humidity_pct: vec![Some(55.0); n],
        // u=3, v=4 m/s equivalent — 18 km/h SW.
        wind_kmh: vec![Some(18.0); n],
        wind_dir_deg: vec![Some(216.87); n],
        gust_kmh: vec![Some(21.6); n],
        cloud_cover_pct: vec![Some(40.0); n],
        precip_mm,
        weather_code,
        warning: None,
    }
    .build()
    .expect("valid rich snapshot")
}

fn test_render_cfg() -> RenderConfig {
    RenderConfig {
        width: 64,
        height: 32,
        ..Default::default()
    }
}

fn test_layout() -> crate::dashboard::layout::Layout {
    crate::dashboard::layout::Layout::embedded_default().clone()
}

fn loop_cfg(interval: Duration) -> PublishLoopConfig {
    PublishLoopConfig {
        render_cfg: test_render_cfg(),
        layout: test_layout(),
        interval,
    }
}

fn loop_cfg_with_render(
    render_cfg: RenderConfig,
    interval: Duration,
) -> PublishLoopConfig {
    PublishLoopConfig {
        render_cfg,
        layout: test_layout(),
        interval,
    }
}

#[tokio::test]
async fn next_filename_is_monotonic_and_url_safe() {
    let sink = MockSink::new();
    let loop_ = PublishLoop::new(
        FakeProvider::ok(simple_snapshot()),
        Renderer::new(),
        sink,
        loop_cfg(Duration::from_secs(60)),
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
    let sink = MockSink::new();
    let loop_ = PublishLoop::new(
        FakeProvider::ok(simple_snapshot()),
        Renderer::new(),
        sink.clone(),
        loop_cfg(Duration::from_secs(60)),
    );
    let filename = loop_.tick_once().await.unwrap();
    let calls = sink.record();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, filename);
    assert!(calls[0].1 >= 62);
}

#[tokio::test]
async fn tick_once_filenames_are_strictly_increasing() {
    let sink = MockSink::new();
    let loop_ = PublishLoop::new(
        FakeProvider::ok(simple_snapshot()),
        Renderer::new(),
        sink.clone(),
        loop_cfg(Duration::from_secs(60)),
    );
    let a = loop_.tick_once().await.unwrap();
    let b = loop_.tick_once().await.unwrap();
    assert_ne!(a, b);
    assert!(b > a, "expected {b} > {a}");
}

#[tokio::test]
async fn tick_once_propagates_sink_errors() {
    let sink = MockSink::new();
    sink.set_scripted(vec![Err("disk full".into())]);
    let loop_ = PublishLoop::new(
        FakeProvider::ok(simple_snapshot()),
        Renderer::new(),
        sink,
        loop_cfg(Duration::from_secs(60)),
    );
    let err = loop_.tick_once().await.unwrap_err();
    let PublishError::Sink(inner) = err else {
        panic!("expected Sink, got {err:?}")
    };
    assert_eq!(inner.to_string(), "disk full");
}

#[tokio::test]
async fn tick_once_surfaces_provider_errors() {
    let sink = MockSink::new();
    let loop_ = PublishLoop::new(
        FakeProvider::provider_error("nope"),
        Renderer::new(),
        sink.clone(),
        loop_cfg(Duration::from_secs(60)),
    );
    let err = loop_.tick_once().await.unwrap_err();
    assert!(
        matches!(err, PublishError::Weather(_)),
        "expected Weather, got {err:?}",
    );
    assert!(sink.record().is_empty());
}

#[tokio::test]
async fn run_recovers_after_transient_sink_error() {
    // Test error-swallowing behaviour deterministically:
    // script the sink to Err, then Ok. If run() propagates
    // instead of swallowing, the loop never reaches the
    // second publish.
    let sink = MockSink::new();
    sink.set_scripted(vec![Err("transient".into()), Ok(()), Ok(())]);
    let loop_ = PublishLoop::new(
        FakeProvider::ok(simple_snapshot()),
        Renderer::new(),
        sink.clone(),
        loop_cfg(Duration::from_millis(5)),
    );
    let handle = tokio::spawn(loop_.run());
    tokio::time::sleep(Duration::from_millis(80)).await;
    handle.abort();
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

#[tokio::test]
async fn tick_once_renders_plausible_trmnl_og_bmp() {
    // End-to-end sanity: feed a 72-hour snapshot through
    // the real model → SVG → Renderer::with_default_fonts
    // pipeline at TRMNL OG resolution. Assert the output
    // is a syntactically valid 1-bit BMP of the exact
    // expected size (62-byte header+palette +
    // 100-byte-per-row × 480 rows = 48062) and contains
    // enough black pixels that text + icons are
    // contributing — a silently-broken font pipeline
    // would leave the layout mostly white.
    //
    // The snapshot's start time is irrelevant to this
    // assertion — `tick_once` calls `Utc::now()` itself
    // for the day-bucketing. The pipeline produces a
    // full BMP in every case (tiles just render as
    // placeholders when they don't match the forecast
    // window), so the size/coverage checks are
    // wall-clock-stable.
    let start = Utc::now() + ChronoDuration::minutes(30);
    let capture = CapturingSink::default();
    let loop_ = PublishLoop::new(
        FakeProvider::ok(rich_snapshot_at(start)),
        Renderer::with_default_fonts(),
        capture.clone(),
        loop_cfg_with_render(trmnl_og_render_cfg(), Duration::from_secs(60)),
    );

    let filename = loop_.tick_once().await.unwrap();
    let (published_name, bmp) = capture.take().expect("one publish");
    assert_eq!(published_name, filename);

    // Size: 62 (header + 2-colour palette) + 100 bytes
    // per row × 480 rows = 48062 exactly.
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

/// Regenerate dashboard preview artefacts for eyeball
/// inspection during layout work. Marked `#[ignore]` so
/// it doesn't run by default — `cargo xtask preview`
/// invokes it on demand and serves the results.
///
/// Writes three files next to each other in the
/// **workspace** `target/`:
///
/// - `dashboard-sample.svg` — raw SVG the renderer
///   consumes, useful for inspecting the author-space
///   layout without any rasterisation.
/// - `dashboard-sample.png` — `resvg` raster at the
///   configured resolution, before any dithering.
///   Shows what the renderer saw; any visual issue
///   present here is an SVG or resvg bug.
/// - `dashboard-sample.bmp` — final 1-bit output sent
///   to the TRMNL. Differences from the PNG are the
///   dither contribution in isolation.
///
/// The snapshot is the same `rich_snapshot_at` the
/// end-to-end test uses, so the outputs match what the
/// production pipeline would produce at that moment.
#[tokio::test]
#[ignore = "manual tool: writes target/dashboard-sample.{svg,png,bmp}"]
async fn generate_dashboard_sample() {
    let now = Utc::now();
    let start = now + ChronoDuration::minutes(30);
    let cfg = trmnl_og_render_cfg();
    let ctx = dashboard::ModelContext {
        tz: cfg.timezone,
        location: TEST_LOCATION,
        now,
        telemetry: DeviceTelemetry::default(),
    };
    let now_local = ctx.now.with_timezone(&ctx.tz).time();
    let snapshot = rich_snapshot_at(start);
    let model = dashboard::build_model(&snapshot, ctx);
    let svg = dashboard::build_svg(&model, now_local);
    let renderer = Renderer::with_default_fonts();
    let png = renderer.render_to_png(&svg, &cfg).expect("render png");
    let bmp = renderer.render_to_bmp(&svg, &cfg).expect("render bmp");

    // CARGO_MANIFEST_DIR points at the crate root; jump
    // two levels to reach the workspace target/ so all
    // preview artefacts live in one place regardless of
    // which crate the test was invoked from.
    let target = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target");
    std::fs::create_dir_all(&target).unwrap();
    let write = |name: &str, bytes: &[u8]| {
        let out = target.join(name);
        std::fs::write(&out, bytes).unwrap();
        eprintln!("wrote {} ({} bytes)", out.display(), bytes.len());
    };
    write("dashboard-sample.svg", svg.as_bytes());
    write("dashboard-sample.png", &png);
    write("dashboard-sample.bmp", &bmp);
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
