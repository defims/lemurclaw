//! Lemurclaw top-level configuration: the frontend selector and the runtime
//! config assembled from CLI args.
//!
//! `lemurclaw` is a thin launcher that currently passes straight through to
//! `codex_tui::run_main` for the `tui` frontend; `gui` and `webui` are stubs.

use std::path::PathBuf;

use clap::Parser;
use serde::{Deserialize, Serialize};

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
    /// Browser-based UI over WebSocket. Not implemented yet.
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
}

/// Top-level lemurclaw CLI.
///
/// Only the lemurclaw-specific flags live here. The TUI path re-parses the
/// full `codex_tui::Cli` from argv so it picks up every flag codex supports.
#[derive(Parser, Debug, Clone)]
#[command(name = "lemurclaw", version, about = "Lemurclaw launcher (TUI/GUI/WebUI)")]
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
}

impl From<Cli> for RuntimeConfig {
    fn from(cli: Cli) -> Self {
        Self {
            agent_name: cli.agent_name,
            frontend: cli.frontend,
            cwd: cli.cwd,
            model: cli.model,
            yolo: cli.yolo,
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
        assert_eq!(serde_json::to_string(&Frontend::Webui).unwrap(), "\"webui\"");

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
}
