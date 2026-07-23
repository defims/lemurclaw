//! LemurClaw xtask: release tooling for publishing the codex-rs workspace to
//! crates.io under the `lemurclaw-*` name.
//!
//! Phase 0 (`verify`): measure compressed tarball sizes and probe whether the
//! `[patch.crates-io]` fork dependencies are actually required.
//!
//! Phase 1 (`publish rename`): generate a parallel `publish/` workspace where
//! every `codex-*` crate has been renamed to `lemurclaw-*` without touching the
//! original `codex-rs/` source tree.
//!
//! Phase 2 (`publish bundle`): merge the 23 `lemurclaw-utils-*` crates in
//! `publish/` into a single `lemurclaw-utils` crate (each as a `pub mod`).

mod bundle;
mod forks;
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
    /// Phase 1 / 1.5: generate the parallel publish workspace and publish forks.
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
    /// Phase 1: rename `codex-*` to `lemurclaw-*` and emit the publish/ workspace.
    Rename,
    /// Phase 1.5: publish the 4 git forks as `lemurclaw-*` crates and rewire
    /// the publish workspace to reference them.
    Fork {
        #[command(subcommand)]
        kind: ForkKind,
    },
    /// Phase 2/3: merge a cluster of crates in publish/ into a single
    /// mega-crate (each former crate becomes a `pub mod`).
    Bundle {
        /// Which cluster to merge.
        #[arg(long, default_value = "utils")]
        target: BundleTarget,
        /// Print the migration plan without modifying any files.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum BundleTarget {
    /// Merge the 17 cycle-free `lemurclaw-utils-*` crates.
    Utils,
    /// Merge core's 84-crate closure into `lemurclaw-core` (utils disappears).
    Core,
}

#[derive(Subcommand)]
enum ForkKind {
    /// Clone the 4 fork repositories to publish.forks/.
    Clone,
    /// Rewrite each fork's Cargo.toml (package.name + internal dep aliases).
    Prepare,
    /// Publish the 4 forks to crates.io in topological order.
    Publish {
        /// Run `cargo publish --dry-run` instead of actually publishing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Rewrite publish/Cargo.toml to reference the published forks via
    /// `package = "lemurclaw-X"` aliases, dropping [patch.crates-io].
    Rewire,
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
            PublishKind::Fork { kind } => match kind {
                ForkKind::Clone => forks::run_clone(),
                ForkKind::Prepare => forks::run_prepare(),
                ForkKind::Publish { dry_run } => forks::run_publish(dry_run),
                ForkKind::Rewire => forks::run_rewire(),
            },
            PublishKind::Bundle { target, dry_run } => {
                let cluster = match target {
                    BundleTarget::Utils => bundle::utils_cluster(),
                    BundleTarget::Core => bundle::core_cluster()?,
                };
                bundle::run(&cluster, dry_run)
            }
        },
    }
}
