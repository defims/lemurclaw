//! Phase 1.5: publish the 4 git forks as `lemurclaw-*` crates on crates.io,
//! then rewire the publish workspace to reference them via `package = "..."`
//! aliases instead of `[patch.crates-io]`.
//!
//! The key trick: forks keep their original `[lib].name` (so `use ratatui::`
//! continues to work in both fork-internal and codex source), but their
//! `package.name` changes to `lemurclaw-X`. Each downstream consumer's
//! Cargo.toml gets `ratatui = { version = "0.29.0", package = "lemurclaw-ratatui" }`
//! — the dep key stays `ratatui`, only the resolved package is renamed.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Description of one of the 4 fork crates we publish as `lemurclaw-*`.
struct ForkSpec {
    /// Local directory name under `publish.forks/`.
    dir: &'static str,
    /// Git URL.
    repo: &'static str,
    /// Exact commit to checkout after clone.
    rev: &'static str,
    /// Upstream package name (the original `package.name` in the fork).
    upstream_name: &'static str,
    /// Version to keep (matches upstream version).
    version: &'static str,
    /// The crate name used in `use <lib_name>::...` imports — i.e. the value
    /// Cargo derives for `[lib].name` from the upstream `package.name`
    /// (underscores, not dashes). Forks without an explicit `[lib].name` would
    /// otherwise re-derive from our renamed `package.name`, breaking every
    /// `examples/*.rs` / test that imports the crate by name.
    lib_name: &'static str,
    /// Internal deps that point at sibling forks and need a `package = "..."`
    /// alias. Each entry is (dep_key, lemurclaw_package_name).
    internal_fork_deps: &'static [(&'static str, &'static str)],
}

/// The 4 forks, in publication order (topological: leaves first).
///
/// crossterm and tungstenite are leaves. ratatui depends on crossterm.
/// tokio-tungstenite depends on tungstenite.
const FORKS: &[ForkSpec] = &[
    ForkSpec {
        dir: "crossterm",
        repo: "https://github.com/nornagon/crossterm",
        rev: "87db8bfa6dc99427fd3b071681b07fc31c6ce995",
        upstream_name: "crossterm",
        version: "0.28.1",
        lib_name: "crossterm",
        internal_fork_deps: &[],
    },
    ForkSpec {
        dir: "tungstenite",
        repo: "https://github.com/openai-oss-forks/tungstenite-rs",
        rev: "4fffad30fe373adbdcffab9545e9e9bf4f2fc19f",
        upstream_name: "tungstenite",
        version: "0.27.0",
        lib_name: "tungstenite",
        internal_fork_deps: &[],
    },
    ForkSpec {
        dir: "ratatui",
        repo: "https://github.com/nornagon/ratatui",
        rev: "9b2ad1298408c45918ee9f8241a6f95498cdbed2",
        upstream_name: "ratatui",
        version: "0.29.0",
        lib_name: "ratatui",
        // ratatui's crossterm dep is `crossterm = { version = "0.28.1", optional = true }`.
        // We add `package = "lemurclaw-crossterm"` so it resolves to our fork.
        internal_fork_deps: &[("crossterm", "lemurclaw-crossterm")],
    },
    ForkSpec {
        dir: "tokio-tungstenite",
        repo: "https://github.com/openai-oss-forks/tokio-tungstenite",
        rev: "0e5b2d73aa18dd9f0a50ee9ff199d5aef7594186",
        upstream_name: "tokio-tungstenite",
        version: "0.28.0",
        // Cargo derives the lib name with dashes → underscores.
        lib_name: "tokio_tungstenite",
        // tokio-tungstenite's tungstenite dep is in a sub-table
        // `[dependencies.tungstenite]` with git/rev. We rewrite the whole dep.
        internal_fork_deps: &[("tungstenite", "lemurclaw-tungstenite")],
    },
];

fn forks_root(repo_root: &Path) -> PathBuf {
    repo_root.join("publish.forks")
}

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

