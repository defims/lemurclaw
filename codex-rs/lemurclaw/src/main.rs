use clap::Parser;
use lemurclaw::{config::Cli, run};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    run(cli.into())
}
