# Design Spike — TRMNL, Rendering, HA, Windy

Written 2026-04-16 to resolve the five open questions
flagged in `HANDOFF.md` before any code is written.
This document is decision-oriented: each section ends
with a **Decision** that PR 1+ can assume.

## TL;DR

| Topic             | Decision                                           |
|-------------------|----------------------------------------------------|
| TRMNL device      | **OG 7.5"** — 800×480, 1-bit black/white (confirmed) |
| Integration path  | **BYOS** — device polls our RPi (confirmed v1 target) |
| Fallback path     | TRMNL Cloud *Webhook Image* plugin (kept as option) |
| Image format      | 1-bit BMP, 800×480, Floyd–Steinberg dithered       |
| Render stack      | `resvg` (SVG → RGBA) + `image` (dither + BMP)      |
| HA auth           | Long-lived access token, file-referenced (deferred) |
| Windy API         | Point Forecast v2 JSON, key file-referenced        |

**PR 1 scope change:** Home Assistant integration moves
to backlog at the user's request. PR 1 wires config for
Windy + TRMNL + render only. HA-related work (client,
auth, entity selection, `[home_assistant]` config
section) is deferred to a later PR. The decision to use
a long-lived access token stands and applies when that
work resumes.

## 1. TRMNL device & panel

Two models ship today:

- **OG** — 7.5", 800×480, **1-bit black/white** EPD.
  This is what the user almost certainly owns (the
  brief and the HA community plugins all assume it).
- **X** — 10.3", 1872×1404, 4-bit grayscale (16 levels).
  Newer and more expensive; unlikely but worth asking.

We target OG by default. The render pipeline stays
grayscale-clean internally so that switching to X is a
resolution + palette swap, not a rewrite.

**Open user question (before PR 3):** *Which TRMNL
model do you own, and at what firmware version?* Needed
to confirm the 800×480 1-bit assumption and whether
the firmware supports BYOS redirect.

## 2. Integration paths — three exist, pick one

The TRMNL ecosystem offers three ways to get pixels
onto the device. Each has different tradeoffs for a
server-side-rendered Rust daemon on an RPi.

### Path A — BYOS (Bring Your Own Server) — **Recommended**

The device is reconfigured to poll our server instead
of `trmnl.com`. Our server implements the firmware's
polling contract:

- `GET /api/setup` — first-boot device registration
- `GET /api/display` — returns JSON:
  ```json
  {
    "image_url": "http://malina.local/images/abc.bmp",
    "filename": "abc.bmp",
    "refresh_rate": 900,
    "update_firmware": false,
    "firmware_url": "",
    "reset_firmware": false,
    "status": 0
  }
  ```
  Headers on the request include `ID` (device MAC) and
  `Access-Token`. The device then does a second GET for
  `image_url` and expects a raw BMP body.
- `POST /api/log` — device telemetry

**Pros:** no cloud dependency, no rate limit, full
control over refresh cadence, offline-friendly, fits
the "malina on the LAN" deployment target.

**Cons:** device must be re-provisioned to point at our
server (firmware supports this, but it's not
zero-config). We own uptime — if `bellwether` is down,
the screen freezes.

**Canonical reference:** `usetrmnl/terminus` (Ruby).
Read its controllers to pin the exact JSON schema
before PR 3; the public docs page is thin.

### Path B — Cloud Webhook Image plugin (experimental)

We POST a rendered BMP directly to a plugin-specific
URL at `trmnl.com`. The plugin does no processing — it
just stores the image and serves it to the device on
its next poll.

- `POST https://trmnl.com/api/custom_plugins/{UUID}`
- `Content-Type: image/png | image/jpeg | image/bmp`
- Body: raw image bytes, ≤5 MB, 800×480 recommended
- Rate limit: **12 uploads/hour** (30/h for TRMNL+)

**Pros:** no device reconfiguration, trivial to
implement (one HTTP POST).

**Cons:** rate limit is aggressive (5-minute minimum
refresh), cloud round-trip, relies on an explicitly
*experimental* endpoint.

### Path C — Cloud Private Plugin with merge_variables

We POST a small JSON blob of variables (`≤2 KB` free,
`≤5 KB` plus) and the user hand-authors a Liquid/HTML
template in TRMNL's markup editor. Cloud renders it.

- `POST https://trmnl.com/api/custom_plugins/{UUID}`
- `Content-Type: application/json`
- Body: `{"merge_variables": {...}}`

**Pros:** cloud handles dithering and layout.

**Cons:** our server-side Rust renderer becomes
pointless — the entire render is a web-based Liquid
template on trmnl.com. This conflicts with the project
goal ("server-side e-ink renderer"). Rejected.

### Decision

**Primary target: Path A (BYOS).** It aligns with the
project goal of server-side rendering on the RPi and
removes cloud dependencies.

**Fallback: Path B (Webhook Image).** Implement the
renderer crate so its output (a 1-bit 800×480 BMP) is
valid for both paths. If the user's device can't be
moved to BYOS, the same BMP can be POSTed to the
Webhook Image endpoint instead — a different
transport, same payload. Path C is rejected because it
bypasses our renderer.

The config layer (`[trmnl]` section in PR 1) should
model both paths from the start: a `mode = "byos"`
or `mode = "webhook"` discriminator picks the
publisher impl.

## 3. Image format & dithering

800×480 at 1-bit = 48,000 bytes of pixel data plus BMP
header. Trivial size; no compression needed.

