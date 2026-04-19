use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use axum::body::Bytes;
use bellwether::clients::open_meteo::{
    Client as OpenMeteoClient, FetchRequest as OpenMeteoFetchRequest,
    OpenMeteoProvider,
};
use bellwether::config::{
    Config, ProviderKind, RenderConfig, TrmnlConfig, WeatherConfig,
};
use bellwether::dashboard::layout::Layout;
use bellwether::publish::{PublishLoop, PublishLoopConfig, supervise};
use bellwether::render::Renderer;
use bellwether::weather::WeatherProvider;
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
    #[arg(short, long, default_value = "3100")]
    port: u16,

    /// Bind address
    #[arg(short, long, default_value = "127.0.0.1")]
    bind: IpAddr,
}

/// Everything `build_trmnl_state` needs to produce and
/// everything `main` needs afterwards to wire the
/// publish loop.
struct Startup {
    trmnl: TrmnlState,
    /// Weather config when `--config` was given; `None`
    /// in `--dev` mode. Used to spawn the publish loop.
    weather: Option<WeatherConfig>,
    render_cfg: RenderConfig,
    refresh_interval: RefreshInterval,
    /// Effective dashboard layout — from config's
    /// `[dashboard]` section if present, else the
    /// embedded default.
    dashboard_layout: Layout,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(
            |_| {
                // `bellwether` is the library crate — the
                // publish loop's INFO/WARN lines
                // ("published image", "publish tick
                // failed") come from there and must stay
                // visible by default, otherwise silent
                // fetch failures look like a missing BMP.
                EnvFilter::new(
                    "bellwether=info,bellwether_web=debug,\
                     tower_http=debug",
                )
            },
        ))
        .init();

    let cli = Cli::parse();
    let startup = build_startup(&cli)?;

    if let Some(weather) = &startup.weather {
        spawn_publish_loop(
            weather,
            startup.trmnl.clone(),
            startup.refresh_interval,
            startup.render_cfg.clone(),
            startup.dashboard_layout.clone(),
        )?;
    } else {
        tracing::info!(
            "--dev mode: skipping publish loop; /api/display will \
             keep serving the placeholder",
        );
    }

    let app = api::create_router(startup.trmnl);

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
    let (
        weather,
        public_image_base,
        refresh_interval,
        render_cfg,
        dashboard_layout,
    ) = resolve_serving_config(cli)?;

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
        weather,
        render_cfg,
        refresh_interval,
        dashboard_layout,
    })
}

/// Returns `(weather, public_image_base, interval, render_cfg)`.
/// `weather` is `Some` when `--config` was given and
/// `None` for `--dev`.
fn resolve_serving_config(
    cli: &Cli,
) -> Result<(
    Option<WeatherConfig>,
    String,
    RefreshInterval,
    RenderConfig,
    Layout,
)> {
    match (&cli.config, cli.dev) {
        (Some(path), _) => {
            let (weather, base, interval, render, layout) =
                load_byos_config(path)?;
            Ok((Some(weather), base, interval, render, layout))
        }
        (None, true) => {
            tracing::warn!(
                "--dev: running with localhost defaults; /api/display \
                 image_url will not resolve from a real TRMNL device",
            );
            Ok((
                None,
                "http://localhost:3100/images".to_owned(),
                RefreshInterval::from_secs(900),
                RenderConfig::default(),
                Layout::embedded_default().clone(),
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
) -> Result<(WeatherConfig, String, RefreshInterval, RenderConfig, Layout)> {
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
    let layout = cfg.dashboard_layout().clone();
    let Config { weather, .. } = cfg;
    Ok((weather, base, interval, render, layout))
}

fn seed_placeholder(
    state: &TrmnlState,
    render_cfg: &RenderConfig,
) -> Result<()> {
    let bmp = Renderer::with_default_fonts()
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
/// auto-restart: a crash-loop would spam the log (and
/// hammer any future paid provider's API quota) within
/// minutes. If the task dies, the operator investigates
/// and restarts the process.
fn spawn_publish_loop(
    weather_cfg: &WeatherConfig,
    trmnl: TrmnlState,
    refresh_interval: RefreshInterval,
    render_cfg: RenderConfig,
    dashboard_layout: Layout,
) -> Result<()> {
    let provider = build_provider(weather_cfg)?;
    let publish_loop = PublishLoop::new(
        provider,
        Renderer::with_default_fonts(),
        trmnl,
        PublishLoopConfig {
            render_cfg,
            layout: dashboard_layout,
            interval: refresh_interval.as_duration(),
        },
    );
    supervise("publish_loop", publish_loop.run());
    tracing::info!(
        "publish loop spawned (provider {}, fetch interval {} s)",
        weather_cfg.provider.name(),
        refresh_interval.as_secs(),
    );
    Ok(())
}

/// Construct the concrete [`WeatherProvider`] for the
/// configured provider tag and wrap it in an
/// `Arc<dyn …>` for the publish loop.
///
/// Provider construction is infallible: the
/// `[weather.<provider>]` subtable invariant is
/// enforced at [`Config::load`] time, so this
/// function can pattern-match on the provider kind
/// and pull the already-validated subtable without
/// re-checking.
fn build_provider(
    weather_cfg: &WeatherConfig,
) -> Result<Arc<dyn WeatherProvider>> {
    match weather_cfg.provider {
        ProviderKind::OpenMeteo => {
            let sub = weather_cfg.open_meteo.as_ref().with_context(|| {
                "[weather.open_meteo] subtable missing; this should \
                 have been caught by Config::load's validation"
            })?;
            let fetch_request = OpenMeteoFetchRequest::from_parts(
                weather_cfg.lat,
                weather_cfg.lon,
                sub,
            );
            Ok(Arc::new(OpenMeteoProvider::new(
                OpenMeteoClient::new(),
                fetch_request,
            )))
        }
    }
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
    tracing::info!("shutting down");
}
