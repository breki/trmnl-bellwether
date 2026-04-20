# Handoff to the next agent

Current as of 2026-04-20, version **0.24.0** (commit
`da8dd7b`). You are the next agent — read this once
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
| 0.22.0 | Model refactor (PR 4): `CurrentConditions` / `DaySummary` collapsed to single `category: ConditionCategory` + `weather_code: Option<WeatherCode>` (populated via `classify_category` at build time, no more dual-representation drift); `Fidelity { Simple, Detailed }` reintroduced as `Option<Fidelity>` on `WidgetKind::WeatherIcon` so `layout.toml` round-trips losslessly; `render_weather_icon(bounds, &DayView, Option<Fidelity>)` consolidates the positional-option signature; `ConditionCategory::label` replaces `Condition::label` on the render path; `Condition::to_category` deleted |
| 0.23.0 | First specialised WMO icon (PR 5): `wi-hail.svg` bundled for `WmoCode::ThunderstormHailHeavy → icon_for_wmo` specialisation; all other codes still coarsen. Exhaustive `dispatch_kind(WmoCode) -> {Specialised, Coarsened}` helper in `icons.rs` tests makes "add a new variant without classifying it" a compile error. Behavioural test (deferred from PR 4) locks the `Fidelity::Detailed` → different SVG bytes contract |
| 0.23.1 | Dither pre-threshold snap (≤ 20% → 0, ≥ 80% → 255) before FS loop; `cargo xtask deploy` now syncs `deploy/bellwether-web.service` to the RPi when the installed unit differs (guards against v0.16.0-style CLI-arg drift that crash-loops the service) |
| 0.24.0 | Rasteriser anti-aliasing disabled (`usvg::ShapeRendering::CrispEdges` + `TextRendering::OptimizeSpeed`) — grayscale buffer contains only pure 0 and pure 255, FS has nothing to diffuse, e-ink panel renders crisp text and icons with zero shimmer. Trade-off: pixel-level staircase aliasing on diagonals, acceptable at TRMNL-OG's ~150 DPI glance distance. Hardware-verified on `malina` |

Every commit goes through `/commit` with red-team +
artisan reviews; ~140 findings resolved and documented
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

The dashboard at v0.24.0 is **hardware-verified on
the TRMNL-OG** (running at `malina`). Crisp, no
shimmer, staircase aliasing on diagonals is within
glance-distance tolerance. The Weather Icons choice
is validated — no need to fall back to Meteocons.

## Recommended next PR sequence

PR 4 (model + Fidelity) landed in v0.22.0. PR 5
(first specialised icon, `wi-hail.svg` for
`ThunderstormHailHeavy`) landed in v0.23.0. Remaining
work is additive icon bundles — each is one SVG file
+ one `icon_for_wmo` match arm + one `PINNED_SHA256`
entry + one `dispatch_kind` classification + one
`README.md` row. No cross-PR dependencies: the
`coarsen()` fallback in `icon_for_wmo` guarantees
every `WmoCode` has a showable icon regardless of
specialisation status.

### PR 6 — Snow variants

Bundle `wi-snowflake-cold.svg` (or a denser snow
glyph) and wire up one or more of:

- `WmoCode::SnowSlight` (71)
- `WmoCode::SnowModerate` (73)
- `WmoCode::SnowHeavy` (75)
- `WmoCode::SnowGrains` (77)
- `WmoCode::SnowShowersSlight` (85)
- `WmoCode::SnowShowersHeavy` (86)

Decision to make: do intensity variants get distinct
glyphs (three snow files), or a single specialised
glyph that visually differs from the coarse
`wi-snow.svg`? Recommend starting with `SnowHeavy` →
a heavier glyph as the only specialised arm, following
the "one file per PR" cadence from PR 5.

### PR 7 — Rain intensities

Same pattern for rain: `RainSlight` / `RainModerate`
/ `RainHeavy` + `RainShowers*`. Natural candidate
glyphs: `wi-raindrops.svg` or `wi-showers.svg` for
heavier variants.

### PR 8 — Freezing variants

Freezing drizzle and freezing rain. Candidate:
`wi-snowflake-cold.svg` or `wi-sleet.svg` for visual
distinction from plain drizzle/rain.

### Mechanics for every PR 6+ bundle

1. Fetch the upstream SVG verbatim from
   `https://raw.githubusercontent.com/erikflowers/weather-icons/master/svg/`.
   Place under `crates/bellwether/assets/icons/weather-icons/`.
2. Add a `const FOO_RAW: &str = include_str!(...)` in
   `dashboard/icons.rs` with a docstring naming the
   upstream glyph and the `WmoCode` it serves.
