# Artisan Findings -- Resolved

Archive of fixed Artisan code quality findings, newest
first. See [artisan-log.md](artisan-log.md) for open
findings.

---

## 2026-04-19 (feat — v0.13.0 RPi deployment + `/api/setup`)

### AQ-107 — `/api/setup` returned bare `Response` instead of typed `Result<Json<_>, StatusCode>`
**Category:** API design
**Description:** `setup()` returned an opaque `Response`, hiding the JSON shape and 404 error path inside `.into_response()` calls, while the sibling `display()` returned `Result<Json<DisplayResponse>, StatusCode>`.
**Fix:** Changed signature to match `display()` — `Result<Json<SetupResponse>, StatusCode>` with `?` on the MAC header extraction. `crates/bellwether-web/src/api/trmnl/handlers.rs`.

### AQ-108 — Advertised `image_url` pointed at a filename with no bytes behind it
**Category:** Type safety / correctness
**Description:** When the image store was empty, `setup` advertised `image_url=".../empty_state"` but `serve_image` would 404 on that filename — the device would poll and get nothing.
**Fix:** Now returns 503 when no image is available, matching `/api/display`'s pattern. Removed the unused `SETUP_PLACEHOLDER_FILENAME` constant. `crates/bellwether-web/src/api/trmnl/handlers.rs`.

### AQ-109 — `derive_friendly_id` was a free function returning `String`
**Category:** Type safety
**Description:** The "6-char uppercase-hex device id" format invariant lived in a free function name rather than a type. `SetupResponse.friendly_id: String` could be confused with any other string field.
**Fix:** Introduced `FriendlyId` newtype with `from_mac(&str) -> Self` and `Display`, mirroring the module's existing `RefreshInterval` pattern. `SetupResponse.friendly_id: FriendlyId` now self-documents. `crates/bellwether-web/src/api/trmnl/handlers.rs`.

### AQ-110 — `trmnl/mod.rs` exceeded 500-line threshold
**Category:** Module size
**Description:** The module had grown to 618 lines bundling validation, store, state, five handlers, middleware, response types, and router composition.
**Fix:** Split handlers into sibling `handlers.rs` (260 lines: response types, `FriendlyId`, `display`/`setup`/`log`/`serve_image`, auth middleware). `mod.rs` is now 383 lines (state + store + router). Added `TrmnlState::access_token()` accessor so handlers don't need field-level access. `crates/bellwether-web/src/api/trmnl/{mod,handlers}.rs`.

### AQ-111 — `SetupResponse.status: u16` stringly-typed
**Category:** Type safety
**Description:** The response body carries its own `status` field that the firmware inspects, constructed with a bare literal `200`, leaving room for drift between HTTP status and body status.
**Fix:** Accepted that both `DisplayResponse` and `SetupResponse` share this pattern (mirror of the TRMNL firmware contract). The literal is colocated with the one `Json(...)` construction site, which is minimum-viable. Noted here rather than opening a fresh log entry; a bigger "split status out of response bodies" refactor would span both types.

### AQ-112 — xtask deploy modules flattened `RemoteError` source chains
**Category:** Error handling
**Description:** `deploy_remote::RemoteError` carries a proper `source()` chain, but every caller in `deploy.rs` / `deploy_setup.rs` discarded it with `.map_err(|e| e.to_string())?` at 12 call sites — typed error was effectively write-only.
**Fix:** Added `anyhow = "1"` to xtask. Converted `deploy::deploy`, `deploy_setup::deploy_setup`, and `deploy_config::load` to `anyhow::Result<()>`. Replaced `.map_err(|e| e.to_string())` with `.context(...)` so `RemoteError`'s source chain is preserved and rendered via `{e:#}` in `main.rs`. Internal `validate()` stays `Result<_, String>` so unit tests' `.contains()` assertions don't need rewriting.

