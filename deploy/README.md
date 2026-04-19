# Deploying bellwether-web on Raspberry Pi

Two xtask commands cover the lifecycle:

| Command | Purpose |
|---------|---------|
| `cargo xtask deploy-setup` | One-time provisioning |
| `cargo xtask deploy`       | Repeatable deploy |

The binary is built **on the RPi** (no cross-compile
toolchain needed locally). First build is slow; later
ones are incremental thanks to a persisted `target/`
cache under `~/bellwether-build`.

## Prerequisites

On your dev machine:

- SSH access to the RPi (key-based, no password prompts)
- A working `config.toml` at the project root
  (copy from `config.example.toml` and customise). Set
  `trmnl.public_image_base` to the LAN-visible URL of
  the RPi, e.g. `http://malina.local:9300/images`.

On the RPi (one-time, manual):

- Rust toolchain for the `pi`/`igor` user
  (`curl https://sh.rustup.rs -sSf | sh`)
- Build deps: `sudo apt install build-essential pkg-config`

## First deploy

1. Copy `.deploy.sample` to `.deploy` and fill in
   `rpi_host`, `rpi_user`.
2. `cargo xtask deploy-setup` — creates the
   `bellwether` system user, copies `config.toml`,
   installs the systemd unit.
3. `cargo xtask deploy` — builds and starts the
   service.
4. Open `http://<rpi>:9300` to confirm the control
   panel is up.

## Updating

```bash
cargo xtask deploy
```

This rebuilds source on the RPi, atomically swaps the
binary, and restarts the service. It will fail loudly
if the service doesn't come back up `active`.

## Changing config

`config.toml` is only copied during `deploy-setup`. To
update it later, either:

- Re-run `cargo xtask deploy-setup` (idempotent), or
- `scp config.toml pi@malina:~/ && ssh pi@malina 'sudo
  mv ~/config.toml /opt/bellwether/ && sudo chown
  bellwether:bellwether /opt/bellwether/config.toml &&
  sudo systemctl restart bellwether-web'`

## Service management

```bash
sudo systemctl status bellwether-web
journalctl -u bellwether-web -f
sudo systemctl restart bellwether-web
```

Defaults (baked into `deploy/bellwether-web.service`):

| Setting | Value                       |
|---------|-----------------------------|
| Port    | 9300                        |
| Bind    | 0.0.0.0 (all interfaces)    |
| Config  | /opt/bellwether/config.toml |

To override (e.g. different port), use a systemd drop-in:

```bash
sudo systemctl edit bellwether-web
```

Then:

```ini
[Service]
ExecStart=
ExecStart=/opt/bellwether/bellwether-web \
    --config /opt/bellwether/config.toml \
    --port 8080 --bind 0.0.0.0
```

## Firewall

The service binds to `0.0.0.0:9300` with no TLS and no
access token. Restrict to trusted devices:

```bash
sudo ufw allow from 192.168.1.0/24 to any port 9300
```

## Pointing a TRMNL device at it

Configure the TRMNL device (BYOS mode) with the
server base URL, e.g. `http://malina.local:9300`.
See the TRMNL docs for the device-side setup.
