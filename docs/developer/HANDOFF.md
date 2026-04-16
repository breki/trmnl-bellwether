# Handoff to the next agent

Written 2026-04-16 by the agent that scaffolded this
project from the rustbase template. You are the next
agent — read this once end-to-end before touching
anything.

## Your job

Build `bellwether`: a Rust server that pulls state from
Home Assistant + weather from the Windy Point Forecast
API, renders e-ink dashboards server-side, and serves
them to a [TRMNL](https://trmnl.com/) e-paper display
via webhook. Intended to run on a Raspberry Pi (hostname
`malina`).

The project is **scaffold only** right now. Every line
of domain logic is yours to add.

## What's already done (don't redo)

- Rust workspace renamed to `bellwether` /
  `bellwether-web`. Binary is `bellwether`, web binary
  is `bellwether-web`. **Do not rename again.**
- Scaffold inherited from the `rustbase` template at
  commit `076cf44` (v0.4.0): `xtask validate` with fmt
  + clippy + tests + coverage (>=90%) + dupes (<=6%) +
  `svelte-check`; Playwright E2E; `/commit` discipline;
  CI + release workflows.
- `.template-sync.toml` pins the upstream baseline.
  When you want rustbase improvements, use
  `/template-sync` — don't cherry-pick manually.
- Frontend is a Svelte 5 + Vite scaffold with TS. It
  shows the placeholder status/greeting cards. Treat
  it as the future **control panel** for selecting HA
  entities and editing layouts, not as the e-ink
  output.

## What to figure out before writing code

These questions were flagged in the source brief
(`D:/src/phren/knowledge/projects/trmnl-display-for-home-assistant-and-weather.md`)
and are still open. Resolve them before designing
anything.

1. **TRMNL webhook protocol.** What HTTP endpoint does
   the TRMNL device poll? What content-type and
   dimensions does it expect (raw BMP? PNG? base64?
   manifest JSON?)? Read the firmware source at
   [usetrmnl/firmware](https://github.com/usetrmnl/firmware)
   and the official HA add-on at
   [usetrmnl/trmnl-home-assistant](https://github.com/usetrmnl/trmnl-home-assistant)
   — that repo is the canonical reference. Look at
   [pwojtaszko/trmnl-home-assistant-plugin](https://github.com/pwojtaszko/trmnl-home-assistant-plugin)
   and
   [TilmanGriesel/ha_trmnl_weather_station](https://github.com/TilmanGriesel/ha_trmnl_weather_station)
   for working examples of the webhook payload.
2. **Grayscale vs. 1-bit.** Does the TRMNL panel
   support 2-bit / 4-bit grayscale or only
   black/white? Dithering strategy depends on the
   answer. The firmware repo has hardware specs.
3. **Image format.** Native resolution (800x480 is the
   rumor), bit depth, orientation, refresh rate
   expectations.
4. **Rust rendering crate.** Candidates:
   [`resvg`](https://crates.io/crates/resvg) (author
   layouts as SVG, render to raster — author-friendly),
   [`image`](https://crates.io/crates/image) (direct
   raster manipulation — faster but layout is code),
   [`tiny-skia`](https://crates.io/crates/tiny-skia)
   (2D drawing). Recommend `resvg` for v1 — SVG
   templates are the easiest thing to iterate on.
5. **HA auth model.** Long-lived access token vs.
   OAuth. LLA token is simpler for a personal project.

Answer these via README/doc writeup before you begin
implementing — a 10-minute spike doc beats a week of
wrong code.

## First three PRs (suggested order)

Each PR should go through `/commit`: red-team +
artisan review, version bump, CHANGELOG, DIARY.

1. **Config skeleton.** Add a TOML config file
   (`config.toml` by default, `--config` flag) with
   sections: `[home_assistant]` (base_url, token_file),
   `[windy]` (api_key_file, lat, lon), `[trmnl]`
   (device_id, webhook_url or plugin_id),
   `[render]` (layout_path, dimensions). Token /
   key files keep secrets out of the config. Add a
   `config::load` unit test and a fixture file under
   `test-data/`.
2. **HA + Windy client stubs.** Two crates or modules:
   `clients::home_assistant` and `clients::windy`.
   Each has a `fetch()` method returning a small
   domain type. Start with mock HTTP (use
   [`wiremock`](https://crates.io/crates/wiremock) in
   tests) and **don't** hit real endpoints in unit
   tests. Leave real-network tests behind a
   `#[ignore]` attribute with an env-var guard.
3. **First render + TRMNL publish.** Given a baked-in
   SVG layout and fixture data from steps 1-2,
   render to a PNG (or whatever TRMNL wants, per
   question 3 above) and POST it to the webhook
   endpoint. Add an E2E test using
   `wiremock` as a stand-in for the TRMNL server.

After PR 3, the loop `fetch -> render -> publish` is
working end-to-end with fakes; all downstream work is
filling in real data sources, layouts, and scheduling.

## User's working style (observed)

- **Small, reviewed PRs over big ones.** `/commit`
  triggers red-team + artisan reviews; don't bypass
  them even on "obvious" changes.
- **TDD red/green/refactor.** `CLAUDE.md` enforces
  it. No "I'll add tests later."
- **80-char line width** for both code (`rustfmt.toml`)
  and markdown. `cargo xtask validate` doesn't check
  markdown width — you do.
- **Strict quality gates.** Coverage >=90%, duplication
  <=6%, zero clippy warnings, `#[forbid(unsafe_code)]`.
  If a threshold gets in your way, don't lower it —
  talk to the user.
- **Ask when in doubt.** The user prefers
  `AskUserQuestion` with 2-4 concrete options over
  open-ended "what do you want?" questions. Options
  should include a `(Recommended)` marker when you
  have a strong preference.
- **No `cd` to project root, no `git -C <dir>`.**
  Both trigger permission prompts in the user's
  Claude Code setup. Use dedicated tools (Read, Edit,
  Grep, Glob) and absolute paths.
- **Never push without being asked.** The user runs
  `push` explicitly after reviewing the commit.
- **Secrets.** The user has an annual Windy
  subscription; **don't** ask them to generate new
  keys or sign up for anything. Expect keys loaded
  from `.env` or a file path named in config.

## Key files to read first

Spend 10 minutes before your first tool call on these:

1. `CLAUDE.md` — project conventions
2. `README.md` — project-facing summary
3. `docs/developer/DIARY.md` — what happened so far
4. `docs/developer/template-feedback.md` — log anything
   awkward about the inherited template here instead
   of just fixing it silently
5. `llms.txt` — machine-readable summary
6. `crates/bellwether-web/src/api/mod.rs` — the one
   piece of backend code that exists (scaffold API)
7. The source brief at
   `D:/src/phren/knowledge/projects/trmnl-display-for-home-assistant-and-weather.md`
   if it's still there. If not, the relevant content
   is: TRMNL e-paper dashboard + HA REST API + Windy
   Point Forecast + server-side render + RPi host
   `malina`.

## External references

- TRMNL: https://trmnl.com/
- TRMNL firmware (reference for webhook payload):
  https://github.com/usetrmnl/firmware
- TRMNL HA add-on (official):
  https://github.com/usetrmnl/trmnl-home-assistant
- TRMNL HA plugin (community, working code):
  https://github.com/pwojtaszko/trmnl-home-assistant-plugin
- TRMNL sensor push (community):
  https://github.com/gitstua/trmnl-sensor-push
- Weather station example for TRMNL:
  https://github.com/TilmanGriesel/ha_trmnl_weather_station
- Windy Point Forecast API:
  https://api.windy.com/point-forecast/docs
- Home Assistant REST API:
  https://developers.home-assistant.io/docs/api/rest/
- Rust image rendering:
  https://crates.io/crates/resvg ,
  https://crates.io/crates/image ,
  https://crates.io/crates/tiny-skia

## A note on the rustbase template

This project tracks `breki/rustbase` upstream. If you
find template issues (bad defaults, missing features),
log them in `docs/developer/template-feedback.md` with
one of the status prefixes `[Deferred]`,
`[Fixed locally]`, or `[N/A for template]`. Upstream
improvements flow back via `/template-sync`.
