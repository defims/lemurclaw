//! `publish strip-v8`: remove the V8 runtime from the lemurclaw-rs/ tree.
//!
//! Runs **after** `publish rename`. It rewrites the already-renamed
//! `lemurclaw-code-mode` crate into a V8-free stub (only re-exports the
//! protocol crate and provides session-provider stubs that always fail at
//! runtime), then removes the V8-bearing crates and workspace dependency
//! declarations so the final `lemurclaw` binary no longer links
//! `librusty_v8` (~120 MB saved).
//!
//! The transformation is idempotent: running it twice produces the same
//! output. The original `codex-rs/` source tree is never touched.
//!
//! ## Why a stub instead of Cargo features
//!
//! Downstream code (`lemurclaw-core`, `lemurclaw-app-server`) holds session
//! providers behind the `Arc<dyn CodeModeSessionProvider>` trait-object
//! boundary and constructs the concrete types (`InProcessCodeModeSessionProvider`,
//! `ProcessOwnedCodeModeSessionProvider`, `WebSocketCodeModeSessionProvider`)
//! at exactly three call sites. By replacing `lemurclaw-code-mode` with a stub
//! that re-exports those type names with matching method signatures, every
//! downstream source file compiles unchanged — no `#[cfg]` gates, no Cargo
//! features, and no edits to `codex-rs/`.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::manifest::rename_package;

/// Crates that exist only to provide or exercise the V8 runtime. They are
/// dropped entirely from the lemurclaw-rs workspace.
const V8_ONLY_CRATES: &[&str] = &["code-mode-host", "v8-poc"];

pub fn run() -> Result<()> {
    let publish_root = locate_publish_root()?;

    println!("publish strip-v8\n  target: {}\n", publish_root.display());

    // The lemurclaw-rs tree has already been renamed codex-* → lemurclaw-*, so the
    // code-mode crate lives under lemurclaw-rs/code-mode/ with package name
    // `lemurclaw-code-mode` and lib name `lemurclaw_code_mode`.
    let code_mode_dir = publish_root.join("code-mode");
    let manifest_path = code_mode_dir.join("Cargo.toml");
    if !manifest_path.exists() {
        bail!(
            "expected the code-mode crate at {}, but it is missing.\n\
             Run `xtask publish rename` before `strip-v8`.",
            code_mode_dir.display()
        );
    }

    // Read the current (renamed) manifest to recover the exact package / lib
    // names rather than assuming the lemurclaw- prefix.
    let (package_name, lib_name) = read_code_mode_identity(&manifest_path)?;
    println!(
        "  code-mode crate: package `{}`, lib `{}`",
        package_name, lib_name
    );

    // The protocol crate name follows the same rename. Recover it from the
    // manifest's dependency list if present, else fall back to the rename of
    // the upstream protocol package name.
    let protocol_dep_key = protocol_dependency_key(&manifest_path)?;
    println!(
        "  protocol dep key in code-mode manifest: `{}`",
        protocol_dep_key
    );

    // Transform A: replace the code-mode crate with a V8-free stub.
    stub_code_mode_crate(&code_mode_dir, &package_name, &lib_name, &protocol_dep_key)?;

    // Transform B: drop V8-only crates from the lemurclaw-rs tree.
    let mut removed_crates = Vec::new();
    for name in V8_ONLY_CRATES {
        let dir = publish_root.join(name);
        if dir.exists() {
            fs::remove_dir_all(&dir).with_context(|| format!("remove {}", dir.display()))?;
            removed_crates.push(name.to_string());
            println!("  removed crate directory: {}", name);
        }
    }

    // Transform C: rewrite the workspace manifest — drop v8/deno deps, drop
    // V8-only members, drop any dependency entries pointing at removed crates.
    rewrite_workspace_manifest(&publish_root, &removed_crates)?;

    // Transform D: regenerate the lockfile so V8 packages disappear.
    regenerate_lockfile(&publish_root)?;

    println!();
    println!("✓ strip-v8 complete. The lemurclaw-rs/ tree no longer links V8.");
    println!("  Verify with: `cargo check --workspace` in lemurclaw-rs/.");
    Ok(())
}

