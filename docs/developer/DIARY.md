# Development Diary

This diary tracks functional changes to the codebase in
reverse chronological order.

---

### 2026-04-17

- Real dashboard layout (v0.8.0)

    `bellwether::dashboard` replaces the placeholder
    temperature bar with a current-conditions panel
    (big temp, condition word, wind label) and three
    day tiles (weekday, icon, high) along the bottom.
    Module structure:

    - `classify` — `Condition` (Sunny/PartlyCloudy/
      Cloudy/Rain) and `Compass8` enums; pure
      `classify_weather(cloud_pct, precip_mmh)` and
      `wind_to_compass(u, v)` functions with
      meteorological "wind from" convention.
    - `model` — `DashboardModel`, `CurrentConditions`,
      `DaySummary` structs; `build_model(forecast, tz,
      now)` that handles Kelvin→Celsius, wind u/v →
      km/h + compass, local-date bucketing, partial-day
      threshold (fewer than 6 samples drops the tile),
      and null-temperature handling (`high_c:
      Option<i32>` so the SVG can show an em-dash
      rather than a misleading "0°").
    - `icons` — four hand-drawn 48 × 48 SVG icon
      fragments.
    - `svg` — `build_svg(model)` that emits an 800 × 480
      SVG at integer-multiple-of-18 font sizes (the
      size family m6x11plus is designed for).

    Wiring: `publish::tick_once` passes `Utc::now()`
    through so "current" is the sample closest to
    wall-clock, not `ts[0]` (which can be stale by
    hours depending on Windy's model-run cadence).
    `bellwether-web/main.rs` switched to
    `Renderer::with_default_fonts()` so text
    actually renders.

    Config validation: `parameters` (when non-empty)
    must include temp, wind, clouds, precip — the
    four the v1 dashboard consumes. A pre-0.8 config
    missing `clouds` now fails at load rather than
    silently rendering "Cloudy" forever.

    Tests: 43 new unit tests across classify / model /
    icons / svg, plus an end-to-end test that renders
    the full pipeline at TRMNL OG resolution and
    asserts a 48,062-byte BMP with meaningful black
    coverage. An `#[ignore]`'d
    `generate_dashboard_sample_bmp` writes
    `target/dashboard-sample.bmp` for manual eyeball.

- Bundled m6x11plus pixel font (v0.7.0)

    Added `bellwether::render::M6X11_TTF: &[u8]` as a
    compile-time-embedded font blob and
    `Renderer::with_default_fonts()` as the production
    constructor that pre-loads it. Font is Daniel
    Linssen's m6x11plus — a proportional 6×11 pixel
    font with extended Latin coverage (attribution in
    `crates/bellwether/src/render/fonts/README.md`).
    Covers `U+00B0 °`, verified by an
    iteration-over-full-ranges test rather than
    endpoint spot-checks.

    This is step 1 of PR 3d. The dashboard layout
    itself lands in follow-up commits; this commit
    only bundles the font and wires the renderer
    constructor, leaving the placeholder SVG in place
    for the moment. Isolating the font step means the
    glyph-coverage decision is verifiable on its own
    and future steps don't have to re-argue the font
    choice.

    `ttf-parser` added as a dev-dep (exact-pinned to
    0.25.1, matching fontdb's transitive pull) so the
    glyph-coverage test can check `Face::glyph_index`
    directly rather than black-box rasterizing.

- Fetch → render → publish loop (v0.6.0)

    New `bellwether::publish` module ties the Windy
    client, renderer, and BYOS image store into a
    repeating `tokio::time::interval` task. First tick
    fires immediately; subsequent ticks on the
    configured cadence (shared with the device's
    refresh rate). Per-tick errors log at `warn!` and
    are swallowed so transient Windy / DNS / render
    failures don't kill the loop — the server keeps
    serving the last-good image.

    Dashboard SVG for PR 3c is a placeholder (a bar
    whose width tracks current temperature on a
    0–40 °C scale, with an explicit diagonal X overlay
    when temperature is missing so "no data" is
    distinguishable from a real 0 °C reading). Real
    layout + fonts defer to a later PR.

    Filenames are `dash-{counter:08}.bmp` from an
    `AtomicU64`, avoiding wall-clock collisions and
    negative timestamps on RTC-less Pis. `FetchRequest`
    picked up a manual `Debug` redacting the api_key
    so the key can't leak via a future
    `tracing::debug!(?req, …)`. `Client::fetch` now
    takes `&FetchRequest` so the publish loop doesn't
    clone per tick. `Config::validate` rejects
    `default_refresh_rate_s` outside `1..=86400`
    (zero would have panicked `tokio::time::interval`).
    `publish::supervise` wraps `tokio::spawn` with a
    log-on-exit tripwire — clean return or panic both
    land in the error log rather than vanishing; no
    auto-restart (avoids crash-loop Windy quota burn).
    16 review findings (8 red-team + 8 artisan), 15
    addressed in-PR.

- TRMNL BYOS endpoints on `bellwether-web` (v0.5.0)

    New `api::trmnl` module exposes `GET /api/display`
    (JSON manifest matching TRMNL OG firmware fields),
    `POST /api/log` (telemetry, 16 KiB body cap,
    known fields logged structurally at INFO / extras
    at DEBUG), and `GET /images/{filename}` (zero-copy
    `Bytes` response). `ImageStore` uses a single
    composite `RwLock` so readers never see a filename
    whose bytes aren't yet inserted. Filenames are
    validated at insert time
    (`[A-Za-z0-9._-]{1,128}`) so nothing
    user-controllable can flow into the advertised
    `image_url`. `public_image_base` is validated for
    scheme + no-query at construction. Optional
    `Access-Token` middleware reads
    `BELLWETHER_ACCESS_TOKEN`; absent token emits a
    `WARN` at startup for LAN-only deployments.
    `bellwether-web --config` is now required unless
    `--dev` is passed. `Renderer::placeholder_bmp`
    moved to the library (`crates/bellwether/src/render/
    placeholder.svg` via `include_str!`) so the
    render-loop work in PR 3c can reuse the helper.
    29 review findings from red-team + artisan; 24
    addressed in-PR, 5 deferred to TODO.md (docs only).

- Render pipeline: SVG → 1-bit BMP (v0.4.0)

    `render::Renderer` parses SVG via `resvg`/`usvg`
    (text feature only; no system fonts, no
    raster-image embeds), rasterizes to `tiny-skia`
    RGBA, converts to grayscale via fixed-point
    Rec. 601 (transparent regions composited over
    white), Floyd–Steinberg dithers to 1-bit, and
    emits a monochrome BMP with the TRMNL OG
    firmware's canonical palette (`palette[0] =
    black, palette[1] = white; bit 1 = white`).
    Verified against `usetrmnl/firmware`
    `lib/trmnl/src/bmp.cpp` — matches ImageMagick /
    Pillow defaults and the firmware's `"standart"`
    path. Module split into `bmp.rs`, `dither.rs`,
    `mod.rs`, `tests.rs` (directory layout mirrors
    `config/` and `clients/windy/`).

    Render pipeline rejects pathological inputs: SVG
    viewports that would require scales above 8192 or
    non-finite, render dimensions outside 1..=4096 at
    `Config::load`/`from_toml_str`. Regression test
    locks in that `<image href="file://...">` is
    silently ignored. 12 red-team + 15 artisan
    findings from review; all applicable ones
    addressed in-PR. RT-024 (palette inversion
    concern) specifically verified against firmware
    source and left as-is. Open in the review logs:
    nothing; Cluster D items documented inline and in
    TODO.md.

- Windy Point Forecast client (v0.3.0)

    `clients::windy::{Client, FetchRequest, Forecast,
    WindyError}` — thin transport over reqwest that
    POSTs lat/lon/model/parameters/key to Windy's
    Point Forecast v2 and returns a parsed
    `Forecast` with typed
    `values(WindyParameter)` lookup. `null` values
    preserved as `Option<f64>`; `ts` + series length
    mismatch rejected at parse time; empty `ts`
    returns `EmptyForecast`. Forward-compat
    non-numeric metadata fields in responses are
    silently ignored rather than breaking parsing.
    Security posture: `Policy::none()` on redirects
    (prevents cross-origin key leak on DNS hijack);
    API key redacted from error bodies; per-Client
    body-size caps (4 MiB success, 4 KiB error).
    Added `connect_timeout(5s)` + `gzip` feature on
    reqwest for RPi network realities. `WindyParameter`
    picked up `Serialize` + per-variant renames so
    `windGust` round-trips correctly (it was silently
    emitted as `windgust` before). 30 review findings
    from red-team + artisan — all addressed in PR;
    see `redteam-resolved.md` / `artisan-resolved.md`.

- Design spike + config skeleton (v0.2.0)

    Closed the five open questions flagged in
    `HANDOFF.md`: TRMNL OG 7.5" @ 800×480 1-bit; **BYOS**
    (device polls our server) as the v1 integration
    target; Webhook Image plugin kept as the fallback;
    render stack = `resvg` (SVG → RGBA) + `image`
    (grayscale + Floyd–Steinberg dither + 1-bit BMP).
    Design decisions captured in
    `docs/developer/spike.md`. Home Assistant
    integration moved to the backlog at the user's
    request — PR 1 covers Windy + TRMNL + render only.

    Config module lives under `crates/bellwether/src/config/`
    (split into `mod`, `windy`, `trmnl`, `render`).
    `Config::load(impl AsRef<Path>)` parses the TOML,
    resolves `api_key_file` against the config file's
    directory, validates lat/lon range, reads the Windy
    secret eagerly, and caches it on `WindyConfig`
    (redacted in `Debug`). `Config::from_toml_str` is
    a disk-free entry point for tests and preview
    flows. `TrmnlConfig` is an internally-tagged enum
    so `mode = "byos"` cannot coexist with missing
    BYOS fields — illegal states are unrepresentable.
    Strong types for `WindyParameter`, `BitDepth`, and
    `timezone: chrono_tz::Tz`. Red-team + artisan
    reviews ran in parallel; all 23 findings (bar one
    noted exception) landed in this PR — see
    `redteam-resolved.md` / `artisan-resolved.md`.

### 2026-04-16

- Scaffold from rustbase template (v0.1.0)

    Generated from [rustbase](https://github.com/breki/rustbase)
    at commit `076cf44` (template v0.4.0). Renamed crates
    from `rustbase` / `rustbase-web` to `bellwether` /
    `bellwether-web` and updated all references (workspace
    config, binary names, release workflow, dev scripts,
    Claude Code skills, CI). Reset project-tracking files
    (`CHANGELOG`, diary, red-team / artisan logs,
    template-feedback) to a fresh v0.1.0 starting point.
    `.template-sync.toml` points at the 076cf44 baseline
    so future `/template-sync` runs can pull upstream
    improvements.
