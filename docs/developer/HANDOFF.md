# Handoff to the next agent

Current as of 2026-04-20, version **0.19.0** (commit
`baddff9`). You are the next agent — read this once
end-to-end before touching anything.

## What's built

The project is well past its scaffold. Recent
feature versions:

| Version | What |
|---------|------|
| 0.6.0 | Publish loop (fetch → render → publish) under `supervise` |
| 0.7.0 | Windy → Open-Meteo migration (see `weather-provider-migration.md`) |
| 0.13.0 | Dense 5-band dashboard layout matching the mockup |
| 0.14.0 | Configurable widget layout via `layout.toml` |
| 0.15.0 | `[dashboard]` layout folded into main config |
| 0.16.0 | Svelte frontend dropped; hand-rolled Rust landing page at `/` |
| 0.16.1 | Landing-page preview renders the latest dashboard |
| 0.17.0 | Atomic widgets (`weather-icon`, `temp-now`, etc.) |
| 0.18.0 | Source Sans 3 Semibold as bundled font |
| 0.19.0 | `cargo xtask preview` + `Renderer::render_to_png` |

Every commit goes through `/commit` with red-team +
artisan reviews; ~115 findings resolved and documented
in `redteam-resolved.md` / `artisan-resolved.md`. The
non-obvious design decisions live there, not in code
comments — read them before changing the design.

## What works end-to-end

```bash
cargo run -p bellwether-web -- --config config.toml
# open http://localhost:3100
```

Weather provider is **Open-Meteo** (free, keyless). No
on-disk secret file. The landing page at `/` lists
endpoints and embeds the latest rendered dashboard via
`/preview.bmp` (unauthenticated, intended LAN-only
use — see RT-113 in `redteam-log.md` for the open
decision).

`--dev` runs without a config file; the publish loop
is skipped and the image store is seeded with the
placeholder BMP only. Good for iterating on the
landing page or xtask tooling.

## What's immediately in flight (uncommitted WIP)

As of this handoff, these files are modified or
untracked in the working tree and **belong to the
next PR, not this one**:

- `crates/bellwether/src/dashboard/icons.rs` — replaced
  hand-rolled 4-icon SVG primitives with `include_str!`
  of upstream Weather Icons SVG files.
- `crates/bellwether/src/dashboard/svg/mod.rs` — the
  icon renderer now emits `<svg x y width height>…</svg>`
  wrapping the bundled SVG document, instead of
  `<g transform="translate scale">` wrapping a
  48-user-unit fragment.
- `crates/bellwether/assets/icons/weather-icons/`
  (untracked) — 4 SVG files downloaded verbatim from
  https://github.com/erikflowers/weather-icons
  (SIL OFL 1.1): `wi-day-sunny.svg`, `wi-day-cloudy.svg`,
  `wi-cloudy.svg`, `wi-rain.svg`.

Three stray `.png` screenshots at the workspace root
(`dashboard-weather-icons.png`, `preview-dashboard-vector.png`,
`xtask-preview-three-panel.png`) and a `.playwright-mcp/`
directory are scratch artefacts from the icon
exploration — either delete them or gitignore them.

## Recommended next PR sequence

The user approved this plan in conversation. Two-tier
fidelity, per-widget-instance selector, incremental
icon design. All four PRs below together replace the
current 4-variant `Condition` enum with a WMO-backed
two-tier model. **Each PR is independently mergeable
and visually verifiable via `cargo xtask preview`.**

### PR 1 — Plumb raw `weather_code` end-to-end (data only, no visual change)

Append `weather_code` to Open-Meteo's `HOURLY_VARIABLES`
(currently `crates/bellwether/src/clients/open_meteo/mod.rs:78-79`,
7 fields, no `weather_code`). Add `weather_code: Vec<Option<u8>>`
to `WeatherSnapshotBuilder` and `WeatherSnapshot`;
extend the length-validation tuple at
`weather/mod.rs:114`; narrow the incoming JSON number
to `u8` (values outside 0..=99 → `None`, non-integers
→ `None`).

Display layer unchanged in this PR — the existing
cloud+precip classifier still drives all icons. This
PR is pure plumbing.

**TDD order:** wiremock test with canned JSON carrying
`weather_code` → `RawHourly` struct field → `pick_series`
adapter → accessor + round-trip test. Validate green
when done.

### PR 2 — Expand display taxonomy to two tiers

Introduce two types in `dashboard/classify.rs`:

