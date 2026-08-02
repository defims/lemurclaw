//! Stub session providers for V8-free builds.
//!
//! These types mirror the public API surface of the real V8-backed providers
//! (`InProcessCodeModeSessionProvider`, `ProcessOwnedCodeModeSessionProvider`,
//! `WebSocketCodeModeSessionProvider`) so that downstream code compiles
//! unchanged. Every `create_session` call returns an error at runtime.

use std::path::PathBuf;
use std::sync::Arc;

use lemurclaw_code_mode_protocol::CodeModeSessionDelegate;
use lemurclaw_code_mode_protocol::CodeModeSessionProvider;
use lemurclaw_code_mode_protocol::CodeModeSessionProviderFuture;

const UNAVAILABLE: &str = "code-mode V8 runtime is excluded from this build";

/// In-process provider stub. The real implementation embeds a V8 isolate; this
/// stub always fails to create a session.
pub struct InProcessCodeModeSessionProvider;

impl CodeModeSessionProvider for InProcessCodeModeSessionProvider {
    fn create_session<'a>(
        &'a self,
        _delegate: Arc<dyn CodeModeSessionDelegate>,
    ) -> CodeModeSessionProviderFuture<'a> {
        Box::pin(async { Err(UNAVAILABLE.to_string()) })
    }
}

impl Default for InProcessCodeModeSessionProvider {
    fn default() -> Self {
        Self
    }
}

/// Process-owned provider stub. The real implementation spawns a
/// `code-mode-host` subprocess (with an in-process V8 fallback); this stub
/// preserves the constructor surface so call sites compile.
pub struct ProcessOwnedCodeModeSessionProvider;

impl ProcessOwnedCodeModeSessionProvider {
    /// Stub constructor. The host program argument is ignored.
    pub fn with_host_program(_host_program: PathBuf) -> Self {
        Self
    }

    /// Stub configurator. Returns `self` unchanged.
    pub fn without_in_process_fallback(self) -> Self {
        self
    }
}

impl Default for ProcessOwnedCodeModeSessionProvider {
    fn default() -> Self {
        Self
    }
}

impl CodeModeSessionProvider for ProcessOwnedCodeModeSessionProvider {
    fn create_session<'a>(
        &'a self,
        _delegate: Arc<dyn CodeModeSessionDelegate>,
    ) -> CodeModeSessionProviderFuture<'a> {
        Box::pin(async { Err(UNAVAILABLE.to_string()) })
    }
}

/// WebSocket provider stub. The real implementation connects to a remote
/// code-mode host over WebSocket; this stub preserves the constructor surface
/// so `lemurclaw-app-server` compiles unchanged.
pub struct WebSocketCodeModeSessionProvider;

impl WebSocketCodeModeSessionProvider {
    /// Stub constructor. Both arguments are ignored.
    pub fn with_http_client_factory(
        _websocket_url: String,
        _http_client_factory: lemurclaw_http_client::HttpClientFactory,
    ) -> Self {
        Self
    }
}

impl CodeModeSessionProvider for WebSocketCodeModeSessionProvider {
    fn create_session<'a>(
        &'a self,
        _delegate: Arc<dyn CodeModeSessionDelegate>,
    ) -> CodeModeSessionProviderFuture<'a> {
        Box::pin(async { Err(UNAVAILABLE.to_string()) })
    }
}
