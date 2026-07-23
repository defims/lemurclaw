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

/// One sub-crate being folded into a merged crate. Fields are `&'static str`
/// because the static utils cluster uses compile-time constants; the dynamic
/// core cluster will construct owned equivalents.
#[derive(Clone)]
struct SubCrate {
    /// Directory name under publish/ (from the source layout).
    dir: &'static str,
    /// Package name assigned by Phase 1 rename.
    package: &'static str,
    /// Module name inside the merged crate (`pub mod <module>;`).
    module: &'static str,
}

/// The 23 sub-crates, in the order they appear in the source `Cargo.toml`
/// workspace member list. All 23 contribute to the ident→module mapping used
/// for downstream `.rs` rewrites (their `lemurclaw_utils_X` idents get
/// repointed to `lemurclaw_utils::<module>`).
///
/// Only the 17 in `MERGE_DIRS` are actually folded into the merged crate. The
/// other 6 (`approval-presets`, `cli`, `oss`, `output-truncation`, `plugins`,
/// `sandbox-summary`) depend on heavier `codex-*` workspace crates
/// (core/protocol/exec-server/...) that themselves depend on utils — merging
/// them would create a cyclic dependency, so they stay as standalone crates.
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

/// A merge plan: which crates fold into a single mega-crate, and the metadata
/// for the resulting crate. Parameterizes the bundle logic so the same code
/// path serves both the `utils` cluster and the `core` cluster.
pub struct Cluster {
    /// Subdirectory under `publish/` where the source crates live
    /// (e.g. `"utils"`). Empty string means crates live at the publish/ root.
    pub source_subdir: &'static str,
    /// Workspace member path for the merged crate relative to publish/
    /// (e.g. `"utils/utils"` or `"core"`).
    pub merged_member_path: &'static str,
    /// Package name of the merged crate (e.g. `"lemurclaw-utils"`).
    pub merged_package: &'static str,
    /// Lib identifier of the merged crate (e.g. `"lemurclaw_utils"`).
    pub merged_lib_ident: &'static str,
    /// The sub-crates to fold in.
    pub members: Vec<SubCrate>,
}

impl Cluster {
    /// The lib identifier prefix that downstream crates use to reference the
    /// sub-crates (e.g. `lemurclaw_utils_<sub>`). Used by the `.rs` rewriter.
    fn sub_crate_ident_prefix(&self) -> String {
        format!("{}_", self.merged_lib_ident)
    }

    /// Whether a dep package key (e.g. `lemurclaw-utils-absolute-path`) names
    /// one of THIS cluster's members.
    fn contains_package(&self, key: &str) -> bool {
        self.members.iter().any(|sc| sc.package == key)
    }

    /// Build the ident→module lookup table for this cluster's members.
    fn ident_to_module(&self) -> BTreeMap<String, String> {
        self.members
            .iter()
            .map(|sc| {
                let ident = sc.package.replace('-', "_");
                (ident, sc.module.to_string())
            })
            .collect()
    }

    /// The set of module names for this cluster (used by self-ref fix).
    fn module_names(&self) -> std::collections::HashSet<String> {
        self.members
            .iter()
            .map(|sc| sc.module.to_string())
            .collect()
    }
}

/// Construct the `utils` cluster: 23 known utils crates, of which 17 are
/// merged (the 6 cycle-causing ones stay standalone — see MERGE_DIRS).
pub fn utils_cluster() -> Cluster {
    Cluster {
        source_subdir: "utils",
        merged_member_path: "utils/utils",
        merged_package: MERGED_PACKAGE,
        merged_lib_ident: MERGED_LIB_IDENT,
        members: merge_crates()
            .into_iter()
            .map(|sc| SubCrate {
                dir: sc.dir,
                package: sc.package,
                module: sc.module,
            })
            .collect(),
    }
}

/// Construct the `core` cluster: codex-core's 84-crate transitive closure,
/// dynamically computed via `cargo metadata`. Implemented in Stage 2.
pub fn core_cluster() -> Result<Cluster> {
    anyhow::bail!("core cluster not yet implemented (Stage 2); use `--target utils` for now")
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

/// Directories of sub-crates that are actually folded into the merged crate.
/// Excludes the 6 cycle-causing crates (those depending on heavier `codex-*`
/// crates that transitively depend on utils). Returns only the `SubCrate`
/// entries whose `dir` is in the merge set.
fn merge_crates() -> Vec<&'static SubCrate> {
    /// The 17 dirs that get merged (all except the 6 cycle-causing ones).
    const MERGE_DIRS: &[&str] = &[
        "absolute-path",
        "cache",
        "cargo-bin",
        "elapsed",
        "fuzzy-match",
        "home-dir",
        "image",
        "json-to-toml",
        "path-uri",
        "path-utils",
        "pty",
        "readiness",
        "rustls-provider",
        "sleep-inhibitor",
        "stream-parser",
        "string",
        "template",
    ];
    SUB_CRATES
        .iter()
        .filter(|sc| MERGE_DIRS.contains(&sc.dir))
        .collect()
}

