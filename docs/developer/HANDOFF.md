# Handoff to the next agent

Current as of 2026-04-21, version **0.27.0** (commit
`7b0a56f`). You are the next agent — read this once
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
| 0.25.0 | PR 6: `wi-snow-wind.svg` bundled as the second specialised glyph; `WmoCode::SnowHeavy` now dispatches to the wind-driven snow shape under `fidelity = "detailed"`. Slight / Moderate / Grains / Showers snow variants still coarsen through plain `wi-snow.svg`. First review-clean PR (no findings from either reviewer) — PR 5's forcing-function scaffolding paying off |
| 0.26.0 | PR 7: `wi-rain-wind.svg` bundled for `WmoCode::RainHeavy`. Exact parallel to PR 6. Slight / Moderate / Showers / Freezing rain variants still coarsen through `wi-rain.svg`. Second consecutive review-clean PR |
| 0.27.0 | PR 8: `wi-sleet.svg` bundled for `WmoCode::FreezingRainHeavy`. Closes the HANDOFF's PR 5–8 specialisation plan — four "Heavy variant → distinct glyph" specialisations, one per category where intensity-peak detail matters (thunderstorm+hail, snow, rain, freezing rain). Third consecutive review-clean PR |

Every commit goes through `/commit` with red-team +
artisan reviews; ~149 findings resolved and documented
in `redteam-resolved.md` / `artisan-resolved.md`. The
non-obvious design decisions live there, not in code
comments — read them before changing the design. The
icon-bundle PRs (5–8) settled into a review-clean
rhythm after the forcing-function scaffolding landed
in PR 5: the exhaustive `dispatch_kind` match, the
SHA-256 pin table, and the `BUNDLED_ICONS` registry
between them catch every mechanical mistake at
compile time, leaving craftsmanship review with
nothing to flag on a pure-mechanical bundle.

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

## Recommended next work

**The HANDOFF's original PR 4–8 plan is complete.**
PR 4 (model + `Option<Fidelity>`) landed in v0.22.0.
The PR 5–8 icon specialisations all landed across
2026-04-20/21:

| PR | Version | Specialised arm |
|----|---------|-----------------|
| 5 | 0.23.0 | `ThunderstormHailHeavy` → `wi-hail.svg` |
| 6 | 0.25.0 | `SnowHeavy` → `wi-snow-wind.svg` |
| 7 | 0.26.0 | `RainHeavy` → `wi-rain-wind.svg` |
| 8 | 0.27.0 | `FreezingRainHeavy` → `wi-sleet.svg` |

De facto convention that emerged: **the `Heavy`
suffix triggers specialisation**; lighter intensities
coarsen through the nine-category icons. Every
specialised glyph is visually distinct from its
coarse fallback at e-ink glance distance.

### `classify.rs` split — done (AQ-132, commit `7b0a56f`)

The 876-line `classify.rs` is now
`classify/{mod,weather,compass}.rs`. `mod.rs`
re-exports the public API; `weather.rs` holds the
weather-state taxonomy (Condition now private,
ConditionCategory, WmoCode, WeatherCode,
UnknownWmoCode, classify_weather, classify_category,
and the heuristic constants); `compass.rs` holds
Compass8. The split shipped alongside three
visibility narrowings (AQ-133/134/135) that the
localisation made natural:

- `Condition` and `classify_weather` narrowed from
  `pub` to `pub(super)` — no external consumers
  remained after PR 4's render-path migration.
- `SUNNY_CEILING_PCT` / `CLOUDY_FLOOR_PCT` narrowed
  from `pub` to `pub(super)`. `RAIN_THRESHOLD_MMH`
  stays `pub` (consumed by
  `model::build::day_category`).
- `Condition::label` removed — dead code after
  `ConditionCategory::label` replaced it on the
  render path.

### Possible PR 9+ — further icon specialisation

The "Heavy variant → distinct glyph" convention is
exhausted for the current WMO 4677 subset. Any future
specialisation would be in one of these directions,
each worth deliberate scoping rather than a reflexive
"one more arm":

