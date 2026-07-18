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
//! on it. The backend never touches the webview; results of requests the JS
//! side cares about come back as `ServerNotification`s through the normal
//! `next_event` loop.

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
}

impl BackendHandles {
    /// Forward a raw JSON `ClientRequest` body (from the JS side) to the
    /// backend. Deserialization happens on the backend runtime so a malformed
    /// body only logs, never blocks the UI thread.
    pub fn handle_ipc(&self, body: &str) {
        let req_result = serde_json::from_str::<ClientRequest>(body);
        let handle = self.request_handle.clone();
        self.handle.spawn(async move {
            match req_result {
                Ok(req) => {
                    if let Err(e) = handle.request(req).await {
                        eprintln!("[lemurclaw] backend request failed: {e}");
                    }
                }
                Err(e) => {
                    eprintln!("[lemurclaw] ipc body not a valid ClientRequest: {e}");
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
