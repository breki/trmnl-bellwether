---
name: architect
description: >
  Project overview, structure, and conventions. Use when
  planning features, onboarding, or making architectural
  decisions.
invocation: >
  Use /architect to get a full project briefing before
  planning or designing a new feature.
---

# Architecture Guide

## Project Identity

| Field | Value |
|-------|-------|
| Language | Rust (edition 2024, stable toolchain) |
| Frontend | Svelte 5 + Vite (optional) |
| Backend | Axum (optional) |
| License | MIT |
| Version | `crates/rustbase/Cargo.toml` (single source) |
| Versioning | SemVer 2.0.0 |
| Platforms | Linux, Windows, macOS (CI on all three) |

## Repository Layout

```
rustbase/
  .cargo/
    config.toml         # cargo xtask alias
  .claude/
    hooks/              # Claude Code hook scripts
    commands/           # slash commands (commit, todo)
    skills/             # domain knowledge skills
    settings.json       # hook configuration
  .github/
    workflows/
      ci.yml            # test + fmt + clippy on 3 OS
      release.yml       # 5-target binary releases
  crates/
    rustbase/           # core library + CLI binary
      src/
        lib.rs          # library code
        bin/rustbase/
          main.rs       # CLI entry point
      tests/
        integration_test.rs
    rustbase-web/       # Axum web server (optional)
      src/
        main.rs         # server entry point
        api/
          mod.rs        # router + API handlers
  frontend/             # Svelte 5 + Vite (optional)
    src/
      App.svelte        # root component
      main.js           # bootstrap
      app.css           # global styles
  e2e/
    tests/              # Playwright E2E tests
  xtask/
    src/
      main.rs           # build automation
  scripts/              # bash wrappers
  docs/
    developer/
      DIARY.md          # development diary
      redteam-log.md    # security review findings
      artisan-log.md    # quality review findings
```

## Crate Responsibilities

### `crates/rustbase` (core)

Library code in `lib.rs`, CLI binary in
`src/bin/rustbase/main.rs`. All domain logic lives in
the library; the binary is thin dispatch.

**Dependencies**: `anyhow` (CLI errors), `thiserror`
(library errors), `clap` (argument parsing).

### `crates/rustbase-web` (optional)

Axum web server that depends on the core library.
Serves the Svelte frontend as static files with SPA
fallback. API routes under `/api/`.

**Dependencies**: `axum`, `tokio`, `tower-http`,
`tracing`, `tracing-subscriber`.

**Key patterns**:
- `create_router(frontend_path)` builds the full
  router with API routes + static file fallback
- `/health` returns `"OK"` for load balancers
- SPA fallback: non-API routes serve `index.html`

### `xtask` (build automation)

Not published. Provides `cargo xtask` commands:
`validate`, `test`, `clippy`, `fmt`, `coverage`.

## Quality Gates

| Gate | Threshold |
|------|-----------|
| Clippy | Zero warnings (`-D warnings`) |
| Formatting | `cargo fmt --check` |
| Coverage | 90% overall, 85% per module |
| Unsafe code | Forbidden (`#[forbid(unsafe_code)]`) |

All gates enforced by `cargo xtask validate` and
the Claude Code Stop hook.

## Frontend Architecture

Svelte 5 SPA (not SvelteKit). Manual client-side
routing if needed. Vite dev server proxies `/api`
to the Axum backend.

**Key files**:
- `frontend/vite.config.js` -- proxy config, reads
  `.ports` file and `Cargo.toml` version
- `frontend/src/App.svelte` -- root component with
  Svelte 5 runes (`$state`)
- `frontend/src/app.css` -- CSS custom properties
  for theming

## Adding a New Feature

1. Write tests first (TDD)
2. Implement in library crate, not binary
3. Add API endpoint in `crates/rustbase-web/src/api/`
   if web-facing
4. Add frontend UI in `frontend/src/`
5. Run `cargo xtask validate`
6. Commit with `/commit`

## Adding an API Endpoint

1. Add handler function in `api/mod.rs` (or new file)
2. Register route in `api_routes()`
3. Return `(StatusCode, Json<T>)` for consistency
4. Add test using `tower::ServiceExt::oneshot`
5. Update `llms.txt` endpoint table
