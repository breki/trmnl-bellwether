# Artisan Findings -- Resolved

Archive of fixed Artisan code quality findings, newest
first. See [artisan-log.md](artisan-log.md) for open
findings.

---

## 2026-04-17 (PR 3b — v0.5.0 TRMNL BYOS endpoints)

### AQ-046 — Same as RT-036 (composite RwLock)
**Resolution:** See RT-036.

### AQ-047 — Same as RT-040 (zero-copy BMP response)
**Resolution:** See RT-040.

### AQ-048 — `#[non_exhaustive]` missing on public types
**Category:** API evolution
**Resolution:** Added to `TrmnlState`, `DisplayResponse`,
`ImageStore`, `InvalidFilename`, `InvalidBaseUrl` to
match the convention used in config / render modules.

### AQ-049 — Inconsistent re-exports from `api::mod`
**Resolution:** Trimmed to the types callers actually
use: `TrmnlState`, `RefreshInterval`.
`DisplayResponse`, `ImageStore`, and error types stay
reachable via `api::trmnl::*` but aren't re-exported
at the module level.

### AQ-050 — Same as RT-047 (single `/api` nest + tested)
**Resolution:** See RT-047.

### AQ-051 — `--config` optional silently masked misconfiguration
**Category:** UX / deployment
**Resolution:** `--config` now errors out if absent
unless the new `--dev` flag is passed. `--dev` emits a
clear warning. Production deployments that forget
`--config` fail fast with a clear message.

### AQ-053 — Same as RT-046 (delete `LogRequest` wrapper)
**Resolution:** See RT-046.

### AQ-054 — `default_refresh_rate_s: u32` unit-encoded in field name only
**Category:** Type safety
**Resolution:** Introduced `RefreshInterval(Duration)`
newtype with `from_secs(u32)` constructor and a
`Serialize` impl that emits `u32` seconds. Every
construction site reads as "seconds"; future
constructors can introduce `from_minutes` / `from_millis`
without breaking the wire format. Test
`refresh_interval_serializes_as_u32_seconds` locks
the JSON shape.

### AQ-055 — `firmware_url` always-empty String
**Category:** Type honesty
**Resolution:** Changed to
`firmware_url: Option<String>` with
`#[serde(skip_serializing_if = "Option::is_none")]`.
Field is omitted from `/api/display` JSON when
no firmware update is pending (always, for now).

### AQ-056 — Module ~390 lines — break the directory convention
**Resolution:** Split into
`crates/bellwether-web/src/api/trmnl/mod.rs` +
`api/trmnl/tests.rs`. Matches the pattern in
`config/`, `clients/windy/`, and `render/`.

### AQ-057 — `PLACEHOLDER_SVG` in binary
**Category:** Abstraction
**Resolution:** Moved to
`crates/bellwether/src/render/placeholder.svg` and
exposed as `Renderer::placeholder_bmp(&RenderConfig)`.
Both `bellwether-web`'s startup and the future
render loop (PR 3c) will call the same helper; no
copy-paste.

### AQ-058 — Logging full telemetry payload at INFO
**Resolution:** See RT-041. Known fields log at INFO,
extras only at DEBUG.

### AQ-059 — `trim_end_matches('/')` per request
**Resolution:** `TrmnlState::new` now calls
`normalize_base` which trims / validates once. The
`display` handler concatenates the already-normalized
base with the filename.

### AQ-060 — Over-generic `impl Into<T>` on cold-path setters
**Resolution:** `put_image`, `TrmnlState::new`, and
`RefreshInterval::from_secs` take concrete types
(`String`, `Bytes`, `&str`, `u32`). Callers with
`&str` write `.to_owned()` once; compile times and
docs are both slightly cleaner.

### AQ-061 — `TrmnlState::images: pub Arc<ImageStore>` leaked storage shape
**Category:** Abstraction
**Resolution:** Field is now private. Public methods
`put_image`, `get_image`, `latest_filename`,
`public_image_base`, `default_refresh_interval`.
Future ImageStore refactors (eviction, size cap) can
change the shape without breaking callers.

---

## 2026-04-17 (PR 3a — v0.4.0 render module)

### AQ-031 — `ParseSvg(String)` loses structured usvg error info
**Category:** Error design
**Resolution:** Documented the trade-off explicitly in
the `RenderError::ParseSvg` variant doc: "flattened
`usvg::Error` message for human consumption. Matching
on parse subcategories is not supported; if that
becomes necessary, expose the typed error via
`#[source]`." Future-upgrade path preserved.