/// `xtask publish fork clone`
pub fn run_clone() -> Result<()> {
    let repo_root = locate_repo_root()?;
    let forks_dir = forks_root(&repo_root);
    println!("Phase 1.5 — fork clone\n  output: {}\n", forks_dir.display());
    fs::create_dir_all(&forks_dir).context("create publish.forks/")?;

    for fork in FORKS {
        let dest = forks_dir.join(fork.dir);
        if dest.exists() {
            println!("  ✓ {} already cloned, skipping", fork.dir);
            continue;
        }
        println!("  cloning {} @ {}...", fork.repo, &fork.rev[..8]);
        // Clone shallow, then fetch the specific rev. (`git clone --branch <sha>`
        // doesn't work for SHA on all servers; fetch + checkout is reliable.)
        let status = Command::new("git")
            .args(["clone", "--depth", "1", fork.repo, dest.to_str().unwrap()])
            .status()
            .context("spawn git clone")?;
        if !status.success() {
            anyhow::bail!("git clone failed for {}", fork.repo);
        }
        // Fetch the specific commit (unshallow as needed).
        let fetch_status = Command::new("git")
            .args(["-C", dest.to_str().unwrap(), "fetch", "--depth=1", "origin", fork.rev])
            .status()
            .context("spawn git fetch")?;
        if !fetch_status.success() {
            // Fall back to full fetch if the shallow fetch of an arbitrary sha fails.
            let full = Command::new("git")
                .args(["-C", dest.to_str().unwrap(), "fetch", "--unshallow"])
                .status();
            let _ = full;
        }
        let co = Command::new("git")
            .args(["-C", dest.to_str().unwrap(), "checkout", fork.rev])
            .status()
            .context("spawn git checkout")?;
        if !co.success() {
            anyhow::bail!("git checkout {} failed in {}", fork.rev, fork.dir);
        }
        println!("    ✓ {} @ {}", fork.dir, &fork.rev[..8]);
    }

    println!("\nNext: `xtask publish fork prepare` to rewrite Cargo.tomls.");
    Ok(())
}

/// `xtask publish fork prepare`
pub fn run_prepare() -> Result<()> {
    let repo_root = locate_repo_root()?;
    let forks_dir = forks_root(&repo_root);
    println!("Phase 1.5 — fork prepare\n  source: {}\n", forks_dir.display());

    if !forks_dir.exists() {
        anyhow::bail!(
            "publish.forks/ missing — run `xtask publish fork clone` first"
        );
    }

    for fork in FORKS {
        let fork_dir = forks_dir.join(fork.dir);
        let manifest_path = fork_dir.join("Cargo.toml");
        if !manifest_path.exists() {
            anyhow::bail!("missing {} — run `clone` first", manifest_path.display());
        }
        println!("  preparing {} ...", fork.dir);
        let raw = fs::read_to_string(&manifest_path)
            .with_context(|| format!("read {}", manifest_path.display()))?;
        let rewritten = rewrite_fork_manifest(&raw, fork)
            .with_context(|| format!("rewrite {}", manifest_path.display()))?;
        if rewritten != raw {
            fs::write(&manifest_path, rewritten)
                .with_context(|| format!("write {}", manifest_path.display()))?;
            println!("    ✓ Cargo.toml rewritten");
        } else {
            println!("    (no changes needed — already prepared)");
        }
    }

    println!("\nVerifying fork manifest syntax (cargo metadata)...");
    for fork in FORKS {
        let fork_dir = forks_dir.join(fork.dir);
        print!("  cargo metadata {} ... ", fork.dir);
        std::io::Write::flush(&mut std::io::stdout()).ok();
        let out = Command::new("cargo")
            .args(["metadata", "--no-deps", "--format-version=1"])
            .current_dir(&fork_dir)
            .output()
            .context("spawn cargo metadata")?;
        if out.status.success() {
            println!("✓");
        } else {
            println!("✗");
            let stderr = String::from_utf8_lossy(&out.stderr);
            for line in stderr.lines().filter(|l| l.starts_with("error")) {
                eprintln!("    {}", line);
            }
            anyhow::bail!("cargo metadata failed for {}", fork.dir);
        }
    }
    println!("\nNote: full cargo check is deferred to `publish --dry-run`,");
    println!("which validates packaging + registry resolution.");

    println!("\nNext: `xtask publish fork publish --dry-run` to validate packaging.");
    Ok(())
}

