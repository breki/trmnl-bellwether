# Red Team Findings -- Resolved

Archive of fixed red team findings, newest first.
See [redteam-log.md](redteam-log.md) for open findings.

---

## 2026-04-20 (feat — v0.19.0 xtask preview + pre-dither PNG render)

### RT-A — Preview server exposed the entire workspace `target/` directory
**Category:** Security / information disclosure
**Description:** The first cut of `xtask preview` served `workspace_target_dir()` as an unrestricted document root. Any file reachable under `target/` — debug binaries with potentially-baked-in secrets via `include_str!`, coverage reports with code snippets, build-script outputs — was fetchable over HTTP while the preview ran. Binding to loopback did not help against malicious local processes (VS Code extensions, devcontainers, npm postinstall scripts on the same host) that could simply `fetch('http://127.0.0.1:8123/debug/bellwether-web')`.
**Fix:** Replaced the directory root with a 4-entry filename allowlist (`preview-index.html`, `dashboard-sample.{svg,png,bmp}`) expressed as a `match` in `allowed_mime`. Non-allowed URLs short-circuit to 404 before any filesystem touch, eliminating both the exposure and the previous canonicalize+`starts_with` path-traversal guard as redundant. `xtask/src/preview.rs`.

### RT-B — `std::fs::read` loaded entire file into xtask memory
**Category:** Correctness / DoS (subsumed by RT-A)
**Description:** With `target/` as document root, a single GET for a multi-hundred-MB debug binary would pull the whole file into xtask RAM before responding.
**Fix:** Eliminated by the RT-A allowlist — only three small artefact files (SVG ~13 KB, PNG ~100 KB, BMP ~48 KB) plus the inline HTML template are reachable. `xtask/src/preview.rs`.

### RT-C — `--open` fired browser before server was ready
**Category:** Correctness / spec violation
**Description:** `open_browser(&url)` ran before `Server::http(...)` bound the listener, racing the browser's GET against socket readiness and producing intermittent `ECONNREFUSED` contrary to the documented "once the server is ready" contract on `XCommand::Preview.open`.
**Fix:** Inlined `serve()` into `preview()`, bound `Server::http` first, then printed the URL, then invoked `open_browser`, then entered the request loop. `xtask/src/preview.rs`.

### RT-D — `cmd /C start "" <url>` latent command-injection trap
**Category:** Security (latent)
**Description:** `url` is format-constructed from a `u16` port today, so cmd.exe metacharacters can't sneak in — but `Command::args`'s Rust-side escaping targets `CreateProcessW`, which `cmd /C start` then re-parses. A future refactor threading user-controlled text (filename, config value) into `url` would silently become a command-injection sink without the cmd.exe quirk being obvious.
**Fix:** Added an explicit `SAFETY-BY-CONTRACT` comment on the Windows arm of `open_browser` pinning the numeric-port-only invariant and naming the escape alternatives (`ShellExecuteW` via the `windows` crate, or `rundll32 url.dll,FileProtocolHandler`). `xtask/src/preview.rs`.

### RT-E — `127.0.0.1` bind advertised as `localhost` URL
**Category:** Correctness / UX
**Description:** Server bound IPv4 loopback only; printed URL said `localhost`. On hosts that resolve `localhost` → `::1` first (some Windows setups, Linux with `/etc/hosts` ordering), browsers hit the IPv6 loopback, time out, then fall back to IPv4 with a delay.
**Fix:** Changed the printed URL to `http://127.0.0.1:{port}/...` to match the bind address exactly. `xtask/src/preview.rs`.

### RT-F — `cargo test` substring filter matched too broadly
**Category:** Correctness / robustness
**Description:** `cargo test generate_dashboard_sample` uses substring matching by default. A future `#[ignore]`d test named e.g. `generate_dashboard_sample_v2` would run alongside, silently doubling the work done per preview invocation.
**Fix:** Passed `--exact` with the fully-qualified `publish::tests::generate_dashboard_sample` path. `xtask/src/preview.rs`.

## 2026-04-19 (feat — v0.17.0 atomic widgets)

### RT-A — `fit_font_px` could silently render unreadable glyphs
**Category:** Correctness / UX
**Description:** A long `label` on `temp-high`/`temp-low` in a narrow cell collapsed the width-candidate font size to 1-2 px (only clamped to `.max(1)`), producing effectively-invisible text.
**Fix:** Added `MIN_LEGIBLE_PX = 8` floor applied after choosing min(from_h, from_w) in `fit_font_px`. `crates/bellwether/src/dashboard/svg/mod.rs`.

### RT-B — Hidden data-source split for `DaySelector::Today`
**Category:** Correctness / documentation
**Description:** `Condition`/`WeatherIcon` read from `model.current` while `TempHigh`/`TempLow` read from `model.today`. The two `Option`s are independent so a `day = "today"` layout can show a live icon next to em-dash hi/lo (or vice versa).
**Fix:** Added a type-level doc block on `DaySelector::Today` explaining the split. Kept the split intentionally — "icon right now" and "today's high" are genuinely different queries — but made it discoverable. `crates/bellwether/src/dashboard/layout/mod.rs`.

### RT-C — `weather-icon` emitted empty string for missing data
**Category:** Correctness / UX consistency
**Description:** Out-of-range `day = N` or missing `current` rendered `String::new()` while sibling text widgets showed "—", breaking the module-top "every optional field renders an em-dash placeholder" convention.
**Fix:** `render_weather_icon` now emits a centred em-dash when `condition` is `None`. `crates/bellwether/src/dashboard/svg/mod.rs`.

### RT-D — `DaySelector` string match was case-sensitive
**Category:** Correctness / UX
**Description:** Only exact `"today"` parsed; `"Today"` / `"TODAY"` produced confusing errors despite being the most natural capitalisation in prose.
**Fix:** `s.eq_ignore_ascii_case("today")` in `DaySelector::deserialize`. `crates/bellwether/src/dashboard/layout/mod.rs`.

### RT-E — Legacy compound-widget TOMLs failed with opaque serde error
**Category:** Migration UX
**Description:** After the compound variants (`current-conditions`, `forecast-day`, `today-hi-lo`) were deleted, existing user configs hit serde's generic "data did not match any variant of untagged enum Node" — no pointer to CHANGELOG or migration path. User ran straight into this on first use.
**Fix:** Added `ConfigError::LegacyCompoundWidget` with a dedicated migration message; `parse_and_validate` runs a substring scan of the raw TOML on parse failure and surfaces the pointed error when any legacy name appears. `crates/bellwether/src/config/mod.rs`.

## 2026-04-19 (feat — v0.15.0 inline `[dashboard]` layout)

### RT-110 — `expect("layout validated")` in publish tick panicked the loop
**Category:** Correctness
**Description:** `PublishLoop::tick_once` unconditionally unwrapped `build_svg_with_layout`. The comment said the layout was validated at `Config::load` time, but `PublishLoop::new` is public and takes any `Layout` — a hand-built or post-mutated layout could fail `resolve()` at render time and crash the loop.
**Fix:** Added `PublishError::Layout(#[from] LayoutError)`; `tick_once` propagates the error with `?`, `run()` logs it at `warn` and skips the tick just like any other transient publish error. `crates/bellwether/src/publish/mod.rs`.

### RT-111 — `Layout::embedded_default` accepted a parseable-but-unresolvable asset
**Category:** Correctness
**Description:** The `OnceLock` init only called `toml::from_str`; an edit to `assets/layout.toml` that parsed but had a resolver-level problem (e.g. oversized fixed children) would sit dormant until the first render tick or test that called `.resolve()`.
**Fix:** `embedded_default()` now calls `.resolve()` inside `get_or_init` with its own `expect`, so any asset-level breakage fires at startup. `crates/bellwether/src/dashboard/layout/mod.rs`.

