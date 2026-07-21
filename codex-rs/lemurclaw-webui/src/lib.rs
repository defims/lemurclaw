//! LemurClaw shared frontend assets + (Stage 2+) webui server.
//!
//! This is the bottom crate in the lemurclaw frontend stack. It owns:
//! - the shared React frontend under `assets/` (built by this crate's
//!   `build.rs` into `assets/dist/`, then embedded at compile time via
//!   `include_dir!`)
//! - crate-agnostic asset-serving helpers used by both `lemurclaw-gui`
//!   (wry/tao webview) and the in-process webui server (Stage 2)
//! - (Stage 2) the axum HTTP + WebSocket bridge that backs `--frontend webui`,
//!   plus the shared codex glue (event serialization, JSON-RPC envelope
//!   building, dev `InProcessAppServerClient` bootstrap) that `lemurclaw-gui`
//!   also consumes
//!
//! Dependency direction: `lemurclaw-gui` depends on `lemurclaw-webui`, not the
//! reverse — this keeps wry/tao isolated in gui and lets webui serve browsers
//! without inheriting native windowing deps.

pub mod assets;
