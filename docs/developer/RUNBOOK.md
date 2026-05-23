# Operational runbook

Symptom-keyed diagnostic playbooks for the TRMNL +
bellwether-web deployment on `malina`. Where the
`/trmnl-expert` skill is the **protocol reference**,
this document is the **"I see X, do Y" guide** for
operators triaging a live problem.

For protocol / schema questions, see
`/trmnl-expert` (`.claude/skills/trmnl-expert/SKILL.md`).
For project history and architectural choices, see
`docs/developer/HANDOFF.md` and `docs/developer/DIARY.md`.

---

## "Device shows noise on the panel"

E-ink retains the last drawn frame with zero power.
"Noise" usually means a partial / aborted panel
write — most often from battery brownout during the
refresh-cycle current spike (~150 mA for ~3 s).
Order of investigation:

1. **Is the device WiFi-reachable from malina?**
   ```bash
   ssh igor@malina "ping -c 3 -W 2 <device_ip>"
   ```
   0% reply = device is off or off-network.
2. **ARP entry?**
   ```bash
   ssh igor@malina "arp -n | grep <device_ip>"
   ```
   No entry = device hasn't been WiFi-associated
   recently. Combined with #1, the device is fully
   offline.
3. **Is the publish loop healthy?**
   ```bash
   ssh igor@malina "curl http://localhost:9300/api/display"
   ```
   Filename should change every ~5 min. If yes,
   server-side is fine and the issue is downstream
   of malina.
4. **Battery state.** Plug in USB. The device may
   need 30+ minutes of charge to sustain a full
   refresh-cycle without brownout. The status LED
   should be green throughout.

If steps 1-3 all pass but the panel still shows
noise, the device's e-ink buffer needs a clean
refresh — a reset-button press or a long enough USB
charge to drive one full cycle without brownout
will clear it.

---

## "Battery indicator is stuck on em-dash"

The dashboard renders an em-dash placeholder when
`TrmnlState.telemetry.battery_voltage` is `None`.
Two paths feed that cache:

1. **`/api/display`** reads `Battery-Voltage` header
   on every device poll (~5 min). Fast feedback.
2. **`/api/log`** reads `battery_voltage` from the
   per-entry JSON body when the device POSTs an
   event log. Sparse, event-driven.

If the indicator is stuck:

1. Has any device poll happened since the bellwether-web
   service started?
   ```bash
   ssh igor@malina "sudo ss -tnp 'sport = :9300' | grep <device_ip>"
   ```
   Even one ESTABLISHED connection from the device
   IP means a recent poll. None = device hasn't
   connected since service start.
2. Are the headers actually arriving?
   ```bash
   ssh igor@malina "sudo journalctl -u bellwether-web -n 50 --no-pager | grep -i 'battery\|telemetry\|tower_http'"
   ```
   If `tower_http=warn` is the configured level,
   422s on `/api/log` would appear here. If you see
   422s, the JSON shape drifted from the firmware
   schema — re-verify against `usetrmnl/firmware:lib/trmnl/src/serialize_log.cpp`.
3. Is the parser rejecting the header value?
   Temporarily raise log level:
   ```bash
   sudo systemctl edit bellwether-web
   # add: Environment=RUST_LOG=bellwether=info,bellwether_web=debug
   sudo systemctl restart bellwether-web
   ```
   The handler's `parse_battery_voltage_header`
   returns `None` silently on missing / non-UTF-8 /
   non-float / non-finite values. A debug log of the
   raw header value would reveal what's actually
   arriving.

Historical bug (resolved in v0.27.1 / v0.27.2): the
`/api/log` parser expected a flat-shape body but
the firmware sends `{"logs": [...]}`. If you ever
see `battery_voltage=None` with `extra_keys=1`
across many lines, the parser is drifting from the
firmware schema again — re-verify against the
firmware source (pinned commit listed in
`/trmnl-expert`).

---

## "Device hasn't POSTed `/api/log` in hours"

This is normal during stable operation. The device
only POSTs `/api/log` when it has queued events
(boot, error, retry, threshold crossing). A
healthy device that's just polling for images can
go for hours without an event log.

**Do not diagnose battery state from `/api/log`
cadence.** Use `/api/display` headers (faster) or
ping/ARP (fastest).

