# Development Diary

This diary tracks functional changes to the codebase in
reverse chronological order.

---

### 2026-04-21

- Split `classify.rs` into
  `classify/{mod,weather,compass}.rs` + narrowed dead
  public surface (no version bump)

    AQ-132 finally shipped. The old 876-line
    `dashboard/classify.rs` became three files under a
    `classify/` directory: `mod.rs` carries the
    orientation doc and `pub use` re-exports; `weather.rs`
    holds the weather-state taxonomy (Condition,
    ConditionCategory, WmoCode, WeatherCode,
    UnknownWmoCode, classify_weather, classify_category,
    private condition_to_category, the three heuristic
    constants, and all weather tests); `compass.rs`
    holds Compass8 and its tests.

    Pure mechanical split — no semantic changes, every
    test moved verbatim with identical assertions. The
    `pub use` block in `classify/mod.rs` keeps the
    external path `crate::dashboard::classify::X`
    working untouched for callers.

    The post-split review surfaced three "narrow
    visibility now that it's localised" findings
    (AQ-133/134/135) all fixed in-PR:
    - `Condition` and `classify_weather` narrowed to
      `pub(super)` — no external consumers remained
      after PR 4's render-path migration, but their
      visibility still said "public API." Matching the
      docstring's "legacy, no longer reaches render
      path" wording to the actual reach.
    - `SUNNY_CEILING_PCT` and `CLOUDY_FLOOR_PCT`
      narrowed to `pub(super)` — knobs for the now-
      private heuristic. `RAIN_THRESHOLD_MMH` stays
      `pub` because `model::build::day_category`
      genuinely consumes it externally.
    - `Condition::label` removed entirely (dead code
      after the render-path swap to
      `ConditionCategory::label`).
    - `classify/mod.rs` doc thinned from a duplicated
      full-taxonomy bullet list to a one-paragraph
      orientation, letting `weather.rs`'s module doc
      carry the detail as single source of truth.

    AQ-136 (the Artisan's "should WmoCode be a third
    submodule?" question) answered as "no" — the
    coupling between WmoCode, ConditionCategory, and
    classify_weather is too tight to benefit from
    further subdivision.

- Bundled `wi-sleet.svg` as the fourth specialised
  detailed-fidelity glyph — closes the HANDOFF's
  PR 5–8 icon sequence (v0.27.0)

    PR 8 of the WMO-icon sequence. Adds the upstream
    sleet glyph (byte-identical, SHA-256-pinned as
    `beddfdc…`) and wires `WmoCode::FreezingRainHeavy`
    to it. The sleet shape carries the "cold
    precipitation, watch the roads" signal a dashboard
    reader actually uses for freezing rain, which
    reads as plain rain at a meteorological level but
    behaves like ice operationally. Lighter freezing
    variants still coarsen through the plain
    `wi-rain.svg`, matching the "only heaviest gets
    specialised" precedent from PR 5/6/7.

    Third consecutive review-clean PR. Artisan added
    two forward-looking notes worth preserving: (a) the
    constant-name vs. filename tension
    (`FREEZING_RAIN_HEAVY_RAW` naming the WmoCode
    while the file is `wi-sleet.svg`) is a deliberate
    feature — constants describe the semantic role,
    filenames preserve upstream provenance for the
    hash-pin grep loop. (b) The four-arm match +
    manual six-step recipe is still visibly cheap; the
    natural point to revisit macro-vs-table is
    PR 10–11 if the pattern keeps extending.

    Specialised arms now cover: `ThunderstormHailHeavy`
    → `wi-hail`, `SnowHeavy` → `wi-snow-wind`,
    `RainHeavy` → `wi-rain-wind`, `FreezingRainHeavy`
    → `wi-sleet`. The "Heavy suffix → distinct glyph"
    convention emerged organically from PR 5 onward and
    is now the de-facto contract for specialisation.

- Bundled `wi-rain-wind.svg` as the third specialised
  detailed-fidelity glyph (v0.26.0)

    PR 7 of the WMO-icon sequence. Adds the upstream
    wind-driven rain glyph (byte-identical,
    SHA-256-pinned as `053048e7…`) and wires
    `WmoCode::RainHeavy` to it via one `match` arm in
    `icon_for_wmo`. Slight / Moderate / Showers /
    Freezing rain variants still coarsen to the plain
    `wi-rain.svg`; only the heaviest reading escalates
    to the wind-driven shape. Exact parallel to PR 6's
    `wi-snow-wind.svg` for `SnowHeavy`.

    Second consecutive review-clean PR — no findings
    from either reviewer. The PR 5 scaffolding
    (exhaustive `dispatch_kind`, SHA-256 pin table,
    BUNDLED_ICONS registry) really is doing its job:
    the mechanical recipe has enough compile-time
    forcing functions that it's hard to do wrong, and
    craftsmanship review has nothing to object to.

- Bundled `wi-snow-wind.svg` as the second specialised
  detailed-fidelity glyph (v0.25.0)

    PR 6 of the WMO-icon sequence. Adds one upstream
    Weather Icons SVG (byte-identical, SHA-256-pinned
    as `fa3556e4…`), one `match` arm in
    `icon_for_wmo` for `WmoCode::SnowHeavy`, and one
    classification flip in `dispatch_kind` (SnowHeavy
    moves from Coarsened to Specialised via an
    `or`-pattern with ThunderstormHailHeavy). Slight,
    Moderate, Grains, and Showers variants still
    coarsen through the plain `wi-snow.svg` — only the
    heaviest reading escalates to the wind-driven
    glyph, matching the pattern established in PR 5
    where only the heaviest hail variant got a
    distinct arm.

    Clean review: both red team and artisan returned
    "no issues found" — first review-clean PR this
    session. The compile-time forcing functions added
    in PR 5 (exhaustive `dispatch_kind` match, SHA-256
    pin table, BUNDLED_ICONS registry) leave almost
    no room for a mechanical specialised-icon bundle
    to go wrong.

- Gitignored the Playwright session scratch files and
  dropped the now-stale "Working-tree scratch" note
  from HANDOFF.md.

### 2026-04-20

- Disabled anti-aliasing at the rasteriser for a
  pure-bilevel pipeline (v0.24.0)

    The supersample + downsample approach in the
    earlier v0.23.2 attempt didn't help on hardware —
    actually made things worse for thin strokes, because
    averaging a 1-pixel line at 2× resolution produces
    a 50%-grey band that FS then dithered into speckle.
    Reverted v0.23.2 and went surgical instead:
    set `usvg::Options::shape_rendering =
    ShapeRendering::CrispEdges` plus `text_rendering =
    TextRendering::OptimizeSpeed` in the `Renderer`
    constructor. resvg honours those settings per-path
    (`path.rs: paint.anti_alias =
    path.rendering_mode().use_shape_antialiasing()`),
    and usvg flattens text glyph paths with the same
    rendering mode (`text::flatten::resolve_rendering_mode`
    maps OptimizeSpeed → CrispEdges). Result: the
    grayscale buffer contains only pure 0 and pure 255.
    Floyd-Steinberg has nothing to diffuse. The e-ink
    output is pixel-exact bilevel.

    Trade-off is pixel-level staircase aliasing on
    diagonals. At ~150 DPI glance distance on the
    TRMNL-OG panel, that's a much smaller perceptual
    cost than the AA-and-diffuse shimmer we were
    paying for.

    New test
    `rasteriser_produces_bilevel_luma_with_no_intermediate_greys`
    rasterises a 2-pixel-wide diagonal line and
    confirms every luma sample is either 0 or 255.
    Locks the invariant against a future settings
    change or upstream resvg behaviour shift.

- Killed edge-shimmer on the e-ink panel with a
  pre-threshold snap in the Floyd–Steinberg dither
  (v0.23.1)

    Hardware verification on `malina` showed visible
    shimmer on both text and icon curves. Root cause
    was the AA edges from `resvg`'s rasterizer —
    thousands of ~90%-grey pixels at every glyph border
    were each pushing small errors into the FS
    diffusion, and the resulting pattern stacked into a
    perceptible buzzing texture across the panel.

    Fix: before the diffusion loop, snap any pixel with
    value ≤ `SNAP_BLACK_AT` (51 = 20% grey) to 0, and
    any pixel ≥ `SNAP_WHITE_AT` (204 = 80% grey) to 255.
    The dashboard's source material is pure
    black-on-white (Weather Icons SVG paths + Source
    Sans 3 glyphs), so sub-extreme greys are by
    definition AA noise, not intentional content.
    Snapping them contributes zero error to the FS
    loop; only genuine midtones (if any future icon
    ships one) still dither.

    Also fixed a latent v0.16.0 deploy regression:
    `cargo xtask deploy` didn't sync the systemd unit
    file, so the stale `--frontend` CLI flag on
    `malina`'s installed unit crash-looped the service
    on the first post-v0.16.0 deploy that actually
    noticed. `install()` now runs `sync_service_unit`
    which ships the unit file only when it differs from
    what's installed, then `daemon-reload`s.

- Bundled `wi-hail.svg` as the first specialised
  detailed-fidelity glyph (v0.23.0)

    PR 5 of the WMO-icon sequence. Adds one upstream
    Weather Icons SVG (`wi-hail.svg`, SHA-256 pinned),
    one `match` arm in `icon_for_wmo` (`WmoCode::ThunderstormHailHeavy
    → skip_to_svg_root(HAIL_RAW)`), and one README row
    under a new "Detailed-fidelity icons (per `WmoCode`)"
    section.

    **Structural tests.** Replaced the PR 4 mirror-pair
    (`icon_for_wmo_falls_back_to_coarsened_category` +
    `specialised_wmo_arms_diverge_from_the_coarse_category`)
    with a single unified
    `icon_for_wmo_respects_its_dispatch_classification`
    backed by an exhaustive `fn dispatch_kind(WmoCode)
    -> {Specialised, Coarsened}`. Adding a new `WmoCode`
    variant is now a compile error until classified —
    the reviewer-enforced lists that AQ-3/AQ-141 flagged
    are gone.

    **Behavioural test** (deferred from PR 4): two
    single-widget layouts rendered independently under
    Simple vs Detailed, compared byte-wise with
    `assert_ne!`. No coupling to any upstream SVG byte
    pattern — the original form used a `"M4.64,16.9"`
    substring that turned out to be shared with
    rain/snow/sprinkle glyphs (RT-116).

    **Six review findings fixed in-PR.** RT-116
    (fragile substring) + AQ-139/140 (same issue from
    the craftsmanship angle) all resolved by the
    test rewrite. AQ-141 (exhaustive dispatch_kind
    match), AQ-142 (stabilised inline comment), AQ-143
    (restored `Thunderstorm` to the fallback canary
    test) landed as separate targeted fixes.

- Threaded `ConditionCategory` through the presentation
  model + reintroduced `Option<Fidelity>` on the
  weather-icon widget (v0.22.0)

    PR 4 of the WMO-icon sequence sketched in HANDOFF.md.
    Builds on v0.21.0's two-tier taxonomy (`WmoCode` +
    `ConditionCategory`) by finally wiring the new
    taxonomy into the render path, and deletes the
    deprecated `Condition::to_category` bridge now that
    nothing reads it.

    **Model shape.** `CurrentConditions` and `DaySummary`
    used to carry both `condition: Condition` (the
    four-variant legacy) and `weather_code:
    Option<WeatherCode>`. Two fields meant every
    consumer had to pick a precedence rule, and tests
    deliberately built contradictions between the two
    (`Condition::Rain` next to `WmoCode::Thunderstorm`).
    Collapsed to a single `category: ConditionCategory`
    computed via `classify_category` at build time,
    plus `weather_code` retained only for the detailed
    dispatch. `Condition` no longer touches the model.

    **Fidelity.** Reintroduced the `Fidelity { Simple,
    Detailed }` enum and a `fidelity: Option<Fidelity>`
    field on `WidgetKind::WeatherIcon`. Chose
    `Option<Fidelity>` over `Fidelity` + `#[serde(default)]`
    so `layout.toml` round-trips losslessly — a reviewer
    can tell "author wrote `fidelity = \"simple\"`" from
    "author forgot the field". Dispatch unwraps to
    `Simple` at the render site via
    `fidelity.unwrap_or_default()`.

    **Render signature.** `render_weather_icon` now
    takes `(bounds, &DayView, Option<Fidelity>)` instead
    of three positional options. The struct is the
    contract — swap-at-call-site bugs become
    impossible.

    **Condition label.** Added `ConditionCategory::label`
    ("Sunny"/"Partly cloudy"/… for all nine variants).
    Replaces the four-variant `Condition::label` on the
    render path; `Condition` still exists to back the
    numeric heuristic's output inside
    `classify_category`'s fallback branch.

    **Five artisan findings (AQ-134..AQ-138) all fixed
    in-PR.** The big refactor (AQ-135) subsumed AQ-134
    (duplicate dispatch) and AQ-138 (public migration
    bridge) by construction — `resolve_category` went
    away and `condition_to_category` is now private.

