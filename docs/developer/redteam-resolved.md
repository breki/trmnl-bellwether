# Red Team Findings -- Resolved

Archive of fixed red team findings, newest first.
See [redteam-log.md](redteam-log.md) for open findings.

---

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