/// Rewrite a fork's Cargo.toml for publication as `lemurclaw-<name>`.
///
/// - package.name: `<upstream>` → `lemurclaw-<upstream>`
/// - Each internal fork dep gets `package = "lemurclaw-<dep>"` alias added.
///   For sub-table form `[dependencies.X]`, we delete `git = ...` / `rev = ...`
///   and replace with `version = "..."` + `package = "lemurclaw-X"`.
/// - `[lib].name` is forced to `fork.lib_name` so the lib keeps its original
///   crate identifier (e.g. `tungstenite`, `tokio_tungstenite`). Without this,
///   forks lacking an explicit `[lib].name` would re-derive it from the renamed
///   `package.name` (`lemurclaw_X`) and break their own `examples/` / tests.
fn rewrite_fork_manifest(raw: &str, fork: &ForkSpec) -> Result<String> {
    let new_name = format!("lemurclaw-{}", fork.upstream_name);

    let mut out = String::with_capacity(raw.len());
    // Lines belonging to a sub-table for one of the internal fork deps. These
    // get rewritten wholesale (delete git/rev, add version + package alias).
    let mut in_fork_dep_subtable: Option<&str> = None;
    let mut wrote_dep_subtable_header = false;
    // Track whether we're inside [package] so we only rename the package.name
    // field (not e.g. [lib].name which must stay `crossterm`).
    let mut current_section: Option<String> = None;
    // Whether we've ensured the [lib].name is present (in the output).
    let mut ensured_lib_name = false;
    // Did the *input* contain a [lib] section? Used to decide between
    // injecting `name =` into an existing [lib] vs. appending a fresh block.
    let mut saw_lib_section = false;
    // First line of an existing [lib] section (header just emitted); we inject
    // `name =` immediately after it.
    let mut at_lib_section_start = false;

    for line in raw.lines() {
        let trimmed = line.trim_start();

        // Detect entering a new section header.
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            // Exit any active sub-table.
            in_fork_dep_subtable = None;
            wrote_dep_subtable_header = false;
            current_section = Some(trimmed.to_string());

            // Entering [lib]: note it and arm the "inject name after header"
            // path. If the input [lib] already has `name =`, the per-line pass
            // below will pick it up and we won't re-inject.
            if trimmed == "[lib]" {
                saw_lib_section = true;
                at_lib_section_start = true;
            }

            // Check if this is a sub-table for one of our internal fork deps.
            // Format: `[dependencies.tungstenite]` or `[dev-dependencies.tungstenite]`.
            if let Some(dep_name) = find_fork_dep_subtable(trimmed) {
                if let Some(&(_, lemurclaw_name)) = fork
                    .internal_fork_deps
                    .iter()
                    .find(|(k, _)| *k == dep_name)
                {
                    in_fork_dep_subtable = Some(dep_name);
                    let version = find_internal_dep_version(fork, dep_name);
                    let indent = &line[..line.len() - trimmed.len()];
                    out.push_str(&format!("{}[dependencies.{}]\n", indent, dep_name));
                    out.push_str(&format!("{}version = \"{}\"\n", indent, version));
                    out.push_str(&format!("{}package = \"{}\"\n", indent, lemurclaw_name));
                    out.push_str(&format!("{}default-features = false\n", indent));
                    wrote_dep_subtable_header = true;
                    continue;
                }
            }

            // Ordinary section header — pass through.
            out.push_str(line);
            out.push('\n');

            // If we just entered [lib], inject `name =` right after the header
            // (only if we haven't already seen/ensured it).
            if at_lib_section_start && !ensured_lib_name {
                let indent = &line[..line.len() - trimmed.len()];
                out.push_str(&format!("{}name = \"{}\"\n", indent, fork.lib_name));
                ensured_lib_name = true;
            }
            at_lib_section_start = false;
            continue;
        }

        if let Some(_dep) = in_fork_dep_subtable {
            // We're inside a fork-dep sub-table whose header we already rewrote.
            // Skip the original key=value lines.
            continue;
        }

        // Rewrite package.name ONLY inside [package] section.
        if current_section.as_deref() == Some("[package]") {
            if let Some(rest) = trimmed.strip_prefix("name = ") {
                let value = rest.trim().trim_matches('"');
                if value == fork.upstream_name {
                    let indent = &line[..line.len() - trimmed.len()];
                    out.push_str(&format!("{}name = \"{}\"\n", indent, new_name));
                    continue;
                }
            }
        }

        // Inside [lib]: drop any existing `name =` line — the authoritative
        // value is the one we injected right after the [lib] header. (Without
        // this we'd emit a duplicate `name =` for forks like crossterm that
        // already pin it upstream.)
        if current_section.as_deref() == Some("[lib]")
            && trimmed.starts_with("name = ")
        {
            continue;
        }

        // Inline-form fork dep: `crossterm = { version = "0.28.1", optional = true }`
        // Add `package = "lemurclaw-crossterm"` to the inline table.
        if let Some(rewritten) = rewrite_inline_fork_dep(trimmed, fork) {
            let indent = &line[..line.len() - trimmed.len()];
            out.push_str(&format!("{}{}\n", indent, rewritten));
            continue;
        }

        let _ = wrote_dep_subtable_header;
        out.push_str(line);
        out.push('\n');
    }

    // No [lib] section at all: append one so the lib name is pinned. This
    // matters for tungstenite / tokio-tungstenite whose manifests omit [lib].
    if !saw_lib_section {
        out.push_str("\n[lib]\n");
        out.push_str(&format!("name = \"{}\"\n", fork.lib_name));
        ensured_lib_name = true;
    }

    debug_assert!(
        ensured_lib_name,
        "rewrite_fork_manifest failed to pin [lib].name for {}",
        fork.dir
    );

    Ok(out)
}

