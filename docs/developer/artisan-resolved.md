# Artisan Findings -- Resolved

Archive of fixed Artisan code quality findings, newest
first. See [artisan-log.md](artisan-log.md) for open
findings.

---

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
