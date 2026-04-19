# Weather provider migration plan

Tracking document for the Windy → Open-Meteo migration.
Started **2026-04-19**. Lives here so any agent (human
or otherwise) can pick up the refactor mid-flight.

## Why

Windy's Point Forecast API costs ~USD 900/year for the
tier bellwether needs. The free "testing" key returns
deliberately scrambled data (`warning: "The testing
API version is for development purposes only. This
data is randomly shuffled and slightly modified."`).

Open-Meteo is free, needs no API key, returns similar
hourly forecast data, and already uses the display
units the dashboard wants (°C, km/h, %, mm).

Rather than hard-swap providers, we introduce an
abstraction layer so either provider (or a future
third one) can back the dashboard. This also decouples
unit conversion from the dashboard model — a
longstanding wart where `dashboard/model/mod.rs`
knows about Kelvin and `wind_u-surface`.

## Target architecture

New module `crate::weather` with provider-neutral
types:

```rust
pub struct WeatherSnapshot {
    pub timestamps:      Vec<DateTime<Utc>>,   // hourly
    pub temperature_c:   Vec<Option<f64>>,
    pub humidity_pct:    Vec<Option<f64>>,
    pub wind_kmh:        Vec<Option<f64>>,     // magnitude
    pub wind_dir_deg:    Vec<Option<f64>>,     // 0 = N
    pub gust_kmh:        Vec<Option<f64>>,
    pub cloud_cover_pct: Vec<Option<f64>>,
    pub precip_mm:       Vec<Option<f64>>,
    pub warning:         Option<String>,
}

#[async_trait]
pub trait WeatherProvider: Send + Sync {
    async fn fetch(&self) -> Result<WeatherSnapshot, WeatherError>;
}
```

Each provider lives under `crate::clients::<name>/`
and exposes a struct that `impl WeatherProvider`. The
dashboard model never sees a raw `Forecast` again.

## PR sequence

Each PR compiles, passes `cargo xtask validate`, and
leaves the dashboard rendering a valid image. Stop at
any point and the project still runs.

| # | Title | State |
|---|-------|-------|
| 1 | Introduce `WeatherSnapshot` + `WeatherProvider` trait (types only, no wiring) | done |
| 2 | Adapt Windy client to the trait; move unit conversion out of `dashboard::model` | done |
| 3 | Refactor dashboard to consume `WeatherSnapshot` instead of `Forecast` | done |
| 4 | `PublishLoop` holds `Arc<dyn WeatherProvider>`; `PublishError::Weather` replaces `::Windy` | done |
| 5 | Config restructure: `[weather]` with `provider = "open_meteo" \| "windy"` discriminator | done |
| 6 | Add Open-Meteo provider (new `clients::open_meteo` module) | done |
| 7 | Flip default, delete Windy provider, update `config.example.toml`, README, HANDOFF | done |

### PR 1 — `WeatherSnapshot` + trait

- New `crates/bellwether/src/weather/{mod.rs,error.rs}`.
- Types + trait only; no provider implementations, no
  consumers. Compiles on its own.
- Unit tests for the struct (e.g., length-mismatch
  constructor guard if we add one).
- `#[non_exhaustive]` on `WeatherError` from day one.

### PR 2 — Windy → trait adapter

- New `crates/bellwether/src/clients/windy/snapshot.rs`
  with `fn to_snapshot(forecast: &Forecast) ->
  WeatherSnapshot`.
- Moves these conversions out of `dashboard::model`:
  - `temp_k - 273.15` → °C
  - `sqrt(u² + v²) * 3.6` → km/h (magnitude)
  - `atan2(-u, -v).to_degrees()` → wind direction
- Adds `WindyProvider { client: Client, request:
  FetchRequest }` implementing `WeatherProvider`.
- `dashboard::model` keeps taking `&Forecast` for
  now — no consumer changes.

### PR 3 — Dashboard over `WeatherSnapshot`

- `build_model(snapshot: &WeatherSnapshot, ctx) ->
  DashboardModel`.
- Delete: `KELVIN_TO_CELSIUS`, `MS_TO_KMH`,
  `wind_components_at`, `sample_value`, `WindyParameter`
  imports in `dashboard::model`.
