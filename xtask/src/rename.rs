//! Sync the parallel `lemurclaw-rs/` workspace against `codex-rs/`.
//!
//! Reads the source workspace at `codex-rs/` and refreshes `lemurclaw-rs/`
//! with every `codex-*` crate renamed to `lemurclaw-*`. The original tree is
//! left untouched; the lemurclaw-rs workspace is self-contained (a sibling
//! root, not a workspace member of the source).
//!
//! **Sync mode only.** `lemurclaw-rs/` must already exist (it is a persistent,
//! git-tracked workspace). lemurclaw's own 4 crates (`lemurclaw`,
//! `lemurclaw-gui`, `lemurclaw-webui`, `lemurclaw-transport`) live exclusively
//! in `lemurclaw-rs/` — they are never copied from `codex-rs/` and are
//! protected from orphan cleanup. Only the `codex-*` renamed products are
//! refreshed on each sync.
//!
//! Pipeline:
//!   1. Discover all crates via `cargo metadata --no-deps`.
//!   2. Filter out excluded / unpublishable crates.
//!   3. For each codex-* crate: overwrite its destination directory (delete +
//!      copy + rename + brand rewrite).
//!   4. Emit lemurclaw-rs/Cargo.toml (cloned members + injected own-crate
//!      members, renamed deps, preserved profiles, dropped [patch.crates-io]).
//!   5. Orphan directory cleanup: remove destination dirs for crates no longer
//!      in the source (own crates are always protected).

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::manifest::{
    rename_package, rewrite_crate_manifest, rewrite_workspace_manifest, CrateManifest,
    WorkspaceManifest,
};
use crate::source_rewrite;

/// lemurclaw's own crates that live exclusively in `lemurclaw-rs/`. They are
/// workspace members but are NOT copied from `codex-rs/` (Phase 4, Option B:
/// fully relocated out of the upstream workspace). These directory names are
/// injected into the generated `lemurclaw-rs/Cargo.toml` members list and
/// protected from orphan cleanup.
const OWN_CRATE_MEMBERS: &[&str] = &[
    "lemurclaw",
    "lemurclaw-gui",
    "lemurclaw-webui",
    "lemurclaw-transport",
];

/// The subset of own crates that are referenced as workspace path dependencies
/// by other own crates (e.g. `lemurclaw` depends on `lemurclaw-gui`). These
/// entries are injected into the generated `[workspace.dependencies]` table.
/// Each tuple is (dependency key, path, version).
const OWN_CRATE_DEPS: &[(&str, &str, &str)] = &[
    ("lemurclaw-gui", "lemurclaw-gui", "0.0.0"),
    ("lemurclaw-transport", "lemurclaw-transport", "0.0.0"),
    ("lemurclaw-webui", "lemurclaw-webui", "0.0.0"),
];

/// Package names of the own crates, for lockfile keep-set injection. Unlike
/// codex-* renamed products (whose lockfile entries are derived from
/// `cargo metadata`), these are hardcoded because the crates are absent from
/// the source workspace.
const OWN_CRATE_PACKAGE_NAMES: &[&str] = &[
    "lemurclaw",
    "lemurclaw-gui",
    "lemurclaw-webui",
    "lemurclaw-transport",
];

