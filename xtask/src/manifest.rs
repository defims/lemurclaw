//! Cargo.toml parsing and rewriting helpers.
//!
//! We operate on raw TOML documents via `toml_edit` semantics would be ideal,
//! but to keep the dependency surface small we use the `toml` crate for
//! structured reads and targeted string rewrites for the parts that need to
//! preserve layout. In practice the crate manifests are machine-generated
//! enough that round-tripping through `toml::to_string` is acceptable for the
//! publish workspace (comments are rare in generated manifests).

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Parsed view of a single crate's Cargo.toml relevant for renaming.
#[derive(Debug, Clone)]
pub struct CrateManifest {
    pub path: PathBuf,
    pub raw: String,
    pub doc: toml::Value,
}

impl CrateManifest {
    pub fn read(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("read manifest {}", path.display()))?;
        let doc: toml::Value = toml::from_str(&raw)
            .with_context(|| format!("parse manifest {}", path.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
            raw,
            doc,
        })
    }

    /// The package name, e.g. `codex-core`.
    pub fn package_name(&self) -> Result<&str> {
        self.doc
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|v| v.as_str())
            .context("missing package.name")
    }

    pub fn version(&self) -> Result<&str> {
        self.doc
            .get("package")
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str())
            .context("missing package.version")
    }

    /// True if this is a proc-macro crate (language rule: must be standalone).
    pub fn is_proc_macro(&self) -> bool {
        self.doc
            .get("lib")
            .and_then(|l| l.get("proc-macro"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// `publish = false` short-circuits publishing.
    pub fn is_unpublishable(&self) -> bool {
        self.doc
            .get("package")
            .and_then(|p| p.get("publish"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Names of all workspace deps (in [dependencies] / [dev-dependencies] /
    /// [build-dependencies]) that match the `codex-` prefix and therefore need
    /// renaming in the publish workspace.
    pub fn workspace_codex_deps(&self) -> Vec<String> {
        let mut out = Vec::new();
        for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(table) = self.doc.get(section).and_then(|v| v.as_table()) {
                for key in table.keys() {
                    if key.starts_with("codex-") {
                        out.push(key.clone());
                    }
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }
}

/// Workspace-root manifest view. Captures the bits we need to clone into the
/// publish workspace: `[workspace].members`, `[workspace.dependencies]`, and
/// `[profile.*]` tables. `[patch.crates-io]` is intentionally NOT carried over
/// (Phase 0 decides separately whether fork crates need publishing).
#[derive(Debug, Clone)]
pub struct WorkspaceManifest {
    pub path: PathBuf,
    pub raw: String,
    pub doc: toml::Value,
}

impl WorkspaceManifest {
    pub fn read(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("read workspace manifest {}", path.display()))?;
        let doc: toml::Value = toml::from_str(&raw)
            .with_context(|| format!("parse workspace manifest {}", path.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
            raw,
            doc,
        })
    }

    pub fn members(&self) -> Result<Vec<String>> {
        let members = self
            .doc
            .get("workspace")
            .and_then(|w| w.get("members"))
            .and_then(|v| v.as_array())
            .context("workspace.members missing")?;
        Ok(members
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect())
    }

    /// `[workspace.dependencies]` table as (name, value) pairs.
    pub fn workspace_deps(&self) -> Result<BTreeMap<String, toml::Value>> {
        let mut out = BTreeMap::new();
        if let Some(table) = self
            .doc
            .get("workspace")
            .and_then(|w| w.get("dependencies"))
            .and_then(|v| v.as_table())
        {
            for (k, v) in table.iter() {
                out.insert(k.clone(), v.clone());
            }
        }
        Ok(out)
    }
}

/// Rename a package name: `codex-foo` -> `lemurclaw-foo`. Crates without the
/// prefix pass through unchanged (e.g. workspace-internal helpers like
/// `core_test_support` are handled separately by the caller).
pub fn rename_package(name: &str) -> String {
    if let Some(rest) = name.strip_prefix("codex-") {
        format!("lemurclaw-{}", rest)
    } else {
        name.to_string()
    }
}

/// Rename the Rust identifier form: `codex_foo` -> `lemurclaw_foo`. Used when
/// rewriting `use codex_foo::...` imports.
pub fn rename_ident(name: &str) -> String {
    if let Some(rest) = name.strip_prefix("codex_") {
        format!("lemurclaw_{}", rest)
    } else {
        name.to_string()
    }
}

/// Rewrite a Cargo.toml body for the publish workspace:
///   - `package.name` codex-X -> lemurclaw-X
///   - dependency keys codex-X -> lemurclaw-X
///   - drop `[[bin]]` arrays when `drop_bins` is true
///   - drop dependency lines (in any of `[dependencies]` /
///     `[dev-dependencies]` / `[build-dependencies]`) whose key matches an
///     entry in `drop_deps` — used to elide references to crates that are
///     excluded from the publish workspace (test_support helpers, etc.).
///
/// We work on the raw string because generated manifests are regular enough
/// that targeted line-based rewrites are safer than a full toml round-trip
/// (which would flatten any inline tables and lose formatting).
pub fn rewrite_crate_manifest(
    raw: &str,
    drop_bins: bool,
    drop_deps: &[String],
) -> Result<String> {
    // Parse to validate, then do line-based rewrite to preserve layout.
    let _doc: toml::Value = toml::from_str(raw).context("parse crate manifest")?;

    let mut out = String::with_capacity(raw.len());
    let mut in_bin_section = false;
    let mut in_drop_dep_subtable = false;

    for line in raw.lines() {
        let trimmed = line.trim_start();

        // Leaving any sub-table on the next header.
        if trimmed.starts_with('[') {
            in_bin_section = false;
            in_drop_dep_subtable = false;
        }

        // Detect [[bin]] section boundaries.
        if trimmed.starts_with("[[bin]]") {
            if drop_bins {
                in_bin_section = true;
                continue;
            }
        } else if in_bin_section {
            // Handled above — skip.
            continue;
        }

        if in_bin_section {
            continue;
        }

        // Detect drop-dep sub-table headers like `[dependencies.core_test_support]`.
        if is_drop_dep_subtable_header(trimmed, drop_deps) {
            in_drop_dep_subtable = true;
            continue;
        }
        if in_drop_dep_subtable {
            // Skip every line inside the dropped sub-table until next header.
            continue;
        }

        // Drop inline dependency lines that reference excluded crates.
        if line_matches_drop_dep(trimmed, drop_deps) {
            continue;
        }

        let rewritten = rewrite_manifest_line(line);
        out.push_str(&rewritten);
        out.push('\n');
    }

    Ok(out)
}

fn line_matches_drop_dep(trimmed: &str, drop_deps: &[String]) -> bool {
    if drop_deps.is_empty() {
        return false;
    }

    // Inline form: `<ws>dep_name = ...`
    for dep in drop_deps {
        if let Some(rest) = trimmed.strip_prefix(dep.as_str()) {
            let after = rest.trim_start();
            if after.starts_with('=') {
                return true;
            }
        }
        if let Some(eq_idx) = trimmed.find('=') {
            let key = trimmed[..eq_idx].trim_end();
            if key == dep.as_str() {
                return true;
            }
        }
    }

    // Sub-table form: `[dependencies.dep_name]` or `[dev-dependencies.dep_name]`.
    // For these we mark the header line AND all following lines until the next
    // header. The caller handles this by tracking section state.
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        for dep in drop_deps {
            let needle = format!(".{}]", dep);
            if trimmed.contains(&needle) {
                return true;
            }
        }
    }

    false
}

/// Returns true if `trimmed` is a sub-table header for one of `drop_deps`,
/// e.g. `[dependencies.core_test_support]`.
fn is_drop_dep_subtable_header(trimmed: &str, drop_deps: &[String]) -> bool {
    if !(trimmed.starts_with('[') && trimmed.ends_with(']')) {
        return false;
    }
    for dep in drop_deps {
        let needle = format!(".{}]", dep);
        if trimmed.contains(&needle) {
            return true;
        }
    }
    false
}

fn rewrite_manifest_line(line: &str) -> String {
    // Sub-table headers like `[dependencies.codex-foo]` — rename the trailing
    // segment. Must be checked before the generic string-rewrite below because
    // headers don't contain quoted strings.
    let trimmed = line.trim_start();
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        if let Some(dot_idx) = trimmed.rfind('.') {
            let after_dot = &trimmed[dot_idx + 1..trimmed.len() - 1];
            if after_dot.starts_with("codex-") {
                let renamed = rename_package(after_dot);
                let indent = &line[..line.len() - line.trim_start().len()];
                let prefix = &trimmed[..dot_idx + 1];
                return format!("{}{}{}]", indent, prefix, renamed);
            }
        }
        // Other headers pass through unchanged.
        return line.to_string();
    }

    // Bare-key dependency lines: `codex-foo = { ... }` or `codex-foo = "1.0"`.
    if let Some(idx) = find_codex_key(line) {
        let (prefix, key, suffix) = split_at_key(line, idx);
        let renamed = rename_package(&key);
        // Then continue rewriting any inline `"codex-..."` string literals in
        // the suffix (e.g. `package = "codex-..."` inside `{ ... }`).
        let suffix_rewritten = rewrite_codex_strings_in_line(&suffix);
        return format!("{}{}{}", prefix, renamed, suffix_rewritten);
    }

    // Any other line: rewrite any inline `"codex-..."` string literals. This
    // covers `name = "codex-foo"`, `package = "codex-foo"` (inline table or
    // sub-table value), and `[lib] name = "codex_foo"` (handled below).
    rewrite_codex_strings_in_line(line)
}

/// Replace every occurrence of `"codex-..."` (a quoted package name) with its
/// renamed form. Only handles the hyphenated form — underscore form
/// (`"codex_foo"`) is reserved for lib names which are handled by
/// `rewrite_lib_name_line` to avoid touching unrelated string constants.
fn rewrite_codex_strings_in_line(line: &str) -> String {
    // Special-case `[lib] name = "codex_X"` (underscore form): the lib name
    // is the Rust identifier the crate is referenced by externally. Renaming
    // it requires corresponding changes to `use codex_X::...` in downstream
    // crates — which our source_rewrite module does. So we DO rename lib
    // names to keep everything consistent.
    let trimmed = line.trim_start();
    if trimmed.starts_with("name = ") {
        if let Some(rest) = trimmed
            .strip_prefix("name = ")
            .and_then(|s| s.trim().strip_prefix('"'))
            .and_then(|s| s.strip_suffix('"'))
        {
            if rest.starts_with("codex_") {
                let renamed = rename_ident(rest);
                let indent = &line[..line.len() - trimmed.len()];
                return format!("{}name = \"{}\"", indent, renamed);
            }
        }
    }

    // Fast path for hyphenated package names: byte-level scan.
    let mut out = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            if let Some(end_rel) = line[i + 1..].find('"') {
                let end = i + 1 + end_rel;
                let inner = &line[i + 1..end];
                if inner.starts_with("codex-") {
                    let renamed = rename_package(inner);
                    out.push('"');
                    out.push_str(&renamed);
                    out.push('"');
                    i = end + 1;
                    continue;
                }
                out.push_str(&line[i..=end]);
                i = end + 1;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    // The byte-level push above corrupts multi-byte UTF-8. Verify and fall
    // back to a UTF-8-safe path if needed.
    if out.len() == line.len() {
        return out;
    }
    rewrite_codex_strings_in_line_safe(line)
}

fn rewrite_codex_strings_in_line_safe(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '"' {
            // Find the closing quote by scanning forward in the source slice.
            if let Some(end_rel) = line[i + 1..].find('"') {
                let end = i + 1 + end_rel;
                let inner = &line[i + 1..end];
                if inner.starts_with("codex-") {
                    let renamed = rename_package(inner);
                    out.push('"');
                    out.push_str(&renamed);
                    out.push('"');
                    // Advance chars past the closing quote.
                    while let Some(&(j, _)) = chars.peek() {
                        if j > end {
                            break;
                        }
                        chars.next();
                    }
                    continue;
                }
                out.push_str(&line[i..=end]);
                while let Some(&(j, _)) = chars.peek() {
                    if j > end {
                        break;
                    }
                    chars.next();
                }
                continue;
            }
        }
        out.push(c);
    }
    out
}

/// If this line starts with `<ws>codex-foo<ws>=`, return the byte index where
/// the key begins.
fn find_codex_key(line: &str) -> Option<usize> {
    let trimmed_start = line.len() - line.trim_start().len();
    let rest = &line[trimmed_start..];
    if !rest.starts_with("codex-") {
        return None;
    }
    // Find the end of the bare key (Rust TOML bare keys: A-Za-z0-9_-).
    let key_end = rest
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
        .unwrap_or(rest.len());
    let after = &rest[key_end..];
    // Must be followed by whitespace then `=`.
    let after_trimmed = after.trim_start();
    if after_trimmed.starts_with('=') {
        Some(trimmed_start)
    } else {
        None
    }
}

fn split_at_key(line: &str, key_start: usize) -> (String, String, String) {
    let prefix = line[..key_start].to_string();
    let rest = &line[key_start..];
    let key_end = rest
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
        .unwrap_or(rest.len());
    let key = rest[..key_end].to_string();
    let suffix = rest[key_end..].to_string();
    (prefix, key, suffix)
}

/// Build the publish workspace Cargo.toml from the source workspace manifest.
///
/// - members: filtered to `keep_members` (the publishable crates' rel_dirs);
///   directory layout preserved
/// - workspace.dependencies: codex-* keys renamed to lemurclaw-*; entries
///   for excluded crates are dropped; path values keep their original rel_dir
///   (directory layout is preserved)
/// - profiles: copied verbatim
/// - `[patch.crates-io]`: intentionally dropped (Phase 0 decides fork policy
///   separately)
pub fn rewrite_workspace_manifest(
    src: &WorkspaceManifest,
    keep_members: &[String],
) -> Result<String> {
    let doc = &src.doc;
    let mut out = String::new();

    out.push_str("[workspace]\n");
    out.push_str("resolver = \"2\"\n");

    // members: keep only the publishable rel_dirs, preserving source order.
    out.push_str("\nmembers = [\n");
    let src_members = src.members()?;
    let keep_set: std::collections::HashSet<&str> =
        keep_members.iter().map(|s| s.as_str()).collect();
    for m in &src_members {
        if keep_set.contains(m.as_str()) {
            out.push_str(&format!("    \"{}\",\n", m));
        }
    }
    out.push_str("]\n");

    // workspace.package: copy verbatim (edition/license/version inheritance).
    if let Some(pkg) = doc.get("workspace").and_then(|w| w.get("package")) {
        out.push_str("\n[workspace.package]\n");
        if let Some(table) = pkg.as_table() {
            for (k, v) in table.iter() {
                out.push_str(&format!("{} = {}\n", k, render_toml_value(v)));
            }
        }
    }

    // workspace.lints: copy verbatim (clippy/rust lint config inheritance).
    if let Some(lints) = doc.get("workspace").and_then(|w| w.get("lints")) {
        if let Some(table) = lints.as_table() {
            out.push_str("\n[workspace.lints]\n");
            for (k, v) in table.iter() {
                if k == "clippy" {
                    continue; // emitted separately below
                }
                out.push_str(&format!("{} = {}\n", k, render_toml_value(v)));
            }
            if let Some(clippy) = table.get("clippy").and_then(|v| v.as_table()) {
                out.push_str("\n[workspace.lints.clippy]\n");
                for (k, v) in clippy.iter() {
                    out.push_str(&format!("{} = {}\n", k, render_toml_value(v)));
                }
            }
        }
    }

    // workspace.dependencies: rename codex-* keys, drop entries for crates
    // that are not in keep_members (those packages don't exist in publish/).
    let deps = src.workspace_deps()?;
    if !deps.is_empty() {
        out.push_str("\n[workspace.dependencies]\n");
        for (k, v) in &deps {
            // Path deps point at member dirs; if the dir isn't in keep_members,
            // skip this entry (it's for an excluded crate).
            if let Some(table) = v.as_table() {
                if let Some(path) = table.get("path").and_then(|p| p.as_str()) {
                    let rel = path.strip_prefix("./").unwrap_or(path);
                    if !keep_set.contains(rel) {
                        continue;
                    }
                }
            }
            let key = if k.starts_with("codex-") {
                rename_package(k)
            } else {
                k.clone()
            };
            // For internal path deps that don't already specify a version, add
            // `version = "0.0.0"` (the workspace.package.version). This is
            // required for `cargo publish`: cargo strips `path` from the
            // published manifest and relies on the version to resolve the dep
            // from crates.io. Without this, `cargo package` errors out with
            // "dependency X does not specify a version".
            let v_with_version = ensure_version_for_internal_path(v, &key);
            let value_str = render_dep_value(&v_with_version, k);
            out.push_str(&format!("{} = {}\n", key, value_str));
        }
    }

    // profiles (preserve release/ci-test/etc.)
    if let Some(profiles) = doc.get("profile").and_then(|v| v.as_table()) {
        for (profile_name, profile_val) in profiles.iter() {
            out.push_str(&format!("\n[profile.{}]\n", profile_name));
            if let Some(table) = profile_val.as_table() {
                for (k, v) in table.iter() {
                    out.push_str(&format!("{} = {}\n", k, render_toml_value(v)));
                }
            }
        }
    }

    // [patch.crates-io] and [patch."<url>"] sections — preserved verbatim. The
    // forks (ratatui / crossterm / tokio-tungstenite / tungstenite) are hard
    // compile dependencies (see Phase 0 `verify patches`); the publish
    // workspace still needs them to build. They don't block `cargo publish`
    // (cargo strips [patch] from published manifests), but they do block
    // `cargo check` until the forks are also published to crates.io or
    // re-pointed at published `lemurclaw-*` versions.
    //
    // toml parses `[patch.crates-io]` as doc["patch"]["crates-io"] (nested
    // table). Iterate that way so we catch both crates-io and URL-keyed
    // patches.
    if let Some(patch_table) = doc.get("patch").and_then(|v| v.as_table()) {
        for (target, entries) in patch_table.iter() {
            // `crates-io` is a bare identifier; URLs must be quoted.
            let target_rendered = if target
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                target.to_string()
            } else {
                format!("\"{}\"", target)
            };
            out.push_str(&format!("\n[patch.{}]\n", target_rendered));
            if let Some(table) = entries.as_table() {
                for (k, v) in table.iter() {
                    out.push_str(&format!("{} = {}\n", k, render_toml_value(v)));
                }
            }
        }
    }

    Ok(out)
}

/// For deps that are internal (path-only, no version), add `version = "0.0.0"`
/// so `cargo publish` can resolve them after stripping the `path` field. The
/// version matches `[workspace.package]` version (currently 0.0.0 for all
/// lemurclaw-* crates).
///
/// External deps (with version already specified) pass through unchanged.
fn ensure_version_for_internal_path(v: &toml::Value, renamed_key: &str) -> toml::Value {
    // Only add version for our own crates.
    if !renamed_key.starts_with("lemurclaw-") {
        return v.clone();
    }
    match v {
        toml::Value::Table(t) => {
            let mut new_t = t.clone();
            if !new_t.contains_key("version") {
                new_t.insert(
                    "version".to_string(),
                    toml::Value::String("0.0.0".to_string()),
                );
            }
            toml::Value::Table(new_t)
        }
        // String form (e.g. `foo = "1.0"`) — not internal-path form, leave alone.
        _ => v.clone(),
    }
}

fn render_dep_value(v: &toml::Value, original_key: &str) -> String {
    // For path deps pointing at a codex-* workspace member, the path stays the
    // same (directory layout is preserved); only the package name changes.
    // Workspace inheritance stays as-is: `{ workspace = true }`.
    let rendered = match v {
        toml::Value::Table(t) => {
            let mut parts: Vec<String> = Vec::new();
            for (k, vv) in t.iter() {
                if k == "package" {
                    // Override package alias: if it references codex-, rename it.
                    if let Some(s) = vv.as_str() {
                        let renamed = rename_package(s);
                        parts.push(format!("package = \"{}\"", renamed));
                        continue;
                    }
                }
                parts.push(format!("{} = {}", k, render_toml_value(vv)));
            }
            format!("{{ {} }}", parts.join(", "))
        }
        _ => render_toml_value(v),
    };
    // original_key is currently unused; kept for future logic that may need to
    // rename path targets based on the source dependency name.
    let _ = original_key;
    rendered
}

fn render_toml_value(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => format!("\"{}\"", s),
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
        toml::Value::Datetime(d) => format!("\"{}\"", d),
    }
}

/// Phase 1.5: rewrite the publish workspace Cargo.toml to drop the
/// `[patch.crates-io]` / `[patch."<url>"]` sections and add
/// `package = "lemurclaw-X"` aliases to the 4 fork deps.
///
/// `cargo publish` strips `[patch]` from published manifests — they only work
/// in the source workspace. So we replace the patches with direct dep aliases
/// pointing at the published `lemurclaw-*` fork crates.
pub fn rewrite_publish_workspace_for_forks(raw: &str) -> Result<String> {
    // The 4 fork dep keys → (lemurclaw package name, marker for the dep line).
    // We rewrite any workspace.dependencies line whose key matches.
    const FORK_ALIASES: &[(&str, &str)] = &[
        ("ratatui", "lemurclaw-ratatui"),
        ("crossterm", "lemurclaw-crossterm"),
        ("tokio-tungstenite", "lemurclaw-tokio-tungstenite"),
        ("tungstenite", "lemurclaw-tungstenite"),
    ];

    let mut out = String::with_capacity(raw.len());
    let mut in_patch_section = false;
    let mut in_workspace_deps = false;

    for line in raw.lines() {
        let trimmed = line.trim_start();

        // Detect entering a new section.
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_patch_section = trimmed.starts_with("[patch.");
            in_workspace_deps = trimmed == "[workspace.dependencies]";

            // Skip the [patch.*] header line entirely.
            if in_patch_section {
                continue;
            }

            out.push_str(line);
            out.push('\n');
            continue;
        }

        // Skip all body lines of [patch.*] sections.
        if in_patch_section {
            continue;
        }

        // In [workspace.dependencies], rewrite the 4 fork dep lines.
        if in_workspace_deps {
            if let Some(rewritten) = rewrite_workspace_fork_dep_line(line, FORK_ALIASES) {
                out.push_str(&rewritten);
                out.push('\n');
                continue;
            }
        }

        out.push_str(line);
        out.push('\n');
    }

    Ok(out)
}

/// If `line` is a workspace.dependencies entry for one of the fork deps,
/// rewrite it to include `package = "lemurclaw-X"`. Otherwise return None.
fn rewrite_workspace_fork_dep_line(
    line: &str,
    aliases: &[(&str, &str)],
) -> Option<String> {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];

    for (dep_key, lemurclaw_name) in aliases {
        // Two forms to handle:
        //   (1) `ratatui = "0.29.0"`
        //   (2) `tokio-tungstenite = { version = "...", features = [...] }`
        let bare_prefix = format!("{} = \"", dep_key);
        let inline_prefix = format!("{} = {{", dep_key);

        if trimmed.starts_with(&bare_prefix) {
            // Form (1): bare version string. Convert to inline table.
            // Extract the version from `ratatui = "0.29.0"`.
            let after_eq = trimmed
                .strip_prefix(&format!("{} = ", dep_key))?
                .trim();
            let version = after_eq.trim_matches('"');
            return Some(format!(
                "{}{} = {{ version = \"{}\", package = \"{}\" }}",
                indent, dep_key, version, lemurclaw_name
            ));
        }

        if trimmed.starts_with(&inline_prefix) {
            // Form (2): inline table. Inject `package = "..."` if not present.
            if trimmed.contains("package =") {
                return Some(line.to_string());
            }
            // Insert before the closing `}`.
            let close_idx = trimmed.rfind('}')?;
            let mut before = trimmed[..close_idx].trim_end().trim_end_matches(',').to_string();
            if !before.ends_with('{') {
                before.push_str(", ");
            }
            return Some(format!(
                "{}{}package = \"{}\" }}",
                indent, before, lemurclaw_name
            ));
        }
    }
    None
}