/// Directories of sub-crates that stay standalone (not merged) because merging
/// them would create a cyclic dependency. They remain as separate
/// `lemurclaw-utils-*` crates but still get their downstream refs rewritten.
fn standalone_crates() -> Vec<&'static SubCrate> {
    SUB_CRATES
        .iter()
        .filter(|sc| !merge_crates().iter().any(|m| m.dir == sc.dir))
        .collect()
}

/// Build the lookup table mapping a former crate identifier
/// (`lemurclaw_utils_absolute_path`) to its submodule name (`absolute_path`).
/// Used by the `.rs` rewrite pass. Covers all 23 (merged + standalone), since
/// standalone crates' refs to merged crates also need repointing.
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

/// `xtask publish bundle [--target <utils|core>] [--dry-run]`
///
/// Merges a cluster of `publish/` crates into a single mega-crate. With
/// `--dry-run`, prints the plan and exits without modifying any files.
pub fn run(cluster: &Cluster, dry_run: bool) -> Result<()> {
    let repo_root = locate_repo_root()?;
    let publish_root = repo_root.join("publish");
    let source_root = if cluster.source_subdir.is_empty() {
        publish_root.clone()
    } else {
        publish_root.join(cluster.source_subdir)
    };
    // The merged crate lives one level inside source_root, named after the
    // last segment of merged_member_path (e.g. "utils" or "core").
    let merged_leaf = cluster
        .merged_member_path
        .rsplit('/')
        .next()
        .unwrap_or("merged");
    let merged_dir = source_root.join(merged_leaf);
    println!(
        "bundle {} {}\n  source: {}\n  target: {}\n",
        cluster.merged_package,
        if dry_run { "(dry-run)" } else { "" },
        source_root.display(),
        merged_dir.display(),
    );

    if !source_root.is_dir() {
        anyhow::bail!(
            "{} missing — run `xtask publish rename` first",
            source_root.display()
        );
    }

    // Step 0: verify all member sub-crate dirs exist.
    let mut missing = Vec::new();
    for sc in &cluster.members {
        if !source_root.join(sc.dir).is_dir() {
            missing.push(sc.dir);
        }
    }
    if !missing.is_empty() {
        anyhow::bail!(
            "missing {} member dir(s) under {}: {}",
            missing.len(),
            source_root.display(),
            missing.join(", ")
        );
    }
    println!(
        "Found all {} members under {}.",
        cluster.members.len(),
        source_root.display()
    );

    if dry_run {
        return print_dry_run_plan(cluster, &source_root);
    }

    if merged_dir.exists() {
        anyhow::bail!(
            "{} already exists — remove it before re-running (or it was left from a partial run)",
            merged_dir.display()
        );
    }

    // Step 1: collect each member's dependencies, then create the merged crate
    // dir with a synthesized Cargo.toml and lib.rs.
    println!(
        "Merging {} members into {}.",
        cluster.members.len(),
        cluster.merged_package
    );
    let agg = collect_aggregate_deps(&source_root, &cluster.members, cluster)?;
    create_merged_crate(&merged_dir, &agg, cluster)?;

    // Step 2: move each member's src/ into the merged crate.
    for sc in &cluster.members {
        migrate_subcrate_src(&source_root.join(sc.dir), &merged_dir, sc.module)?;
    }

    // Step 3: delete the merged member dirs (standalone ones stay).
    for sc in &cluster.members {
        let dir = source_root.join(sc.dir);
        fs::remove_dir_all(&dir).with_context(|| format!("delete {}", dir.display()))?;
    }
    println!("Deleted {} member dirs.", cluster.members.len());

    // Step 4: rewrite publish/Cargo.toml (members + workspace deps).
    rewrite_publish_manifest(&publish_root, cluster)?;

    // Step 5: rewrite .rs imports + downstream Cargo.toml dep lines.
    //   - Inside the merged crate:  <lib>_<sub>::  →  crate::<sub>::
    //     (cross-module refs), then fix self-refs: a sub-module's own
    //     `crate::Foo` becomes `crate::<module>::Foo`.
    //   - In downstream crates: <lib>_<sub>:: → <lib>::<sub>::
    rewrite_rust_files_in(&merged_dir.join("src"), RewriteScope::IntraCrate, cluster)?;
    for sc in &cluster.members {
        fix_self_refs_in_submodule(&merged_dir.join("src").join(sc.module), sc.module, cluster)?;
    }
    rewrite_downstream(&publish_root, &merged_dir, cluster)?;

    println!("\n✓ bundle {} complete.", cluster.merged_package);
    println!(
        "Verify: cd publish && cargo check -p {}",
        cluster.merged_package
    );
    Ok(())
}

