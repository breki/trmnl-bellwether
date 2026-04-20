# Handoff to the next agent

Current as of 2026-04-20, version **0.21.0** (commit
`371b437`). You are the next agent — read this once
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
| 0.20.0 | Weather Icons (Erik Flowers, SIL OFL 1.1) replace hand-rolled SVG primitives for the 4 existing conditions |
| 0.20.1 | Post-commit review fixes: `bellwether::licenses` + `/licenses` endpoint close OFL §2 binary redistribution; `skip_to_svg_root` hardened; pinned SHA-256 per bundled icon; `each_icon_renders_visible_pixels` test replaces the regressed `fill="black"` invariant |
| 0.21.0 | WMO `weather_code` plumbed end-to-end; two-tier taxonomy (`WmoCode` 28 variants + `ConditionCategory` 9 variants) with `WeatherCode { Wmo, Unrecognised }` at the narrowing boundary; nine Weather Icons SVGs with `icon_for_category` / `icon_for_wmo` dispatch (coarsen fallback); `Condition::to_category` deprecated as temporary bridge |

Every commit goes through `/commit` with red-team +
artisan reviews; ~130 findings resolved and documented
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

`GET /licenses` serves every bundled third-party
license text as `text/plain`, exempt from the
access-token middleware. Binary-only redistribution
of `bellwether-web` (e.g. `cargo xtask deploy` to
`malina`) satisfies SIL OFL 1.1 §2 via this route —
the compiled binary carries the OFL text with it.
Don't let a future refactor gate this route behind
auth; RT-A in `redteam-resolved.md` explains why.

## Working-tree scratch (not part of any commit)

The following untracked paths were created during
the 2026-04-20 icon exploration and should either be
deleted or gitignored:

- `dashboard-weather-icons.png`,
  `preview-dashboard-vector.png`,
  `xtask-preview-three-panel.png` — Playwright
  screenshots from the icon exploration.
- `.playwright-mcp/` — Playwright MCP runtime cache.

None of these are load-bearing; they're just visual
evidence of the exploration that predates the icon
swap commit.

## Recommended next PR sequence

v0.21.0 shipped the first three PRs from the WMO-icon
plan (weather_code plumbing, two-tier taxonomy,
nine-icon dispatch). Remaining work splits into two
phases — PR 4 is the prerequisite structural step, PR
5+ are incremental icon bundles.

### PR 4 — Thread `WeatherCode` through the model + reintroduce `Fidelity`

The render path currently reads `Condition` off
`DashboardModel::current` / `days[i]` and bridges it
through the deprecated `Condition::to_category()` into
`icon_for_category`. The detailed-icon path
(`icon_for_wmo`) isn't reachable because the model
doesn't carry a `WmoCode` / `WeatherCode`. **Fixing
this unlocks every subsequent PR**, so it's PR 4.

Scope:

1. Add `weather_code: Option<WeatherCode>` to
   `model::Current` and `model::DayTile`. Populate from
   `WeatherSnapshot::weather_code()` in the same place
   the model currently derives `condition`.
2. Replace `render_weather_icon`'s `Option<Condition>`
   parameter with `(Option<WeatherCode>, Option<Condition>)`
   — or better, compute the category up front via
   `classify_category` and pass that plus the raw
   `WeatherCode` for detailed dispatch. Delete the
   `#[allow(deprecated)]` call to
   `Condition::to_category()` at `svg/mod.rs:591`.
3. **Reintroduce `Fidelity { Simple, Detailed }` with
   `#[derive(Default)]` + optional `fidelity` on
   `WidgetKind::WeatherIcon`** (reverted from the
   original PR 3 because the renderer had nowhere to
   consume it — RT-115/AQ-131). The dispatcher now
   honours it: `Simple` → `icon_for_category(category)`,
   `Detailed` → `icon_for_wmo(code)` when a
   `WeatherCode::Wmo(_)` is present, else the category.
4. Add a renderer test that locks the behavioural
   difference: same `WeatherCode`, two widget
   instances, different fidelity → different SVG bytes.
5. Delete `Condition::to_category` and the
   `#[allow(deprecated)]` sites once (4) passes —
   that's the deprecation's exit criterion.
