mod check;
mod clippy_cmd;
mod coverage;
mod deploy;
mod deploy_config;
mod deploy_remote;
mod deploy_setup;
mod dupes;
mod fmt_cmd;
mod helpers;
mod preview;
mod test_cmd;
mod validate;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask")]
struct Cli {
    #[command(subcommand)]
    command: XCommand,
}

#[derive(Subcommand)]
enum XCommand {
    /// Fast compilation check (no tests)
    Check,
    /// Run clippy (deny warnings)
    Clippy,
    /// Run all tests
    Test {
        /// Optional test filter
        filter: Option<String>,
        /// Show raw cargo test output
        #[arg(long)]
        verbose: bool,
        /// Run only tests marked `#[ignore]`. Matches
        /// `cargo test -- --ignored` exactly: the
        /// non-ignored suite is skipped. Combine with a
        /// filter to run a single manual tool, e.g.
        /// `cargo xtask test --ignored generate_dashboard_sample_bmp`.
        #[arg(long)]
        ignored: bool,
    },
    /// Run fmt + clippy + tests + coverage + duplication
    Validate,
    /// Format code
    Fmt,
    /// Run coverage check (requires cargo-llvm-cov)
    Coverage,
    /// Run code duplication check (requires code-dupes)
    Dupes,
    /// One-time `RPi` provisioning (user, dirs, service)
    DeploySetup,
    /// Build and deploy to the `RPi`
    Deploy,
    /// Regenerate the sample dashboard and serve an
    /// HTML preview (SVG + pre-dither PNG + final BMP)
    /// on a local HTTP port.
    Preview {
        /// TCP port the preview server listens on.
        #[arg(long, default_value_t = 8123)]
        port: u16,
        /// Launch the system's default browser at the
        /// preview URL once the server is ready.
        #[arg(long)]
        open: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        XCommand::Check => check::check(),
        XCommand::Clippy => clippy_cmd::clippy(),
        XCommand::Test {
            filter,
            verbose,
            ignored,
        } => test_cmd::test(test_cmd::TestOptions {
            filter: filter.as_deref(),
            verbose,
            ignored,
        }),
        XCommand::Validate => validate::validate(),
        XCommand::Fmt => fmt_cmd::fmt(),
        XCommand::Coverage => coverage::coverage(),
        XCommand::Dupes => dupes::dupes(),
        XCommand::DeploySetup => {
            deploy_setup::deploy_setup().map_err(|e| format!("{e:#}"))
        }
        XCommand::Deploy => deploy::deploy().map_err(|e| format!("{e:#}")),
        XCommand::Preview { port, open } => preview::preview(port, open),
    };

    if let Err(e) = result {
        eprintln!("xtask error: {e}");
        std::process::exit(1);
    }
}
