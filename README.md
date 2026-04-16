# trmnl-bellwether

TRMNL e-ink dashboard server: aggregates data from
[Home Assistant](https://www.home-assistant.io/) and the
[Windy Point Forecast API](https://api.windy.com/point-forecast/docs),
renders server-side e-ink layouts, and serves them to a
[TRMNL](https://trmnl.com/) e-paper display via webhook.

Status: **idea / scaffold** (generated from
[rustbase](https://github.com/breki/rustbase)).

## What's planned

- Fetch Home Assistant entity state via HA REST API
- Fetch weather from the Windy Point Forecast API
  (annual subscription already in place)
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
cd frontend && npm install    # first time
.\build.ps1 dev               # backend + frontend
```

Open http://localhost:5173. Vite proxies `/api` and
`/health` to the Axum backend on port 3000.

E2E tests:

```bash
npx playwright test           # auto-starts servers
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
