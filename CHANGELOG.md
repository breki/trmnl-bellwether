# Changelog

All notable changes to this project will be documented
in this file.

The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Atomic weather widgets: `weather-icon`, `temp-now`,
  `condition`, `feels-like`, `day-name`, `temp-high`,
  `temp-low`. Weather-domain widgets take a `day`
  selector (`"today"` or numeric forecast offset
  `0..N`). `temp-high`/`temp-low` accept an optional
  `label` prefix (e.g. `label = "H"` → `"H 12°"`).
  Text widgets auto-size their font to the assigned
  bounds so layout controls visual weight purely via
  splits.
- Hand-rolled HTML landing page at `/` listing the
  server's endpoints (`/health`, `/api/status`,
  `/api/display`, `/api/setup`, `/api/log`,
  `/images/*`) and embedding the latest rendered
  dashboard image. Served by `bellwether-web`
  directly, no build step required.
- `GET /preview.bmp` serves the latest rendered BMP
  directly (unauthenticated, `Cache-Control:
  no-store`). Backs the landing-page preview
  `<img>`.

### Fixed

- Landing-page dashboard preview now actually
  renders. Previously the `<img>` pointed at
  `/api/display?preview=1`, which returns a JSON
  manifest (and 401s when an access token is
  configured), so the preview always fell through
  to the "no image yet" fallback.

### Changed

- Dashboard font swapped from Atkinson Hyperlegible
  (Regular) to Source Sans 3 (Semibold, weight 600).
  Source Sans 3 uses a dotted zero rather than a
  slashed one and has heavier strokes at the Semibold
  weight, which dithers cleanly to 1-bit e-ink. Public
  constant `bellwether::render::ATKINSON_HYPERLEGIBLE_TTF`
  replaced by `bellwether::render::SOURCE_SANS_3_SEMIBOLD_TTF`,
  paired with new `SOURCE_SANS_3_FAMILY: &str` and
  `SOURCE_SANS_3_WEIGHT: u16` so SVG callers don't
  have to hardcode the font-family/weight separately.
- **Breaking layout DSL:** compound widgets
  `current-conditions`, `forecast-day`, and
  `today-hi-lo` have been removed. Compose the same
  visual using atomic widgets + nested splits (see
  the rewritten default `assets/layout.toml`). Any
  custom `[dashboard]` TOML referencing the old
  names must be updated.
- **Breaking CLI:** `bellwether-web --frontend <path>`
  flag removed. The systemd unit, `deploy/README.md`,
  and PowerShell build script updated accordingly.

### Removed

- **Svelte / Vite frontend** (`frontend/`), **Playwright
  E2E harness** (`e2e/`, `playwright.config.ts`,
  `scripts/e2e.sh`), root `package.json` and
  `tsconfig.json`. The scaffold was leftover template
  material — the TRMNL is the display, and admin
  tasks don't need an SPA. If a richer admin UI is
  needed later, server-rendered HTML / HTMX is a
  better fit for this project's size.
- `xtask frontend_check` (the `svelte-check` step)
  and `[6/6] Frontend` step in `cargo xtask validate`.
  Validate now runs 5 steps.
- `tower-http` `fs` feature from `bellwether-web`
  (no more `ServeDir` / `ServeFile`).
- `/api/greeting` scaffold endpoint. `/api/status`
  still returns the server version.

### Added

- `[dashboard]` section in the main config (e.g.
  `config.toml`) now optionally carries a full widget
  layout. `canvas` sits as a sibling field next to the
  root node's own fields (`split`, `divider`,
  `children`, etc.) — no intermediate `[dashboard.root]`
  wrapper, achieved via `#[serde(flatten)]` on the
  `Layout.root` field. The `[dashboard]` section is
  validated at `Config::load` time — a layout whose
  splits fail to resolve is rejected with
  `ConfigError::InvalidDashboardLayout` instead of
  crashing at the first publish tick.
