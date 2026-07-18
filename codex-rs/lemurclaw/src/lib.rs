//! Lemurclaw launcher.
//!
//! A thin entry point that selects a frontend (`tui`, `gui`, or `webui`) and
//! runs it. Today only the `tui` frontend is implemented: it passes straight
//! through to `codex_tui::run_main` via `codex_arg0::arg0_dispatch_or_else`,
//! mirroring `codex-rs/tui/src/main.rs`. The `gui` and `webui` frontends are
//! stubs that return an error and will be wired up to `lemurclaw_transport`
//! in later tasks.
//!
//! [`run`] is intentionally synchronous: `arg0_dispatch_or_else` owns process
//! exit (spawns its own runtime + thread and calls `process::exit` on fatal),
//! so the TUI path has no need for an async entry point. The GUI/WebUI paths
//! will become async later; for now they simply error.

pub mod config;

pub use config::{Cli, Frontend, RuntimeConfig};
pub use lemurclaw_transport as transport;

use std::io::Write;

use clap::Parser;
use codex_arg0::Arg0DispatchPaths;
use codex_arg0::arg0_dispatch_or_else;
use codex_config::LoaderOverrides;
use codex_tui::AppExitInfo;
use codex_tui::Cli as TuiCli;
use codex_tui::ExitReason;
use codex_tui::run_main;

use crate::Frontend::{Gui, Tui, Webui};

/// Run lemurclaw with the given [`RuntimeConfig`].
///
/// Synchronous: the TUI path delegates to `arg0_dispatch_or_else`, which owns
/// process exit. The GUI/WebUI paths are not implemented yet and return an
/// error without taking over the process.
pub fn run(config: RuntimeConfig) -> anyhow::Result<()> {
    match config.frontend {
        Tui => run_tui(),
        Gui => Err(anyhow::anyhow!(
            "lemurclaw `gui` frontend is not implemented yet"
        )),
        Webui => Err(anyhow::anyhow!(
            "lemurclaw `webui` frontend is not implemented yet"
        )),
    }
}

/// TUI frontend: pass through to `codex_tui::run_main` under
/// `arg0_dispatch_or_else` (which owns process exit on fatal errors).
fn run_tui() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|arg0_paths: Arg0DispatchPaths| async move {
        // Re-parse codex_tui::Cli from argv so all codex flags keep working.
        // (Lemurclaw-specific flags like --frontend/--agent-name are NOT known
        //  to codex_tui::Cli; callers relying on them should drop them before
        //  invoking the TUI path. See crate docs.)
        let cli = TuiCli::parse();
        let exit_info = run_main(
            cli,
            arg0_paths,
            LoaderOverrides::default(),
            /*explicit_remote_endpoint*/ None,
        )
        .await?;
        handle_exit_reason(exit_info);
        Ok(())
    })
}

/// Mirror the exit-reason handling from `codex-rs/tui/src/main.rs`: on `Fatal`,
/// print `ERROR: <message>` to stderr and exit non-zero. On `UserRequested`
/// (or any other reason), return normally and let `arg0` exit cleanly.
fn handle_exit_reason(exit_info: AppExitInfo) {
    match exit_info.exit_reason {
        ExitReason::Fatal(message) => {
            eprintln!("ERROR: {message}");
            // Best-effort flush before we bail; ignore flush errors.
            let _ = std::io::stdout().flush();
            let _ = std::io::stderr().flush();
            std::process::exit(1);
        }
        ExitReason::UserRequested => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webui_frontend_returns_error() {
        let cfg = RuntimeConfig {
            frontend: Frontend::Webui,
            ..Default::default()
        };
        let err = run(cfg).unwrap_err();
        assert!(
            err.to_string().contains("webui"),
            "expected webui in error message, got: {err}"
        );
    }

    #[test]
    fn gui_frontend_returns_error() {
        let cfg = RuntimeConfig {
            frontend: Frontend::Gui,
            ..Default::default()
        };
        let err = run(cfg).unwrap_err();
        assert!(
            err.to_string().contains("gui"),
            "expected gui in error message, got: {err}"
        );
    }
}
