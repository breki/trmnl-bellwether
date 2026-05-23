---
name: trmnl-expert
description: >
  TRMNL device protocol, firmware schema, and operational
  reference. Use when debugging device behavior, modifying
  the bellwether-web `/api/*` handlers, reasoning about
  battery / network / telemetry semantics, or extending
  the BYOS surface.
invocation: >
  Use /trmnl-expert before touching any handler under
  `crates/bellwether-web/src/api/trmnl/` or when a device
  symptom (silent device, blank panel, stuck telemetry)
  needs root-causing.
---

# TRMNL Expert Reference

Hard-won knowledge about the TRMNL OG device and its
firmware interaction with bellwether-web. Sourced from
the upstream `usetrmnl/firmware` repo and verified
against the late-v0.27.x deployment on `malina` (a
Raspberry Pi running on the LAN).

For symptom-driven diagnostic playbooks ("I see X, do
Y"), see `docs/developer/RUNBOOK.md`. This skill is
the protocol / schema reference; the runbook is the
operational guide.

**Schema pin:** `usetrmnl/firmware@6cf2617` (2026-05-22)
— re-verify against `git log` of that repo if anything
in this document looks off when you read it.

## Hardware

| Spec | Value |
|------|-------|
| Panel | 7.5" e-ink, 800 × 480, 1-bit |
| SoC | ESP32 (with WiFi) |
| Power | USB-C charge + Li-ion battery |
| Battery cutoff / full | ~3.3 V / ~4.2 V |
| Refresh cadence | Set by server response (`refresh_rate` seconds, default 300) |
| Sleep mode | Deep-sleep between wakes — radio + CPU off |
| Reset | Recessed button on the back |
| Status LED | Green = normal; other colors documented in TRMNL hardware guide |

## Lifecycle

```
[USB plug-in or wake-timer]
        ↓
   WiFi reconnect (DHCP, new TCP socket per cycle)
        ↓
   POST /api/log    ← only if device has queued events
        ↓
   GET  /api/display ← every wake; rich telemetry in headers
        ↓
   GET  /images/<filename>  ← fetch the BMP
        ↓
   Draw to e-ink panel (~3 s, ~150 mA spike)
        ↓
   Deep-sleep `refresh_rate` seconds
```

The device does NOT cleanly close its TCP sockets
before deep-sleeping — the server kernel sees no FIN
and keeps connections in `ESTABLISHED` until its own
timer reaps them, leading to slow file-descriptor
accumulation on the server side. Tracked under
TODO.md "Idle-connection timeout on bellwether-web
HTTP routes".

## API endpoints

### `GET /api/setup` — first-boot registration

Headers the device sends:

| Header | Type | Notes |
|--------|------|-------|
| `ID` | string | Device MAC, colon- or dash-separated |

Response (`SetupResponse`):

| Field | Type | Notes |
|-------|------|-------|
| `status` | u16 | Mirrors HTTP status; firmware reads this not the HTTP code |
| `api_key` | string | Device sends as `Access-Token` on later requests |
| `friendly_id` | string | 6-char uppercase-hex derived from MAC |
| `image_url` | string | First image to fetch |
| `filename` | string | Same image's filename |

Exempt from the `require_access_token` middleware on
purpose — a fresh device has no token yet.

### `GET /api/display` — periodic image-fetch (~every 5 min)

**The richest telemetry channel.** The device sends
live device-state on every poll via HTTP headers,
not in a JSON body. As of this writing
bellwether-web's `display()` handler **does not read
these headers** — the live battery signal is sent
every 5 minutes and dropped on the floor. Reading
these headers is the right path for the
dashboard's battery indicator, not the `/api/log`
JSON body.

Headers the device sends (verified against
`src/api-client/display.cpp:addHeaders`):

| Header | Source | Notes |
|--------|--------|-------|
| `ID` | MAC | Same as `/api/setup` |
| `Access-Token` | configured api_key | Validated by middleware |
| `Update-Source` | string | Reason for fetch (e.g. wake timer) |
| `Refresh-Rate` | seconds (string) | Last `refresh_rate` device honored |
| **`Battery-Voltage`** | **f32 (volts)** | **Live ADC reading every wake** |
| `FW-Version` | string | Firmware version |
| `Model` | string | Device model |
| `RSSI` | i16 (dBm) | Live WiFi signal strength |
| `Width`, `Height` | u16 | Display dimensions |
| `temperature-profile` | "true" | Always present |
| `special_function` | "true" | Only when device is in a special mode |

TRMNL X-only additional headers (do not appear on
TRMNL OG):

- `Battery-Charging`, `Battery-Count`, `Percent-Charged`
- `Battery-Health`, `Battery-Current`, `Battery-Temp`
- `Battery-Capacity` (formatted as `current/max`)

Response (`DisplayResponse`):

| Field | Type | Notes |
|-------|------|-------|
| `filename` | string | URL-safe filename, validated by `validate_filename` |
| `image_url` | string | Absolute URL; firmware reads only this for the fetch |
| `refresh_rate` | u32 | Seconds; device honors and echoes back next time |
| `update_firmware` | bool | True triggers firmware update path |
| `firmware_url` | string | Only present when `update_firmware = true` |
| `reset_firmware` | bool | True triggers a soft-reset |
| `status` | u16 | 0 = OK |

### `POST /api/log` — event-driven log batch

Sparse. The device queues log entries while in
deep-sleep and ships them periodically — bursts of
several entries followed by quiet stretches.
Historical pattern on this deployment: ~24
POSTs/day on average, but bursty (15-60 min apart
during active hours, with hour-plus gaps overnight
or during stable operation).

**Do not rely on this endpoint for fast telemetry.**
For live battery readings, use the `/api/display`
headers above. `/api/log` is for log content
(messages, source lines, error context) and for
catching events the device deemed worth reporting
(boot, retry, error).

Body (`TrmnlLogRequest`):

```json
{
  "logs": [
    {
      "created_at": <unix_seconds>,
      "id": <log_id>,
      "message": "<string>",
      "source_line": <int>,
      "source_path": "<string>",
      "wifi_signal": <rssi_dbm>,
      "wifi_status": <int>,
      "refresh_rate": <seconds>,
      "sleep_duration": <seconds>,
      "firmware_version": "<string>",
      "special_function": <int>,
      "battery_voltage": <volts>,
      "wake_reason": <enum>,
      "free_heap_size": <bytes>,
      "max_alloc_size": <bytes>,
      "retry": <attempt_number>  // optional
    },
    ...
  ]
}
```

Source:
- `lib/trmnl/src/serialize_request_api_log.cpp` —
  the envelope (`{"logs": [...]}`)
- `lib/trmnl/src/serialize_log.cpp` — the per-entry
  shape

Entries are in **chronological FIFO order**, verified
against `lib/trmnl/src/stored_logs.cpp:gather_stored_logs`.
The device-status snapshot in each entry is
captured at entry-creation time (not send time), so
the freshest device-status lives in the **last**
entry of the array.

Response: 204 No Content. The firmware retries on
4xx / 5xx; bellwether-web returns 422 only for
malformed JSON.

Body limit: `MAX_LOG_BODY_BYTES = 16 KiB` via
`DefaultBodyLimit`. Beyond that → 413.

### `GET /images/<filename>` — image fetch

The device hits the URL it got from `/api/display`'s
`image_url`. `validate_filename` rejects path
traversal (`/`, `..`, leading dot, etc.) at the
boundary; there's no filesystem lookup, only an
in-memory store.

`MAX_RETAINED_IMAGES` images are kept; older ones
get evicted on insert. Older filenames stay
fetchable until evicted so in-flight device polls
don't 404.

## Battery model

| Voltage | Percent | Notes |
|---------|---------|-------|
| ≤ 3.3 V | 0% | Below cutoff; device may brown out |
| 3.75 V | ~50% | Linear midpoint |
| ≥ 4.2 V | 100% | Full charge |

Mapping lives in `crates/bellwether/src/telemetry.rs::battery_voltage_to_pct`
— linear between 3.3 and 4.2, clamped to `[0, 100]`,
rejects non-finite.

The e-ink refresh draws ~150 mA for ~3 seconds. A
weak battery (high internal resistance) can sustain
WiFi + HTTP just fine but brown out during the
panel write — leaving the panel showing partial /
noise pixels until the next successful refresh.
This is the failure mode we observed on 2026-05-23.

## Operational diagnostics

Symptom-driven playbooks live in `docs/developer/RUNBOOK.md`:

- Device shows noise on the panel → battery
  brownout during refresh
- Battery indicator stuck on em-dash → telemetry
  cache investigation
- Device hasn't POSTed `/api/log` in hours →
  expected; `/api/log` is event-driven
- Zombie `ESTABLISHED` connections → expected
  TCP-FIN leak from deep-sleep
- Deploy crash-loops the service → stale unit file
- Dashboard renders but battery indicator wrong → cache
  staleness or voltage→pct mapping check
- Image updated but data looks stale → Open-Meteo /
  publish-loop check

The runbook is the operational counterpart to this
protocol/schema reference. Diagnostic commands and
"what to check first" flows live there.

## Firmware source map

These paths are at `github.com/usetrmnl/firmware` —
clone or browse via `gh api repos/usetrmnl/firmware/contents/<path>`.

| Path | Purpose |
|------|---------|
| `src/api-client/display.cpp` | `/api/display` GET; **header construction** |
| `src/api-client/setup.cpp` | `/api/setup` request |
| `src/api-client/submit_log.cpp` | `/api/log` POST flow |
| `lib/trmnl/src/serialize_log.cpp` | Per-entry log JSON shape |
| `lib/trmnl/src/serialize_request_api_log.cpp` | Log envelope (`{"logs": [...]}`) |
| `lib/trmnl/src/stored_logs.cpp` | In-device log queue (circular buffer, FIFO order) |
| `lib/trmnl/src/parse_response_api_display.cpp` | What the device reads from `/api/display` JSON |
| `lib/trmnl/src/parse_response_api_setup.cpp` | What the device reads from `/api/setup` JSON |
| `lib/trmnl/src/bmp.cpp` | BMP palette ordering the renderer must match |
| `include/api-client/*.h` | Public API signatures |

## bellwether-web map

| File | Purpose |
|------|---------|
| `crates/bellwether-web/src/api/trmnl/handlers.rs` | `display`, `setup`, `log`, `preview`, `serve_image`, `require_access_token` |
| `crates/bellwether-web/src/api/trmnl/mod.rs` | Router, `TrmnlState`, image store |
| `crates/bellwether-web/src/api/trmnl/tests.rs` | Integration tests for every route |
| `crates/bellwether/src/telemetry.rs` | `DeviceTelemetry`, `battery_voltage_to_pct` |
| `crates/bellwether/src/render/dither.rs` | FS dither + pre-threshold edge snap |
| `crates/bellwether/src/dashboard/svg/mod.rs` | Battery indicator rendering (em-dash when cached telemetry is `None`) |

## Open follow-ups

Tracked in `TODO.md`:

- **`/api/display` header reader.** The live battery
  signal arrives on every poll but is dropped. Adding
  `HeaderMap` to the `display` handler and calling
  `state.update_telemetry` is the right path for fast
  battery telemetry — `/api/log` is too sparse.
- **Idle-connection timeout on `/api/log` and
  `/api/display`.** TCP zombies accumulate at ~1 per
  battery brownout.
- **Persist last device telemetry** (TODO.md
  "PR 3d / later"). Expose RSSI / FW version / wake
  reason via `/api/status` for at-a-glance device
  health.

## Verification cadence

Re-pin against upstream firmware whenever you touch
this skill or any `api/trmnl/*` handler:

```bash
gh api repos/usetrmnl/firmware/commits/HEAD --jq \
  '{sha: .sha[:7], date: .commit.committer.date}'
```

If the firmware HEAD has moved past the pinned
commit, spot-check the files listed above for
schema changes before trusting this document.