### AQ-032 — `UnsupportedBitDepth { bits: u8 }` erased enum context
**Category:** Type safety
**Resolution:** Changed to `UnsupportedBitDepth { depth:
BitDepth }`. Error message uses `{depth:?}`; if
`BitDepth` grows a third variant, the error message
is automatically up to date. Test updated to assert
`depth == BitDepth::Four`.

### AQ-034 — `load_font_data(Vec<u8>)` rationale not documented
**Resolution:** Added explicit doc explaining that
`fontdb` stores the bytes for the database's lifetime
without copying; `&[u8]` would force an internal
clone. Also added the "Trust boundary" paragraph
covered under RT-032.

### AQ-035 — Same as RT-033 (Clone coherence)
**Resolution:** See RT-033. Docs now state Renderer is
not `Clone` and why.

### AQ-036 — `Debug` field `fonts` read like a collection
**Resolution:** Renamed to `font_count`. Test updated
to match.

### AQ-038 — `255` literal repeated in `rgba_to_luma`
**Resolution:** Extracted `WHITE_BG: u32 = 255`
constant; alpha compositing now goes through a small
`composite(channel, alpha, inv_alpha)` helper so
`WHITE_BG` is named exactly once per channel write.

### AQ-039 — Same as RT-026
**Resolution:** See RT-026.

### AQ-042 — Same as RT-035
**Resolution:** See RT-035.

## Noted — not acted on

### AQ-033 — `render_to_bmp(&RenderConfig)` signature
**Resolution:** Confirmed as-is; matches project
convention.

### AQ-037 — Triple-param `(bits, width, height)` refactor
**Resolution:** Deferred. Introduce an `Image` /
`Gray8` / `Bits1` struct when a third pipeline stage
(e.g., 4-bit grayscale for TRMNL X) lands. Two
consumers don't yet justify the abstraction.

### AQ-041 — Builder pattern for Renderer
**Resolution:** Deferred. Current shape (`new()` +
`load_font_data`) is close enough to a builder that
migration will be cheap when a second post-construct
knob arrives.

---

## 2026-04-17 (PR 2 — v0.3.0 Windy client)

### AQ-016 — `WindyError` over-differentiated `reqwest::Error`
**Category:** Error design
**Description:** Three variants (`BuildClient`,
`Transport`, `Parse`) wrapped the same underlying type
for no programmatic distinction.
**Resolution:** Collapsed to a single
`Http(Box<reqwest::Error>)` variant with
`impl From<reqwest::Error>`. Callers needing
fine-grained distinctions can match on the inner error
via `.is_timeout()` / `.is_connect()` / `.is_decode()`.
`BuildClient` variant removed entirely because
`Client::new` is now infallible (see AQ-019).

### AQ-017 — `reqwest::Error` leaked in public source chain
**Category:** Abstraction
**Resolution:** Wrapped in `Box<reqwest::Error>` in the
`Http` variant (parallels the `Box<toml::de::Error>`
treatment in config `ParseToml`). Still exposes the
type, but boxed and explicitly called out; consumers
that don't care can Display/Debug without depending on
`reqwest` themselves.

### AQ-018 — `Api::body: String` payload size
**Category:** Memory layout
**Resolution:** Changed to `Box<str>` — 16 bytes in the
discriminant union instead of 24, and signals the body
is immutable once constructed.

### AQ-019 — `Client::new` / `with_base_url` returned `Result` despite near-infallibility
**Category:** API design
**Resolution:** Both are now infallible: they panic
with a clear message if TLS init fails. Matches
`reqwest::Client::new`'s semantics. Added `Default` impl
for ergonomics. `WindyError::BuildClient` removed.

### AQ-020 — `FetchRequest<'a>` borrowed-only forced awkward storage
**Category:** API design
**Resolution:** `FetchRequest` is now owned
(`String`/`Vec<WindyParameter>`). Schedulers and the
web layer can store one between ticks without lifetime
juggling. One extra clone per fetch, negligible
compared to the HTTP round-trip.

### AQ-021 — No `fetch_with_config(&WindyConfig)` convenience
**Category:** API design
**Resolution:** Added `Client::fetch_with_config` and
`FetchRequest::from_config(&WindyConfig)` — the latter
returns `WindyError::MissingApiKey` if the config was
parsed via `Config::from_toml_str` (which doesn't
populate the secret). Two tests cover happy path and
missing-key path.

