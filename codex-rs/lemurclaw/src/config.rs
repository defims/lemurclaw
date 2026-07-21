//! Lemurclaw top-level configuration: the frontend selector and the runtime
//! config assembled from CLI args.
//!
//! `lemurclaw` is a thin launcher that currently passes straight through to
//! `codex_tui::run_main` for the `tui` frontend; `gui` and `webui` are stubs.

use std::path::PathBuf;

use clap::Parser;
use serde::Deserialize;
use serde::Serialize;

/// Which frontend lemurclaw should launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Frontend {
    /// Terminal UI: pass through to `codex_tui::run_main`.
    #[default]
    #[value(name = "tui")]
    Tui,
    /// Native GUI (wry). Not implemented yet.
    #[value(name = "gui")]
    Gui,
    /// Browser-based UI over WebSocket. Serves the shared React app over HTTP
    /// and bridges browser WS frames to the in-process AppServerClient.
    #[value(name = "webui")]
    Webui,
}

/// Frontend-agnostic runtime configuration assembled from the CLI.
///
/// This is what [`crate::run`] consumes. Most codex flags are still parsed
/// directly by `codex_tui::Cli` inside the TUI path; the fields here are the
/// lemurclaw-specific knobs plus the few we forward explicitly.
#[derive(Debug, Clone, Default)]
pub struct RuntimeConfig {
    /// Logical name for the agent instance (used by future GUI/WebUI modes).
    pub agent_name: Option<String>,
    /// Which frontend to launch.
    pub frontend: Frontend,
    /// Working directory to run the agent in (`None` = inherit).
    pub cwd: Option<PathBuf>,
    /// Model override forwarded to the frontend.
    pub model: Option<String>,
    /// Skip all approvals/sandbox (`--dangerously-bypass-approvals-and-sandbox`).
    pub yolo: bool,
    /// WebUI only: host address to bind the HTTP+WS server on. Defaults to
    /// `127.0.0.1` (loopback only — the webui server has no auth). Ignored by
    /// the TUI and GUI frontends.
    pub host: String,
    /// WebUI only: TCP port to bind the HTTP+WS server on. `None` means
    /// ephemeral (the OS assigns a free port, printed in the startup banner).
    /// Ignored by the TUI and GUI frontends.
    pub port: Option<u16>,
}

/// Top-level lemurclaw CLI.
///
/// Only the lemurclaw-specific flags live here. The TUI path re-parses the
/// full `codex_tui::Cli` from argv so it picks up every flag codex supports.
#[derive(Parser, Debug, Clone)]
#[command(
    name = "lemurclaw",
    version,
    about = "Lemurclaw launcher (TUI/GUI/WebUI)"
)]
pub struct Cli {
    /// Logical agent name (used by future GUI/WebUI modes).
    #[arg(long = "agent-name", value_name = "NAME")]
    pub agent_name: Option<String>,

    /// Which frontend to launch.
    #[arg(long = "frontend", value_enum, default_value_t = Frontend::default())]
    pub frontend: Frontend,

    /// Working directory to run the agent in (defaults to current directory).
    #[arg(long = "cwd", short = 'C', value_name = "DIR")]
    pub cwd: Option<PathBuf>,

    /// Model override forwarded to the frontend.
    #[arg(long = "model", short = 'm', value_name = "MODEL")]
    pub model: Option<String>,

    /// Skip all confirmation prompts and execute commands without sandboxing
    /// (alias of codex's `--dangerously-bypass-approvals-and-sandbox`).
    #[arg(long = "yolo", default_value_t = false)]
    pub yolo: bool,

    /// WebUI only: host address to bind on (default 127.0.0.1, loopback only).
    /// Non-loopback hosts are refused unless/until WS auth is added.
    #[arg(long = "host", value_name = "HOST", default_value = "127.0.0.1")]
    pub host: String,

    /// WebUI only: TCP port to bind on. Omit (or pass 0) for an ephemeral
    /// port chosen by the OS.
    #[arg(long = "port", value_name = "PORT")]
    pub port: Option<u16>,
}

impl From<Cli> for RuntimeConfig {
    fn from(cli: Cli) -> Self {
        Self {
            agent_name: cli.agent_name,
            frontend: cli.frontend,
            cwd: cli.cwd,
            model: cli.model,
            yolo: cli.yolo,
            host: cli.host,
            port: cli.port,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontend_default_is_tui() {
        assert_eq!(Frontend::default(), Frontend::Tui);
    }

    #[test]
    fn frontend_serde_lowercase() {
        // Serialize
        assert_eq!(serde_json::to_string(&Frontend::Tui).unwrap(), "\"tui\"");
        assert_eq!(serde_json::to_string(&Frontend::Gui).unwrap(), "\"gui\"");
        assert_eq!(
            serde_json::to_string(&Frontend::Webui).unwrap(),
            "\"webui\""
        );

        // Deserialize (round-trip)
        let tui: Frontend = serde_json::from_str("\"tui\"").unwrap();
        assert_eq!(tui, Frontend::Tui);
        let gui: Frontend = serde_json::from_str("\"gui\"").unwrap();
        assert_eq!(gui, Frontend::Gui);
        let webui: Frontend = serde_json::from_str("\"webui\"").unwrap();
        assert_eq!(webui, Frontend::Webui);
    }

    #[test]
    fn cli_defaults_to_tui_frontend() {
        let cli = Cli::parse_from(["lemurclaw"]);
        assert_eq!(cli.frontend, Frontend::Tui);
        assert!(!cli.yolo);
        assert!(cli.model.is_none());
        assert!(cli.cwd.is_none());
        assert!(cli.agent_name.is_none());
    }

    #[test]
    fn cli_parses_frontend_flag() {
        let cli = Cli::parse_from(["lemurclaw", "--frontend", "webui"]);
        assert_eq!(cli.frontend, Frontend::Webui);
    }

    #[test]
    fn cli_into_runtime_config_preserves_fields() {
        let cli = Cli::parse_from([
            "lemurclaw",
            "--frontend",
            "gui",
            "--agent-name",
            "ava",
            "--model",
            "gpt-5",
            "--yolo",
        ]);
        let cfg: RuntimeConfig = cli.into();
        assert_eq!(cfg.frontend, Frontend::Gui);
        assert_eq!(cfg.agent_name.as_deref(), Some("ava"));
        assert_eq!(cfg.model.as_deref(), Some("gpt-5"));
        assert!(cfg.yolo);
    }

    #[test]
    fn cli_defaults_host_to_loopback_and_port_to_none() {
        let cli = Cli::parse_from(["lemurclaw"]);
        assert_eq!(cli.host, "127.0.0.1");
        assert!(cli.port.is_none());
    }

    #[test]
    fn cli_parses_host_and_port_flags() {
        let cli = Cli::parse_from([
            "lemurclaw",
            "--frontend",
            "webui",
            "--host",
            "0.0.0.0",
            "--port",
            "8080",
        ]);
        assert_eq!(cli.host, "0.0.0.0");
        assert_eq!(cli.port, Some(8080));
        let cfg: RuntimeConfig = cli.into();
        assert_eq!(cfg.host, "0.0.0.0");
        assert_eq!(cfg.port, Some(8080));
    }

    #[test]
    fn cli_parses_port_zero_as_explicit_ephemeral() {
        // Users can pass --port 0 to force the OS to pick a free port, vs.
        // omitting --port entirely. Both yield `None`-equivalent behavior at
        // bind time, but the parsed shape differs: omitted = None, 0 = Some(0).
        let cli = Cli::parse_from(["lemurclaw", "--port", "0"]);
        assert_eq!(cli.port, Some(0));
    }
}