- Plumbed WMO `weather_code` end-to-end with a
  9-variant coarse display taxonomy (v0.21.0)

    Three logical PRs bundled into one commit per the
    PR sequence sketched in HANDOFF.md:

    1. **PR 1 — `weather_code` plumbing.** Open-Meteo's
       `weather_code` hourly field is now in
       `WeatherSnapshot`, narrowed at the adapter
       boundary so non-integer / out-of-byte-range
       values become `None` (wire noise) and in-byte
       codes outside the WMO 4677 subset are preserved
       distinctly — see PR 2 for why.
    2. **PR 2 — Two-tier taxonomy.** New
       `WmoCode` enum (28 variants, `#[repr(u8)]`) +
       `TryFrom<u8>` + `coarsen() -> ConditionCategory`
       (9 variants) + `From<WmoCode> for u8`. New
       `WeatherCode { Wmo(_), Unrecognised(u8) }` at
       the boundary preserves the distinction between
       "provider sent nothing" and "provider sent a
       code we don't recognise", so the composite
       `classify_category` can surface
       `ConditionCategory::Unknown` for the latter
       rather than collapsing both cases into the
       cloud+precip fallback. `WmoCode::ALL` is the
       single source of truth for "documented
       variants", consumed by both classify tests and
       icon tests. `Condition::to_category` marked
       `#[deprecated]` as a temporary bridge.
    3. **PR 3 — Nine-icon dispatch.** Bundled five new
       Weather Icons SVGs (`wi-fog`, `wi-sprinkle`,
       `wi-snow`, `wi-thunderstorm`, `wi-na`) verbatim
       from upstream with SHA-256 pins. New
       `icon_for_category(ConditionCategory)` covers
       the mandatory 9 categories; `icon_for_wmo`
       dispatches through `coarsen` with specialised
       arms landing in PR 4+. The existing render
       path routes `Condition` → `ConditionCategory`
       via the deprecated bridge, exercising the new
       dispatch with current data without touching the
       model.

    Two structural review findings (unreachable
    `Unknown` variant + silently-ignored `Fidelity`
    field) surfaced by both reviewers resolved
    in-PR: the former by plumbing
    `WeatherCode::Unrecognised` through the boundary
    so `Unknown` becomes reachable; the latter by
    removing `Fidelity` entirely from this PR so PR 4
    can reintroduce it coupled to the renderer
    change.

