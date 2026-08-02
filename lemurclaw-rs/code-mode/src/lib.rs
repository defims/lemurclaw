//! V8-free stub of code-mode for lemurclaw builds.
//!
//! Only the protocol types are re-exported. The session providers exist as
//! type stubs so downstream call sites compile, but they return errors at
//! runtime because the V8 runtime is excluded from this build. To restore
//! real V8-backed code mode, re-run `xtask publish rename` without stripping.

pub use lemurclaw_code_mode_protocol::*;

mod service;

pub use service::InProcessCodeModeSessionProvider;
pub use service::ProcessOwnedCodeModeSessionProvider;
pub use service::WebSocketCodeModeSessionProvider;
