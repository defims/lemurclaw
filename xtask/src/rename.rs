//! Phase 1: generate the parallel `publish/` workspace.
//!
//! Reads the source workspace at `codex-rs/` and produces `publish/` with
//! every `codex-*` crate renamed to `lemurclaw-*`. The original tree is left
//! untouched; the publish workspace is self-contained (a sibling root, not a
//! workspace member of the source).
//!
//! Pipeline:
//!   1. Discover all crates via `cargo metadata --no-deps`.
//!   2. Filter out excluded / unpublishable crates.
//!   3. For each crate: copy its directory to publish/, rewrite Cargo.toml,
//!      rewrite all `.rs` source files.
//!   4. Emit publish/Cargo.toml (cloned members, renamed deps, preserved
//!      profiles, dropped [patch.crates-io]).

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::manifest::{
    rewrite_crate_manifest, rewrite_workspace_manifest, CrateManifest, WorkspaceManifest,
};
use crate::source_rewrite;

pub fn run() -> Result<()> {
    let repo_root = locate_repo_root()?;
    let codex_root = repo_root.join("codex-rs");
    let publish_root = repo_root.join("publish");

    println!(
        "Phase 1 — publish rename\n  source: {}\n  output: {}\n",
        codex_root.display(),
        publish_root.display()
    );

    // Clean slate. Move any existing publish/ aside first to avoid races with
    // any cargo process still holding files, then delete in the background.
    if publish_root.exists() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let backup = publish_root.with_extension(format!("old.{}", stamp));
        println!("Moving existing publish/ -> {} ...", backup.display());
        fs::rename(&publish_root, &backup).ok();
        // Spawn a background rm so we don't block on stuck file handles.
        let _ = std::thread::spawn(move || {
            std::fs::remove_dir_all(&backup).ok();
        });
    }
    fs::create_dir_all(&publish_root).context("create publish/")?;

    // 1. Discover all crates via cargo metadata.
    let (publishable, excluded_count) = discover_crates(&codex_root)?;
    println!("Discovered {} crates in source workspace.", publishable.len() + excluded_count);
    println!(
        "Publishing {} crates (excluding {} bin-only / sample / publish=false).",
        publishable.len(),
        excluded_count
    );

    // 3. Copy + rewrite each crate. We drop dep references to excluded crates
    //    that are still reachable via dev-deps (test_support helpers).
    let drop_deps = excluded_dep_names();
    let mut copied = 0usize;
    let mut bin_dropped = 0usize;
    for info in &publishable {
        let changed = process_crate(&codex_root, &publish_root, info, &drop_deps)?;
        if changed.dropped_bins > 0 {
            bin_dropped += changed.dropped_bins;
        }
        copied += 1;
    }
    println!(
        "Copied and rewrote {} crates; dropped {} [[bin]] targets.",
        copied, bin_dropped
    );

    // 4. Emit publish/Cargo.toml.
    let workspace_src = WorkspaceManifest::read(&codex_root.join("Cargo.toml"))?;
    let keep_members: Vec<String> = publishable.iter().map(|c| c.rel_dir.clone()).collect();
    let new_workspace_toml = rewrite_workspace_manifest(&workspace_src, &keep_members)?;
    fs::write(publish_root.join("Cargo.toml"), new_workspace_toml)
        .context("write publish/Cargo.toml")?;
    println!("Wrote publish/Cargo.toml.");

    // 4b. Copy and rewrite Cargo.lock so the publish workspace resolves the
    // same external dependency versions as the source. Without this, cargo
    // picks latest-compatible versions which can diverge (we observed
    // rama-core 0.3.0-alpha.4 vs the version the source lockfile pins).
    // The codex-* → lemurclaw-* rename is mechanical: lockfile only contains
    // package names as string values.
    let src_lock = codex_root.join("Cargo.lock");
    if src_lock.exists() {
        println!("Copying and rewriting Cargo.lock...");
        let lock = fs::read_to_string(&src_lock)
            .with_context(|| format!("read {}", src_lock.display()))?;
        let new_lock = rewrite_lockfile(&lock, &publishable);
        fs::write(publish_root.join("Cargo.lock"), new_lock)
            .context("write publish/Cargo.lock")?;
    }

    // 5. Verify with cargo check.
    println!();
    println!("Running `cargo check --workspace` in publish/ ...");
    let status = Command::new("cargo")
        .args(["check", "--workspace"])
        .current_dir(&publish_root)
        .status()
        .context("spawn cargo check")?;
    if status.success() {
        println!();
        println!("✓ publish/ workspace compiles. Ready for `cargo publish`.");
    } else {
        println!();
        println!("✗ cargo check reported errors. Inspect the diagnostics above;");
        println!("  any remaining `codex_*` references need to be added to the");
        println!("  rewriter in source_rewrite.rs.");
    }

    Ok(())
}