- When `[dashboard]` is absent,
  `Config::dashboard_layout()` falls back to
  `Layout::embedded_default()` (the bundled
  `assets/layout.toml`, which itself now uses the
  flattened shape).
- New `PublishError::Layout` variant surfacing render-
  time `LayoutError`s as logged-and-skipped publish
  ticks instead of a panic.
- `PublishLoopConfig { render_cfg, layout, interval }`
  params struct for `PublishLoop::new`, so the three
  non-dependency configuration values sit in one
  place rather than as four positional arguments.

### Changed

- **Breaking API:** `Layout.layout` field renamed to
  `Layout.root` and now uses `#[serde(flatten)]`, so
  the TOML no longer wraps the root node in a separate
  `[layout]` / `[root]` section.
- **Breaking API:** `PublishLoop::new(provider,
  renderer, sink, PublishLoopConfig)` replaces the old
  5-positional form.
- `Layout::embedded_default` now resolves the embedded
  layout eagerly inside `OnceLock::get_or_init`,
  surfacing any embedded-asset breakage at startup
  rather than at first render.
- `bellwether-web` threads `cfg.dashboard_layout()`
  into the publish loop, so editing `[dashboard]` in
  the main config rearranges the rendered dashboard
  without touching source.

### Added

- Configurable widget-layout system. The dashboard
  tree (splits, widgets, dividers, sizing) is now
  declared in `crates/bellwether/assets/layout.toml`
  and resolved at render time by
  `dashboard::layout::Layout::resolve`. A new
  strongly-typed `WidgetKind` enum covers every
  widget in the default dashboard (brand, header
  title, clock, battery, current conditions, wind,
  gust, humidity, forecast-day, today hi/lo,
  sunrise, sunset). Children declare their sizing
  as `size = N` (fixed pixels) or `flex = N`
  (weighted share); `flex = 0`, both size and flex,
  or neither are rejected at TOML parse time.
  `SplitNode.divider = true` reserves 2 px between
  children and emits a line there, replacing the
  previous hardcoded section/column separators.

### Changed

- `dashboard::build_svg` still renders the embedded
  default layout; for user-supplied layouts use the
  new `build_svg_with_layout(&Layout, ...) ->
  Result<String, LayoutError>`. Widget Y
  coordinates are now bounds-relative — resizing a
  band in `layout.toml` moves its widgets with it.
- `<text>` content is XML-escaped before
  interpolation, so arbitrary `HeaderTitle.text`
  values (including `&`, `<`, `>`) produce
  well-formed SVG.

### Fixed

- `ImageStore` now evicts the oldest image once the
  retained count exceeds `MAX_RETAINED_IMAGES` (= 4).
  Previously the store was unbounded: at the default
  5-minute refresh the process accumulated ~13.5 MB of
  BMPs per day and would OOM-kill under `MemoryMax=512M`
  after about 37 days of uptime. The TRMNL BYOS protocol
  only needs the image most recently advertised via
  `/api/display`; the small tail is preserved so devices
  that fetch slightly after the next render tick don't
  see a 404.

### Added

- `GET /api/setup` TRMNL BYOS endpoint for first-boot
  device registration. Returns `api_key`,
  `friendly_id` (6-char uppercase hex from the
  device MAC), and the current image URL. Exempt
  from the `Access-Token` middleware since a fresh
  device has no token yet. Returns 503 when no
  image has been rendered yet.
- `cargo xtask deploy-setup` — one-time RPi
  provisioning (creates the `bellwether` system
  user, installs `config.toml`, installs + enables
  the systemd unit).
- `cargo xtask deploy` — repeatable deploy (source
  tar → scp → remote cargo build with persisted
  `target` cache → atomic binary + frontend swap →
  service restart with `reset-failed` guard).
- `deploy/bellwether-web.service` hardened systemd
  unit (system user, `ProtectSystem=strict`, empty
  `CapabilityBoundingSet`, `SystemCallFilter=@system-service`,
  `MemoryMax=512M`).