/// `(package name, lib name)` read from the existing (renamed) manifest.
fn read_code_mode_identity(manifest_path: &Path) -> Result<(String, String)> {
    let raw = fs::read_to_string(manifest_path)
        .with_context(|| format!("read {}", manifest_path.display()))?;
    let doc: toml::Value =
        toml::from_str(&raw).with_context(|| format!("parse {}", manifest_path.display()))?;
    let package_name = doc
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|v| v.as_str())
        .context("code-mode manifest missing package.name")?
        .to_string();
    let lib_name = doc
        .get("lib")
        .and_then(|l| l.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or(&package_name.replace('-', "_"))
        .to_string();
    Ok((package_name, lib_name))
}

/// The dependency key the code-mode crate uses to reference the protocol crate
/// (e.g. `lemurclaw-code-mode-protocol`). Falls back to the rename of the
/// upstream key if the manifest does not declare it.
fn protocol_dependency_key(manifest_path: &Path) -> Result<String> {
    let raw = fs::read_to_string(manifest_path)?;
    let doc: toml::Value = toml::from_str(&raw)?;
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(table) = doc.get(section).and_then(|v| v.as_table()) {
            for key in table.keys() {
                if key.contains("code-mode-protocol") || key.contains("code_mode_protocol") {
                    return Ok(key.clone());
                }
            }
        }
    }
    // Fallback: apply the same codex- → lemurclaw- rename to the upstream key.
    Ok(rename_package("codex-code-mode-protocol"))
}

/// Replace every file under `code-mode/src/` with the stub sources and rewrite
/// `Cargo.toml` to drop the V8 dependencies.
fn stub_code_mode_crate(
    code_mode_dir: &Path,
    package_name: &str,
    lib_name: &str,
    protocol_dep_key: &str,
) -> Result<()> {
    let src_dir = code_mode_dir.join("src");

    // Wipe the existing src/ tree (V8 implementation + tests) and recreate it
    // with just the stub files. This is what makes strip-v8 idempotent.
    if src_dir.exists() {
        fs::remove_dir_all(&src_dir).with_context(|| format!("remove {}", src_dir.display()))?;
    }
    fs::create_dir_all(&src_dir).context("recreate code-mode src/")?;

    // Determine the protocol crate's Rust identifier for the `pub use`.
    let protocol_ident = protocol_dep_key.replace('-', "_");

    fs::write(src_dir.join("lib.rs"), stub_lib_rs(&protocol_ident)).context("write stub lib.rs")?;
    fs::write(src_dir.join("service.rs"), stub_service_rs(&protocol_ident))
        .context("write stub service.rs")?;

    // Rewrite Cargo.toml: keep only package/lib/lints + the protocol dep.
    fs::write(
        code_mode_dir.join("Cargo.toml"),
        stub_cargo_toml(package_name, lib_name, protocol_dep_key),
    )
    .context("write stub Cargo.toml")?;

    println!(
        "  stubbed `code-mode` crate ({} → V8-free stub)",
        package_name
    );
    Ok(())
}

fn stub_cargo_toml(package_name: &str, lib_name: &str, protocol_dep_key: &str) -> String {
    // `lemurclaw-http-client` is retained because the
    // WebSocketCodeModeSessionProvider stub keeps the
    // `with_http_client_factory(url, HttpClientFactory)` signature so downstream
    // call sites compile unchanged. It does NOT pull in V8. The dependency key
    // follows the publish-tree rename of the upstream `codex-http-client`.
    let http_client_key = rename_package("codex-http-client");
    format!(
        r#"[package]
edition.workspace = true
license.workspace = true
name = "{package_name}"
version.workspace = true

[lib]
doctest = false
name = "{lib_name}"
path = "src/lib.rs"

[lints]
workspace = true

[dependencies]
{protocol_dep_key} = {{ workspace = true }}
{http_client_key} = {{ workspace = true }}
"#
    )
}

fn stub_lib_rs(protocol_ident: &str) -> String {
    format!(
        r#"//! V8-free stub of code-mode for lemurclaw builds.
//!
//! Only the protocol types are re-exported. The session providers exist as
//! type stubs so downstream call sites compile, but they return errors at
//! runtime because the V8 runtime is excluded from this build. To restore
//! real V8-backed code mode, re-run `xtask publish rename` without stripping.

pub use {protocol_ident}::*;

mod service;

pub use service::InProcessCodeModeSessionProvider;
pub use service::ProcessOwnedCodeModeSessionProvider;
pub use service::WebSocketCodeModeSessionProvider;
"#
    )
}