3. Add an arm **above** the catch-all in `icon_for_wmo`:
   `WmoCode::Foo => skip_to_svg_root(FOO_RAW)`.
4. Add the SHA-256 pin (`sha256sum < the-file`) to
   `PINNED_SHA256` and the `(filename, BYTES)` row to
   `BUNDLED_ICONS` in the test module.
5. Reclassify the `WmoCode` variant in `dispatch_kind`
   from `Coarsened` to `Specialised`. Compile error
   until all three tables (`icon_for_wmo` arms,
   `PINNED_SHA256`, `dispatch_kind`) agree.
6. Add a row to the "Detailed-fidelity icons" section
   of `assets/icons/weather-icons/README.md`.
7. Deploy to `malina` and eyeball.

## Open decisions / caveats for the next agent

1. **`classify.rs` is ~870 lines.** AQ-132 in
   `artisan-log.md` flags splitting it into
   `classify/{mod,weather,compass}.rs`. `Compass8`
   shares no types or invariants with the weather-state
   taxonomy; keeping them in one file is an accident
   of "both are display-layer bucketing". Low-risk
   mechanical refactor; good warm-up task before a
   feature session if you want to touch the module
   without semantic risk.
2. **New Weather Icons additions must pin a SHA-256,
   update `BUNDLED_ICONS`, and reclassify in
   `dispatch_kind`.** The
   `bundled_icons_match_pinned_sha256` test enforces
   byte-identity with upstream. The
   `icon_for_wmo_respects_its_dispatch_classification`
   test enforces that every `WmoCode` is either
   Specialised (arm exists) or Coarsened (falls through)
   — nothing in between. Both tests are compile-time
   forcing functions for the mechanical steps above.
3. **`bellwether::licenses::ALL` must grow with
   every new bundled asset.** If a future PR bundles
   icons from a second upstream source (e.g.
   Meteocons as a fallback set) its license text
   must be wired into the `ALL` registry so
   `/licenses` surfaces it. The
   `every_bundle_has_a_non_empty_license_entry` test
   catches empty entries but not missing ones — you
   have to remember.
4. **`WmoCode::ALL` is the single source of truth.**
   Adding a variant to `WmoCode` requires adding it
   to `ALL`, to `TryFrom<u8>`'s table, to
   `coarsen()`'s exhaustive match, to `dispatch_kind`,
   and to the `coarsen_follows_handoff_mapping_exhaustively`
   test's pair table. The compiler catches four of
   those five (exhaustive matches + length assertion);
   only `ALL` itself is human memory — a new variant
   not listed there silently drops from every
   iteration-based test.
5. **`Renderer::new()` disables anti-aliasing at the
   rasterizer by default.** `usvg::Options` gets
   `shape_rendering = CrispEdges` and
   `text_rendering = OptimizeSpeed` set inside
   `configure_bilevel`. If a future caller ever
   needs anti-aliased output (e.g. for a PNG preview
   of a photographic asset), they must either clone
   and mutate `options`, or a new
   `Renderer::with_antialiasing()` constructor needs
   adding. The choice is deliberate: the 1-bit
   e-ink pipeline is pure-bilevel-by-design after
   v0.24.0 — see the `configure_bilevel` docstring.
   The `rasteriser_produces_bilevel_luma_with_no_intermediate_greys`
   test locks the invariant against an accidental
   regression.

## User working style

Hard rules are in `CLAUDE.md`. Soft preferences learned
from recent sessions:

- **Fix review findings in-PR**, don't defer. When
  red-team/artisan surface actionable findings, the
  user consistently chooses fix-in-PR over
  commit-as-is. Precedent: PR 4 fixed 5 artisan
  findings in-PR (v0.22.0); PR 5 fixed 6 findings
  (v0.23.0).
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
- **Revert-and-retry is preferred over pile-on.**
  v0.23.2's supersample attempt didn't help on
  hardware and was reverted cleanly via `git revert`
  rather than papering over with a follow-up. The
  revert commit is a feature of the log, not noise.
- **Hardware iteration is cheap; do it.** Three
  shimmer-fix iterations (pre-threshold snap,
  supersample, CrispEdges) shipped in one afternoon
  because deploys to `malina` take ~1 minute and the
  visual feedback is unambiguous. Preview tooling
  (`cargo xtask preview`) is useful for layout but
  can't replicate the e-ink panel's dither
  characteristics — the panel itself is the oracle.

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
- Meteocons (Bas Milius, backup option — not
  currently needed now that CrispEdges eliminated
  shimmer, but documented in case future icon
  bundling drives a full swap):
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
