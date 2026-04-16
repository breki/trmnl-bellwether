# Template Feedback

Issues, improvements, and observations about the
[rustbase](https://github.com/breki/rustbase) template
discovered during development of this project.

Use this log to feed improvements back to the template.
Newest entries first.

---

## 2026-04-16

- **[Deferred] Deploy logic belongs in xtask, not bash
  scripts.** Logged in this session; deferred for a
  dedicated design discussion -- hoard's deploy is
  specific to a Raspberry Pi / SSH / systemd workflow,
  and the right template baseline needs a call on
  config format (toml?), target (any SSH host vs.
  arch-specific cross-compile), and whether to ship
  placeholder or no implementation at all. Not
  resolved by the implement pass that followed the
  initial log.

- **[Fixed locally] `@eslint/js` pin in
  `frontend/package.json` was unresolvable.** The
  template shipped `"@eslint/js": "^10.2.0"`, but
  10.0.1 is the latest published version on npm, so
  `npm install` from a clean clone failed with
  `ETARGET`. The lockfile had 10.0.1 installed,
  masking the problem until someone ran `install`
  without the lock. **Fix:** changed the pin to
  `^10.0.0`. Template should pin to a version that
  actually exists and ideally be kept in sync by a
  bot like Renovate.

- **[N/A for template] Playwright config should isolate
  test data from committed fixtures.** Logged in this
  session, but not directly applicable to the template
  as-is: `rustbase-web` has no persistent data --
  `/api/status` and `/api/greeting` return static
  values. The fixture-isolation pattern becomes
  relevant the moment a downstream project adds a data
  store. **Fix (for the template):** when a
  data-persistence example is eventually added to the
  template, include the hoard pattern alongside it
  (globalSetup copies `e2e/fixtures/*.json` to
  `test-data/`, backend run with `--data test-data`).
  Until then, leave it to individual projects.

## Pre-2026-04-16 entries

## vite.config.js uses CommonJS `__dirname` in ESM context

`frontend/package.json` declares `"type": "module"`,
which means all `.js` files are ES modules. However,
`vite.config.js` uses `__dirname` (lines 9, 22), which
is a CommonJS-only global -- it does not exist in native
ES modules.

This works today only because Vite's config loader shims
`__dirname` when it processes the config file. But it is
not idiomatic, could confuse contributors, and would
break if the same pattern were copy-pasted into other
`.js` files in the project (e.g. a helper script).

**Fix:** Replace `__dirname` with
`import.meta.dirname` (available since Node 21.2 / 22+,
which the template already requires) or the more
portable `path.dirname(fileURLToPath(import.meta.url))`.

## Tokio uses `features = ["full"]` unnecessarily

`crates/rustbase-web/Cargo.toml` depends on
`tokio = { version = "1", features = ["full"] }`. The
`full` feature flag pulls in every Tokio subsystem:
`io-std`, `io-util`, `process`, `test-util`, `time`,
`fs`, `sync`, `signal`, etc. The web server only needs
a small subset.

Pulling in unused features increases compile time and
binary size. It also makes it harder to audit what
capabilities the application actually requires.

**Fix:** Replace `"full"` with an explicit feature list.
For a typical Axum app, this is:
`["macros", "rt-multi-thread", "net", "signal"]`.
Add `"time"` or `"fs"` only if the application
actually uses them.

## build.ps1 `build` command double-validates

`Invoke-Build` (line 36) calls `Invoke-Validate`, which
runs `cargo xtask validate` (fmt + clippy + test +
coverage). It then calls `Invoke-BuildOnly`, which runs
`cargo build --release`. But `cargo xtask validate`
already compiled the entire workspace (in debug mode) as
part of running clippy and tests.

The result: a full `.\build.ps1 build` compiles the
workspace twice -- once in debug for validate, once in
release for the final binary. This is expected if the
intent is "check everything, then produce a release
binary." But if a user just wants a validated release
build, there is no single command that avoids the
double-compile.

**Suggestion:** Document that `build` intentionally
compiles twice (validate in debug, then release), so
users understand the cost. Alternatively, consider a
`build-release` command that runs clippy and tests
against the release profile directly, avoiding the
redundant debug build.

## `/health` endpoint returns plain text, not JSON

In `crates/rustbase-web/src/api/mod.rs`, the `/health`
endpoint (line 34) returns `"OK"` as plain text, while
`/api/status` and `/api/greeting` both return JSON.
This inconsistency can surprise API consumers and makes
it harder to write uniform response-handling logic on
the client side.

Health endpoints are also commonly consumed by
orchestrators (Kubernetes, Docker, load balancers) that
may expect a JSON body or at least a
`Content-Type: application/json` header.

**Fix:** Return a JSON response like
`{ "status": "ok" }` for consistency with the rest of
the API, or at minimum document the plain-text contract
in the route comment.

## Release workflow assumes `7z` is available on Windows

In `.github/workflows/release.yml` (line 114), the
Windows packaging step runs `7z a "${STAGING}.zip" ...`.
GitHub-hosted Windows runners include 7-Zip, so this
works today. But self-hosted runners or alternative CI
providers may not have it, and the workflow gives no
error message explaining the dependency.

**Fix:** Use PowerShell's built-in
`Compress-Archive -Path "$STAGING/*" -DestinationPath
"${STAGING}.zip"` instead of `7z`. This requires no
external tools and works on any Windows environment with
PowerShell 5.1+. Alternatively, add a step that checks
for `7z` and prints a clear error if it is missing.

## Release notes extraction from CHANGELOG is fragile

The release workflow extracts notes using an `awk`
script (line 143) that looks for a heading matching
`## [<version>]`. If the CHANGELOG heading format
deviates even slightly -- e.g. extra whitespace, a
different date format, or a missing version entry -- the
`awk` script silently produces an empty
`release_notes.md`. The release then goes out with a
blank description and no error is raised.

**Fix:** Add a check after the `awk` extraction:
```bash
if [ ! -s release_notes.md ]; then
  echo "::warning::No release notes found for $VERSION"
fi
```
This at least surfaces the problem. For a more robust
approach, consider a dedicated changelog-parsing tool
or a simpler grep-based extractor with explicit error
handling.

## No frontend linting or formatting tools

The Rust side of the template has strict quality gates:
`rustfmt` for formatting, Clippy with `deny(warnings)`
and pedantic lints. The frontend has nothing comparable.
There is no ESLint config, no Prettier config, and
`cargo xtask validate` does not check frontend code
quality.

This means frontend code style drifts silently, and
common mistakes (unused variables, missing accessibility
attributes, inconsistent formatting) go undetected.

**Fix:** Add a minimal linting/formatting setup:
- `eslint` with `eslint-plugin-svelte` for lint rules
- `prettier` with `prettier-plugin-svelte` for
  formatting
- A `lint` script in `frontend/package.json`
- Optionally wire it into `cargo xtask validate` or
  `build.ps1 validate` so frontend and backend quality
  checks run together

## No frontend unit test infrastructure

The template includes Playwright for E2E testing, but
has no setup for frontend unit or component tests.
Svelte components with non-trivial logic (data
transformations, conditional rendering, event handling)
cannot be tested in isolation without a framework like
Vitest.

E2E tests are slow, require the full backend running,
and are poorly suited for testing edge cases in
individual components. A unit test layer fills this gap.

**Fix:** Add Vitest with `@testing-library/svelte`:
- `npm install -D vitest @testing-library/svelte
  jsdom`
- Add a `vitest.config.js` (or extend `vite.config.js`)
- Add a `test` script in `frontend/package.json`
- Include a sample component test as a starting point