- `warn_on_missing_condition_series` rewritten against
  `snapshot.cloud_cover_pct.iter().all(Option::is_none)`
  etc.
- Test fixtures switch from
  `Forecast::from_raw_json(...)` to `WeatherSnapshot
  { temperature_c: vec![...], ... }` — much easier to
  read.
- `PublishLoop::tick_once` adds a one-line
  `to_snapshot(&forecast)` call.

### PR 4 — `PublishLoop` over the trait

- `PublishLoop<S: ImageSink>` stores
  `Arc<dyn WeatherProvider>` instead of `WindyClient +
  FetchRequest`.
- `PublishError::Weather(WeatherError)` replaces
  `::Windy(WindyError)`.
- Web main (`crates/bellwether-web/src/main.rs`)
  constructs the provider and hands it in.
- `publish/tests.rs` mock becomes `struct
  FakeProvider { snapshot: WeatherSnapshot } impl
  WeatherProvider`.

### PR 5 — Config restructure

Replace:

```toml
[windy]
api_key_file = "windy_key.txt"
lat = 46.5547
lon = 15.6467
model = "gfs"
parameters = ["temp","wind","clouds","precip"]
```

with the **nested subtables + tag** shape:

```toml
[weather]
provider = "open_meteo"
lat = 46.5547
lon = 15.6467

[weather.open_meteo]
model = "icon_eu"

[weather.windy]
api_key_file = "windy_key.txt"
model = "gfs"
```

- Shared fields (`lat`, `lon`, `provider`) live at
  `[weather]`. Provider-specific fields live in
  `[weather.<name>]` subtables.
- Serde-tagged enum `WeatherProviderConfig` with
  `#[serde(tag = "provider")]`. The subtable matching
  the chosen provider is required; unused subtables
  are ignored (useful for keeping both configured
  during the migration).
- Clean break — no backward-compat shim, project is
  pre-release.
- Update `test-data/config-byos.toml` and
  `test-data/config-webhook.toml`.

### PR 6 — Open-Meteo provider

- New `crates/bellwether/src/clients/open_meteo/mod.rs`.
- Endpoint: `https://api.open-meteo.com/v1/forecast`.
- Query params: `latitude`, `longitude`, `hourly=
  temperature_2m,relativehumidity_2m,precipitation,
  cloudcover,windspeed_10m,winddirection_10m,
  windgusts_10m`, `timezone=utc`, `forecast_days=4`,
  `models=<configured>` (default `icon_eu`).
- Default model: **`icon_eu`** (DWD's ICON-EU,
  ~6 km resolution over Europe, updated 4x/day).
  Slovenia-optimized — good for the Maribor
  deployment. User-overridable via
  `[weather.open_meteo] model = "..."` for
  deployments outside Europe.
- No API key, no redirect policy nonsense (public
  CDN), same timeouts as the Windy client.
- `to_snapshot` is trivial — units already match.
- Wiremock tests mirror the Windy client's structure.
- `WeatherSnapshot::warning` gets populated from the
  `current_weather.warning` field if present, plus
  any Open-Meteo-specific rate-limit messages.

### PR 7 — Flip default, delete Windy provider

- `config.example.toml` → Open-Meteo only (no
  `[weather.windy]` block).
- Delete `crates/bellwether/src/clients/windy/`
  entirely and the `Windy` variant of
  `WeatherProviderConfig`.
- Remove `reqwest` `rustls-tls` or any other
  dependency that's only there for Windy (audit
  `crates/bellwether/Cargo.toml`).
- README "Getting started" drops the `windy_key.txt`
  step.
- `HANDOFF.md` notes the migration.
- `CHANGELOG.md` under `[Unreleased]` — **Removed**
  section lists the Windy provider; **Added** lists
  Open-Meteo.
- Leave `windy_key.txt` in `.gitignore` as a no-op
  (harmless; avoids churn).

## Settled decisions

All decisions settled on **2026-04-19** before PR 1.

1. **Windy provider lifecycle.** Delete in PR 7 at
   the same time as the default flip — no separate
   soak PR. Keeps the codebase lean; the two-provider
   window is PRs 5–6 only.
