//! wry custom-protocol adapter around `lemurclaw_webui::assets`.
//!
//! The embedded React `dist/`, asset path resolution, and content-type map
//! live in the leaf `lemurclaw_webui` crate so both gui (wry) and the webui
//! server can share them. This module is the thin wry-only glue: it takes a
//! `wry::http::Request`, delegates path resolution to
//! `lemurclaw_webui::assets::serve_path`, and wraps the result in the
//! `wry::http::Response` shape wry's custom-protocol handler expects.

use std::borrow::Cow;

use lemurclaw_webui::assets as shared;
use wry::WebViewId;
use wry::http::Request;
use wry::http::Response;
use wry::http::StatusCode;

pub use shared::PROTOCOL_SCHEME;
pub use shared::entry_url;

/// Wry custom-protocol handler. Called for every request under
/// `lemurclaw://app/...`. Delegates path resolution to the shared
/// `lemurclaw_webui::assets::serve_path` and wraps the result as a wry
/// response (200 + content-type for hits, 404 text/plain for misses).
pub fn handle(_id: WebViewId, request: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let (status, content_type, body) = shared::serve_path(request.uri().path());
    let body: Cow<'static, [u8]> = match body {
        Some(b) => Cow::Borrowed(b),
        // Small static message; matches the pre-refactor behavior.
        None => Cow::Borrowed(b"not found".as_slice()),
    };
    let mut resp = Response::new(body);
    *resp.status_mut() = StatusCode::from_u16(status).unwrap_or(StatusCode::NOT_FOUND);
    resp.headers_mut().insert(
        wry::http::header::CONTENT_TYPE,
        wry::http::HeaderValue::from_str(content_type)
            .unwrap_or_else(|_| wry::http::HeaderValue::from_static("application/octet-stream")),
    );
    resp
}
