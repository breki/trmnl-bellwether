# Changelog

All notable changes to this project will be documented
in this file.

The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-04-16

### Added

- `build.ps1 dev` command launches backend + frontend
  dev servers with one invocation (parses `.ports`,
  pre-builds the backend, kills descendants cleanly on
  Ctrl+C)
- Frontend TypeScript support: `tsconfig.json`,
  `typescript`, `@tsconfig/svelte`, `svelte-check`
  dev dependencies; `.ts` entry point; `lang="ts"` in
  `App.svelte` with typed API response interfaces and
  runtime `res.ok` narrowing
- `npm run check` script in `frontend/package.json`
- `cargo xtask validate` now runs `svelte-check` as
  step 6 (skipped gracefully when no frontend is
  present or `node_modules` is missing)
- Modular xtask with agent-friendly stepwise output
  (`[1/5] Fmt... OK (0.3s)`) and structured result types
- `cargo xtask check` fast compilation check
- `/check`, `/test`, `/validate` slash commands
- `/todo` dual-mode: add items with arguments, implement
  without
- Cross-platform `scripts/kill-servers.sh` and
  `scripts/e2e.sh` for E2E test workflow
- `docs/ai-agents/guidelines.md` for agent-consumed
  tooling conventions
- E2E test policy in `CLAUDE.md` (UI features require
  Playwright tests)
- Root `tsconfig.json` for TypeScript E2E tests and
  Playwright config

### Changed

- `/commit` skill: code reviews before E2E tests,
  expanded review scope (frontend, config, deployment
  files), Deployment category in Red Team prompt,
  all findings reported via `AskUserQuestion`
- Playwright config: `127.0.0.1` to `localhost`,
  `cd frontend` to `cwd` option, `.js` to `.ts`
- E2E smoke test renamed from `.spec.js` to `.spec.ts`

### Fixed

- `@eslint/js` pin corrected from `^10.2.0` to
  `^10.0.0` (10.2.0 was never published to npm, so
  `npm install` failed with `ETARGET` on clean clones)
- Vite dev proxy now forwards `/health` to the backend
  (previously only `/api/*` was proxied, which broke
  the `health endpoint returns OK` E2E test against
  the frontend origin)
- `vitest` config: `passWithNoTests: true` prevents
  failure with no test files
- xtask: `CARGO_TERM_COLOR=never` for all JSON-parsed
  cargo output (coverage, metadata)
- xtask: clippy noise lines (`generated N warning`)
  filtered from output
- `kill-servers.sh`: `pkill -x` instead of `pkill -f`;
  PowerShell `Get-CimInstance` instead of deprecated
  `wmic`

### Added

- Initial project template with workspace structure
- xtask build automation (validate, test, clippy, fmt,
  coverage)
- Claude Code configuration with Stop hook, commit
  skill with Red Team + Artisan code review
- GitHub Actions CI (multi-platform) and release
  workflow (5 targets)
- Development diary and code review finding logs
- Optional web app: Axum backend + Svelte 5/Vite
  frontend with dev proxy, SPA routing, health/status
  API endpoints
- PowerShell build script (`build.ps1`)
- Integration test scaffold with `assert_cmd`
- Playwright E2E test scaffold with auto-server start
- `.ports` config pattern for port management
- `.mise.toml` for Node.js version management
- `llms.txt` AI-agent reference (llmstxt.org)
- `/architect` and `/web-dev` Claude Code skills
- CI frontend build job; release packages both
  binaries with frontend dist
- Code duplication check (`cargo xtask dupes`) using
  `code-dupes` with 6% threshold
- `/template-improve` slash command for logging
  template feedback
- TDD (red/green/refactor) guidance in `CLAUDE.md`
- Frontend linting with ESLint + `eslint-plugin-svelte`
- Frontend formatting with Prettier +
  `prettier-plugin-svelte`
- Frontend unit testing with Vitest +
  `@testing-library/svelte`
- `/template-sync` slash command for syncing upstream
  template changes into derived projects
- `.template-sync.toml` for tracking template version
  origin and last sync point

### Fixed

- `/health` endpoint now returns JSON (`{"status":"ok"}`)
  instead of plain text for API consistency
- `vite.config.js` uses `import.meta.dirname` instead
  of CommonJS `__dirname`
- Tokio dependency narrowed from `full` to explicit
  feature list (`macros`, `rt-multi-thread`, `net`,
  `signal`)
- Release workflow uses `Compress-Archive` instead of
  `7z` for Windows packaging
- Release workflow warns when CHANGELOG extraction
  produces empty release notes
- Coverage no longer fails out of the box by excluding
  `xtask` crate and binary `main.rs` entry points
- Clarified `anyhow` vs `thiserror` dependency split
  in `Cargo.toml` comments
- Enforced that all commits must use `/commit` skill
- Release workflow uses `env:` blocks instead of inline
  `${{ }}` interpolation in `run:` blocks
- Release workflow fails on empty release notes instead
  of just warning
- Release checksum generation uses `nullglob` and
  explicit archive globs
- Release notes extraction uses exact version match
  instead of substring
- `create_router` accepts `&Path` instead of `&str`
- CLI bind address parsed as `IpAddr` via clap instead
  of string format + parse
- Added `edition = "2024"` to `rustfmt.toml`
- Documented `code-dupes` prerequisite in README