To force a log: short-press the reset button on the
back of the device. Boot sequence always queues at
least one log entry.

---

## "Zombie ESTABLISHED connections on the server"

```bash
ssh igor@malina "sudo ss -tnp 'sport = :9300'"
# ESTAB 0  0  192.168.x.x:9300  <device_ip>:62830  ...
# ESTAB 0  0  192.168.x.x:9300  <device_ip>:63655  ...
# ...
```

Expected on a long-running deployment. The device
deep-sleeps without sending FIN; the server kernel
keeps the socket open until its own keepalive timer
reaps it (typically 2 hours by default). A service
restart forcibly closes them:

```bash
ssh igor@malina "sudo systemctl restart bellwether-web"
```

Worth flagging if the count exceeds ~20
(file-descriptor pressure). Tracked in `TODO.md` as
"Idle-connection timeout on bellwether-web HTTP
routes".

---

## "Deploy crash-loops the service"

If `cargo xtask deploy` shows the service in
`activating (auto-restart)` state, the new binary
is being killed at startup. Most likely cause is a
stale `bellwether-web.service` unit file on the
device referencing a CLI flag the new binary
doesn't recognize (the v0.16.0 `--frontend`
regression).

The deploy tool syncs the unit file when it
differs from `deploy/bellwether-web.service` in the
repo (added in v0.23.1), so this should be
self-healing now. If it isn't:

```bash
ssh igor@malina "sudo journalctl -u bellwether-web -n 20 --no-pager"
# Look for: error: unexpected argument '--foo' found
ssh igor@malina "cat /etc/systemd/system/bellwether-web.service"
# Compare against repo's deploy/bellwether-web.service
```

If the installed unit file is stale, force-update:

```bash
scp deploy/bellwether-web.service igor@malina:/tmp/
ssh igor@malina "sudo mv /tmp/bellwether-web.service /etc/systemd/system/ \
  && sudo systemctl daemon-reload \
  && sudo systemctl restart bellwether-web"
```

---

## "Dashboard renders but battery indicator shows wrong value"

Possible causes:

1. **Stale cache.** The cached voltage is the last
   reading from any poll; nothing invalidates it on
   age. A device that goes offline keeps its last
   reading visible. There's no current code to
   timestamp the cache. Future work: track
   `last_seen_at` and render `?` instead of a
   percentage if older than some threshold.
2. **Wrong voltage→percent mapping.** The mapping
   in `crates/bellwether/src/telemetry.rs::battery_voltage_to_pct`
   is linear 3.3V→0%, 4.2V→100%. If the device
   reports voltages outside this range
   consistently, the mapping endpoints may need
   adjustment for your specific battery chemistry.
   Check the device's actual reported voltages
   via:
   ```bash
   ssh igor@malina "sudo journalctl -u bellwether-web --since '1 day ago' --no-pager | grep battery_voltage | grep -v None"
   ```

---

## "Image updated but dashboard data looks stale"

The publish loop fetches Open-Meteo every 5 min by
default. If the dashboard shows yesterday's
weather:

1. Check publish-loop timing in journalctl —
   should see `published image filename=...` every
   ~5 min.
2. Check Open-Meteo response:
   ```bash
   ssh igor@malina "curl 'https://api.open-meteo.com/v1/forecast?latitude=...&longitude=...&current=temperature_2m'"
   ```
   Configured lat/lon are in `config.toml`.
3. Check `/api/status` (if implemented) for the
   last-fetched timestamp.

---

## Common one-liners

```bash
# Last 5 device-log lines
ssh igor@malina "sudo journalctl -u bellwether-web --no-pager | grep 'trmnl device log' | tail -5"

# Recent battery readings (non-None)
ssh igor@malina "sudo journalctl -u bellwether-web --since '1 day ago' --no-pager | grep battery_voltage | grep -v None | tail -10"

# Active TCP connections from device IP
ssh igor@malina "sudo ss -tnp 'sport = :9300'"

# Latest BMP the device would fetch
ssh igor@malina "curl -s http://localhost:9300/api/display | jq"

# Service restart (clears zombie sockets, restarts publish loop)
ssh igor@malina "sudo systemctl restart bellwether-web"

# Tail logs live
ssh igor@malina "sudo journalctl -u bellwether-web -f"
```
