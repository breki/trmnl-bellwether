# TODO

## Pending

- **PR 1 (in progress):** Config loading. TOML at
  `config.toml` with `[windy]`, `[trmnl]`, `[render]`
  sections. `api_key_file` path indirection for
  secrets. Unit test with fixture. No
  `[home_assistant]` in this PR — it's in the backlog.
- **PR 2:** Windy Point Forecast client
  (`clients::windy`). `wiremock` unit tests; real
  network tests behind `#[ignore]` + env var guard.
  User has an existing annual subscription — do not
  generate keys.
- **PR 3:** First render + TRMNL publish. SVG layout
  baked in; `resvg` → grayscale → Floyd–Steinberg
  dither → 1-bit 800×480 BMP. BYOS-compatible
  publishing (`/api/display` + `image_url` serving).
  Confirm Terminus JSON schema from its source before
  coding.
- **PR N:** Wire the `fetch → render → publish` loop
  behind a `bellwether run` subcommand with a
  configurable refresh interval.

## Backlog

- **Home Assistant integration** (deferred from PR 1).
  Adds `[home_assistant]` config section (base_url +
  `token_file`), a `clients::home_assistant` module
  that fetches entity states via REST, and
  `[[home_assistant.entities]]` in config. Auth:
  long-lived access token. Test with `wiremock`.
- **BYOS provisioning.** Confirm the TRMNL device is
  reconfigured to point at `malina` before PR 3 goes
  live. Fallback: Webhook Image plugin path (already
  modeled in config as `mode = "webhook"`).
- **Scheduler + retry.** `tokio-cron-scheduler` or
  hand-rolled tick loop. Backoff on HA / Windy
  failures. Cache last-good data.
- **Control panel** (Svelte frontend). Entity picker,
  layout editor, live preview.
- **Alternate layouts / plugin system.** Multiple SVG
  templates selectable by config or time of day.

## Done

- **Spike.** TRMNL protocol, hardware specs, render
  crate, HA auth decisions captured in
  `docs/developer/spike.md` (2026-04-16). OG 7.5"
  device + BYOS path confirmed by user.