/// Print the migration plan without touching the filesystem.
fn print_dry_run_plan(cluster: &Cluster, source_root: &Path) -> Result<()> {
    let merged_leaf = cluster
        .merged_member_path
        .rsplit('/')
        .next()
        .unwrap_or("merged");
    let merged_dir = source_root.join(merged_leaf);
    println!("Plan:");
    println!(
        "  Merging {} members into {}:",
        cluster.members.len(),
        cluster.merged_package
    );
    println!();
    println!("  1. Create merged crate at {}", merged_dir.display());
    println!("     package.name = \"{}\"", cluster.merged_package);
    println!("     [lib].name   = \"{}\"", cluster.merged_lib_ident);
    println!(
        "     src/lib.rs   = {} `pub mod <name>;` lines\n",
        cluster.members.len()
    );

    println!("  2. Migrate each member's src/ into a submodule:");
    let mut total_files = 0usize;
    for sc in &cluster.members {
        let src = source_root.join(sc.dir).join("src");
        let count = count_rs_files(&src)?;
        total_files += count;
        println!(
            "     {:<24} (pkg {:<36}) → mod {} [{} files]",
            sc.dir, sc.package, sc.module, count
        );
    }
    println!("     total: {} .rs files\n", total_files);

    println!("  3. Delete the {} member dirs.\n", cluster.members.len());

    println!("  4. Rewrite publish/Cargo.toml:");
    println!(
        "     [workspace.members]: {} merged → 1 {}",
        cluster.members.len(),
        cluster.merged_member_path
    );
    println!(
        "     [workspace.dependencies]: {} merged → 1 {}\n",
        cluster.members.len(),
        cluster.merged_package
    );

    println!("  5. Rewrite downstream crates:");
    println!(
        "     .rs:        use {}_<sub>::X  →  use {}::<sub>::X",
        cluster.merged_lib_ident, cluster.merged_lib_ident
    );
    println!(
        "     Cargo.toml: {}-* = {{ ws }}  →  {} = {{ ws }} (dedup)",
        cluster.merged_package, cluster.merged_package
    );
    let table = cluster.ident_to_module();
    println!("\n     ident → module mapping ({} entries):", table.len());
    for (ident, module) in &table {
        println!("       {} → ::{}", ident, module);
    }

    // Note: #[path] attributes are preserved — moving the whole src/ dir keeps
    // sibling-relative paths intact.
    println!("\n  Note: internal #[path = \"..._tests.rs\"] attributes are preserved");
    println!("        (the whole src/ dir moves, so sibling-relative paths survive).");
    println!("\nDry-run complete. Re-run without --dry-run to execute.");
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

// ---------------------------------------------------------------------------
// Stage 2: dependency aggregation + crate creation + source migration.
// ---------------------------------------------------------------------------

/// Aggregated dependency tables collected across all 23 sub-crates, ready to
/// render into the merged crate's `Cargo.toml`.
struct AggregateDeps {
    /// `[dependencies]` — dep key → value (merged, features unified).
    deps: BTreeMap<String, toml::Value>,
    /// `[dev-dependencies]`.
    dev_deps: BTreeMap<String, toml::Value>,
    /// `[target.'cfg(...)'.dependencies]` — cfg expr → (dep key → value).
    target_deps: BTreeMap<String, BTreeMap<String, toml::Value>>,
    /// `[target.'cfg(...)'.dev-dependencies]` — cfg expr → (dep key → value).
    target_dev_deps: BTreeMap<String, BTreeMap<String, toml::Value>>,
}

/// Read every *merged* sub-crate's Cargo.toml and merge their dependency
/// tables. Deps pointing at sibling `lemurclaw-utils-*` workspace crates are
/// dropped — they become intra-crate references inside the merged crate (a
/// `pub mod` is in the same crate, so no dep declaration is needed). Other
/// `lemurclaw-*` workspace crates (protocol, core, etc.) are preserved.
fn collect_aggregate_deps(
    source_root: &Path,
    members: &[SubCrate],
    cluster: &Cluster,
) -> Result<AggregateDeps> {
    let mut agg = AggregateDeps {
        deps: BTreeMap::new(),
        dev_deps: BTreeMap::new(),
        target_deps: BTreeMap::new(),
        target_dev_deps: BTreeMap::new(),
    };
    for sc in members {
        let path = source_root.join(sc.dir).join("Cargo.toml");
        let raw = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let doc: toml::Value =
            toml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
        let table = doc.as_table().context("manifest is not a table")?;

        if let Some(deps) = table.get("dependencies").and_then(|v| v.as_table()) {
            merge_dep_table(&mut agg.deps, deps, cluster);
        }
        if let Some(deps) = table.get("dev-dependencies").and_then(|v| v.as_table()) {
            merge_dep_table(&mut agg.dev_deps, deps, cluster);
        }
        // Target-scoped deps: [target.'cfg(windows)'.dependencies] etc.
        if let Some(targets) = table.get("target").and_then(|v| v.as_table()) {
            for (cfg, scope) in targets {
                let scope = match scope.as_table() {
                    Some(t) => t,
                    None => continue,
                };
                if let Some(d) = scope.get("dependencies").and_then(|v| v.as_table()) {
                    let entry = agg.target_deps.entry(cfg.clone()).or_default();
                    merge_dep_table(entry, d, cluster);
                }
                if let Some(d) = scope.get("dev-dependencies").and_then(|v| v.as_table()) {
                    let entry = agg.target_dev_deps.entry(cfg.clone()).or_default();
                    merge_dep_table(entry, d, cluster);
                }
            }
        }
    }
    Ok(agg)
}

/// Merge a source dep table into the accumulator. Internal workspace crates
/// (`codex-*` / `lemurclaw-*`) are skipped — they become intra-crate refs.
/// When both old and new specify a dep, features arrays are unioned.
fn merge_dep_table(
    accum: &mut BTreeMap<String, toml::Value>,
    src: &toml::map::Map<String, toml::Value>,
    cluster: &Cluster,
) {
    for (key, val) in src {
        // Skip this cluster's member packages — they become intra-crate refs
        // (a `pub mod` is in the same crate). Other workspace crates stay.
        if cluster.contains_package(key) {
            continue;
        }
        match accum.get(key) {
            None => {
                accum.insert(key.clone(), val.clone());
            }
            Some(existing) => {
                // Both present — unify features if either is a table.
                let merged = unify_dep_value(existing, val);
                accum.insert(key.clone(), merged);
            }
        }
    }
}

/// Unify two dep values for the same key. If both are inline tables, merge
/// their `features` arrays (deduped, preserving order). Otherwise the newer
/// value wins.
fn unify_dep_value(a: &toml::Value, b: &toml::Value) -> toml::Value {
    let (ta, tb) = match (a.as_table(), b.as_table()) {
        (Some(ta), Some(tb)) => (ta, tb),
        _ => return b.clone(),
    };
    let mut out = tb.clone();
    // Union features.
    let fa = ta.get("features").and_then(|v| v.as_array());
    let fb = tb.get("features").and_then(|v| v.as_array());
    if fa.is_some() || fb.is_some() {
        let mut seen: Vec<toml::Value> = Vec::new();
        for f in fa.into_iter().flatten().chain(fb.into_iter().flatten()) {
            if !seen.iter().any(|s| s == f) {
                seen.push(f.clone());
            }
        }
        if !seen.is_empty() {
            out.insert("features".to_string(), toml::Value::Array(seen));
        }
    }
    toml::Value::Table(out)
}

/// Create the merged crate directory with a synthesized Cargo.toml and lib.rs.
fn create_merged_crate(merged_dir: &Path, agg: &AggregateDeps, cluster: &Cluster) -> Result<()> {
    fs::create_dir_all(merged_dir.join("src")).context("create merged crate src/")?;

    // Cargo.toml
    let mut toml = String::new();
    toml.push_str("[package]\n");
    toml.push_str(&format!("name = \"{}\"\n", cluster.merged_package));
    toml.push_str("version.workspace = true\n");
    toml.push_str("edition.workspace = true\n");
    toml.push_str("license.workspace = true\n");
    toml.push_str("\n[lints]\nworkspace = true\n");

    render_dep_section(&mut toml, "dependencies", &agg.deps);
    render_dep_section(&mut toml, "dev-dependencies", &agg.dev_deps);
    for (cfg, deps) in &agg.target_deps {
        render_target_dep_section(&mut toml, cfg, "dependencies", deps);
    }
    for (cfg, deps) in &agg.target_dev_deps {
        render_target_dep_section(&mut toml, cfg, "dev-dependencies", deps);
    }

    toml.push_str("\n[lib]\n");
    toml.push_str(&format!("name = \"{}\"\n", cluster.merged_lib_ident));
    toml.push_str("doctest = false\n");

    fs::write(merged_dir.join("Cargo.toml"), &toml).context("write merged Cargo.toml")?;

    // lib.rs: one `pub mod` per member.
    let mut lib = format!(
        "//! Merged `{}` crate — union of former sub-crates, each as `pub mod`.\n",
        cluster.merged_package
    );
    for sc in &cluster.members {
        lib.push_str(&format!("pub mod {};\n", sc.module));
    }
    fs::write(merged_dir.join("src").join("lib.rs"), &lib).context("write merged lib.rs")?;

    println!(
        "Created merged crate {} ({} submodules).",
        cluster.merged_package,
        cluster.members.len()
    );
    Ok(())
}

/// Render a `[<section>]` dep table into the TOML buffer.
fn render_dep_section(out: &mut String, section: &str, deps: &BTreeMap<String, toml::Value>) {
    if deps.is_empty() {
        return;
    }
    out.push_str(&format!("\n[{}]\n", section));
    for (key, val) in deps {
        out.push_str(&format!("{} = {}\n", key, render_toml_value(val)));
    }
}

/// Render a `[target.'cfg(...)'.<section>]` dep table.
fn render_target_dep_section(
    out: &mut String,
    cfg: &str,
    section: &str,
    deps: &BTreeMap<String, toml::Value>,
) {
    if deps.is_empty() {
        return;
    }
    out.push_str(&format!("[target.'{}'.{}]\n", cfg, section));
    for (key, val) in deps {
        out.push_str(&format!("{} = {}\n", key, render_toml_value(val)));
    }
    out.push('\n');
}

/// Render a toml::Value back to inline TOML (mirrors manifest.rs logic).
fn render_toml_value(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => format!("\"{}\"", s),
        toml::Value::Boolean(b) => b.to_string(),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Array(a) => {
            let items: Vec<String> = a.iter().map(render_toml_value).collect();
            format!("[{}]", items.join(", "))
        }
        toml::Value::Table(t) => {
            let items: Vec<String> = t
                .iter()
                .map(|(k, v)| format!("{} = {}", k, render_toml_value(v)))
                .collect();
            format!("{{ {} }}", items.join(", "))
        }
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Datetime(d) => format!("\"{}\"", d),
    }
}

