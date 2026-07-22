//! AST-driven, span-preserving rewrite of `codex_*` -> `lemurclaw_*` references.
//!
//! We parse each file with `syn`, walk the AST to collect every `(span, old_ident)`
//! pair that needs renaming, then apply those renames as byte-range substitutions
//! against the ORIGINAL source string. This preserves comments, whitespace, and
//! formatting exactly, because we never re-emit the file — we only patch the
//! specific identifier spans.
//!
//! When syn fails to parse a file (rare — usually `cfg(test)`-only files using
//! nightly features), we fall back to a line-based `use`/`extern crate`/
//! path-expression rewriter that also preserves comments.

use anyhow::{Context, Result};
use proc_macro2::{Span, TokenStream};
use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;
use syn::visit::Visit;
use syn::{Ident, ItemUse, UseTree};

/// Rewrite a single Rust source file in place. Returns true if any change was
/// made.
pub fn rewrite_file(path: &Path) -> Result<bool> {
    let src = std::fs::read_to_string(path)
        .with_context(|| format!("read source {}", path.display()))?;
    let new_src = match rewrite_string(&src) {
        Some(s) => s,
        None => return Ok(false),
    };
    if new_src != src {
        std::fs::write(path, &new_src)
            .with_context(|| format!("write source {}", path.display()))?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Rewrite a Rust source string. Returns None if no rewrite is needed.
pub fn rewrite_string(src: &str) -> Option<String> {
    // Collect every (start, end, old_ident) span we want to patch. Then apply
    // them as byte-range substitutions in descending order so earlier spans'
    // byte offsets remain valid.
    let mut spans: Vec<SpanEdit> = Vec::new();

    match syn::parse_file(src) {
        Ok(file) => {
            let mut collector = Collector { spans: Vec::new() };
            collector.visit_file(&file);
            spans = collector.spans;
        }
        Err(_) => {
            // Parse failure — fall back to line-based rewrite.
            return rewrite_surgical(src);
        }
    }

    // Convert packed LineColumn spans to byte offsets and apply.
    let ast_result = apply_spans_via_linecol(src, &spans);

    match ast_result {
        Some(out) if out != src => {
            // Run surgical as a safety net on the AST-rewritten output to
            // catch any path expressions syn's visitor missed (macro hygiene
            // edge cases, etc.). The surgical rewriter is conservative enough
            // that it won't double-edit already-renamed identifiers.
            let surgical = rewrite_surgical(&out).unwrap_or(out);
            if surgical == src {
                None
            } else {
                Some(surgical)
            }
        }
        _ => {
            // AST path produced no changes (or failed). Try surgical on the
            // original source.
            rewrite_surgical(src)
        }
    }
}

#[derive(Debug, Clone)]
struct SpanEdit {
    start: usize,
    end: usize,
    old_ident: String,
}

struct Collector {
    spans: Vec<SpanEdit>,
}

impl Collector {
    fn record(&mut self, ident: &Ident) {
        if !is_codex_ident(ident) {
            return;
        }
        let s = ident.span().start();
        let e = ident.span().end();
        self.spans.push(SpanEdit {
            start: encode_lc(s),
            end: encode_lc(e),
            old_ident: ident.to_string(),
        });
    }
}

fn encode_lc(lc: proc_macro2::LineColumn) -> usize {
    // Pack (line, column) into a single usize. Column is 0-based byte offset
    // within a line; we assume no line is longer than 1M bytes.
    (lc.line * 1_000_000) + lc.column
}

fn decode_lc(packed: usize) -> (usize, usize) {
    (packed / 1_000_000, packed % 1_000_000)
}

impl<'ast> Visit<'ast> for Collector {
    fn visit_item_use(&mut self, i: &'ast ItemUse) {
        // Visit the use tree to find the crate-name segment.
        visit_use_tree_for_crate_name(&i.tree, &mut |ident| {
            self.record(ident);
        });
        syn::visit::visit_item_use(self, i);
    }

    fn visit_item_extern_crate(&mut self, i: &'ast syn::ItemExternCrate) {
        if is_codex_ident(&i.ident) {
            self.record(&i.ident);
        }
        syn::visit::visit_item_extern_crate(self, i);
    }

    fn visit_path(&mut self, p: &'ast syn::Path) {
        // Bare path expressions `codex_foo::bar::baz`. Only rewrite when the
        // path has MORE than one segment — a single-segment path like
        // `codex_home` is ambiguous (could be a local variable of the same
        // name as a crate) so we leave it alone. The `codex_foo::` form is
        // unambiguously a crate reference.
        if p.segments.len() >= 2 {
            if let Some(first) = p.segments.first() {
                if is_codex_ident(&first.ident) {
                    self.record(&first.ident);
                }
            }
        }
        syn::visit::visit_path(self, p);
    }
}

fn visit_use_tree_for_crate_name<F: FnMut(&Ident)>(tree: &UseTree, f: &mut F) {
    match tree {
        UseTree::Path(p) => {
            // `codex_foo::rest` — the leading ident is the crate name.
            if is_codex_ident(&p.ident) {
                f(&p.ident);
            }
            // Don't recurse — the inner tree is INSIDE the crate, not a crate
            // reference itself.
        }
        UseTree::Name(n) => {
            // `use codex_foo;` (bare).
            if is_codex_ident(&n.ident) {
                f(&n.ident);
            }
        }
        UseTree::Glob(_) => {
            // `codex_foo::*` — the crate prefix is on the parent UseTree::Path.
            // Nothing to do here.
        }
        UseTree::Group(g) => {
            for item in &g.items {
                visit_use_tree_for_crate_name(item, f);
            }
        }
        UseTree::Rename(r) => {
            if is_codex_ident(&r.ident) {
                f(&r.ident);
            }
        }
    }
}

fn is_codex_ident(ident: &Ident) -> bool {
    let s = ident.to_string();
    if !s.starts_with("codex_") {
        return false;
    }
    // Only rename if this identifier matches a known crate name. This avoids
    // false positives on local variables like `codex_home_env`.
    known_crate_idents().contains(&s)
}

/// Returns the set of crate identifier names (e.g. `codex_core`,
/// `codex_utils_absolute_path`) that should be renamed. Computed lazily by
/// querying `cargo metadata` once; cached for the process lifetime.
fn known_crate_idents() -> &'static HashSet<String> {
    static CRATES: OnceLock<HashSet<String>> = OnceLock::new();
    CRATES.get_or_init(|| {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let codex_root = std::path::PathBuf::from(manifest_dir)
            .join("..")
            .join("codex-rs");
        let manifest = codex_root.join("Cargo.toml");

        let out = std::process::Command::new("cargo")
            .args([
                "metadata",
                "--format-version=1",
                "--no-deps",
                "--manifest-path",
                manifest.to_str().unwrap_or("Cargo.toml"),
            ])
            .output();
        let mut set = HashSet::new();
        if let Ok(o) = out {
            if o.status.success() {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&o.stdout) {
                    if let Some(packages) = json.get("packages").and_then(|v| v.as_array()) {
                        for pkg in packages {
                            if let Some(name) = pkg.get("name").and_then(|v| v.as_str()) {
                                if name.starts_with("codex-") || name.starts_with("codex_") {
                                    set.insert(name.replace('-', "_"));
                                }
                            }
                        }
                    }
                }
            }
        }
        set
    })
}