- Closed review gaps from the Weather Icons swap
  (v0.20.1)

    Post-commit review of `6a867a8` surfaced 15
    findings across red-team + artisan reviewers. All
    15 fixed in this follow-up — largest three were:

    1. **OFL §2 binary-redistribution gap.** The
       swap commit embedded the SVG bytes via
       `include_str!` but not the LICENSE text, so
       every `cargo xtask deploy` push shipped Font
       Software to `malina` without the accompanying
       license. New `bellwether::licenses` module
       embeds both the Weather Icons LICENSE and the
       Source Sans 3 font README; new
       `GET /licenses` endpoint on `bellwether-web`
       serves them as `text/plain` (exempt from the
       access-token middleware so they stay publicly
       reachable per §2's "easily viewed by the user"
       clause).
    2. **`each_icon_renders_visible_pixels` test
       regression.** The swap flipped the existing
       test from `fill="black"` presence (weak but
       non-trivial "icon will paint something"
       invariant) to `<svg` + `<path` presence —
       satisfied by a `<path fill="none">` that would
       render blank. Replaced with a check that no
       bundled SVG contains `fill="none"`, combined
       with the SVG-spec default of black fill.
    3. **4-for-1 API bundle.** Folded
       `strip_xml_prolog` into `icon_for` so the
       latter returns a pre-stripped slice directly.
       One change resolved: `pub` → private (RT-D +
       AQ-A), rename to `skip_to_svg_root` (AQ-B),
       opaque return type (AQ-C), and the footgun of
       forgetting to pair the two calls.

    Other fixes: harden `skip_to_svg_root` to step
    past XML PI / comment spans sequentially so a
    future upstream comment containing `<svg>` can't
    slice mid-comment; panic loudly on missing root
    (compile-time constants mean malformed bundled
    asset = build bug, not runtime); soften the
    LICENSE header to drop fabricated date range and
    Reserved Font Name claim; add `<!DOCTYPE` /
    `<!ENTITY` rejection test for defence-in-depth;
    add SHA-256 pin test backing the "byte-identical
    to upstream" README claim (required `sha2 = "0.10"`
    in `[dev-dependencies]`); add OFL §5 Reserved
    Font Name warning to the bundled README; fix
    whitespace leak in `render_weather_icon`'s format
    string.