### AQ-113 — Dead `.parent()` checks on paths built from `project_root.join()`
**Category:** API design / dead code
**Description:** `deploy_setup::copy_config` and `install_service` called `.parent().ok_or_else(|| format!(...))` on paths that were always constructed as `project_root.join(...)`, so the error branches were unreachable.
**Fix:** Pass `project_root` / `deploy_dir` directly as the `scp` cwd. Dropped the `parent()` computations. Changed `install_service` to accept a `deploy_dir: &Path` parameter instead of deriving it. `xtask/src/deploy_setup.rs`.

### AQ-114 — `parse_port` returned `Option<String>` where `Option<u16>` fit
**Category:** Type safety
**Description:** `Option<String>` allowed impossible values (e.g. `"999999"`) to flow into the printed URL without any warning.
**Fix:** `parse_port(&str) -> Option<u16>` via `.parse::<u16>().ok()`. Added `parse_port_rejects_out_of_range` test. Caller uses `map_or_else` to format. `xtask/src/deploy_setup.rs`.

## 2026-04-19 (feat — v0.12.0 Windy → Open-Meteo migration)

### AQ-096 — Provider-error types carried config-shape errors
**Category:** Abstraction boundary
**Description:** `OpenMeteoError::NotActiveProvider` and `MissingProviderSubtable` described config-shape issues that duplicate `ConfigError::MissingProviderSubtable`. `FetchRequest::from_config` re-validated an invariant `Config::load` already enforced.
**Fix:** Dropped `from_config`; added `FetchRequest::from_parts(lat, lon, &OpenMeteoProviderConfig)` as the only construction path. Binary crate pulls the validated subtable from `WeatherConfig` and passes the pieces through. `OpenMeteoError` loses both config-shape variants. `clients/open_meteo/mod.rs`, `bellwether-web/src/main.rs`.

### AQ-097 — `ReadBodyError::NotUtf8(String)` discarded the underlying error
**Category:** Error handling
**Description:** The variant held a preformatted string; the real `FromUtf8Error` (byte offset, invalid-byte diagnostic) was thrown away at conversion.
**Fix:** Changed variant to `NotUtf8(#[from] FromUtf8Error)`. The `source()` chain now preserves the underlying error; providers forward it through `#[from]`. `clients/http_util.rs`.

### AQ-098 — `ReadBodyError` was not a `std::error::Error`
**Category:** Error handling
**Description:** Type derived only `Debug` — no `Display`, no `thiserror::Error`, no `source()` chain. Required hand-conversion everywhere.
**Fix:** Added `thiserror::Error` derive with `#[error(...)]` per variant and `#[from] reqwest::Error` / `FromUtf8Error` for automatic conversion. `clients/http_util.rs`.

### AQ-099 — `OpenMeteoError::Parse(String)` flattened structured errors
**Category:** Error handling
**Description:** Both `serde_json::Error` and `FromUtf8Error` were stringified at the first hop, destroying `source()` and programmatic distinction.
**Fix:** Split into `Json(#[from] serde_json::Error)` and `Utf8(#[from] FromUtf8Error)` variants. Invariant bugs in the parser now surface as `Invariant(WeatherError)` rather than losing type. `clients/open_meteo/mod.rs`.

### AQ-100 — Error message prefix stacking was noisy
**Category:** Error handling
**Description:** `WeatherError::Transport("weather transport error: {0}")` + `PublishError::Weather("fetching weather forecast: {0}")` + provider prefix produced logs like `fetching weather forecast: weather transport error: Open-Meteo HTTP error: ...`.
**Fix:** Changed `WeatherError::Transport` / `::Provider` messages to `"{0}"` — pass through to the inner error. The outer `PublishError::Weather` supplies the "fetching weather forecast" framing once. `weather/error.rs`.

### AQ-101 — `WeatherSnapshot::validate` was not enforced at construction
**Category:** Type safety
**Description:** Public fields + `Default` meant any provider could build a `WeatherSnapshot` with mismatched series lengths and skip `validate()`, silently corrupting dashboard output.
**Fix:** Made fields private. Added `WeatherSnapshotBuilder` (public fields for ergonomic construction) with a `build() -> Result<WeatherSnapshot, WeatherError>` that enforces invariants. Accessors (`temperature_c()`, `wind_kmh()`, etc.) return slices. Illegal snapshots are now unrepresentable. `weather/mod.rs`.

