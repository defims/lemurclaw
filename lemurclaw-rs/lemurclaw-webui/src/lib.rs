//! LemurClaw shared frontend assets + in-process webui server.
//!
//! This is the bottom crate in the lemurclaw frontend stack. It owns:
//! - the shared React frontend under `assets/` (built by this crate's
//!   `build.rs` into `assets/dist/`, then embedded at compile time via
//!   `include_dir!`)
//! - crate-agnostic asset-serving helpers used by both `lemurclaw-gui`
//!   (wry/tao webview) and this crate's own webui server
//! - [`codex_glue`]: shared codex JSON-RPC plumbing (event serialization,
//!   JSON-RPC envelope building, dev `InProcessAppServerClient` bootstrap)
//!   consumed by both `lemurclaw-gui::backend` and [`server`]
//! - [`server`]: the axum HTTP + WebSocket bridge that backs
//!   `--frontend webui` — serves the embedded React app over HTTP and
//!   bridges browser WS frames to the in-process AppServerClient
//!
//! Dependency direction: `lemurclaw-gui` depends on `lemurclaw-webui`, not the
//! reverse — this keeps wry/tao isolated in gui and lets webui serve browsers
//! without inheriting native windowing deps.

pub mod assets;
pub mod codex_glue;
pub mod server;

use crate::server::serve;

/// Entry point for `lemurclaw --frontend webui`.
///
/// Builds a multi-thread tokio runtime with a 16 MB worker stack (matching
/// codex's production runtime — tokio's default 2 MB stack is too small for
/// codex-core's deep recursion in agent::control / config loading paths and
/// crashes on the first non-trivial request), then blocks on [`serve`] until
/// Ctrl-C.
pub fn run_webui(host: String, port: Option<u16>) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(16 * 1024 * 1024)
        .build()?;
    runtime.block_on(serve(host, port))
}