- Replaced hand-rolled weather-icon primitives with
  Weather Icons (Erik Flowers) (v0.20.0)

    The previous icons in
    `crates/bellwether/src/dashboard/icons.rs` were
    ~4-10 circles + rectangles per variant, authored
    in a 48-user-unit coordinate space. At display
    size (~96×96 px) the filled-silhouette approach
    made `Sunny`, `PartlyCloudy`, `Cloudy`, and `Rain`
    hard to tell apart — the user flagged them as
    "too pixelated and hard to distinguish".

    Swapped to verbatim SVG files from
    [Weather Icons](https://erikflowers.github.io/weather-icons/)
    (SIL OFL 1.1). Bundled under
    `crates/bellwether/assets/icons/weather-icons/`:
    `wi-day-sunny.svg`, `wi-day-cloudy.svg`,
    `wi-cloudy.svg`, `wi-rain.svg`. Pinned
    byte-for-byte to the upstream `master/svg/` tree.

    Consequence: the icon constants now carry a full
    `<svg viewBox="0 0 30 30">…`, so the 48-unit
    fragment convention had to go. `render_weather_icon`
    in `dashboard/svg/mod.rs` now emits
    `<svg x=".." y=".." width=".." height="..">…inner
    document…</svg>` — resvg's nested-SVG semantics
    scale the inner viewBox to fill the outer box.
    Cleaner than a `<g transform="translate scale">`
    wrap and survives icon sources that declare
    non-30-unit viewBoxes.

    Added `strip_xml_prolog` helper: upstream files
    start with `<?xml …?>` + generator comments, which
    are illegal as children of an outer `<svg>`. The
    helper slices from the first `<svg` occurrence.
    Kept the upstream files byte-identical so future
    `/template-sync`-style bulk refreshes don't fight
    hand-edits.

    License bundling: upstream declares SIL OFL 1.1
    in its README but doesn't ship the text itself
    (probed `master/LICENSE*`, `OFL.txt`, all 404).
    Pulled the canonical OFL.txt from
    `openfontlicense.org`, customised the header with
    Erik Flowers' copyright (2013-2015), and committed
    next to the SVGs with a `README.md` documenting
    provenance — closes the §2 "bundled and
    redistributed" condition.

    Deferred to later PRs per HANDOFF: the 9-variant
    `ConditionCategory` taxonomy, per-instance
    `fidelity` widget setting, and detailed
    WMO-specific icons. Today's commit is the icon
    source swap only; the existing 4-variant
    `Condition` enum is unchanged, so the dashboard
    layout and widget API are untouched.

- Added `cargo xtask preview` for rendered-dashboard
  iteration loop (v0.19.0)

    New xtask subcommand that regenerates the sample
    dashboard via the existing
    `publish::tests::generate_dashboard_sample` ignored
    test and serves a three-panel HTML viewer on
    `127.0.0.1:8123`. The panels show, top-to-bottom:
    the raw SVG, a `resvg` raster PNG before dither,
    and the final 1-bit BMP. Seeing all three
    side-by-side makes it possible to isolate a visual
    regression to the SVG layout, the rasteriser, or
    the Floyd–Steinberg dither in one glance rather
    than diffing BMPs. Flags: `--port N` (default
    8123), `--open` (off by default — opt-in browser
    launch to stay SSH-safe).

    Split the renderer at a new seam to enable the
    middle panel: factored out a private
    `Renderer::rasterize` helper that runs the
    usvg-parse + resvg-render stages and returns a
    `tiny_skia::Pixmap`. The existing `render_to_bmp`
    now composes `rasterize` + grayscale + dither +
    BMP encode, and a new public `render_to_png`
    composes `rasterize` + `pixmap.encode_png()`.
    Both methods share the bit-depth pre-check so
    they reject the same inputs (AQ-E).

    Sample-generator test `generate_dashboard_sample_bmp`
    renamed to `generate_dashboard_sample` and now
    writes all three artefacts (SVG, PNG, BMP) to
    the workspace-root `target/` via
    `CARGO_MANIFEST_DIR`-anchored navigation, so xtask
    can serve them regardless of which crate the test
    ran from.

    Security posture: preview server binds loopback
    only, serves a hardcoded 4-entry filename
    allowlist (`preview-index.html` +
    `dashboard-sample.{svg,png,bmp}`), and never
    exposes `target/` directly (RT-A). Windows
    browser-open path documented with an explicit
    contract comment about the `cmd /C start`
    metacharacter trap (RT-D). Server readiness
    guaranteed before `--open` fires (RT-C).
    Stale-preview failure mode (silent pass-through
    when the ignored test name drifts) caught via
    artefact-mtime verification after the cargo test
    call (AQ-D).

- Swapped dashboard font to Source Sans 3
  Semibold (v0.18.0)

    Replaced Atkinson Hyperlegible (Regular) with
    Adobe's Source Sans 3 at weight 600. User
    wanted a dotted (non-slashed) zero; Source
    Sans 3 Semibold also carries more stroke mass
    than Atkinson Regular, which dithers more
    robustly to 1-bit e-ink. Public constant
    `ATKINSON_HYPERLEGIBLE_TTF` renamed to
    `SOURCE_SANS_3_SEMIBOLD_TTF` (filename-matching
    and weight-disambiguating, per Artisan review
    AQ-125). Added `SOURCE_SANS_3_FAMILY` and
    `SOURCE_SANS_3_WEIGHT` next to the TTF bytes
    so the SVG builder references them instead of
    hardcoding literals (AQ-124) — one place to
    change when the bundled font rolls over.

    Non-obvious wrinkle: svgtypes 0.15.3's
    unquoted `font-family` parser at
    `svgtypes/src/font.rs:53-87` tokenises the
    attribute value as CSS identifiers, which
    cannot start with a digit. `Source Sans 3`
    without quotes errors at the `3`, the parser
    returns `Err`, usvg falls back to the default
    `Times New Roman` (not in our fontdb), and
    all text silently drops. Fix: wrap the family
    in single quotes inside the double-quoted
    attribute — `font-family="'Source Sans 3'"`
    — so the quoted-string branch treats the
    whole value as one family name. Caught by the
    `with_default_fonts_renders_degree_sign_glyph`
    end-to-end test (which asserts >200 black
    pixels after rasterisation); the family-match
    test at `dashboard/svg/tests.rs:225` asserts
    the SVG emits exactly the typographic family
    name (name ID 16) as the TTF advertises.

- Split compound weather widgets into atomic
  widgets (v0.17.0)

    The old compound widgets —
    `current-conditions` (icon + big temp +
    condition label + feels-like), `forecast-day`
    (day name + icon + H/L line per tile), and
    `today-hi-lo` (combined high/low footer item)
    — positioned their sub-elements by absolute
    pixel offsets inside the compound bounds,
    which made it impossible to re-arrange any
    sub-element without touching the renderer.
    Replaced with seven atomic widgets:
    `weather-icon`, `temp-now`, `condition`,
    `feels-like`, `day-name`, `temp-high`,
    `temp-low`. Each atomic widget centres itself
    inside its own `Rect` and auto-sizes its font
    to the bounds' height, so emphasis is now a
    pure function of the split tree in
    `layout.toml`.

    Weather-domain widgets take a `day` selector:
    `day = "today"` reads from `CurrentConditions`
    + `TodaySummary`, numeric `day = N` reads
    forecast offset N. `temp-high`/`temp-low`
    accept an optional `label` prefix so a tile
    can render `"H 12°"` without needing a
    separate label widget. `feels-like` and
    `temp-now` are today-only because the forecast
    model has no corresponding per-day datum.

    Default `assets/layout.toml` rewritten to
    compose the same visual from atomic widgets +
    nested splits. The compound variants are
    removed from `WidgetKind` entirely — any user
    `[dashboard]` TOML using the old names will
    fail to parse. SemVer minor bump because the
    breakage is limited to the layout DSL, which
    is documented as a user-facing configuration
    surface but not a stable API.

- Fix landing-page preview `<img>` (v0.16.1)

    The landing page pointed its preview `<img>` at
    `/api/display?preview=1`, but `/api/display`
    returns a JSON manifest (and is gated behind the
    access token when one is configured) — so the
    browser always fell through to the "no image
    yet" fallback. Added an unauthenticated
    `GET /preview.bmp` that streams the latest
    rendered BMP directly from the in-memory
    `ImageStore`, and repointed the `<img>` at it.
    The new handler uses a single atomic read via
    `ImageStore::latest_image()` so the composite
    lock invariant (never advertise a filename whose
    bytes are absent) stays local to the store, sets
    `Cache-Control: no-store` so the static URL
    always fetches the newest render, and returns
    `404` (not `503`) when no image has been
    produced yet so the `<img>` onerror handler
    fires immediately. Logged RT-113 tracking the
    information-disclosure tradeoff of an
    unauthenticated preview route.

- Drop Svelte frontend; replace `/` with hand-rolled
  HTML landing page (v0.16.0)

    The Svelte/Vite scaffold and Playwright E2E setup
    were leftover template material that never
    graduated to a real UI. The only endpoint that
    actually needed the frontend build was `/` serving
    `index.html`. For a server whose job is to render
    BMPs for an e-ink device, an SPA was pure overhead.

    Replaced with a hand-rolled HTML landing page
    (`fn landing_page() -> Html<String>` in
    `bellwether-web/src/api/mod.rs`) that lists the
    available endpoints (`/health`, `/api/status`,
    `/api/display`, `/api/setup`, `/api/log`,
    `/images/*`) and embeds the latest rendered
    dashboard. Styled with ~40 lines of inline CSS —
    light/dark aware, no external assets.

    Deleted: `frontend/`, `e2e/`, `playwright.config.ts`,
    `scripts/e2e.sh`, root `package.json`, root
    `tsconfig.json`, `xtask::frontend_check`,
    `bellwether-web --frontend <path>` CLI flag,
    `tower-http` `fs` feature (no more `ServeDir` /
    `ServeFile`), `/api/greeting` scaffold endpoint.

    `cargo xtask validate` drops from 6 steps to 5 (no
    more frontend type-check). The RPi deploy script
    no longer runs `npm run build` or scp's
    `frontend-dist`; `cargo xtask deploy` is
    4-step now. Systemd unit lost its `--frontend`
    arg.

    If a real admin UI shows up on the backlog later,
    HTMX + server-rendered templates fit this
    project's size better than a full SPA.

- Inline layout config under `[dashboard]` (v0.15.0)

    The layout can now be declared inline in the main
    `config.toml` as a `[dashboard]` section with the
    canvas and root-node fields as siblings:

    ```toml
    [dashboard]
    canvas = { width = 800, height = 480 }
    split = "vertical"
    divider = true

    [[dashboard.children]]
    size = 50
    # ...
    ```

    Achieved by flattening `Layout.root` into the outer
    struct via `#[serde(flatten)]`, so users don't see
    a superfluous `[dashboard.root]` wrapper. The
    standalone `assets/layout.toml` uses the same
    shape (no leading section header). When
    `[dashboard]` is absent,
    `Config::dashboard_layout()` falls back to
    `Layout::embedded_default()`.

    The layout is validated at `Config::load` time via
    `layout.resolve()` → any `Overflow` / `EmptySplit`
    / arithmetic-overflow is rejected as
    `ConfigError::InvalidDashboardLayout` at startup,
    and `Layout::embedded_default` now calls
    `.resolve()` inside its `OnceLock` init so a
    broken embedded asset also surfaces at startup.

    `PublishLoop::new` takes a new `PublishLoopConfig
    { render_cfg, layout, interval }` struct (reducing
    it from 6 positional args to 4), and `tick_once`'s
    render path now propagates `LayoutError` as
    `PublishError::Layout` — `run()` logs it at warn
    and skips the tick rather than panicking.

- Configurable widget layout via `layout.toml` (v0.14.0)

    Replaced the hardcoded 5-band SVG builder with a
    data-driven layout system. Dashboard structure now
    lives in `crates/bellwether/assets/layout.toml` as
    a recursive tree of splits (horizontal / vertical,
    optional divider) and strongly-typed widgets
    (`brand`, `header-title`, `clock`, `battery`,
    `current-conditions`, `wind`, `gust`, `humidity`,
    `forecast-day`, `today-hi-lo`, `sunrise`, `sunset`).
    Children declare sizing as `size = N` (fixed px) or
    `flex = N` (weighted share); the parser rejects
    `flex = 0`, both, or neither at TOML load time so
    invalid states can't reach the resolver.

    `Layout::resolve` walks the tree into a `Resolved`
    struct (widget placements + divider placements),
    the SVG builder dispatches each placement to a
    bounds-relative widget render fn. `SplitNode.divider
    = true` is now the single source of truth for
    between-children lines — no more hardcoded
    `section_dividers` / `meteo_column_separators`.

    All resolver arithmetic is `u64`-internal with
    `checked_add` / `checked_mul` so pathological user
    values can't wrap past the `Overflow` check.
    `<text>` content is XML-escaped, so `HeaderTitle`
    strings with `&` / `<` / `>` produce well-formed
    SVG. `build_svg_with_layout` returns
    `Result<String, LayoutError>` for user-supplied
    layouts; `build_svg` keeps its panic-free signature
    via the test-guaranteed embedded default.

- Cap `ImageStore` retention at 4 (v0.13.1)

    The in-memory BMP store grew without bound: every
    `put_image` inserted into a `BTreeMap` and nothing
    ever evicted. At the default 5-minute refresh that
    accumulated ~13.5 MB/day, heading for OOM-kill under
    `MemoryMax=512M` in roughly a month. The TRMNL BYOS
    protocol only needs the current image; keeping a
    small tail covers the race window between a device's
    `/api/display` poll and its subsequent image fetch.
    New `MAX_RETAINED_IMAGES = 4` constant; eviction
    sweeps the oldest key while guarding against
    dropping the current `latest`.

- Deploy to Raspberry Pi + `/api/setup` endpoint (v0.13.0)

    Bellwether now runs on `malina` as a hardened
    systemd service. Ported hoard's build-on-RPi
    deploy mechanism: `cargo xtask deploy-setup` for
    one-time provisioning (creates the `bellwether`
    system user, copies `config.toml`, installs the
    unit), `cargo xtask deploy` for repeatable
    deploys (tar source → scp → remote cargo build
    with persisted `target` cache → atomic binary +
    frontend swap → service restart with
    `reset-failed` guard). No cross-compile
    toolchain needed locally. Setup and deploy
    functions use `anyhow::Result` so ssh/scp error
    source chains survive through to the CLI.

    Added `GET /api/setup` — the fourth TRMNL BYOS
    endpoint, which a factory-fresh device hits on
    first boot to exchange its MAC for an `api_key`
    and `friendly_id`. Exempt from the
    `Access-Token` middleware (a fresh device has
    none). Returns 503 when no image has been
    rendered yet, matching `/api/display`'s
    contract. `FriendlyId` newtype carries the
    6-char-uppercase-hex format invariant.
    `DEFAULT_UNCONFIGURED_API_KEY` documents the
    no-auth-mode placeholder and the factory-reset
    caveat when the operator later enables auth.

    Split `trmnl/mod.rs` into `mod.rs` (state +
    store + router) and `handlers.rs` (response
    types, `FriendlyId`, handlers, auth middleware)
    to stay under the 500-line threshold.

    Systemd hardening: `config.toml` staging file
    locked down with `umask 077` + `chmod 600` to
    avoid a brief world-readable window during
    `scp`; `MemoryMax` raised to 512 MiB for BMP
    rendering headroom; `StartLimitIntervalSec` /
    `StartLimitBurst` moved under `[Unit]` where
    modern systemd expects them.

- Migrate weather backend from Windy to Open-Meteo (v0.12.0)

    Replaced the Windy Point Forecast API with
    Open-Meteo behind a `WeatherProvider` trait.
    Windy wanted ~$900/year for this use case; the
    free testing key was returning deliberately
    scrambled data and would have silently poisoned
    the dashboard in production. Open-Meteo is free
    and keyless.

    The migration landed as a single PR stitched
    from seven planned steps (see
    `docs/developer/weather-provider-migration.md`):
    (1) new `crate::weather` with `WeatherSnapshot`
    + `WeatherProvider` trait; (2) Windy → snapshot
    adapter; (3) `dashboard::build_model` takes
    `&WeatherSnapshot`; (4) `PublishLoop` holds
    `Arc<dyn WeatherProvider>`; (5) config
    restructure to `[weather]` +
    `[weather.<provider>]` subtables with a
    `provider` tag; (6) Open-Meteo provider;
    (7) delete Windy, flip default.

    Unit conversion (Kelvin → °C, m/s → km/h,
    u/v → compass degrees) moved out of
    `dashboard::model` into provider adapters —
    the dashboard only sees display units now.
    `Compass8::from_degrees(deg)` replaces the old
    u/v-based `wind_to_compass`. `WeatherSnapshot`
    uses a builder
    (`WeatherSnapshotBuilder::build -> Result<_,
    WeatherError>`) so the
    length-matches-timestamps invariant is
    unskippable at construction.

    Along the way: fixed 18 findings from the
    red-team + artisan review in the same PR —
    notably a DoS window in `read_capped_body`
    (allocated past the cap before checking),
    silent wire-format drift in Open-Meteo's
    response parser, non-finite float propagation
    through `feels_like_c`, a chrono overflow
    panic in `nearest_sample_index`, and the
    addition of `WeatherProvider::location()` so
    `PublishLoop` has one source of truth for the
    forecast point. Extracted
    `clients::http_util` so both providers share
    the body-reading + client-builder code.

    Default `RUST_LOG` filter widened to include
    `bellwether=info` — previously the publish
    loop's `published image` / `publish tick
    failed` log lines were filtered out by the
    binary's default, so a failing fetch looked
    like a missing BMP.

### 2026-04-18

- Dense 5-band dashboard layout (v0.11.0)

    Rewrote `dashboard::svg` to consume the v0.10
    data model and render the dense weather-app-style
    layout the user mocked up: branded header with
    TRMNL / "Weather Report" / clock / battery;
    current-conditions band with icon + big temp +
    condition + feels-like; three-cell meteorology
    strip (wind + gust + humidity); forecast row of
    three tiles (weekday + icon + H/L); footer with
    today's high/low and sunrise/sunset. All text
    goes through one shared `text()` renderer so the
    opening-tag boilerplate lives in exactly one
    place, and the 3-column grid centres are a
    module const reused by the meteo and forecast
    bands.

    Missing-data handling matches the "never show
    fake numbers" project convention: em-dash for
    every optional field when the underlying data is
    `None`, a neutral "No current reading" label
    when the current-conditions panel collapses, and
    "Wind calm" instead of a fake `"Wind N 0 km/h"`
    for calm conditions. Forecast tile placeholders
    still render their weekday header so an operator
    can see *which* day is missing — a new
    `day_weekdays: [Weekday; 3]` field on
    `DashboardModel` keeps the layout labels
    independent of the data rows.

    `build_svg` signature changed: now takes
    `(&DashboardModel, now_local: NaiveTime)` — the
    clock input stays out of the model so a
    rendered model doesn't go stale the instant the
    caller holds it. `publish::tick_once` derives
    `now_local` from the existing `ctx.now` +
    `ctx.tz`.

    Moved `svg.rs` to `svg/mod.rs` + `svg/tests.rs`
    (matching the `model/` split convention so
    neither file is over 500 lines of production
    code).

- Dashboard model groundwork for redesign (v0.10.0)

    No user-visible output change yet — the SVG
    builder is still the v0.9 layout. This commit
    extends the data-model pipeline to carry
    everything the upcoming dense-layout SVG will
    consume:

    - `dashboard::astro` — hand-rolled NOAA
      sunrise/sunset algorithm, no new crate
      dependency. Anchored to local noon on the
      requested local date so the ephemeris is
      always within ±12h of any sunrise/sunset
      event; avoids spurious "polar day/night"
      flips near date-line longitudes at equinox.
      `GeoPoint { lat_deg, lon_deg }` packs the
      coordinates so a swap compiles as an error
      rather than rendering the wrong city.
    - `dashboard::feels_like` — pure
      `apparent_temperature_c` combining NWS heat
      index (above 26.7 °C, ≥ 40 % RH) and wind
      chill (below 10 °C, > 4.8 km/h). NaN-guarded
      fallback to raw temp.
    - `crate::telemetry` — new neutral module
      hosting `DeviceTelemetry` and
      `battery_voltage_to_pct`. Both `publish` and
      `dashboard` depend on it, breaking what used
      to be a mutual `publish ↔ dashboard`
      dependency. `ImageSink` gains a default-method
      `latest_telemetry()` returning all-`None` by
      default. `DeviceTelemetry::merge_from` keeps
      prior field values when a keepalive post
      omits them.
    - `TrmnlState` (web crate) caches the latest
      telemetry behind an `Arc<RwLock<_>>` (matching
      the `ImageStore` convention) and
      `/api/log` merges parsed battery voltages into
      it on every post.
    - `ModelContext` struct unifies `tz`, `location`,
      `now`, and `telemetry` into one `Copy` value
      passed to `build_model`. `TodaySummary`
      adds today's high/low + sunrise/sunset.
      `CurrentConditions` gains `feels_like_c`,
      `gust_kmh`, `humidity_pct`. `DaySummary`
      gains `low_c`. Humidity clamped to `[0, 100]`
      to protect the Rothfusz formula from Windy
      glitch values.
    - Config validation extended to require `rh` +
      `windGust` in `[windy] parameters`; pre-0.10
      configs fail at `Config::load` with a clear
      message.
    - `dashboard/model.rs` split to `model/mod.rs`
      + `model/tests.rs` to stay under the 500-line
      CLAUDE.md threshold.

    63 new unit tests (astro 6, feels_like 11,
    telemetry 7, dashboard model 12, trmnl log 3,
    config 2, +21 test-file moves / extensions).
    All existing tests pass through the new
    `ModelContext` shape.

- Swapped dashboard font to Atkinson Hyperlegible (v0.9.0)

    The m6x11plus pixel font was correct at its
    native 18-px grid but scaled up 10× for the big
    current-conditions temperature looked blocky on
    the 800 × 480 canvas — the TRMNL OG e-ink can
    render a smooth vector font through Floyd-
    Steinberg dither far more crisply. Swapped in
    Atkinson Hyperlegible Regular (Braille
    Institute, SIL OFL): a sans-serif designed for
    character-to-character distinctiveness. The
    slashed zero and wide-aperture lowercase shapes
    come through cleanly in the 1-bit output at every
    size the dashboard uses, from the 36 px wind
    label up to the 180 px current temperature.

    Public API change (breaking):
    `bellwether::render::M6X11_TTF` →
    `ATKINSON_HYPERLEGIBLE_TTF`.
    `Renderer::with_default_fonts()` signature
    unchanged. Font sizes hoisted into named
    constants (`CURRENT_TEMP_PX`,
    `CONDITION_LABEL_PX`, `WIND_LABEL_PX`,
    `DAY_LABEL_PX`, `DAY_HIGH_PX`) so the visual
    hierarchy lives in one place and a typo shows
    up as a compile error instead of at eyeball
    time.

- Real dashboard layout (v0.8.0)

    `bellwether::dashboard` replaces the placeholder
    temperature bar with a current-conditions panel
    (big temp, condition word, wind label) and three
    day tiles (weekday, icon, high) along the bottom.
    Module structure:

    - `classify` — `Condition` (Sunny/PartlyCloudy/
      Cloudy/Rain) and `Compass8` enums; pure
      `classify_weather(cloud_pct, precip_mmh)` and
      `wind_to_compass(u, v)` functions with
      meteorological "wind from" convention.
    - `model` — `DashboardModel`, `CurrentConditions`,
      `DaySummary` structs; `build_model(forecast, tz,
      now)` that handles Kelvin→Celsius, wind u/v →
      km/h + compass, local-date bucketing, partial-day
      threshold (fewer than 6 samples drops the tile),
      and null-temperature handling (`high_c:
      Option<i32>` so the SVG can show an em-dash
      rather than a misleading "0°").
    - `icons` — four hand-drawn 48 × 48 SVG icon
      fragments.
    - `svg` — `build_svg(model)` that emits an 800 × 480
      SVG at integer-multiple-of-18 font sizes (the
      size family m6x11plus is designed for).

    Wiring: `publish::tick_once` passes `Utc::now()`
    through so "current" is the sample closest to
    wall-clock, not `ts[0]` (which can be stale by
    hours depending on Windy's model-run cadence).
    `bellwether-web/main.rs` switched to
    `Renderer::with_default_fonts()` so text
    actually renders.

    Config validation: `parameters` (when non-empty)
    must include temp, wind, clouds, precip — the
    four the v1 dashboard consumes. A pre-0.8 config
    missing `clouds` now fails at load rather than
    silently rendering "Cloudy" forever.

    Tests: 43 new unit tests across classify / model /
    icons / svg, plus an end-to-end test that renders
    the full pipeline at TRMNL OG resolution and
    asserts a 48,062-byte BMP with meaningful black
    coverage. An `#[ignore]`'d
    `generate_dashboard_sample_bmp` writes
    `target/dashboard-sample.bmp` for manual eyeball.

- Bundled m6x11plus pixel font (v0.7.0)

    Added `bellwether::render::M6X11_TTF: &[u8]` as a
    compile-time-embedded font blob and
    `Renderer::with_default_fonts()` as the production
    constructor that pre-loads it. Font is Daniel
    Linssen's m6x11plus — a proportional 6×11 pixel
    font with extended Latin coverage (attribution in
    `crates/bellwether/src/render/fonts/README.md`).
    Covers `U+00B0 °`, verified by an
    iteration-over-full-ranges test rather than
    endpoint spot-checks.

    This is step 1 of PR 3d. The dashboard layout
    itself lands in follow-up commits; this commit
    only bundles the font and wires the renderer
    constructor, leaving the placeholder SVG in place
    for the moment. Isolating the font step means the
    glyph-coverage decision is verifiable on its own
    and future steps don't have to re-argue the font
    choice.

    `ttf-parser` added as a dev-dep (exact-pinned to
    0.25.1, matching fontdb's transitive pull) so the
    glyph-coverage test can check `Face::glyph_index`
    directly rather than black-box rasterizing.

- Fetch → render → publish loop (v0.6.0)

    New `bellwether::publish` module ties the Windy
    client, renderer, and BYOS image store into a
    repeating `tokio::time::interval` task. First tick
    fires immediately; subsequent ticks on the
    configured cadence (shared with the device's
    refresh rate). Per-tick errors log at `warn!` and
    are swallowed so transient Windy / DNS / render
    failures don't kill the loop — the server keeps
    serving the last-good image.

    Dashboard SVG for PR 3c is a placeholder (a bar
    whose width tracks current temperature on a
    0–40 °C scale, with an explicit diagonal X overlay
    when temperature is missing so "no data" is
    distinguishable from a real 0 °C reading). Real
    layout + fonts defer to a later PR.

    Filenames are `dash-{counter:08}.bmp` from an
    `AtomicU64`, avoiding wall-clock collisions and
    negative timestamps on RTC-less Pis. `FetchRequest`
    picked up a manual `Debug` redacting the api_key
    so the key can't leak via a future
    `tracing::debug!(?req, …)`. `Client::fetch` now
    takes `&FetchRequest` so the publish loop doesn't
    clone per tick. `Config::validate` rejects
    `default_refresh_rate_s` outside `1..=86400`
    (zero would have panicked `tokio::time::interval`).
    `publish::supervise` wraps `tokio::spawn` with a
    log-on-exit tripwire — clean return or panic both
    land in the error log rather than vanishing; no
    auto-restart (avoids crash-loop Windy quota burn).
    16 review findings (8 red-team + 8 artisan), 15
    addressed in-PR.

- TRMNL BYOS endpoints on `bellwether-web` (v0.5.0)

    New `api::trmnl` module exposes `GET /api/display`
    (JSON manifest matching TRMNL OG firmware fields),
    `POST /api/log` (telemetry, 16 KiB body cap,
    known fields logged structurally at INFO / extras
    at DEBUG), and `GET /images/{filename}` (zero-copy
    `Bytes` response). `ImageStore` uses a single
    composite `RwLock` so readers never see a filename
    whose bytes aren't yet inserted. Filenames are
    validated at insert time
    (`[A-Za-z0-9._-]{1,128}`) so nothing
    user-controllable can flow into the advertised
    `image_url`. `public_image_base` is validated for
    scheme + no-query at construction. Optional
    `Access-Token` middleware reads
    `BELLWETHER_ACCESS_TOKEN`; absent token emits a
    `WARN` at startup for LAN-only deployments.
    `bellwether-web --config` is now required unless
    `--dev` is passed. `Renderer::placeholder_bmp`
    moved to the library (`crates/bellwether/src/render/
    placeholder.svg` via `include_str!`) so the
    render-loop work in PR 3c can reuse the helper.
    29 review findings from red-team + artisan; 24
    addressed in-PR, 5 deferred to TODO.md (docs only).

- Render pipeline: SVG → 1-bit BMP (v0.4.0)

    `render::Renderer` parses SVG via `resvg`/`usvg`
    (text feature only; no system fonts, no
    raster-image embeds), rasterizes to `tiny-skia`
    RGBA, converts to grayscale via fixed-point
    Rec. 601 (transparent regions composited over
    white), Floyd–Steinberg dithers to 1-bit, and
    emits a monochrome BMP with the TRMNL OG
    firmware's canonical palette (`palette[0] =
    black, palette[1] = white; bit 1 = white`).
    Verified against `usetrmnl/firmware`
    `lib/trmnl/src/bmp.cpp` — matches ImageMagick /
    Pillow defaults and the firmware's `"standart"`
    path. Module split into `bmp.rs`, `dither.rs`,
    `mod.rs`, `tests.rs` (directory layout mirrors
    `config/` and `clients/windy/`).

    Render pipeline rejects pathological inputs: SVG
    viewports that would require scales above 8192 or
    non-finite, render dimensions outside 1..=4096 at
    `Config::load`/`from_toml_str`. Regression test
    locks in that `<image href="file://...">` is
    silently ignored. 12 red-team + 15 artisan
    findings from review; all applicable ones
    addressed in-PR. RT-024 (palette inversion
    concern) specifically verified against firmware
    source and left as-is. Open in the review logs:
    nothing; Cluster D items documented inline and in
    TODO.md.

- Windy Point Forecast client (v0.3.0)

    `clients::windy::{Client, FetchRequest, Forecast,
    WindyError}` — thin transport over reqwest that
    POSTs lat/lon/model/parameters/key to Windy's
    Point Forecast v2 and returns a parsed
    `Forecast` with typed
    `values(WindyParameter)` lookup. `null` values
    preserved as `Option<f64>`; `ts` + series length
    mismatch rejected at parse time; empty `ts`
    returns `EmptyForecast`. Forward-compat
    non-numeric metadata fields in responses are
    silently ignored rather than breaking parsing.
    Security posture: `Policy::none()` on redirects
    (prevents cross-origin key leak on DNS hijack);
    API key redacted from error bodies; per-Client
    body-size caps (4 MiB success, 4 KiB error).
    Added `connect_timeout(5s)` + `gzip` feature on
    reqwest for RPi network realities. `WindyParameter`
    picked up `Serialize` + per-variant renames so
    `windGust` round-trips correctly (it was silently
    emitted as `windgust` before). 30 review findings
    from red-team + artisan — all addressed in PR;
    see `redteam-resolved.md` / `artisan-resolved.md`.

- Design spike + config skeleton (v0.2.0)

    Closed the five open questions flagged in
    `HANDOFF.md`: TRMNL OG 7.5" @ 800×480 1-bit; **BYOS**
    (device polls our server) as the v1 integration
    target; Webhook Image plugin kept as the fallback;
    render stack = `resvg` (SVG → RGBA) + `image`
    (grayscale + Floyd–Steinberg dither + 1-bit BMP).
    Design decisions captured in
    `docs/developer/spike.md`. Home Assistant
    integration moved to the backlog at the user's
    request — PR 1 covers Windy + TRMNL + render only.

    Config module lives under `crates/bellwether/src/config/`
    (split into `mod`, `windy`, `trmnl`, `render`).
    `Config::load(impl AsRef<Path>)` parses the TOML,
    resolves `api_key_file` against the config file's
    directory, validates lat/lon range, reads the Windy
    secret eagerly, and caches it on `WindyConfig`
    (redacted in `Debug`). `Config::from_toml_str` is
    a disk-free entry point for tests and preview
    flows. `TrmnlConfig` is an internally-tagged enum
    so `mode = "byos"` cannot coexist with missing
    BYOS fields — illegal states are unrepresentable.
    Strong types for `WindyParameter`, `BitDepth`, and
    `timezone: chrono_tz::Tz`. Red-team + artisan
    reviews ran in parallel; all 23 findings (bar one
    noted exception) landed in this PR — see
    `redteam-resolved.md` / `artisan-resolved.md`.

### 2026-04-16

- Scaffold from rustbase template (v0.1.0)

    Generated from [rustbase](https://github.com/breki/rustbase)
    at commit `076cf44` (template v0.4.0). Renamed crates
    from `rustbase` / `rustbase-web` to `bellwether` /
    `bellwether-web` and updated all references (workspace
    config, binary names, release workflow, dev scripts,
    Claude Code skills, CI). Reset project-tracking files
    (`CHANGELOG`, diary, red-team / artisan logs,
    template-feedback) to a fresh v0.1.0 starting point.
    `.template-sync.toml` points at the 076cf44 baseline
    so future `/template-sync` runs can pull upstream
    improvements.