### AQ-102 — `PublishLoop` held `GeoPoint` separately from the provider
**Category:** Type safety / invariants
**Description:** Two sources of truth for the same location — provider fetched for lat/lon X, dashboard computed sunrise/sunset for Y. Enforcement lived in a doc comment.
**Fix:** Added `fn location(&self) -> GeoPoint` to the `WeatherProvider` trait; `PublishLoop::new` dropped the `location` parameter and reads from the provider. Single source of truth. `weather/mod.rs`, `publish/mod.rs`, `clients/open_meteo/mod.rs`.

### AQ-103 — `FetchRequest::from_config` pattern would duplicate per provider
**Category:** API design
**Description:** Each new provider would need its own `from_config` + redundant `NotActiveProvider` / `MissingProviderSubtable` variants.
**Fix:** Delegated provider selection to `bellwether-web::build_provider` (single match on `ProviderKind`). Per-provider constructors take validated pieces via `from_parts`. Paired with AQ-096.

### AQ-104 — `Config::from_toml_str` and `Config::load` duplicated parse logic
**Category:** API design
**Description:** Minor — both methods carried the same parse + validate sequence with path-present / path-absent variants.
**Fix:** Extracted `parse_and_validate(toml_text, Option<&Path>) -> Result<Config, ConfigError>` helper. Both public methods become thin wrappers. `config/mod.rs`.

### AQ-105 — `OpenMeteoError::Http(Box<reqwest::Error>)` leaked a dependency type
**Category:** Abstraction boundary
**Description:** `reqwest::Error` as a public enum variant inner type forces downstream matchers to depend on the exact `reqwest` version.
**Fix:** Changed to `Http(Box<dyn StdError + Send + Sync>)`. Public surface is now stable against reqwest upgrades; `From<reqwest::Error>` still boxes automatically. `clients/open_meteo/mod.rs`.

### AQ-106 — `dashboard/model/mod.rs` was 536 lines with 5 public structs
**Category:** Module size
**Description:** Mixed presentation-data types, the public builder, and ~10 private helpers in one file.
**Fix:** Split into `types.rs` (structs + shape constants) and `build.rs` (`build_model` + helpers). `mod.rs` is a 32-line re-exporter. `dashboard/model/{mod.rs,types.rs,build.rs}`.

---

## 2026-04-18 (feat — v0.11.0 dashboard SVG rewrite)

### AQ-091 — `battery_indicator` hand-rolled `<text>` instead of using the shared `text()` helper
**Category:** Abstraction boundary
**Description:** The new `text()` helper was introduced specifically to consolidate every `<text>`-opening-tag boilerplate into one place — but `battery_indicator` kept its own inline `format!("<text x= y= font-size= text-anchor= fill=>… </text>")` arms, ~18 lines of duplication against the freshly-introduced invariant.
**Resolution:** Replaced the inline arms with a single `text()` call, computing the label content into a `String` first so both branches share the positional arguments.

### AQ-092 — Duplicated 3-column grid centre array
**Category:** Code quality
**Description:** `[133_u32, 400_u32, 667_u32]` appeared as a local in both `meteo_band` and `forecast_band`. Drift between the two would silently mis-align the cells vertically.
**Resolution:** Hoisted to a module const `TRIPLE_COLUMN_CENTRES`. Both bands reference it directly; any future geometry change updates one place.

### AQ-093 — `day_extreme_celsius` took a function pointer with a correlated `f64` identity
**Category:** Type safety
**Description:** The shared min/max helper took
`fn(f64, f64) -> f64` for the reducer and a separate
`f64` for the fold identity. Nothing prevented
`day_extreme_celsius(indices, temps, f64::INFINITY,
f64::max)` — a silently-wrong fold returning
`INFINITY`.
**Resolution:** Introduced a private `Extreme { High,
Low }` enum with `identity(self) -> f64` and
`reduce(self, acc, x) -> f64` methods. The two
values are coupled at the type level, and
`day_high_celsius`/`day_low_celsius` each pass a
single variant.