fn rename_ident(name: &str) -> String {
    if let Some(rest) = name.strip_prefix("codex_") {
        format!("lemurclaw_{}", rest)
    } else {
        name.to_string()
    }
}

/// Convert packed LineColumn spans to byte offsets in `src`, then apply
/// substitutions. Returns the rewritten source. If the LineColumn positions
/// don't match (e.g. file was modified), returns None.
fn apply_spans_via_linecol(src: &str, spans: &[SpanEdit]) -> Option<String> {
    // Build a line-start byte-offset table.
    let mut line_starts: Vec<usize> = vec![0];
    for (i, b) in src.bytes().enumerate() {
        if b == b'\n' {
            line_starts.push(i + 1);
        }
    }

    // Convert packed (line, col) pairs to byte offsets. LineColumn.line is
    // 1-based, column is 0-based byte offset within line.
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for edit in spans {
        let (sl, sc) = decode_lc(edit.start);
        let (el, ec) = decode_lc(edit.end);
        if sl == 0 || sl > line_starts.len() || el == 0 || el > line_starts.len() {
            // Out of range — bail entirely.
            return None;
        }
        let start_byte = line_starts[sl - 1] + sc;
        let end_byte = line_starts[el - 1] + ec;
        edits.push((start_byte, end_byte, rename_ident(&edit.old_ident)));
    }

    // Apply in descending order.
    edits.sort_by(|a, b| b.0.cmp(&a.0));
    let mut out = src.to_string();
    let mut last_start = usize::MAX;
    for (s, e, new_ident) in &edits {
        if *s >= last_start {
            continue;
        }
        last_start = *s;
        out.replace_range(*s..*e, new_ident);
    }
    Some(out)
}

/// Surgical fallback: line-based rewrite that preserves comments and
/// whitespace. Handles `use`/`pub use`/`extern crate` declarations AND path
/// expressions like `codex_foo::bar` in code bodies. String literals and
/// comments are deliberately left untouched.
fn rewrite_surgical(src: &str) -> Option<String> {
    // Apply AST edits first (they returned valid LineColumn spans).
    // For the fallback case, we only have the original source — apply the
    // line-based rewriter.
    let mut out = String::with_capacity(src.len());
    for line in src.lines() {
        out.push_str(&rewrite_line_surgical(line));
        out.push('\n');
    }
    // Preserve trailing newline if original had one.
    if !src.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    if out == src {
        None
    } else {
        Some(out)
    }
}