/// Move a sub-crate's `src/` contents into the merged crate as a submodule
/// directory. `lib.rs` becomes `mod.rs`; all other files keep their names.
/// Moving the whole `src/` dir preserves internal `#[path]` and `mod`
/// relative references.
fn migrate_subcrate_src(sub_dir: &Path, merged_dir: &Path, module: &str) -> Result<()> {
    let src_dir = sub_dir.join("src");
    let dest_dir = merged_dir.join("src").join(module);
    if !src_dir.is_dir() {
        anyhow::bail!("missing src/ in {}", sub_dir.display());
    }
    fs::create_dir_all(&dest_dir).with_context(|| format!("create {}", dest_dir.display()))?;

    // Move every entry from src/ into dest/.
    for entry in fs::read_dir(&src_dir)? {
        let entry = entry?;
        let from = entry.path();
        let name = entry.file_name();
        let to = dest_dir.join(&name);
        fs::rename(&from, &to)
            .with_context(|| format!("move {} -> {}", from.display(), to.display()))?;
    }

    // Rename lib.rs -> mod.rs (the submodule entry point).
    let lib_rs = dest_dir.join("lib.rs");
    let mod_rs = dest_dir.join("mod.rs");
    if lib_rs.exists() {
        fs::rename(&lib_rs, &mod_rs)
            .with_context(|| format!("rename {} -> {}", lib_rs.display(), mod_rs.display()))?;
    }

    println!(
        "  ✓ {} → mod {}",
        sub_dir.file_name().unwrap().to_string_lossy(),
        module
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Stage 3: rewrite .rs imports + Cargo.toml dep lines.
// ---------------------------------------------------------------------------

/// Whether a `.rs` rewrite targets the merged crate itself (use `crate::`) or
/// downstream crates (use `<lib>::`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum RewriteScope {
    IntraCrate,
    Downstream,
}

impl RewriteScope {
    fn prefix(self, cluster: &Cluster) -> String {
        match self {
            RewriteScope::IntraCrate => "crate".to_string(),
            RewriteScope::Downstream => cluster.merged_lib_ident.to_string(),
        }
    }
}

/// Rewrite `publish/Cargo.toml`: replace the 17 merged `utils/<dir>` workspace
/// members with a single `utils/utils`, and collapse the 17 merged
/// `lemurclaw-utils-*` workspace dep entries into a single `lemurclaw-utils`.
/// Standalone crates keep their members + deps.
fn rewrite_publish_manifest(publish_root: &Path, cluster: &Cluster) -> Result<()> {
    let path = publish_root.join("Cargo.toml");
    let raw = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;

    let merge_dirs: std::collections::HashSet<&str> =
        cluster.members.iter().map(|sc| sc.dir).collect();
    let merge_pkgs: std::collections::HashSet<&str> =
        cluster.members.iter().map(|sc| sc.package).collect();

    // The member prefix in workspace.members lines. For utils it's "utils/";
    // for core (source_subdir empty) members appear bare (e.g. "protocol").
    let member_prefix = if cluster.source_subdir.is_empty() {
        String::new()
    } else {
        format!("{}/", cluster.source_subdir)
    };
    let merged_member_line = format!("\"{}\"", cluster.merged_member_path);
    let merged_dep_line = format!(
        "{} = {{ path = \"{}\" }}",
        cluster.merged_package, cluster.merged_member_path
    );
    let merged_dep_check = format!("{} = ", cluster.merged_package);

    let mut out = String::with_capacity(raw.len());
    let mut section = String::new();
    let mut in_workspace = false;
    let mut in_workspace_deps = false;

    for line in raw.lines() {
        let trimmed = line.trim_start();

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_workspace = trimmed == "[workspace]";
            in_workspace_deps = trimmed == "[workspace.dependencies]";
            section = trimmed.to_string();
            out.push_str(line);
            out.push('\n');
            continue;
        }

        // Inside [workspace] members: drop merged member entries (replace with
        // one merged_member_path); keep standalone ones.
        if in_workspace {
            let member_str = format!("\"{}", member_prefix);
            if trimmed.starts_with(&member_str) {
                // Extract the dir from "<prefix><dir>",
                let member_dir = trimmed
                    .trim_start_matches(&member_str)
                    .trim_end_matches("\",");
                if merge_dirs.contains(member_dir) {
                    // Merged — replace with the merged member (once).
                    if !out.contains(&merged_member_line) {
                        let indent = &line[..line.len() - trimmed.len()];
                        out.push_str(indent);
                        out.push_str(&merged_member_line);
                        out.push_str(",\n");
                    }
                    continue;
                }
                // Standalone — keep as-is.
                out.push_str(line);
                out.push('\n');
                continue;
            }
        }

        // Inside [workspace.dependencies]: collapse merged packages into one
        // merged_package; keep standalone ones.
        if in_workspace_deps {
            if let Some(pkg) = extract_dep_key(trimmed) {
                if merge_pkgs.contains(pkg.as_str()) {
                    if !out.contains(&merged_dep_check) {
                        let indent = &line[..line.len() - trimmed.len()];
                        out.push_str(&format!("{}{}\n", indent, merged_dep_line));
                    }
                    continue;
                }
            }
        }

        let _ = &section;
        out.push_str(line);
        out.push('\n');
    }

    fs::write(&path, &out).with_context(|| format!("write {}", path.display()))?;
    println!("Rewrote publish/Cargo.toml (members + workspace deps).");
    Ok(())
}