### AQ-094 — `battery_pct: u8` accepted 101..=255
**Category:** API design / documentation
**Description:** `battery_indicator` accepted any
`u8` without a type-level invariant that it sat in
`0..=100`. The silent `pct.min(100)` clamp hid a
hypothetical broken upstream.
**Resolution:** Added `debug_assert!(pct <= 100)` in
`battery_fill_rect` so a broken path surfaces in
tests, plus an explicit comment that upstream
`battery_voltage_to_pct` guarantees the range. A
full `Percent(u8)` newtype would be more churn than
the one-call-site invariant warrants; flagged for
revisit if telemetry grows additional percent-typed
fields.

### AQ-095 — Fragile `x="150"` locator in current-panel-collapse test
**Category:** Test quality
**Description:** The test for "current panel
collapses to placeholder" asserted
`svg.contains("x=\"150\"")`. It passes
coincidentally today (that x is unique in the
current layout), but any future element that drifts
to x=150 would make the test a false-positive.
**Resolution:** Rewrote the assertion to look for
the literal "No current reading" string that the
new placeholder emits; the locator is now the
intent rather than a byte-offset coincidence.

## 2026-04-17 (feat — v0.10.0 dashboard data-model groundwork)

### AQ-086 — `model.rs` grew to 912 lines and held 5 pub structs
**Category:** Module size
**Description:** After adding `TodaySummary`,
`ModelContext`, extended `CurrentConditions`, and
`DaySummary.low_c`, `dashboard/model.rs` crossed
the 500-line CLAUDE.md threshold. Tests alone
contributed ~420 lines to the file.
**Resolution:** Converted `model.rs` to
`model/mod.rs` (497 lines — production code + doc
comments) and extracted the test module to
`model/tests.rs` (417 lines). Matches the existing
`clients/windy` split convention.

### AQ-087 — `dashboard::model` depended on `publish`
**Category:** Abstraction boundaries
**Description:** `model.rs` imported
`DeviceTelemetry` and `battery_voltage_to_pct` from
`crate::publish`. But `publish::tick_once` imports
`dashboard::build_model`. The mutual recursion between
`dashboard` and `publish` left readers with a "where
does telemetry belong?" question at every touch.
**Resolution:** Created a neutral
`crate::telemetry` module and moved both
`DeviceTelemetry` and `battery_voltage_to_pct`
there. `publish` re-exports them for backwards
compatibility of the import path
(`bellwether::publish::DeviceTelemetry` still works)
while `dashboard::model` imports from
`crate::telemetry` directly. The mutual-recursion
is gone.

### AQ-088 — `lat: f64, lon: f64` pair was a swap hazard
**Category:** Type safety
**Description:** `ModelContext` and
`sunrise_sunset` both took two bare `f64`s for
geographic coordinates. Swapping them at a call
site compiles cleanly and silently returns wrong
times; the sign convention (positive N/E) was
documented only in doc comments.
**Resolution:** Introduced `GeoPoint { lat_deg,
lon_deg }` in `dashboard::astro`. `ModelContext`
stores a `location: GeoPoint`;
`sunrise_sunset(date, location, tz)` takes it. The
swap hazard is now a compile error rather than a
silent wrong-time.

