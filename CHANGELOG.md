# Changelog

All notable changes to this project will be documented
in this file.

The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