struct CrateInfo {
    /// Package name, e.g. `codex-core`.
    name: String,
    /// Directory relative to codex-rs/, e.g. `core`, `utils/absolute-path`.
    rel_dir: String,
    /// True if this crate is `lib + bin` — we drop the bin targets.
    drop_bins: bool,
    /// True if this crate is a proc-macro.
    is_proc_macro: bool,
}

fn discover_crates(codex_root: &Path) -> Result<(Vec<CrateInfo>, usize)> {
    // Use cargo metadata to enumerate crates. Parse JSON manually for the few
    // fields we need.
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--format-version=1",
            "--no-deps",
            "--manifest-path",
            codex_root.join("Cargo.toml").to_str().unwrap(),
        ])
        .output()
        .context("run cargo metadata")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("cargo metadata failed: {}", stderr);
    }
    let json = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value = serde_json::from_str(&json).context("parse metadata json")?;

    let packages = value
        .get("packages")
        .and_then(|v| v.as_array())
        .context("metadata.packages missing")?;

    let mut publishable = Vec::new();
    let mut excluded_count = 0usize;
    for pkg in packages {
        let name = pkg
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let manifest_path = pkg
            .get("manifest_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if manifest_path.is_empty() || name.is_empty() {
            continue;
        }
        // rel_dir relative to codex_root.
        let manifest_path = PathBuf::from(&manifest_path);
        let crate_dir = manifest_path
            .parent()
            .with_context(|| format!("manifest_path has no parent: {}", manifest_path.display()))?;
        let rel_dir = crate_dir
            .strip_prefix(codex_root)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| crate_dir.to_string_lossy().into_owned());

        let manifest = CrateManifest::read(&manifest_path)?;

        // Skip non-codex internal helpers (e.g. test_support crates that live
        // under tests/common/). These aren't meant for publication.
        let is_test_support = rel_dir.starts_with("tests/") || rel_dir.contains("/tests/");

        // Excluded crates: bin-only samples, publish=false, non-codex helpers.
        let exclude = is_excluded(&name, &rel_dir, &manifest) || is_test_support;
        if exclude {
            excluded_count += 1;
            continue;
        }
        let drop_bins = should_drop_bins(&manifest);
        let is_proc_macro = manifest.is_proc_macro();

        publishable.push(CrateInfo {
            name,
            rel_dir,
            drop_bins,
            is_proc_macro,
        });
    }
    // Sort for deterministic output.
    publishable.sort_by(|a, b| a.name.cmp(&b.name));
    Ok((publishable, excluded_count))
}

fn is_excluded(name: &str, rel_dir: &str, manifest: &CrateManifest) -> bool {
    // publish = false
    if manifest.is_unpublishable() {
        return true;
    }
    // bin-only sample crates
    let bin_only = matches!(
        name,
        "codex-bwrap" | "codex-thread-manager-sample"
    );
    if bin_only {
        return true;
    }
    // Lemurclaw's own crates have their own publish path (they're already
    // prefixed lemurclaw- and live in codex-rs/ only for cargo convenience).
    // Skip them here — they publish separately.
    if name.starts_with("lemurclaw-") || rel_dir.starts_with("lemurclaw") {
        return true;
    }
    // Helper / sample crates that aren't part of the publishable closure.
    let is_helper = matches!(name, "codex-test-binary-support");
    if is_helper {
        return true;
    }
    false
}