### AQ-089 — `build_model` took `&ModelContext` despite the Copy contract
**Category:** API design
**Description:** `ModelContext` is explicitly
`Copy` (documented as "call sites can pass by
value"), but `build_model` took `&ModelContext`.
Every call site had to write `&ctx`; every internal
access threaded through an extra reference.
**Resolution:** Changed the signature to
`build_model(forecast: &Forecast, ctx: ModelContext)`.
Call sites and internal `build_today` follow suit.
Matches the documented intent and makes the struct's
`Copy` derivation load-bearing instead of
decorative.

### AQ-090 — `TrmnlState` used `Mutex` for telemetry while `ImageStore` used `RwLock`
**Category:** Code quality (consistency)
**Description:** `TrmnlState.images` is an
`Arc<ImageStore>` backed by `RwLock`. The new
`telemetry` field introduced an
`Arc<Mutex<DeviceTelemetry>>` — a different
primitive for a similarly-shaped cache. Not
functional, just a consistency wrinkle.
**Resolution:** Changed to
`Arc<RwLock<DeviceTelemetry>>`. Reads grab a
`.read()` snapshot (cheap since `DeviceTelemetry`
is `Copy`); writes take `.write()` and call
`merge_from`.

## 2026-04-17 (feat — swap dashboard font to Atkinson Hyperlegible)

### AQ-084 — New font-family test duplicated existing coverage
**Category:** Type safety / test quality
**Description:**
`font_family_matches_bundled_font_registration_name`
asserted that the SVG contained a hard-coded
`font-family="Atkinson Hyperlegible"` literal — the
exact same check
`font_family_set_at_svg_root` was already making.
The docstring promised to catch a mismatch between
the SVG `font-family` and the font's actual fontdb
registration name, but neither test read the TTF's
`name` table: a typo like `Atkinson-Hyperlegible`
(or a font swap that changed the family name)
would have silently dropped every glyph through
usvg's fallback behaviour.
**Resolution:** Upgraded the test to a true
registration-name check — reads the `FAMILY` record
out of `ATKINSON_HYPERLEGIBLE_TTF`'s `name` table
via `ttf-parser`, formats the expected
`font-family="..."` string, then asserts the SVG
contains it. Renamed to
`font_family_in_svg_matches_ttf_name_table_family`.

### AQ-085 — Font sizes reverted to free-floating magic numbers
**Category:** Code quality
**Description:** Removing the m6x11plus-era "font
sizes must be multiples of 18" test eliminated the
only structural guard on the size literals (180,
54, 36, 36, 72). A typo turning `72` into `27`
would now pass CI with no visible failure.
**Resolution:** Hoisted the five sizes into named
constants at the top of `dashboard/svg.rs`:
`CURRENT_TEMP_PX`, `CONDITION_LABEL_PX`,
`WIND_LABEL_PX`, `DAY_LABEL_PX`, `DAY_HIGH_PX`.
Each `format!`-ed `font-size` attribute now
references the constant, so the visual hierarchy
lives in one place and a typo shows up as a Rust
compile error rather than at eyeball time.

## 2026-04-17 (chore — `cargo xtask test --ignored`)

### AQ-083 — `test_cmd::test` grew to three positional args
**Category:** API design
**Description:** Adding `ignored: bool` alongside the
existing `verbose: bool` made `test(filter, verbose,
ignored)` a three-arg signature where two are bools.
Readable at first, but bug-inviting when a future
fourth flag (e.g. `include_ignored`, `test_threads`)
lands — the call site would have to count positions.
**Resolution:** Introduced a `TestOptions<'a> {
filter, verbose, ignored }` struct. `test` now takes
it by value. `main.rs` destructures `XCommand::Test`
into the struct at the call site, so every field is
named at both the producer and consumer. `test_check`
(internal to validate) keeps its `filter`-only
signature because it never exposes the other flags.

## 2026-04-17 (feat — dashboard module with current + 3-day forecast layout)

### AQ-078 — `build_svg` took `&RenderConfig` for "forward compatibility" but read none of it
**Category:** API design / YAGNI
**Description:** `build_svg(model, _cfg: &RenderConfig) -> String`
had an `#[allow(clippy::needless_pass_by_value)]`
masking that `_cfg` was completely unused — the
clippy-allow annotation was the smell. Callers had no
way to tell what the function actually read.
**Resolution:** Dropped the parameter. Signature is
now `build_svg(&DashboardModel) -> String`. If a
future layout variant needs render config it can be
re-added then.

### AQ-079 — `DaySummary::high_c: i32` conflated "0 °C" with "no data"
**Category:** Type safety
**Description:** `high_c` stored the magic value `0`
when the day's temperature series was all-null,
indistinguishable in the SVG from a real 0 °C day.
**Resolution:** Changed to `Option<i32>`; SVG renders
em-dash for `None`. Joint resolution with RT-067.

### AQ-080 — `build_model` took `&RenderConfig` but used only `cfg.timezone`
**Category:** API design
**Description:** The signature advertised a dependency
on the whole render config that wasn't real. Callers
couldn't tell `build_model` didn't care about
width/height/bit_depth.
**Resolution:** Changed to
`build_model(&Forecast, Tz, DateTime<Utc>) -> DashboardModel`.
Publish loop call site extracts `self.render_cfg.timezone`
at the call.

### AQ-081 — `DaySummary::label` was stringly-typed
**Category:** Type safety
**Description:** `label: String` stored a
pre-rendered 3-char weekday string. Tests had to
assert on exact strings ("Sat", "Sun", …), and a
locale-slip in `format!("{}", date.weekday())` would
have compiled silently. The "labels are always
English" invariant was implicit in whichever call
site formatted first.
**Resolution:** Field is now `weekday: chrono::Weekday`.
The SVG builder has a private `weekday_label(Weekday)
-> &'static str` table that is the single source of
truth for the English abbreviations. A new test
`weekday_label_is_three_char_english` locks it.

### AQ-082 — `first_value` and `first_wind_components` duplicated option-chain shape
**Category:** Code quality
**Description:** Both helpers did "get series, copy,
flatten, get first" with slightly different accessor
paths, fuzzing the reader's mental model of how the
`Forecast` shape is consulted.
**Resolution:** Extracted `series_value_at(map, key,
idx) -> Option<f64>` as the single option-chain site.
`sample_value` wraps it with the
`{wire_name}-surface` convention used by most series;
`wind_components_at` calls it directly for the
`wind_u`/`wind_v` keys.

## 2026-04-17 (feat — bundle m6x11plus font + `Renderer::with_default_fonts`)

### AQ-075 — `to_vec()` copy in `with_default_fonts` begged for an explanation
**Category:** API design / readability
**Description:** The new constructor did
`renderer.load_font_data(M6X11_TTF.to_vec())`
— a 18 KiB copy of a compile-time `&'static [u8]`.
Technically unavoidable (fontdb's API takes owned
`Vec<u8>`, see AQ-034), but without an inline
comment future readers could mimic the pattern in
hot paths thinking the copy is cheap.
**Resolution:** Added a four-line comment at the
call site naming the constraint, citing AQ-034, and
referring callers to the "construct once per
process" guidance on `Renderer`. Joint resolution
with RT-062.

### AQ-076 — Test reached into `Renderer`'s private `options.fontdb`
**Category:** Abstraction boundary
**Description:** `with_default_fonts_registers_m6x11_face`
asserted `r.options.fontdb.len() == 1` in addition
to the `Debug`-impl string check. The next-line
`"font_count: 1"` check already covers the public
contract; the field-access made the test fragile
against harmless internal renames.
**Resolution:** Dropped the private-field assertion.
Test now relies solely on the `Debug`-impl string,
which is the public contract.

### AQ-077 — Single-letter binding `r` in public constructor body
**Category:** Readability
**Description:** `with_default_fonts` used
`let mut r = Self::new(); … r`. The rest of the
crate uses descriptive names (`renderer`,
`pixmap`, `tree`); terse one-letter names read as
draft code in a method that's directly reachable
from the crate's public API docs.
**Resolution:** Renamed to `renderer` throughout
the function body. Test that similarly used `r`
renamed to match.

## 2026-04-17 (chore — port 3100 + config.example + HANDOFF rewrite)

### AQ-070 — HANDOFF.md duplicated CLAUDE.md content
**Category:** Docs size / drift risk
**Description:** Rewrite grew HANDOFF to 206 lines
with a "User working style" section restating rules
from CLAUDE.md and a "Memory (local only)" section
restating what's in the memory store itself.
**Resolution:** User-style section trimmed to 2
guardrails that aren't in CLAUDE.md yet (the `/tmp`
rule, the six-field finding format). Memory section
shortened to one sentence. HANDOFF now 174 lines.

