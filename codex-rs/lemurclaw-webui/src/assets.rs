//! Embedded React build + crate-agnostic serving helpers.
//!
//! `assets/dist/` (the Vite build output of the shared React frontend) is
//! baked into the binary at compile time via `include_dir!`, so the binary is
//! self-contained — no on-disk `dist/` required at runtime.
//!
//! This module is deliberately free of any web-server or webview types: it
//! returns plain `(status_code, content_type, body)` tuples so both
//! `lemurclaw-gui` (wry custom-protocol handler) and the in-process webui
//! server (Stage 2 axum handlers) can adapt the same embedded assets to their
//! own response types without this crate having to depend on either wry or
//! axum. The two adapters live in their respective crates.
//!
//! Build-time requirement: `assets/dist/` must exist. `build.rs` runs
//! `npm install && npm run build` to produce it; if Node is unavailable it
//! emits a `cargo:warning` and lets compilation continue, but `include_dir!`
//! then fails loudly — a release build with missing frontend assets should
//! never silently produce a blank UI.

use include_dir::Dir;
use include_dir::include_dir;

/// The Vite-built React frontend, embedded at compile time.
///
/// `$CARGO_MANIFEST_DIR` is understood by the `include_dir!` macro and points
/// at `lemurclaw-webui/`. If `assets/dist/` doesn't exist at compile time the
/// build fails with a clear error — that's intentional.
static DIST: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/assets/dist");

/// Scheme + host the wry custom protocol serves under. URLs look like
/// `lemurclaw://app/index.html`. The host segment (`app`) is required by the
/// URL grammar but otherwise arbitrary; the wry handler ignores it and serves
/// purely on path. Re-exported by `lemurclaw-gui` for its `entry_url`.
pub const PROTOCOL_SCHEME: &str = "lemurclaw";
pub const PROTOCOL_HOST: &str = "app";

/// Entry URL the wry webview loads. Re-exported by `lemurclaw-gui`.
pub fn entry_url() -> String {
    format!("{PROTOCOL_SCHEME}://{PROTOCOL_HOST}/index.html")
}

/// Look up an embedded file by path relative to `dist/` (e.g. `"index.html"`,
/// `"assets/index-abc.js"`). Returns the raw bytes as a static borrow (the
/// `Dir` lives for the program lifetime, so no copy is needed).
pub fn dist_file(rel_path: &str) -> Option<&'static [u8]> {
    DIST.get_file(rel_path).map(|f| f.contents())
}

/// Enumerate files in an embedded subdirectory of `dist/` (e.g. `"assets"`),
/// returning `(filename, bytes)` pairs. Files are listed non-recursively and
/// exclude subdirectories. Returns an empty vec if the subdir doesn't exist.
///
/// Exposed so the integration smoke test can assert bundle sizes without the
/// `include_dir::Dir` type leaking into the crate's public API.
pub fn dist_subdir_files(rel_dir: &str) -> Vec<(&'static str, &'static [u8])> {
    match DIST.get_dir(rel_dir) {
        Some(dir) => dir
            .files()
            .filter_map(|f| {
                let name = f.path().file_name()?.to_str()?;
                Some((name, f.contents()))
            })
            .collect(),
        None => Vec::new(),
    }
}

/// Resolve a URL path (e.g. `/index.html`, `/assets/index-abc.js`) to an
/// embedded file, returning a crate-agnostic `(status, content_type, body)`
/// tuple. `body` is `None` for 404 so the caller can substitute its own
/// not-found body if it wants.
///
/// Both the wry handler (`lemurclaw-gui::assets::handle`) and the axum handler
/// (Stage 2 webui server) call this; each wraps the result into its own
/// response type.
pub fn serve_path(path: &str) -> (u16, &'static str, Option<&'static [u8]>) {
    // Strip leading `/`; treat empty as index.
    let rel = path.trim_start_matches('/');
    let rel = if rel.is_empty() { "index.html" } else { rel };

    match dist_file(rel) {
        Some(bytes) => (200, content_type_for(rel), Some(bytes)),
        None => (404, "text/plain", None),
    }
}

/// Best-effort content-type from file extension. Covers everything Vite
/// typically emits; unknown extensions fall through to octet-stream.
pub fn content_type_for(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("wasm") => "application/wasm",
        Some("map") => "application/json; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dist_file_returns_html_for_index() {
        // The build.rs must have produced dist/index.html before this test
        // runs; if not, include_dir! would have failed at compile time, so we
        // can safely assert Some here.
        let bytes = dist_file("index.html").expect("dist/index.html must exist");
        let head = std::str::from_utf8(&bytes[..bytes.len().min(64)])
            .unwrap_or("")
            .trim_start();
        // vite emits lowercase `<!doctype html>`; accept either casing.
        let lower = head.to_ascii_lowercase();
        assert!(
            lower.starts_with("<!doctype") || lower.starts_with("<html"),
            "expected HTML for index.html, got: {head:?}"
        );
    }

    #[test]
    fn serve_path_returns_html_for_root() {
        let (status, content_type, body) = serve_path("/");
        assert_eq!(status, 200);
        assert_eq!(content_type, "text/html; charset=utf-8");
        assert!(body.is_some());
    }

    #[test]
    fn serve_path_returns_404_for_missing() {
        let (status, content_type, body) = serve_path("/does-not-exist.js");
        assert_eq!(status, 404);
        assert_eq!(content_type, "text/plain");
        assert!(body.is_none());
    }

    #[test]
    fn content_type_covers_common_extensions() {
        assert_eq!(content_type_for("index.html"), "text/html; charset=utf-8");
        assert_eq!(
            content_type_for("app.js"),
            "application/javascript; charset=utf-8"
        );
        assert_eq!(content_type_for("style.css"), "text/css; charset=utf-8");
        assert_eq!(content_type_for("logo.svg"), "image/svg+xml");
        assert_eq!(
            content_type_for("data.json"),
            "application/json; charset=utf-8"
        );
        assert_eq!(content_type_for("font.woff2"), "font/woff2");
        assert_eq!(content_type_for("unknown.xyz"), "application/octet-stream");
    }
}