fn should_drop_bins(manifest: &CrateManifest) -> bool {
    // Drop bins for lib+bin crates (keep them as libs for publish). We detect
    // this as "has both [lib] and [[bin]] sections OR has src/main.rs alongside
    // a lib.rs". For simplicity, we drop [[bin]] sections unconditionally on
    // any crate that has a [lib] — that's safe because a lib-only crate won't
    // have [[bin]] sections.
    manifest.doc.get("bin").is_some()
        || manifest.doc.get("lib").is_some()
}

struct ProcessOutcome {
    dropped_bins: usize,
}

fn process_crate(
    codex_root: &Path,
    publish_root: &Path,
    info: &CrateInfo,
    drop_deps: &[String],
) -> Result<ProcessOutcome> {
    let src_dir = codex_root.join(&info.rel_dir);
    let dst_dir = publish_root.join(&info.rel_dir);

    // Copy the whole crate dir, excluding target/ if present.
    copy_crate_dir(&src_dir, &dst_dir)
        .with_context(|| format!("copy {} -> {}", src_dir.display(), dst_dir.display()))?;

    // Rewrite Cargo.toml.
    let manifest_path = dst_dir.join("Cargo.toml");
    let original = fs::read_to_string(&manifest_path)
        .with_context(|| format!("read {}", manifest_path.display()))?;
    // Count [[bin]] sections before dropping.
    let bin_count = original
        .lines()
        .filter(|l| l.trim_start().starts_with("[[bin]]"))
        .count();
    let new_manifest = rewrite_crate_manifest(&original, info.drop_bins, drop_deps)
        .with_context(|| format!("rewrite {}", manifest_path.display()))?;
    fs::write(&manifest_path, new_manifest)
        .with_context(|| format!("write {}", manifest_path.display()))?;

    // Rewrite every .rs file under dst_dir (including tests/, benches/, etc.).
    rewrite_rust_files(&dst_dir)
        .with_context(|| format!("rewrite .rs in {}", dst_dir.display()))?;

    // If we dropped bins, remove src/bin/ if present.
    if info.drop_bins {
        let bin_dir = dst_dir.join("src").join("bin");
        if bin_dir.is_dir() {
            fs::remove_dir_all(&bin_dir).ok();
        }
        // Also drop src/main.rs if present (lib+bin auto-detected main.rs).
        let main_rs = dst_dir.join("src").join("main.rs");
        if main_rs.is_file() {
            fs::remove_file(&main_rs).ok();
        }
    }

    Ok(ProcessOutcome {
        dropped_bins: if info.drop_bins { bin_count } else { 0 },
    })
}

/// Names of excluded crates that other publishable crates may reference via
/// dev-dependencies. References to these are elided from rewritten manifests.
fn excluded_dep_names() -> Vec<String> {
    // Test-support helpers (live under tests/common, never published).
    vec![
        "core_test_support".to_string(),
        "app_test_support".to_string(),
        "mcp_test_support".to_string(),
        "codex-test-binary-support".to_string(),
    ]
}