### AQ-071 — Same as RT-057 (example quota mismatch)
**Resolution:** See RT-057.

### AQ-072 — Em-dash in `config.example.toml`
**Category:** Encoding consistency
**Description:** The example file was the only
TOML/tooling-facing file in the repo with non-ASCII
(`—`). Windows PowerShell with the default code page
can mojibake UTF-8 TOML piped through unaware tools.
**Resolution:** Replaced em-dash with `:`. All
tooling-facing config files now ASCII-only.

### AQ-073 — HANDOFF "Memory" section baked in machine-specific path
**Category:** Docs accuracy
**Description:** Referenced
`~/.claude/projects/D--src-trmnl-bellwether/memory/`
— the `D--src-...` slug is specific to this user's
Windows checkout. A fresh agent on Linux at
`~/code/bellwether` has a different slug.
**Resolution:** Section rewritten to a single
sentence that doesn't quote a path and just says
"local memory may add context on the operator's
machine; everywhere else this file + CLAUDE.md are
the whole picture."

### AQ-074 — README log-line recipe brittle under JSON formatter
**Category:** Docs accuracy
**Description:** README quoted `published image
filename="dash-NNNNNNNN.bmp"` as the log grep target.
With a JSON `tracing_subscriber` layer (common on
systemd deploys), the line becomes structured JSON
and the literal quote won't match.
**Resolution:** Softened to "watch for a `published
image` entry in the log" — matches both text and JSON
layers via substring.

---

## 2026-04-17 (PR 3c — v0.6.0 fetch/render/publish loop)

### AQ-062 — `tick` private forces real-time test
**Category:** Test hygiene / API
**Resolution:** Renamed `tick` to `tick_once` and
made it `pub`. Tests now drive the loop
deterministically (no `tokio::time::sleep` timing
dependencies for most assertions). Doc calls out the
future `/api/refresh` use case.

### AQ-063 — `PublishError::Sink(String)` threw away structure
**Category:** Error design
**Resolution:** `ImageSink::publish_image` returns
`Result<(), SinkError>` where
`SinkError = Box<dyn std::error::Error + Send + Sync>`.
`PublishError::Sink` wraps the same via `#[source]`,
so callers get the full error chain via
`std::error::Error::source`.

