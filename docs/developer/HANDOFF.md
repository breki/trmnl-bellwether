# Handoff to the next agent

Current as of 2026-04-17, version **0.6.0**. You are
the next agent — read this once end-to-end before
touching anything. Previous handoff was written by the
scaffolding agent and is now superseded; the scaffold
is long done.

## What's built

Six feature commits after the scaffold:

| Version | PR | What |
|---------|----|------|
| 0.2.0 | PR 1 | Config skeleton (TOML loader, `api_key_file` indirection, `Config::load`) |
| 0.3.0 | PR 2 | Windy Point Forecast client (`clients::windy`) |
| 0.4.0 | PR 3a | SVG → 1-bit BMP renderer (`render::Renderer`) |
| 0.5.0 | PR 3b | TRMNL BYOS endpoints (`/api/display`, `/api/log`, `/images/:filename`) |
| 0.6.0 | PR 3c | Fetch → render → publish loop (`publish::PublishLoop`) under a `supervise` wrapper |

Every commit went through `/commit` with red-team +
artisan reviews; ~80 findings addressed and documented
in `redteam-resolved.md` / `artisan-resolved.md`. The
non-obvious design decisions live there, not in code
comments — read them before changing the design.

## What works end-to-end

```bash
cargo run -p bellwether-web -- --config config.toml
```

- Loads TOML config (validated: lat/lon ranges, render
  dims ≤ 4096, refresh rate ∈ 1..=86400).
- Reads Windy API key from `windy_key.txt`.
- Seeds the image store with a geometric placeholder
  BMP.
- Spawns the publish loop under
  `bellwether::publish::supervise`. Every
  `default_refresh_rate_s` the loop fetches Windy,
  renders a placeholder dashboard (bar scaled by
  current temperature; X overlay when no data), and
  puts the BMP into the `ImageStore`.
- Axum serves `/api/display`, `/api/log`,
  `/images/{filename}`. Optional `Access-Token`
  middleware gated on `BELLWETHER_ACCESS_TOKEN`.

`--dev` mode skips the publish loop and serves only
the placeholder — useful for frontend work without a
Windy key.

Default port is 3100 (was 3000 originally; changed
because operator's 3000 was taken).

## Open decisions you need the user to make

The user hasn't confirmed these; they may block later
PRs.

1. **Device BYOS status.** Is the TRMNL device at
   `malina` already reconfigured to point at the
   bellwether server, or still polling `trmnl.com`?
   BYOS needs the device's server URL to be flashed
   or configured. Verify before PR 3d's real
   dashboard goes live — otherwise you're rendering
   into a store no device ever reads.
2. **Dashboard font choice.** PR 3d needs at least
   one bundled font for text rendering. Options: m6x11
   (permissively licensed, ~6KB), Geist Mono (SIL OFL,
   larger but nice), or a pixel font crafted for
   e-ink. The user has preferences — ask with
   `AskUserQuestion` showing 2-3 options before
   bundling anything.
3. **Dashboard layout.** What should the dashboard
   actually show? Candidates: current conditions +
   3-day forecast, hourly sparklines, astro info
   (sunrise/sunset), HA entity list. Sketch a
   wireframe before rendering.
4. **Production deployment.** Is the plan a
   systemd unit on `malina`, a Docker container, or
   something else? Affects the "deployment" section
   a future PR will need.

## Recommended next PRs

In rough order of value / unblock-ratio:

1. **PR 3d — real dashboard layout.** Biggest
   visible win. Pick a font, design a layout, replace
   `build_dashboard_svg` placeholder. Extends the
   `ForecastRenderer` trait boundary (AQ-065 TODO in
   publish/mod.rs — if a second sink consumer
   appears, promote `ImageSink` to a neutral
   `crate::sink` module).
2. **PR 3e — telemetry persistence + `/api/status`.**
   Currently `/api/log` drops everything after
   logging. Persist in `TrmnlState` → expose via
   `/api/status` so the operator sees device health
   at a glance. Enables refresh-rate adaptation
   later. Tracked in `TODO.md` → "PR 3d / later".
3. **PR 3f — Home Assistant client.** Mirrors the
   Windy client. Spike §5 settled on long-lived
   access token via `token_file`. Use `wiremock`
   for tests (same pattern as `clients::windy`).
4. **PR 3g — real-device smoke test.** End-to-end
   validation against the actual TRMNL at `malina`.
   Depends on open decision #1.
5. **CI hardening.** `cargo audit` + `cargo deny`
   in the xtask pipeline. Queued as a chore in
   `TODO.md`.

## User working style

See `CLAUDE.md` for the full list. Two guardrails
learned from past corrections that aren't there yet:

- **Don't stash intermediate files in `/tmp`.** On
  Windows it maps to `%AppData%\Local\Temp\` — outside
  the workspace, invisible to the operator. When
  handing large output to a subagent, have the
  subagent run the command itself, or write to
  `target/…` (git-ignored).
- **Use the six-field finding format.** When
  presenting review findings, match the `/commit`
  skill spec: ID, Source, Category, Description,
  Impact, Suggested fix. Don't compress to prose.

## Memory

Local Claude Code memory may add context on the
operator's own machine; everywhere else, this file +
`CLAUDE.md` are the whole picture. Any memory entry
load-bearing enough that the project depends on it
should migrate into `CLAUDE.md` or here.

## Where to find more

- `docs/developer/DIARY.md` — timeline of every
  feature with design rationale.
- `docs/developer/spike.md` — the original design
  spike that settled TRMNL protocol choice (BYOS),
  render stack (`resvg` + hand-rolled BMP),
  dithering (Floyd–Steinberg), HA auth (long-lived
  token).
- `docs/developer/redteam-resolved.md` +
  `artisan-resolved.md` — ~80 resolved review
  findings with resolution notes. The "why" behind
  most non-obvious decisions.
- `docs/developer/template-feedback.md` — upstream
  rustbase issues discovered while building this.
  Feeds back via `/template-sync`.
- `TODO.md` — prioritized next work + backlog +
  chores.
- `CHANGELOG.md` — per-version user-visible
  changes.
- `CLAUDE.md` — hard project conventions.

## External references

- TRMNL firmware: https://github.com/usetrmnl/firmware
  (critical reference — `lib/trmnl/src/bmp.cpp`
  defines the BMP palette ordering we match).
- TRMNL HA add-on: https://github.com/usetrmnl/trmnl-home-assistant
- Windy Point Forecast API: https://api.windy.com/point-forecast/docs
- Home Assistant REST API: https://developers.home-assistant.io/docs/api/rest/
- rustbase template:
  https://github.com/breki/rustbase

## On the rustbase template

This project tracks `breki/rustbase`. Log template
issues in `docs/developer/template-feedback.md` —
upstream improvements flow back via `/template-sync`.
Don't cherry-pick manually.