---

## 2026-04-19 (feat — v0.14.0 configurable widget layout)

### RT-100 — `ForecastDay.offset` out-of-range panicked the renderer
**Category:** Correctness
**Description:** `render_widget` indexed `ctx.model.days[idx]` / `day_weekdays[idx]` with a `u8` from layout TOML; `offset = 3` or higher panicked with OOB.
**Fix:** Dispatch now uses `.get(idx)` on both arrays and falls back to `forecast_tile_out_of_range` (em-dash placeholder) for bad offsets. `crates/bellwether/src/dashboard/svg/mod.rs`.

### RT-101 — `expect("layout is well-formed")` panicked on user layouts
**Category:** Correctness
**Description:** `build_svg_with_layout` unconditionally unwrapped `compute_bounds`, so any user TOML triggering `MissingSizing`, `BothSizings`, or `Overflow` crashed the render thread.
**Fix:** `build_svg_with_layout` now returns `Result<String, LayoutError>`; the panic-free `build_svg` path uses the embedded default, whose success is guarded by a dedicated test (`embedded_layout_parses_and_resolves`). `MissingSizing`/`BothSizings`/`flex = 0` are now parse-time errors (see RT-102) rather than resolve-time ones. `crates/bellwether/src/dashboard/svg/mod.rs`, `layout/mod.rs`.

### RT-102 — XML injection via `HeaderTitle { text }`
**Category:** Security
**Description:** `render_header_title` interpolated user-supplied `text` directly into `<text>`; `debug_assert!` in the `text` helper panicked on `<`/`&` in debug, release emitted malformed SVG. Values like `"R&D"` or `"Tom's"` broke the pipeline.
**Fix:** Added `escape_xml` covering the five predefined entities (`&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`); `text()` now escapes its content unconditionally. `crates/bellwether/src/dashboard/svg/mod.rs`.

### RT-103 — `u32` overflow in layout arithmetic bypassed `Overflow` check
**Category:** Correctness
**Description:** `gaps * sep_per_gap`, `fixed_total + sep_total`, and `flex_budget * weight` all used plain `u32` math. Pathological user values (`gap = u32::MAX`, `flex = u32::MAX`) could wrap silently, fooling the `axis_len` guard and producing nonsense layouts or underflow panics downstream.
**Fix:** All resolver arithmetic is now `u64`-internal with `checked_add` / `checked_mul`, narrowed back to `u32` at the output boundary. New `LayoutError::ArithmeticOverflow` variant. Test `huge_gap_overflow_errors` covers the regression. `crates/bellwether/src/dashboard/layout/mod.rs`.

### RT-104 — `section_dividers` scanned flattened placements heuristically
**Category:** Correctness
**Description:** Inferred band-top Ys by scanning placement Ys — worked for the 5-band default, silently drew spurious full-width lines for nested horizontal/vertical structures.
**Fix:** `SplitNode.divider` is now first-class — the resolver emits `PlacedDivider` entries in the reserved 2-px gap, the renderer draws a single line per placement. No heuristic inference. `crates/bellwether/src/dashboard/layout/mod.rs`, `svg/mod.rs`.

### RT-105 — `SplitNode.divider = true` reserved space but never drew the line
**Category:** Semantics
**Description:** The `divider` flag consumed 2 px between children but the renderer ignored it; dividers were drawn by an unrelated hardcoded heuristic that checked widget identity (`Wind → Gust`) — data and render diverged.
**Fix:** Same as RT-104 — `divider` is now the single source of truth. Hardcoded `meteo_column_separators` deleted; the meteo `[[layout.children]]` entry in `assets/layout.toml` sets `divider = true` to get its two vertical column rules.

### RT-106 — Widget Y coordinates hardcoded — "configurable layout" was horizontal-only
**Category:** Semantics
**Description:** Clock (y=34), meteo cell (y=222), forecast label/icon/H-L (y=280/300/412/345), footer (y=462), battery (y=14/34) — every widget's vertical placement was a constant tied to the original 5-band heights. Resizing a band in `layout.toml` did nothing vertically.
**Fix:** Every widget render helper derives its Y from `bounds.y + bounds.h * pct / 100` (or a bounded constant like `BATTERY_LEFT_PAD`). Resizing the current-conditions band from 140 to 132 px in `layout.toml` now correctly moves its widgets with it. `crates/bellwether/src/dashboard/svg/mod.rs`.

### RT-107 — CHANGELOG `[Unreleased]` entry absent for 0.14.0
**Category:** Hygiene
**Description:** Version bumped to 0.14.0 without a corresponding CHANGELOG entry.
**Fix:** Added `### Added` / `### Changed` bullets under `[Unreleased]` describing the layout system and the `build_svg_with_layout` Result-typed signature change. `CHANGELOG.md`.

### RT-108 — `flex = 0` silently unpainted `flex_budget` pixels
**Category:** Correctness
**Description:** A child with `flex = 0` passed the resolver with zero share while still counted in `flex_total`; the last-flex-remainder override caught most cases but the all-zero-weight case leaked budget.
**Fix:** `flex = 0` is now rejected at TOML parse time in `Child`'s `TryFrom<ChildRaw>`; `Sizing::Flex(u32)` is guaranteed `>= 1`. Test `child_with_flex_zero_fails_to_parse` covers. `crates/bellwether/src/dashboard/layout/mod.rs`.

---

## 2026-04-19 (feat — v0.13.0 RPi deployment + `/api/setup`)

### RT-094 — Tautological test in deploy.rs never guards against build-dir wipe
**Category:** Correctness
**Description:** `sync_source_does_not_wipe_build_dir` searched for literal `rm -rf ~/bellwether-build` but the actual code uses `find ... -exec rm -rf {} +`, so the test passed vacuously regardless of the implementation.
**Fix:** Rewrote as `sync_source_preserves_target_cache` — slices out `sync_source`'s body, asserts the `! -name target` guard is present, and rejects any unguarded `rm -rf ~/bellwether-build` occurrence. `xtask/src/deploy.rs`.

### RT-095 — `StartLimitBurst` locked out recovery after reboot between setup and deploy
**Category:** Correctness
**Description:** If the Pi rebooted between `deploy-setup` (which enables the unit) and `deploy` (which places the binary), systemd burned through 5 failed starts in 60s and rejected further starts until `reset-failed`. `restart_and_verify` would report "not active" with no hint.
**Fix:** `deploy.rs:restart_and_verify` now runs `sudo systemctl reset-failed bellwether-web || true` before `start`. `xtask/src/deploy.rs`.

### RT-096 — Hardcoded `"bellwether"` `api_key` burned in at setup time
**Category:** Correctness / design
**Description:** `/api/setup` handed out the literal `"bellwether"` as `api_key` when no access token was configured. If the operator later set `BELLWETHER_ACCESS_TOKEN`, every previously-registered device sent the stale placeholder and got a 401 — the TRMNL firmware has no way to re-trigger `/api/setup` short of a factory reset.
**Fix:** Extracted to a named `pub const DEFAULT_UNCONFIGURED_API_KEY` with a doc comment that spells out the factory-reset caveat. Added a test asserting the constant is returned when no token is configured. Not a full solution (the operator still has to factory-reset to switch modes), but the contract is now explicit and discoverable. `crates/bellwether-web/src/api/trmnl/handlers.rs`.