### AQ-022 — `Forecast` exposed Windy wire-format keys as stringly-typed map
**Category:** Abstraction
**Resolution:** Added `Forecast::values(parameter:
WindyParameter) -> Option<&[Option<f64>]>` that handles
the `"{wire_name}-surface"` key computation internally.
Keys remain `String` for flexibility (levels beyond
surface will be added later); renderers should prefer
`values()` for known parameters. Also added
`WindyParameter::wire_name()` with a test
(`wire_name_matches_serde_rename`) that asserts the
function and the `#[serde(rename)]` attributes stay
aligned.

### AQ-023 — same concern as RT-009 (flatten trap)
**Resolution:** See RT-009.

### AQ-024 — `InvalidTimestamp` message was unhelpful
**Category:** Error messaging
**Resolution:** Renamed to a struct variant
`InvalidTimestamp { ms }` with message "invalid
timestamp {ms} ms from Windy response (outside
DateTime<Utc> range)".

### AQ-025 — `Client` lacked `#[derive(Debug)]`
**Resolution:** Added `#[derive(Debug, Clone)]`.

### AQ-026 — `DEFAULT_BASE_URL` + path duplication
**Resolution:** Added `ENDPOINT_PATH` and
`DEFAULT_ENDPOINT` constants plus a
`Client::endpoint()` accessor. Test
`endpoint_composes_base_and_path` locks the
composition. Test `default_endpoint_constant_matches_composition`
asserts the constant agrees with the computed value.

### AQ-027 — Module approaching 500-line threshold
**Resolution:** Promoted `clients/windy.rs` to a
directory module (`clients/windy/{mod,tests}.rs`).
Production code in `mod.rs`; all unit tests in
`tests.rs` via `#[cfg(test)] mod tests;`.

### AQ-028 — `live_windy` used runtime env-var branching
**Category:** Test hygiene
**Resolution:** Gated on `#[cfg(feature =
"live-tests")]`. Added a `live-tests` feature in
`Cargo.toml`. Default builds no longer compile the
test; `cargo test --features live-tests -- --ignored
live_windy` runs it with `BELLWETHER_WINDY_KEY` set.

### AQ-029 — `fetch(&FetchRequest<'_>)` took by reference
**Resolution:** Takes `FetchRequest` by value now,
matching reqwest's builder idiom. Works with owned
fields from AQ-020.

### AQ-030 — Trailing commas on single-line `assert_eq!`
**Resolution:** rustfmt left most alone; those it kept
are multi-line. Functionally irrelevant.

## 2026-04-17 (PR 1 — v0.2.0 config skeleton)

### AQ-001 — `ConfigError::Toml` leaked `toml::de::Error`
**Category:** Abstraction boundaries
**Description:** The public variant payload was
`toml::de::Error`, pinning consumers to the `toml`
crate version.
**Resolution:** Renamed variant to `ParseToml`;
payload is now a struct `{ path: Option<PathBuf>,
source: Box<toml::de::Error> }`. Path identifies the
offending file (missing info before). Boxed the
`toml::de::Error` because clippy's `result_large_err`
lint flagged the raw type (128 bytes). Consumers can
still introspect via `#[source]` if they depend on
`toml`, but the common case (Display / Debug) no
longer requires it.

### AQ-002 — `Config::load(&Path)` too rigid
**Category:** API design
**Description:** Callers with `&str`, `String`,
or `PathBuf` had to manually convert.
**Resolution:** Changed signature to
`pub fn load(path: impl AsRef<Path>) ...`, idiomatic
for file-loading APIs.

### AQ-003 — No in-memory parse entry point
**Category:** API design
**Description:** Consumers wanting to parse a TOML
string had to write it to disk first.
**Resolution:** Added `Config::from_toml_str` that
parses + validates without touching the filesystem.
Useful for tests, preview flows, and future snapshot
testing. Test `from_toml_str_parses_without_disk_io`.

### AQ-004 — `timezone: String` stringly-typed
**Category:** Type safety
**Description:** Typos in timezone names surfaced
only at render time.
**Resolution:** Added `chrono-tz` dep (with `serde`
feature); `RenderConfig::timezone` is now
`chrono_tz::Tz`. Typos fail at config load.

### AQ-005 — `parameters: Vec<String>` stringly-typed
**Category:** Type safety
**Description:** Windy's parameter set is closed; a
typo like `"temperature"` would 400 at runtime.
**Resolution:** Introduced `WindyParameter` enum with
`#[serde(rename_all = "lowercase")]` and
`#[non_exhaustive]`. Closed set covers `temp`, `wind`,
`windGust`, `precip`, `pressure`, `clouds`, `rh`,
`dewpoint`.

