use clap::Parser;

#[derive(Parser)]
#[command(name = "bellwether", version, about)]
struct Cli {
    /// Example flag
    #[arg(short, long)]
    verbose: bool,
}

#[allow(clippy::unnecessary_wraps)]
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.verbose {
        println!("verbose mode enabled");
    }

    println!("Hello from bellwether!");

    // TODO: add your application logic here

    Ok(())
}