### RT-097 — `config.toml` briefly world-readable during `scp` staging
**Category:** Security / information leak
**Description:** `scp` landed the file at `~/bellwether-config-tmp.toml` with the user's default umask (typically 0022 → mode 0644) before `sudo cp` moved it into `/opt/bellwether/config.toml` with 0640. On a multi-user Pi, other local users could read the file — and any secrets it contains — during that window.
**Fix:** `COPY_CONFIG` now starts with `umask 077` and `chmod 600 ~/bellwether-config-tmp.toml` as the first actions, so the staging file is only readable by the `scp` user. `xtask/src/deploy_setup.rs`.

### RT-098 — `ReadOnlyPaths` on `frontend-dist` conflicted with atomic-swap inode replacement
**Category:** Deployment
**Description:** `INSTALL_FRONTEND` does `rm -rf && mv` which replaces the directory inode. On older systemd versions the `ReadOnlyPaths` entry could end up pointing at the stale inode, causing restart-time mount failures. The entry was also redundant given `ProtectSystem=strict` already makes `/opt` read-only for the service.
**Fix:** Removed `frontend-dist` from `ReadOnlyPaths`; `config.toml` remains pinned there (no such swap). `deploy/bellwether-web.service`.

### RT-099 — `MemoryMax=256M` combined with `StartLimitBurst=5` risked OOM lockout
**Category:** Deployment
**Description:** 256 MiB is tight for axum + reqwest TLS + in-memory BMP rendering. An OOM kill combined with the start-limit burst would lock the service out of auto-restart for 60s.
**Fix:** Raised `MemoryMax=512M` to give rendering more headroom. `deploy/bellwether-web.service`.

## 2026-04-19 (feat — v0.12.0 Windy → Open-Meteo migration)

### RT-087 — `read_capped_body` allocated past the cap before checking
**Category:** Security / DoS
**Description:** `http_util::read_capped_body` called `buf.extend_from_slice(&chunk)` before testing `total > limit`, so a hostile server sending one large HTTP/2 frame could force a multi-megabyte allocation before the cap fired.
**Fix:** Check `total.saturating_add(chunk_len) > limit` *before* extending the buffer. `clients/http_util.rs:read_capped_body`.

### RT-088 — `series_or_nones` silently padded wire-format drift
**Category:** Correctness
**Description:** When Open-Meteo returned a series of different length than `time`, the helper ran `s.resize(n, None)` — half-a-forecast bugs would render as a dashboard with silently-disappearing data.
**Fix:** Replaced with `pick_series`, which returns the typed `OpenMeteoError::SeriesLengthMismatch` on any length mismatch. Absent series still map to `vec![None; n]`. Added regression test. `clients/open_meteo/mod.rs`.

### RT-089 — Non-finite floats propagated through dashboard calculations
**Category:** Correctness
**Description:** Nothing filtered `NaN` / `±Inf` at parse time, so a provider glitch could leak into `CurrentConditions::temp_c` and `feels_like_c`.
**Fix:** Added `sanitise_non_finite` which maps non-finite IEEE-754 values to `None` inside `pick_series`. Finite sentinels like `-9999` pass through for downstream tolerance. `clients/open_meteo/mod.rs`.

### RT-090 — `nearest_sample_index` could panic on extreme timestamps
**Category:** Correctness
**Description:** `(ts - now).num_seconds().saturating_abs()` delegates to chrono's `Sub<DateTime>` which panics on `TimeDelta` overflow. The `saturating_abs` guarded the i64 *after* the panic point.
**Fix:** Rewrote using raw Unix seconds: `ts.timestamp().saturating_sub(now_s).saturating_abs()`. Never panics. `dashboard/model/build.rs:nearest_sample_index`.

### RT-091 — `f64::to_string` for lat/lon could emit scientific notation
**Category:** Correctness
**Description:** Subnormals pass config validation but serialise as `"2.2e-308"`, which Open-Meteo's query parser rejects.
**Fix:** Format with `format!("{:.6}", req.lat)` — six decimals ≈ 11 cm precision, no exponent. `clients/open_meteo/mod.rs:fetch`.

### RT-092 — `OpenMeteoError::NotActiveProvider` was unreachable and untested
**Category:** Correctness / dead code
**Description:** With a single `ProviderKind` variant the `!=` comparison was statically false. First new provider would break the defensive check with no test catching it.
**Fix:** Dropped `FetchRequest::from_config` entirely. `FetchRequest::from_parts(lat, lon, &sub)` is infallible; `bellwether-web::build_provider` is the single dispatch point. Eliminates `NotActiveProvider` and `MissingProviderSubtable` variants. `clients/open_meteo/mod.rs`, `bellwether-web/src/main.rs`.

### RT-093 — `Policy::none()` turned legitimate 3xx into opaque errors
**Category:** Correctness
**Description:** Blocking all redirects meant a CDN canonical-host bounce would surface as an empty 301 body with no diagnostic.
**Fix:** Switched to `reqwest::redirect::Policy::limited(3)`. `clients/http_util.rs:build_http_client`.

---

## 2026-04-18 (feat — v0.11.0 dashboard SVG rewrite)

### RT-082 — Lone 120-px em-dash rendered when current conditions missing
**Category:** Correctness / UX
**Description:** `current_temperature_placeholder` emitted a single em-dash at `font-size=120` when the forecast had no usable current-temperature sample. The glyph floated alone in an otherwise-empty 140-px band — visually ambiguous (is it drawing garbage? a typo? a failed render?).
**Resolution:** Placeholder now renders "No current reading" at `font-size=44` centred in the band. Condition label + feels-like line remain suppressed. Test updated to assert the new string.

### RT-083 — Battery fill rect used integer truncation
**Category:** Correctness (minor visual)
**Description:** `battery_fill_rect` computed `inner_max * pct / 100`. With truncation `pct=99` rendered as 51 pixels (98% of 52), and `pct=1` rendered as a zero-width `<rect>` — syntactically valid but useless SVG bytes.
**Resolution:** Round-half-to-nearest (`(inner_max * pct + 50) / 100`). Skip the `<rect>` entirely when the rounded width is zero. New test `battery_fill_rounds_not_truncates` locks the invariant.

### RT-084 — Missing forecast tile dropped its weekday context
**Category:** Correctness / UX
**Description:** `day_placeholder` emitted only a bare em-dash at tile centre; the weekday label, icon, and H/L row were all absent. "Sat / — / Mon" gave the user no way to know whether Sunday or some other day was missing.
**Resolution:** `DashboardModel` gained a `day_weekdays: [Weekday; 3]` field populated by `build_model` from `ctx.now` + the same "skip-today, next-3" rule as the data rows. The SVG builder always renders the weekday header regardless of whether the data row is a placeholder. Updated test asserts all three labels are always present.

### RT-085 — Calm wind rendered as "Wind N 0 km/h"
**Category:** Correctness (data misrepresentation)
**Description:** `wind_to_compass` returns `Compass8::N` as a sentinel for calm conditions. The new meteo cell formatter passed that through verbatim, producing `"Wind N 0 km/h"` — a fake north wind at zero speed. Calm conditions should read as "calm", not a directional wind.
**Resolution:** `format_wind_cell` detects `round_i32(kmh) == 0` and emits `"Wind calm"` instead. New test `calm_wind_renders_as_calm_not_zero_knot_north`.