### AQ-006 — `bit_depth: u8` accepted nonsense values
**Category:** Type safety
**Description:** `bit_depth = 7` parsed fine; the
invariant "1 or 4" lived only in doc comments.
**Resolution:** Introduced `BitDepth` enum with
`#[serde(try_from = "u8")]` accepting 1 or 4.
`BitDepth::bits() -> u8` returns the numeric value for
rendering. Test `rejects_invalid_bit_depth`.

### AQ-007 — same as RT-001 (discriminator/payload coupling)
**Resolution:** See RT-001 above.

### AQ-008 — `lat`/`lon` had no range validation
**Category:** Type safety
**Description:** Any `f64` parsed, including `NaN`
and out-of-range values.
**Resolution:** Added a private `Config::validate`
called from both `from_toml_str` and `load`. Checks
`is_finite()` and `[-90, 90]` / `[-180, 180]`
respectively. New variants
`ConfigError::InvalidLatitude` and
`ConfigError::InvalidLongitude`. Tests
`rejects_out_of_range_latitude` and
`rejects_nan_longitude`. Chose validation over a
newtype to keep the deserialization shape simple; a
`LatLon` newtype can follow if more call sites need
the guarantee.

### AQ-009 — `read_api_key` did per-call I/O; `String` unsafe in `Debug`
**Category:** API design / secrets
**Description:** Spike §5 called for startup-time
load; the bare `String` return also risked leaking
into `Debug` output.
**Resolution:** Secret is read once in `Config::load`
and stashed in a private `api_key: Option<String>`
field on `WindyConfig` (skipped from serde). Manual
`impl Debug for WindyConfig` redacts the key as
`<redacted>`. No external consumer can read the path
→ key mapping except via `api_key()`. Test
`debug_redacts_api_key`.

### AQ-010 — Relative-path resolution hardcoded to Windy
**Category:** Abstraction
**Description:** When HA's `token_file` lands, the
current resolution block wouldn't pick it up.
**Resolution:** Extracted a `pub(super) fn
resolve_relative(base: &Path, p: &mut PathBuf)`
helper in `config/mod.rs`. `WindyConfig` now calls
it via a `resolve_api_key_path(&mut self, base)`
method. The future HA module will call the same
helper from its own `resolve_*_path` method.

### AQ-011 — CLI summary format lived in the binary
**Category:** Abstraction
**Description:** `main.rs` hand-formatted the loaded
config summary; `{:?}` on `TrmnlMode` printed Rust
casing.
**Resolution:** Added `impl Display for Config` and
`impl Display for TrmnlConfig` (lowercase mode names
matching the TOML). Binary is now
`println!("loaded config: {cfg}");`. Test
`display_uses_lowercase_mode`.

### AQ-012 — Missing `#[non_exhaustive]` on public enums
**Category:** API evolution / SemVer
**Resolution:** Added `#[non_exhaustive]` to
`ConfigError`, `TrmnlConfig`, `WindyParameter`, and
`BitDepth`. Adding variants is now a minor-version
change.

### AQ-013 — `RenderConfig::default` duplicated serde defaults
**Category:** Maintainability
**Resolution:** `Default::default` now returns
`toml::from_str("").expect(...)`, which rebuilds the
struct via the already-configured field-level
defaults. Adding a field to `RenderConfig` now
requires one edit (the field + its
`#[serde(default)]`) instead of three. Test
`default_matches_serde_defaults`.

### AQ-015 — `config.rs` was 302 lines and growing
**Category:** Module size
**Resolution:** Promoted to a directory module:
`config/mod.rs` (Config, ConfigError, load,
from_toml_str, helpers), `config/windy.rs`
(WindyConfig, WindyParameter, Windy defaults),
`config/trmnl.rs` (TrmnlConfig, variants, TRMNL
defaults), `config/render.rs` (RenderConfig,
BitDepth, render defaults). Each file owns its
struct, defaults, and focused tests. The crate-facing
API is unchanged — all types still re-exported from
`bellwether::config`.

## Noted — not acted on

### AQ-014 — `PartialEq` vs `Eq` inconsistency
**Category:** Minor API
**Description:** `Config` and `WindyConfig` derive
`PartialEq` only (due to `f64` lat/lon); the other
types derive both.
**Resolution:** Not fixed. The `f64` fields are
inherently non-`Eq`. Extracting lat/lon into a newtype
wouldn't change this (they'd still be `f64`). A
`LatLon` newtype might be worth introducing later if
call sites multiply; for now, one less type is worth
the minor derive inconsistency.
