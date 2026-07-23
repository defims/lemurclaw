//! Phase 2: merge the 23 fine-grained `lemurclaw-utils-*` crates in `publish/`
//! into a single `lemurclaw-utils` crate, where each former crate becomes a
//! `pub mod <name>` submodule.
//!
//! This is a post-hoc transform on the `publish/` workspace emitted by Phase 1
//! (`xtask publish rename`), mirroring the Phase 1.5 fork-rewire pattern. The
//! source tree `codex-rs/` is never touched, so upstream changes can be merged
//! by re-running the pipeline: `publish rename && publish bundle && publish fork
//! rewire`.
//!
//! ## Why submodule namespacing
//!
//! Each sub-crate keeps its own `pub mod` namespace
//! (`lemurclaw_utils::absolute_path::AbsolutePathBuf`), so the 23 public API
//! surfaces never collide and consumers get an unambiguous import path. This
//! is a crate→module path rewrite, not a 1:1 identifier rename: the crate
//! identifier `lemurclaw_utils_absolute_path` becomes the two-segment path
//! `lemurclaw_utils::absolute_path`.

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// One of the 23 utils sub-crates being folded into `lemurclaw-utils`.
struct SubCrate {
    /// Directory name under `publish/utils/` (from the source layout).
    dir: &'static str,
    /// The `lemurclaw-utils-*` package name assigned by Phase 1 rename.
    package: &'static str,
    /// Module name inside the merged crate (`pub mod <module>;`). Derived from
    /// the crate ident with `lemurclaw_utils_` stripped and trimmed.
    module: &'static str,
}

/// The 23 sub-crates, in the order they appear in the source `Cargo.toml`
/// workspace member list (preserves a stable lib.rs ordering).
///
/// Note the `path-utils` quirk: its directory is `path-utils` but its package
/// name is `lemurclaw-utils-path` (matching upstream `codex-utils-path`), so
/// its crate ident is `lemurclaw_utils_path` and its module is `path`.
const SUB_CRATES: &[SubCrate] = &[
    SubCrate {
        dir: "absolute-path",
        package: "lemurclaw-utils-absolute-path",
        module: "absolute_path",
    },
    SubCrate {
        dir: "approval-presets",
        package: "lemurclaw-utils-approval-presets",
        module: "approval_presets",
    },
    SubCrate {
        dir: "cache",
        package: "lemurclaw-utils-cache",
        module: "cache",
    },
    SubCrate {
        dir: "cargo-bin",
        package: "lemurclaw-utils-cargo-bin",
        module: "cargo_bin",
    },
    SubCrate {
        dir: "cli",
        package: "lemurclaw-utils-cli",
        module: "cli",
    },
    SubCrate {
        dir: "elapsed",
        package: "lemurclaw-utils-elapsed",
        module: "elapsed",
    },
    SubCrate {
        dir: "fuzzy-match",
        package: "lemurclaw-utils-fuzzy-match",
        module: "fuzzy_match",
    },
    SubCrate {
        dir: "home-dir",
        package: "lemurclaw-utils-home-dir",
        module: "home_dir",
    },
    SubCrate {
        dir: "image",
        package: "lemurclaw-utils-image",
        module: "image",
    },
    SubCrate {
        dir: "json-to-toml",
        package: "lemurclaw-utils-json-to-toml",
        module: "json_to_toml",
    },
    SubCrate {
        dir: "oss",
        package: "lemurclaw-utils-oss",
        module: "oss",
    },
    SubCrate {
        dir: "output-truncation",
        package: "lemurclaw-utils-output-truncation",
        module: "output_truncation",
    },
    SubCrate {
        dir: "path-uri",
        package: "lemurclaw-utils-path-uri",
        module: "path_uri",
    },
    SubCrate {
        dir: "path-utils",
        package: "lemurclaw-utils-path",
        module: "path",
    },
    SubCrate {
        dir: "plugins",
        package: "lemurclaw-utils-plugins",
        module: "plugins",
    },
    SubCrate {
        dir: "pty",
        package: "lemurclaw-utils-pty",
        module: "pty",
    },
    SubCrate {
        dir: "readiness",
        package: "lemurclaw-utils-readiness",
        module: "readiness",
    },
    SubCrate {
        dir: "rustls-provider",
        package: "lemurclaw-utils-rustls-provider",
        module: "rustls_provider",
    },
    SubCrate {
        dir: "sandbox-summary",
        package: "lemurclaw-utils-sandbox-summary",
        module: "sandbox_summary",
    },
    SubCrate {
        dir: "sleep-inhibitor",
        package: "lemurclaw-utils-sleep-inhibitor",
        module: "sleep_inhibitor",
    },
    SubCrate {
        dir: "stream-parser",
        package: "lemurclaw-utils-stream-parser",
        module: "stream_parser",
    },
    SubCrate {
        dir: "string",
        package: "lemurclaw-utils-string",
        module: "string",
    },
    SubCrate {
        dir: "template",
        package: "lemurclaw-utils-template",
        module: "template",
    },
];

/// The merged crate's package name and lib identifier.
const MERGED_PACKAGE: &str = "lemurclaw-utils";
const MERGED_LIB_IDENT: &str = "lemurclaw_utils";