/// Fix `crate::` self-references inside a merged sub-module directory. When a
/// sub-crate used `crate::Foo` to reach its own root items, that path now needs
/// the module prefix: `crate::<module>::Foo`. Cross-module refs
/// (`crate::absolute_path::...`, already rewritten from `lemurclaw_utils_X`) are
/// left alone — we detect them by checking the first segment against the known
/// known module names.
fn fix_self_refs_in_submodule(dir: &Path, module: &str, cluster: &Cluster) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    let modules = cluster.module_names();
    fix_self_refs_recursive(dir, module, &modules)
}

fn fix_self_refs_recursive(
    dir: &Path,
    module: &str,
    modules: &std::collections::HashSet<String>,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            fix_self_refs_recursive(&path, module, modules)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            let raw =
                fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
            let rewritten = fix_self_refs_in_src(&raw, module, modules);
            if rewritten != raw {
                fs::write(&path, &rewritten)
                    .with_context(|| format!("write {}", path.display()))?;
            }
        }
    }
    Ok(())
}

/// Rewrite self-referential `crate::X` to `crate::<module>::X` where X is NOT a
/// known cross-module name. A `crate::` followed by a known module name (e.g.
/// `crate::absolute_path`) is a cross-module ref and is preserved.
fn fix_self_refs_in_src(
    src: &str,
    module: &str,
    modules: &std::collections::HashSet<String>,
) -> String {
    // Walk the source and rewrite `crate::` when the segment after it is not a
    // known module. We do a careful scan to avoid touching string literals or
    // `crate::<module>::` cross-refs.
    let needle = "crate::";
    let nbytes = needle.len();
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if i + nbytes <= bytes.len() && &bytes[i..i + nbytes] == needle.as_bytes() {
            // Check whether the previous char is an identifier char — if so,
            // this `crate::` is part of a longer identifier (e.g. `my_crate::`)
            // and must not be touched.
            let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
            // Read the segment following `crate::`.
            let seg_start = i + nbytes;
            let seg_end = seg_start
                + bytes[seg_start..]
                    .iter()
                    .take_while(|b| is_ident_byte(**b))
                    .count();
            if before_ok && seg_end > seg_start {
                let segment = &src[seg_start..seg_end];
                // Prefix this `crate::<segment>` if it's a self-ref. A self-ref
                // is either (a) a segment that's not a known cross-module name,
                // or (b) a segment equal to this submodule's own name — e.g.
                // inside `pty/`, `crate::pty` refers to pty's own `pty` child
                // module (now nested at `crate::pty::pty`), NOT the cross-module
                // `pty`. Without this, the name collision between the submodule
                // and its identically-named child module would be misread as a
                // cross-module ref.
                if !modules.contains(segment) || segment == module {
                    // Self-ref: inject module prefix.
                    out.push_str(&format!("crate::{}::{}", module, segment));
                    i = seg_end;
                    continue;
                }
            }
        }
        let ch_end = next_char_boundary(bytes, i);
        out.push_str(&src[i..ch_end]);
        i = ch_end;
    }
    out
}