fn stub_service_rs(protocol_ident: &str) -> String {
    // The http-client crate identifier follows the publish-tree rename.
    let http_client_ident = crate::manifest::rename_ident("codex_http_client");
    format!(
        r#"//! Stub session providers for V8-free builds.
//!
//! These types mirror the public API surface of the real V8-backed providers
//! (`InProcessCodeModeSessionProvider`, `ProcessOwnedCodeModeSessionProvider`,
//! `WebSocketCodeModeSessionProvider`) so that downstream code compiles
//! unchanged. Every `create_session` call returns an error at runtime.

use std::path::PathBuf;
use std::sync::Arc;

use {protocol_ident}::CodeModeSessionDelegate;
use {protocol_ident}::CodeModeSessionProvider;
use {protocol_ident}::CodeModeSessionProviderFuture;

const UNAVAILABLE: &str = "code-mode V8 runtime is excluded from this build";

/// In-process provider stub. The real implementation embeds a V8 isolate; this
/// stub always fails to create a session.
pub struct InProcessCodeModeSessionProvider;

impl CodeModeSessionProvider for InProcessCodeModeSessionProvider {{
    fn create_session<'a>(
        &'a self,
        _delegate: Arc<dyn CodeModeSessionDelegate>,
    ) -> CodeModeSessionProviderFuture<'a> {{
        Box::pin(async {{ Err(UNAVAILABLE.to_string()) }})
    }}
}}

impl Default for InProcessCodeModeSessionProvider {{
    fn default() -> Self {{
        Self
    }}
}}

/// Process-owned provider stub. The real implementation spawns a
/// `code-mode-host` subprocess (with an in-process V8 fallback); this stub
/// preserves the constructor surface so call sites compile.
pub struct ProcessOwnedCodeModeSessionProvider;

impl ProcessOwnedCodeModeSessionProvider {{
    /// Stub constructor. The host program argument is ignored.
    pub fn with_host_program(_host_program: PathBuf) -> Self {{
        Self
    }}

    /// Stub configurator. Returns `self` unchanged.
    pub fn without_in_process_fallback(self) -> Self {{
        self
    }}
}}

impl Default for ProcessOwnedCodeModeSessionProvider {{
    fn default() -> Self {{
        Self
    }}
}}

impl CodeModeSessionProvider for ProcessOwnedCodeModeSessionProvider {{
    fn create_session<'a>(
        &'a self,
        _delegate: Arc<dyn CodeModeSessionDelegate>,
    ) -> CodeModeSessionProviderFuture<'a> {{
        Box::pin(async {{ Err(UNAVAILABLE.to_string()) }})
    }}
}}

/// WebSocket provider stub. The real implementation connects to a remote
/// code-mode host over WebSocket; this stub preserves the constructor surface
/// so `lemurclaw-app-server` compiles unchanged.
pub struct WebSocketCodeModeSessionProvider;

impl WebSocketCodeModeSessionProvider {{
    /// Stub constructor. Both arguments are ignored.
    pub fn with_http_client_factory(
        _websocket_url: String,
        _http_client_factory: {http_client_ident}::HttpClientFactory,
    ) -> Self {{
        Self
    }}
}}

impl CodeModeSessionProvider for WebSocketCodeModeSessionProvider {{
    fn create_session<'a>(
        &'a self,
        _delegate: Arc<dyn CodeModeSessionDelegate>,
    ) -> CodeModeSessionProviderFuture<'a> {{
        Box::pin(async {{ Err(UNAVAILABLE.to_string()) }})
    }}
}}
"#,
    )
}

