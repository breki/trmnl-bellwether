# Development Diary

This diary tracks functional changes to the codebase in
reverse chronological order.

---

### 2026-04-17

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