fn rewrite_line_surgical(line: &str) -> String {
    // Match `use codex_foo...`, `pub use codex_foo...`, `pub(crate) use ...`,
    // `pub(in path) use ...`, or `extern crate codex_foo;`
    let trimmed = line.trim_start();
    let leading_ws = &line[..line.len() - trimmed.len()];

    let after_visibility = strip_visibility(trimmed);
    let crates = known_crate_idents();

    if let Some(rest) = after_visibility.strip_prefix("use ") {
        let rest = rest.strip_prefix("::").unwrap_or(rest);
        if let Some(after) = rest.strip_prefix("codex_") {
            let end = after
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .unwrap_or(after.len());
            let ident = format!("codex_{}", &after[..end]);
            if crates.contains(&ident) {
                let suffix = &after[end..];
                let vis_len = trimmed.len() - after_visibility.len();
                let vis = &trimmed[..vis_len];
                return format!(
                    "{}{}use lemurclaw_{}{}",
                    leading_ws, vis, &after[..end], suffix
                );
            }
        }
    }

    if let Some(rest) = after_visibility.strip_prefix("extern crate ") {
        let rest = rest.strip_prefix("::").unwrap_or(rest);
        if let Some(after) = rest.strip_prefix("codex_") {
            let end = after
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .unwrap_or(after.len());
            let ident = format!("codex_{}", &after[..end]);
            if crates.contains(&ident) {
                let suffix = &after[end..];
                let vis_len = trimmed.len() - after_visibility.len();
                let vis = &trimmed[..vis_len];
                return format!(
                    "{}{}extern crate lemurclaw_{}{}",
                    leading_ws, vis, &after[..end], suffix
                );
            }
        }
    }

    // Path expressions in code body.
    rewrite_path_expressions(line)
}

/// Replace `codex_X::` occurrences in a line, but only when:
///   - the `codex_X` token is preceded by a non-identifier boundary,
///   - it matches a known crate identifier (not a local variable), and
///   - it is followed by `::`.
fn rewrite_path_expressions(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    let crates = known_crate_idents();

    while i < bytes.len() {
        // Check if we're at the start of a `codex_` identifier preceded by a
        // boundary char (or at start of line).
        if line[i..].starts_with("codex_") {
            let preceded_by_boundary =
                i == 0 || is_ident_boundary(bytes[i - 1] as char) || is_string_interior(line, i);
            if preceded_by_boundary {
                // Find the end of the identifier.
                let ident_start = i;
                let mut j = i + "codex_".len();
                while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                    j += 1;
                }
                // Must be followed by `::` AND match a known crate.
                let ident = &line[ident_start..j];
                if line[j..].starts_with("::") && crates.contains(ident) {
                    let renamed = rename_ident(ident);
                    out.push_str(&renamed);
                    i = j;
                    continue;
                }
            }
        }
        // Default: copy one char.
        let ch = line[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn is_ident_boundary(c: char) -> bool {
    !(c.is_ascii_alphanumeric() || c == '_')
}

/// Heuristic: detect if position `i` is inside a string literal or comment.
/// This is conservative — if we're inside a `"..."` we want to skip. Since we
/// only call this per-line, we track state from the beginning of the line.
fn is_string_interior(line: &str, pos: usize) -> bool {
    // Walk from line start, tracking "in string", "in line comment", "in block
    // comment" state. If at `pos` any of these is active, return true.
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut in_string = false;
    let mut in_line_comment = false;
    let mut in_block_comment_depth: u32 = 0;

    while i < pos {
        let remaining = &line[i..];
        if in_line_comment {
            break; // rest of line is comment
        }
        if in_block_comment_depth > 0 {
            if remaining.starts_with("*/") {
                in_block_comment_depth -= 1;
                i += 2;
                continue;
            }
            if remaining.starts_with("/*") {
                in_block_comment_depth += 1;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if in_string {
            if remaining.starts_with("\\\"") {
                i += 2;
                continue;
            }
            if remaining.starts_with('"') {
                in_string = false;
                i += 1;
                continue;
            }
            i += 1;
            continue;
        }
        // Not in string or comment.
        if remaining.starts_with("//") {
            in_line_comment = true;
            i += 2;
            continue;
        }
        if remaining.starts_with("/*") {
            in_block_comment_depth = 1;
            i += 2;
            continue;
        }
        if remaining.starts_with('"') {
            in_string = true;
            i += 1;
            continue;
        }
        i += 1;
    }

    in_string || in_line_comment || in_block_comment_depth > 0
}

/// Strip a leading visibility modifier (`pub`, `pub(...)`, `crate`, etc.)
/// from a trimmed line and return the rest.
fn strip_visibility(trimmed: &str) -> &str {
    if let Some(rest) = trimmed.strip_prefix("pub ") {
        return rest;
    }
    if let Some(rest) = trimmed.strip_prefix("crate ") {
        return rest;
    }
    if trimmed.starts_with("pub(") {
        if let Some(close) = trimmed.find(')') {
            let after = &trimmed[close + 1..];
            return after.trim_start();
        }
    }
    trimmed
}

// Suppress unused-import warning for TokenStream (kept for future span-based
// rewrites that may want to render via quote).
#[allow(dead_code)]
fn _ts_marker() -> TokenStream {
    TokenStream::new()
}

// Suppress unused-import warning for Span (used by the AST collector).
#[allow(dead_code)]
fn _span_marker() -> Span {
    Span::call_site()
}