/// Rewrite `lemurclaw-rs/Cargo.toml`: drop `v8` and `deno_core_icudata` from
/// `[workspace.dependencies]`, drop V8-only members from `members`, and drop
/// any workspace-dep entries pointing at removed crates.
fn rewrite_workspace_manifest(publish_root: &Path, removed_crates: &[String]) -> Result<()> {
    let manifest_path = publish_root.join("Cargo.toml");
    let raw = fs::read_to_string(&manifest_path)
        .with_context(|| format!("read {}", manifest_path.display()))?;

    let mut out = String::with_capacity(raw.len());
    let mut in_workspace_deps = false;
    // `members = [` opens a multi-line array; we are inside it until the
    // closing `]`. Unlike TOML section headers this is a key = value line, so
    // it is tracked separately from the `[...]` section detection.
    let mut in_members = false;

    let removed_dep_keys: Vec<String> = removed_crates
        .iter()
        .map(|c| rename_package(&format!("codex-{c}")))
        .collect();

    for line in raw.lines() {
        let trimmed = line.trim_start();

        // Detect TOML section headers like `[workspace.dependencies]`. A new
        // header always closes any members array we were inside.
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_workspace_deps = trimmed == "[workspace.dependencies]";
            in_members = false;
            out.push_str(line);
            out.push('\n');
            continue;
        }

        // Detect the members-array opening line `members = [`.
        if trimmed.starts_with("members = [") {
            in_members = !trimmed.contains(']');
            out.push_str(line);
            out.push('\n');
            continue;
        }

        // Inside members = [...], skip entries referencing removed crate dirs
        // and detect the closing bracket.
        if in_members {
            if trimmed.starts_with(']') {
                in_members = false;
                out.push_str(line);
                out.push('\n');
                continue;
            }
            if removed_member_should_skip(trimmed, removed_crates) {
                continue;
            }
            out.push_str(line);
            out.push('\n');
            continue;
        }

        // Inside [workspace.dependencies], drop v8 / deno / removed-crate keys.
        if in_workspace_deps {
            if let Some(key) = dep_key_on_line(trimmed) {
                if key == "v8"
                    || key == "deno_core_icudata"
                    || removed_dep_keys.iter().any(|r| r == &key)
                {
                    continue;
                }
            }
        }

        out.push_str(line);
        out.push('\n');
    }

    fs::write(&manifest_path, &out).context("write lemurclaw-rs/Cargo.toml")?;
    println!(
        "  rewrote workspace manifest (dropped v8, deno_core_icudata{} entries)",
        if removed_dep_keys.is_empty() {
            String::new()
        } else {
            format!(", {}", removed_dep_keys.join(", "))
        }
    );
    Ok(())
}

/// Returns true if a `members` array entry line references a removed crate dir.
fn removed_member_should_skip(trimmed: &str, removed_crates: &[String]) -> bool {
    // Member entries look like `    "code-mode-host",` (directory paths are
    // preserved as-is by rename; only package names change). Match the quoted
    // dir name.
    let stripped = trimmed.trim().trim_start_matches('"');
    for crate_dir in removed_crates {
        if stripped.starts_with(crate_dir.as_str()) {
            return true;
        }
    }
    false
}

/// Extract the dependency key from a `key = value` line within a deps section.
fn dep_key_on_line(trimmed: &str) -> Option<String> {
    let eq = trimmed.find('=')?;
    let key = trimmed[..eq].trim();
    if key.is_empty() || key.starts_with('[') {
        return None;
    }
    Some(key.to_string())
}

/// Regenerate the lockfile so V8 packages are no longer pinned. Instead of
/// deleting the lockfile (which causes version drift on full regeneration),
/// run `cargo update` so only the changed entries are updated — preserving the
/// existing version selections for non-V8 dependencies. After the bulk update,
/// pin back any pre-release crates that `cargo update` may have promoted to a
/// stable release within the same semver range (cargo treats `0.3.0-alpha.4`
/// as compatible with `^0.3.0`, but the alpha and stable lines are not actually
/// ABI-compatible — e.g. `rama-core 0.3.0-alpha.4` requires `rama-error
/// 0.3.0-alpha.4`, not the stable `0.3.0`).
fn regenerate_lockfile(publish_root: &Path) -> Result<()> {
    let lockfile = publish_root.join("Cargo.lock");
    if !lockfile.exists() {
        // No lockfile yet — `cargo check` will generate one, nothing to do.
        return Ok(());
    }
    println!("  running `cargo update` to refresh lockfile (preserve existing versions)…");
    let status = Command::new("cargo")
        .args(["update"])
        .current_dir(publish_root)
        .status()
        .context("spawn cargo update")?;
    if !status.success() {
        // cargo update may fail if the manifest is inconsistent; fall back to
        // deleting the lockfile so cargo regenerates from scratch.
        eprintln!("  warn: cargo update failed; removing Cargo.lock for full regeneration");
        fs::remove_file(&lockfile).context("remove Cargo.lock")?;
        return Ok(());
    }

    // Re-pin pre-release crates that cargo may have promoted to a stable
    // release. The `rama-*` family ships alpha and stable lines that share a
    // `^0.3.0` range but are NOT ABI-compatible (the alpha has types the
    // stable release renamed/removed, e.g. `OpaqueError`).
    for (pkg, version) in PRERELEASE_PINS {
        pin_package(publish_root, pkg, version)?;
    }

    Ok(())
}