// We avoid regex dep — use a simple byte-substring matcher for sub-table headers.
// `[dependencies.tungstenite]` -> extract `tungstenite`.
fn find_fork_dep_subtable<'a>(s: &'a str) -> Option<&'a str> {
    // Match `[dependencies.<X>]` or `[dev-dependencies.<X>]` or
    // `[build-dependencies.<X>]`. Return <X>.
    for prefix in ["[dependencies.", "[dev-dependencies.", "[build-dependencies."] {
        if let Some(rest) = s.strip_prefix(prefix) {
            if let Some(inner) = rest.strip_suffix(']') {
                return Some(inner);
            }
        }
    }
    None
}

fn find_internal_dep_version(fork: &ForkSpec, dep_name: &str) -> &'static str {
    // crossterm is 0.28.1, tungstenite is 0.27.0. Hard-coded for the 2 known
    // internal deps.
    match dep_name {
        "crossterm" => "0.28.1",
        "tungstenite" => "0.27.0",
        _ => {
            eprintln!(
                "warn: unknown internal dep {} for fork {} — using 0.0.0",
                dep_name, fork.dir
            );
            "0.0.0"
        }
    }
}

/// If `trimmed` is an inline fork-dep declaration like
/// `crossterm = { version = "0.28.1", optional = true }` where the key matches
/// one of fork's internal_fork_deps, return the rewritten line (with
/// `package = "lemurclaw-X"` injected). Otherwise return None.
fn rewrite_inline_fork_dep(trimmed: &str, fork: &ForkSpec) -> Option<String> {
    for (dep_key, lemurclaw_name) in fork.internal_fork_deps {
        // Must start with `<dep_key> = {`.
        let prefix = format!("{} = {{", dep_key);
        let prefix_eq = format!("{} =", dep_key);
        if trimmed.starts_with(&prefix) {
            // Inline table. Inject `package = "..."` if not already present.
            if trimmed.contains("package =") {
                return Some(trimmed.to_string());
            }
            // Insert before the closing `}`.
            let close_idx = trimmed.rfind('}')?;
            let before = &trimmed[..close_idx];
            let mut before = before.trim_end().trim_end_matches(',').to_string();
            if !before.ends_with('{') {
                before.push_str(", ");
            }
            return Some(format!(
                "{}package = \"{}\" }}",
                before, lemurclaw_name
            ));
        }
        let _ = prefix_eq;
    }
    None
}