### AQ-064 — Doc comment referenced "PR 3c"
**Category:** Doc rot
**Resolution:** Rewritten to say "temporary
placeholder until a real layout lands — tracked in
TODO.md". Ages well.

### AQ-065 — `ImageSink` trait placement
**Category:** Abstraction
**Resolution:** Left in `publish` for now. Module
doc explains: "Once a second producer shows up
(e.g., an on-demand /api/refresh handler or a CLI
'render once to disk' command), promote it to its
own crate::sink module." Pre-empts the future
refactor surprise.

### AQ-066 — `Startup.config` carried whole Config
**Category:** Coupling
**Resolution:** Replaced with
`Startup.windy: Option<WindyConfig>` — the exact
slice `spawn_publish_loop` needs. Explicit in
`main.rs`'s intermediate `let Config { windy, .. } =
cfg` destructuring.

### AQ-067 — same as RT-053

### AQ-068 — Test name / assertion mismatch
**Category:** Test hygiene
**Resolution:** Replaced the old real-time
`run_publishes_on_interval_and_swallows_errors` test
with a deterministic
`run_recovers_after_transient_sink_error` that
scripts the MockSink to return Err on call #1, Ok
afterward. Asserts at least 2 calls happen — proves
the loop genuinely swallows errors rather than just
inferring from "didn't panic".

### AQ-069 — `ForecastFixture` duplicated parse logic
**Category:** Test hygiene
**Resolution:** Added `Forecast::from_raw_json(json:
&str) -> Result<Forecast, WindyError>` in the Windy
module. Tests now build `Forecast` instances via the
same parser the live client uses, so fixtures stay
in sync with the wire format automatically.
`ForecastFixture` helper deleted.

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
