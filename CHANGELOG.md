# Changelog

All notable changes to this project will be documented
in this file.

The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `config.example.toml` — commitable template. Copy
  to `config.toml` (gitignored), fill in Windy key in
  `windy_key.txt` (also gitignored).
- `README.md` grew a "Running the server" section
  covering the config → key → run flow and the
  `--dev` placeholder-only path.
- `docs/developer/HANDOFF.md` rewritten for the
  post-scaffold (v0.6.0) state.

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
