//! Backend: tokio runtime + in-process AppServerClient + IPC forwarding.
//!
//! Owns the non-UI half of the GUI:
//! - a dedicated tokio runtime (spun up on its own OS thread, since the main
//!   thread must run the tao event loop)
//! - the `InProcessAppServerClient`, started with a minimal dev config
//! - a `next_event` worker that serializes each `AppServerEvent` to JSON and
//!   ships it back to the main thread via a tao `EventLoopProxy`
//! - an `handle_ipc` entry point the main-thread `ipc_handler` calls to
//!   forward `ClientRequest` JSON to the backend
//!
//! Design note: the `ipc_handler` runs synchronously on the main thread. To
//! call the async `request_handle().request(...)`, we capture a
//! `tokio::runtime::Handle` (Clone + Send + Sync) and `spawn` a one-shot task
//! on it. The backend never touches the webview; the `RequestResult` of each
//! ClientRequest is wrapped in a `{jsonrpc, id, result|error}` envelope and
//! pushed back to the main thread via the same `EventLoopProxy` the
//! `next_event` loop uses. The main thread then `evaluate_script`s it into
//! `window.__lemurclaw.onResponse`, where transport.ts matches it by id
//! against pending promises.
//!
//! Shared codex-protocol glue (event serialization, JSON-RPC envelope
//! building, ServerRequest resolve/reject, dev client bootstrap) lives in
//! `lemurclaw_webui::codex_glue` and is reused verbatim by the in-process
//! webui server — same wire contract on both frontends.

use codex_app_server_client::InProcessAppServerClient;
use codex_app_server_client::InProcessAppServerRequestHandle;
use lemurclaw_webui::codex_glue;
use lemurclaw_webui::codex_glue::ResolveKind;
use tao::event_loop::EventLoopProxy;
use tokio::runtime::Handle;

use crate::GuiEvent;

/// Handles held by the main thread to talk to the backend.
///
/// `handle` lets the `ipc_handler` spawn async work on the backend runtime.
/// `request_handle` is the actual channel into the AppServerClient.
/// `proxy` is cloned into each `handle_ipc` task so it can push the
/// JSON-RPC response envelope back to the main thread (matched by id in
/// transport.ts).
///
/// Kept as a plain struct (not `Drop`) because the backend runtime is
/// intentionally detached: when the GUI window closes, the process exits and
/// the OS reaps the runtime thread. A graceful shutdown path can be added in
/// a later task.
pub struct BackendHandles {
    /// Handle to the backend tokio runtime (for spawning async work from
    /// synchronous main-thread callers like the wry `ipc_handler`).
    pub handle: Handle,
    /// Clone-able channel into the AppServerClient for sending ClientRequests.
    request_handle: InProcessAppServerRequestHandle,
    /// Clone of the tao `EventLoopProxy`: used by `handle_ipc`'s task to push
    /// JSON-RPC response envelopes back to the main thread (which then
    /// forwards them to JS via `evaluate_script`).
    proxy: EventLoopProxy<GuiEvent>,
}

impl BackendHandles {
    /// Forward a raw JSON body from JS to the backend. Three shapes:
    /// - `{"__resolve": id, "result": {...}}` resolves a pending ServerRequest
    ///   (ApprovalCard accept).
    /// - `{"__reject": id, "error": {code, message}}` rejects a pending
    ///   ServerRequest (ApprovalCard decline/cancel).
    /// - any other JSON is parsed as a ClientRequest (turn/start, thread/list,
    ///   ...). The returned `RequestResult` is wrapped in a JSON-RPC
    ///   `{jsonrpc, id, result|error}` envelope and pushed back via the proxy
    ///   so transport.ts can settle the pending promise.
    ///
    /// All deserialization happens on the backend runtime so malformed bodies
    /// only log, never block the UI thread.
    pub fn handle_ipc(&self, body: &str) {
        let parsed: serde_json::Result<serde_json::Value> = serde_json::from_str(body);
        let handle = self.request_handle.clone();
        let proxy = self.proxy.clone();
        self.handle.spawn(async move {
            let value = match parsed {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[lemurclaw] ipc body not valid JSON: {e}");
                    return;
                }
            };
            if let Some(req_id) = value.get("__resolve") {
                if let Some(result) = value.get("result") {
                    codex_glue::respond_to_server_request(
                        &handle,
                        req_id.clone(),
                        result.clone(),
                        ResolveKind::Resolve,
                    )
                    .await;
                } else {
                    eprintln!("[lemurclaw] __resolve envelope missing 'result' field, dropping");
                }
            } else if let Some(req_id) = value.get("__reject") {
                if let Some(error) = value.get("error") {
                    codex_glue::respond_to_server_request(
                        &handle,
                        req_id.clone(),
                        error.clone(),
                        ResolveKind::Reject,
                    )
                    .await;
                } else {
                    eprintln!("[lemurclaw] __reject envelope missing 'error' field, dropping");
                }
            } else {
                let (req_id_json, req) = match codex_glue::parse_client_request(value) {
                    Ok(pair) => pair,
                    Err(e) => {
                        eprintln!("[lemurclaw] ipc body not a valid ClientRequest: {e}");
                        return;
                    }
                };
                match handle.request(req).await {
                    Ok(result) => {
                        let payload = codex_glue::build_response_envelope(req_id_json, result);
                        match serde_json::to_string(&payload) {
                            Ok(json) => {
                                if let Err(e) = proxy.send_event(GuiEvent::Response(json)) {
                                    eprintln!("[lemurclaw] response proxy closed: {e}");
                                }
                            }
                            Err(e) => {
                                eprintln!("[lemurclaw] failed to serialize response envelope: {e}")
                            }
                        }
                    }
                    Err(e) => eprintln!("[lemurclaw] backend request failed: {e}"),
                }
            }
        });
    }
}

