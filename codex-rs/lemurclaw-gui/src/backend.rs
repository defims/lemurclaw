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

use std::sync::Arc;

use codex_app_server_client::legacy_core::config::ConfigBuilder;
use codex_app_server_client::{
    DEFAULT_IN_PROCESS_CHANNEL_CAPACITY, EnvironmentManager, InProcessAppServerClient,
    InProcessAppServerRequestHandle, InProcessClientStartArgs, InProcessServerEvent,
};
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_arg0::Arg0DispatchPaths;
use codex_config::LoaderOverrides;
use codex_protocol::protocol::SessionSource;
use codex_rollout::state_db;
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

/// Distinguishes the two envelope kinds handled in `handle_ipc`.
enum ResolveKind {
    Resolve,
    Reject,
}

/// Respond to (resolve or reject) a pending ServerRequest on the backend
/// runtime. Awaited inside the task spawned by `handle_ipc` (does not spawn
/// anything itself). Pulls the RequestId / JsonRpcResult / JSONRPCErrorError
/// types from the protocol crate; falls back to a safe null/error payload on
/// malformed input so a bad JS envelope never kills the worker.
async fn respond_to_server_request(
    handle: &InProcessAppServerRequestHandle,
    request_id: serde_json::Value,
    payload: serde_json::Value,
    kind: ResolveKind,
) {
    let req_id = match serde_json::from_value::<codex_app_server_protocol::RequestId>(request_id) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("[lemurclaw] resolve: bad request_id: {e}");
            return;
        }
    };
    let result = match kind {
        ResolveKind::Resolve => {
            // `codex_app_server_protocol::Result` is a type alias for
            // `serde_json::Value` (the JSON-RPC result payload). On a
            // malformed envelope fall back to JSON null rather than failing
            // the resolve, so a bad JS body never hangs an approval flow.
            let json_result: codex_app_server_protocol::Result =
                serde_json::from_value(payload).unwrap_or(serde_json::Value::Null);
            handle.resolve_server_request(req_id, json_result).await
        }
        ResolveKind::Reject => {
            let err: codex_app_server_protocol::JSONRPCErrorError = serde_json::from_value(payload)
                .unwrap_or_else(|e| codex_app_server_protocol::JSONRPCErrorError {
                    code: -32000,
                    message: format!("malformed reject payload: {e}"),
                    data: None,
                });
            handle.reject_server_request(req_id, err).await
        }
    };
    if let Err(e) = result {
        eprintln!("[lemurclaw] resolve/reject failed: {e}");
    }
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
                    respond_to_server_request(
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
                    respond_to_server_request(
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
                // Capture the raw `id` from the value BEFORE from_value
                // consumes it. Every ClientRequest variant carries an `id`
                // field named `id` (typed as `RequestId = string | number`),
                // but pulling it back out of the enum after deserialization is
                // awkward; the untyped Value makes it a one-liner.
                let req_id_json =
                    value.get("id").cloned().unwrap_or(serde_json::Value::Null);
                match serde_json::from_value::<ClientRequest>(value) {
                    Ok(req) => {
                        match handle.request(req).await {
                            Ok(result) => {
                                let envelope = match result {
                                    Ok(val) => serde_json::json!({
                                        "jsonrpc": "2.0", "id": req_id_json, "result": val,
                                    }),
                                    Err(err) => serde_json::json!({
                                        "jsonrpc": "2.0", "id": req_id_json,
                                        "error": { "code": err.code, "message": err.message, "data": err.data },
                                    }),
                                };
                                if let Ok(json) = serde_json::to_string(&envelope) {
                                    if let Err(e) = proxy.send_event(GuiEvent::Response(json)) {
                                        eprintln!("[lemurclaw] response proxy closed: {e}");
                                    }
                                }
                            }
                            Err(e) => eprintln!("[lemurclaw] backend request failed: {e}"),
                        }
                    }
                    Err(e) => eprintln!("[lemurclaw] ipc body not a valid ClientRequest: {e}"),
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
            let client = match build_and_start_client().await {
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

/// Build the in-process AppServerClient using the dev path (no real auth,
/// `EnvironmentManager::default_for_tests()`). This is sufficient for the
/// subproject 2 completion criterion: a visible Initialize handshake.
/// Production-grade env / config wiring lands in a later task.
async fn build_and_start_client() -> anyhow::Result<InProcessAppServerClient> {
    let config = Arc::new(
        ConfigBuilder::default()
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("failed to build config: {e}"))?,
    );

    let state_db = state_db::try_init(config.as_ref())
        .await
        .map_err(|e| anyhow::anyhow!("failed to init state db: {e}"))?;

    let client = InProcessAppServerClient::start(InProcessClientStartArgs {
        arg0_paths: Arg0DispatchPaths::default(),
        config,
        cli_overrides: Vec::new(),
        loader_overrides: LoaderOverrides::default(),
        strict_config: false,
        cloud_config_bundle: Default::default(),
        feedback: Default::default(),
        log_db: None,
        state_db: Some(state_db),
        environment_manager: Arc::new(EnvironmentManager::default_for_tests()),
        config_warnings: Vec::<ConfigWarningNotification>::new(),
        session_source: SessionSource::Cli,
        enable_codex_api_key_env: false,
        client_name: "lemurclaw-gui".to_string(),
        client_version: env!("CARGO_PKG_VERSION").to_string(),
        experimental_api: true,
        mcp_server_openai_form_elicitation: false,
        opt_out_notification_methods: Vec::new(),
        channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    })
    .await
    .map_err(|e| anyhow::anyhow!("failed to start in-process app server: {e}"))?;

    Ok(client)
}

/// Run the `next_event` loop until the backend stream closes. Each event is
/// serialized to JSON and shipped to the main thread via the proxy. Events
/// that fail to serialize are logged and skipped (a single bad event must
/// not kill the whole loop).
async fn run_next_event_loop(
    mut client: InProcessAppServerClient,
    proxy: EventLoopProxy<GuiEvent>,
) {
    while let Some(event) = client.next_event().await {
        let json = serialize_event(event);
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

/// Serialize an `InProcessServerEvent` for JS delivery.
///
/// `InProcessServerEvent` itself does not derive `Serialize`, but its inner
/// `ServerNotification` / `ServerRequest` variants do. The `Lagged` variant
/// (which carries only a `skipped` count) gets a hand-rolled JSON envelope so
/// the JS side still sees *something* useful instead of a silent drop.
fn serialize_event(event: InProcessServerEvent) -> String {
    match event {
        InProcessServerEvent::ServerNotification(n) => serde_json::to_string(&n)
            .unwrap_or_else(|e| format!(r#"{{"serializeError":"notification: {e}"}}"#)),
        InProcessServerEvent::ServerRequest(r) => serde_json::to_string(&r)
            .unwrap_or_else(|e| format!(r#"{{"serializeError":"serverRequest: {e}"}}"#)),
        InProcessServerEvent::Lagged { skipped } => {
            format!(r#"{{"type":"lagged","skipped":{skipped}}}"#)
        }
    }
}

/// Worker → spawner start status. Sent exactly once over the std mpsc.
enum StartMessage {
    Started(InProcessAppServerRequestHandle),
    Failed(anyhow::Error),
}