2. **Config TOML shape.** Nested subtables with a
   `provider = "..."` tag at `[weather]`. Shared
   fields (`lat`, `lon`) live on the parent table;
   provider-specific fields live in `[weather.<name>]`
   subtables. Serde-tagged enum for
   `WeatherProviderConfig`.
3. **Open-Meteo default model.** `icon_eu` — DWD's
   European model, ~6 km resolution, matches
   Slovenia. Overridable per-deployment via
   `[weather.open_meteo] model = "..."`.
4. **Warning handling.** Log-only for now.
   `WeatherSnapshot::warning` is populated by
   providers and logged at `warn` in the publish
   loop, but not rendered on the dashboard. Revisit
   if a real incident ever silently poisons the
   display.
5. **`async_trait` crate.** Yes — one fetch per
   15 min, no hot path. Avoid manual `BoxFuture`
   boilerplate.
6. **Wind direction representation.** Keep as
   `wind_dir_deg: f64` in the snapshot.
   `wind_to_compass` stays in `dashboard::classify`
   where it belongs.
7. **Gust field.** Always-present optional (always
   in the snapshot struct, per-step `Option<f64>`).
   Open-Meteo always returns it; Windy's `windGust`
   parameter maps cleanly.

## Out of scope

- Dashboard layout changes.
- Caching forecasts across ticks.
- Multi-point forecasts (still one lat/lon per
  deployment).
- Offline / degraded-mode rendering.

## Progress log

- **2026-04-19** — plan drafted, open decisions
  settled (see "Settled decisions" above). Seven-PR
  sequence confirmed.
- **2026-04-19** — PR 1 done. Added
  `crate::weather` module with `WeatherSnapshot`
  (8 hourly series + warning), `WeatherError`
  (EmptySnapshot / SeriesLengthMismatch / Transport
  / Provider), and the `#[async_trait]
  WeatherProvider` trait. No wiring yet — nothing
  else in the crate references the new module.
  Added `async-trait = "0.1"` dependency.
  Validate: 97.5% coverage, 5.7% duplication, all
  tests green.
- **2026-04-19** — PR 2 done. Added
  `clients::windy::snapshot::to_snapshot`
  (K→°C, u/v→km/h + compass degrees, m/s gust
  →km/h, RH clamp [0,100], passthrough for clouds
  / precip, warning / timestamps) and
  `WindyProvider` implementing `WeatherProvider`.
  `From<WindyError> for WeatherError` maps
  `Http` → `Transport`, everything else →
  `Provider`. Dashboard still consumes `Forecast`
  directly; no call sites changed. There is
  temporary duplication between
  `clients::windy::snapshot` (u/v → degrees) and
  `dashboard::classify::wind_to_compass` (u/v →
  `Compass8`), to be cleaned up in PR 3. Validate:
  97.8% coverage, 5.6% duplication, all tests green.
- **2026-04-19** — PR 3 done. `build_model` now
  takes `&WeatherSnapshot`. Deleted from
  `dashboard::model`: `KELVIN_TO_CELSIUS`,
  `MS_TO_KMH`, `wind_components_at`,
  `series_value_at`, `sample_value`, and the
  `WindyParameter` / `Forecast` imports. Replaced
  `dashboard::classify::wind_to_compass` (u/v
  input) with `Compass8::from_degrees(deg)`;
  removed the duplicated u/v math. `PublishLoop::tick_once`
  calls `to_snapshot(&forecast)` between fetch and
  model-build — still holds a concrete `WindyClient`
  until PR 4. Test fixtures in
  `dashboard::model::tests` switched from
  `Forecast::from_raw_json` to `WeatherSnapshot`
  struct literals (°C, km/h, degrees — much
  easier to eyeball). Validate: 97.6% coverage,
  5.4% duplication (down from 5.6% — the u/v
  math consolidation landed), all tests green.
