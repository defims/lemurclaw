use clap::Parser;
use lemurclaw::config::Cli;
use lemurclaw::run;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    run(cli.into())
}
