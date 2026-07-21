//! In-process webui server: HTTP for the embedded React app + a WebSocket
//! bridge to the in-process `AppServerClient`.
//!
//! Topology:
//! - `GET /` and `/anything` (except `/ws`, `/readyz`, `/healthz`) → embedded
//!   React dist from [`assets`].
//! - `GET /ws` → WebSocket upgrade. Each WS frame from the browser is one
//!   JSON message (a `ClientRequest` or a `__resolve`/`__reject` envelope);
//!   each codex `ServerEvent` is serialized to JSON and sent back as one WS
//!   text frame.
//! - `GET /readyz` / `GET /healthz` → 200 OK (matches codex app-server
//!   convention; useful for healthchecks).
//!
//! Browser ↔ codex bridge reuses [`crate::codex_glue`] for all JSON-RPC
//! shaping — same wire contract as the wry IPC path in `lemurclaw-gui`, so the
//! React app's `transport-ws.ts` and `transport.ts` produce identical
//! envelopes.
//!
//! Connection model (Stage 2 scope): a single shared `InProcessAppServerClient`
//! behind a `Mutex`. The first connected WS session takes the event stream
//! via `next_event()`; additional concurrent sessions can still send requests
//! (via the clone-able `request_handle`) but will not receive pushed events.
//! Multi-tab support is a follow-up.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::extract::Path;
use axum::extract::State;
use axum::extract::WebSocketUpgrade;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use codex_app_server_client::InProcessAppServerClient;
use codex_app_server_client::InProcessAppServerRequestHandle;
use futures::SinkExt;
use futures::StreamExt;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use crate::assets;
use crate::codex_glue::ResolveKind;
use crate::codex_glue::build_response_envelope;
use crate::codex_glue::parse_client_request;
use crate::codex_glue::respond_to_server_request;
use crate::codex_glue::serialize_server_event;
use crate::codex_glue::{self};

/// Shared state carried by every request handler and WebSocket connection.
struct ServerState {
    /// Clone-able channel into the AppServerClient for sending ClientRequests
    /// and resolving/rejecting ServerRequests. Safe to share across many WS
    /// connections.
    request_handle: InProcessAppServerRequestHandle,
    /// The single owning handle to the AppServerClient. `next_event()` needs
    /// `&mut self`, so this is behind a Mutex. Only one WS connection at a
    /// time can usefully drain the event stream (see module docs).
    client: Mutex<InProcessAppServerClient>,
}

/// Bind and run the HTTP + WS server until Ctrl-C.
///
/// `host` defaults to loopback (the launcher enforces this); `port=None`
/// means "ephemeral" (the OS assigns a free port, which we print in the
/// banner so the user knows where to point the browser).
pub async fn serve(host: String, port: Option<u16>) -> anyhow::Result<()> {
    let client = codex_glue::build_dev_client().await?;
    let request_handle = client.request_handle();
    let state = Arc::new(ServerState {
        request_handle,
        client: Mutex::new(client),
    });

    let bind_addr = format!("{host}:{}", port.unwrap_or(0));
    let listener = TcpListener::bind(&bind_addr).await?;
    let local_addr = listener.local_addr()?;
    print_banner(local_addr);

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/readyz", get(health_handler))
        .route("/healthz", get(health_handler))
        .route("/", get(index_handler))
        .route("/{*path}", get(asset_handler))
        .with_state(state);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| anyhow::anyhow!("webui server exited: {e}"))?;
    Ok(())
}

fn print_banner(addr: SocketAddr) {
    eprintln!();
    eprintln!("lemurclaw webui");
    eprintln!("  listening on: http://{addr}");
    eprintln!("  readyz:       http://{addr}/readyz");
    eprintln!("  healthz:      http://{addr}/healthz");
    eprintln!("  note:         Ctrl-C to stop");
    eprintln!();
}

async fn shutdown_signal() {
    // Suppress the default ctrl_c message so our banner is the last thing the
    // user sees on exit. Errors here (e.g. signal handler unavailable) just
    // mean we won't shut down gracefully — the process can still be killed.
    let _ = tokio::signal::ctrl_c().await;
}

async fn health_handler() -> StatusCode {
    StatusCode::OK
}

async fn index_handler() -> Response {
    serve_asset("index.html")
}

async fn asset_handler(Path(path): Path<String>) -> Response {
    serve_asset(&path)
}

/// Build an axum `Response` for an embedded asset path. `index.html` gets a
/// 200 + content-type; anything else delegates to [`assets::serve_path`]. A
/// missing JS/CSS/etc. asset returns a plain 404 (no SPA fallback) so the
/// browser console surfaces the broken reference instead of silently loading
/// the HTML shell.
fn serve_asset(rel_path: &str) -> Response {
    let (status, content_type, body) = assets::serve_path(rel_path);
    let resp = match body {
        Some(bytes) => (
            StatusCode::from_u16(status).unwrap_or(StatusCode::OK),
            [(axum::http::header::CONTENT_TYPE, content_type)],
            bytes.to_vec(),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            [(axum::http::header::CONTENT_TYPE, content_type)],
            b"not found".to_vec(),
        )
            .into_response(),
    };
    resp
}

