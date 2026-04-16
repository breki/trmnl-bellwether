# Artisan Findings -- Resolved

Archive of fixed Artisan code quality findings, newest
first. See [artisan-log.md](artisan-log.md) for open
findings.

---

### AQ-018 -- tsconfig.json redeclared @tsconfig/svelte defaults

- **Date:** 2026-04-16
- **Category:** Project Configuration (Low)
- **Commit context:** v0.4.0 dev command + frontend TypeScript
- **Resolution:** Reduced to only `noEmit: true`;
  everything else inherited from the extended base.

### AQ-017 -- template-feedback entries mixed statuses

- **Date:** 2026-04-16
- **Category:** Maintainability (Low)
- **Commit context:** v0.4.0 dev command + frontend TypeScript
- **Resolution:** Added `[Deferred]`, `[Fixed locally]`,
  `[N/A for template]` prefixes to the 2026-04-16
  entries so triage is immediate.

### AQ-016 -- JSON type assertions trusted server blindly

- **Date:** 2026-04-16
- **Category:** Type Safety (Medium)
- **Commit context:** v0.4.0 dev command + frontend TypeScript
- **Resolution:** `App.svelte` now throws on `!res.ok`
  and narrows results via `Partial<T>` + `??` fallbacks,
  so a 500 response or a missing field no longer
  silently produces `undefined` values in state.

### AQ-015 -- Invoke-Dev had no guard for missing node_modules

- **Date:** 2026-04-16
- **Category:** Maintainability (Medium)
- **Commit context:** v0.4.0 dev command + frontend TypeScript
- **Resolution:** `Invoke-Dev` now checks
  `frontend/node_modules` before launching the backend
  and emits an actionable error pointing at
  `cd frontend && npm install`.

### AQ-014 -- Vec<&str> limits future extensibility

- **Date:** 2026-04-15
- **Category:** API Design (Low)
- **Commit context:** v0.3.0 template improvements
- **Resolution:** Low severity, not fixed -- pragmatic
  for current usage.

### AQ-013 -- validate stops on first failure

- **Date:** 2026-04-15
- **Category:** API Design (Low)
- **Commit context:** v0.3.0 template improvements
- **Resolution:** Low severity, not fixed -- fail-fast
  is defensible since later steps may depend on earlier.

### AQ-012 -- String-based errors across all modules

- **Date:** 2026-04-15
- **Category:** Error Handling (Low)
- **Commit context:** v0.3.0 template improvements
- **Resolution:** Low severity, not fixed -- pragmatic
  for xtask scope, consistent with AQ-004 resolution.

### AQ-011 -- Option<String> error pattern in results

- **Date:** 2026-04-15
- **Category:** Type Safety (Medium)
- **Commit context:** v0.3.0 template improvements
- **Resolution:** Low severity, not changed structurally.
  Applied `match` on `Option` instead of
  `unwrap_or_default()` to eliminate masked errors.

### AQ-010 -- unwrap_or_default on known-Some

- **Date:** 2026-04-15
- **Category:** Correctness (Low)
- **Commit context:** v0.3.0 template improvements
- **Resolution:** Replaced `unwrap_or_default()` with
  `match` in `dupes()`, `run_clippy()`,
  `run_coverage()`.

### AQ-009 -- helpers.rs tests don't call step_output

- **Date:** 2026-04-15
- **Category:** Type Safety (High)
- **Commit context:** v0.3.0 template improvements
- **Resolution:** Extracted `format_step()` function,
  tests now assert on actual function output.

### AQ-008 -- Duplicated clippy argument arrays

- **Date:** 2026-04-15
- **Category:** API Design (Medium)
- **Commit context:** v0.3.0 template improvements
- **Resolution:** Extracted `CLIPPY_ARGS` constant,
  implemented `clippy()` in terms of `clippy_check()`.

### AQ-007 -- format! + parse for SocketAddr

- **Date:** 2026-04-10
- **Category:** API Design
- **Commit context:** v0.2.1 review finding fixes
- **Resolution:** Changed `cli.bind` from `String` to
  `IpAddr` (parsed by clap). Construct `SocketAddr`
  directly via `SocketAddr::new(cli.bind, cli.port)`,
  eliminating the fallible `format!` + `.parse()` +
  `.expect()` chain.

### AQ-006 -- `create_router` accepts `&str` not `&Path`

- **Date:** 2026-04-10
- **Category:** API Design
- **Commit context:** v0.2.1 review finding fixes
- **Resolution:** Changed `frontend_path` parameter
  from `&str` to `&Path`. Uses `Path::join` instead of
  `format!` for index path. Updated `cli.frontend` to
  `PathBuf` and all test call sites.

### AQ-005 -- Inconsistent String vs &'static str

- **Date:** 2026-04-10
- **Category:** Type Safety
- **Commit context:** v0.1.2 template feedback fixes
- **Resolution:** Changed all response struct fields
  that hold compile-time-known values to
  `&'static str`. Simplified all handlers to return
  `Json<T>` directly instead of
  `(StatusCode::OK, Json(...))` tuple since 200 is
  the Axum default.

### AQ-004 -- Stringly-typed errors throughout xtask

- **Date:** 2026-04-10
- **Category:** Type Safety
- **Commit context:** v0.1.1 template feedback fixes
- **Resolution:** Kept `Result<(), String>` but
  structured error messages with consistent prefixes
  ("failed to run", "exited with") so callers can
  pattern-match on content. Added conditional install
  hint in `run_dupes()` that checks the prefix.

### AQ-003 -- Install hint on all code-dupes errors

- **Date:** 2026-04-10
- **Category:** Error Handling
- **Commit context:** v0.1.1 template feedback fixes
- **Resolution:** `run_dupes()` now only appends the
  install hint when the error contains "failed to run"
  (command not found), not when code-dupes exits
  non-zero due to excessive duplication.

### AQ-002 -- Loop-invariant threshold allocation

- **Date:** 2026-04-10
- **Category:** API Design
- **Commit context:** v0.1.1 template feedback fixes
- **Resolution:** Hoisted `threshold` string above the
  loop. Used `:.1` format for consistency.

### AQ-001 -- Hardcoded crate paths vs workspace-aware

- **Date:** 2026-04-10
- **Category:** Abstraction Boundaries
- **Commit context:** v0.1.1 template feedback fixes
- **Resolution:** Replaced hardcoded paths with
  `discover_src_dirs()` which uses `cargo metadata` to
  dynamically discover workspace members, consistent
  with how `run_coverage()` uses `--workspace`.
