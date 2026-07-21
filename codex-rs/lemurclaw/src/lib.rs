//! Lemurclaw launcher.
//!
//! A thin entry point that selects a frontend (`tui`, `gui`, or `webui`) and
//! runs it. The `tui` frontend passes straight through to `codex_tui::run_main`
//! via `codex_arg0::arg0_dispatch_or_else`, mirroring `codex-rs/tui/src/main.rs`.
//! The `gui` frontend opens a wry+tao window via `lemurclaw_gui::run_gui`,
//! loading the shared React app from `lemurclaw_webui::assets` over a custom
//! `lemurclaw://` protocol. The `webui` frontend serves the same shared React
//! app over HTTP from `lemurclaw_webui::run_webui` (browser connects via
//! WebSocket to a bridge backed by the in-process `InProcessAppServerClient`).
//!
//! [`run`] is intentionally synchronous: `arg0_dispatch_or_else` (TUI), the
//! tao event loop (GUI), and the axum server (WebUI) all own process exit /
//! the main thread.

pub mod config;

pub use config::Cli;
pub use config::Frontend;
pub use config::RuntimeConfig;
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

use crate::Frontend::Gui;
use crate::Frontend::Tui;
use crate::Frontend::Webui;

/// Run lemurclaw with the given [`RuntimeConfig`].
///
/// Synchronous: the TUI path delegates to `arg0_dispatch_or_else` (owns
/// process exit), the GUI path enters the tao event loop inside
/// `lemurclaw_gui::run_gui` (owns the main thread until the window closes),
/// and the WebUI path enters the axum server loop inside
/// `lemurclaw_webui::run_webui` (owns the thread until Ctrl-C).
pub fn run(config: RuntimeConfig) -> anyhow::Result<()> {
    match config.frontend {
        Tui => run_tui(),
        Gui => lemurclaw_gui::run_gui(),
        Webui => {
            // Loopback-only for now: the webui server has no WS auth. Reject
            // any non-loopback bind with a clear message so a user passing
            // `--host 0.0.0.0` gets a hint instead of silently exposing an
            // unauthenticated codex bridge on their LAN. Mirrors codex's
            // `is_unauthenticated_non_loopback_listener` stance.
            if !is_loopback_host(&config.host) {
                anyhow::bail!(
                    "refusing non-loopback webui bind `{}` without auth; pass `--host 127.0.0.1` (default) or `--host ::1`",
                    config.host
                );
            }
            lemurclaw_webui::run_webui(config.host.clone(), config.port)
        }
    }
}

/// True for `127.0.0.1`, `::1`, and `localhost`. Anything else (including
/// `0.0.0.0`, `::`, public IPs, hostnames) is treated as non-loopback. We
/// don't resolve hostnames via DNS — the user can pass a literal IP for any
/// non-default bind they actually want.
fn is_loopback_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "::1" | "localhost")
}

