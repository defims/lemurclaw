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

    let merged_dir = utils_root.join("utils");
    if merged_dir.exists() {
        anyhow::bail!(
            "{} already exists — remove it before re-running (or it was left from a partial run)",
            merged_dir.display()
        );
    }

    // Step 1: collect each sub-crate's dependencies, then create the merged
    // crate dir with a synthesized Cargo.toml and lib.rs.
    let agg = collect_aggregate_deps(&utils_root)?;
    create_merged_crate(&merged_dir, &agg)?;

    // Step 2: move each sub-crate's src/ into the merged crate as a submodule.
    for sc in SUB_CRATES {
        migrate_subcrate_src(&utils_root.join(sc.dir), &merged_dir, sc.module)?;
    }

    // Step 3: delete the 23 original sub-crate dirs.
    for sc in SUB_CRATES {
        let dir = utils_root.join(sc.dir);
        fs::remove_dir_all(&dir).with_context(|| format!("delete {}", dir.display()))?;
    }
    println!("Deleted {} original sub-crate dirs.", SUB_CRATES.len());

    // Steps 4-5 (Cargo.toml + downstream .rs rewrite) land in Stage 3.
    println!("\nCrate structure created. Run `cargo metadata` in publish/ to verify,");
    println!("then Stage 3 will rewrite publish/Cargo.toml + downstream imports.");
    Ok(())
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

/// Read every sub-crate's Cargo.toml and merge their dependency tables. Deps
/// pointing at sibling `codex-*`/`lemurclaw-*` workspace crates are dropped
/// here — they become intra-crate references inside the merged crate (a
/// `pub mod` is in the same crate, so no dep declaration is needed).
fn collect_aggregate_deps(utils_root: &Path) -> Result<AggregateDeps> {
    let mut agg = AggregateDeps {
        deps: BTreeMap::new(),
        dev_deps: BTreeMap::new(),
        target_deps: BTreeMap::new(),
        target_dev_deps: BTreeMap::new(),
    };
    for sc in SUB_CRATES {
        let path = utils_root.join(sc.dir).join("Cargo.toml");
        let raw = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let doc: toml::Value =
            toml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
        let table = doc.as_table().context("manifest is not a table")?;

        if let Some(deps) = table.get("dependencies").and_then(|v| v.as_table()) {
            merge_dep_table(&mut agg.deps, deps);
        }
        if let Some(deps) = table.get("dev-dependencies").and_then(|v| v.as_table()) {
            merge_dep_table(&mut agg.dev_deps, deps);
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
                    merge_dep_table(entry, d);
                }
                if let Some(d) = scope.get("dev-dependencies").and_then(|v| v.as_table()) {
                    let entry = agg.target_dev_deps.entry(cfg.clone()).or_default();
                    merge_dep_table(entry, d);
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
) {
    for (key, val) in src {
        // Skip internal workspace crates (now intra-crate after merge).
        if key.starts_with("codex-") || key.starts_with("lemurclaw-") {
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
fn create_merged_crate(merged_dir: &Path, agg: &AggregateDeps) -> Result<()> {
    fs::create_dir_all(merged_dir.join("src")).context("create merged crate src/")?;

    // Cargo.toml
    let mut toml = String::new();
    toml.push_str("[package]\n");
    toml.push_str(&format!("name = \"{}\"\n", MERGED_PACKAGE));
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
    toml.push_str(&format!("name = \"{}\"\n", MERGED_LIB_IDENT));
    toml.push_str("doctest = false\n");

    fs::write(merged_dir.join("Cargo.toml"), &toml).context("write merged Cargo.toml")?;

    // lib.rs: one `pub mod` per sub-crate, in SUB_CRATES order.
    let mut lib = String::from(
        "//! Merged `lemurclaw-utils` crate — union of the 23 former\n\
         //! `lemurclaw-utils-*` sub-crates, each exposed as a `pub mod`.\n",
    );
    for sc in SUB_CRATES {
        lib.push_str(&format!("pub mod {};\n", sc.module));
    }
    fs::write(merged_dir.join("src").join("lib.rs"), &lib).context("write merged lib.rs")?;

    println!(
        "Created merged crate {} ({} submodules).",
        MERGED_PACKAGE,
        SUB_CRATES.len()
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