- `WmoCode` — exhaustive enum with one variant per
  WMO 4677 code (~27 variants). `TryFrom<u8>`
  converts raw code values; out-of-table codes return
  `None`. Store on `WeatherSnapshot` as
  `Vec<Option<WmoCode>>` (narrow from `u8` at parse).
- `ConditionCategory` — 9-variant coarse view:
  `Clear`, `PartlyCloudy`, `Cloudy`, `Fog`, `Drizzle`,
  `Rain`, `Snow`, `Thunderstorm`, `Unknown`. Never
  stored; always computed via
  `WmoCode::coarsen() -> ConditionCategory`.

Fallback classifier: when `weather_code` is `None`
(provider gap, older data), call the existing
cloud+precip logic — but it produces a
`ConditionCategory` directly, because numeric signals
aren't precise enough to fabricate a specific WMO
code. Document this boundary in the module doc.

**Coarsen mapping** (locked in conversation — ship as
part of PR 2 tests):

| `WmoCode`s | `ConditionCategory` |
|---|---|
| Clear | Clear |
| MainlyClear, PartlyCloudy | PartlyCloudy |
| Overcast | Cloudy |
| Fog, RimeFog | Fog |
| DrizzleLight/Moderate/Dense, FreezingDrizzleLight/Dense | Drizzle |
| RainSlight/Moderate/Heavy, FreezingRainLight/Heavy, RainShowersSlight/Moderate/Violent | Rain |
| SnowSlight/Moderate/Heavy, SnowGrains, SnowShowersSlight/Heavy | Snow |
| Thunderstorm, ThunderstormHailSlight/Heavy | Thunderstorm |

### PR 3 — Per-instance `fidelity` widget setting + icon dispatch

Add `fidelity: Fidelity { Simple, Detailed }` (default
`Simple`) as an **optional per-widget-instance** field
in `layout.toml`. The user specifically chose
per-instance over per-kind — so the same
`weather-icon` widget can render detailed in the
today-band and simple in the forecast tiles:

```toml
{ size = 150, widget = "weather-icon", day = "today",
  fidelity = "detailed" }
{ flex = 1,  widget = "weather-icon", day = 0 }
# ^ defaults to "simple"
```

Two icon lookup functions with a **graceful fallback**
so PR 3 is mergeable before any detailed icons exist:

```rust
pub fn icon_for_category(c: ConditionCategory)
    -> &'static str;  // 9 icons, mandatory

pub fn icon_for_wmo(code: WmoCode) -> &'static str {
    match code {
        // Specialized arms added here in PR 4+:
        // WmoCode::Fog => FOG,
        // WmoCode::ThunderstormHailHeavy => HAIL_THUNDER,
        other => icon_for_category(other.coarsen()),
    }
}
```

Replace the current 4 hand-rolled-but-now-Weather-Icons
constants with the 9 category icons from Weather Icons:

| `ConditionCategory` | Weather Icons filename |
|---|---|
| Clear | wi-day-sunny.svg |
| PartlyCloudy | wi-day-cloudy.svg |
| Cloudy | wi-cloudy.svg |
| Fog | wi-fog.svg |
| Drizzle | wi-sprinkle.svg |
| Rain | wi-rain.svg |
| Snow | wi-snow.svg |
| Thunderstorm | wi-thunderstorm.svg |
| Unknown | wi-na.svg |

Existing `layout.toml` deserializes unchanged (serde
default). Assert this with a test.

### PR 4+ — Draw/bundle specialized detailed icons (one arm at a time)

Each PR adds one or more `wi-*.svg` files plus the
matching `icon_for_wmo` arm. Suggested priority order
(based on visual impact and dither-legibility
expected):

1. `Fog`, `Thunderstorm`, `ThunderstormHailHeavy` —
   dramatic weather deserves a distinct glyph before
   intensity splits.
2. Snow variants (`SnowSlight/Moderate/Heavy`).
3. Rain intensities (`RainSlight/Moderate/Heavy`).
4. Freezing-precipitation variants.

No PR depends on a later one — the `coarsen()`
fallback ensures every WMO code has a showable icon
from day one.

## Open decisions / caveats for the next agent

1. **Weather Icons LICENSE file needs bundling.** I
   tried to download `LICENSE` from
   `raw.githubusercontent.com/erikflowers/weather-icons/master/LICENSE`
   and got a 404. The upstream project uses SIL OFL
   1.1 for the icons per its README, but the exact
   file path at the tag we're pinned to needs to be
   found. Look under `font/`, `LICENSE.md`, or
   `OFL.txt`. Bundle into
   `crates/bellwether/assets/icons/weather-icons/LICENSE`
   and link from `docs/credits.md` (new file).