pub fn run() -> Result<()> {
    let repo_root = locate_repo_root()?;
    let codex_root = repo_root.join("codex-rs");
    let publish_root = repo_root.join("lemurclaw-rs");

    // Sync mode only: lemurclaw-rs/ must already exist (persistent git-tracked
    // workspace). Initialization from scratch is no longer supported because
    // the own crates are absent from codex-rs/.
    if !publish_root.join("Cargo.toml").exists() {
        anyhow::bail!(
            "lemurclaw-rs/ does not exist at {}.\n\
             It is a persistent, git-tracked workspace — obtain it via `git clone` \
             or `git checkout`, not via this command.\n\
             Once it exists, re-run to sync codex-* renamed products.",
            publish_root.display()
        );
    }

    println!(
        "Sync — incremental rename\n  source: {}\n  output: {}\n",
        codex_root.display(),
        publish_root.display()
    );

    // 1. Discover all crates via cargo metadata.
    let discovered = discover_crates(&codex_root)?;
    println!(
        "Discovered {} crates in source workspace.",
        discovered.renamed.len() + discovered.excluded_count
    );
    println!(
        "Syncing {} renamed crates (excluding {} bin-only / sample / publish=false).",
        discovered.renamed.len(),
        discovered.excluded_count
    );

    // 2. Copy + rewrite each codex-* renamed product. We drop dep references
    //    to excluded crates that are still reachable via dev-deps.
    let drop_deps = excluded_dep_names();
    let mut copied = 0usize;
    let mut bin_dropped = 0usize;

    for info in &discovered.renamed {
        // Delete the target directory before copying so stale files from a
        // previous run don't linger.
        let dst_dir = publish_root.join(&info.dst_rel_dir);
        if dst_dir.is_dir() {
            fs::remove_dir_all(&dst_dir)
                .with_context(|| format!("remove {}", dst_dir.display()))?;
        }
        let changed = process_crate(&codex_root, &publish_root, info, &drop_deps)?;
        if changed.dropped_bins > 0 {
            bin_dropped += changed.dropped_bins;
        }
        copied += 1;
    }
    println!(
        "Synced {} renamed crates; dropped {} [[bin]] targets.",
        copied, bin_dropped
    );

    // 3. Emit lemurclaw-rs/Cargo.toml. Own-crate members and path deps are
    //    injected because they're absent from the source workspace manifest.
    let workspace_src = WorkspaceManifest::read(&codex_root.join("Cargo.toml"))?;
    let keep_members: Vec<String> = discovered
        .renamed
        .iter()
        .map(|c| c.dst_rel_dir.clone())
        .collect();
    let own_members: Vec<String> = OWN_CRATE_MEMBERS.iter().map(|s| s.to_string()).collect();
    let own_deps: Vec<(String, String, String)> = OWN_CRATE_DEPS
        .iter()
        .map(|(k, p, v)| (k.to_string(), p.to_string(), v.to_string()))
        .collect();
    let new_workspace_toml =
        rewrite_workspace_manifest(&workspace_src, &keep_members, &own_members, &own_deps)?;
    fs::write(publish_root.join("Cargo.toml"), new_workspace_toml)
        .context("write lemurclaw-rs/Cargo.toml")?;
    println!("Wrote lemurclaw-rs/Cargo.toml.");

    // 3b. Copy and rewrite Cargo.lock so the lemurclaw-rs workspace resolves the
    // same external dependency versions as the source. The codex-* → lemurclaw-*
    // rename is mechanical: lockfile only contains package names as string
    // values. Own-crate package names are injected into the keep-set so their
    // lockfile entries survive the rewrite (though they'll typically be
    // regenerated by `cargo check` since the source lockfile no longer has them).
    let src_lock = codex_root.join("Cargo.lock");
    if src_lock.exists() {
        println!("Copying and rewriting Cargo.lock...");
        let lock = fs::read_to_string(&src_lock)
            .with_context(|| format!("read {}", src_lock.display()))?;
        let all_publishable: Vec<&CrateInfo> = discovered.renamed.iter().collect();
        let new_lock = rewrite_lockfile(&lock, &all_publishable);
        fs::write(publish_root.join("Cargo.lock"), new_lock)
            .context("write lemurclaw-rs/Cargo.lock")?;
    }

    // 4. Orphan directory cleanup: remove destination dirs for crates that
    //    were removed from the source workspace. Own crates are always
    //    protected (hardcoded, not derived from discovery).
    let removed = clean_orphan_dirs(&publish_root, &discovered)?;
    if removed > 0 {
        println!("Cleaned up {} orphan crate directories.", removed);
    }

    // 5. Apply the full-scope brand rewrite (Codex→lemurclaw: env vars, paths,
    //    CLI flags, internal protocol identifiers, system prompts, telemetry).
    crate::bundle::rewrite_brand_full(&publish_root)?;

    // 6. Verify with cargo check.
    println!();
    println!("Running `cargo check --workspace` in lemurclaw-rs/ ...");
    let status = Command::new("cargo")
        .args(["check", "--workspace"])
        .current_dir(&publish_root)
        .status()
        .context("spawn cargo check")?;
    if status.success() {
        println!();
        println!("✓ lemurclaw-rs/ workspace compiles. Ready for `cargo publish`.");
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
    /// Destination directory relative to lemurclaw-rs/. Differs from `rel_dir`
    /// only when the source leaf directory is itself `codex-*`-prefixed
    /// (e.g. source `codex-api/` → `dst_rel_dir = "lemurclaw-api"`).
    /// `rewrite_brand_full` later rewrites manifest text `codex-*` →
    /// `lemurclaw-*` (including `[workspace]` path/member strings), so the
    /// destination dir must match the post-brand name or cargo fails to
    /// resolve the member path.
    dst_rel_dir: String,
    /// True if this crate is `lib + bin` — we drop the bin targets.
    drop_bins: bool,
    /// True if this crate is a proc-macro.
    is_proc_macro: bool,
}

/// Compute the publish-side directory name for a source `rel_dir`.
///
/// Delegates to `manifest::dst_member_path` so the path-rewrite rule stays
/// in one place (also used when comparing source `path = "..."` values
/// against the destination-form `keep_members`).
fn dst_rel_dir_for(rel_dir: &str) -> String {
    crate::manifest::dst_member_path(rel_dir)
}

/// Discovered crates from the source workspace. Only `codex-*` renamed
/// products are tracked here — lemurclaw's own crates live exclusively in
/// lemurclaw-rs/ (hardcoded in `OWN_CRATE_MEMBERS`) and are never discovered
/// from the source.
struct DiscoveredCrates {
    renamed: Vec<CrateInfo>,
    excluded_count: usize,
}

fn discover_crates(codex_root: &Path) -> Result<DiscoveredCrates> {
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

    let mut renamed = Vec::new();
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
        let drop_bins = should_drop_bins(&name, &manifest);
        let is_proc_macro = manifest.is_proc_macro();

        renamed.push(CrateInfo {
            dst_rel_dir: dst_rel_dir_for(&rel_dir),
            name,
            rel_dir,
            drop_bins,
            is_proc_macro,
        });
    }
    // Sort for deterministic output.
    renamed.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(DiscoveredCrates {
        renamed,
        excluded_count,
    })
}