6. Update the sample model at `dashboard/svg/tests.rs`
   + `generate_dashboard_sample` to cover every
   `ConditionCategory` variant and include at least
   one `fidelity = "detailed"` widget in the preview
   layout, so `cargo xtask preview` shows both tiers.

PR 4 lands visually as a **zero-change-on-current-data
PR** (coarsen fallback still picks the same 9 icons
the category path picks) but makes detailed dispatch
reachable for PR 5+.

### PR 5+ — Bundle specialized detailed icons (one arm at a time)

Each PR adds one or more `wi-*.svg` files to
`assets/icons/weather-icons/`, a matching
`icon_for_wmo` arm in `icons.rs`, and a SHA-256 pin
in `PINNED_SHA256`. Suggested priority order (visual
impact + dither-legibility expected):

1. `Fog`, `Thunderstorm`, `ThunderstormHailHeavy` —
   dramatic weather deserves a distinct glyph before
   intensity splits.
2. Snow variants (`SnowSlight/Moderate/Heavy`).
3. Rain intensities (`RainSlight/Moderate/Heavy`).
4. Freezing-precipitation variants.

No PR depends on a later one — the `coarsen()`
fallback in `icon_for_wmo` ensures every `WmoCode`
variant has a showable icon from day one, so
specialised arms are pure additions.

## Open decisions / caveats for the next agent

1. **Dither verification on physical e-ink still
   pending.** `cargo xtask preview` shows vector,
   pre-dither PNG, and 1-bit BMP side-by-side for the
   nine bundled Weather Icons, but no one has
   confirmed the curves look acceptable on actual
   TRMNL hardware yet. Deploy to `malina` before
   declaring the icon work fully done; if curves
   shimmer, Meteocons (https://bas.dev/work/meteocons)
   is a heavier-fill alternative using the same SVG
   integration pattern.
2. **`classify.rs` is at ~720 lines.** AQ-132 in
   `artisan-log.md` flags splitting it into
   `classify/{mod,weather,compass}.rs`. Low-risk
   mechanical refactor; `Compass8` is unrelated to
   the weather-state taxonomy and doesn't belong in
   the same file. A good warm-up task before PR 4 if
   you want to touch the module without semantic
   risk.
3. **New Weather Icons additions must pin a
   SHA-256.** The `bundled_icons_match_pinned_sha256`
   test in `dashboard/icons.rs` enforces the
   "byte-identical to upstream" claim. Any PR adding
   a new `wi-*.svg` file must add its hash to the
   `PINNED_SHA256` table or the build fails. Compute
   via `sha256sum < the-file`. The `BUNDLED_ICONS`
   table in the test module is the single source of
   truth for which files exist — adding a file
   without updating that table fails the coverage
   sweep.
4. **`bellwether::licenses::ALL` must grow with
   every new bundled asset.** If PR 5+ bundles icons
   from a second upstream source (e.g. Meteocons as
   a fallback set) its license text must be wired
   into the `ALL` registry so `/licenses` surfaces
   it. The `every_bundle_has_a_non_empty_license_entry`
   test catches empty entries but not missing ones —
   you have to remember.
5. **`WmoCode::ALL` is the single source of truth.**
   Adding a variant to `WmoCode` requires adding it
   to `ALL`, to `TryFrom<u8>`'s table, to
   `coarsen()`'s exhaustive match, and to the
   `coarsen_follows_handoff_mapping_exhaustively`
   test's pair table. The compiler catches the first
   two (exhaustive match on the enum); `ALL.len()`
   vs the pair table's `len()` assertion catches the
   fourth. Only the `ALL` step is human memory — a
   new variant not listed there silently drops from
   every iteration-based test.

## User working style

Hard rules are in `CLAUDE.md`. Soft preferences learned
from this session:

- **Fix review findings in-PR**, don't defer. When
  red-team/artisan surface actionable findings, the
  user consistently chooses fix-in-PR over
  commit-as-is. Recent precedent: v0.19.0 fixed all
  11 findings before committing; v0.20.1 was itself
  a 15-finding follow-up to v0.20.0 rather than an
  open-logs deferral.
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