2. **Dither verification on physical e-ink still
   pending.** `cargo xtask preview` shows vector,
   pre-dither PNG, and 1-bit BMP side-by-side, but no
   one has confirmed the Weather Icons curves look
   acceptable on actual TRMNL hardware yet. Deploy to
   `malina` before declaring the icon PR merged; if
   curves shimmer, Meteocons
   (https://bas.dev/work/meteocons) is a
   heavier-fill alternative using the same SVG
   integration pattern.
3. **Coverage impact of the 27-variant `WmoCode` enum.**
   Exhaustive-match tests over every variant avoid
   the coverage trap — write one per mapping
   (`TryFrom<u8>` round-trip, `coarsen()`
   correctness). The existing
   `each_icon_covers_every_condition_variant` test at
   `dashboard/icons.rs` is the pattern to follow.
4. **Sample model needs fidelity coverage.** The
   `sample_model()` in
   `dashboard/svg/tests.rs` + the rich snapshot
   behind `generate_dashboard_sample` currently
   exercise 4 conditions. After PR 2, extend to cover
   every `ConditionCategory`; after PR 3 add a
   `fidelity = "detailed"` instance in the preview
   layout so `cargo xtask preview` renders both
   tiers.

## User working style

Hard rules are in `CLAUDE.md`. Soft preferences learned
from this session:

- **Fix review findings in-PR**, don't defer. When
  red-team/artisan surface actionable findings, the
  user consistently chooses fix-in-PR over
  commit-as-is. Commit v0.19.0 fixed all 11 findings
  before committing.
- **Narrate each tool-calling step** in user-visible
  text — don't rely on the Bash `description`
  parameter alone. One-liner per logical step,
  including parallel tool-call groups.
- **Use `AskUserQuestion` for multi-choice decisions.**
  The user prefers the structured UI over
  prose-and-wait when there are 2–4 options with
  trade-offs.
- **No `cd` or `git -C <dir>`** — those can't be
  pre-allowlisted and trigger permission prompts on
  every call. Always use absolute paths.
- **Use Windows OpenSSH for ssh/scp.** From Git Bash
  the bare binaries can't reach the Windows
  ssh-agent. Use `/c/Windows/System32/OpenSSH/ssh.exe`
  for deploy operations.

## Where to find more

- `docs/developer/DIARY.md` — timeline of every
  feature with design rationale.
- `docs/developer/spike.md` — original spike that
  settled TRMNL protocol (BYOS), render stack (`resvg`
  + hand-rolled BMP), dithering (Floyd–Steinberg).
- `docs/developer/weather-provider-migration.md` —
  Windy → Open-Meteo transition notes.
- `docs/developer/redteam-resolved.md` +
  `artisan-resolved.md` — resolved review findings.
  The "why" behind most non-obvious decisions, in
  reverse-chronological order.
- `docs/developer/redteam-log.md` +
  `artisan-log.md` — open findings. Threshold of
  10+ triggers a full-codebase review.
- `docs/developer/template-feedback.md` — upstream
  rustbase issues. Feeds back via `/template-sync`.
- `CHANGELOG.md` — per-version user-visible
  changes.
- `CLAUDE.md` — hard project conventions.

## External references

- TRMNL firmware:
  https://github.com/usetrmnl/firmware
  (`lib/trmnl/src/bmp.cpp` defines the BMP palette
  ordering the renderer matches).
- Open-Meteo Forecast API:
  https://open-meteo.com/en/docs
  (WMO 4677 weather-code table is at the bottom of
  that page).
- WMO Code 4677 reference:
  https://artefacts.ceda.ac.uk/badc_datadocs/surface/code.html
- Weather Icons (Erik Flowers):
  https://erikflowers.github.io/weather-icons/
- Meteocons (Bas Milius, backup option):
  https://bas.dev/work/meteocons
- Home Assistant REST API:
  https://developers.home-assistant.io/docs/api/rest/
- rustbase template:
  https://github.com/breki/rustbase

## On the rustbase template

This project tracks `breki/rustbase`. Log template
issues in `docs/developer/template-feedback.md` —
upstream improvements flow back via `/template-sync`.
Don't cherry-pick manually.