/// Recursively rewrite every `.rs` file under `dir` using the given scope.
fn rewrite_rust_files_in(dir: &Path, scope: RewriteScope, cluster: &Cluster) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            rewrite_rust_files_in(&path, scope, cluster)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            let raw =
                fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
            let rewritten = rewrite_rs(&raw, scope, cluster);
            if rewritten != raw {
                fs::write(&path, &rewritten)
                    .with_context(|| format!("write {}", path.display()))?;
            }
        }
    }
    Ok(())
}

/// Rewrite `<lib>_<module>` crate-path segments (for the cluster's members
/// only) in `src` to `<prefix>::<module>`. Standalone crates' idents are left
/// untouched — they remain separate crates. This is a targeted text
/// substitution: the idents are specific enough that they only appear as crate
/// path segments in valid Rust.
fn rewrite_rs(src: &str, scope: RewriteScope, cluster: &Cluster) -> String {
    let prefix = scope.prefix(cluster);
    let mut out = src.to_string();
    for sc in &cluster.members {
        let ident = sc.package.replace('-', "_");
        let replacement = format!("{}::{}", prefix, sc.module);
        out = replace_ident_whole(&out, &ident, &replacement);
    }
    out
}

/// Replace every whole-word occurrence of `needle` in `haystack` with
/// `replacement`. "Whole word" means the char before (if any) and after (if
/// any) the match is not an identifier-continuation char (`[A-Za-z0-9_]`).
/// This prevents partial matches like `lemurclaw_utils_path` matching inside
/// `lemurclaw_utils_path_uri`.
fn replace_ident_whole(haystack: &str, needle: &str, replacement: &str) -> String {
    let bytes = haystack.as_bytes();
    let nbytes = needle.as_bytes();
    if nbytes.is_empty() || bytes.len() < nbytes.len() {
        return haystack.to_string();
    }
    let mut out = String::with_capacity(haystack.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if i + nbytes.len() <= bytes.len() && &bytes[i..i + nbytes.len()] == nbytes {
            let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
            let after_idx = i + nbytes.len();
            let after_ok = after_idx >= bytes.len() || !is_ident_byte(bytes[after_idx]);
            if before_ok && after_ok {
                out.push_str(replacement);
                i = after_idx;
                continue;
            }
        }
        // Copy one UTF-8 char (safe: we advance by byte only when no match).
        let ch_end = next_char_boundary(bytes, i);
        out.push_str(&haystack[i..ch_end]);
        i = ch_end;
    }
    out
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Return the byte index of the next char boundary after `start`.
fn next_char_boundary(bytes: &[u8], start: usize) -> usize {
    let mut j = start + 1;
    while j < bytes.len() && (bytes[j] & 0xC0) == 0x80 {
        j += 1;
    }
    j
}

/// Rewrite every downstream (non-utils) crate in `publish/`: `.rs` files get
/// `lemurclaw_utils::<sub>` path conversion, and each crate's `Cargo.toml`
/// collapses its `lemurclaw-utils-* = { workspace = true }` dep lines into a
/// single `lemurclaw-utils = { workspace = true }`.
///
/// Crates can be nested (e.g. `ext/items`, `memories/write`, `utils/cargo-bin`),
/// so this walks the whole tree, skipping the merged `utils/utils` crate
/// (handled by the intra-crate pass) and `target/`.
fn rewrite_downstream(publish_root: &Path, merged_dir: &Path, cluster: &Cluster) -> Result<()> {
    let mut rs_files = 0usize;
    let mut toml_files = 0usize;
    rewrite_downstream_dir(
        publish_root,
        merged_dir,
        cluster,
        &mut rs_files,
        &mut toml_files,
    )?;
    let _ = merged_dir;
    println!(
        "Rewrote downstream: {} .rs files, {} Cargo.toml dep-collapsed.",
        rs_files, toml_files
    );
    Ok(())
}

/// Recursive worker for `rewrite_downstream`.
fn rewrite_downstream_dir(
    dir: &Path,
    merged_dir: &Path,
    cluster: &Cluster,
    rs_files: &mut usize,
    toml_files: &mut usize,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        // Skip build artifacts and the merged crate (handled by intra-crate pass).
        if name == "target" || path == *merged_dir {
            continue;
        }
        if path.is_dir() {
            rewrite_downstream_dir(&path, merged_dir, cluster, rs_files, toml_files)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            let raw =
                fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
            let rewritten = rewrite_rs(&raw, RewriteScope::Downstream, cluster);
            if rewritten != raw {
                fs::write(&path, &rewritten)
                    .with_context(|| format!("write {}", path.display()))?;
                *rs_files += 1;
            }
        } else if name == "Cargo.toml" {
            // The workspace root (publish/Cargo.toml) is already handled by
            // rewrite_publish_manifest; collapse is a no-op there. For all
            // other crate manifests, collapse member dep lines.
            collapse_deps_in_manifest(&path, cluster)?;
            *toml_files += 1;
        }
    }
    Ok(())
}