fn is_excluded(name: &str, _rel_dir: &str, manifest: &CrateManifest) -> bool {
    // publish = false
    if manifest.is_unpublishable() {
        return true;
    }
    // bin-only sample crates
    let bin_only = matches!(name, "codex-bwrap" | "codex-thread-manager-sample");
    if bin_only {
        return true;
    }
    // Helper / sample crates that aren't part of the publishable closure.
    // (lemurclaw's own crates are no longer in codex-rs/ — they live in
    // lemurclaw-rs/ only, so they never reach this function.)
    let is_helper = matches!(name, "codex-test-binary-support");
    if is_helper {
        return true;
    }
    false
}

fn should_drop_bins(_name: &str, manifest: &CrateManifest) -> bool {
    // Drop bins for lib+bin crates (keep them as libs for publish). We detect
    // this as "has both [lib] and [[bin]] sections OR has src/main.rs alongside
    // a lib.rs". For simplicity, we drop [[bin]] sections unconditionally on
    // any crate that has a [lib] — that's safe because a lib-only crate won't
    // have [[bin]] sections.
    manifest.doc.get("bin").is_some() || manifest.doc.get("lib").is_some()
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
    let dst_dir = publish_root.join(&info.dst_rel_dir);

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
    // Test-support helpers (live under tests/common or tests/support, never
    // published; `cargo publish` strips dev-deps anyway, so dropping the
    // reference keeps the lemurclaw-rs workspace self-consistent without the
    // crate itself).
    vec![
        "core_test_support".to_string(),
        "app_test_support".to_string(),
        "mcp_test_support".to_string(),
        "codex-exec-server-test-support".to_string(),
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
        } else if meta.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
            // Best-effort rewrite; parse failures produce a warning but
            // don't abort the whole lemurclaw-rs generation.
            match source_rewrite::rewrite_file(&path) {
                Ok(true) => {}
                Ok(false) => {}
                Err(e) => {
                    eprintln!("warn: failed to rewrite {}: {}", path.display(), e);
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

/// Remove destination directories that correspond to crates no longer present
/// in the source workspace. lemurclaw own dirs are always protected (hardcoded
/// in `OWN_CRATE_MEMBERS`) — they live exclusively in lemurclaw-rs/ and are
/// never derived from source discovery.
///
/// Algorithm:
///   1. Collect the set of expected `dst_rel_dir` values from the discovered
///      renamed crates.
///   2. Hardcode the protected set from `OWN_CRATE_MEMBERS`.
///   3. Recursively discover all crate directories under `publish_root` (a
///      directory containing Cargo.toml). This handles nested layouts like
///      `ext/items/`, `utils/absolute-path/`, etc.
///   4. Any crate directory that is NOT in the expected set AND NOT in the
///      protected set is an orphan and gets removed.
fn clean_orphan_dirs(publish_root: &Path, discovered: &DiscoveredCrates) -> Result<usize> {
    let expected: HashSet<String> = discovered
        .renamed
        .iter()
        .map(|c| c.dst_rel_dir.clone())
        .collect();
    let protected: HashSet<String> = OWN_CRATE_MEMBERS.iter().map(|s| s.to_string()).collect();

    // Find all crate directories under publish_root.
    let mut existing_crates = Vec::new();
    list_crate_dirs_recursive(publish_root, publish_root, &mut existing_crates)?;

    let mut removed = 0usize;
    for rel_dir in &existing_crates {
        if expected.contains(rel_dir) || protected.contains(rel_dir) {
            continue;
        }
        // Orphan. Remove it.
        let path = publish_root.join(rel_dir);
        println!("  removing orphan crate: {}", rel_dir);
        fs::remove_dir_all(&path).with_context(|| format!("remove orphan {}", path.display()))?;
        removed += 1;
    }
    Ok(removed)
}

/// Recursively find all directories containing `Cargo.toml` under `current`,
/// recording their paths relative to `root`.
fn list_crate_dirs_recursive(root: &Path, current: &Path, out: &mut Vec<String>) -> Result<()> {
    for entry in fs::read_dir(current).context("read directory")? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        // Skip well-known non-crate directories.
        if name_str == "target" || name_str == ".git" || name_str == "node_modules" {
            continue;
        }
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        if path.join("Cargo.toml").exists() {
            // This is a crate directory. Record its relative path.
            let rel = path
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| path.to_string_lossy().into_owned());
            out.push(rel);
        } else {
            // Not a crate dir itself, but may contain nested crates (e.g.
            // `ext/`, `utils/`, `memories/`). Recurse into them.
            list_crate_dirs_recursive(root, &path, out)?;
        }
    }
    Ok(())
}

/// Rewrite a Cargo.lock file from the source workspace to match the
/// lemurclaw-rs workspace. Two transformations:
///   1. `codex-` → `lemurclaw-` and `codex_` → `lemurclaw_` in every string
///      value (package names, dependencies lists).
///   2. Drop package entries for crates that are excluded from the
///      lemurclaw-rs workspace (we don't have a manifest for them anymore, so their lock
///      entries would be stale).
fn rewrite_lockfile(lock: &str, publishable: &[&CrateInfo]) -> String {
    use std::collections::HashSet;

    // Build the set of renamed package names we DO want to keep.
    let mut keep: HashSet<String> = HashSet::new();
    for c in publishable {
        keep.insert(rename_package(&c.name));
    }
    // Inject own-crate package names — they're workspace members of
    // lemurclaw-rs but absent from the source lockfile (Phase 4). Including
    // them here is harmless: if the source lockfile has no matching block,
    // nothing is emitted; if it does (e.g. older state), it's preserved.
    for name in OWN_CRATE_PACKAGE_NAMES {
        keep.insert((*name).to_string());
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
            let renamed = rename_package(&name);
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
    line.replace(&format!("\"{}\"", old), &format!("\"{}\"", new))
}
