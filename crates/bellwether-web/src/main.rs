use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use axum::body::Bytes;
use bellwether::config::{Config, RenderConfig, TrmnlConfig};
use bellwether::render::Renderer;
use clap::Parser;
use tracing_subscriber::EnvFilter;

mod api;

use api::{RefreshInterval, TrmnlState};

/// Env var: if non-empty, every TRMNL BYOS request must
/// include an `Access-Token` header matching this.
const ACCESS_TOKEN_ENV: &str = "BELLWETHER_ACCESS_TOKEN";

#[derive(Parser)]
#[command(name = "bellwether-web", version, about)]
struct Cli {
    /// Path to the TOML config file. Required unless
    /// `--dev` is set.
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Run without a config file using developer
    /// defaults (localhost image base, 900 s refresh).
    /// Only use for local frontend development — the
    /// resulting image URLs will not resolve from a real
    /// TRMNL device on the LAN.
    #[arg(long, default_value_t = false)]
    dev: bool,

    /// Port to listen on
    #[arg(short, long, default_value = "3000")]
    port: u16,

    /// Bind address
    #[arg(short, long, default_value = "127.0.0.1")]
    bind: IpAddr,

    /// Path to frontend dist directory
    #[arg(short, long, default_value = "frontend/dist")]
    frontend: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(
            |_| EnvFilter::new("bellwether_web=debug,tower_http=debug"),
        ))
        .init();

    let cli = Cli::parse();
    let trmnl = build_trmnl_state(&cli)?;

    let app = api::create_router(&cli.frontend, trmnl);

    let addr = SocketAddr::new(cli.bind, cli.port);
    tracing::info!("listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .context("binding listener")?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;
    Ok(())
}

/// Build [`TrmnlState`] from config and seed the image
/// store with a placeholder so devices that poll before
/// the first real render see a valid BMP instead of a
/// 503.
///
/// Fails fast on misconfiguration, missing access token
/// file, or placeholder render failure. The argument is
/// that a TRMNL server with a broken renderer is
/// useless; making the operator notice at startup is
/// strictly better than serving 503 forever.
fn build_trmnl_state(cli: &Cli) -> Result<TrmnlState> {
    let (public_image_base, refresh_interval, render_cfg) =
        resolve_serving_config(cli)?;

    let access_token = std::env::var(ACCESS_TOKEN_ENV).unwrap_or_default();
    if access_token.is_empty() {
        tracing::warn!(
            "no {} set; TRMNL endpoints are unauthenticated \
             (fine for a LAN-only deployment, a bad idea on a \
             public interface)",
            ACCESS_TOKEN_ENV,
        );
    }

    let state = TrmnlState::new(&public_image_base, refresh_interval)
        .with_context(|| {
            format!("invalid public_image_base {public_image_base:?}")
        })?
        .with_access_token(&access_token);

    seed_placeholder(&state, &render_cfg)?;
    Ok(state)
}

/// Compute the (base URL, refresh interval, render
/// config) triple from CLI + TOML. Returns an error if
/// `--config` is missing without `--dev`, or if the
/// config's TRMNL mode is anything other than `byos`.
fn resolve_serving_config(
    cli: &Cli,
) -> Result<(String, RefreshInterval, RenderConfig)> {
    match (&cli.config, cli.dev) {
        (Some(path), _) => load_byos_triple(path),
        (None, true) => {
            tracing::warn!(
                "--dev: running with localhost defaults; /api/display \
                 image_url will not resolve from a real TRMNL device",
            );
            Ok((
                "http://localhost:3000/images".to_owned(),
                RefreshInterval::from_secs(900),
                RenderConfig::default(),
            ))
        }
        (None, false) => {
            bail!(
                "--config <FILE> is required (pass --dev to run \
                 with developer defaults)",
            );
        }
    }
}

fn load_byos_triple(
    path: &Path,
) -> Result<(String, RefreshInterval, RenderConfig)> {
    let cfg = Config::load(path)
        .with_context(|| format!("loading config from {}", path.display()))?;
    let TrmnlConfig::Byos(byos) = &cfg.trmnl else {
        bail!(
            "bellwether-web currently only supports \
             trmnl.mode = \"byos\"; found \"{}\"",
            cfg.trmnl.mode_name(),
        );
    };
    Ok((
        byos.public_image_base.clone(),
        RefreshInterval::from_secs(byos.default_refresh_rate_s),
        cfg.render,
    ))
}

/// Render the placeholder SVG and insert it into the
/// store as `placeholder.bmp`. Errors bubble up so
/// operator-visible misconfigurations (broken renderer,
/// bad dimensions) fail at startup.
fn seed_placeholder(
    state: &TrmnlState,
    render_cfg: &RenderConfig,
) -> Result<()> {
    let bmp = Renderer::new()
        .placeholder_bmp(render_cfg)
        .context("rendering placeholder image")?;
    state
        .put_image("placeholder.bmp".into(), Bytes::from(bmp))
        .context("storing placeholder image")?;
    tracing::info!("seeded placeholder image");
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
    tracing::info!("shutting down");
}
