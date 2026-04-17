# Changelog

All notable changes to this project will be documented
in this file.

The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