fn copy_crate_dir(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        // Skip target/, node_modules/, dist/ build artifacts.
        if name_str == "target" || name_str == "node_modules" || name_str == "dist" {
            continue;
        }
        let src_path = entry.path();
        let dst_path = dst.join(&name);
        let meta = entry.metadata()?;
        if meta.is_dir() {
            copy_crate_dir(&src_path, &dst_path)?;
        } else if meta.is_file() {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn rewrite_rust_files(dir: &Path) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let meta = entry.metadata()?;
        if meta.is_dir() {
            rewrite_rust_files(&path)?;
        } else if meta.is_file() {
            if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                // Best-effort rewrite; parse failures produce a warning but
                // don't abort the whole publish.
                match source_rewrite::rewrite_file(&path) {
                    Ok(true) => {}
                    Ok(false) => {}
                    Err(e) => {
                        eprintln!(
                            "warn: failed to rewrite {}: {}",
                            path.display(),
                            e
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

fn locate_repo_root() -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("get cwd")?;
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // manifest_dir is <repo>/xtask.
    if manifest_dir.join("..").join("codex-rs").exists() {
        return Ok(manifest_dir.join("..").canonicalize()?);
    }
    // Fall back to searching cwd and ancestors.
    let mut candidate = cwd.clone();
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

/// Rewrite a Cargo.lock file from the source workspace to match the publish
/// workspace. Two transformations:
///   1. `codex-` → `lemurclaw-` and `codex_` → `lemurclaw_` in every string
///      value (package names, dependencies lists).
///   2. Drop package entries for crates that are excluded from the publish
///      workspace (we don't have a manifest for them anymore, so their lock
///      entries would be stale).
fn rewrite_lockfile(lock: &str, publishable: &[CrateInfo]) -> String {
    use std::collections::HashSet;

    // Build the set of renamed package names we DO want to keep.
    let mut keep: HashSet<String> = HashSet::new();
    for c in publishable {
        keep.insert(crate::manifest::rename_package(&c.name));
    }

    let mut out = String::with_capacity(lock.len());
    let mut skip_block = false;
    let mut pending_header = false;
    let mut current_name: Option<String> = None;

    for line in lock.lines() {
        let trimmed = line.trim_start();

        // Track [[package]] blocks: a block starts with `[[package]]` header.
        // When we see a new header, decide skip vs keep based on the NEXT
        // `name = ` line.
        if trimmed.starts_with("[[package]]") {
            // If we were skipping, suppress the header too (drop entirely).
            // The skip decision happens on the next `name = ` line, so we
            // need to buffer this header and emit it only if we don't skip.
            // Simplest: defer header emission to the name line.
            current_name = None;
            pending_header = true;
            continue;
        }

        if pending_header && !skip_block {
            // We hit the name line of a not-yet-decided block. Decide now.
            // (handled below by emitting the buffered header first)
        }

        // Capture package name to decide whether to keep this block.
        if let Some(rest) = trimmed.strip_prefix("name = ") {
            let name = rest.trim().trim_matches('"').to_string();
            let renamed = crate::manifest::rename_package(&name);
            current_name = Some(renamed.clone());
            // Decide skip: codex-* packages we're NOT publishing.
            if renamed.starts_with("lemurclaw-") && !keep.contains(&renamed) {
                skip_block = true;
                pending_header = false;
                continue;
            }
            // Otherwise, this block is kept. Emit the buffered header.
            skip_block = false;
            if pending_header {
                out.push_str("[[package]]\n");
                pending_header = false;
            }
            out.push_str(&line_renamed_package_name(line, &name, &renamed));
            out.push('\n');
            continue;
        }

        if skip_block {
            continue;
        }

        // Non-package sections (version 1, metadata, etc.) — emit any pending
        // header then the line itself.
        if pending_header {
            // We're outside a [[package]] block (e.g. top-level metadata).
            // Don't emit the header here; it would only apply to packages.
            pending_header = false;
        }

        // Rewrite codex-*/codex_* in any string on this line.
        let rewritten = line
            .replace("codex-", "lemurclaw-")
            .replace("codex_", "lemurclaw_");
        out.push_str(&rewritten);
        out.push('\n');
    }

    let _ = current_name;
    out
}

fn line_renamed_package_name(line: &str, old: &str, new: &str) -> String {
    line.replace(
        &format!("\"{}\"", old),
        &format!("\"{}\"", new),
    )
}