/// Pre-release crates that must stay on their pre-release version even though
/// their semver range (`^0.3.0`) admits a newer stable release. Each entry is
/// (package name, pre-release version that must be enforced).
///
/// Order matters: a crate must be pinned before any crate that depends on it
/// with a stable-only semver range. `rama-utils` depends on `rama-macros`
/// (`^0.3.0`), so `rama-utils` must be downgraded to `0.3.0-alpha.4` first —
/// only then can `rama-macros` be pinned to `0.3.0-alpha.4` without cargo
/// rejecting it as incompatible with the stable `rama-utils 0.3.0`.
const PRERELEASE_PINS: &[(&str, &str)] = &[
    ("rama-utils", "0.3.0-alpha.4"),
    ("rama-macros", "0.3.0-alpha.4"),
    ("rama-error", "0.3.0-alpha.4"),
];

/// Run `cargo update -p <pkg> --precise <version>` and treat "already at this
/// version" as success (cargo exits non-zero when the requested version equals
/// the current one).
fn pin_package(publish_root: &Path, pkg: &str, version: &str) -> Result<()> {
    let output = Command::new("cargo")
        .args(["update", "-p", pkg, "--precise", version])
        .current_dir(publish_root)
        .output()
        .with_context(|| format!("spawn cargo update -p {} --precise {}", pkg, version))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Cargo reports "not updated" when the package is already at the requested
    // version — that's the desired state, not an error.
    if stderr.contains("did not do anything") || stderr.contains("already") {
        return Ok(());
    }
    // If the package isn't in the lockfile at all, that's fine too.
    if stderr.contains("could not find") || stderr.contains("not found") {
        return Ok(());
    }
    anyhow::bail!(
        "cargo update -p {} --precise {} failed: {}",
        pkg,
        version,
        stderr.trim()
    );
}

fn locate_publish_root() -> Result<PathBuf> {
    // Allow callers (and tests) to point strip-v8 at an arbitrary tree.
    if let Ok(override_path) = std::env::var("STRIP_V8_OUTPUT_ROOT") {
        let publish = PathBuf::from(override_path);
        if !publish.is_dir() {
            bail!(
                "STRIP_V8_OUTPUT_ROOT is set to {}, which is not a directory.",
                publish.display()
            );
        }
        return Ok(publish);
    }
    let repo_root = locate_repo_root()?;
    let publish = repo_root.join("lemurclaw-rs");
    if !publish.is_dir() {
        bail!(
            "lemurclaw-rs/ not found at {}. Run `xtask publish rename` first.",
            publish.display()
        );
    }
    Ok(publish)
}

/// The original `codex-rs/` source tree, used by `restore` to re-copy the
/// V8 implementation.
fn locate_codex_root() -> Result<PathBuf> {
    let repo_root = locate_repo_root()?;
    let codex = repo_root.join("codex-rs");
    if !codex.is_dir() {
        bail!("codex-rs/ source tree not found at {}", codex.display());
    }
    Ok(codex)
}

fn locate_repo_root() -> Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .join("..")
        .canonicalize()
        .context("canonicalize repo root")
}

// ===========================================================================
// restore: undo strip-v8 by re-copying the full V8 implementation from the
// source tree.
// ===========================================================================

/// Crates that `strip-v8` removes and `restore` must re-create. Maps the
/// source-tree directory name to the lemurclaw-rs directory name (unchanged —
/// directory layout is preserved across rename).
const RESTORE_CRATE_DIRS: &[(&str, &str)] = &[
    ("code-mode", "code-mode"),
    ("code-mode-host", "code-mode-host"),
    ("v8-poc", "v8-poc"),
];

/// Workspace dependency entries that `strip-v8` drops and `restore` must
/// re-add. Each entry is `(dep_key, rendered_value)` where the value
/// is already renamed for the lemurclaw-rs tree.
const RESTORE_WORKSPACE_DEPS: &[(&str, &str)] = &[
    ("v8", "\"=150.4.0\""),
    ("deno_core_icudata", "\"0.77.0\""),
    // code-mode-host's workspace dep is re-derived from the source manifest in
    // restore_workspace_manifest, so it is not hardcoded here.
];

