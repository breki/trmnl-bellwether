# Red Team Findings -- Resolved

Archive of fixed red team findings, newest first.
See [redteam-log.md](redteam-log.md) for open findings.

---

## 2026-04-17 (PR 1 — v0.2.0 config skeleton)

### RT-001 — `TrmnlMode` not coupled to its subsection
**Category:** Correctness
**Description:** `mode = "byos"` with no `[trmnl.byos]`
table parsed to `Ok` with `byos: None`. Bug would only
surface at publish time.
**Resolution:** Collapsed `TrmnlConfig` into a
`#[serde(tag = "mode")]` internally-tagged enum. The
TOML schema flattened (`[trmnl]` holds `mode` +
variant-specific fields directly; no nested
`[trmnl.byos]`). Illegal states are now
unrepresentable; missing variant fields fail at parse
time. Added `rejects_mode_without_matching_payload`
test.

### RT-002 — `#[serde(deny_unknown_fields)]` not set
**Category:** Correctness
**Description:** Typos in defaulted fields (e.g.
`heigth = 500`) were silently ignored.
**Resolution:** Added `#[serde(deny_unknown_fields)]`
to `Config`, `WindyConfig`, and `RenderConfig`.
Skipped on `ByosConfig` / `WebhookConfig` because
serde's internally-tagged enum representation is
incompatible with `deny_unknown_fields` on variant
structs — a known serde limitation. Added
`rejects_unknown_top_level_field` test to lock in the
behaviour.

### RT-003 — `Path::parent()` returns `Some("")` for bare filenames
**Category:** Correctness (broken doc promise)
**Description:** `bellwether --config config.toml`
with a bare filename caused `Path::parent()` to return
`Some("")`, leaving `api_key_file` relative and read
against CWD, violating the documented "resolved
against config file's parent directory" contract.
**Resolution:** Introduced `config_base_dir` that
treats an empty parent as "no directory component"
and falls back to `std::env::current_dir()`.

### RT-004 — `api_key_file` not validated at load time
**Category:** Correctness / UX
**Description:** Missing or unreadable secret files
only failed at first forecast fetch, long after the
"loaded config" success print.
**Resolution:** `Config::load` now eagerly reads the
secret via `read_secret` and stashes the trimmed
result in a private `api_key` field on `WindyConfig`.
Misconfig fails at startup.

### RT-005 — `read_api_key` returned `Ok("")` for empty secret files
**Category:** Correctness
**Description:** Whitespace-only key files produced
an empty key, which would surface as a confusing
remote 401/403.
**Resolution:** New `ConfigError::EmptySecret { path }`
variant; `read_secret` returns it when the trimmed
contents are empty. Test `rejects_empty_secret`.

### RT-007 — `ConfigError::Io` conflated config-file and secret-file errors
**Category:** Correctness / API design
**Description:** Callers couldn't distinguish a
broken `config.toml` from a broken secret file.
**Resolution:** Split into `ConfigError::ReadConfig`
and `ConfigError::ReadSecret` — each carries the
specific `path` and wraps `std::io::Error`. Tests
`reports_read_config_error_for_missing_file` and
`reports_read_secret_error_for_missing_key_file`.

### RT-008 — Test used bare relative path; false-pass risk
**Category:** Correctness (test hygiene)
**Description:**
`cli_reports_error_for_missing_config` used
`"definitely-not-a-real-path.toml"` as a CLI arg, so
any developer creating that file in the crate root
could silently break it.
**Resolution:** Switched to `TempDir::new().path().join(...)`
for an absolute, guaranteed-nonexistent path.

## Noted — not acted on

### RT-006 — Dependency versions loosely pinned
**Category:** Project configuration (low-priority)
**Description:** `serde = "1"`, `toml = "0.8"`,
`tempfile = "3"` admit any compatible semver release.
**Resolution:** No change — consistent with the rest
of the crate's dep policy (`anyhow = "1"`, `clap =
"4"`) and `Cargo.lock` locks the actual versions
that ship. Would need to be addressed project-wide
rather than per-dep.
