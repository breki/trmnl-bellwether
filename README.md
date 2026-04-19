# trmnl-bellwether

TRMNL e-ink dashboard server: aggregates data from
[Home Assistant](https://www.home-assistant.io/) and
[Open-Meteo](https://open-meteo.com/), renders
server-side e-ink layouts, and serves them to a
[TRMNL](https://trmnl.com/) e-paper display via webhook.

Status: **idea / scaffold** (generated from
[rustbase](https://github.com/breki/rustbase)).

## What's planned

- Fetch Home Assistant entity state via HA REST API
- Fetch weather from the Open-Meteo Forecast API
  (free, keyless)
- Render customizable layouts server-side as e-ink
  friendly images (black/white or grayscale)
- Serve images to TRMNL via its webhook protocol
- Run as a daemon on a Raspberry Pi (`malina`)
- Web control panel for layout editing / entity
  selection (Svelte 5 + Axum)

## Architecture

| Crate | Purpose |
|-------|---------|
| `crates/bellwether` | Core library + CLI binary |
| `crates/bellwether-web` | Axum web server, webhook + control panel |
| `xtask` | Build automation |

## Development

```bash
cargo xtask validate          # full quality check
cargo run -p bellwether       # run CLI
```

Web dev loop:

```bash
cargo run -p bellwether-web -- --dev
```

Open http://localhost:3100 for the landing page (lists
available endpoints and shows the latest rendered
dashboard). `--dev` skips the publish loop, useful for
iterating on endpoints without live data.

## Running the server

Copy the example config and tune it for your point of
interest:

```bash
cp config.example.toml config.toml
# edit config.toml: set lat / lon / timezone
```

`config.toml` is gitignored. No API key needed —
Open-Meteo is free and keyless.

Start the server:

```bash
cargo run -p bellwether-web -- --config config.toml
```

The publish loop fires immediately, then every
`default_refresh_rate_s`. Watch for a `published
image` entry in the log, then:

```bash
curl http://localhost:3100/api/display | jq
curl http://localhost:3100/images/dash-00000000.bmp > current.bmp
```

For a quick look at the pipeline without going through
Open-Meteo at all, use `--dev` (serves only the
built-in placeholder, no fetch loop):

```bash
cargo run -p bellwether-web -- --dev
curl http://localhost:3100/images/placeholder.bmp > placeholder.bmp
```

## Prerequisites

- Rust stable (via `rust-toolchain.toml`)
- Node.js 22+ (for the web control panel)
- `cargo-llvm-cov` for coverage:
  `cargo install cargo-llvm-cov`
- `code-dupes` for duplication checks:
  `cargo install code-dupes`

## License

MIT