pub fn restore() -> Result<()> {
    let publish_root = locate_publish_root()?;
    let codex_root = locate_codex_root()?;

    println!(
        "publish restore-v8\n  output:  {}\n  source:  {}\n",
        publish_root.display(),
        codex_root.display()
    );

    // Re-copy the full code-mode crate (V8 implementation) over the stub.
    restore_code_mode_crate(&codex_root, &publish_root)?;

    // Re-create the V8-only crate directories that strip-v8 removed.
    let mut restored_crates = Vec::new();
    for &(src_dir, dst_dir) in RESTORE_CRATE_DIRS {
        if dst_dir == "code-mode" {
            continue; // handled above
        }
        let src = codex_root.join(src_dir);
        let dst = publish_root.join(dst_dir);
        if dst.exists() {
            // Already present (e.g. restore run twice) — leave as-is.
            continue;
        }
        if !src.exists() {
            println!("  warn: source {} missing, skipping", src.display());
            continue;
        }
        copy_crate_dir(&src, &dst)?;
        rewrite_rust_files(&dst)?;
        println!("  restored crate directory: {}", dst_dir);
        restored_crates.push(dst_dir.to_string());
    }

    // Restore the workspace manifest: re-add v8/deno deps and the removed
    // member entries.
    restore_workspace_manifest(&publish_root, &codex_root, &restored_crates)?;

    // Regenerate the lockfile so V8 packages re-enter resolution.
    regenerate_lockfile(&publish_root)?;

    println!();
    println!("✓ restore-v8 complete. The lemurclaw-rs/ tree links V8 again.");
    println!("  Verify with: `cargo check --workspace` in lemurclaw-rs/.");
    Ok(())
}

/// Re-copy the full code-mode crate from the source tree, applying the same
/// codex-* → lemurclaw-* rename that `publish rename` uses.
fn restore_code_mode_crate(codex_root: &Path, publish_root: &Path) -> Result<()> {
    let src = codex_root.join("code-mode");
    let dst = publish_root.join("code-mode");
    if !src.is_dir() {
        bail!("source code-mode crate missing at {}", src.display());
    }

    // Replace the (possibly stubbed) lemurclaw-rs code-mode with the full source.
    if dst.exists() {
        fs::remove_dir_all(&dst).with_context(|| format!("remove {}", dst.display()))?;
    }
    copy_crate_dir(&src, &dst)?;

    // Rewrite Cargo.toml: rename codex-* → lemurclaw-*.
    let manifest_path = dst.join("Cargo.toml");
    let raw = fs::read_to_string(&manifest_path)
        .with_context(|| format!("read {}", manifest_path.display()))?;
    let renamed = crate::manifest::rewrite_crate_manifest(&raw, false, &[])?;
    fs::write(&manifest_path, &renamed).context("write restored code-mode Cargo.toml")?;

    // Rewrite every .rs file (codex_foo → lemurclaw_foo).
    rewrite_rust_files(&dst)?;

    println!("  restored code-mode crate (full V8 implementation)");
    Ok(())
}

/// Copy a crate directory recursively, skipping build artifacts.
fn copy_crate_dir(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
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

/// Run the AST-driven source rewrite over every `.rs` file under `dir`.
fn rewrite_rust_files(dir: &Path) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let meta = entry.metadata()?;
        if meta.is_dir() {
            rewrite_rust_files(&path)?;
        } else if meta.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
            if let Err(e) = crate::source_rewrite::rewrite_file(&path) {
                eprintln!("warn: failed to rewrite {}: {}", path.display(), e);
            }
        }
    }
    Ok(())
}

