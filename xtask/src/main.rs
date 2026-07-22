//! LemurClaw xtask: release tooling for publishing the codex-rs workspace to
//! crates.io under the `lemurclaw-*` name.
//!
//! Phase 0 (`verify`): measure compressed tarball sizes and probe whether the
//! `[patch.crates-io]` fork dependencies are actually required.
//!
//! Phase 1 (`publish rename`): generate a parallel `publish/` workspace where
//! every `codex-*` crate has been renamed to `lemurclaw-*` without touching the
//! original `codex-rs/` source tree.

mod manifest;
mod rename;
mod source_rewrite;
mod verify;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask", about = "LemurClaw release tooling")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Phase 0: run pre-publish verification probes.
    Verify {
        #[command(subcommand)]
        kind: VerifyKind,
    },
    /// Phase 1: generate the parallel publish workspace.
    Publish {
        #[command(subcommand)]
        kind: PublishKind,
    },
}

#[derive(Subcommand)]
enum VerifyKind {
    /// Measure the compressed `.crate` tarball size for the largest crates.
    Size,
    /// Probe whether the `[patch.crates-io]` fork deps are required.
    Patches,
}

#[derive(Subcommand)]
enum PublishKind {
    /// Rename `codex-*` to `lemurclaw-*` and emit the publish/ workspace.
    Rename,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Verify { kind } => match kind {
            VerifyKind::Size => verify::run_size(),
            VerifyKind::Patches => verify::run_patches(),
        },
        Command::Publish { kind } => match kind {
            PublishKind::Rename => rename::run(),
        },
    }
}