- `.deploy.sample` config template; `deploy/README.md`
  deployment guide.
- `FriendlyId` newtype wrapping the TRMNL firmware's
  6-char hex device identifier.
- `DEFAULT_UNCONFIGURED_API_KEY` constant documenting
  the placeholder `api_key` returned when no access
  token is configured.

### Changed

- Split `crates/bellwether-web/src/api/trmnl/mod.rs`
  into `mod.rs` (state + store + router) and
  `handlers.rs` (response types, handlers,
  middleware) to keep both files under the 500-line
  module threshold.
- `xtask` deploy modules use `anyhow::Result` so
  ssh/scp `RemoteError` source chains are preserved
  through to the CLI.

- `crate::weather` — provider-neutral forecast types
  (`WeatherSnapshot`, `WeatherError`,
  `WeatherProvider` trait) plus
  `clients::http_util` (shared `build_http_client`,
  `read_capped_body`, `truncate_with_ellipsis`).
- Open-Meteo provider (`clients::open_meteo`): free,
  keyless, GET-based forecast client. Default model
  `icon_eu` (DWD's European regional model,
  Slovenia-optimised). Units already match
  `WeatherSnapshot` so the adapter is a near-trivial
  passthrough.
- `Compass8::from_degrees(deg: f64) -> Self` —
  degree-to-octant bucketing replaces the old u/v
  `wind_to_compass`. Same half-open sector
  convention.

### Changed

- **Breaking:** config TOML restructured. The old
  `[windy]` table is gone. New shape:
  ```toml
  [weather]
  provider = "open_meteo"
  lat = 46.05
  lon = 14.51

  [weather.open_meteo]
  model = "icon_eu"
  ```
- `dashboard::build_model` now takes
  `&WeatherSnapshot` instead of `&Forecast`. Unit
  conversion (Kelvin → Celsius, u/v → km/h +
  compass degrees) moved out of `dashboard::model`
  into provider adapters — the dashboard only sees
  display units now.
- `PublishLoop` holds `Arc<dyn WeatherProvider>` +
  `GeoPoint` instead of `WindyClient +
  FetchRequest`. `PublishError::Weather(WeatherError)`
  replaces `::Windy`.
- Default env filter in `bellwether-web` now
  includes `bellwether=info` so publish-loop log
  lines (`published image`, `publish tick failed`)
  are visible without setting `RUST_LOG`.
- Duplication threshold raised from 6.0% → 7.0% to
  absorb the residual parallel-provider structure.
  See `xtask/src/dupes.rs`.

### Removed

- **Breaking:** Windy Point Forecast provider.
  Deleted `clients::windy`, `WindyConfig`,
  `WindyProviderConfig`, `WindyParameter`,
  `ConfigError::MissingRequiredWindyParameters` /
  `ReadSecret` / `EmptySecret`,
  `REQUIRED_WINDY_PARAMETERS`, the `live-tests`
  feature, and the `scripts/windy-test.ps1` helper.
  Migration rationale and settled decisions:
  `docs/developer/weather-provider-migration.md`.

## [0.11.0] - 2026-04-18

### Added