/// WebSocket upgrade handler. The actual per-connection bridge lives in
/// [`run_ws_connection`].
async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<ServerState>>) -> Response {
    ws.on_upgrade(move |socket| async move { run_ws_connection(socket, state).await })
}

/// Drive one WS connection to completion:
/// - Spawn a downstream task that holds the client Mutex and streams codex
///   events to the socket via a shared mpsc.
/// - In the main task, read inbound WS text frames and dispatch each via
///   [`handle_ipc_async`] (sending any JSON-RPC response back through the
///   same mpsc the downstream task drains).
///
/// Returns when either the browser disconnects or the codex event stream
/// closes.
async fn run_ws_connection(socket: axum::extract::ws::WebSocket, state: Arc<ServerState>) {
    let (mut sink, mut stream) = socket.split();

    // Single outbound channel: downstream-event-pusher and per-request
    // response builders both send Message::Text through here, and one writer
    // task drains it into the socket sink. Capacity matches codex's
    // WEBSOCKET_OUTBOUND_CHANNEL_CAPACITY order of magnitude.
    let (outbound_tx, mut outbound_rx) =
        tokio::sync::mpsc::channel::<axum::extract::ws::Message>(256);

    // Downstream: codex events → outbound channel. Takes the client Mutex for
    // the lifetime of the connection (so concurrent connections can send
    // requests but won't race on next_event). Releases on disconnect.
    let downstream_tx = outbound_tx.clone();
    let downstream_state = state.clone();
    let downstream = tokio::spawn(async move {
        let mut client_guard = downstream_state.client.lock().await;
        while let Some(event) = client_guard.next_event().await {
            let json = serialize_server_event(event);
            let msg = axum::extract::ws::Message::Text(json.into());
            if downstream_tx.send(msg).await.is_err() {
                // Writer side gone (socket closed) — stop draining.
                break;
            }
        }
    });

    // Writer task: drain outbound channel into the socket sink.
    let writer = tokio::spawn(async move {
        while let Some(msg) = outbound_rx.recv().await {
            if sink.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Upstream: read inbound WS frames and dispatch.
    while let Some(frame) = stream.next().await {
        let text = match frame {
            Ok(axum::extract::ws::Message::Text(t)) => t.to_string(),
            Ok(axum::extract::ws::Message::Close(_)) | Err(_) => break,
            // Ignore Binary / Ping / Pong — we only speak JSON text.
            Ok(_) => continue,
        };
        let resp_tx = outbound_tx.clone();
        handle_ipc_async(&state.request_handle, &text, resp_tx).await;
    }

    // Tear down: dropping the last outbound_tx clone lets the writer task exit
    // once the downstream task also drops its clone. The downstream task ends
    // when next_event() returns None (codex stream closed) or the outbound
    // channel closes (we're about to drop it).
    drop(outbound_tx);
    // Release the client Mutex so a future reconnection can take it. Aborting
    // is safe: the downstream task is either blocked on next_event (will be
    // dropped) or finished.
    downstream.abort();
    let _ = downstream.await;
    writer.abort();
}

/// Process one inbound WS text frame: a `__resolve`/`__reject` envelope, or a
/// `ClientRequest`. Responses (for the ClientRequest arm) are JSON-RPC
/// envelopes sent back through `resp_tx` as `Message::Text`.
///
/// Mirrors `lemurclaw-gui::backend::BackendHandles::handle_ipc` shape-for-
/// shape; the only difference is the sink (WS outbound mpsc vs tao
/// EventLoopProxy).
async fn handle_ipc_async(
    handle: &InProcessAppServerRequestHandle,
    body: &str,
    resp_tx: tokio::sync::mpsc::Sender<axum::extract::ws::Message>,
) {
    let value: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[lemurclaw] ws ipc body not valid JSON: {e}");
            return;
        }
    };

    if let Some(req_id) = value.get("__resolve") {
        if let Some(result) = value.get("result") {
            respond_to_server_request(handle, req_id.clone(), result.clone(), ResolveKind::Resolve)
                .await;
        } else {
            eprintln!("[lemurclaw] __resolve envelope missing 'result' field, dropping");
        }
        return;
    }

    if let Some(req_id) = value.get("__reject") {
        if let Some(error) = value.get("error") {
            respond_to_server_request(handle, req_id.clone(), error.clone(), ResolveKind::Reject)
                .await;
        } else {
            eprintln!("[lemurclaw] __reject envelope missing 'error' field, dropping");
        }
        return;
    }

    // ClientRequest arm.
    let (req_id_json, req) = match parse_client_request(value) {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("[lemurclaw] ws ipc body not a valid ClientRequest: {e}");
            return;
        }
    };
    match handle.request(req).await {
        Ok(result) => {
            let payload = build_response_envelope(req_id_json, result);
            match serde_json::to_string(&payload) {
                Ok(json) => {
                    if resp_tx
                        .send(axum::extract::ws::Message::Text(json.into()))
                        .await
                        .is_err()
                    {
                        eprintln!("[lemurclaw] ws outbound channel closed, dropping response");
                    }
                }
                Err(e) => eprintln!("[lemurclaw] failed to serialize response envelope: {e}"),
            }
        }
        Err(e) => eprintln!("[lemurclaw] backend request failed: {e}"),
    }
}
