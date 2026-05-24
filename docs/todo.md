# TODO

Captured issues and ideas. New items added via
`/todo <text>`. Implementation tracked per item under
`docs/issues/<slug>.md` via `/implement <slug>`.

## Pending

- **idle-connection-timeout** -- attach a TimeoutLayer
  to the TRMNL webhook routes (~30 s) so half-open
  sockets get force-closed server-side.
  Nine ESTABLISHED TCP connections from the TRMNL
  device IP accumulated over 32 days of uptime, all
  idle (Recv-Q = Send-Q = 0). Each is a leftover
  from a wake cycle where the battery-powered device
  opens a TCP socket then deep-sleeps without sending
  FIN -- malina's kernel keeps the connection
  ESTABLISHED indefinitely, leaking file descriptors
  one or two per battery brownout. Chronic, not
  urgent, but accumulates with every battery-brownout
  event.

- **cargo-audit-deny-ci** -- add a security-advisory
  gate to xtask validate or a dedicated CI job.
  Flagged during PR 3a (RT-034). New RUSTSEC
  advisories should fail the build.

- **persist-device-telemetry** -- persist last
  /api/log payload (battery voltage, RSSI, FW
  version) in TrmnlState and expose via /api/status.
  /api/log logs structured fields but drops them.
  Persisting enables device-health at a glance, and
  refresh-rate adaptation (faster polls when battery
  healthy + data fresh, slower when battery low).
  Flagged during PR 3b (RT-048).

- **byos-access-token-file** -- mirror
  windy.api_key_file with
  trmnl.byos.access_token_file for consistency.
  Today the token comes from
  BELLWETHER_ACCESS_TOKEN. Flagged during PR 3b
  (RT-042).

- **async-startup-placeholder** -- move
  seed_placeholder behind tokio::spawn so the
  listener binds before the render finishes.
  Low priority on a Pi (~20 ms render). Flagged
  during PR 3b (RT-044).

- **home-assistant-integration** -- add
  [home_assistant] config section (base_url +
  token_file), a clients::home_assistant module
  that fetches entity states via REST, and
  [[home_assistant.entities]] in config.
  Auth: long-lived access token. Test with
  wiremock. (Deferred from PR 1.)

- **byos-provisioning** -- confirm the TRMNL device
  is reconfigured to point at malina before BYOS
  goes live.
  Fallback: Webhook Image plugin path (already
  modeled in config as mode = "webhook").

- **scheduler-retry** -- tokio-cron-scheduler or
  hand-rolled tick loop. Backoff on HA / Windy
  failures. Cache last-good data.

- **control-panel** -- entity picker, layout editor,
  live preview.
  Likely server-rendered HTML or HTMX -- the Svelte
  scaffold was removed because it hadn't earned its
  complexity.

- **alternate-layouts** -- multiple SVG templates
  selectable by config or time of day.

## Done

- **device-log-battery-voltage** (2026-05-23,
  v0.27.1) -- TRMNL device-log parser:
  battery_voltage always None.
  Root cause was schema drift between
  bellwether-web's TelemetryPayload (expected
  top-level battery_voltage / rssi / fw_version)
  and the upstream
  usetrmnl/firmware:lib/trmnl/src/serialize_log.cpp
  shape ({"logs": [{"battery_voltage": ...,
  "wifi_signal": ..., "firmware_version": ...}]}).
  The device's entire payload was falling into the
  #[serde(flatten)] extra catchall under one key
  ("logs"), surfacing as extra_keys=1 across all
  811 device-log lines over 34 days. Replaced the
  struct with the firmware-faithful
  TrmnlLogRequest { logs: Vec<TrmnlLogEntry> }
  envelope; handler now iterates entries and picks
  the freshest battery_voltage. Restored the
  dashboard's battery indicator from em-dash to a
  real percentage.

- **protocol-spike** (2026-04-16) -- TRMNL protocol,
  hardware specs, render crate, HA auth decisions
  captured in docs/developer/spike.md.
  OG 7.5" device + BYOS path confirmed by user.