/// TUI frontend: pass through to `codex_tui::run_main` under
/// `arg0_dispatch_or_else` (which owns process exit on fatal errors).
fn run_tui() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|arg0_paths: Arg0DispatchPaths| async move {
        // Parse codex_tui::Cli from argv, but first strip lemurclaw-only flags
        // (--frontend / --agent-name) so codex_tui::Cli doesn't reject them as
        // unknown arguments. We also strip --cwd/--model/--yolo because lemurclaw
        // owns them (they'll be passed to codex via config, not argv).
        let filtered = strip_lemurclaw_args(std::env::args_os());
        let cli = TuiCli::parse_from(filtered);
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

/// Lemurclaw-owned CLI flags that must be stripped from argv before handing
/// control to `codex_tui::Cli` (which would reject them as unknown).
const LEMURCLAW_VALUE_FLAGS: &[&str] = &[
    "--frontend",
    "--agent-name",
    "--cwd",
    "--model",
    "--host",
    "--port",
];
/// Boolean flags (no value follows).
const LEMURCLAW_BOOL_FLAGS: &[&str] = &["--yolo"];

/// Strip lemurclaw-owned flags from an argv iterator.
///
/// Handles both `--flag value` and `--flag=value` forms, plus the short
/// equivalents `-C` (cwd) and `-m` (model). Unknown args are passed through
/// untouched so codex_tui::Cli sees exactly its own flags.
fn strip_lemurclaw_args<I, S>(args: I) -> Vec<std::ffi::OsString>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    use std::ffi::OsString;

    fn matches_long(arg: &str, name: &str) -> bool {
        arg == name || arg.starts_with(&format!("{name}="))
    }

    let mut out: Vec<OsString> = Vec::new();
    let mut iter = args.into_iter().peekable();
    while let Some(arg) = iter.next() {
        let arg_owned: OsString = arg.as_ref().into();
        // Compare on the lossy UTF-8 form (our flags are all ASCII).
        let arg_str = arg_owned.to_string_lossy().into_owned();
        // Long value flags: --flag or --flag=...; consume following value if bare.
        if LEMURCLAW_VALUE_FLAGS
            .iter()
            .any(|f| matches_long(&arg_str, f))
        {
            if !arg_str.contains('=') {
                let _ = iter.next(); // swallow the value token
            }
            continue;
        }
        // Long bool flags: --yolo or --yolo=...
        if LEMURCLAW_BOOL_FLAGS
            .iter()
            .any(|f| matches_long(&arg_str, f))
        {
            continue;
        }
        // Short value flags: -C <dir>, -m <model>
        if arg_str == "-C" || arg_str == "-m" {
            let _ = iter.next();
            continue;
        }
        // Short flags glued to value: -Cdir / -mmodel — drop them.
        if arg_str.starts_with("-C") || arg_str.starts_with("-m") {
            continue;
        }
        out.push(arg_owned);
    }
    out
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

    /// Helper: run strip and stringify the result for easy assertion.
    fn stripped<I, S>(args: I) -> Vec<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        strip_lemurclaw_args(args)
            .into_iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn strip_removes_frontend_and_value() {
        let out = stripped(vec!["lemurclaw", "--frontend", "tui", "hello"]);
        assert_eq!(out, vec!["lemurclaw".to_string(), "hello".to_string()]);
    }

    #[test]
    fn strip_removes_equals_form() {
        let out = stripped(vec!["lemurclaw", "--agent-name=foo", "--yolo", "prompt"]);
        assert_eq!(out, vec!["lemurclaw".to_string(), "prompt".to_string()]);
    }

    #[test]
    fn strip_removes_short_flags() {
        let out = stripped(vec!["lemurclaw", "-C", "/tmp", "-m", "gpt", "p"]);
        assert_eq!(out, vec!["lemurclaw".to_string(), "p".to_string()]);
    }

    #[test]
    fn strip_removes_webui_host_and_port() {
        // --host/--port are lemurclaw-owned (webui only); codex_tui::Cli must
        // never see them. Both bare and =value forms.
        let out = stripped(vec![
            "lemurclaw",
            "--frontend",
            "webui",
            "--host",
            "0.0.0.0",
            "--port=8080",
            "prompt",
        ]);
        assert_eq!(out, vec!["lemurclaw".to_string(), "prompt".to_string()]);
    }

    #[test]
    fn strip_passes_through_unknown() {
        // codex flags like --search / --no-alt-screen must survive.
        let out = stripped(vec!["lemurclaw", "--search", "--no-alt-screen", "p"]);
        assert_eq!(
            out,
            vec![
                "lemurclaw".to_string(),
                "--search".to_string(),
                "--no-alt-screen".to_string(),
                "p".to_string()
            ]
        );
    }

    #[test]
    fn strip_handles_yolo_bool() {
        let out = stripped(vec!["lemurclaw", "--yolo", "--frontend=tui"]);
        assert_eq!(out, vec!["lemurclaw".to_string()]);
    }

    #[test]
    fn webui_refuses_non_loopback_host() {
        // The webui server has no auth; loopback-only is enforced in run().
        // A non-loopback host must fail fast with a clear message instead of
        // silently binding an unauthenticated codex bridge on the LAN.
        let cfg = RuntimeConfig {
            frontend: Frontend::Webui,
            host: "0.0.0.0".to_string(),
            ..Default::default()
        };
        let err = run(cfg).unwrap_err();
        assert!(
            err.to_string().contains("non-loopback"),
            "expected non-loopback refusal, got: {err}"
        );
    }

    #[test]
    fn is_loopback_host_recognizes_known_loopback() {
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("::1"));
        assert!(is_loopback_host("localhost"));
        assert!(!is_loopback_host("0.0.0.0"));
        assert!(!is_loopback_host("::"));
        assert!(!is_loopback_host("192.168.1.5"));
        assert!(!is_loopback_host("my-host.example"));
    }

    // Note: we deliberately do NOT smoke-test `run_webui()` directly here — it
    // binds a real socket and blocks on Ctrl-C. The webui crate has its own
    // integration tests (lemurclaw-webui/tests) covering the handler shapes.
}
