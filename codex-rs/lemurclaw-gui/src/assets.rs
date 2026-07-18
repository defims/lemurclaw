//! Embedded React build + wry custom-protocol handler.
//!
//! `assets/dist/` (the Vite build output of the React frontend) is baked into
//! the binary at compile time via `include_dir!`, so the binary is
//! self-contained — no on-disk `dist/` required at runtime.
//!
//! The wry webview loads `lemurclaw://app/index.html`; all relative asset
//! URLs in that HTML (`./assets/index-*.js`) resolve to the same custom
//! protocol and are served from the embedded `Dir`. This is the standard
//! wry/Tauri pattern and avoids the macOS WKWebView restriction that blocks
//! `file://`-loaded pages (origin `null`) from fetching ES-module subresources.
//!
//! Why not `file://` + `allowingReadAccessToURL`: wry 0.54 does not expose
//! that WKWebView option as a builder method, and even if it did, `file://`
//! would still pin us to a baked absolute path and require shipping `dist/`
//! alongside the binary. A custom protocol solves both problems at once.

use std::borrow::Cow;

use include_dir::{Dir, include_dir};
use wry::http::{HeaderMap, HeaderValue, Request, Response, StatusCode};
use wry::WebViewId;

/// The Vite-built React frontend, embedded at compile time.
///
/// `$CARGO_MANIFEST_DIR` is understood by the `include_dir!` macro and points
/// at `lemurclaw-gui/`. If `assets/dist/` doesn't exist at compile time the
/// build fails with a clear error — that's intentional: a release build with
/// missing frontend assets should never silently produce a blank UI.
static DIST: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/assets/dist");

/// Scheme + host the protocol serves under. URLs look like
/// `lemurclaw://app/index.html`. The host segment (`app`) is required by the
/// URL grammar but otherwise arbitrary; the handler ignores it and serves
/// purely on path.
pub const PROTOCOL_SCHEME: &str = "lemurclaw";
pub const PROTOCOL_HOST: &str = "app";

/// Entry URL the webview loads.
pub fn entry_url() -> String {
    format!("{PROTOCOL_SCHEME}://{PROTOCOL_HOST}/index.html")
}

/// Wry custom-protocol handler. Called for every request under
/// `lemurclaw://app/...`. Looks up the path in the embedded `DIST`, returns
/// the bytes + a best-effort content-type, or a 404.
pub fn handle(_id: WebViewId, request: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let path = request.uri().path();
    serve_path(path)
}

/// Resolve a URL path (e.g. `/index.html`, `/assets/index-abc.js`) to an
/// embedded file. Returns `404 text/plain` for unknown paths so the JS
/// console shows a meaningful error instead of a wry default.
fn serve_path(path: &str) -> Response<Cow<'static, [u8]>> {
    // Strip leading `/`; treat empty as index.
    let rel = path.trim_start_matches('/');
    let rel = if rel.is_empty() { "index.html" } else { rel };

    match DIST.get_file(rel) {
        Some(file) => {
            let content_type = content_type_for(rel);
            let mut headers = HeaderMap::new();
            headers.insert(
                wry::http::header::CONTENT_TYPE,
                HeaderValue::from_str(content_type)
                    .unwrap_or(HeaderValue::from_static("application/octet-stream")),
            );
            // Embed asset bytes as a static borrow to avoid a copy — the Dir
            // lives for the program lifetime.
            let body = Cow::Borrowed(file.contents());
            let mut resp = Response::new(body);
            *resp.headers_mut() = headers;
            resp
        }
        None => {
            let body: Cow<'static, [u8]> = Cow::Borrowed(b"not found".as_slice());
            let mut resp = Response::new(body);
            *resp.status_mut() = StatusCode::NOT_FOUND;
            resp.headers_mut().insert(
                wry::http::header::CONTENT_TYPE,
                HeaderValue::from_static("text/plain"),
            );
            resp
        }
    }
}

/// Best-effort content-type from file extension. Covers everything Vite
/// typically emits; unknown extensions fall through to octet-stream.
fn content_type_for(path: &str) -> &'static str {
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