/// Re-add the v8/deno_core_icudata workspace deps and the V8-only member
/// entries that `strip-v8` removed.
fn restore_workspace_manifest(
    publish_root: &Path,
    codex_root: &Path,
    restored_members: &[String],
) -> Result<()> {
    let manifest_path = publish_root.join("Cargo.toml");
    let raw = fs::read_to_string(&manifest_path)
        .with_context(|| format!("read {}", manifest_path.display()))?;

    // Recover the code-mode-host workspace dep key from the source manifest so
    // we re-add the renamed entry.
    let codex_ws = codex_root.join("Cargo.toml");
    let code_mode_host_ws_dep = read_source_workspace_dep(&codex_ws, "codex-code-mode-host");

    let mut out = String::with_capacity(raw.len());
    let mut in_workspace_deps = false;
    let mut in_members = false;
    let mut members_indent = String::from("    ");
    let mut members_added: Vec<String> = restored_members.to_vec();

    for line in raw.lines() {
        let trimmed = line.trim_start();

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_workspace_deps = trimmed == "[workspace.dependencies]";
            in_members = false;
            out.push_str(line);
            out.push('\n');
            continue;
        }

        if trimmed.starts_with("members = [") {
            in_members = !trimmed.contains(']');
            // Capture the indentation of member entries (usually 4 spaces).
            let line_indent = &line[..line.len() - line.trim_start().len()];
            members_indent = line_indent.to_string() + "    ";
            out.push_str(line);
            out.push('\n');
            continue;
        }

        // Insert restored member entries just before the members array closes.
        if in_members && trimmed.starts_with(']') {
            for m in restored_members {
                out.push_str(&format!("{}\"{}\",\n", members_indent, m));
            }
            members_added.clear();
            in_members = false;
            out.push_str(line);
            out.push('\n');
            continue;
        }

        // Re-add v8/deno_core_icudata deps at the end of [workspace.dependencies].
        if in_workspace_deps && (trimmed.starts_with('[') || is_section_boundary(trimmed)) {
            // Before leaving the deps section, append the missing entries.
            append_missing_deps(&mut out, &raw, code_mode_host_ws_dep.as_deref());
            in_workspace_deps = false;
        }

        out.push_str(line);
        out.push('\n');
    }

    // If the file ended while still in [workspace.dependencies], append now.
    if in_workspace_deps {
        append_missing_deps(&mut out, &raw, code_mode_host_ws_dep.as_deref());
    }
    let _ = members_added;

    fs::write(&manifest_path, &out).context("write lemurclaw-rs/Cargo.toml")?;
    println!("  restored workspace manifest (v8, deno_core_icudata entries)");
    Ok(())
}
/// Detect whether a trimmed line marks the end of `[workspace.dependencies]`
/// (e.g. another top-level key like `[profile...]`). Section headers are
/// already handled by the caller; this catches bare-key section transitions.
fn is_section_boundary(trimmed: &str) -> bool {
    trimmed.starts_with('[')
}

/// Append any of the RESTORE_WORKSPACE_DEPS entries (plus the code-mode-host
/// dep) that are not already present in `current_manifest`.
fn append_missing_deps(out: &mut String, current_manifest: &str, code_mode_host_dep: Option<&str>) {
    for (key, value) in RESTORE_WORKSPACE_DEPS {
        if !current_manifest.contains(&format!("{key} =")) {
            out.push_str(&format!("{key} = {value}\n"));
        }
    }
    if let Some(rendered) = code_mode_host_dep {
        let key = rename_package("codex-code-mode-host");
        if !current_manifest.contains(&format!("{key} =")) {
            out.push_str(&format!("{rendered}\n"));
        }
    }
}

/// Read a single `[workspace.dependencies]` entry from the source workspace
/// manifest, returning it rendered for the lemurclaw-rs tree (renamed). Returns
/// None if not found.
fn read_source_workspace_dep(codex_ws: &Path, key: &str) -> Option<String> {
    let raw = fs::read_to_string(codex_ws).ok()?;
    let doc: toml::Value = toml::from_str(&raw).ok()?;
    let deps = doc
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(|v| v.as_table())?;
    let value = deps.get(key)?;
    // Render the value, renaming any codex-* package references.
    let renamed_key = rename_package(key);
    let rendered_value = match value {
        toml::Value::Table(t) => {
            let mut parts: Vec<String> = Vec::new();
            for (k, vv) in t.iter() {
                if k == "package" {
                    if let Some(s) = vv.as_str() {
                        parts.push(format!("package = \"{}\"", rename_package(s)));
                        continue;
                    }
                }
                parts.push(format!("{} = {}", k, render_toml_value(vv)));
            }
            format!("{{ {} }}", parts.join(", "))
        }
        other => render_toml_value(other),
    };
    Some(format!("{renamed_key} = {rendered_value}"))
}

fn render_toml_value(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => format!("\"{s}\""),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        toml::Value::Array(a) => {
            let items: Vec<String> = a.iter().map(render_toml_value).collect();
            format!("[{}]", items.join(", "))
        }
        toml::Value::Table(t) => {
            let parts: Vec<String> = t
                .iter()
                .map(|(k, vv)| format!("{} = {}", k, render_toml_value(vv)))
                .collect();
            format!("{{ {} }}", parts.join(", "))
        }
        toml::Value::Datetime(d) => format!("\"{d}\""),
    }
}