/// Collapse the cluster's member dep lines in a crate manifest into a single
/// `<merged_package> = { workspace = true }`. Standalone crates are preserved.
fn collapse_deps_in_manifest(path: &Path, cluster: &Cluster) -> Result<()> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let merge_pkgs: std::collections::HashSet<&str> =
        cluster.members.iter().map(|sc| sc.package).collect();
    let collapsed_line = format!("{} = {{ workspace = true }}", cluster.merged_package);
    let mut out = String::with_capacity(raw.len());
    let mut emitted = false;
    for line in raw.lines() {
        let trimmed = line.trim_start();
        if let Some(pkg) = extract_dep_key(trimmed) {
            if merge_pkgs.contains(pkg.as_str()) {
                if !emitted {
                    let indent = &line[..line.len() - trimmed.len()];
                    out.push_str(&format!("{}{}\n", indent, collapsed_line));
                    emitted = true;
                }
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    if out != raw {
        fs::write(path, &out).with_context(|| format!("write {}", path.display()))?;
    }
    Ok(())
}

/// Extract the dep key (the part before ` = `) from a `key = value` manifest
/// line. Returns None if the line isn't a dep assignment.
fn extract_dep_key(trimmed: &str) -> Option<String> {
    let eq = trimmed.find(" = ")?;
    Some(trimmed[..eq].trim().to_string())
}