### RT-086 — `text()` helper interpolated `content` raw with no XML escape
**Category:** Defensive (security)
**Description:** The new shared `<text>` renderer spliced `content` directly into the SVG. Safe today (every call site passes enum-label returns, numeric-derived strings, or compile-time literals — none containing XML-special characters), but a future refactor letting a Windy-supplied string flow into `content` would open an injection path.
**Resolution:** Added a `debug_assert!` in `text()` that rejects `<` or `&` in `content`, and a doc comment locking the "literal / numeric / enum-returned only" invariant. Tests (which use `build_svg` with the sample model) exercise this path.

## 2026-04-17 (feat — v0.10.0 dashboard data-model groundwork)

### RT-077 — astro ephemeris anchored to UTC noon of local date
**Category:** Correctness
**Description:** `sunrise_sunset` computed its
Julian century from UTC noon of the calendar date
`date`, regardless of the supplied `tz`. For
timezones offset more than ±12 hours from UTC
(Kiritimati UTC+14, Samoa/Kiribati, American Samoa),
that anchor can be a full day away from the actual
sunrise/sunset event, stretching ephemeris drift
(declination, equation-of-time) to ~0.4°. On top of
that the returned `NaiveTime` was constructed from
UTC midnight of the supplied `date` — which worked
for the tested latitudes but was fragile.
**Resolution:** `sunrise_sunset` now computes JD from
the UTC instant of **local noon** on the requested
local date. That keeps the ephemeris reference within
±12h of any sunrise/sunset on the date regardless
of longitude, and the returned wall-clock time is
derived by converting the sunrise/sunset UTC instant
into `tz`. Added a Kiritimati (UTC+14) test that
would have failed under the old anchor. Joint
resolution with RT-080.

### RT-078 — `nearest_sample_index` could panic on extreme timestamp
**Category:** Correctness (DoS)
**Description:** `(ts - now).num_seconds().abs()`
panics on `i64::MIN`. A crafted or corrupt Windy
JSON with a timestamp near the extremes of
`i64::MIN_MS` would crash the publish loop.
**Resolution:** Replaced with `.saturating_abs()`
and added an inline comment citing the hardening
rationale.

### RT-079 — `/api/log` handler wiped cached battery on partial posts
**Category:** Correctness (semantics)
**Description:** The TRMNL firmware posts `/api/log`
for multiple reasons — wake-up reports, error
reports, keepalives. Not every post includes a
battery voltage. The previous `update_telemetry`
overwrote the whole cached `DeviceTelemetry`, so a
keepalive without a battery field would wipe the
last-known voltage, making the dashboard's battery
indicator flicker to "unknown" between genuine
reports.
**Resolution:** Added `DeviceTelemetry::merge_from`
that only updates fields whose value in the incoming
post is `Some`. `TrmnlState::update_telemetry` now
calls merge. Updated the
`log_without_battery_voltage_keeps_previous_value`
test to lock the new semantic (previously asserted
the overwrite-to-`None` behaviour, now asserts
preservation).

### RT-080 — Humidity from Windy not clamped before feeding feels-like
**Category:** Correctness
**Description:** `build_current` stored the raw
`rh-surface` value and passed it to
`apparent_temperature_c`. The Rothfusz heat-index
regression is only calibrated for `rh` in `[40,
100]`; feeding `rh=150` (Windy glitch territory)
produces a wildly high apparent temperature without
any boundary check.
**Resolution:** Clamp humidity to `[0, 100]` at the
`build_current` boundary with an inline comment
citing the rationale. Both the `humidity_pct` field
and the feels-like input see the clamped value.

### RT-081 — Astro at equinox near date line could flip "polar day" flag
**Category:** Correctness (edge case)
**Description:** A 1-day ephemeris error at high
latitudes near an equinox could push the
`hour_angle` arccos argument across the ±1
boundary, spuriously flipping between "sun rises"
and "polar night" on the wrong day.
**Resolution:** Subsumed by RT-077's local-noon
anchor; the ephemeris is now always within ±12h of
the target date's events so the declination value
is correct for the day the dashboard is rendering.

## 2026-04-17 (feat — swap dashboard font to Atkinson Hyperlegible)

### RT-074 — Changelog missed 0.9.0 breaking rename
**Category:** Project configuration
**Description:** The 0.9.0 bump on `Cargo.toml` was
driven entirely by the `M6X11_TTF` →
`ATKINSON_HYPERLEGIBLE_TTF` public rename, and that
rename was invisible in `CHANGELOG.md` — the
`[Unreleased]` section was left empty. A downstream
consumer reading release notes between 0.8.0 and
0.9.0 would have no signal that a public symbol was
renamed.
**Resolution:** Added a `[0.9.0] - 2026-04-17`
section with **Changed** entries covering the
bundled-font swap, the const rename (called out as
breaking), and the drop of the "multiples of 18"
font-size constraint. Historical 0.7.0 /0.8.0
entries left intact as history.

### RT-075 — Stale `m6x11plus` reference in Cargo.toml
**Category:** Project configuration
**Description:** The `ttf-parser` dev-dep comment in
`crates/bellwether/Cargo.toml:53` still described
the font as `m6x11plus` after the swap. The only
remaining live reference to the retired font in
non-historical files.
**Resolution:** Rewrote the comment to name the
Atkinson Hyperlegible font and list the two
non-ASCII glyphs that matter (U+00B0 degree sign,
U+2014 em dash).

### RT-076 — Em-dash placeholder glyph not in coverage test
**Category:** Correctness
**Description:** `dashboard/svg.rs` emits `—`
(U+2014) as the `PLACEHOLDER` string for every
missing-data field (null `high_c`, absent current
conditions, short-day tiles). The
`bundled_dashboard_font_covers_dashboard_glyphs`
test checked digits, ASCII letters, space, and
U+00B0, but not U+2014 — so a future font swap to
something without em-dash coverage would silently
render blank fields rather than the placeholder.
**Resolution:** Added `'—'..='—'` to the `ranges`
array, with a comment explaining why the em-dash
is required.

## 2026-04-17 (chore — `cargo xtask test --ignored`)

### RT-071 — `build_args` partially built args before validating filter
**Category:** Correctness (ordering discipline)
**Description:** With `ignored=true`, `build_args`
pushed `"--"` into the result vec before checking
whether the filter was empty. The function still
returned `Err` in that case, but only because the
`harness_args` outer guard and the empty-filter check
sat in sequence — a future refactor that dropped
either guard would ship a malformed command.
**Resolution:** Moved the empty-filter check to the
top of the function. The function now validates
inputs before touching the output vec, so any future
caller observing `Err` cannot see partial state. Added
`build_args_empty_filter_errors_even_with_ignored` to
pin the invariant.

### RT-072 — Trailing-comma quirk in test assert
**Category:** Style
**Description:** `assert_eq!(args, vec![...],);`
compiled but read like a missing third argument.
**Resolution:** Dropped the trailing comma.

### RT-073 — Em-dash in source comment
**Category:** Style
**Description:** `test_check` doc comment used a
literal `—` (U+2014) while the rest of the file used
ASCII `--`.
**Resolution:** Replaced with ASCII and rephrased to
avoid it.

## 2026-04-17 (feat — dashboard module with current + 3-day forecast layout)

### RT-066 — `build_current` used `timestamps[0]` regardless of `now`
**Category:** Correctness
**Description:** `tick_once` passed `Utc::now()` through to
`build_model`, but the current-conditions builder then
ignored `now` and pulled sample index 0. Windy returns
model-grid timestamps — `ts[0]` can be hours stale
(old run) or a few hours ahead of wall-clock (fresh
run), so the "big temperature" at the top of the
dashboard could be a past or future reading labelled
as "now".
**Resolution:** Added `nearest_sample_index(forecast,
now)` that picks the timestamp closest to `now` and
routed `build_current` through it. Test
`current_picks_sample_nearest_to_now_not_index_zero`
locks the behaviour: a 12-hour forecast with the
current wall-clock 6 samples in picks index 6, not
index 0.