fn locate_repo_root() -> Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if manifest_dir.join("..").join("codex-rs").exists() {
        return Ok(manifest_dir.join("..").canonicalize()?);
    }
    let cwd = std::env::current_dir().context("get cwd")?;
    let mut candidate = cwd;
    loop {
        if candidate.join("codex-rs").is_dir() {
            return Ok(candidate);
        }
        if !candidate.pop() {
            break;
        }
    }
    anyhow::bail!("could not locate repo root containing codex-rs/")
}

/// Build the lookup table mapping a former crate identifier
/// (`lemurclaw_utils_absolute_path`) to its submodule name (`absolute_path`).
/// Used by the `.rs` rewrite pass.
fn ident_to_module() -> BTreeMap<&'static str, &'static str> {
    SUB_CRATES
        .iter()
        .map(|sc| {
            // crate ident = package name with dashes → underscores
            let ident: &'static str = sc.package.replace('-', "_").leak();
            (ident, sc.module)
        })
        .collect()
}

/// `xtask publish bundle [--dry-run]`
///
/// Merges the 23 `publish/utils/<dir>` crates into a single
/// `publish/utils/utils/` crate. With `--dry-run`, prints the plan and exits
/// without modifying any files.
pub fn run(dry_run: bool) -> Result<()> {
    let repo_root = locate_repo_root()?;
    let publish_root = repo_root.join("publish");
    let utils_root = publish_root.join("utils");
    println!(
        "Phase 2 — utils bundle {}\n  source: {}\n  target: {}\n",
        if dry_run { "(dry-run)" } else { "" },
        utils_root.display(),
        utils_root.join("utils").display(),
    );

    if !utils_root.is_dir() {
        anyhow::bail!("publish/utils/ missing — run `xtask publish rename` first");
    }

    // Step 0: verify all 23 sub-crate dirs exist.
    let mut missing = Vec::new();
    for sc in SUB_CRATES {
        if !utils_root.join(sc.dir).is_dir() {
            missing.push(sc.dir);
        }
    }
    if !missing.is_empty() {
        anyhow::bail!(
            "missing {} sub-crate dir(s) under publish/utils/: {}",
            missing.len(),
            missing.join(", ")
        );
    }
    println!(
        "Found all {} utils sub-crates under publish/utils/.",
        SUB_CRATES.len()
    );

    if dry_run {
        return print_dry_run_plan(&utils_root);
    }

    // Steps 1-5 (migration + rewrites) land in Stages 2-3.
    anyhow::bail!(
        "bundle migration is not yet implemented (Stage 2+); run with --dry-run for the plan"
    );
}

/// Print the migration plan without touching the filesystem.
fn print_dry_run_plan(utils_root: &Path) -> Result<()> {
    let merged_dir = utils_root.join("utils");
    println!("Plan:");
    println!("  1. Create merged crate at {}", merged_dir.display());
    println!("     package.name = \"{}\"", MERGED_PACKAGE);
    println!("     [lib].name   = \"{}\"", MERGED_LIB_IDENT);
    println!(
        "     src/lib.rs   = {} `pub mod <name>;` lines\n",
        SUB_CRATES.len()
    );

    println!("  2. Migrate each sub-crate's src/ into a submodule:");
    let mut total_files = 0usize;
    for sc in SUB_CRATES {
        let src = utils_root.join(sc.dir).join("src");
        let count = count_rs_files(&src)?;
        total_files += count;
        println!(
            "     {:<20} (pkg {:<32}) → mod {} [{} files]",
            sc.dir, sc.package, sc.module, count
        );
    }
    println!("     total: {} .rs files\n", total_files);

    println!(
        "  3. Delete the {} original sub-crate dirs.\n",
        SUB_CRATES.len()
    );

    println!("  4. Rewrite publish/Cargo.toml:");
    println!(
        "     [workspace.members]: {} utils/* → 1 utils/utils",
        SUB_CRATES.len()
    );
    println!(
        "     [workspace.dependencies]: {} lemurclaw-utils-* → 1 lemurclaw-utils\n",
        SUB_CRATES.len()
    );

    println!("  5. Rewrite downstream crates (all non-utils crates in publish/):");
    println!(
        "     .rs:        use {}_<sub>::X  →  use {}::<sub>::X",
        MERGED_LIB_IDENT, MERGED_LIB_IDENT
    );
    println!(
        "     Cargo.toml: {}-* = {{ ws }}  →  {} = {{ ws }} (dedup)",
        MERGED_PACKAGE, MERGED_PACKAGE
    );
    let table = ident_to_module();
    println!("\n     ident → module mapping ({} entries):", table.len());
    for (ident, module) in &table {
        println!("       {} → ::{}", ident, module);
    }

    // Flag the #[path] attributes that need relative-path fixes post-move.
    println!("\n  Note: #[path = \"..._tests.rs\"] attributes inside sub-crates");
    println!("        (6 known sites) will need relative-path adjustment after move.");
    println!("\nDry-run complete. Re-run without --dry-run to execute (Stage 2+).");
    Ok(())
}

/// Count `.rs` files under a directory (recursive).
fn count_rs_files(dir: &Path) -> Result<usize> {
    if !dir.is_dir() {
        return Ok(0);
    }
    let mut count = 0usize;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            count += count_rs_files(&path)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            count += 1;
        }
    }
    Ok(count)
}
