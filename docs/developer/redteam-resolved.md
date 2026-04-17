# Red Team Findings -- Resolved

Archive of fixed red team findings, newest first.
See [redteam-log.md](redteam-log.md) for open findings.

---

## 2026-04-17 (PR 2 — v0.3.0 Windy client)

### RT-009 — `#[serde(flatten)]` into `HashMap<String, Vec<f64>>` brittle to new fields
**Category:** Correctness (forward-compat)
**Description:** Any new top-level field Windy added
(scalar metadata, nested objects) would fail to
deserialize as `Vec<f64>`, turning a 200 OK into
`WindyError::Parse`.
**Resolution:** Flatten into
`HashMap<String, serde_json::Value>`; in
`RawResponse::into_forecast`, try-convert each value to
`Vec<Option<f64>>` and silently drop entries that don't
match. Test
`fetch_ignores_unknown_non_numeric_fields` locks it in
with elevation/model_id/metadata noise.

### RT-010 — No length-match check between `ts` and series
**Category:** Correctness
**Description:** Doc claimed "length matches timestamps"
but nothing enforced it.
**Resolution:** New variant
`WindyError::SeriesLengthMismatch { key, expected, got }`;
enforced in `into_forecast`. Test
`fetch_rejects_series_length_mismatch`.

### RT-011 — Empty `ts` accepted as a valid Forecast
**Category:** Correctness
**Description:** Downstream consumers would panic on
indexing into empty timestamps.
**Resolution:** New variant
`WindyError::EmptyForecast`; `into_forecast` returns it
when `ts.is_empty()`. Doc on `Forecast::timestamps`
guarantees non-empty. Test `fetch_rejects_empty_ts`.

### RT-012 — API key could leak via `Api { body }` if server echoed it
**Category:** Security
**Description:** Proxies (Cloudflare, nginx 413) and
verbose API errors sometimes echo the request body.
That body was stored verbatim in the `Api` variant,
which downstream logs/anyhow would happily print.
**Resolution:** New `redact_secret(text, secret)`
helper; called on error bodies before they're stored
in `WindyError::Api`. Empty-secret is a no-op to avoid
blanking everything when key isn't set. Test
`fetch_surfaces_api_error_with_redacted_body` uses a
bait key and asserts redaction.

### RT-013 — Unbounded response body read (DoS / OOM)
**Category:** Security
**Description:** `resp.text()` / `resp.json()` read
the full body without any cap.
**Resolution:** New `read_capped_body(resp, limit)`
helper: checks `Content-Length` up front, then streams
`resp.chunk().await?` with a running counter. Exceeds
the cap → `WindyError::ResponseTooLarge { limit }`. The
cap is per-Client (`max_response_bytes` /
`max_error_body_bytes`) with `with_max_response_bytes`
builder for tests. Defaults: 4 MiB success, 4 KiB
errors. Tests
`fetch_rejects_oversized_response_via_content_length`
and `fetch_rejects_oversized_error_body`.

### RT-014 — `InvalidTimestamp` branch was unreachable in tests
**Category:** Robustness / coverage
**Resolution:** Added test
`fetch_surfaces_invalid_timestamp` that sends `i64::MAX`
(~292M years) — outside `NaiveDateTime` range — and
asserts the variant fires. Branch now covered.

### RT-015 — `Mock::expect(1)` Drop-time assertion unreliable
**Category:** Test hygiene
**Resolution:** The rewrite doesn't use
`.expect(1)` anymore. Request-body assertions rely on
`body_json` matchers that force an unmatched request to
404 → `WindyError::Api` → `.unwrap()` panic. The test
comment documents this explicitly.

### RT-016 — Default redirect policy could re-POST API key cross-origin
**Category:** Security
**Description:** DNS hijack or CDN compromise could
redirect `api.windy.com` to an attacker; reqwest's
default policy follows 10 redirects and does NOT strip
the body cross-origin, leaking the key.
**Resolution:** `Client::with_base_url` configures
`.redirect(Policy::none())`. A 302 now surfaces as
`WindyError::Api { status: 302 }` instead. Test
`fetch_does_not_follow_redirects`.

### RT-017 — No long-lived-Client documentation
**Resolution:** `Client` doc now says "Build once per
process and reuse. Construction pays for TLS setup and
connection-pool initialization."

### RT-018 — `Vec<f64>` fails on Windy `null` values
**Category:** Correctness
**Description:** Windy returns `null` at grid edges for
some models/timesteps. `Vec<f64>` couldn't deserialize
that.
**Resolution:** `Forecast::series` is now
`HashMap<String, Vec<Option<f64>>>`. Test
`fetch_preserves_null_values_in_series`.

### RT-019 — `serde_json` now a direct dep
**Resolution:** Added a comment above the reqwest dep
in `Cargo.toml` noting that `serde_json` is for the
Windy JSON wire format only; config is TOML-only.

### RT-020 — Unused tokio `test-util` feature
**Resolution:** Removed from dev-dependencies.

### RT-021 — No separate `connect_timeout`
**Category:** Operational
**Resolution:** `connect_timeout(5s)` set, overall
`timeout(20s)` — gives a slow RPi connect room to
finish before the overall timeout fires.

### RT-022 — No gzip compression negotiated
**Category:** Operational
**Resolution:** Added `"gzip"` to reqwest's feature
set. Halves typical Windy response sizes over
cellular.

### RT-023 — Empty `parameters` list silently sent
**Category:** Correctness / waste
**Resolution:** `Client::fetch` short-circuits with
`WindyError::NoParameters` before building the
request. Test `fetch_rejects_empty_parameters_early`
exercises the early-return path (no mock server
needed).

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