/// `xtask publish fork publish [--dry-run]`
pub fn run_publish(dry_run: bool) -> Result<()> {
    let repo_root = locate_repo_root()?;
    let forks_dir = forks_root(&repo_root);
    println!(
        "Phase 1.5 — fork publish {}\n  source: {}\n",
        if dry_run { "(dry-run)" } else { "" },
        forks_dir.display()
    );

    if !forks_dir.exists() {
        anyhow::bail!("publish.forks/ missing — run `clone` then `prepare` first");
    }

    for fork in FORKS {
        let fork_dir = forks_dir.join(fork.dir);
        print!(
            "  cargo publish{} {} ... ",
            if dry_run { " --dry-run" } else { "" },
            fork.dir
        );
        std::io::Write::flush(&mut std::io::stdout()).ok();

        let mut args = vec!["publish", "--allow-dirty"];
        if dry_run {
            args.push("--dry-run");
        }
        let out = Command::new("cargo")
            .args(&args)
            .current_dir(&fork_dir)
            .output()
            .context("spawn cargo publish")?;
        if out.status.success() {
            println!("✓");
        } else {
            println!("✗");
            let stderr = String::from_utf8_lossy(&out.stderr);
            for line in stderr.lines().filter(|l| {
                l.starts_with("error") || l.contains("Uploading")
            }) {
                eprintln!("    {}", line);
            }
            if dry_run {
                anyhow::bail!(
                    "cargo publish --dry-run failed for {} — fix manifest before publishing",
                    fork.dir
                );
            } else {
                anyhow::bail!("cargo publish failed for {}", fork.dir);
            }
        }
    }

    println!();
    if dry_run {
        println!("All forks dry-run OK. Re-run without --dry-run to actually publish.");
    } else {
        println!("All forks published. Next: `xtask publish fork rewire`.");
    }
    Ok(())
}

/// `xtask publish fork rewire`
pub fn run_rewire() -> Result<()> {
    let repo_root = locate_repo_root()?;
    let publish_root = repo_root.join("publish");
    let manifest_path = publish_root.join("Cargo.toml");
    println!(
        "Phase 1.5 — fork rewire\n  target: {}\n",
        manifest_path.display()
    );

    if !manifest_path.exists() {
        anyhow::bail!("publish/Cargo.toml missing — run `xtask publish rename` first");
    }

    let raw = fs::read_to_string(&manifest_path)
        .with_context(|| format!("read {}", manifest_path.display()))?;
    let rewritten = crate::manifest::rewrite_publish_workspace_for_forks(&raw)?;
    if rewritten == raw {
        println!("No changes — publish/Cargo.toml is already rewired.");
        return Ok(());
    }
    fs::write(&manifest_path, rewritten)
        .with_context(|| format!("write {}", manifest_path.display()))?;
    println!("Rewrote publish/Cargo.toml:");
    println!("  - removed [patch.crates-io] and [patch.\"<url>\"] sections");
    println!("  - added `package = \"lemurclaw-X\"` aliases to 4 workspace deps");

    println!("\nRunning `cargo check --workspace` in publish/ ...");
    let status = Command::new("cargo")
        .args(["check", "--workspace"])
        .current_dir(&publish_root)
        .status()
        .context("spawn cargo check")?;
    if status.success() {
        println!("\n✓ publish/ workspace compiles with fork aliases.");
    } else {
        println!("\n✗ cargo check failed — inspect errors above.");
    }

    Ok(())
}
