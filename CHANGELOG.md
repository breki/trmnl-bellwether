# Changelog

All notable changes to this project will be documented
in this file.

The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