### RT-067 — `day_high_celsius` returned magic `0` for all-null days
**Category:** Correctness
**Description:** `MIN_SAMPLES_PER_DAY` gated on
`indices.len()`, not on temperature availability. A day
with 24 indices but every `temp-surface` entry null
returned `high_c = 0` — indistinguishable from a real
0 °C day on the rendered dashboard.
**Resolution:** `DaySummary::high_c` is now
`Option<i32>`; `day_high_celsius` returns `None` when
`fold` produces `NEG_INFINITY`. The SVG builder's
`day_high` renders the em-dash placeholder for `None`.
New tests
`day_with_6_indices_but_all_null_temp_returns_high_none`
(model) and `day_with_none_high_renders_placeholder_temp`
(svg) pin it, and the latter also asserts the SVG never
emits `"0°"` for that state. Joint resolution with
AQ-079.

### RT-068 — Missing `clouds`/`precip` config parameters silently defaulted to Cloudy
**Category:** Correctness (operator footgun)
**Description:** An operator upgrading from 0.7 whose
`parameters = ["temp","wind","precip"]` still parsed
cleanly but produced "Cloudy" every tick because the
classifier had no cloud data to work with. The
one-shot `tracing::warn!` from `build_model` was the
only signal.
**Resolution:** Added
`ConfigError::MissingRequiredWindyParameters` and
`REQUIRED_WINDY_PARAMETERS` constant listing temp,
wind, clouds, precip. `Config::validate` now rejects a
non-empty `parameters` list that omits any of them at
load time. Empty `parameters` still works (webhook-
only deployments don't call Windy). New tests:
`rejects_byos_config_missing_required_windy_parameters`
and `accepts_empty_parameters_list_for_webhook_only_deployments`.

### RT-069 — DST spring-forward test didn't actually cross the boundary
**Category:** Test weakness
**Description:** `timezone_buckets_samples_by_local_date`
start timestamps were 2026-03-30 22:00 UTC — a day
after the UK 2026-03-29 01:00 DST transition. The
test name and comments promised DST coverage that the
fixture never exercised.
**Resolution:** Kept the original TZ-bucketing test
under a clearer name and added
`samples_straddling_spring_forward_bucket_into_same_local_date`
with samples at 2026-03-29 00:30 UTC and 01:30 UTC
(straddling the transition), asserting both land on
the 29th London local date.

### RT-070 — `tick_once_renders_plausible_trmnl_og_bmp` was wall-clock-dependent
**Category:** Test weakness (flake risk)
**Description:** `rich_forecast_fixture_from_now()` used
`Utc::now()` inside the fixture, and the test ran
`tick_once` which also calls `Utc::now()`. Near UTC
midnight the two calls straddled a date boundary and
the third day tile fell below the sample threshold;
only the generous `> 2000 black pixels` assertion hid
the flake.
**Resolution:** Renamed to `rich_forecast_fixture_at(start)`
and pushed `start` to the call site. The assertion
test still uses `now()`-relative times but the
internal behaviour is deterministic (any fixture
produces a valid BMP; coverage claim is untouched by
the skip-today edge). The `#[ignore]`'d sample writer
uses `Utc::now()` explicitly.

## 2026-04-17 (feat — bundle m6x11plus font + `Renderer::with_default_fonts`)

### RT-062 — `Renderer::with_default_fonts` copies 18 KiB per call
**Category:** Correctness (minor)
**Description:** Every call to the new
`Renderer::with_default_fonts` constructor allocates
a fresh `Vec<u8>` from the static `M6X11_TTF` slice.
Intent is "construct once per process" (documented
on `Renderer`), so in practice the copy happens once
at startup, but a caller who misread the doc and
built a renderer per-request would multiply that
cost.
**Resolution:** Not reverted — the copy is forced by
`fontdb::Database::load_font_data` taking owned
`Vec<u8>` (AQ-034). An inline comment on the call
site now names this, cites AQ-034, and points
readers back to the "construct once" guidance on
`Renderer`. Joint resolution with AQ-075.

### RT-063 — Glyph-coverage test only spot-checked range endpoints
**Category:** Correctness (test weakness)
**Description:** The `bundled_m6x11_font_covers_dashboard_glyphs`
test asserted presence of `'A', 'Z', 'a', 'z', '0',
'9'` — range endpoints only. A future font-subset
step could drop a middle glyph (e.g. `'M'`) and the
test would still pass even though the dashboard's
day labels or condition names would render
incorrectly.
**Resolution:** Test rewritten to iterate
`'0'..='9'`, `'A'..='Z'`, `'a'..='z'`, plus
single-character ranges for `' '` and `'°'`. Every
glyph the dashboard plausibly uses is now checked
individually.

### RT-064 — `"0°C"` rasterization threshold too lenient
**Category:** Correctness (test weakness)
**Description:** The end-to-end text-rendering test
`with_default_fonts_renders_degree_sign_glyph`
asserted `black > 50`. A broken font pipeline would
produce zero glyph pixels; "0°C" at 36 px
realistically produces hundreds. The threshold was
low enough that e.g. stroke residue on an empty
canvas could have slipped through.
**Resolution:** Threshold raised to `> 200` with a
comment explaining the calibration — well above the
"stroke residue" floor, well below actual glyph
coverage.

### RT-065 — `ttf-parser` dev-dep claimed to be pinned but wasn't
**Category:** Project configuration
**Description:** `Cargo.toml` commented
`ttf-parser = "0.25"` as "Pinned to the same version
fontdb/usvg pull in transitively". Caret 0.25
accepts any 0.25.x, not the exact version in
`Cargo.lock`. If fontdb bumped to `0.26` upstream,
the dev-dep and transitive could drift and land two
`ttf-parser` copies in the tree.
**Resolution:** Pinned to `"=0.25.1"` (the version
already in `Cargo.lock`) and the comment rewritten
to explain the intent (shared floor with fontdb,
no dup-on-minor-drift).

## 2026-04-17 (chore — port 3100 + config.example + HANDOFF rewrite)

### RT-057 — Example config recommended quota-exhausting refresh rate
**Category:** Correctness
**Description:** `config.example.toml` shipped
`default_refresh_rate_s = 60` with a comment claiming
it kept calls inside Windy's "12-calls/hour free-tier
quota." The 12/hour figure is the TRMNL cloud webhook
plugin limit (BYOS mode has none), and 60 s ≠ 12/hour
anyway.
**Resolution:** Default bumped to 900 s (15 min), in
line with the fixture and spike. Comment rewritten to
reference the Windy Personal plan budget correctly
and note that 60 s is acceptable for local iteration
but should not be committed.

### RT-058 — `.gitignore` root-anchored for credential files
**Category:** Security
**Description:** `/windy_key.txt` and `/ha_token.txt`
with leading `/` only matched at repo root. A secret
file dropped into `crates/bellwether/` or `deploy/`
would slip past the ignore and commit on `git add -A`.
**Resolution:** Leading `/` dropped for the credential
filenames (kept on `/config.toml` where the
root-anchoring is defensible). `windy_key.txt` and
`ha_token.txt` now ignored at any depth.

### RT-059 — CHANGELOG `[Unreleased]` empty despite user-visible change
**Category:** Project config
**Description:** The port default moved from 3000 to
3100 — user-visible. `config.example.toml`, new
`.gitignore` conventions, and a new README section
also landed. `[Unreleased]` was empty.
**Resolution:** Added `### Changed` (port default)
and `### Added` (config.example.toml, README section,
HANDOFF rewrite) entries to `[Unreleased]`.

### RT-060 — `CLAUDE.md` pointer described stale HANDOFF content
**Category:** Correctness (doc accuracy)
**Description:** The pointer text referenced "open
questions, recommended first steps" — scaffolding-era
language that no longer matches the rewritten
HANDOFF.
**Resolution:** Pointer now reads "current build
state, open decisions that block future PRs,
recommended next PRs, and user preferences."

### RT-061 — README `echo` recipe taught fragile whitespace pattern
**Category:** Correctness (low severity)
**Description:** `echo "your-windy-api-key" >
windy_key.txt` appends a newline. The loader trims so
it works, but the pattern is a footgun if ever copied
to a non-trimming context.
**Resolution:** Switched to `printf '%s' "..." >
windy_key.txt`, portable and newline-free.

---

## 2026-04-17 (PR 3c — v0.6.0 fetch/render/publish loop)

### RT-049 — `tokio::time::interval(0)` panics the publish task
**Category:** Correctness / DoS
**Resolution:** `Config::validate` now rejects
`default_refresh_rate_s` outside `1..=86400` with a
new `ConfigError::InvalidRefreshRate` variant. Tests
cover zero and above-max. The operator can't
accidentally write `default_refresh_rate_s = 0` and
silently lose the publish loop.

### RT-050 — Detached `JoinHandle` hides future
**Category:** Observability (defensive)
**Resolution:** New `publish::supervise(name, future)`
wraps the spawn with an outer task that logs at
`error!` if the inner task ends (clean return or
panic) and at `info!` on cancel. `main.rs` now uses
`supervise("publish_loop", loop_.run())`. The comment
explicitly says we do **not** auto-restart — a crash
loop would burn through Windy API quota. If the task
dies, the operator sees the error and restarts the
process.

### RT-051 — Wall-clock filenames collide on Pi without RTC
**Category:** Correctness
**Resolution:** Filenames are now
`dash-{counter:08}.bmp` from an `AtomicU64` inside
the `PublishLoop`. Monotonic regardless of clock
state; ordering visible in the filename; no
collisions. Test asserts the format
(`dash-00000000.bmp`, `dash-00000001.bmp`, …) and
URL-safety.

### RT-052 — `FetchRequest` derived `Debug` leaked api_key
**Category:** Security
**Resolution:** Replaced `#[derive(Debug)]` with a
manual `Debug` impl that emits `api_key:
"<redacted>"`. Any future `tracing::debug!(?req, …)`
is now safe.

### RT-053 — `fetch_request.clone()` per tick
**Category:** Performance (minor)
**Resolution:** `Client::fetch` now takes
`&FetchRequest`; the publish loop passes
`&self.fetch_request`. Eliminates per-tick String
cloning.

### RT-054 — Missing-temp indistinguishable from 0 °C
**Category:** UX / correctness
**Resolution:** `build_dashboard_svg` now branches on
`Some(temp)` vs `None`. With data: a filled bar.
Without data: the outline plus a diagonal X overlay
(two `<line>`s) so the operator can see "no data" at
a glance. A `tracing::warn!` also fires on the None
branch. Tests assert the X overlay is present when
no temp, absent when present.

### RT-055 — `build_dashboard_svg` bypassed config bounds
**Category:** Robustness
**Resolution:** Signature changed from
`(forecast, width, height)` to
`(forecast, &RenderConfig)`. The SVG builder now
always uses validated dimensions from `RenderConfig`.

### RT-056 — Supervisor + instant first tick → crash-loop quota burn
**Category:** Operational (deferred)
**Resolution:** `supervise` deliberately does NOT
auto-restart. Doc comment explicitly explains why.
If the process ever gets wrapped in a systemd
restart loop, a future change may need a startup
backoff — tracked as a TODO if/when supervision
policy changes.

---

## 2026-04-17 (PR 3b — v0.5.0 TRMNL BYOS endpoints)

### RT-036 / RT-045 — Split-lock race (torn state between `images` and `latest`)
**Category:** Correctness
**Resolution:** Collapsed `ImageStore` into a single
`RwLock<ImageStoreInner>` containing both the map and
the `Option<String>` latest pointer. All mutations are
atomic; readers of `latest_filename()` always see the
matching bytes.

### RT-037 — Lock poisoning propagates to every request
**Category:** Correctness (DoS)
**Resolution:** All `RwLock::read()`/`write()` calls
use `.unwrap_or_else(PoisonError::into_inner)` to
recover from poisoning. The BTreeMap can't be left in
a logically invalid state by an in-flight panic, so
recovery is safe.

### RT-038 — URL injection via filename in `image_url`
**Category:** Security
**Resolution:** `ImageStore::put_image` validates
filenames against `[A-Za-z0-9._-]{1,128}`, rejects
empty strings and leading dots. New
`InvalidFilename` error variant. Any `/`, `?`, `#`,
CRLF, or URL-like path flowing through `put_image` is
rejected at insert time before it can reach
`image_url`. Tests cover the common attempts.

### RT-039 — Malformed `public_image_base` produces nonsense URLs
**Category:** Correctness
**Resolution:** `TrmnlState::new` now validates the
base URL: non-empty, `http://` or `https://` scheme,
no query or fragment. Returns `InvalidBaseUrl`.
`"/"`, `"///"`, bare hostnames, and query/fragment
strings all rejected.

### RT-040 — `bytes.to_vec()` copies full BMP per GET
**Category:** Performance
**Resolution:** `ImageStore` stores `Bytes` (refcounted)
instead of `Arc<[u8]>`; `serve_image` returns the
`Bytes` directly, which axum implements
`IntoResponse` for. Zero-copy from store to wire.

### RT-041 — `/api/log` unbounded body + full-payload INFO log
**Category:** Security (DoS + log amplification)
**Resolution:** `DefaultBodyLimit::max(16 KiB)`
applied to the route; `TelemetryPayload` struct
parses known fields (battery_voltage, rssi,
fw_version) and structurally logs them at INFO.
Extras flow through `#[serde(flatten)] HashMap` and
are only printed at DEBUG, with an `extra_keys` count
at INFO so operators see unusual shapes without log
flooding. Test `log_rejects_oversized_body` confirms
the 413 response.

### RT-042 — No `Access-Token` authentication
**Category:** Security
**Resolution:** Optional token gating wired through a
`require_access_token` middleware and
`TrmnlState::with_access_token(token)`. Token comes
from `BELLWETHER_ACCESS_TOKEN` env var at startup;
empty/unset is treated as "no auth" for LAN-only
deployments with a prominent `tracing::warn` at
startup. Full config-file integration (mirroring
`windy.api_key_file`) can follow in a later PR
without breaking the wire format. Tests exercise
missing/wrong/correct token paths and the
empty-token escape hatch.

### RT-043 / AQ-052 — `seed_placeholder` swallowed errors
**Category:** Correctness / UX
**Resolution:** `seed_placeholder` now returns
`Result<()>`; `build_trmnl_state` uses `?` so a
broken renderer surfaces at startup as an anyhow
error rather than a silent 503.

### RT-046 / AQ-053 — `LogRequest` `#[serde(transparent)]` earned nothing
**Category:** API clarity
**Resolution:** Deleted the wrapper. The handler now
takes `Json<TelemetryPayload>` — a proper struct with
the known device fields plus a flatten catch-all.

### RT-047 / AQ-050 — Two `.nest("/api")` calls — startup-panic risk
**Category:** Correctness (test coverage)
**Resolution:** TRMNL router is now returned as a
complete `Router<()>` from `trmnl::router(state)`
(with state baked in), merged once into the top-level
router. Scaffold routes remain a separate nest; both
are merged via `.merge()`. New integration test
`trmnl_routes_reachable_through_create_router`
verifies `/api/display` resolves via the full
`create_router()`, so route-tree conflicts surface at
`cargo test` instead of at first request.

## Noted — not acted on

### RT-044 — `seed_placeholder` runs sync on the runtime thread
**Category:** Performance (minor)
**Resolution:** Deferred. The placeholder render
takes ~20 ms on a Pi; moving to `tokio::spawn` after
listener bind would let the server respond 503 during
rendering instead of delaying "listening on …" by
20 ms. Not worth the complexity right now.

### RT-048 — `/api/log` drops telemetry
**Category:** Feature gap
**Resolution:** Deferred. Added TODO for PR 3d:
persist last telemetry in `TrmnlState`, expose via
`/api/status` for operator visibility and for
refresh-rate adaptation.

---

## 2026-04-17 (PR 3a — v0.4.0 render module)

### RT-024 — Palette convention may be inverted (VERIFIED CORRECT)
**Category:** Correctness (blocker concern)
**Description:** Reviewer flagged that `palette[0] =
black, palette[1] = white` with `bit 1 = white` might
be inverted from TRMNL firmware expectations,
producing a photo-negative display.
**Resolution:** Investigated `usetrmnl/firmware`
`lib/trmnl/src/bmp.cpp`. Firmware supports **both**
orderings via a `reversed` flag; our current layout
matches its `"standart"` (canonical) path and also
matches `ImageMagick -monochrome` / Pillow
`convert('1')` default output and the official
TRMNL HA add-on converter. No code change needed;
added a doc comment in `bmp.rs`
`encode_1bit_bmp` + module-level note in `render/mod.rs`
pointing at the firmware reference.

### RT-025 — Dead-looking branch in `write_row` for widths % 8 == 0
**Resolution:** Added an inline comment explaining the
branch only fires when `width % 8 != 0`, so readers
don't mistake it for a bug.

### RT-026 — `.min(255)` in `rgba_to_luma` unreachable
**Category:** Correctness (defensive-code cleanup)
**Resolution:** Dropped the `.min(255)` clamp; the
Rec. 601 coefficients (77 + 150 + 29 = 256) paired
with 0..=255 inputs keep `y` strictly in `0..=255`.
Added `#[allow(clippy::cast_possible_truncation)]` on
the `as u8` cast and a comment explaining the bound.
See also AQ-039.

### RT-027 — Tiny-SVG scale-factor DoS
**Category:** Security (DoS)
**Description:** SVG with `width="0.0001"` produced
`scale_x ≈ 8_000_000`, causing the rasterizer to
burn CPU on degenerate geometry.
**Resolution:** New `RenderError::InvalidScale
{ scale_x, scale_y }`; renderer rejects scales
outside `(0, MAX_SCALE]` with `MAX_SCALE = 8192.0`.
Test `rejects_svg_that_would_require_excessive_scale`
exercises the path with a 0.001 × 0.001 viewport.

### RT-028 — Non-finite scale from crafted SVG
**Resolution:** Same `InvalidScale` check handles
`is_finite()` defensively alongside the range check.

### RT-029 — Unbounded `RenderConfig.width` / `height`
**Category:** Security (DoS / OOM)
**Description:** `[render]` with
`width = 65535, height = 65535` would request a 17 GB
RGBA pixmap on 64-bit systems, or overflow `width *
height` on 32-bit RPi targets.
**Resolution:** New
`ConfigError::InvalidRenderDimensions { width, height }`
variant. `Config::validate` now rejects dimensions
outside `1..=4096` (TRMNL X tops out at 1872; 4096
leaves headroom for future devices without another
SemVer-breaking bound change). Tests
`rejects_out_of_range_render_dimensions` and
`rejects_zero_render_dimension` added to
`config/mod.rs`.

### RT-030 — Regression test for safe-by-default usvg resolver
**Resolution:** New test
`ignores_external_file_references_in_svg` feeds an
SVG with `<image href="file:///etc/passwd">` and
asserts the output remains all-white. Locks in the
defense-in-depth today and fails loudly if a future
feature flip enables a filesystem-aware resolver.

### RT-033 — `Renderer` not `Clone` despite doc claim
**Category:** API coherence
**Resolution:** Updated the `Renderer` doc to state
explicitly that it is **not** `Clone` (usvg's
`FontResolver` trait object isn't `Clone`; losing
loaded fonts on an implicit copy would be a footgun).
`Arc::make_mut` kept because it's the right primitive
if anyone later wraps the Renderer in an Arc graph.
See also AQ-035.

### RT-035 — `i32::try_from(width).unwrap()` panics without message
**Category:** Correctness (diagnostics)
**Resolution:** Added an explicit top-of-function
assertion in `encode_1bit_bmp`:
`assert!(i32::try_from(width).is_ok() &&
i32::try_from(height).is_ok(), "dimensions exceed BMP
i32 field capacity")`. Unreachable in practice given
the new RenderConfig bound (4096), but surfaces a
clear message if ever hit.

## Noted — not acted on

### RT-031 — XML-bomb via deeply nested SVG
**Resolution:** Not a render-module concern. Added
module-level doc ("Caller responsibilities") saying
web consumers should cap `svg_text.len()` (~1 MiB).
Caller (PR 3b, web crate) will enforce.

### RT-032 — Adversarial fonts DoS
**Resolution:** Added a trust-boundary paragraph to
`load_font_data`'s doc comment. Font parsing is safe
Rust but crafted fonts can panic / spin; doc says
not to pass user-uploaded blobs unsandboxed.

### RT-034 — `cargo audit` / `cargo deny` in CI
**Resolution:** Logged to `TODO.md` as a follow-up
chore; not in scope for this PR.

---

## 2026-04-17 (PR 2 — v0.3.0 Windy client)

### RT-009 — `#[serde(flatten)]` into `HashMap<String, Vec<f64>>` brittle to new fields
**Category:** Correctness (forward-compat)
**Description:** Any new top-level field Windy added
(scalar metadata, nested objects) would fail to
deserialize as `Vec<f64>`, turning a 200 OK into
`WindyError::Parse`.
**Resolution:** Flatten into
`HashMap<String, serde_json::Value>`; in
`RawResponse::into_forecast`, try-convert each value to
`Vec<Option<f64>>` and silently drop entries that don't
match. Test
`fetch_ignores_unknown_non_numeric_fields` locks it in
with elevation/model_id/metadata noise.

### RT-010 — No length-match check between `ts` and series
**Category:** Correctness
**Description:** Doc claimed "length matches timestamps"
but nothing enforced it.
**Resolution:** New variant
`WindyError::SeriesLengthMismatch { key, expected, got }`;
enforced in `into_forecast`. Test
`fetch_rejects_series_length_mismatch`.

### RT-011 — Empty `ts` accepted as a valid Forecast
**Category:** Correctness
**Description:** Downstream consumers would panic on
indexing into empty timestamps.
**Resolution:** New variant
`WindyError::EmptyForecast`; `into_forecast` returns it
when `ts.is_empty()`. Doc on `Forecast::timestamps`
guarantees non-empty. Test `fetch_rejects_empty_ts`.

### RT-012 — API key could leak via `Api { body }` if server echoed it
**Category:** Security
**Description:** Proxies (Cloudflare, nginx 413) and
verbose API errors sometimes echo the request body.
That body was stored verbatim in the `Api` variant,
which downstream logs/anyhow would happily print.
**Resolution:** New `redact_secret(text, secret)`
helper; called on error bodies before they're stored
in `WindyError::Api`. Empty-secret is a no-op to avoid
blanking everything when key isn't set. Test
`fetch_surfaces_api_error_with_redacted_body` uses a
bait key and asserts redaction.

### RT-013 — Unbounded response body read (DoS / OOM)
**Category:** Security
**Description:** `resp.text()` / `resp.json()` read
the full body without any cap.
**Resolution:** New `read_capped_body(resp, limit)`
helper: checks `Content-Length` up front, then streams
`resp.chunk().await?` with a running counter. Exceeds
the cap → `WindyError::ResponseTooLarge { limit }`. The
cap is per-Client (`max_response_bytes` /
`max_error_body_bytes`) with `with_max_response_bytes`
builder for tests. Defaults: 4 MiB success, 4 KiB
errors. Tests
`fetch_rejects_oversized_response_via_content_length`
and `fetch_rejects_oversized_error_body`.

### RT-014 — `InvalidTimestamp` branch was unreachable in tests
**Category:** Robustness / coverage
**Resolution:** Added test
`fetch_surfaces_invalid_timestamp` that sends `i64::MAX`
(~292M years) — outside `NaiveDateTime` range — and
asserts the variant fires. Branch now covered.

### RT-015 — `Mock::expect(1)` Drop-time assertion unreliable
**Category:** Test hygiene
**Resolution:** The rewrite doesn't use
`.expect(1)` anymore. Request-body assertions rely on
`body_json` matchers that force an unmatched request to
404 → `WindyError::Api` → `.unwrap()` panic. The test
comment documents this explicitly.

### RT-016 — Default redirect policy could re-POST API key cross-origin
**Category:** Security
**Description:** DNS hijack or CDN compromise could
redirect `api.windy.com` to an attacker; reqwest's
default policy follows 10 redirects and does NOT strip
the body cross-origin, leaking the key.
**Resolution:** `Client::with_base_url` configures
`.redirect(Policy::none())`. A 302 now surfaces as
`WindyError::Api { status: 302 }` instead. Test
`fetch_does_not_follow_redirects`.

### RT-017 — No long-lived-Client documentation
**Resolution:** `Client` doc now says "Build once per
process and reuse. Construction pays for TLS setup and
connection-pool initialization."

### RT-018 — `Vec<f64>` fails on Windy `null` values
**Category:** Correctness
**Description:** Windy returns `null` at grid edges for
some models/timesteps. `Vec<f64>` couldn't deserialize
that.
**Resolution:** `Forecast::series` is now
`HashMap<String, Vec<Option<f64>>>`. Test
`fetch_preserves_null_values_in_series`.

### RT-019 — `serde_json` now a direct dep
**Resolution:** Added a comment above the reqwest dep
in `Cargo.toml` noting that `serde_json` is for the
Windy JSON wire format only; config is TOML-only.

### RT-020 — Unused tokio `test-util` feature
**Resolution:** Removed from dev-dependencies.

### RT-021 — No separate `connect_timeout`
**Category:** Operational
**Resolution:** `connect_timeout(5s)` set, overall
`timeout(20s)` — gives a slow RPi connect room to
finish before the overall timeout fires.

### RT-022 — No gzip compression negotiated
**Category:** Operational
**Resolution:** Added `"gzip"` to reqwest's feature
set. Halves typical Windy response sizes over
cellular.

### RT-023 — Empty `parameters` list silently sent
**Category:** Correctness / waste
**Resolution:** `Client::fetch` short-circuits with
`WindyError::NoParameters` before building the
request. Test `fetch_rejects_empty_parameters_early`
exercises the early-return path (no mock server
needed).

## 2026-04-17 (PR 1 — v0.2.0 config skeleton)

### RT-001 — `TrmnlMode` not coupled to its subsection
**Category:** Correctness
**Description:** `mode = "byos"` with no `[trmnl.byos]`
table parsed to `Ok` with `byos: None`. Bug would only
surface at publish time.
**Resolution:** Collapsed `TrmnlConfig` into a
`#[serde(tag = "mode")]` internally-tagged enum. The
TOML schema flattened (`[trmnl]` holds `mode` +
variant-specific fields directly; no nested
`[trmnl.byos]`). Illegal states are now
unrepresentable; missing variant fields fail at parse
time. Added `rejects_mode_without_matching_payload`
test.

### RT-002 — `#[serde(deny_unknown_fields)]` not set
**Category:** Correctness
**Description:** Typos in defaulted fields (e.g.
`heigth = 500`) were silently ignored.
**Resolution:** Added `#[serde(deny_unknown_fields)]`
to `Config`, `WindyConfig`, and `RenderConfig`.
Skipped on `ByosConfig` / `WebhookConfig` because
serde's internally-tagged enum representation is
incompatible with `deny_unknown_fields` on variant
structs — a known serde limitation. Added
`rejects_unknown_top_level_field` test to lock in the
behaviour.

### RT-003 — `Path::parent()` returns `Some("")` for bare filenames
**Category:** Correctness (broken doc promise)
**Description:** `bellwether --config config.toml`
with a bare filename caused `Path::parent()` to return
`Some("")`, leaving `api_key_file` relative and read
against CWD, violating the documented "resolved
against config file's parent directory" contract.
**Resolution:** Introduced `config_base_dir` that
treats an empty parent as "no directory component"
and falls back to `std::env::current_dir()`.

### RT-004 — `api_key_file` not validated at load time
**Category:** Correctness / UX
**Description:** Missing or unreadable secret files
only failed at first forecast fetch, long after the
"loaded config" success print.
**Resolution:** `Config::load` now eagerly reads the
secret via `read_secret` and stashes the trimmed
result in a private `api_key` field on `WindyConfig`.
Misconfig fails at startup.

### RT-005 — `read_api_key` returned `Ok("")` for empty secret files
**Category:** Correctness
**Description:** Whitespace-only key files produced
an empty key, which would surface as a confusing
remote 401/403.
**Resolution:** New `ConfigError::EmptySecret { path }`
variant; `read_secret` returns it when the trimmed
contents are empty. Test `rejects_empty_secret`.

### RT-007 — `ConfigError::Io` conflated config-file and secret-file errors
**Category:** Correctness / API design
**Description:** Callers couldn't distinguish a
broken `config.toml` from a broken secret file.
**Resolution:** Split into `ConfigError::ReadConfig`
and `ConfigError::ReadSecret` — each carries the
specific `path` and wraps `std::io::Error`. Tests
`reports_read_config_error_for_missing_file` and
`reports_read_secret_error_for_missing_key_file`.

### RT-008 — Test used bare relative path; false-pass risk
**Category:** Correctness (test hygiene)
**Description:**
`cli_reports_error_for_missing_config` used
`"definitely-not-a-real-path.toml"` as a CLI arg, so
any developer creating that file in the crate root
could silently break it.
**Resolution:** Switched to `TempDir::new().path().join(...)`
for an absolute, guaranteed-nonexistent path.

## Noted — not acted on

### RT-006 — Dependency versions loosely pinned
**Category:** Project configuration (low-priority)
**Description:** `serde = "1"`, `toml = "0.8"`,
`tempfile = "3"` admit any compatible semver release.
**Resolution:** No change — consistent with the rest
of the crate's dep policy (`anyhow = "1"`, `clap =
"4"`) and `Cargo.lock` locks the actual versions
that ship. Would need to be addressed project-wide
rather than per-dep.