- **Intensity gradient** (e.g., `DrizzleLight` /
  `DrizzleModerate` / `DrizzleDense` all getting their
  own glyph instead of all coarsening to
  `wi-sprinkle.svg`). Would need three new files and
  three new arms at once — breaks the PR-5-onward
  "one file per PR" cadence.
- **Day/night variants** (e.g., `wi-day-cloudy.svg`
  vs `wi-night-cloudy.svg`). Requires the model to
  carry "is it day?" state, which doesn't exist yet.
- **Visual distinction for fog variants**
  (`RimeFog` vs plain `Fog`). Questionable user
  value — the difference is unlikely to be legible
  on the e-ink panel at forecast-tile size.

The Artisan's PR 8 review flagged **PR 10–11 as
the natural point to revisit `match` vs. lookup
table vs. declarative macro** for specialised arms.
Not a concern today at four arms, but if PR 9+ adds
several more, the `PINNED_SHA256` + `BUNDLED_ICONS`
pair starts feeling like "two places that must
agree" and a single `SPECIALISED_ICON!` declaration
could collapse them. Don't do this preemptively —
the current manual shape produces zero drift bugs
and each PR diff is trivially local.

### Mechanics for any future specialised-icon PR

The recipe that emerged from PR 5–8 and stayed
stable across all four:

1. Fetch the upstream SVG verbatim from
   `https://raw.githubusercontent.com/erikflowers/weather-icons/master/svg/`.
   Place under `crates/bellwether/assets/icons/weather-icons/`.
2. Add a `const FOO_RAW: &str = include_str!(...)` in
   `dashboard/icons.rs` with a docstring naming the
   upstream glyph, the `WmoCode` it serves, and the
   coarse fallback it visibly differs from.
3. Add an arm **above** the catch-all in `icon_for_wmo`:
   `WmoCode::Foo => skip_to_svg_root(FOO_RAW)`.
4. Add the SHA-256 pin (`sha256sum < the-file`) to
   `PINNED_SHA256` and the `(filename, BYTES)` row to
   `BUNDLED_ICONS` in the test module.
5. Reclassify the `WmoCode` variant in `dispatch_kind`
   from `Coarsened` to `Specialised` (add it to the
   `or`-pattern in the Specialised arm, remove it
   from the Coarsened arm). Compile error until the
   arm count matches.
6. Add a row to the "Detailed-fidelity icons" section
   of `assets/icons/weather-icons/README.md`.
7. `cargo xtask validate` — the compile-time forcing
   functions catch mechanical mistakes before AI
   review sees the diff.
8. Deploy to `malina` and eyeball. If the code matches
   real weather, visible on the panel within ~5 min.

## Open decisions / caveats for the next agent

1. **New Weather Icons additions must pin a SHA-256,
   update `BUNDLED_ICONS`, and reclassify in
   `dispatch_kind`.** The
   `bundled_icons_match_pinned_sha256` test enforces
   byte-identity with upstream. The
   `icon_for_wmo_respects_its_dispatch_classification`
   test enforces that every `WmoCode` is either
   Specialised (arm exists) or Coarsened (falls through)
   — nothing in between. Both tests are compile-time
   forcing functions for the mechanical steps above.
2. **`bellwether::licenses::ALL` must grow with
   every new bundled asset.** If a future PR bundles
   icons from a second upstream source (e.g.
   Meteocons as a fallback set) its license text
   must be wired into the `ALL` registry so
   `/licenses` surfaces it. The
   `every_bundle_has_a_non_empty_license_entry` test
   catches empty entries but not missing ones — you
   have to remember.
3. **`WmoCode::ALL` is the single source of truth.**
   Adding a variant to `WmoCode` requires adding it
   to `ALL`, to `TryFrom<u8>`'s table, to
   `coarsen()`'s exhaustive match, to `dispatch_kind`,
   and to the `coarsen_follows_handoff_mapping_exhaustively`
   test's pair table. The compiler catches four of
   those five (exhaustive matches + length assertion);
   only `ALL` itself is human memory — a new variant
   not listed there silently drops from every
   iteration-based test.
4. **`Renderer::new()` disables anti-aliasing at the
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
