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
- **Idle-connection timeout on bellwether-web HTTP
  routes.** Nine ESTABLISHED TCP connections from the
  TRMNL device IP accumulated over 32 days of uptime,
  all idle (Recv-Q = Send-Q = 0). Each is a leftover
  from a wake cycle where the battery-powered device
  opens a TCP socket then deep-sleeps without sending
  FIN — malina's kernel keeps the connection
  ESTABLISHED indefinitely, leaking file descriptors
  one or two per battery brownout. Mitigation: attach
  `tower::timeout::TimeoutLayer` (or similar idle
  timeout) to the TRMNL webhook routes at ~30 s so
  half-open sockets get force-closed server-side.
  Chronic, not urgent, but accumulates with every
  battery-brownout event.

## Chores

- **`cargo audit` + `cargo deny` in CI.** Add a
  security-advisory gate to `xtask validate` or a
  dedicated CI job so new RUSTSEC advisories fail the
  build. Flagged during PR 3a (RT-034).

## PR 3d / later

- **Persist last device telemetry.** `/api/log` logs
  structured fields but drops them. PR 3d should
  persist the last payload in `TrmnlState` (battery
  voltage, RSSI, FW version) and expose it via
  `/api/status` so the operator can see device health
  at a glance. Also enables refresh-rate adaptation
  (faster polls when battery healthy + data fresh,
  slower when battery low). Flagged during PR 3b
  (RT-048).
- **Config file for Access-Token.** Today the token
  comes from `BELLWETHER_ACCESS_TOKEN`. Mirror
  `windy.api_key_file` with `trmnl.byos.access_token_file`
  for consistency. Flagged during PR 3b (RT-042).
- **Async placeholder render on startup.** Move
  `seed_placeholder` behind `tokio::spawn` so the
  listener binds before the render finishes. Low
  priority on a Pi (~20 ms render). Flagged during
  PR 3b (RT-044).

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
- **Control panel.** Entity picker, layout editor,
  live preview. Likely server-rendered HTML or
  HTMX — the Svelte scaffold was removed because it
  hadn't earned its complexity.
- **Alternate layouts / plugin system.** Multiple SVG
  templates selectable by config or time of day.

## Done

- **TRMNL device-log parser: `battery_voltage` always
  `None` (fixed in v0.27.1, 2026-05-23).** Root cause was schema drift
  between bellwether-web's `TelemetryPayload`
  (expected top-level `battery_voltage` / `rssi` /
  `fw_version`) and the upstream
  `usetrmnl/firmware:lib/trmnl/src/serialize_log.cpp`
  shape (`{"logs": [{"battery_voltage": …,
  "wifi_signal": …, "firmware_version": …}]}`). The
  device's entire payload was falling into the
  `#[serde(flatten)] extra` catchall under one key
  (`"logs"`), surfacing as `extra_keys=1` across all
  811 device-log lines over 34 days. Replaced the
  struct with the firmware-faithful
  `TrmnlLogRequest { logs: Vec<TrmnlLogEntry> }`
  envelope; handler now iterates entries and picks
  the freshest battery_voltage. Restored the
  dashboard's battery indicator from em-dash to a
  real percentage.
- **Spike.** TRMNL protocol, hardware specs, render
  crate, HA auth decisions captured in
  `docs/developer/spike.md` (2026-04-16). OG 7.5"
  device + BYOS path confirmed by user.