Pipeline:

1. Author layout as SVG.
2. `resvg` → 800×480 RGBA buffer.
3. `image::imageops` → grayscale (luma).
4. Floyd–Steinberg dither (via `image` crate's
   `dither` or `imageproc`) → 1-bit pixels.
5. Encode as 1-bit BMP (BI_RGB, 2-entry palette:
   black 0x00, white 0xFF).

**Why BMP, not PNG.** The firmware fetches `image_url`
and hands the bytes straight to the EPD driver; the
wiki and firmware examples consistently show `.bmp`.
PNG *may* work (the Webhook Image plugin accepts it),
but BMP is the safe baseline and is what BYOS
deployments ship. Support for PNG output can be added
later behind a config flag.

**Orientation.** Landscape (800 wide × 480 tall). No
rotation in v1.

**Refresh cadence.** Returned in the `/api/display`
response (`refresh_rate` in seconds). Start with 900
(15 minutes) — aggressive enough to keep weather fresh,
slow enough to save battery. Tune later.

## 4. Rust rendering crate

| Crate       | What it gives us                    | Fit for v1 |
|-------------|-------------------------------------|-----------|
| `resvg`     | SVG → pixmap (RGBA)                 | **Yes** |
| `tiny-skia` | 2D drawing primitives (deps of resvg) | Indirect |
| `image`     | Raster ops, dithering, BMP encode   | **Yes** |

### Decision

- `resvg` for layout (SVG templates are the fastest
  way to iterate on a dashboard — designer-friendly,
  diff-friendly, no code rebuild for tweaks).
- `image` for the grayscale → dither → BMP tail.
- `tiny-skia` comes in as a transitive dep of resvg;
  we don't consume it directly in v1.

Future: if SVG-as-template becomes limiting (dynamic
charts, arbitrary sparklines), add a `chart-rs` or
`plotters` step that drops SVG fragments into the
layout before resvg. Defer until we hit it.

## 5. Home Assistant auth

**Long-lived access token.** Generated once in HA's
profile UI, stored in a file referenced by
`token_file` in the config. OAuth is overkill for a
single-tenant RPi project and would require us to host
a redirect URL.

Config:

```toml
[home_assistant]
base_url = "http://homeassistant.local:8123"
token_file = "/etc/bellwether/ha_token"
```

The file contents are the bare token string. The
config loader reads the file at startup (not on every
request).

Entities to fetch are defined in a later PR —
probably a `[[home_assistant.entities]]` list with
name + entity_id. The control panel (Svelte frontend)
eventually edits this, but v1 hand-writes the TOML.

## 6. Windy Point Forecast

The Point Forecast v2 API takes a POST with lat/lon,
model (default `gfs`), parameters (wind, temp, precip,
pressure, etc.), and returns a time-indexed JSON.

Key points for the config:

```toml
[windy]
api_key_file = "/etc/bellwether/windy_key"
lat = 46.05
lon = 14.51       # defaults; real values come later
model = "gfs"
parameters = ["temp", "wind", "precip"]
```

Rate limits: Windy's personal/developer plan allows
hundreds of calls/day; we'll call ~4× per hour at
most. Not a constraint.

**User note from handoff:** don't ask for new Windy
keys — reuse the existing annual subscription.

## 7. TRMNL config sketch

```toml
[trmnl]
mode = "byos"                # or "webhook"

# BYOS mode: our server is polled by the device.
# We only need to know what to serve; the device has
# our URL baked in via its own setup.
[trmnl.byos]
public_image_base = "http://malina.local:3100/images"
default_refresh_rate_s = 900

# Webhook mode: we push to trmnl.com.
[trmnl.webhook]
url = "https://trmnl.com/api/custom_plugins/<uuid>"
content_type = "image/bmp"
```

Only one of `[trmnl.byos]` / `[trmnl.webhook]` is read
depending on `mode`. The other is ignored.

## 8. Open risks & follow-ups

1. **Exact BYOS JSON schema.** The public docs point
   at the Terminus source. Before PR 3, skim
   `usetrmnl/terminus/app/controllers/api` (or the
   openapi file if one exists) to lock field names.
2. **Device provisioning flow.** BYOS requires
   flashing or configuring the device's server URL.
   The user owns the device — confirm this is viable
   before committing to Path A. If not, Path B is
   ready.
3. **Dithering quality.** Floyd–Steinberg on a
   grayscale source looks fine for weather icons and
   text, less fine for gradient backgrounds. Keep
   layouts high-contrast in v1.
4. **Time zones.** HA and Windy both return UTC. The
   dashboard shows local time (Europe/Ljubljana).
   Config needs a `[render] timezone = "..."` entry.
5. **Scheduling.** Not in first three PRs. `tokio` +
   a cron-ish trigger (`tokio-cron-scheduler` or a
   hand-rolled tick loop). Decide when we get there.

## 9. Ready to start PR 1

The config skeleton assumes:

- sections `[windy]`, `[trmnl]`, `[render]`
  (**no `[home_assistant]`** — deferred to backlog)
- secret-bearing values live in separate files
  (`api_key_file`)
- `[trmnl]` has a `mode` discriminator (`byos` is the
  default; `webhook` is wired but unused for v1)
- dimensions default to `800×480`, bit depth `1`

This document is the spike. Implementation starts with
PR 1 — narrower than originally spec'd in `HANDOFF.md`
because HA moved to backlog.