/// Spawn the backend: a dedicated OS thread hosting a tokio runtime, which
/// builds the in-process AppServerClient and runs its `next_event` loop,
/// forwarding events as JSON to the main thread via `proxy`.
///
/// Blocks until the AppServerClient has either started successfully (in which
/// case `Ok(handles)` is returned and the worker keeps running detached) or
/// failed to start (in which case the worker thread tears down and the error
/// is surfaced).
pub fn spawn(proxy: EventLoopProxy<GuiEvent>) -> anyhow::Result<BackendHandles> {
    // Channel for the worker thread to report start success/failure back to
    // the spawner before `spawn` returns. Capacity 1: exactly one message is
    // ever sent.
    let (start_tx, start_rx) = std::sync::mpsc::channel::<StartMessage>();

    // Build the runtime with a 16 MB worker stack to match codex's own
    // production runtime (see codex-rs/arg0/src/lib.rs:
    // TOKIO_WORKER_STACK_SIZE_BYTES). tokio's default 2 MB worker stack is
    // too small for codex-core's deep recursion in agent::control / config
    // loading paths, and crashes the process with a stack overflow on the
    // first non-trivial request (e.g. thread/list).
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(16 * 1024 * 1024)
        .build()?;
    let handle = runtime.handle().clone();
    // Clone the proxy for the BackendHandles struct: `handle_ipc` needs to
    // push JSON-RPC response envelopes back to the main thread. The original
    // `proxy` is moved into the worker thread for the next_event loop.
    let handles_proxy = proxy.clone();

    // Enter the runtime on a background thread so the main thread is free
    // for tao. Use a dedicated OS thread (not `runtime.spawn`), because the
    // runtime's own worker threads aren't a place we can block indefinitely
    // for our orchestration; we want this thread to own the runtime.
    std::thread::spawn(move || {
        runtime.block_on(async move {
            let client = match codex_glue::build_dev_client().await {
                Ok(c) => c,
                Err(e) => {
                    let _ = start_tx.send(StartMessage::Failed(e));
                    return;
                }
            };
            let request_handle = client.request_handle();
            // Report successful start to the spawner.
            let _ = start_tx.send(StartMessage::Started(request_handle));

            // Run the next_event loop until the backend closes (returns None).
            // `proxy` is moved into the worker (the main thread consumes
            // events via the EventLoop itself, not via this handle).
            run_next_event_loop(client, proxy).await;
        });
    });

    match start_rx.recv() {
        Ok(StartMessage::Started(request_handle)) => Ok(BackendHandles {
            handle,
            request_handle,
            proxy: handles_proxy,
        }),
        Ok(StartMessage::Failed(e)) => Err(e),
        Err(_) => anyhow::bail!("backend thread panicked before reporting start status"),
    }
}

/// Run the `next_event` loop until the backend stream closes. Each event is
/// serialized to JSON (via the shared `codex_glue::serialize_server_event`)
/// and shipped to the main thread via the proxy. Events that fail to
/// serialize are logged and skipped (a single bad event must not kill the
/// whole loop).
async fn run_next_event_loop(
    mut client: InProcessAppServerClient,
    proxy: EventLoopProxy<GuiEvent>,
) {
    while let Some(event) = client.next_event().await {
        let json = codex_glue::serialize_server_event(event);
        if let Err(e) = proxy.send_event(GuiEvent::ServerEvent(json)) {
            // EventLoopClosed means the window went away; we can stop.
            eprintln!("[lemurclaw] event proxy closed, stopping worker: {e}");
            break;
        }
    }
    // Stream ended. The runtime stays up so handle_ipc still works against a
    // closed channel and just no-ops; the process exits when the user closes
    // the window.
}

/// Worker → spawner start status. Sent exactly once over the std mpsc.
enum StartMessage {
    Started(InProcessAppServerRequestHandle),
    Failed(anyhow::Error),
}