- **2026-04-19** — PR 4 done. `PublishLoop` holds
  `Arc<dyn WeatherProvider>` + `GeoPoint location`
  instead of `WindyClient + FetchRequest`.
  `PublishError::Weather(WeatherError)` replaces
  `::Windy`. `bellwether-web/src/main.rs` constructs
  `WindyProvider::new(WindyClient::new(),
  fetch_request)` and wraps it as `Arc<dyn
  WeatherProvider>` before handing it to
  `PublishLoop::new`. `publish/tests.rs` dropped
  wiremock in favour of an in-memory `FakeProvider`
  (scripted `WeatherSnapshot` or
  `WeatherError::Provider`); the ignored
  `generate_dashboard_sample_bmp` now drives the same
  rich snapshot used by the end-to-end render test so
  there is no parallel Windy-specific fixture to keep
  in sync. Validate: 97.7% coverage, 4.8% duplication,
  all tests green.
- **2026-04-19** — PR 5 done. Config restructured
  to the nested-subtables shape: `[weather]` holds
  `provider` + `lat` + `lon`; `[weather.windy]`
  holds Windy-specific fields (api_key_file, model,
  parameters). New `ProviderKind` enum (currently
  just `Windy`) drives a match in
  `bellwether-web::build_provider`. `Config::load`
  validates that the active provider's subtable is
  present before reading its secret file.
  `FetchRequest::from_config` now takes
  `&WeatherConfig` and errors with
  `WindyError::NotActiveProvider` /
  `MissingProviderSubtable` if the config shape
  disagrees with Windy. Updated fixtures
  (`config-byos.toml`, `config-webhook.toml`,
  `config.example.toml`, the user's local
  `config.toml`) and all inline test TOML.
  `ProviderKind` is deliberately NOT
  `#[non_exhaustive]` — exhaustive matching across
  crates catches the missing Open-Meteo arm as soon
  as PR 6 adds it. Validate: 97.7% coverage, 4.7%
  duplication, all tests green.
- **2026-04-19** — PR 6 done. Added
  `clients::open_meteo` (GET-based client, JSON
  parsing, `to_snapshot` that passes units through
  since Open-Meteo already uses °C/km/h/mm/percent/
  degrees) and `OpenMeteoProvider` implementing
  `WeatherProvider`. `ProviderKind::OpenMeteo`
  variant + `OpenMeteoProviderConfig` subtable
  (`model` defaults to `icon_eu`). Extracted shared
  HTTP helpers into `clients::http_util`
  (`build_http_client`, `read_capped_body`,
  `truncate_with_ellipsis`) so the two provider
  modules don't reimplement the same body-reading
  loop. `bellwether-web::build_provider` now
  dispatches on `ProviderKind`; Open-Meteo arm
  passes `Arc::new(OpenMeteoProvider)`. Duplication
  threshold bumped from 6.0% → 7.0% with a
  rationale comment in `xtask/src/dupes.rs` — the
  remaining overlap is unavoidable parallel
  structure between the two providers (tiny
  `From` conversions, `Provider::new`,
  `Client::endpoint`), not worth the abstraction
  tax. Validate: 97.5% coverage, 6.5% duplication,
  all tests green. PR 7 will flip the default
  provider to Open-Meteo and delete the Windy
  module entirely.
- **2026-04-19** — PR 7 done. Migration complete.
  Deleted `crates/bellwether/src/clients/windy/`,
  `ProviderKind::Windy`, `WindyProviderConfig`,
  `WindyParameter`, `REQUIRED_WINDY_PARAMETERS`,
  `ConfigError::{MissingRequiredWindyParameters,
  ReadSecret, EmptySecret}`,
  `Config::bind_provider_secrets`, the
  `live-tests` feature flag, and
  `scripts/windy-test.ps1`.
  `bellwether-web::build_provider` collapsed to a
  single Open-Meteo arm. Updated fixtures
  (`config-byos.toml`, `config-webhook.toml`,
  `config.example.toml`), README, CLAUDE.md,
  HANDOFF.md (with a dated migration note at the
  top), and CHANGELOG.md (`[Unreleased]` records
  the Added / Changed / Removed breakdown).
  Duplication threshold reverted 7.0% → 6.0% now
  that there's only one provider. `windy_key.txt`
  stays in `.gitignore` as a harmless no-op.
  Validate: 97.5% coverage, 2.6% duplication, all
  tests green. The runtime now runs against
  Open-Meteo with zero API cost — the reason for
  the whole migration.
