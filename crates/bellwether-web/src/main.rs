use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use axum::body::Bytes;
use bellwether::clients::windy::{
    Client as WindyClient, FetchRequest as WindyFetchRequest,
};
use bellwether::config::{Config, RenderConfig, TrmnlConfig, WindyConfig};
use bellwether::publish::{PublishLoop, supervise};
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
    /// defaults (localhost image base, 900 s refresh,
    /// no publish loop). Only use for local frontend
    /// development.
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

/// Everything `build_trmnl_state` needs to produce and
/// everything `main` needs afterwards to wire the
/// publish loop.
struct Startup {
    trmnl: TrmnlState,
    /// Windy config when `--config` was given; `None`
    /// in `--dev` mode. Used to spawn the publish loop.
    windy: Option<WindyConfig>,
    render_cfg: RenderConfig,
    refresh_interval: RefreshInterval,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(
            |_| EnvFilter::new("bellwether_web=debug,tower_http=debug"),
        ))
        .init();

    let cli = Cli::parse();
    let startup = build_startup(&cli)?;

    if let Some(windy) = &startup.windy {
        spawn_publish_loop(
            windy,
            startup.trmnl.clone(),
            startup.refresh_interval,
            startup.render_cfg.clone(),
        )?;
    } else {
        tracing::info!(
            "--dev mode: skipping publish loop; /api/display will \
             keep serving the placeholder",
        );
    }

    let app = api::create_router(&cli.frontend, startup.trmnl);

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

/// Resolve config → state + metadata needed later to
/// wire the publish loop. Fails fast on
/// misconfiguration, missing access token file, or
/// placeholder render failure.
fn build_startup(cli: &Cli) -> Result<Startup> {
    let (windy, public_image_base, refresh_interval, render_cfg) =
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

    let trmnl = TrmnlState::new(&public_image_base, refresh_interval)
        .with_context(|| {
            format!("invalid public_image_base {public_image_base:?}")
        })?
        .with_access_token(&access_token);

    seed_placeholder(&trmnl, &render_cfg)?;

    Ok(Startup {
        trmnl,
        windy,
        render_cfg,
        refresh_interval,
    })
}

/// Returns `(windy, public_image_base, interval, render_cfg)`.
/// `windy` is `Some` when `--config` was given and
/// `None` for `--dev`.
fn resolve_serving_config(
    cli: &Cli,
) -> Result<(Option<WindyConfig>, String, RefreshInterval, RenderConfig)> {
    match (&cli.config, cli.dev) {
        (Some(path), _) => {
            let (windy, base, interval, render) = load_byos_config(path)?;
            Ok((Some(windy), base, interval, render))
        }
        (None, true) => {
            tracing::warn!(
                "--dev: running with localhost defaults; /api/display \
                 image_url will not resolve from a real TRMNL device",
            );
            Ok((
                None,
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

fn load_byos_config(
    path: &Path,
) -> Result<(WindyConfig, String, RefreshInterval, RenderConfig)> {
    let cfg = Config::load(path)
        .with_context(|| format!("loading config from {}", path.display()))?;
    let TrmnlConfig::Byos(byos) = &cfg.trmnl else {
        bail!(
            "bellwether-web currently only supports \
             trmnl.mode = \"byos\"; found \"{}\"",
            cfg.trmnl.mode_name(),
        );
    };
    let base = byos.public_image_base.clone();
    let interval = RefreshInterval::from_secs(byos.default_refresh_rate_s);
    let render = cfg.render.clone();
    let Config { windy, .. } = cfg;
    Ok((windy, base, interval, render))
}

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

/// Spawn the fetch → render → publish loop under the
/// `publish::supervise` wrapper, which logs at
/// `error!` if the task ever ends (clean return or
/// panic). The handle is detached on purpose —
/// supervising means we get a log tripwire for
/// silent-termination bugs, but we deliberately do not
/// auto-restart: a crash-loop would burn through the
/// Windy API quota in minutes. If the task dies, the
/// operator investigates and restarts the process.
fn spawn_publish_loop(
    windy_cfg: &WindyConfig,
    trmnl: TrmnlState,
    refresh_interval: RefreshInterval,
    render_cfg: RenderConfig,
) -> Result<()> {
    let fetch_request = WindyFetchRequest::from_config(windy_cfg)
        .context("building Windy fetch request from config")?;
    let windy = WindyClient::new();
    let publish_loop = PublishLoop::new(
        windy,
        fetch_request,
        Renderer::new(),
        render_cfg,
        trmnl,
        refresh_interval.as_duration(),
    );
    supervise("publish_loop", publish_loop.run());
    tracing::info!(
        "publish loop spawned (fetch interval {} s)",
        refresh_interval.as_secs(),
    );
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
    tracing::info!("shutting down");
}
