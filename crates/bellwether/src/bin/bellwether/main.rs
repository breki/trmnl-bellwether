use std::path::PathBuf;

use anyhow::Context;
use bellwether::config::Config;
use clap::Parser;

#[derive(Parser)]
#[command(name = "bellwether", version, about)]
struct Cli {
    /// Path to the TOML config file.
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Enable verbose output.
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.verbose {
        println!("verbose mode enabled");
    }

    if let Some(path) = &cli.config {
        let cfg = Config::load(path).with_context(|| {
            format!("loading config from {}", path.display())
        })?;
        println!("loaded config: {cfg}");
    } else {
        println!("Hello from bellwether!");
    }

    Ok(())
}
