use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use clap::Parser;
use tracing_subscriber::EnvFilter;

mod api;

#[derive(Parser)]
#[command(name = "rustbase-web", version, about)]
struct Cli {
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
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(
            |_| EnvFilter::new("rustbase_web=debug,tower_http=debug"),
        ))
        .init();

    let cli = Cli::parse();

    let app = api::create_router(&cli.frontend);

    let addr = SocketAddr::new(cli.bind, cli.port);

    tracing::info!("listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
    tracing::info!("shutting down");
}