- Dense 5-band dashboard SVG layout: header (brand +
  title + clock + battery indicator), current
  conditions (icon + big temp + condition word +
  feels-like), 3-cell meteorology strip (wind + gust
  + humidity), 3-tile forecast row (weekday + icon +
  H/L), footer (today's H/L + sunrise + sunset).
- `dashboard::DashboardModel.day_weekdays:
  [Weekday; 3]` — weekday labels for each forecast
  tile, populated from `ctx.now` regardless of
  whether the tile's data row is `None`. Keeps the
  layout header visible when a tile's data is
  missing so the user can see which day is absent.

### Changed

- **Breaking:** `dashboard::build_svg` signature is
  now `(model, now_local: NaiveTime) -> String`.
  Was `(model) -> String`. The clock input stays
  out of the model so a rendered model doesn't go
  stale the instant it's held.
- Calm wind conditions (`round(wind_kmh) == 0`) now
  render as `"Wind calm"` instead of `"Wind N 0
  km/h"` (the previous output, where `Compass8::N`
  was a calm-sentinel leaking into the label).
- Missing current conditions render a neutral "No
  current reading" label centred in the band
  instead of a lone 120-px em-dash.
- Battery fill width is rounded rather than
  truncated (`pct=99` now renders as 99 % of the
  inner width, not 98 %); zero-width fill rects are
  elided.

## [0.10.0] - 2026-04-17

### Added

- `bellwether::dashboard::astro` — sunrise/sunset
  calculation module (NOAA solar position algorithm,
  no new crate dependency). `GeoPoint { lat_deg,
  lon_deg }` packs coordinates so accidental swaps
  become compile errors.
- `bellwether::dashboard::feels_like` — apparent
  temperature module combining the NWS heat-index
  (Rothfusz) and wind-chill (2001) formulas with
  NaN-guarded fallback to raw temperature.
- `bellwether::telemetry` — new neutral module
  hosting `DeviceTelemetry` (last-reported battery
  voltage from `/api/log`) and
  `battery_voltage_to_pct` (linear `LiPo` voltage →
  percent). `publish` re-exports both for existing
  import paths.
- `publish::ImageSink::latest_telemetry` default-
  method returning `DeviceTelemetry::default()`;
  `TrmnlState` overrides to return its cached value.
- `TrmnlState::update_telemetry` + `telemetry()`
  methods (web crate). Behind an
  `Arc<RwLock<DeviceTelemetry>>`; the `/api/log`
  handler calls `update_telemetry` on every post
  and merges fresh fields into the cache, keeping
  prior values for fields the new post omits.
- `dashboard::ModelContext { tz, location, now,
  telemetry }` — a single `Copy` struct passed to
  `build_model`, replacing the previous
  `(tz, DateTime<Utc>)` positional pair.
- `dashboard::TodaySummary` with today's high / low
  / sunrise / sunset.
- `CurrentConditions.feels_like_c`,
  `gust_kmh: Option<f64>`, `humidity_pct:
  Option<f64>`.
- `DaySummary.low_c: Option<i32>`.
- `DashboardModel.battery_pct: Option<u8>` derived
  from `ctx.telemetry`.

### Changed

- **Breaking:** `dashboard::build_model` signature is
  now `(forecast: &Forecast, ctx: ModelContext) ->
  DashboardModel`. Was `(forecast, tz,
  DateTime<Utc>)`.
- **Breaking:** `REQUIRED_WINDY_PARAMETERS` now
  includes `Rh` and `WindGust`. Pre-0.10 configs
  that only listed `temp` + `wind` + `clouds` +
  `precip` fail at `Config::load` with
  `MissingRequiredWindyParameters`.
- Humidity (`rh-surface`) is clamped to `[0, 100]`
  at the model boundary to protect the feels-like
  calculation from out-of-range Windy glitches.
- `nearest_sample_index` uses `saturating_abs`
  rather than `abs` so a malformed timestamp can't
  panic the publish loop.
- `dashboard/model.rs` split into `model/mod.rs` +
  `model/tests.rs` to stay under the 500-line
  threshold.

### Note

This commit is groundwork — no user-visible output
change. The SVG builder still emits the v0.9
"current + 3-day forecast" layout; the new model
fields populate but aren't rendered. The dense
header + today + meteorology-strip layout lands in
v0.11.0.

## [0.9.0] - 2026-04-17

### Changed

- **Breaking:** bundled dashboard font swapped from
  `m6x11plus` (pixel font) to `Atkinson Hyperlegible`
  Regular (vector sans-serif, SIL OFL). The new font
  is designed by the Braille Institute for maximum
  character-to-character legibility and dithers
  cleanly to 1-bit e-ink at display sizes — much
  crisper than a pixel font scaled 10× for the big
  current-conditions temperature.
- **Breaking:** `bellwether::render::M6X11_TTF` renamed
  to `bellwether::render::ATKINSON_HYPERLEGIBLE_TTF`.
  The `&[u8]` typing and `Renderer::with_default_fonts()`
  constructor are unchanged, so downstream callers that
  only use `with_default_fonts()` need no changes —
  only consumers of the const need to update the name.
- Dashboard SVG builder no longer constrains font
  sizes to integer multiples of 18 (that was specific
  to m6x11plus's pixel grid). Size literals are now
  named constants (`CURRENT_TEMP_PX`,
  `CONDITION_LABEL_PX`, `WIND_LABEL_PX`,
  `DAY_LABEL_PX`, `DAY_HIGH_PX`) at the top of
  `dashboard::svg`.

## [0.8.0] - 2026-04-17

### Added

- `bellwether::dashboard` module — replaces the
  placeholder temperature bar with a real "current +
  3-day forecast" layout. Public API:
  `DashboardModel`, `CurrentConditions`, `DaySummary`,
  `Condition`, `Compass8`, `classify_weather`,
  `wind_to_compass`, `build_model`, `build_svg`, plus
  per-condition SVG icon fragments under
  `dashboard::icons`.
- `dashboard::build_model` now takes an explicit
  `now: DateTime<Utc>` and picks the forecast sample
  closest to `now` for the current-conditions panel
  (instead of blindly using index 0, which could be
  hours stale depending on the Windy model run).
- Config validation: `[windy] parameters` must
  include `temp`, `wind`, `clouds`, and `precip`
  when non-empty. A pre-0.8 config missing any of
  these now fails at `Config::load` rather than
  silently producing "Cloudy" tiles. Empty
  `parameters` is still accepted for webhook-only
  deployments. New error variant
  `ConfigError::MissingRequiredWindyParameters`.

### Changed

- `bellwether-web` now builds its `Renderer` via
  `Renderer::with_default_fonts()` at both the
  placeholder-seeding and publish-loop call sites, so
  dashboard text actually renders on startup and on
  every tick.
- `config.example.toml` `parameters` list updated to
  include `"clouds"` and the comment rewritten.
- `DaySummary::high_c` is `Option<i32>` (was `i32`).
  A day whose temperature series was entirely null
  but passed the sample-count gate now renders with
  an em-dash placeholder instead of a misleading
  "0°".
- `DaySummary::label: String` replaced with
  `DaySummary::weekday: chrono::Weekday`; the SVG
  builder formats the label at render time via its
  private `weekday_label` table. The dashboard's
  "labels are always English" invariant now lives in
  exactly one place.

### Removed

- The placeholder temperature-bar dashboard SVG
  (`publish::build_dashboard_svg` free function) —
  superseded by `dashboard::build_svg`.

## [0.7.0] - 2026-04-17

### Added

- `bellwether::render::M6X11_TTF: &[u8]` — the
  bundled m6x11plus pixel font bytes (Daniel Linssen,
  free-with-attribution). Compile-time-embedded via
  `include_bytes!`; attribution in
  `crates/bellwether/src/render/fonts/README.md`.
- `Renderer::with_default_fonts() -> Self` —
  production constructor that pre-loads `M6X11_TTF`.
  Use this in servers / binaries that render
  dashboard text; `Renderer::new()` stays available
  for test code and callers that prefer to load
  fonts themselves.
- Font-pipeline tests: bundled TTF parses as
  TrueType; glyph coverage spans
  `0-9`/`A-Z`/`a-z`/space/`°`; end-to-end render of
  `"0°C"` through the `with_default_fonts()`
  pipeline produces non-trivial black pixel
  coverage.
- `config.example.toml` — commitable template. Copy
  to `config.toml` (gitignored), fill in Windy key
  in `windy_key.txt` (also gitignored).
- `README.md` grew a "Running the server" section
  covering the config → key → run flow and the
  `--dev` placeholder-only path.
- `docs/developer/HANDOFF.md` rewritten for the
  post-scaffold state.

### Changed

- Default backend port: **3000 → 3100**. Affects CLI
  default on `bellwether-web`, `--dev` mode's
  `public_image_base`, `.ports.sample`, `build.ps1`,
  `vite.config.js`, and `playwright.config.ts`.
  Operators running on a custom port via `.ports`
  are unaffected.

## [0.6.0] - 2026-04-17

### Added

- `bellwether::publish::PublishLoop<S: ImageSink>` —
  fetch (Windy) → render (dashboard SVG → 1-bit BMP)
  → publish (sink) loop. First tick fires immediately;
  subsequent ticks on a `tokio::time::interval` with
  `MissedTickBehavior::Delay`. Per-tick errors logged
  at `warn!` and swallowed so transient failures don't
  kill the process.
- `publish::ImageSink` trait with opaque
  `SinkError = Box<dyn Error + Send + Sync>` return
  type. `TrmnlState` implements it in the web crate.
- `publish::supervise(name, future)` — tokio spawn
  wrapper that logs at `error!` if the task ever ends
  unexpectedly. No auto-restart (avoids Windy quota
  exhaustion via crash loop).
- `Forecast::from_raw_json` — build a `Forecast` from
  a raw Windy JSON string without an HTTP round-trip.
  Used by the publish tests so fixtures stay in sync
  with the wire parser.
- `ConfigError::InvalidRefreshRate` — rejects
  `default_refresh_rate_s` outside `1..=86400` at
  config load (zero would have panicked the tokio
  interval).
- `RefreshInterval::as_duration()` to convert to
  `tokio::time::Duration`.

### Changed

- `clients::windy::Client::fetch` now takes
  `&FetchRequest` instead of owned — removes the
  per-tick clone in the publish loop.
- `FetchRequest` has a manual `Debug` impl that
  redacts `api_key` as `"<redacted>"`.
- `bellwether-web` spawns the publish loop when
  `--config` is given (under `supervise`); `--dev`
  skips the loop.

### Security

- `FetchRequest` no longer leaks the Windy API key
  via derived `Debug`.
- Publish loop filenames use a monotonic counter
  (`dash-{counter:08}.bmp`) instead of wall-clock
  timestamps, so RTC-less Pis at boot don't produce
  colliding or negative filenames.

## [0.5.0] - 2026-04-17

### Added

- TRMNL BYOS endpoints on `bellwether-web`:
  `GET /api/display` returns the image manifest,
  `POST /api/log` accepts device telemetry (16 KiB
  body limit), `GET /images/{filename}` serves
  rendered BMPs.
- `--config <FILE>` required on `bellwether-web`
  (pass `--dev` to run with localhost defaults).
- `Renderer::placeholder_bmp` in the `bellwether`
  library — renders a built-in font-free geometric
  SVG for seeding servers before the first real
  render.
- `TrmnlState` shared state with composite-locked
  `ImageStore`, validated filenames
  (`[A-Za-z0-9._-]{1,128}`), validated base URL
  (`http`/`https` scheme required, no query /
  fragment), and optional `Access-Token` middleware
  driven by the `BELLWETHER_ACCESS_TOKEN` env var.
- `RefreshInterval` newtype so the wire-format unit
  (seconds) is visible at every construction site.

### Security

- TRMNL endpoints reject >16 KiB POST bodies on
  `/api/log` before they reach `tracing` (prevents
  log amplification).
- Known telemetry fields (battery voltage, RSSI,
  firmware version) log at INFO; full payload only
  at DEBUG.
- Filenames passed to `TrmnlState::put_image` are
  validated at insert time, so URL injection via the
  `image_url` field is impossible.
- Optional access-token middleware gates all TRMNL
  BYOS endpoints when `BELLWETHER_ACCESS_TOKEN` is
  set.

## [0.4.0] - 2026-04-17

### Added

- `bellwether::render::Renderer` — server-side SVG →
  1-bit monochrome BMP pipeline for TRMNL OG.
  `resvg`/`tiny-skia` rasterize, grayscale conversion
  composites transparent regions over white with
  Rec. 601 luma coefficients, Floyd–Steinberg dithers
  to 1-bit, and a hand-rolled encoder emits the
  canonical (`"standart"`) palette layout the TRMNL
  firmware accepts.
- `Renderer::load_font_data(Vec<u8>)` — load TTF/OTF
  fonts from baked-in bytes.
- New `RenderError` variants: `ParseSvg`,
  `RasterFailed`, `InvalidScale`,
  `UnsupportedBitDepth`.
- `ConfigError::InvalidRenderDimensions` — render
  dimensions outside `1..=4096` rejected at
  `Config::load` / `Config::from_toml_str`.

### Security

- Render pipeline rejects SVGs that would require a
  scale factor above 8192 or non-finite, foreclosing
  a DoS vector via crafted tiny viewports.
- Render dimensions bounded at 4096 per axis.
- Regression test verifies `<image href="file://...">`
  remains silently ignored.
- `Renderer::load_font_data` documents the font-trust
  boundary; callers warned against unsandboxed
  user-uploaded font blobs.

## [0.3.0] - 2026-04-17

### Added

- `bellwether::clients::windy` — HTTP client for the
  Windy Point Forecast v2 API. `Client`,
  `FetchRequest` (owned fields for schedulers),
  `Forecast`, and `WindyError`. Typed lookup via
  `Forecast::values(WindyParameter)`. Convenience
  `Client::fetch_with_config(&WindyConfig)`.
- `WindyParameter::wire_name()` — stable mapping from
  variant to Windy wire string, test-verified against
  the `#[serde(rename)]` attributes.
- Per-client body-size caps
  (`with_max_response_bytes` / `with_max_error_body_bytes`)
  with sensible defaults (4 MiB / 4 KiB).
- `live-tests` feature flag gating the real-network
  `live_windy` smoke test.

### Changed

- `WindyParameter` now derives `Serialize` and uses
  per-variant `#[serde(rename)]` matching Windy's wire
  format (camelCase for `windGust`, lowercase
  otherwise). Previously `rename_all = "lowercase"`
  silently mis-spelled `windGust` as `windgust`.

### Security

- Windy client rejects cross-origin redirects
  (`reqwest::redirect::Policy::none()`), preventing
  API-key leakage if `api.windy.com` is DNS-hijacked
  or the CDN is compromised.
- Error responses are scanned and the API key is
  redacted before the body surfaces in
  `WindyError::Api`.
- Response bodies are size-capped to prevent OOM from
  a misbehaving proxy or server.

## [0.2.0] - 2026-04-17

### Added

- `bellwether::config::Config` module with TOML loading,
  parsing, and validation. Sections: `[windy]`,
  `[trmnl]` (discriminated by `mode = "byos" | "webhook"`),
  `[render]`.
- `--config <FILE>` CLI flag on the `bellwether` binary.
  Prints a one-line summary via `Display for Config`.
- Windy API key loaded eagerly from `api_key_file` at
  startup (fails fast on missing / empty / unreadable
  secret files). API key redacted in `Debug` output.
- Strongly-typed `WindyParameter`, `BitDepth`
  (`1` or `4`), and `chrono_tz::Tz` for timezone —
  typos and invalid values rejected at config load.
- Latitude / longitude range validation (`[-90, 90]`,
  `[-180, 180]`, finite).
- Design spike: `docs/developer/spike.md` locks the OG
  7.5" / BYOS / 1-bit BMP / `resvg` + `image` stack.

## [0.1.0] - 2026-04-16

### Added

- Initial scaffold generated from the
  [rustbase](https://github.com/breki/rustbase) template
  at commit `076cf44` (v0.4.0)
- Workspace renamed to `bellwether` /
  `bellwether-web`
- Project overview in `CLAUDE.md` and `README.md`
  describing the TRMNL aggregator / renderer goal
