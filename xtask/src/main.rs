mod check;
mod clippy_cmd;
mod coverage;
mod dupes;
mod fmt_cmd;
mod frontend_check;
mod helpers;
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
    };

    if let Err(e) = result {
        eprintln!("xtask error: {e}");
        std::process::exit(1);
    }
}
