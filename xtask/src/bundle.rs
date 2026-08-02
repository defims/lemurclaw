//! Phase 2: merge the 23 fine-grained `lemurclaw-utils-*` crates in `lemurclaw-rs/`
//! into a single `lemurclaw-utils` crate, where each former crate becomes a
//! `pub mod <name>` submodule.
//!
//! This is a post-hoc transform on the `lemurclaw-rs/` workspace emitted by Phase 1
//! (`xtask publish rename`), mirroring the Phase 1.5 fork-rewire pattern. The
//! source tree `codex-rs/` is never touched, so upstream changes can be merged
//! by re-running the pipeline: `publish rename && publish bundle && publish fork
//! rewire`.
//!
//! ## Why submodule namespacing
//!
//! Each sub-crate keeps its own `pub mod` namespace
//! (`lemurclaw_utils::absolute_path::AbsolutePathBuf`), so the 23 public API
//! surfaces never collide and consumers get an unambiguous import path. This
//! is a crate→module path rewrite, not a 1:1 identifier rename: the crate
//! identifier `lemurclaw_utils_absolute_path` becomes the two-segment path
//! `lemurclaw_utils::absolute_path`.

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One sub-crate being folded into a merged crate. Fields are `&'static str`
/// because the static utils cluster uses compile-time constants; the dynamic
/// core cluster will construct owned equivalents.
#[derive(Clone)]
pub(crate) struct SubCrate {
    /// Directory name under lemurclaw-rs/ (from the source layout).
    dir: &'static str,
    /// Package name assigned by Phase 1 rename.
    package: &'static str,
    /// Module name inside the merged crate (`pub mod <module>;`).
    module: &'static str,
}

/// The 23 sub-crates, in the order they appear in the source `Cargo.toml`
/// workspace member list. All 23 contribute to the ident→module mapping used
/// for downstream `.rs` rewrites (their `lemurclaw_utils_X` idents get
/// repointed to `lemurclaw_utils::<module>`).
///
/// Only the 17 in `MERGE_DIRS` are actually folded into the merged crate. The
/// other 6 (`approval-presets`, `cli`, `oss`, `output-truncation`, `plugins`,
/// `sandbox-summary`) depend on heavier `codex-*` workspace crates
/// (core/protocol/exec-server/...) that themselves depend on utils — merging
/// them would create a cyclic dependency, so they stay as standalone crates.
///
/// Note the `path-utils` quirk: its directory is `path-utils` but its package
/// name is `lemurclaw-utils-path` (matching upstream `codex-utils-path`), so
/// its crate ident is `lemurclaw_utils_path` and its module is `path`.
const SUB_CRATES: &[SubCrate] = &[
    SubCrate {
        dir: "absolute-path",
        package: "lemurclaw-utils-absolute-path",
        module: "absolute_path",
    },
    SubCrate {
        dir: "approval-presets",
        package: "lemurclaw-utils-approval-presets",
        module: "approval_presets",
    },
    SubCrate {
        dir: "cache",
        package: "lemurclaw-utils-cache",
        module: "cache",
    },
    SubCrate {
        dir: "cargo-bin",
        package: "lemurclaw-utils-cargo-bin",
        module: "cargo_bin",
    },
    SubCrate {
        dir: "cli",
        package: "lemurclaw-utils-cli",
        module: "cli",
    },
    SubCrate {
        dir: "elapsed",
        package: "lemurclaw-utils-elapsed",
        module: "elapsed",
    },
    SubCrate {
        dir: "fuzzy-match",
        package: "lemurclaw-utils-fuzzy-match",
        module: "fuzzy_match",
    },
    SubCrate {
        dir: "home-dir",
        package: "lemurclaw-utils-home-dir",
        module: "home_dir",
    },
    SubCrate {
        dir: "image",
        package: "lemurclaw-utils-image",
        module: "image",
    },
    SubCrate {
        dir: "json-to-toml",
        package: "lemurclaw-utils-json-to-toml",
        module: "json_to_toml",
    },
    SubCrate {
        dir: "oss",
        package: "lemurclaw-utils-oss",
        module: "oss",
    },
    SubCrate {
        dir: "output-truncation",
        package: "lemurclaw-utils-output-truncation",
        module: "output_truncation",
    },
    SubCrate {
        dir: "path-uri",
        package: "lemurclaw-utils-path-uri",
        module: "path_uri",
    },
    SubCrate {
        dir: "path-utils",
        package: "lemurclaw-utils-path",
        module: "path",
    },
    SubCrate {
        dir: "plugins",
        package: "lemurclaw-utils-plugins",
        module: "plugins",
    },
    SubCrate {
        dir: "pty",
        package: "lemurclaw-utils-pty",
        module: "pty",
    },
    SubCrate {
        dir: "readiness",
        package: "lemurclaw-utils-readiness",
        module: "readiness",
    },
    SubCrate {
        dir: "rustls-provider",
        package: "lemurclaw-utils-rustls-provider",
        module: "rustls_provider",
    },
    SubCrate {
        dir: "sandbox-summary",
        package: "lemurclaw-utils-sandbox-summary",
        module: "sandbox_summary",
    },
    SubCrate {
        dir: "sleep-inhibitor",
        package: "lemurclaw-utils-sleep-inhibitor",
        module: "sleep_inhibitor",
    },
    SubCrate {
        dir: "stream-parser",
        package: "lemurclaw-utils-stream-parser",
        module: "stream_parser",
    },
    SubCrate {
        dir: "string",
        package: "lemurclaw-utils-string",
        module: "string",
    },
    SubCrate {
        dir: "template",
        package: "lemurclaw-utils-template",
        module: "template",
    },
];

/// The merged crate's package name and lib identifier.
const MERGED_PACKAGE: &str = "lemurclaw-utils";
const MERGED_LIB_IDENT: &str = "lemurclaw_utils";

/// A merge plan: which crates fold into a single mega-crate, and the metadata
/// for the resulting crate. Parameterizes the bundle logic so the same code
/// path serves both the `utils` cluster and the `core` cluster.
pub(crate) struct Cluster {
    /// Short identifier for this cluster (e.g. "core", "extensions").
    /// Used to dispatch cluster-specific post-merge fixups.
    pub name: &'static str,
    /// Subdirectory under `lemurclaw-rs/` where the source crates live
    /// (e.g. `"utils"`). Empty string means crates live at the lemurclaw-rs/ root.
    pub source_subdir: &'static str,
    /// Workspace member path for the merged crate relative to lemurclaw-rs/
    /// (e.g. `"utils/utils"` or `"core"`).
    pub merged_member_path: &'static str,
    /// Package name of the merged crate (e.g. `"lemurclaw-utils"`).
    pub merged_package: &'static str,
    /// Lib identifier of the merged crate (e.g. `"lemurclaw_utils"`).
    pub merged_lib_ident: &'static str,
    /// The sub-crates to fold in.
    pub members: Vec<SubCrate>,
}

impl Cluster {
    /// The lib identifier prefix that downstream crates use to reference the
    /// sub-crates (e.g. `lemurclaw_utils_<sub>`). Used by the `.rs` rewriter.
    fn sub_crate_ident_prefix(&self) -> String {
        format!("{}_", self.merged_lib_ident)
    }

    /// Whether a dep package key (e.g. `lemurclaw-utils-absolute-path`) names
    /// one of THIS cluster's members.
    fn contains_package(&self, key: &str) -> bool {
        self.members.iter().any(|sc| sc.package == key)
    }

    /// Build the ident→module lookup table for this cluster's members.
    fn ident_to_module(&self) -> BTreeMap<String, String> {
        self.members
            .iter()
            .map(|sc| {
                let ident = sc.package.replace('-', "_");
                (ident, sc.module.to_string())
            })
            .collect()
    }

    /// The set of module names for this cluster (used by self-ref fix).
    fn module_names(&self) -> std::collections::HashSet<String> {
        self.members
            .iter()
            .map(|sc| sc.module.to_string())
            .collect()
    }
}

/// Construct the `utils` cluster: 23 known utils crates, of which 17 are
/// merged (the 6 cycle-causing ones stay standalone — see MERGE_DIRS).
pub(crate) fn utils_cluster() -> Cluster {
    Cluster {
        name: "utils",
        source_subdir: "utils",
        merged_member_path: "utils/utils",
        merged_package: MERGED_PACKAGE,
        merged_lib_ident: MERGED_LIB_IDENT,
        members: merge_crates()
            .into_iter()
            .map(|sc| SubCrate {
                dir: sc.dir,
                package: sc.package,
                module: sc.module,
            })
            .collect(),
    }
}

/// Construct the `core` cluster: codex-core's transitive closure (normal deps)
/// plus extra crates that are cycle-safe to fold in (utils not in the closure,
/// oss, lmstudio, ollama). Dynamically computed via `cargo metadata` against
/// the SOURCE codex-rs/ tree, so it self-adapts when upstream adds/removes
/// crates.
pub(crate) fn core_cluster() -> Result<Cluster> {
    let repo_root = locate_repo_root()?;
    let codex_root = repo_root.join("codex-rs");
    let graph = load_dep_graph(&codex_root)?;

    // 1. Compute core's transitive closure (normal deps only, recursively).
    let closure = transitive_closure(&graph, "codex-core");
    if !closure.contains("codex-core") {
        anyhow::bail!("codex-core not found in dependency graph");
    }

    // 2. Find extra crates to fold in: crates NOT in the closure whose entire
    //    codex-* dependency surface is already in the closure ∪ the extras set.
    //    This discovers the 9 cycle-free utils + oss + lmstudio + ollama.
    //    We iterate to a fixed point because oss needs lmstudio/ollama (which
    //    themselves need to be added first).
    let mut merge_set = closure.clone();
    loop {
        let mut added = false;
        for (pkg, info) in &graph {
            if merge_set.contains(pkg.as_str()) {
                continue;
            }
            // Restrict candidates to utils crates (to make utils disappear) plus
            // lmstudio/ollama (needed to break oss's cycle). Without this gate,
            // the fixed-point loop would pull in nearly every codex-* crate.
            let is_candidate =
                pkg.starts_with("codex-utils-") || pkg == "codex-lmstudio" || pkg == "codex-ollama";
            if !is_candidate {
                continue;
            }
            // All this crate's normal codex-* deps must be in merge_set.
            let all_in = info
                .deps
                .iter()
                .filter(|d| d.starts_with("codex-"))
                .all(|d| merge_set.contains(d.as_str()));
            if all_in {
                merge_set.insert(pkg.clone());
                added = true;
            }
        }
        if !added {
            break;
        }
    }

    // Remove proc-macro crates from the merge set — they cannot be folded into
    // another crate (Rust forbids using a proc-macro from the defining crate).
    // They stay as external deps of lemurclaw-core.
    let proc_macros: Vec<String> = merge_set
        .iter()
        .filter(|p| graph.get(*p).is_some_and(|info| info.is_proc_macro))
        .cloned()
        .collect();
    for pm in &proc_macros {
        merge_set.remove(pm);
        eprintln!(
            "note: excluding proc-macro crate {} from merge (stays external)",
            pm
        );
    }

    // 3. Warn about potential cycle risks (back-edges from merge-set members to
    //    external crates that back-depend on the merge set).
    for pkg in &merge_set {
        let info = &graph[pkg];
        for dep in &info.deps {
            if !dep.starts_with("codex-") || merge_set.contains(dep.as_str()) {
                continue;
            }
            if let Some(dep_info) = graph.get(dep) {
                for b in dep_info
                    .deps
                    .iter()
                    .filter(|d| merge_set.contains(d.as_str()))
                {
                    eprintln!(
                        "warn: cycle risk: {} (merge) → {} (external) → {} (back to merge)",
                        pkg, dep, b
                    );
                }
            }
        }
    }

    // 4. Build SubCrate entries using the REAL source directory from cargo
    //    metadata (not a guessed name). core itself is excluded.
    let mut members: Vec<SubCrate> = Vec::new();
    for pkg in &merge_set {
        if pkg == "codex-core" {
            continue;
        }
        let info = &graph[pkg];
        let lemurclaw_pkg = format!("lemurclaw-{}", &pkg["codex-".len()..]);
        let module = pkg["codex-".len()..].replace('-', "_");
        members.push(SubCrate {
            dir: info.dir.clone().leak(),
            package: lemurclaw_pkg.leak(),
            module: module.leak(),
        });
    }
    members.sort_by_key(|sc| sc.dir.to_string());

    println!(
        "core cluster: {} crates in merge set (core + {} members)",
        merge_set.len(),
        members.len()
    );

    Ok(Cluster {
        name: "core",
        source_subdir: "", // core members live at lemurclaw-rs/ root level
        merged_member_path: "core",
        merged_package: "lemurclaw-core",
        merged_lib_ident: "lemurclaw_core",
        members,
    })
}

/// Construct the `extensions` cluster: 9 extension crates under lemurclaw-rs/ext/
/// merged into `lemurclaw-extensions`. No host crate -- creates a fresh merged
/// crate at lemurclaw-rs/ext/extensions/.
pub(crate) fn extensions_cluster() -> Cluster {
    Cluster {
        name: "extensions",
        source_subdir: "ext",
        merged_member_path: "ext/extensions",
        merged_package: "lemurclaw-extensions",
        merged_lib_ident: "lemurclaw_extensions",
        members: vec![
            SubCrate {
                dir: "agent",
                package: "lemurclaw-agent-extension",
                module: "agent",
            },
            SubCrate {
                dir: "connectors",
                package: "lemurclaw-connectors-extension",
                module: "connectors",
            },
            SubCrate {
                dir: "goal",
                package: "lemurclaw-goal-extension",
                module: "goal",
            },
            SubCrate {
                dir: "guardian",
                package: "lemurclaw-guardian",
                module: "guardian",
            },
            SubCrate {
                dir: "image-generation",
                package: "lemurclaw-image-generation-extension",
                module: "image_generation",
            },
            SubCrate {
                dir: "mcp",
                package: "lemurclaw-mcp-extension",
                module: "mcp",
            },
            SubCrate {
                dir: "memories",
                package: "lemurclaw-memories-extension",
                module: "memories",
            },
            SubCrate {
                dir: "skills",
                package: "lemurclaw-skills-extension",
                module: "skills",
            },
            SubCrate {
                dir: "web-search",
                package: "lemurclaw-web-search-extension",
                module: "web_search",
            },
        ],
    }
}

/// Construct the `server` cluster: 12 server-layer crates merged into
/// `lemurclaw-server`. No host crate -- creates a fresh merged crate at
/// lemurclaw-rs/server/.
pub(crate) fn server_cluster() -> Cluster {
    Cluster {
        name: "server",
        source_subdir: "",
        merged_member_path: "server",
        merged_package: "lemurclaw-server",
        merged_lib_ident: "lemurclaw_server",
        members: vec![
            SubCrate {
                dir: "app-server",
                package: "lemurclaw-app-server",
                module: "app_server",
            },
            SubCrate {
                dir: "app-server-transport",
                package: "lemurclaw-app-server-transport",
                module: "app_server_transport",
            },
            SubCrate {
                dir: "app-server-daemon",
                package: "lemurclaw-app-server-daemon",
                module: "app_server_daemon",
            },
            SubCrate {
                dir: "app-server-client",
                package: "lemurclaw-app-server-client",
                module: "app_server_client",
            },
            SubCrate {
                dir: "app-server-test-client",
                package: "lemurclaw-app-server-test-client",
                module: "app_server_test_client",
            },
            SubCrate {
                dir: "backend-client",
                package: "lemurclaw-backend-client",
                module: "backend_client",
            },
            SubCrate {
                dir: "codex-backend-openapi-models",
                package: "lemurclaw-backend-openapi-models",
                module: "backend_openapi_models",
            },
            SubCrate {
                dir: "cloud-config",
                package: "lemurclaw-cloud-config",
                module: "cloud_config",
            },
            SubCrate {
                dir: "cloud-tasks-client",
                package: "lemurclaw-cloud-tasks-client",
                module: "cloud_tasks_client",
            },
            SubCrate {
                dir: "file-watcher",
                package: "lemurclaw-file-watcher",
                module: "file_watcher",
            },
            SubCrate {
                dir: "external-agent-migration",
                package: "lemurclaw-external-agent-migration",
                module: "external_agent_migration",
            },
            SubCrate {
                dir: "memories/write",
                package: "lemurclaw-memories-write",
                module: "memories_write",
            },
            // The following 6 crates were moved from the cli cluster to avoid a
            // circular dependency: server's app-server-client and app-server both
            // depend on arg0, uds, chatgpt, home; arg0 depends on linux-sandbox;
            // linux-sandbox depends on process-hardening.
            SubCrate {
                dir: "arg0",
                package: "lemurclaw-arg0",
                module: "arg0",
            },
            SubCrate {
                dir: "chatgpt",
                package: "lemurclaw-chatgpt",
                module: "chatgpt",
            },
            SubCrate {
                dir: "codex-home",
                package: "lemurclaw-home",
                module: "home",
            },
            SubCrate {
                dir: "uds",
                package: "lemurclaw-uds",
                module: "uds",
            },
            SubCrate {
                dir: "linux-sandbox",
                package: "lemurclaw-linux-sandbox",
                module: "linux_sandbox",
            },
            SubCrate {
                dir: "process-hardening",
                package: "lemurclaw-process-hardening",
                module: "process_hardening",
            },
        ],
    }
}

/// Construct the `tui` cluster: ansi-escape + message-history merged into the
/// existing `lemurclaw-tui` crate (host-crate pattern -- lemurclaw-rs/tui/ already
/// exists, its src/ migrates to src/tui_internal/).
pub(crate) fn tui_cluster() -> Cluster {
    Cluster {
        name: "tui",
        source_subdir: "",
        merged_member_path: "tui",
        merged_package: "lemurclaw-tui",
        merged_lib_ident: "lemurclaw_tui",
        members: vec![
            SubCrate {
                dir: "ansi-escape",
                package: "lemurclaw-ansi-escape",
                module: "ansi_escape",
            },
            SubCrate {
                dir: "message-history",
                package: "lemurclaw-message-history",
                module: "message_history",
            },
        ],
    }
}

/// Construct the `cli` cluster: 14 CLI-layer crates merged into the existing
/// `lemurclaw-cli` crate (host-crate pattern -- lemurclaw-rs/cli/ already exists,
/// its src/ migrates to src/cli_internal/).
pub(crate) fn cli_cluster() -> Cluster {
    Cluster {
        name: "cli",
        source_subdir: "",
        merged_member_path: "cli",
        merged_package: "lemurclaw",
        merged_lib_ident: "lemurclaw",
        members: vec![
            SubCrate {
                dir: "cloud-tasks",
                package: "lemurclaw-cloud-tasks",
                module: "cloud_tasks",
            },
            SubCrate {
                dir: "cloud-tasks-mock-client",
                package: "lemurclaw-cloud-tasks-mock-client",
                module: "cloud_tasks_mock_client",
            },
            SubCrate {
                dir: "code-mode-host",
                package: "lemurclaw-code-mode-host",
                module: "code_mode_host",
            },
            SubCrate {
                dir: "core-api",
                package: "lemurclaw-core-api",
                module: "core_api",
            },
            SubCrate {
                dir: "exec",
                package: "lemurclaw-exec",
                module: "exec",
            },
            SubCrate {
                dir: "mcp-server",
                package: "lemurclaw-mcp-server",
                module: "mcp_server",
            },
            SubCrate {
                dir: "responses-api-proxy",
                package: "lemurclaw-responses-api-proxy",
                module: "responses_api_proxy",
            },
            SubCrate {
                dir: "stdio-to-uds",
                package: "lemurclaw-stdio-to-uds",
                module: "stdio_to_uds",
            },
        ],
    }
}

/// Load the codex-* dependency graph from `cargo metadata`. Returns a map of
/// package name → normal (non-dev, non-build) codex-* dependency names.
/// One package's info from cargo metadata: its dependency list and the
/// directory it lives in (relative to codex-rs/).
struct PackageInfo {
    deps: Vec<String>,
    /// Directory relative to codex-rs/ (e.g. "api", "ext/items", "utils/absolute-path").
    dir: String,
    /// True if this is a proc-macro crate (cannot be merged into another crate).
    is_proc_macro: bool,
}

/// Load codex-* package info from `cargo metadata`. Returns a map of package
/// name → PackageInfo (normal deps only + real source directory).
fn load_dep_graph(codex_root: &Path) -> Result<std::collections::HashMap<String, PackageInfo>> {
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--format-version=1",
            "--no-deps",
            "--manifest-path",
            codex_root.join("Cargo.toml").to_str().unwrap(),
        ])
        .output()
        .context("run cargo metadata")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("cargo metadata failed: {}", stderr);
    }
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("parse metadata json")?;
    let packages = value
        .get("packages")
        .and_then(|v| v.as_array())
        .context("metadata.packages missing")?;

    let codex_root_str = codex_root.to_string_lossy().into_owned();
    let mut graph: std::collections::HashMap<String, PackageInfo> =
        std::collections::HashMap::new();
    for pkg in packages {
        let name = pkg.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.is_empty() {
            continue;
        }
        let mut deps = Vec::new();
        if let Some(dep_arr) = pkg.get("dependencies").and_then(|v| v.as_array()) {
            for dep in dep_arr {
                // kind == null → normal dependency (not dev, not build).
                let kind = dep.get("kind").and_then(|v| v.as_str());
                if kind.is_some() {
                    continue;
                }
                let dep_name = dep.get("name").and_then(|v| v.as_str()).unwrap_or("");
                if dep_name.starts_with("codex-") {
                    deps.push(dep_name.to_string());
                }
            }
        }
        // Extract the directory relative to codex-rs/ from manifest_path.
        let manifest_path = pkg
            .get("manifest_path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let dir = manifest_path
            .strip_prefix(&format!("{}/", codex_root_str))
            .and_then(|rel| {
                rel.strip_suffix("/Cargo.toml")
                    .or_else(|| rel.strip_suffix("Cargo.toml"))
            })
            .unwrap_or("")
            .trim_end_matches('/')
            .to_string();
        // Detect proc-macro crates (any target with kind "proc-macro").
        let is_proc_macro = pkg
            .get("targets")
            .and_then(|v| v.as_array())
            .is_some_and(|targets| {
                targets.iter().any(|t| {
                    t.get("kind")
                        .and_then(|v| v.as_array())
                        .is_some_and(|kinds| kinds.iter().any(|k| k.as_str() == Some("proc-macro")))
                })
            });
        graph.insert(
            name.to_string(),
            PackageInfo {
                deps,
                dir,
                is_proc_macro,
            },
        );
    }
    Ok(graph)
}

/// Compute the transitive closure of `start` (normal deps, recursive).
fn transitive_closure(
    graph: &std::collections::HashMap<String, PackageInfo>,
    start: &str,
) -> std::collections::HashSet<String> {
    let mut closure = std::collections::HashSet::new();
    let mut stack = vec![start.to_string()];
    while let Some(node) = stack.pop() {
        if !closure.insert(node.clone()) {
            continue;
        }
        if let Some(info) = graph.get(&node) {
            for d in &info.deps {
                if !closure.contains(d) {
                    stack.push(d.clone());
                }
            }
        }
    }
    closure
}

/// Map a codex-* package name to its (publish_dir, module_name).
fn locate_repo_root() -> Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if manifest_dir.join("..").join("codex-rs").exists() {
        return Ok(manifest_dir.join("..").canonicalize()?);
    }
    let cwd = std::env::current_dir().context("get cwd")?;
    let mut candidate = cwd;
    loop {
        if candidate.join("codex-rs").is_dir() {
            return Ok(candidate);
        }
        if !candidate.pop() {
            break;
        }
    }
    anyhow::bail!("could not locate repo root containing codex-rs/")
}

/// Directories of sub-crates that are actually folded into the merged crate.
/// Excludes the 6 cycle-causing crates (those depending on heavier `codex-*`
/// crates that transitively depend on utils). Returns only the `SubCrate`
/// entries whose `dir` is in the merge set.
fn merge_crates() -> Vec<&'static SubCrate> {
    /// The 17 dirs that get merged (all except the 6 cycle-causing ones).
    const MERGE_DIRS: &[&str] = &[
        "absolute-path",
        "cache",
        "cargo-bin",
        "elapsed",
        "fuzzy-match",
        "home-dir",
        "image",
        "json-to-toml",
        "path-uri",
        "path-utils",
        "pty",
        "readiness",
        "rustls-provider",
        "sleep-inhibitor",
        "stream-parser",
        "string",
        "template",
    ];
    SUB_CRATES
        .iter()
        .filter(|sc| MERGE_DIRS.contains(&sc.dir))
        .collect()
}

/// Directories of sub-crates that stay standalone (not merged) because merging
/// them would create a cyclic dependency. They remain as separate
/// `lemurclaw-utils-*` crates but still get their downstream refs rewritten.
fn standalone_crates() -> Vec<&'static SubCrate> {
    SUB_CRATES
        .iter()
        .filter(|sc| !merge_crates().iter().any(|m| m.dir == sc.dir))
        .collect()
}

/// Build the lookup table mapping a former crate identifier
/// (`lemurclaw_utils_absolute_path`) to its submodule name (`absolute_path`).
/// Used by the `.rs` rewrite pass. Covers all 23 (merged + standalone), since
/// standalone crates' refs to merged crates also need repointing.
fn ident_to_module() -> BTreeMap<&'static str, &'static str> {
    SUB_CRATES
        .iter()
        .map(|sc| {
            // crate ident = package name with dashes → underscores
            let ident: &'static str = sc.package.replace('-', "_").leak();
            (ident, sc.module)
        })
        .collect()
}

/// `xtask publish bundle [--target <utils|core>] [--dry-run]`
///
/// Merges a cluster of `lemurclaw-rs/` crates into a single mega-crate. With
/// `--dry-run`, prints the plan and exits without modifying any files.
pub(crate) fn run(cluster: &Cluster, dry_run: bool) -> Result<()> {
    let repo_root = locate_repo_root()?;
    let publish_root = repo_root.join("lemurclaw-rs");
    let source_root = if cluster.source_subdir.is_empty() {
        publish_root.clone()
    } else {
        publish_root.join(cluster.source_subdir)
    };
    // The merged crate lives one level inside source_root, named after the
    // last segment of merged_member_path (e.g. "utils" or "core").
    let merged_leaf = cluster
        .merged_member_path
        .rsplit('/')
        .next()
        .unwrap_or("merged");
    let merged_dir = source_root.join(merged_leaf);
    println!(
        "bundle {} {}\n  source: {}\n  target: {}\n",
        cluster.merged_package,
        if dry_run { "(dry-run)" } else { "" },
        source_root.display(),
        merged_dir.display(),
    );

    if !source_root.is_dir() {
        anyhow::bail!(
            "{} missing — run `xtask publish rename` first",
            source_root.display()
        );
    }

    // Step 0: verify all member sub-crate dirs exist.
    let mut missing = Vec::new();
    for sc in &cluster.members {
        if !source_root.join(sc.dir).is_dir() {
            missing.push(sc.dir);
        }
    }
    if !missing.is_empty() {
        anyhow::bail!(
            "missing {} member dir(s) under {}: {}",
            missing.len(),
            source_root.display(),
            missing.join(", ")
        );
    }
    println!(
        "Found all {} members under {}.",
        cluster.members.len(),
        source_root.display()
    );

    if dry_run {
        return print_dry_run_plan(cluster, &source_root);
    }

    // For the utils cluster, merged_dir is a fresh new dir. For the core
    // cluster, merged_dir (lemurclaw-rs/core/) ALREADY EXISTS — it's core's own
    // source tree from Phase 1 rename. We handle core's own src/ specially.
    let host_crate_exists = cluster.source_subdir.is_empty() && merged_dir.is_dir();
    if merged_dir.exists() && !host_crate_exists {
        anyhow::bail!(
            "{} already exists — remove it before re-running (or it was left from a partial run)",
            merged_dir.display()
        );
    }

    // Step 1: collect each member's dependencies, then create the merged crate
    // dir with a synthesized Cargo.toml and lib.rs.
    println!(
        "Merging {} members into {}.",
        cluster.members.len(),
        cluster.merged_package
    );
    // For the core cluster, the merged crate dir (lemurclaw-rs/core/) already holds
    // core's own source. Migrate core's own src/ into a submodule first, so
    // create_merged_crate can overwrite lib.rs cleanly.
    let host_module = if host_crate_exists {
        let host_mod = format!("{}_internal", cluster.name);
        let host_mod: &str = host_mod.leak();
        migrate_host_crate_src(&merged_dir, host_mod)?;
        Some(host_mod)
    } else {
        None
    };

    let agg = collect_aggregate_deps(&source_root, &cluster.members, cluster)?;
    // When there's a host crate, include its deps in the aggregate too.
    let agg = if let Some(hm) = host_module {
        let mut agg = agg;
        let host_path = merged_dir.join("src").join(hm).join("Cargo.toml");
        if host_path.exists() {
            // The host's Cargo.toml was moved into the submodule; read it.
            let raw = fs::read_to_string(&host_path)
                .with_context(|| format!("read {}", host_path.display()))?;
            if let Ok(doc) = toml::from_str::<toml::Value>(&raw) {
                if let Some(table) = doc.as_table() {
                    if let Some(deps) = table.get("dependencies").and_then(|v| v.as_table()) {
                        merge_dep_table(&mut agg.deps, deps, cluster);
                    }
                }
            }
        }
        agg
    } else {
        agg
    };
    create_merged_crate(&merged_dir, &agg, cluster, host_module)?;

    // Step 2: move each member's src/ into the merged crate.
    for sc in &cluster.members {
        migrate_subcrate_src(&source_root.join(sc.dir), &merged_dir, sc.module)?;
    }

    // Step 2.5: handle member build.rs files before deletion.
    // Non-trivial build.rs (e.g. cli's macOS -ObjC link arg) that belong to
    // the HOST crate are already at the crate root and unaffected. Member
    // build.rs files (e.g. linux-sandbox's trivial rerun-if-env-changed) are
    // not moved by migrate_subcrate_src and will be deleted with their crate
    // dir. For non-trivial member build.rs, warn so they can be handled manually.
    for sc in &cluster.members {
        let build_rs = source_root.join(sc.dir).join("build.rs");
        if build_rs.is_file() {
            let raw = fs::read_to_string(&build_rs)
                .with_context(|| format!("read {}", build_rs.display()))?;
            // Trivial build.rs: only rerun-if-env-changed directives — safe to discard.
            let is_trivial = raw.lines().all(|line| {
                let t = line.trim();
                t.is_empty()
                    || t.starts_with("fn main()")
                    || t.contains("cargo:rerun-if-env-changed")
                    || t == "}"
            });
            if !is_trivial {
                eprintln!(
                    "warn: member {} has non-trivial build.rs — contents will be lost after merge",
                    sc.dir
                );
            }
        }
    }

    // Step 3: delete the merged member dirs (standalone ones stay).
    for sc in &cluster.members {
        let dir = source_root.join(sc.dir);
        fs::remove_dir_all(&dir).with_context(|| format!("delete {}", dir.display()))?;
    }
    println!("Deleted {} member dirs.", cluster.members.len());

    // Step 4: rewrite lemurclaw-rs/Cargo.toml (members + workspace deps).
    rewrite_publish_manifest(&publish_root, cluster)?;

    // Step 5: rewrite .rs imports + downstream Cargo.toml dep lines.
    //
    // CRITICAL ORDER: fix_self_refs runs BEFORE rewrite_rs.
    // At this point, member cross-refs are still `lemurclaw_X::` (not yet
    // rewritten to `crate::X::`). So any `crate::X::` in the source
    // unambiguously means the host's OWN module X — no per-item guessing
    // needed. We prefix all `crate::<local_mod>` to `crate::<module>::<local_mod>`.
    //
    //   5a. Fix self-refs (prefix crate::X → crate::<module>::X for local X)
    //   5b. Fix include_str!/include_bytes! paths
    //   5c. Rewrite cross-module refs (lemurclaw_X → crate::X)
    //   5d. Post-merge deterministic fixups
    //   5e. Downstream rewrite
    for sc in &cluster.members {
        fix_self_refs_in_submodule(&merged_dir.join("src").join(sc.module), sc.module, cluster)?;
        fix_include_paths_in_submodule(&merged_dir.join("src").join(sc.module))?;
    }
    if let Some(hm) = host_module {
        fix_self_refs_in_submodule(&merged_dir.join("src").join(hm), hm, cluster)?;
        fix_include_paths_in_submodule(&merged_dir.join("src").join(hm))?;
    }
    // NOW rewrite cross-module refs (lemurclaw_X → crate::X).
    rewrite_rust_files_in(&merged_dir.join("src"), RewriteScope::IntraCrate, cluster)?;
    // Post-merge deterministic fixups for non-collision edge cases.
    post_merge_fixups(&merged_dir.join("src"), cluster)?;
    rewrite_downstream(&publish_root, &merged_dir, cluster)?;

    println!("\n✓ bundle {} complete.", cluster.merged_package);
    println!(
        "Verify: cd publish && cargo check -p {}",
        cluster.merged_package
    );
    Ok(())
}

/// Print the migration plan without touching the filesystem.
fn print_dry_run_plan(cluster: &Cluster, source_root: &Path) -> Result<()> {
    let merged_leaf = cluster
        .merged_member_path
        .rsplit('/')
        .next()
        .unwrap_or("merged");
    let merged_dir = source_root.join(merged_leaf);
    println!("Plan:");
    println!(
        "  Merging {} members into {}:",
        cluster.members.len(),
        cluster.merged_package
    );
    println!();
    println!("  1. Create merged crate at {}", merged_dir.display());
    println!("     package.name = \"{}\"", cluster.merged_package);
    println!("     [lib].name   = \"{}\"", cluster.merged_lib_ident);
    println!(
        "     src/lib.rs   = {} `pub mod <name>;` lines\n",
        cluster.members.len()
    );

    println!("  2. Migrate each member's src/ into a submodule:");
    let mut total_files = 0usize;
    for sc in &cluster.members {
        let src = source_root.join(sc.dir).join("src");
        let count = count_rs_files(&src)?;
        total_files += count;
        println!(
            "     {:<24} (pkg {:<36}) → mod {} [{} files]",
            sc.dir, sc.package, sc.module, count
        );
    }
    println!("     total: {} .rs files\n", total_files);

    println!("  3. Delete the {} member dirs.\n", cluster.members.len());

    println!("  4. Rewrite lemurclaw-rs/Cargo.toml:");
    println!(
        "     [workspace.members]: {} merged → 1 {}",
        cluster.members.len(),
        cluster.merged_member_path
    );
    println!(
        "     [workspace.dependencies]: {} merged → 1 {}\n",
        cluster.members.len(),
        cluster.merged_package
    );

    println!("  5. Rewrite downstream crates:");
    println!(
        "     .rs:        use {}_<sub>::X  →  use {}::<sub>::X",
        cluster.merged_lib_ident, cluster.merged_lib_ident
    );
    println!(
        "     Cargo.toml: {}-* = {{ ws }}  →  {} = {{ ws }} (dedup)",
        cluster.merged_package, cluster.merged_package
    );
    let table = cluster.ident_to_module();
    println!("\n     ident → module mapping ({} entries):", table.len());
    for (ident, module) in &table {
        println!("       {} → ::{}", ident, module);
    }

    // Note: #[path] attributes are preserved — moving the whole src/ dir keeps
    // sibling-relative paths intact.
    println!("\n  Note: internal #[path = \"..._tests.rs\"] attributes are preserved");
    println!("        (the whole src/ dir moves, so sibling-relative paths survive).");
    println!("\nDry-run complete. Re-run without --dry-run to execute.");
    Ok(())
}

/// Count `.rs` files under a directory (recursive).
fn count_rs_files(dir: &Path) -> Result<usize> {
    if !dir.is_dir() {
        return Ok(0);
    }
    let mut count = 0usize;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            count += count_rs_files(&path)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            count += 1;
        }
    }
    Ok(count)
}

// ---------------------------------------------------------------------------
// Stage 2: dependency aggregation + crate creation + source migration.
// ---------------------------------------------------------------------------

/// Aggregated dependency tables collected across all 23 sub-crates, ready to
/// render into the merged crate's `Cargo.toml`.
struct AggregateDeps {
    /// `[dependencies]` — dep key → value (merged, features unified).
    deps: BTreeMap<String, toml::Value>,
    /// `[dev-dependencies]`.
    dev_deps: BTreeMap<String, toml::Value>,
    /// `[target.'cfg(...)'.dependencies]` — cfg expr → (dep key → value).
    target_deps: BTreeMap<String, BTreeMap<String, toml::Value>>,
    /// `[target.'cfg(...)'.dev-dependencies]` — cfg expr → (dep key → value).
    target_dev_deps: BTreeMap<String, BTreeMap<String, toml::Value>>,
}

/// Read every *merged* sub-crate's Cargo.toml and merge their dependency
/// tables. Deps pointing at sibling `lemurclaw-utils-*` workspace crates are
/// dropped — they become intra-crate references inside the merged crate (a
/// `pub mod` is in the same crate, so no dep declaration is needed). Other
/// `lemurclaw-*` workspace crates (protocol, core, etc.) are preserved.
fn collect_aggregate_deps(
    source_root: &Path,
    members: &[SubCrate],
    cluster: &Cluster,
) -> Result<AggregateDeps> {
    let mut agg = AggregateDeps {
        deps: BTreeMap::new(),
        dev_deps: BTreeMap::new(),
        target_deps: BTreeMap::new(),
        target_dev_deps: BTreeMap::new(),
    };
    for sc in members {
        let path = source_root.join(sc.dir).join("Cargo.toml");
        let raw = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let doc: toml::Value =
            toml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
        let table = doc.as_table().context("manifest is not a table")?;

        if let Some(deps) = table.get("dependencies").and_then(|v| v.as_table()) {
            merge_dep_table(&mut agg.deps, deps, cluster);
        }
        if let Some(deps) = table.get("dev-dependencies").and_then(|v| v.as_table()) {
            merge_dep_table(&mut agg.dev_deps, deps, cluster);
        }
        // Target-scoped deps: [target.'cfg(windows)'.dependencies] etc.
        if let Some(targets) = table.get("target").and_then(|v| v.as_table()) {
            for (cfg, scope) in targets {
                let scope = match scope.as_table() {
                    Some(t) => t,
                    None => continue,
                };
                if let Some(d) = scope.get("dependencies").and_then(|v| v.as_table()) {
                    let entry = agg.target_deps.entry(cfg.clone()).or_default();
                    merge_dep_table(entry, d, cluster);
                }
                if let Some(d) = scope.get("dev-dependencies").and_then(|v| v.as_table()) {
                    let entry = agg.target_dev_deps.entry(cfg.clone()).or_default();
                    merge_dep_table(entry, d, cluster);
                }
            }
        }
    }
    Ok(agg)
}

/// Merge a source dep table into the accumulator. Internal workspace crates
/// (`codex-*` / `lemurclaw-*`) are skipped — they become intra-crate refs.
/// When both old and new specify a dep, features arrays are unioned.
fn merge_dep_table(
    accum: &mut BTreeMap<String, toml::Value>,
    src: &toml::map::Map<String, toml::Value>,
    cluster: &Cluster,
) {
    for (key, val) in src {
        // Skip this cluster's member packages AND the merged package itself —
        // they become intra-crate refs (a `pub mod` is in the same crate).
        // Skipping the merged package prevents a self-dependency cycle.
        if cluster.contains_package(key) || key == cluster.merged_package {
            continue;
        }
        match accum.get(key) {
            None => {
                accum.insert(key.clone(), val.clone());
            }
            Some(existing) => {
                // Both present — unify features if either is a table.
                let merged = unify_dep_value(existing, val);
                accum.insert(key.clone(), merged);
            }
        }
    }
}

/// Unify two dep values for the same key. If both are inline tables, merge
/// their `features` arrays (deduped, preserving order). Otherwise the newer
/// value wins.
///
/// Feature unification is guarded: features are only unioned when both specs
/// agree on their version source. If one spec pins a `version = "X"` while the
/// other uses `workspace = true`, their features may not be compatible across
/// versions (e.g. reqwest 0.13's "rustls" feature doesn't exist in 0.12). In
/// that case we keep the workspace-pinned spec's features and drop the
/// version-pinned spec's features (the workspace version is authoritative for
/// the merged crate).
fn unify_dep_value(a: &toml::Value, b: &toml::Value) -> toml::Value {
    let (ta, tb) = match (a.as_table(), b.as_table()) {
        (Some(ta), Some(tb)) => (ta, tb),
        _ => return b.clone(),
    };
    let mut out = tb.clone();
    // Determine version source compatibility.
    let a_ws = ta.get("workspace").is_some();
    let b_ws = tb.get("workspace").is_some();
    let a_ver = ta.get("version").and_then(|v| v.as_str());
    let b_ver = tb.get("version").and_then(|v| v.as_str());
    // Compatible = both workspace, or both pin the same version.
    let compatible = (a_ws && b_ws) || (a_ver.is_some() && a_ver == b_ver);
    if compatible {
        // Union features.
        let fa = ta.get("features").and_then(|v| v.as_array());
        let fb = tb.get("features").and_then(|v| v.as_array());
        if fa.is_some() || fb.is_some() {
            let mut seen: Vec<toml::Value> = Vec::new();
            for f in fa.into_iter().flatten().chain(fb.into_iter().flatten()) {
                if !seen.iter().any(|s| s == f) {
                    seen.push(f.clone());
                }
            }
            if !seen.is_empty() {
                out.insert("features".to_string(), toml::Value::Array(seen));
            }
        }
    } else {
        // Version mismatch: prefer the workspace spec's features, drop the
        // version-pinned spec's features to avoid cross-version pollution.
        let ws_features = if a_ws {
            ta.get("features").and_then(|v| v.as_array())
        } else {
            tb.get("features").and_then(|v| v.as_array())
        };
        if let Some(fa) = ws_features {
            out.insert("features".to_string(), toml::Value::Array(fa.clone()));
        }
    }
    toml::Value::Table(out)
}

/// Create the merged crate directory with a synthesized Cargo.toml and lib.rs.
/// `host_module`: if Some, the merged crate's own former source lives in this
/// submodule (used for core, where lemurclaw-rs/core/ already exists).
fn create_merged_crate(
    merged_dir: &Path,
    agg: &AggregateDeps,
    cluster: &Cluster,
    host_module: Option<&str>,
) -> Result<()> {
    // For the core cluster, merged_dir already exists; only ensure src/ exists.
    if !merged_dir.is_dir() {
        fs::create_dir_all(merged_dir).context("create merged crate dir")?;
    }
    fs::create_dir_all(merged_dir.join("src")).context("create merged crate src/")?;

    // Cargo.toml
    let mut toml = String::new();
    toml.push_str("[package]\n");
    toml.push_str(&format!("name = \"{}\"\n", cluster.merged_package));
    toml.push_str("version.workspace = true\n");
    toml.push_str("edition.workspace = true\n");
    toml.push_str("license.workspace = true\n");
    toml.push_str("\n[lints]\nworkspace = true\n");

    render_dep_section(&mut toml, "dependencies", &agg.deps);
    render_dep_section(&mut toml, "dev-dependencies", &agg.dev_deps);
    for (cfg, deps) in &agg.target_deps {
        render_target_dep_section(&mut toml, cfg, "dependencies", deps);
    }
    for (cfg, deps) in &agg.target_dev_deps {
        render_target_dep_section(&mut toml, cfg, "dev-dependencies", deps);
    }

    toml.push_str("\n[lib]\n");
    toml.push_str(&format!("name = \"{}\"\n", cluster.merged_lib_ident));
    toml.push_str("doctest = false\n");

    fs::write(merged_dir.join("Cargo.toml"), &toml).context("write merged Cargo.toml")?;

    // lib.rs: one `pub mod` per member (+ host module if present).
    let mut lib = format!(
        "//! Merged `{}` crate — union of former sub-crates, each as `pub mod`.\n",
        cluster.merged_package
    );
    if let Some(hm) = host_module {
        lib.push_str(&format!("pub mod {};\n", hm));
    }
    for sc in &cluster.members {
        lib.push_str(&format!("pub mod {};\n", sc.module));
    }
    fs::write(merged_dir.join("src").join("lib.rs"), &lib).context("write merged lib.rs")?;

    let total = cluster.members.len() + if host_module.is_some() { 1 } else { 0 };
    println!(
        "Created merged crate {} ({} submodules).",
        cluster.merged_package, total
    );
    Ok(())
}

/// Render a `[<section>]` dep table into the TOML buffer.
fn render_dep_section(out: &mut String, section: &str, deps: &BTreeMap<String, toml::Value>) {
    if deps.is_empty() {
        return;
    }
    out.push_str(&format!("\n[{}]\n", section));
    for (key, val) in deps {
        out.push_str(&format!("{} = {}\n", key, render_toml_value(val)));
    }
}

/// Render a `[target.'cfg(...)'.<section>]` dep table.
fn render_target_dep_section(
    out: &mut String,
    cfg: &str,
    section: &str,
    deps: &BTreeMap<String, toml::Value>,
) {
    if deps.is_empty() {
        return;
    }
    out.push_str(&format!("[target.'{}'.{}]\n", cfg, section));
    for (key, val) in deps {
        out.push_str(&format!("{} = {}\n", key, render_toml_value(val)));
    }
    out.push('\n');
}

/// Render a toml::Value back to inline TOML (mirrors manifest.rs logic).
fn render_toml_value(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => format!("\"{}\"", s),
        toml::Value::Boolean(b) => b.to_string(),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Array(a) => {
            let items: Vec<String> = a.iter().map(render_toml_value).collect();
            format!("[{}]", items.join(", "))
        }
        toml::Value::Table(t) => {
            let items: Vec<String> = t
                .iter()
                .map(|(k, v)| format!("{} = {}", k, render_toml_value(v)))
                .collect();
            format!("{{ {} }}", items.join(", "))
        }
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Datetime(d) => format!("\"{}\"", d),
    }
}

/// Move a sub-crate's `src/` contents into the merged crate as a submodule
/// directory. `lib.rs` becomes `mod.rs`; all other files keep their names.
/// Migrate the HOST crate's own src/ (the crate that becomes the merged
/// crate root — e.g. lemurclaw-rs/core/src/). Unlike member crates, the host's
/// src/ lives directly in merged_dir, so we move its contents into a
/// submodule and preserve the bin/ directory at the crate root.
fn migrate_host_crate_src(merged_dir: &Path, module: &str) -> Result<()> {
    let src_dir = merged_dir.join("src");
    let dest_dir = src_dir.join(module);
    if !src_dir.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(&dest_dir).with_context(|| format!("create {}", dest_dir.display()))?;

    // Move everything from src/ into src/<module>/ EXCEPT bin/ (which must
    // stay at src/bin/ for [[bin]] targets to resolve) and the dest_dir itself
    // (already created above — moving it into itself would error).
    let bin_dir = src_dir.join("bin");
    for entry in fs::read_dir(&src_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let from = entry.path();
        // Keep bin/ at crate root.
        if name == "bin" {
            continue;
        }
        // Skip the dest dir (don't move it into itself).
        if from == dest_dir {
            continue;
        }
        let to = dest_dir.join(&name);
        fs::rename(&from, &to)
            .with_context(|| format!("move {} -> {}", from.display(), to.display()))?;
    }
    // Rename lib.rs → mod.rs.
    let lib_rs = dest_dir.join("lib.rs");
    let mod_rs = dest_dir.join("mod.rs");
    if lib_rs.exists() {
        fs::rename(&lib_rs, &mod_rs)
            .with_context(|| format!("rename {} -> {}", lib_rs.display(), mod_rs.display()))?;
    }
    // Move Cargo.toml into the submodule too (so create_merged_crate can read
    // its deps, and it doesn't conflict with the synthesized merged Cargo.toml).
    let host_toml = merged_dir.join("Cargo.toml");
    let dest_toml = dest_dir.join("Cargo.toml");
    if host_toml.exists() {
        fs::rename(&host_toml, &dest_toml)
            .with_context(|| format!("move {} -> {}", host_toml.display(), dest_toml.display()))?;
    }
    let _ = bin_dir; // bin/ stays at src/bin/
                     // Move crate-root assets into the module (same as member crates).
    move_crate_assets_into_module(merged_dir, &dest_dir)?;
    println!("  ✓ host crate src/ → mod {}", module);
    Ok(())
}

/// Move a sub-crate's `src/` contents into the merged crate as a submodule
/// directory. `lib.rs` becomes `mod.rs`; all other files keep their names.
/// Moving the whole `src/` dir preserves internal `#[path]` and `mod`
/// Fix include_str!/include_bytes! relative paths in the host module. The host
/// module moved one level deeper (src/ → src/<module>/), so every `../` in an
/// include path needs an extra `../` to reach the same target. This works for
/// both crate-root assets (stayed at root) and sibling files (moved with src/).
fn fix_include_paths_in_submodule(dir: &Path) -> Result<()> {
    fix_include_paths_recursive(dir)
}

fn fix_include_paths_recursive(dir: &Path) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            fix_include_paths_recursive(&path)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            let raw =
                fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
            let rewritten = fix_include_paths_in_src(&raw, &path);
            if rewritten != raw {
                fs::write(&path, &rewritten)
                    .with_context(|| format!("write {}", path.display()))?;
            }
        }
    }
    Ok(())
}

/// Fix include_str!/include_bytes! relative paths by resolving them against
/// the actual filesystem. After module migration, paths like `../prompt.md`
/// may no longer point at the right file. This function:
/// 1. Rewrites cross-crate paths `codex-rs/<crate>/X` → module-relative.
/// 2. For each include path, checks if the file exists at the resolved
///    location. If not, searches the module dir and crate root for the file
///    and rewrites to the correct relative path.
fn fix_include_paths_in_src(src: &str, rs_file: &Path) -> String {
    let rs_dir = rs_file.parent().unwrap_or(Path::new("."));
    let mut out = src.to_string();

    // Step 1: Rewrite cross-crate paths ("codex-rs/<crate>/X") to module-
    // relative paths. The crate is now a sibling module under src/.
    // We need the merged crate's src/ dir to resolve these.
    // The rs_file is at .../src/<module>/[...]/file.rs, so src/ is 2+ levels up.
    let src_dir = find_src_dir(rs_file);

    while let Some(start) = out.find("codex-rs/") {
        let prefix = &out[..start];
        let quote_pos = prefix
            .rfind("include_str!(\"")
            .or_else(|| prefix.rfind("include_bytes!(\""));
        if let Some(qp) = quote_pos {
            let quote_start = qp + prefix[qp..].find('"').unwrap() + 1;
            let after = &out[start + "codex-rs/".len()..];
            if let Some(end_rel) = after.find('"') {
                let crate_dir = &after[..end_rel];
                if let Some(slash) = crate_dir.find('/') {
                    let crate_name = &crate_dir[..slash];
                    let rest = &crate_dir[slash..];
                    let module = crate_name.replace('-', "_");
                    // Find the file relative to src_dir/<module>/rest
                    if let Some(sd) = &src_dir {
                        let candidate = sd.join(&module).join(rest.trim_start_matches('/'));
                        if candidate.exists() {
                            // Compute relative path from rs_dir to candidate.
                            let rel = pathdiff_rel(rs_dir, &candidate);
                            let replace_end = start + "codex-rs/".len() + end_rel;
                            out.replace_range(quote_start..replace_end, &rel);
                            continue;
                        }
                    }
                }
            }
        }
        break;
    }

    // Step 2: For each include_str!/include_bytes! with a relative path,
    // check if the file exists. If not, search for it.
    out = fix_single_includes(&out, rs_dir, "include_str!(");
    out = fix_single_includes(&out, rs_dir, "include_bytes!(");

    // Step 3: Fix sqlx::migrate!("./dir") paths — these resolve relative to
    // CARGO_MANIFEST_DIR (crate root), not the .rs file. The migration dirs
    // moved into the module, so "./migrations" → "./src/<module>/migrations".
    if let Some(sd) = &src_dir {
        out = fix_sqlx_migrate_paths(&out, sd, rs_dir);
    }

    out
}

/// Fix `sqlx::migrate!("./dir")` paths. These macros resolve relative to the
/// crate root (CARGO_MANIFEST_DIR), not the .rs file. When migration dirs move
/// into a module, the path needs to become "./src/<module>/dir".
fn fix_sqlx_migrate_paths(src: &str, src_dir: &Path, rs_file: &Path) -> String {
    // Determine the module name from the rs_file path relative to src_dir.
    // rs_file = src_dir/<module>/[...]/file.rs → module = first component.
    let rel = rs_file.strip_prefix(src_dir).unwrap_or(rs_file);
    let module = rel
        .components()
        .next()
        .and_then(|c| c.as_os_str().to_str())
        .unwrap_or("");

    let mut out = src.to_string();
    // Find sqlx::migrate!("...") and rewrite the path.
    while let Some(pos) = out.find("sqlx::migrate!(\"") {
        let path_start = pos + "sqlx::migrate!(\"".len();
        let after = &out[path_start..];
        if let Some(end) = after.find('"') {
            let old_path = &after[..end];
            // Check if the dir exists at crate root (src_dir.parent()).
            let crate_root = src_dir.parent().unwrap_or(Path::new("."));
            let resolved = crate_root.join(old_path.trim_start_matches("./"));
            if !resolved.exists() {
                // Try src/<module>/<dir>.
                let candidate = src_dir.join(module).join(old_path.trim_start_matches("./"));
                if candidate.exists() {
                    let new_path =
                        format!("./src/{}/{}", module, old_path.trim_start_matches("./"));
                    let replace_end = path_start + end;
                    out.replace_range(path_start..replace_end, &new_path);
                    continue;
                }
            }
            // Already resolves or can't fix — advance past this occurrence
            // to continue searching for more sqlx::migrate! calls.
            let skip_to = path_start + end + 1;
            let before = &out[..skip_to];
            let after_skip = &out[skip_to..];
            out = format!("{}{}", before, after_skip);
            // Search only in the remainder to avoid re-matching.
            // Rebuild from the found position forward.
            // Simple approach: replace the search string to break the match.
            // Actually, just continue the while loop — but we need to not
            // re-find the same occurrence. Since we didn't modify it, advance
            // by re-searching from skip_to.
            // Use a different approach: process all matches via replace.
            break; // Fall through to the replacement approach below.
        }
        break;
    }

    // If the loop above broke without fixing all, do a line-based pass.
    let crate_root = src_dir.parent().unwrap_or(Path::new("."));
    let module_final = module;
    out = out
        .lines()
        .map(|line| {
            if !line.contains("sqlx::migrate!(\"") {
                return line.to_string();
            }
            // Extract the path from sqlx::migrate!("...")
            let marker = "sqlx::migrate!(\"";
            if let Some(start) = line.find(marker) {
                let path_start = start + marker.len();
                if let Some(end) = line[path_start..].find('"') {
                    let old_path = &line[path_start..path_start + end];
                    let resolved = crate_root.join(old_path.trim_start_matches("./"));
                    if resolved.exists() {
                        return line.to_string(); // Already resolves.
                    }
                    let candidate = src_dir
                        .join(module_final)
                        .join(old_path.trim_start_matches("./"));
                    if candidate.exists() {
                        let new_path = format!(
                            "./src/{}/{}",
                            module_final,
                            old_path.trim_start_matches("./")
                        );
                        return line.replacen(old_path, &new_path, 1);
                    }
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    out
}

/// Find the `src/` directory that contains this .rs file's module.
fn find_src_dir(rs_file: &Path) -> Option<PathBuf> {
    let mut candidate = rs_file.parent()?;
    while let Some(parent) = candidate.parent() {
        if candidate.file_name().is_some_and(|n| n == "src") {
            return Some(candidate.to_path_buf());
        }
        candidate = parent;
    }
    None
}

/// Compute a relative path from `from` to `to` (both absolute or both relative).
fn pathdiff_rel(from: &Path, to: &Path) -> String {
    // Simple implementation: walk up from `from` until we find a common
    // ancestor with `to`, then descend.
    let from_components: Vec<_> = from.components().collect();
    let to_components: Vec<_> = to.components().collect();
    let mut common = 0;
    for (a, b) in from_components.iter().zip(to_components.iter()) {
        if a == b {
            common += 1;
        } else {
            break;
        }
    }
    let ups = from_components.len() - common;
    let mut result = String::new();
    for _ in 0..ups {
        result.push_str("../");
    }
    for comp in &to_components[common..] {
        result.push_str(&comp.as_os_str().to_string_lossy());
        result.push('/');
    }
    result.trim_end_matches('/').to_string()
}

/// Fix include_str!/include_bytes! paths by checking if the file exists and
/// searching for it if not. `macro_name` is "include_str!(" or "include_bytes!(".
fn fix_single_includes(src: &str, rs_dir: &Path, macro_name: &str) -> String {
    let quote_prefix = format!("{}\"", macro_name);
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(pos) = rest.find(&quote_prefix) {
        let path_start = pos + quote_prefix.len();
        out.push_str(&rest[..path_start]);
        let after = &rest[path_start..];
        if let Some(end) = after.find('"') {
            let path = &after[..end];
            let resolved = rs_dir.join(path);
            if resolved.exists() {
                // Path is fine as-is.
                out.push_str(path);
                out.push('"');
                rest = &after[end + 1..];
            } else {
                // File not found — search for it. Pass the path tail (stripped
                // of leading "../") so we can match directory structure too.
                let path_tail = path.trim_start_matches("../");
                let found = search_for_file(rs_dir, path_tail, &resolved);
                if let Some(correct_path) = found {
                    out.push_str(&correct_path);
                    out.push('"');
                    rest = &after[end + 1..];
                } else {
                    // Can't find — leave as-is (will error at compile).
                    out.push_str(path);
                    out.push('"');
                    rest = &after[end + 1..];
                }
            }
        } else {
            out.push_str(after);
            break;
        }
    }
    out.push_str(rest);
    out
}

/// Search for a file by its path tail (e.g. "schema/generated/X.json" or just
/// "X.json"). Walk up from `rs_dir`, at each level recursively searching for
/// the path tail. Returns the correct relative path from `rs_dir` to the file.
fn search_for_file(rs_dir: &Path, filename: &str, _orig_resolved: &Path) -> Option<String> {
    // The filename may be just "X.json" or a path like "schema/generated/X.json".
    // Strip leading "../" from the original include path to get the tail.
    let tail = filename;
    let mut dir = rs_dir.to_path_buf();
    for _ in 0..10 {
        // Check if dir/tail exists (preserves directory structure).
        if dir.join(tail).exists() {
            return Some(relative_path(rs_dir, &dir.join(tail)));
        }
        // Also do a recursive search for just the basename.
        let basename = Path::new(tail).file_name()?.to_string_lossy();
        if let Some(found) = find_file_recursive(&dir, &basename, 4) {
            return Some(relative_path(rs_dir, &found));
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

/// Recursively search for a file named `name` under `dir`, up to `max_depth`.
fn find_file_recursive(dir: &Path, name: &str, max_depth: usize) -> Option<PathBuf> {
    if max_depth == 0 {
        return None;
    }
    if dir.join(name).exists() {
        return Some(dir.join(name));
    }
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.is_dir() {
            if let Some(found) = find_file_recursive(&p, name, max_depth - 1) {
                return Some(found);
            }
        }
    }
    None
}

/// Compute a relative path string from `base` to `target`.
fn relative_path(base: &Path, target: &Path) -> String {
    pathdiff_rel(base, target)
}

/// Move a sub-crate's `src/` contents into the merged crate as a submodule
/// directory. `lib.rs` becomes `mod.rs`; all other files keep their names.
/// Moving the whole `src/` dir preserves internal `#[path]` and `mod`
/// relative references.
fn migrate_subcrate_src(sub_dir: &Path, merged_dir: &Path, module: &str) -> Result<()> {
    let src_dir = sub_dir.join("src");
    let dest_dir = merged_dir.join("src").join(module);
    if !src_dir.is_dir() {
        anyhow::bail!("missing src/ in {}", sub_dir.display());
    }
    fs::create_dir_all(&dest_dir).with_context(|| format!("create {}", dest_dir.display()))?;

    // Move every entry from src/ into dest/.
    for entry in fs::read_dir(&src_dir)? {
        let entry = entry?;
        let from = entry.path();
        let name = entry.file_name();
        let to = dest_dir.join(&name);
        fs::rename(&from, &to)
            .with_context(|| format!("move {} -> {}", from.display(), to.display()))?;
    }

    // Rename lib.rs -> mod.rs (the submodule entry point).
    let lib_rs = dest_dir.join("lib.rs");
    let mod_rs = dest_dir.join("mod.rs");
    if lib_rs.exists() {
        fs::rename(&lib_rs, &mod_rs)
            .with_context(|| format!("rename {} -> {}", lib_rs.display(), mod_rs.display()))?;
    }

    // Move crate-root asset files/dirs (non-src, non-config) into the module.
    // include_str!("../X") paths referenced these from src/lib.rs; after the
    // move, the assets need to be inside the module so the stripped paths work.
    move_crate_assets_into_module(sub_dir, &dest_dir)?;

    println!(
        "  ✓ {} → mod {}",
        sub_dir.file_name().unwrap().to_string_lossy(),
        module
    );
    Ok(())
}

/// Move non-src, non-config files/dirs from a crate root into its module dir.
/// These are assets (templates/, schema/, *.md, *.json, etc.) referenced by
/// include_str!/include_bytes! with "../" paths.
fn move_crate_assets_into_module(crate_dir: &Path, module_dir: &Path) -> Result<()> {
    for entry in fs::read_dir(crate_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let from = entry.path();
        // Skip src/, Cargo.toml, Cargo.lock, BUILD.bazel, target/, .DS_Store,
        // build.rs (handled separately — merged into host crate root if needed).
        if name == "src"
            || name == "Cargo.toml"
            || name == "Cargo.lock"
            || name == "BUILD.bazel"
            || name == "target"
            || name == ".DS_Store"
            || name == ".git"
            || name == "build.rs"
        {
            continue;
        }
        // Skip the module_dir itself (for host crate, it's inside crate_dir/src/).
        let to = module_dir.join(&name);
        if from == *module_dir || to.exists() {
            continue;
        }
        fs::rename(&from, &to)
            .with_context(|| format!("move asset {} -> {}", from.display(), to.display()))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Stage 3: rewrite .rs imports + Cargo.toml dep lines.
// ---------------------------------------------------------------------------

/// Whether a `.rs` rewrite targets the merged crate itself (use `crate::`) or
/// downstream crates (use `<lib>::`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum RewriteScope {
    IntraCrate,
    Downstream,
}

impl RewriteScope {
    fn prefix(self, cluster: &Cluster) -> String {
        match self {
            RewriteScope::IntraCrate => "crate".to_string(),
            RewriteScope::Downstream => cluster.merged_lib_ident.to_string(),
        }
    }
}

/// Rewrite `lemurclaw-rs/Cargo.toml`: replace the 17 merged `utils/<dir>` workspace
/// members with a single `utils/utils`, and collapse the 17 merged
/// `lemurclaw-utils-*` workspace dep entries into a single `lemurclaw-utils`.
/// Standalone crates keep their members + deps.
fn rewrite_publish_manifest(publish_root: &Path, cluster: &Cluster) -> Result<()> {
    let path = publish_root.join("Cargo.toml");
    let raw = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;

    let merge_dirs: std::collections::HashSet<&str> =
        cluster.members.iter().map(|sc| sc.dir).collect();
    let merge_pkgs: std::collections::HashSet<&str> =
        cluster.members.iter().map(|sc| sc.package).collect();

    // The member prefix in workspace.members lines. For utils it's "utils/";
    // for core (source_subdir empty) members appear bare (e.g. "protocol").
    let member_prefix = if cluster.source_subdir.is_empty() {
        String::new()
    } else {
        format!("{}/", cluster.source_subdir)
    };
    let merged_member_line = format!("\"{}\"", cluster.merged_member_path);
    let merged_dep_line = format!(
        "{} = {{ path = \"{}\" }}",
        cluster.merged_package, cluster.merged_member_path
    );
    let merged_dep_check = format!("{} = ", cluster.merged_package);

    let mut out = String::with_capacity(raw.len());
    let mut section = String::new();
    let mut in_workspace = false;
    let mut in_workspace_deps = false;

    for line in raw.lines() {
        let trimmed = line.trim_start();

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_workspace = trimmed == "[workspace]";
            in_workspace_deps = trimmed == "[workspace.dependencies]";
            section = trimmed.to_string();
            out.push_str(line);
            out.push('\n');
            continue;
        }

        // Inside [workspace] members: drop merged member entries (replace with
        // one merged_member_path); keep standalone ones.
        if in_workspace {
            let member_str = format!("\"{}", member_prefix);
            if trimmed.starts_with(&member_str) {
                // Extract the dir from "<prefix><dir>",
                let member_dir = trimmed
                    .trim_start_matches(&member_str)
                    .trim_end_matches("\",");
                if merge_dirs.contains(member_dir) {
                    // Merged — replace with the merged member (once).
                    if !out.contains(&merged_member_line) {
                        let indent = &line[..line.len() - trimmed.len()];
                        out.push_str(indent);
                        out.push_str(&merged_member_line);
                        out.push_str(",\n");
                    }
                    continue;
                }
                // Standalone — keep as-is.
                out.push_str(line);
                out.push('\n');
                continue;
            }
        }

        // Inside [workspace.dependencies]: collapse merged packages into one
        // merged_package; keep standalone ones. Also skip any pre-existing
        // entry for the merged package itself (the collapsed line replaces it).
        if in_workspace_deps {
            if let Some(pkg) = extract_dep_key(trimmed) {
                if merge_pkgs.contains(pkg.as_str()) {
                    if !out.contains(&merged_dep_check) {
                        let indent = &line[..line.len() - trimmed.len()];
                        out.push_str(&format!("{}{}\n", indent, merged_dep_line));
                    }
                    continue;
                }
                // Skip the merged package's own pre-existing entry — the
                // collapsed line (emitted above) replaces it.
                if pkg == cluster.merged_package {
                    if !out.contains(&merged_dep_check) {
                        let indent = &line[..line.len() - trimmed.len()];
                        out.push_str(&format!("{}{}\n", indent, merged_dep_line));
                    }
                    continue;
                }
            }
        }

        let _ = &section;
        out.push_str(line);
        out.push('\n');
    }

    fs::write(&path, &out).with_context(|| format!("write {}", path.display()))?;
    println!("Rewrote lemurclaw-rs/Cargo.toml (members + workspace deps).");
    Ok(())
}

/// Fix `crate::` self-references inside a merged sub-module directory. When a
/// sub-crate used `crate::Foo` to reach its own root items, that path now needs
/// the module prefix: `crate::<module>::Foo`. Cross-module refs
/// (`crate::absolute_path::...`, already rewritten from `lemurclaw_utils_X`) are
/// left alone — we detect them by checking the first segment against the known
/// module names.
///
/// For the HOST module (e.g. core_internal), there's an additional subtlety:
/// the host's own internal modules (e.g. `config`, `state`, `tools`) may share
/// names with merged member modules. A `crate::config::Config` inside the host
/// meant the host's OWN `config` module, not the member `config` module. We
/// detect this by reading the host's mod.rs for its local `mod` declarations —
/// any `crate::X` where X is a local module gets prefixed to `crate::<module>::X`.
fn fix_self_refs_in_submodule(dir: &Path, module: &str, cluster: &Cluster) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    let modules = cluster.module_names();
    // Collect local module names from the host's mod.rs (if this is the host).
    let local_mods = collect_local_mods(&dir.join("mod.rs"));
    fix_self_refs_recursive(dir, module, &modules, &local_mods)
}

/// Parse a mod.rs file for `mod <name>;` / `pub mod <name>;` declarations,
/// returning the set of local module names.
fn collect_local_mods(mod_rs: &Path) -> std::collections::HashSet<String> {
    let mut mods = std::collections::HashSet::new();
    if let Ok(raw) = fs::read_to_string(mod_rs) {
        for line in raw.lines() {
            let trimmed = line.trim_start();
            // Match "mod foo;" or "pub mod foo;" or "pub(crate) mod foo;".
            let after_vis = trimmed
                .strip_prefix("pub(crate) ")
                .or_else(|| trimmed.strip_prefix("pub "))
                .unwrap_or(trimmed);
            if let Some(rest) = after_vis.strip_prefix("mod ") {
                if let Some(name) = rest.split(';').next() {
                    let name = name.trim().trim_end_matches(" {");
                    if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                        mods.insert(name.to_string());
                    }
                }
            }
        }
    }
    mods
}

fn fix_self_refs_recursive(
    dir: &Path,
    module: &str,
    modules: &std::collections::HashSet<String>,
    local_mods: &std::collections::HashSet<String>,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            fix_self_refs_recursive(&path, module, modules, local_mods)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            let raw =
                fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
            let rewritten = fix_self_refs_in_src(&raw, module, modules, local_mods, &path);
            if rewritten != raw {
                fs::write(&path, &rewritten)
                    .with_context(|| format!("write {}", path.display()))?;
            }
        }
    }
    Ok(())
}

/// Rewrite self-referential `crate::X` to `crate::<module>::X` where X is NOT a
/// known cross-module name. A `crate::` followed by a known module name (e.g.
/// `crate::absolute_path`) is a cross-module ref and is preserved — UNLESS X is
/// also a local module of this submodule (host module case: `crate::config`
/// inside core_internal means core_internal's own config, not the member).
fn fix_self_refs_in_src(
    src: &str,
    module: &str,
    modules: &std::collections::HashSet<String>,
    local_mods: &std::collections::HashSet<String>,
    _rs_file: &Path,
) -> String {
    // Walk the source and rewrite `crate::` self-refs.
    // With the pipeline reordered (this runs BEFORE rewrite_rs), any `crate::X`
    // is a self-ref — member cross-refs are still `lemurclaw_X::`.
    let needle = "crate::";
    let nbytes = needle.len();
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if i + nbytes <= bytes.len() && &bytes[i..i + nbytes] == needle.as_bytes() {
            // Check whether the previous char is an identifier char — if so,
            // this `crate::` is part of a longer identifier (e.g. `my_crate::`)
            // and must not be touched.
            let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
            // Read the segment following `crate::`.
            let seg_start = i + nbytes;
            let seg_end = seg_start
                + bytes[seg_start..]
                    .iter()
                    .take_while(|b| is_ident_byte(**b))
                    .count();
            if before_ok && seg_end > seg_start {
                let segment = &src[seg_start..seg_end];
                // With the pipeline reordered (fix_self_refs BEFORE rewrite_rs),
                // any `crate::X` at this point is a SELF-REF — member cross-refs
                // are still `lemurclaw_X::` and haven't been rewritten yet.
                // So: prefix `crate::X` → `crate::<module>::X` if X is a local
                // module of this submodule. No per-item guessing needed.
                //
                // Exception: skip `crate::feedback_tags` — it's a #[macro_export]
                // that lives at the crate root, not in any submodule.
                let is_local = local_mods.contains(segment);
                let is_macro_export = segment == "feedback_tags";
                let is_self = (is_local || !modules.contains(segment) || segment == module)
                    && !is_macro_export;
                if is_self {
                    // Self-ref: inject module prefix.
                    out.push_str(&format!("crate::{}::{}", module, segment));
                    i = seg_end;
                    continue;
                }
            }
        }
        let ch_end = next_char_boundary(bytes, i);
        out.push_str(&src[i..ch_end]);
        i = ch_end;
    }
    out
}

/// Recursively rewrite every `.rs` file under `dir` using the given scope.
fn rewrite_rust_files_in(dir: &Path, scope: RewriteScope, cluster: &Cluster) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            rewrite_rust_files_in(&path, scope, cluster)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            let raw =
                fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
            let rewritten = rewrite_rs(&raw, scope, cluster);
            if rewritten != raw {
                fs::write(&path, &rewritten)
                    .with_context(|| format!("write {}", path.display()))?;
            }
        }
    }
    Ok(())
}

/// Rewrite `<lib>_<module>` crate-path segments (for the cluster's members
/// only) in `src` to `<prefix>::<module>`. Standalone crates' idents are left
/// untouched — they remain separate crates. This is a targeted text
/// substitution: the idents are specific enough that they only appear as crate
/// path segments in valid Rust.
fn rewrite_rs(src: &str, scope: RewriteScope, cluster: &Cluster) -> String {
    let prefix = scope.prefix(cluster);
    let mut out = src.to_string();
    for sc in &cluster.members {
        let ident = sc.package.replace('-', "_");
        let replacement = format!("{}::{}", prefix, sc.module);
        out = replace_ident_whole(&out, &ident, &replacement);
    }
    out
}

/// Replace every whole-word occurrence of `needle` in `haystack` with
/// `replacement`. "Whole word" means the char before (if any) and after (if
/// any) the match is not an identifier-continuation char (`[A-Za-z0-9_]`).
/// This prevents partial matches like `lemurclaw_utils_path` matching inside
/// `lemurclaw_utils_path_uri`.
fn replace_ident_whole(haystack: &str, needle: &str, replacement: &str) -> String {
    let bytes = haystack.as_bytes();
    let nbytes = needle.as_bytes();
    if nbytes.is_empty() || bytes.len() < nbytes.len() {
        return haystack.to_string();
    }
    let mut out = String::with_capacity(haystack.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if i + nbytes.len() <= bytes.len() && &bytes[i..i + nbytes.len()] == nbytes {
            let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
            let after_idx = i + nbytes.len();
            let after_ok = after_idx >= bytes.len() || !is_ident_byte(bytes[after_idx]);
            if before_ok && after_ok {
                out.push_str(replacement);
                i = after_idx;
                continue;
            }
        }
        // Copy one UTF-8 char (safe: we advance by byte only when no match).
        let ch_end = next_char_boundary(bytes, i);
        out.push_str(&haystack[i..ch_end]);
        i = ch_end;
    }
    out
}

/// Check if the host module's local submodule `<module>/<segment>/` defines
/// a public item named `item`. Used to disambiguate local+member module
/// name collisions: if the host's own module defines the item, the ref is a
/// self-ref (needs prefixing); if not, it's a cross-module ref to the member.
fn host_module_defines(module_dir: &Path, segment: &str, item: &str) -> bool {
    if item.is_empty() || module_dir.as_os_str().is_empty() {
        return true; // Can't check — default to prefixing (safer for host).
    }
    // Check both directory module (segment/) and file module (segment.rs).
    let seg_dir = module_dir.join(segment);
    let seg_file = module_dir.join(format!("{}.rs", segment));
    if seg_dir.is_dir() {
        return grep_in_dir_for_item(&seg_dir, item);
    }
    if seg_file.is_file() {
        return grep_in_file_for_item(&seg_file, item);
    }
    // Neither exists — the host doesn't have this local module, so the ref
    // must be a cross-module ref (don't prefix).
    false
}

/// Check if a single .rs file defines a public item named `item`.
fn grep_in_file_for_item(path: &Path, item: &str) -> bool {
    let raw = match fs::read_to_string(path) {
        Ok(r) => r,
        Err(_) => return false,
    };
    check_defs(&raw, item)
}

fn check_defs(raw: &str, item: &str) -> bool {
    for line in raw.lines() {
        let t = line.trim_start();
        let def_keywords = [
            "pub struct ",
            "pub(crate) struct ",
            "pub enum ",
            "pub(crate) enum ",
            "pub fn ",
            "pub(crate) fn ",
            "pub type ",
            "pub(crate) type ",
            "pub trait ",
            "pub(crate) trait ",
            "pub const ",
            "pub(crate) const ",
            "pub static ",
            "pub(crate) static ",
            "pub mod ",
            "pub(crate) mod ",
            "mod ",
        ];
        let is_def = def_keywords.iter().any(|kw| {
            t.starts_with(kw)
                && t[kw.len()..].starts_with(item)
                && t[kw.len() + item.len()..]
                    .chars()
                    .next()
                    .is_some_and(|c| !(c.is_alphanumeric() || c == '_'))
        });
        if is_def {
            return true;
        }
        if t.starts_with("pub use ") || t.starts_with("pub(crate) use ") {
            let after_use = t
                .strip_prefix("pub use ")
                .or_else(|| t.strip_prefix("pub(crate) use "))
                .unwrap_or("");
            if after_use.starts_with(item) && !after_use[item.len()..].starts_with(':') {
                return true;
            }
        }
    }
    false
}

fn grep_in_dir_for_item(dir: &Path, item: &str) -> bool {
    fn search(dir: &Path, item: &str) -> bool {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return false,
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                if search(&path, item) {
                    return true;
                }
            } else if path.extension().is_some_and(|e| e == "rs") {
                if let Ok(raw) = fs::read_to_string(&path) {
                    if check_defs(&raw, item) {
                        return true;
                    }
                }
            }
        }
        false
    }
    search(dir, item)
}

/// Post-merge deterministic fixups for edge cases that the pipeline ordering
/// can't handle automatically. These are known, fixed patterns.
fn post_merge_fixups(src_dir: &Path, cluster: &Cluster) -> Result<()> {
    match cluster.name {
        "core" => post_merge_fixups_core(src_dir),
        "extensions" => post_merge_fixups_extensions(src_dir),
        "server" => post_merge_fixups_server(src_dir),
        "tui" => post_merge_fixups_tui(src_dir),
        "cli" => post_merge_fixups_cli(src_dir),
        _ => Ok(()),
    }
}

/// Core-specific post-merge fixups (the original 9 deterministic fixups).
fn post_merge_fixups_core(src_dir: &Path) -> Result<()> {
    // 1. Fix `app_server_protocol::protocol::account` → `protocol::account`
    //    (the items live in the member `protocol`, not `app_server_protocol`).
    fix_pattern_in_tree(
        src_dir,
        "crate::app_server_protocol::protocol::account",
        "crate::protocol::account",
    )?;

    // 2. Fix exec_server's `use ... as protocol;` alias problem.
    //    exec_server's lib.rs had `use codex_exec_server_protocol as protocol;`
    //    at crate root, making `crate::protocol::X` resolve to exec_server_protocol.
    //    After merge, `crate::protocol::` resolves to the MEMBER protocol module.
    //
    //    Solution: rewrite `crate::protocol::` → `crate::exec_server_protocol::`
    //    ONLY for CamelCase/UPPER_CASE items (exec_server_protocol types/constants).
    //    We use `crate::exec_server_protocol::` (not `::protocol::`) because
    //    the `protocol` submodule is private and re-exported via `pub use protocol::*`.
    //    Lowercase submodule names (capabilities, models, permissions) stay as
    //    `crate::protocol::` (member protocol submodules).
    let exec_server_dir = src_dir.join("exec_server");
    if exec_server_dir.is_dir() {
        rewrite_protocol_refs_in_exec_server(&exec_server_dir)?;
    }

    // 3. Fix `crate::protocol::EventMsg` / `RolloutItem` in rollout/ and
    //    other modules that had `pub(crate) use codex_protocol::protocol;`.
    //    These resolved `crate::protocol::X` → `codex_protocol::protocol::X`,
    //    but after merge `crate::protocol::X` hits the member `protocol` module
    //    which doesn't directly export EventMsg/RolloutItem (they're in the
    //    public sub-submodule `protocol::protocol`).
    //    → `crate::protocol::protocol::X`.
    fix_pattern_in_tree(
        src_dir,
        "crate::protocol::EventMsg",
        "crate::protocol::protocol::EventMsg",
    )?;
    fix_pattern_in_tree(
        src_dir,
        "crate::protocol::RolloutItem",
        "crate::protocol::protocol::RolloutItem",
    )?;

    // 4. Fix `use lemurclaw_core::` in test files → `use crate::`
    //    (test files reference the crate by its published name, but inside
    //    the crate itself, they should use `crate::`). Only rewrite in `use`
    //    statements — NOT in type positions where `crate::` can't appear.
    fix_use_lemurclaw_core_in_tree(src_dir)?;

    // 5. Add `pub use router::ToolCall;` to core_internal/tools/mod.rs if missing.
    let tools_mod = src_dir.join("core_internal").join("tools").join("mod.rs");
    if tools_mod.is_file() {
        let raw = fs::read_to_string(&tools_mod)?;
        if !raw.contains("pub use router::ToolCall") {
            // Check if ToolCall is defined in router.rs
            let router = src_dir
                .join("core_internal")
                .join("tools")
                .join("router.rs");
            if router.is_file() {
                let router_raw = fs::read_to_string(&router)?;
                if router_raw.contains("pub struct ToolCall")
                    || router_raw.contains("pub enum ToolCall")
                {
                    let new = format!("pub use router::ToolCall;\n{}", raw);
                    fs::write(&tools_mod, new)?;
                }
            }
        }
    }
    // 6. Fix `include_dir!("$CARGO_MANIFEST_DIR/src/...")` paths in member
    //    modules. After merge, the member's src/ became src/<module>/, so
    //    `$CARGO_MANIFEST_DIR/src/assets/samples` needs to become
    //    `$CARGO_MANIFEST_DIR/src/<module>/assets/samples`.
    fix_include_dir_manifest_dir_paths(src_dir)?;

    // 7. Re-export `experimental_api` at crate root. The proc-macro
    //    `lemurclaw-experimental-api-macros` expands `#[experimental(...)]`
    //    into code that references `crate::experimental_api::ExperimentalApi`,
    //    `crate::experimental_api::ExperimentalField`, etc. Before merge,
    //    `crate::` resolved to `app-server-protocol`; now it resolves to
    //    `lemurclaw-core`, which doesn't have `experimental_api` at its root.
    //    Fix: create `src/experimental_api.rs` that re-exports from the member,
    //    and add `pub mod experimental_api;` to lib.rs.
    add_reexport_module(
        src_dir,
        "experimental_api",
        "app_server_protocol::experimental_api",
    )?;

    // 8. Re-export `Config` from `config` module. Several downstream crates
    //    (lmstudio, ollama, utils_oss, core_internal tests) use
    //    `crate::config::Config`, but `Config` is defined in `core_internal::config`,
    //    not in the member `config` module (codex-config). Add a re-export so
    //    `crate::config::Config` resolves correctly.
    add_reexport_from_host(src_dir, "config", "Config")?;

    // 9. Unify reqwest version. After merge, rmcp (which depends on reqwest
    //    0.13) and the main crate (which depends on reqwest 0.12) end up in
    //    the same compilation unit, causing E0308 type mismatches. Fix by
    //    upgrading the workspace reqwest to 0.13 in lemurclaw-rs/Cargo.toml.
    unify_reqwest_version(src_dir)?;

    // 10. Add crate-root re-exports from core_internal. Downstream crates
    //     (server, tui, cli) and extensions import many types from the crate
    //     root (e.g. `lemurclaw_core::CodexThread`) that originally lived at
    //     codex-core's root but after merge are inside `core_internal`.
    add_core_root_reexports(src_dir)?;

    // 11. Add member-module re-exports from core_internal. Some downstream
    //     imports target sub-modules like `lemurclaw_core::config::*` or
    //     `lemurclaw_core::sandboxing::*`, but those member modules don't
    //     contain the items (they're in core_internal's sub-modules).
    add_core_member_reexports(src_dir)?;

    // 12. Fix otel code that was broken by the post-merge fixup pipeline.
    //     The fix_otel_with_http_client() function in step 9 leaves dangling
    //     `let client = ...` assignments and unused `mut` that cause compile
    //     errors. Clean these up.
    fix_otel_dangling_assignments(src_dir)?;

    Ok(())
}

/// Add crate-root re-exports from `core_internal` to `core/src/lib.rs`.
/// Downstream crates import many types from the crate root that, after the
/// mega-crate merge, live inside the `core_internal` host module.
fn add_core_root_reexports(src_dir: &Path) -> Result<()> {
    let lib_rs = src_dir.join("lib.rs");
    if !lib_rs.is_file() {
        return Ok(());
    }
    let raw = fs::read_to_string(&lib_rs)?;

    // The full list of symbols downstream crates import as `lemurclaw_core::<X>`.
    // These are all `pub use`-ed by core_internal/mod.rs.
    let reexports = [
        "CodexThread",
        "context",
        "NewThread",
        "StartThreadOptions",
        "ThreadManager",
        "image_generation_artifact_path",
        "parse_turn_item",
        "web_search_action_detail",
        "X_CODEX_TURN_METADATA_HEADER",
        "AttestationContext",
        "AttestationProvider",
        "GenerateAttestationFuture",
        "ExecPolicyError",
        "check_execpolicy_for_warnings",
        "resolve_installation_id",
        "exec",
        "path_utils",
        "CodexThreadSettingsOverrides",
        "ForkSnapshot",
        "INTERACTIVE_SESSION_SOURCES",
        "McpManager",
        "SleepFuture",
        "SteerInputError",
        "ThreadConfigSnapshot",
        "TimeFuture",
        "TimeProvider",
        "ModelClient",
        "Prompt",
        "ResponseEvent",
        "RolloutRecorder",
        "content_items_to_text",
        "detached_memory_responses_metadata",
        "otel_init",
        "truncate_rollout_after_turn_id",
        "truncate_rollout_before_turn_id",
        "util",
        "exec_env",
        "CodexAppsToolsCache",
        "review_prompts",
        "local_agent_graph_store_from_state_db",
        "build_models_manager",
        "thread_store_from_config",
        "LoadedAgentsMd",
        "StateDbHandle",
        "ThreadShutdownReport",
        "find_thread_meta_by_name_str",
        "format_exec_policy_error_with_source",
        "init_state_db",
        "spawn",
        "spawn_command_under_linux_sandbox",
    ];

    // Check if re-exports are already present (idempotent).
    let marker = "pub use core_internal::CodexThread;";
    if raw.contains(marker) {
        return Ok(());
    }

    let mut block = String::from("\n// Re-export key types from core_internal at the crate root for downstream\n// crates that previously imported them from codex-core's root.\n");
    for sym in &reexports {
        block.push_str(&format!("pub use core_internal::{};\n", sym));
    }

    let new = format!("{}\n{}", raw.trim_end(), block);
    fs::write(&lib_rs, new)?;
    println!(
        "  ✓ Added {} crate-root re-exports to lib.rs",
        reexports.len()
    );
    Ok(())
}

/// Add re-exports from `core_internal` into member modules (config, sandboxing,
/// skills, connectors, windows_sandbox). Downstream crates import items as
/// `lemurclaw_core::config::<X>` etc., but the member module doesn't have them.
fn add_core_member_reexports(src_dir: &Path) -> Result<()> {
    // config/mod.rs: re-export from core_internal::config + a few cross-module.
    let config_mod = src_dir.join("config").join("mod.rs");
    if config_mod.is_file() {
        let config_reexports = [
            "find_codex_home",
            "StartedNetworkProxy",
            "ConfigOverrides",
            "deserialize_config_toml_with_base",
            "edit",
            "validate_feature_requirements_for_config_toml",
            "permission_profile_catalog",
            "ConfigBuilder",
            "set_project_trust_level",
            "ConfigTomlLoadResult",
            "PermissionProfileSnapshot",
            "Permissions",
            "TerminalResizeReflowConfig",
            "TerminalResizeReflowMaxRows",
            "load_config_toml_with_layer_stack",
            "resolve_bootstrap_auth_keyring_backend_kind",
            "resolve_bootstrap_auth_route_config",
            "resolve_oss_provider",
            "resolve_profile_v2_config_path",
            "NetworkProxySpec",
            "ExtraConfig",
            "GhostSnapshotConfig",
            "MultiAgentV2Config",
            "ThreadStoreConfig",
            "log_dir",
        ];
        let raw = fs::read_to_string(&config_mod)?;
        if !raw.contains("pub use crate::core_internal::config::find_codex_home") {
            let mut block =
                String::from("\n// Re-export core_internal::config items for downstream crates.\n");
            for sym in &config_reexports {
                block.push_str(&format!("pub use crate::core_internal::config::{};\n", sym));
            }
            // Cross-module re-exports needed by downstream crates.
            block.push_str("pub use crate::sandboxing::system_bwrap_warning;\n");
            block.push_str("pub use crate::network_proxy::NetworkProxyAuditMetadata;\n");
            let new = format!("{}\n{}", raw.trim_end(), block);
            fs::write(&config_mod, new)?;
            println!("  ✓ Added config member re-exports");
        }
    }

    // sandboxing/mod.rs: re-export from core_internal::sandboxing.
    let sandboxing_mod = src_dir.join("sandboxing").join("mod.rs");
    if sandboxing_mod.is_file() {
        let raw = fs::read_to_string(&sandboxing_mod)?;
        if !raw.contains("pub use crate::core_internal::sandboxing::ExecRequest") {
            let block = "\n// Re-export core_internal::sandboxing items for downstream crates.\npub use crate::core_internal::sandboxing::ExecRequest;\npub use crate::core_internal::sandboxing::SandboxPermissions;\npub use crate::core_internal::sandboxing::execute_env;\n";
            let new = format!("{}\n{}", raw.trim_end(), block);
            fs::write(&sandboxing_mod, new)?;
            println!("  ✓ Added sandboxing member re-exports");
        }
    }

    // skills/mod.rs: re-export from core_internal::skills + core_skills.
    let skills_mod = src_dir.join("skills").join("mod.rs");
    if skills_mod.is_file() {
        let raw = fs::read_to_string(&skills_mod)?;
        if !raw.contains("pub use crate::core_internal::skills::SkillMetadata") {
            let block = "\n// Re-export core_internal skills types for downstream crates.\npub use crate::core_skills::SkillsLoadInput;\npub use crate::core_skills::SkillsService;\npub use crate::core_internal::skills::SkillMetadata;\npub use crate::core_internal::skills::SkillError;\n";
            let new = format!("{}\n{}", raw.trim_end(), block);
            fs::write(&skills_mod, new)?;
            println!("  ✓ Added skills member re-exports");
        }
    }

    // connectors/mod.rs: re-export from core_internal::connectors.
    let connectors_mod = src_dir.join("connectors").join("mod.rs");
    if connectors_mod.is_file() {
        let raw = fs::read_to_string(&connectors_mod)?;
        if !raw.contains("pub use crate::core_internal::connectors::AccessibleConnectorsStatus") {
            let block = "\n// Re-export core_internal connector functions for downstream crates.\npub use crate::core_internal::connectors::AccessibleConnectorsStatus;\npub use crate::core_internal::connectors::list_accessible_connectors_from_mcp_tools;\npub use crate::core_internal::connectors::list_accessible_connectors_from_mcp_tools_with_environment_manager;\npub use crate::core_internal::connectors::list_accessible_connectors_from_mcp_tools_with_mcp_manager;\npub use crate::core_internal::connectors::list_accessible_connectors_from_mcp_tools_with_options;\npub use crate::core_internal::connectors::list_accessible_connectors_from_mcp_tools_with_options_and_status;\npub use crate::core_internal::connectors::list_cached_accessible_connectors_from_mcp_tools;\npub use crate::core_internal::connectors::with_app_enabled_state;\n";
            let new = format!("{}\n{}", raw.trim_end(), block);
            fs::write(&connectors_mod, new)?;
            println!("  ✓ Added connectors member re-exports");
        }
    }

    // windows_sandbox/mod.rs: re-export from core_internal::windows_sandbox.
    let ws_mod = src_dir.join("windows_sandbox").join("mod.rs");
    if ws_mod.is_file() {
        let raw = fs::read_to_string(&ws_mod)?;
        if !raw.contains("pub use crate::core_internal::windows_sandbox::WindowsSandboxLevelExt") {
            let block = "\n// Re-export core_internal windows_sandbox types for downstream crates.\npub use crate::core_internal::windows_sandbox::WindowsSandboxLevelExt;\npub use crate::core_internal::windows_sandbox::WindowsSandboxSetupMode;\npub use crate::core_internal::windows_sandbox::WindowsSandboxSetupRequest;\npub use crate::core_internal::windows_sandbox::sandbox_setup_is_complete;\npub use crate::core_internal::windows_sandbox::run_windows_sandbox_setup;\n";
            let new = format!("{}\n{}", raw.trim_end(), block);
            fs::write(&ws_mod, new)?;
            println!("  ✓ Added windows_sandbox member re-exports");
        }
    }

    Ok(())
}

/// Fix otel code broken by the post-merge fixup pipeline. The
/// `fix_otel_with_http_client()` function (called from `unify_reqwest_version`)
/// comments out `with_http_client()` calls but leaves dangling
/// `let client = crate::otel::otlp::build_async_http_client(...)` assignments
/// and unused `mut` qualifiers. This cleans them up.
fn fix_otel_dangling_assignments(src_dir: &Path) -> Result<()> {
    // provider.rs: remove dangling `let client = build_async_http_client(` lines
    // and unused `mut` on exporter_builder.
    let provider = src_dir.join("otel").join("provider.rs");
    if provider.is_file() {
        let raw = fs::read_to_string(&provider)?;
        let mut new = raw.clone();

        // Remove the dangling `let client = crate::otel::otlp::build_async_http_client(`
        // line that was left by the post-merge comment-out logic. The line is
        // incomplete (no closing `)`) and followed by the TODO comment.
        new = new.replace(
            "                let client = crate::otel::otlp::build_async_http_client(\n",
            "",
        );

        // Remove `mut` from `let mut exporter_builder` where the builder is
        // never mutated (the with_http_client call was commented out).
        // Only do this for the two SpanExporter spots (not LogExporter).
        new = new.replace(
            "let mut exporter_builder = SpanExporter::builder()",
            "let exporter_builder = SpanExporter::builder()",
        );
        new = new.replace(
            "let mut exporter_builder = LogExporter::builder()",
            "let exporter_builder = LogExporter::builder()",
        );

        if new != raw {
            fs::write(&provider, new)?;
            println!("  ✓ Fixed otel/provider.rs dangling assignments");
        }
    }

    // metrics/client.rs: remove unused `let client = build_http_client(...)` block.
    let metrics_client = src_dir.join("otel").join("metrics").join("client.rs");
    if metrics_client.is_file() {
        let raw = fs::read_to_string(&metrics_client)?;
        // Remove the dangling client assignment block and unused mut.
        let old_block = "                let client =\n                    crate::otel::otlp::build_http_client(tls, OTEL_EXPORTER_OTLP_METRICS_TIMEOUT)\n                        .map_err(|err| MetricsError::InvalidConfig {\n                            message: err.to_string(),\n                        })?;\n\n                // TODO(merge): with_http_client disabled — reqwest 0.13 (rmcp) and";
        let new_block =
            "                // TODO(merge): with_http_client disabled — reqwest 0.13 (rmcp) and";
        let mut new = raw.replace(old_block, new_block);
        // Remove unused mut on MetricExporter builder.
        new = new.replace(
            "let mut exporter_builder = opentelemetry_otlp::MetricExporter::builder()",
            "let exporter_builder = opentelemetry_otlp::MetricExporter::builder()",
        );
        if new != raw {
            fs::write(&metrics_client, new)?;
            println!("  ✓ Fixed otel/metrics/client.rs dangling assignments");
        }
    }

    Ok(())
}

/// Extensions-specific post-merge fixups. Populated iteratively as
/// compilation errors surface.
fn post_merge_fixups_extensions(_src_dir: &Path) -> Result<()> {
    Ok(())
}

/// Server-specific post-merge fixups. Populated iteratively as
/// compilation errors surface.
fn post_merge_fixups_server(src_dir: &Path) -> Result<()> {
    // Rewrite user-facing display text (Codex→lemurclaw) in the server cluster
    // (app_server_daemon/update_loop.rs, app_server_test_client/mod.rs).
    rewrite_brand_display_text(src_dir)?;
    Ok(())
}

/// TUI-specific post-merge fixups.
fn post_merge_fixups_tui(src_dir: &Path) -> Result<()> {
    // 1. Fix include_str! paths in frames.rs. The host crate's src/ moved to
    //    tui_internal/, so `../frames/` (relative from the crate root) is now
    //    wrong — the frames directory moved into tui_internal/frames/. Rewrite
    //    to `frames/` (relative to tui_internal/frames.rs).
    let frames_rs = src_dir.join("tui_internal").join("frames.rs");
    if frames_rs.is_file() {
        let raw = fs::read_to_string(&frames_rs)?;
        let new = raw.replace("../frames/", "frames/");
        if new != raw {
            fs::write(&frames_rs, new)?;
            println!("  ✓ Fixed tui_internal/frames.rs include_str! paths");
        }
    }

    // 2. Add missing dependencies (arboard, libc) to Cargo.toml. These were
    //    used by the tui host crate but may be missing from the merged
    //    Cargo.toml if they were only in the host's dep table.
    let cargo_toml = src_dir.parent().unwrap_or(src_dir).join("Cargo.toml");
    if cargo_toml.is_file() {
        let raw = fs::read_to_string(&cargo_toml)?;
        let mut new = raw.clone();
        // Add arboard if missing.
        if !new.contains("arboard") {
            new = new.replace(
                "anyhow = { workspace = true }\n",
                "anyhow = { workspace = true }\narboard = { workspace = true }\n",
            );
        }
        // Add libc if missing.
        if !new.contains("\nlibc") {
            new = new.replace(
                "lemurclaw-server = { workspace = true }\n",
                "lemurclaw-server = { workspace = true }\nlibc = { workspace = true }\n",
            );
        }
        if new != raw {
            fs::write(&cargo_toml, new)?;
            println!("  ✓ Added missing arboard/libc deps to tui Cargo.toml");
        }
    }

    // 3. Add crate-root re-exports from tui_internal. Downstream crates
    //    (cli) import `lemurclaw_tui::ComposerInput` etc.
    let lib_rs = src_dir.join("lib.rs");
    if lib_rs.is_file() {
        let raw = fs::read_to_string(&lib_rs)?;
        if !raw.contains("pub use tui_internal::ComposerInput") {
            let block = "\n// Re-export key types from tui_internal at the crate root for downstream crates.\npub use tui_internal::ComposerInput;\npub use tui_internal::ComposerAction;\npub use tui_internal::render_markdown_text;\npub use tui_internal::AppExitInfo;\npub use tui_internal::Cli;\npub use tui_internal::ExitReason;\npub use tui_internal::UpdateAction;\n#[cfg(not(debug_assertions))]\npub use tui_internal::get_update_action;\npub use tui_internal::SessionArchiveAction;\npub use tui_internal::run_session_archive_command;\npub use tui_internal::SessionArchiveCommandOptions;\npub use tui_internal::DeleteConfirmation;\n";
            let new = format!("{}\n{}", raw.trim_end(), block);
            fs::write(&lib_rs, new)?;
            println!("  ✓ Added tui lib.rs re-exports");
        }
    }

    // 4. Rewrite user-facing display text (Codex→lemurclaw) across the TUI
    //    src tree (tui_internal/**) plus insta .snap snapshot placeholders.
    rewrite_brand_display_text(src_dir)?;

    Ok(())
}

/// CLI-specific post-merge fixups. Populated iteratively as
/// compilation errors surface.
fn post_merge_fixups_cli(src_dir: &Path) -> Result<()> {
    // Rewrite user-facing display text (Codex→lemurclaw) across the CLI src
    // tree (main.rs, exec/*, cli_internal/*, cloud_tasks/*).
    rewrite_brand_display_text(src_dir)?;
    Ok(())
}

/// Create a thin re-export module at the crate root that re-exports items from
/// a member submodule. This is needed when proc-macro expansions or downstream
/// code references `crate::<name>::...` but `<name>` is a private submodule of
/// a member crate (not directly accessible at the crate root).
///
/// `source_path` may reference either a public submodule of a member (e.g.
/// `app_server_protocol::protocol`) or items already re-exported by a member
/// (e.g. `app_server_protocol` for items it `pub use`'d). If the direct path
/// to the submodule is private, we fall back to the parent module.
fn add_reexport_module(src_dir: &Path, module_name: &str, source_path: &str) -> Result<()> {
    let reexport_file = src_dir.join(format!("{}.rs", module_name));
    if reexport_file.exists() {
        return Ok(());
    }
    // If source_path contains "::" and the last segment is a private module,
    // use the parent (which re-exports via `pub use`) instead. E.g.
    // `app_server_protocol::experimental_api` → `app_server_protocol` because
    // `experimental_api` is private inside `app_server_protocol` but its items
    // are re-exported via `pub use experimental_api::*`.
    let effective_path = if let Some((parent, submodule)) = source_path.rsplit_once("::") {
        // Check if the submodule directory is private (mod, not pub mod).
        let submod_dir = src_dir.join(parent.replace("::", "/")).join(submodule);
        let submod_file = src_dir
            .join(parent.replace("::", "/"))
            .join(format!("{}.rs", submodule));
        let mod_file = src_dir.join(parent.replace("::", "/")).join("mod.rs");
        let is_private = if submod_dir.is_dir() || submod_file.is_file() {
            // Check mod.rs for visibility: "pub mod <submodule>" or
            // "pub(crate) mod <submodule>" means public; bare "mod <submodule>"
            // means private.
            if let Ok(raw) = fs::read_to_string(&mod_file) {
                !raw.lines().any(|line| {
                    let t = line.trim_start();
                    (t.starts_with("pub mod ") && t["pub mod ".len()..].starts_with(submodule))
                        || (t.starts_with("pub(crate) mod ")
                            && t["pub(crate) mod ".len()..].starts_with(submodule))
                })
            } else {
                false
            }
        } else {
            false
        };
        if is_private {
            parent
        } else {
            source_path
        }
    } else {
        source_path
    };
    let content = format!("pub use crate::{}::*;\n", effective_path);
    fs::write(&reexport_file, content)
        .with_context(|| format!("write {}", reexport_file.display()))?;

    // Add `pub mod <name>;` to lib.rs if not already present.
    let lib_rs = src_dir.join("lib.rs");
    if lib_rs.is_file() {
        let raw = fs::read_to_string(&lib_rs)?;
        let decl = format!("pub mod {};", module_name);
        if !raw.contains(&decl) {
            let new = format!("{}\n{}\n", raw.trim_end(), decl);
            fs::write(&lib_rs, new)?;
        }
    }
    Ok(())
}

/// Re-export a specific type from the host module (`core_internal`) into a
/// member module, so that `crate::<member>::<Type>` resolves. This is needed
/// when the original crate depended on the host for this type and after merge
/// the member module doesn't contain it.
fn add_reexport_from_host(src_dir: &Path, member_module: &str, type_name: &str) -> Result<()> {
    let mod_rs = src_dir.join(member_module).join("mod.rs");
    if !mod_rs.is_file() {
        return Ok(());
    }
    let raw = fs::read_to_string(&mod_rs)?;
    let reexport = format!(
        "pub use crate::core_internal::{}::{};",
        member_module, type_name
    );
    if raw.contains(&reexport) {
        return Ok(());
    }
    // Add after the last `pub use` line, or at the end.
    let new = format!("{}\n{}\n", raw.trim_end(), reexport);
    fs::write(&mod_rs, new)?;
    Ok(())
}

/// Unify reqwest version in lemurclaw-rs/Cargo.toml. After merging crates that
/// depend on different major versions of reqwest (e.g. rmcp needs 0.13 while
/// the workspace pins 0.12), the type mismatch causes E0308 errors. Fix by
/// upgrading the workspace reqwest entry to the newer version, adding the
/// `blocking` and `query` features (split out in 0.13), updating feature
/// names that changed between versions (e.g. `rustls-tls` → `rustls`),
/// rewriting removed API calls in source files, and commenting out
/// `with_http_client()` calls that pass a reqwest 0.13 Client to
/// opentelemetry (which only implements HttpClient for reqwest 0.12).
fn unify_reqwest_version(src_dir: &Path) -> Result<()> {
    let publish_root = src_dir
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or_else(|| src_dir);
    let publish_manifest = publish_root.join("Cargo.toml");
    if !publish_manifest.is_file() {
        return Ok(());
    }
    // 1. Upgrade workspace reqwest version 0.12 → 0.13, adding `blocking`
    //    and `query` features (optional in 0.13, required by our code).
    let raw = fs::read_to_string(&publish_manifest)?;
    let new = raw.replace(
        "reqwest = { features = [\"cookies\"], version = \"0.12\" }",
        "reqwest = { features = [\"blocking\", \"cookies\", \"query\"], version = \"0.13\" }",
    );
    if new != raw {
        fs::write(&publish_manifest, new)?;
    }

    // 2. In all Cargo.toml files under lemurclaw-rs/, replace the `rustls-tls`
    //    feature of reqwest with `rustls` (renamed in 0.13), and upgrade
    //    any hardcoded reqwest 0.12 version references to 0.13.
    for entry in walkdir(publish_root)? {
        let path = entry.path();
        if path.file_name() != Some(std::ffi::OsStr::new("Cargo.toml")) {
            continue;
        }
        let raw = fs::read_to_string(&path)?;
        if !raw.contains("rustls-tls") && !raw.contains("reqwest") {
            continue;
        }
        let mut new = raw.replace("\"rustls-tls\"", "\"rustls\"");
        new = new.replace(
            "reqwest = { version = \"0.12\"",
            "reqwest = { version = \"0.13\"",
        );
        if new != raw {
            fs::write(&path, new)?;
        }
    }

    // 3. Fix `tls_built_in_root_certs(false).add_root_certificate(cert)` →
    //    `tls_certs_only(std::iter::once(cert))` in otel/otlp.rs.
    //    In reqwest 0.13, `tls_built_in_root_certs()` was removed in favor
    //    of `tls_certs_only()` / `tls_certs_merge()`.
    fix_reqwest_tls_built_in_root_certs(src_dir)?;

    // 4. Comment out `with_http_client(client)` calls in otel code.
    //    After the upgrade, our reqwest::Client is 0.13, but
    //    opentelemetry-http only implements HttpClient for reqwest 0.12.
    //    The default otel client (created from its own reqwest 0.12 dep)
    //    is used instead; custom TLS config is lost as a known limitation.
    fix_otel_with_http_client(src_dir)?;

    Ok(())
}

/// Replace `tls_built_in_root_certs(false).add_root_certificate(cert)` with
/// `tls_certs_only(std::iter::once(cert))` in otel source files.
/// In reqwest 0.13, `tls_built_in_root_certs()` was removed; `tls_certs_only()`
/// is the replacement that both adds the cert and disables built-in roots.
fn fix_reqwest_tls_built_in_root_certs(src_dir: &Path) -> Result<()> {
    let otlp_path = src_dir.join("otel").join("otlp.rs");
    if !otlp_path.is_file() {
        return Ok(());
    }
    let raw = fs::read_to_string(&otlp_path)?;
    if !raw.contains("tls_built_in_root_certs") {
        return Ok(());
    }
    let mut new = raw.clone();
    // Replace ALL occurrences regardless of indentation. The pattern is:
    //   .tls_built_in_root_certs(false)\n<indent>.add_root_certificate(certificate);
    // → .tls_certs_only(std::iter::once(certificate));
    // We handle both multi-line and single-line variants.
    for old in [
        ".tls_built_in_root_certs(false)\n                .add_root_certificate(certificate);",
        ".tls_built_in_root_certs(false)\n            .add_root_certificate(certificate);",
        ".tls_built_in_root_certs(false).add_root_certificate(certificate);",
    ] {
        new = new.replace(old, ".tls_certs_only(std::iter::once(certificate));");
    }
    if new != raw {
        fs::write(&otlp_path, new)?;
    }
    Ok(())
}

/// Comment out `with_http_client(client)` calls in otel source files.
/// After upgrading reqwest to 0.13, `opentelemetry_http::HttpClient` is only
/// implemented for reqwest 0.12's Client (the version otel depends on).
/// Passing our reqwest 0.13 Client causes E0277. The fix is to skip the call
/// and let otel use its default client (from its own reqwest 0.12 dep).
/// Custom TLS config is lost as a known merge limitation.
fn fix_otel_with_http_client(src_dir: &Path) -> Result<()> {
    let otel_dir = src_dir.join("otel");
    if !otel_dir.is_dir() {
        return Ok(());
    }
    fn process(dir: &Path) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                process(&path)?;
            } else if path.extension().is_some_and(|e| e == "rs") {
                let raw = fs::read_to_string(&path)?;
                if !raw.contains("with_http_client") {
                    continue;
                }
                let new = comment_out_otel_tls_blocks(&raw);
                if new != raw {
                    fs::write(&path, new)?;
                }
            }
        }
        Ok(())
    }
    process(&otel_dir)
}

/// Rewrite user-facing display text: Codex/codex → lemurclaw.
///
/// IMPORTANT: only exact full-string-literal replacements from the table
/// below. Do NOT use a generic token walk — it would corrupt env vars
/// (CODEX_*), paths (~/.codex), flag names (--codex-home), identifiers
/// (codex_home, CodexAuth), telemetry names (codex.thread.*), model slugs
/// (gpt-5.x-codex), and OpenAI infrastructure URLs. Each `old` pair is
/// anchored with surrounding context so it only matches display text.
///
/// Design choice: target specific files rather than walking the tree, because
/// (a) exact-string replace is idempotent and safe only when the string is
/// unambiguous, and (b) most `codex`/`Codex` in any given file is an
/// identifier/path that must be preserved — file-scoped replace still risks
/// false hits, so keep `old` strings long enough to be unambiguous within
/// the file. When a phrase appears multiple times identically in one file,
/// `str::replace` replaces all occurrences (which is what we want for
/// replace_all entries).
///
/// Env vars (CODEX_*), paths (~/.codex), flag names (--codex-home),
/// identifiers (codex_home, CodexAuth), telemetry (codex.thread.*),
/// model slugs (gpt-5.x-codex), and OpenAI infrastructure URLs
/// (com.openai.codex, github.com/openai/codex, @openai/codex,
/// chatgpt.com/codex) are all PRESERVED — they never appear as the `old`
/// side of any pair.
fn rewrite_brand_display_text(src_dir: &Path) -> Result<()> {
    // Each entry: (relative path under src_dir, old, new).
    // Paths are cluster-relative: cli fixup → lemurclaw-rs/cli/src/, tui fixup →
    // lemurclaw-rs/tui/src/, server fixup → lemurclaw-rs/server/src/.
    let edits: &[(&str, &str, &str)] = &[
        // ===== CLI cluster (lemurclaw-rs/cli/src/) =====
        // main.rs — top-level CLI doc comments, usage, and user messages.
        ("main.rs", "/// Codex CLI", "/// lemurclaw CLI"),
        (
            "main.rs",
            "override_usage = \"codex [OPTIONS] [PROMPT]\\n       codex [OPTIONS] <COMMAND> [ARGS]\"",
            "override_usage = \"lemurclaw [OPTIONS] [PROMPT]\\n       lemurclaw [OPTIONS] <COMMAND> [ARGS]\"",
        ),
        (
            "main.rs",
            "the generic `codex` command name that users run.",
            "the generic `lemurclaw` command name that users run.",
        ),
        (
            "main.rs",
            "/// Run Codex non-interactively.",
            "/// Run lemurclaw non-interactively.",
        ),
        (
            "main.rs",
            "/// Manage external MCP servers for Codex.",
            "/// Manage external MCP servers for lemurclaw.",
        ),
        (
            "main.rs",
            "/// Manage Codex plugins.",
            "/// Manage lemurclaw plugins.",
        ),
        (
            "main.rs",
            "/// Start Codex as an MCP server (stdio).",
            "/// Start lemurclaw as an MCP server (stdio).",
        ),
        (
            "main.rs",
            "/// Update Codex to the latest version.",
            "/// Update lemurclaw to the latest version.",
        ),
        (
            "main.rs",
            "/// Diagnose local Codex installation",
            "/// Diagnose local lemurclaw installation",
        ),
        (
            "main.rs",
            "/// Run commands within a Codex-provided sandbox.",
            "/// Run commands within a lemurclaw-provided sandbox.",
        ),
        (
            "main.rs",
            "/// Apply the latest diff produced by Codex agent",
            "/// Apply the latest diff produced by lemurclaw agent",
        ),
        (
            "main.rs",
            "/// [EXPERIMENTAL] Browse tasks from Codex Cloud",
            "/// [EXPERIMENTAL] Browse tasks from lemurclaw Cloud",
        ),
        (
            "main.rs",
            "/// [internal] Generate internal JSON Schema artifacts for Codex tooling.",
            "/// [internal] Generate internal JSON Schema artifacts for lemurclaw tooling.",
        ),
        // "this version of Codex." appears 5× in main.rs (replace_all).
        (
            "main.rs",
            "this version of Codex.",
            "this version of lemurclaw.",
        ),
        (
            "main.rs",
            "printenv OPENAI_API_KEY | codex login --with-api-key",
            "printenv OPENAI_API_KEY | lemurclaw login --with-api-key",
        ),
        (
            "main.rs",
            "printenv CODEX_ACCESS_TOKEN | codex login --with-access-token",
            "printenv CODEX_ACCESS_TOKEN | lemurclaw login --with-access-token",
        ),
        (
            "main.rs",
            "Updating Codex via `{cmd_str}`...",
            "Updating lemurclaw via `{cmd_str}`...",
        ),
        ("main.rs", "Please restart Codex.", "Please restart lemurclaw."),
        (
            "main.rs",
            "`codex update` is not available in debug builds. Install a release build of Codex to use this command.",
            "`lemurclaw update` is not available in debug builds. Install a release build of lemurclaw to use this command.",
        ),
        (
            "main.rs",
            "Could not detect the Codex installation method.",
            "Could not detect the lemurclaw installation method.",
        ),
        (
            "main.rs",
            "Codex executable path is not configured",
            "lemurclaw executable path is not configured",
        ),
        (
            "main.rs",
            "run `codex login` or set CODEX_API_KEY",
            "run `lemurclaw login` or set CODEX_API_KEY",
        ),
        (
            "main.rs",
            "Codex's interactive TUI may not work in this terminal.",
            "lemurclaw's interactive TUI may not work in this terminal.",
        ),
        (
            "main.rs",
            "failed to move damaged Codex local database files",
            "failed to move damaged lemurclaw local database files",
        ),
        (
            "main.rs",
            "`codex sandbox` is not supported on this operating system",
            "`lemurclaw sandbox` is not supported on this operating system",
        ),
        (
            "main.rs",
            "--profile only applies to runtime commands and `codex mcp`: `codex`, `codex exec`, `codex review`, `codex resume`, `codex archive`, `codex delete`, `codex unarchive`, `codex fork`, `codex mcp`, `codex sandbox`, and `codex debug prompt-input`.",
            "--profile only applies to runtime commands and `lemurclaw mcp`: `lemurclaw`, `lemurclaw exec`, `lemurclaw review`, `lemurclaw resume`, `lemurclaw archive`, `lemurclaw delete`, `lemurclaw unarchive`, `lemurclaw fork`, `lemurclaw mcp`, `lemurclaw sandbox`, and `lemurclaw debug prompt-input`.",
        ),
        // "not `codex {subcommand}`" appears 2× (replace_all).
        (
            "main.rs",
            "not `codex {subcommand}`",
            "not `lemurclaw {subcommand}`",
        ),
        (
            "main.rs",
            "`--strict-config` is not supported for `codex {subcommand}`",
            "`--strict-config` is not supported for `lemurclaw {subcommand}`",
        ),
        // exec/cli.rs
        (
            "exec/cli.rs",
            "override_usage = \"codex exec [OPTIONS] [PROMPT]\\n       codex exec [OPTIONS] <COMMAND> [ARGS]\"",
            "override_usage = \"lemurclaw exec [OPTIONS] [PROMPT]\\n       lemurclaw exec [OPTIONS] <COMMAND> [ARGS]\"",
        ),
        (
            "exec/cli.rs",
            "/// Allow running Codex outside a Git repository.",
            "/// Allow running lemurclaw outside a Git repository.",
        ),
        // exec/event_processor_with_human_output.rs — "codex".style appears 2×.
        (
            "exec/event_processor_with_human_output.rs",
            "\"codex\".style(self.italic).style(self.magenta)",
            "\"lemurclaw\".style(self.italic).style(self.magenta)",
        ),
        (
            "exec/event_processor_with_human_output.rs",
            "OpenAI Codex v{VERSION}",
            "OpenAI lemurclaw v{VERSION}",
        ),
        // exec/mod.rs (merged from exec/lib.rs)
        (
            "exec/mod.rs",
            "Error finding codex home: {err}",
            "Error finding lemurclaw home: {err}",
        ),
        // cli_internal/login.rs
        (
            "cli_internal/login.rs",
            "Use `codex login --device-auth` instead.",
            "Use `lemurclaw login --device-auth` instead.",
        ),
        (
            "cli_internal/login.rs",
            "printenv OPENAI_API_KEY | codex login --with-api-key",
            "printenv OPENAI_API_KEY | lemurclaw login --with-api-key",
        ),
        (
            "cli_internal/login.rs",
            "printenv CODEX_ACCESS_TOKEN | codex login --with-access-token",
            "printenv CODEX_ACCESS_TOKEN | lemurclaw login --with-access-token",
        ),
        // cli_internal/sandbox_setup.rs (--codex-home flag NOT touched)
        (
            "cli_internal/sandbox_setup.rs",
            "`codex sandbox setup` currently requires --elevated",
            "`lemurclaw sandbox setup` currently requires --elevated",
        ),
        // cli_internal/mcp_cmd.rs
        (
            "cli_internal/mcp_cmd.rs",
            "override_usage = \"codex mcp add [OPTIONS]",
            "override_usage = \"lemurclaw mcp add [OPTIONS]",
        ),
        (
            "cli_internal/mcp_cmd.rs",
            "Run `codex mcp login {name}` to login.",
            "Run `lemurclaw mcp login {name}` to login.",
        ),
        (
            "cli_internal/mcp_cmd.rs",
            "Try `codex mcp add my-tool -- my-command`.",
            "Try `lemurclaw mcp add my-tool -- my-command`.",
        ),
        (
            "cli_internal/mcp_cmd.rs",
            "remove: codex mcp remove {}",
            "remove: lemurclaw mcp remove {}",
        ),
        // cli_internal/marketplace_cmd.rs — bin_name + after_help strings.
        (
            "cli_internal/marketplace_cmd.rs",
            "bin_name = \"codex plugin marketplace\"",
            "bin_name = \"lemurclaw plugin marketplace\"",
        ),
        (
            "cli_internal/marketplace_cmd.rs",
            "bin_name = \"codex plugin marketplace add\"",
            "bin_name = \"lemurclaw plugin marketplace add\"",
        ),
        (
            "cli_internal/marketplace_cmd.rs",
            "bin_name = \"codex plugin marketplace list\"",
            "bin_name = \"lemurclaw plugin marketplace list\"",
        ),
        (
            "cli_internal/marketplace_cmd.rs",
            "bin_name = \"codex plugin marketplace upgrade\"",
            "bin_name = \"lemurclaw plugin marketplace upgrade\"",
        ),
        (
            "cli_internal/marketplace_cmd.rs",
            "bin_name = \"codex plugin marketplace remove\"",
            "bin_name = \"lemurclaw plugin marketplace remove\"",
        ),
        // Every "codex plugin marketplace" in after_help examples → lemurclaw.
        (
            "cli_internal/marketplace_cmd.rs",
            "codex plugin marketplace",
            "lemurclaw plugin marketplace",
        ),
        (
            "cli_internal/marketplace_cmd.rs",
            "List plugin marketplaces Codex is currently considering",
            "List plugin marketplaces lemurclaw is currently considering",
        ),
        // cli_internal/plugin_cmd.rs — bin_name + after_help strings.
        (
            "cli_internal/plugin_cmd.rs",
            "bin_name = \"codex plugin\"",
            "bin_name = \"lemurclaw plugin\"",
        ),
        (
            "cli_internal/plugin_cmd.rs",
            "bin_name = \"codex plugin add\"",
            "bin_name = \"lemurclaw plugin add\"",
        ),
        (
            "cli_internal/plugin_cmd.rs",
            "bin_name = \"codex plugin list\"",
            "bin_name = \"lemurclaw plugin list\"",
        ),
        (
            "cli_internal/plugin_cmd.rs",
            "bin_name = \"codex plugin remove\"",
            "bin_name = \"lemurclaw plugin remove\"",
        ),
        // Every "codex plugin" in after_help examples → lemurclaw plugin.
        (
            "cli_internal/plugin_cmd.rs",
            "codex plugin add",
            "lemurclaw plugin add",
        ),
        (
            "cli_internal/plugin_cmd.rs",
            "codex plugin list",
            "lemurclaw plugin list",
        ),
        (
            "cli_internal/plugin_cmd.rs",
            "codex plugin remove",
            "lemurclaw plugin remove",
        ),
        // cli_internal/state_db_recovery.rs
        (
            "cli_internal/state_db_recovery.rs",
            "Codex couldn't start because its local database appears to be damaged.",
            "lemurclaw couldn't start because its local database appears to be damaged.",
        ),
        (
            "cli_internal/state_db_recovery.rs",
            "Moving the damaged local database aside so Codex can rebuild it from saved data.",
            "Moving the damaged local database aside so lemurclaw can rebuild it from saved data.",
        ),
        (
            "cli_internal/state_db_recovery.rs",
            "Codex rebuilt its local database.",
            "lemurclaw rebuilt its local database.",
        ),
        (
            "cli_internal/state_db_recovery.rs",
            "Codex detected a damaged local database, moved it into a backup folder",
            "lemurclaw detected a damaged local database, moved it into a backup folder",
        ),
        (
            "cli_internal/state_db_recovery.rs",
            "Run `codex doctor` to check your setup",
            "Run `lemurclaw doctor` to check your setup",
        ),
        (
            "cli_internal/state_db_recovery.rs",
            "Codex couldn't start because another Codex process is using its local data.",
            "lemurclaw couldn't start because another lemurclaw process is using its local data.",
        ),
        (
            "cli_internal/state_db_recovery.rs",
            "Quit any other copies of Codex that may still be running",
            "Quit any other copies of lemurclaw that may still be running",
        ),
        // cli_internal/doctor/output.rs
        (
            "cli_internal/doctor/output.rs",
            "bold(\"Codex Doctor\", options)",
            "bold(\"lemurclaw Doctor\", options)",
        ),
        (
            "cli_internal/doctor/output.rs",
            "Run codex doctor without --summary for detailed diagnostics.",
            "Run lemurclaw doctor without --summary for detailed diagnostics.",
        ),
        // cli_internal/doctor/background.rs
        (
            "cli_internal/doctor/background.rs",
            "Run codex app-server daemon version for more details.",
            "Run lemurclaw app-server daemon version for more details.",
        ),
        // cli_internal/doctor/git.rs
        (
            "cli_internal/doctor/git.rs",
            "so Codex can inspect Git metadata.",
            "so lemurclaw can inspect Git metadata.",
        ),
        (
            "cli_internal/doctor/git.rs",
            "so Codex can inspect repository metadata.",
            "so lemurclaw can inspect repository metadata.",
        ),
        (
            "cli_internal/doctor/git.rs",
            "the bundled Git executable Codex resolves first.",
            "the bundled Git executable lemurclaw resolves first.",
        ),
        // cli_internal/doctor/runtime.rs
        (
            "cli_internal/doctor/runtime.rs",
            "repair the bundled Codex package.",
            "repair the bundled lemurclaw package.",
        ),
        // cli_internal/doctor/updates.rs (keep CODEX_MANAGED_PACKAGE_ROOT)
        (
            "cli_internal/doctor/updates.rs",
            "Reinstall or update Codex so the JS shim provides CODEX_MANAGED_PACKAGE_ROOT.",
            "Reinstall or update lemurclaw so the JS shim provides CODEX_MANAGED_PACKAGE_ROOT.",
        ),
        // cli_internal/doctor/mod.rs (merged from doctor.rs; keep
        // "PATH codex entries:" and @openai/codex npm refs untouched).
        (
            "cli_internal/doctor/mod.rs",
            "then rerun codex doctor.",
            "then rerun lemurclaw doctor.",
        ),
        (
            "cli_internal/doctor/mod.rs",
            "failed to load Codex config",
            "failed to load lemurclaw config",
        ),
        (
            "cli_internal/doctor/mod.rs",
            "Run codex login again or provide a supported auth env var.",
            "Run lemurclaw login again or provide a supported auth env var.",
        ),
        (
            "cli_internal/doctor/mod.rs",
            "no Codex credentials were found",
            "no lemurclaw credentials were found",
        ),
        (
            "cli_internal/doctor/mod.rs",
            "Run codex login or provide an API key through a supported auth env var.",
            "Run lemurclaw login or provide an API key through a supported auth env var.",
        ),
        (
            "cli_internal/doctor/mod.rs",
            "Fix auth storage access or run codex login again.",
            "Fix auth storage access or run lemurclaw login again.",
        ),
        (
            "cli_internal/doctor/mod.rs",
            "Reinstall or update Codex so the JS shim provides CODEX_MANAGED_PACKAGE_ROOT.",
            "Reinstall or update lemurclaw so the JS shim provides CODEX_MANAGED_PACKAGE_ROOT.",
        ),
        // cli_internal/doctor/thread_inventory.rs — 2 occurrences (replace_all).
        (
            "cli_internal/doctor/thread_inventory.rs",
            "Start Codex with no state DB present so startup backfill can create it from rollout files.",
            "Start lemurclaw with no state DB present so startup backfill can create it from rollout files.",
        ),
        // cloud_tasks/mod.rs (merged from cloud-tasks/src/lib.rs)
        (
            "cloud_tasks/mod.rs",
            "Please run 'codex login' to sign in with ChatGPT, then re-run 'codex cloud'.",
            "Please run 'lemurclaw login' to sign in with ChatGPT, then re-run 'lemurclaw cloud'.",
        ),
        (
            "cloud_tasks/mod.rs",
            "run `codex cloud` to list available environments",
            "run `lemurclaw cloud` to list available environments",
        ),
        (
            "cloud_tasks/mod.rs",
            "run `codex cloud` to pick the desired environment id",
            "run `lemurclaw cloud` to pick the desired environment id",
        ),
        (
            "cloud_tasks/mod.rs",
            "format!(\"codex cloud list --cursor='{cursor}'\")",
            "format!(\"lemurclaw cloud list --cursor='{cursor}'\")",
        ),

        // ===== TUI cluster (lemurclaw-rs/tui/src/, files under tui_internal/) =====
        // tui_internal/slash_command.rs
        (
            "tui_internal/slash_command.rs",
            "create an AGENTS.md file with instructions for Codex",
            "create an AGENTS.md file with instructions for lemurclaw",
        ),
        (
            "tui_internal/slash_command.rs",
            "exit Codex",
            "exit lemurclaw",
        ),
        (
            "tui_internal/slash_command.rs",
            "use skills to improve how Codex performs specific tasks",
            "use skills to improve how lemurclaw performs specific tasks",
        ),
        (
            "tui_internal/slash_command.rs",
            "choose a communication style for Codex",
            "choose a communication style for lemurclaw",
        ),
        (
            "tui_internal/slash_command.rs",
            "choose what Codex is allowed to do",
            "choose what lemurclaw is allowed to do",
        ),
        (
            "tui_internal/slash_command.rs",
            "log out of Codex",
            "log out of lemurclaw",
        ),
        // tui_internal/history_cell/session.rs
        (
            "tui_internal/history_cell/session.rs",
            " - create an AGENTS.md file with instructions for Codex",
            " - create an AGENTS.md file with instructions for lemurclaw",
        ),
        (
            "tui_internal/history_cell/session.rs",
            " - choose what Codex is allowed to do",
            " - choose what lemurclaw is allowed to do",
        ),
        (
            "tui_internal/history_cell/session.rs",
            "Span::from(\"OpenAI Codex\").bold()",
            "Span::from(\"OpenAI lemurclaw\").bold()",
        ),
        (
            "tui_internal/history_cell/session.rs",
            "format!(\"OpenAI Codex (v{})\", self.version)",
            "format!(\"OpenAI lemurclaw (v{})\", self.version)",
        ),
        (
            "tui_internal/history_cell/session.rs",
            ">_ OpenAI Codex (vX)",
            ">_ OpenAI lemurclaw (vX)",
        ),
        // tui_internal/status/card.rs
        (
            "tui_internal/status/card.rs",
            "Span::from(\"OpenAI Codex\").bold()",
            "Span::from(\"OpenAI lemurclaw\").bold()",
        ),
        (
            "tui_internal/status/card.rs",
            "API key configured (run codex login to use ChatGPT)",
            "API key configured (run lemurclaw login to use ChatGPT)",
        ),
        // tui_internal/history_cell/approvals.rs — sentence-fragment spans
        // (replace_all on each).
        (
            "tui_internal/history_cell/approvals.rs",
            " codex to run ",
            " lemurclaw to run ",
        ),
        (
            "tui_internal/history_cell/approvals.rs",
            " codex network access to ",
            " lemurclaw network access to ",
        ),
        (
            "tui_internal/history_cell/approvals.rs",
            " codex to always run commands that start with ",
            " lemurclaw to always run commands that start with ",
        ),
        (
            "tui_internal/history_cell/approvals.rs",
            " Codex network access to ",
            " lemurclaw network access to ",
        ),
        (
            "tui_internal/history_cell/approvals.rs",
            " for codex to run ",
            " for lemurclaw to run ",
        ),
        (
            "tui_internal/history_cell/approvals.rs",
            " before codex could run ",
            " before lemurclaw could run ",
        ),
        (
            "tui_internal/history_cell/approvals.rs",
            " before codex could access ",
            " before lemurclaw could access ",
        ),
        (
            "tui_internal/history_cell/approvals.rs",
            " the request for codex network access to ",
            " the request for lemurclaw network access to ",
        ),
        (
            "tui_internal/history_cell/approvals.rs",
            " for codex to apply ",
            " for lemurclaw to apply ",
        ),
        (
            "tui_internal/history_cell/approvals.rs",
            " before codex could apply ",
            " before lemurclaw could apply ",
        ),
        // tui_internal/bottom_pane/approval_overlay.rs — 5× label + 1× test
        // (replace_all keeps tests passing).
        (
            "tui_internal/bottom_pane/approval_overlay.rs",
            "No, and tell Codex what to do differently",
            "No, and tell lemurclaw what to do differently",
        ),
        (
            "tui_internal/bottom_pane/approval_overlay.rs",
            "✔ You approved codex to run",
            "✔ You approved lemurclaw to run",
        ),
        // "Ask Codex to do anything" — replace_all in these .rs files
        // (the 52 .snap files are handled separately below).
        (
            "tui_internal/bottom_pane/mod.rs",
            "Ask Codex to do anything",
            "Ask lemurclaw to do anything",
        ),
        (
            "tui_internal/bottom_pane/chat_composer.rs",
            "Ask Codex to do anything",
            "Ask lemurclaw to do anything",
        ),
        (
            "tui_internal/bottom_pane/chat_composer/slash_input.rs",
            "Ask Codex to do anything",
            "Ask lemurclaw to do anything",
        ),
        (
            "tui_internal/bottom_pane/chat_composer/history_search.rs",
            "Ask Codex to do anything",
            "Ask lemurclaw to do anything",
        ),
        (
            "tui_internal/keymap_setup.rs",
            "Ask Codex to do anything",
            "Ask lemurclaw to do anything",
        ),
        // tui_internal/model_migration.rs (do NOT touch gpt-5.x-codex slugs).
        (
            "tui_internal/model_migration.rs",
            "Codex just got an upgrade. Introducing {target_display_name}.",
            "lemurclaw just got an upgrade. Introducing {target_display_name}.",
        ),
        (
            "tui_internal/model_migration.rs",
            "Choose how you'd like Codex to proceed.",
            "Choose how you'd like lemurclaw to proceed.",
        ),
        // tui_internal/bottom_pane/list_selection_view.rs — test fixtures
        // (do NOT touch gpt-5.1-codex / gpt-4.1-codex name: fields).
        (
            "tui_internal/bottom_pane/list_selection_view.rs",
            "Optimized for Codex. Balance of reasoning quality and coding ability.",
            "Optimized for lemurclaw. Balance of reasoning quality and coding ability.",
        ),
        (
            "tui_internal/bottom_pane/list_selection_view.rs",
            "Optimized for Codex. Cheaper, faster, but less capable.",
            "Optimized for lemurclaw. Cheaper, faster, but less capable.",
        ),
        // tui_internal/bottom_pane/memories_settings_view.rs
        (
            "tui_internal/bottom_pane/memories_settings_view.rs",
            "Choose how Codex uses and creates memories.",
            "Choose how lemurclaw uses and creates memories.",
        ),
        (
            "tui_internal/bottom_pane/memories_settings_view.rs",
            "for the current Codex home.",
            "for the current lemurclaw home.",
        ),
        // tui_internal/keymap_setup/debug.rs
        (
            "tui_internal/keymap_setup/debug.rs",
            "Tip: Codex can only inspect keys your terminal sends.",
            "Tip: lemurclaw can only inspect keys your terminal sends.",
        ),
        (
            "tui_internal/keymap_setup/debug.rs",
            "your terminal is not sending that key to Codex.",
            "your terminal is not sending that key to lemurclaw.",
        ),
        (
            "tui_internal/keymap_setup/debug.rs",
            "Press any key to see what Codex receives.",
            "Press any key to see what lemurclaw receives.",
        ),
        // tui_internal/keymap_setup/picker.rs
        (
            "tui_internal/keymap_setup/picker.rs",
            "See the key Codex detects and any shortcuts assigned to it.",
            "See the key lemurclaw detects and any shortcuts assigned to it.",
        ),
        // tui_internal/pets/image_protocol.rs
        (
            "tui_internal/pets/image_protocol.rs",
            "Run Codex outside tmux to use pets.",
            "Run lemurclaw outside tmux to use pets.",
        ),
        (
            "tui_internal/pets/image_protocol.rs",
            "Run Codex outside Zellij to use pets.",
            "Run lemurclaw outside Zellij to use pets.",
        ),
        (
            "tui_internal/pets/image_protocol.rs",
            "or run Codex outside tmux.",
            "or run lemurclaw outside tmux.",
        ),
        // tui_internal/tooltips.rs (keep chatgpt.com/codex URL).
        (
            "tui_internal/tooltips.rs",
            "Run 'codex app' or visit https://chatgpt.com/codex?app-landing-page=true",
            "Run 'lemurclaw app' or visit https://chatgpt.com/codex?app-landing-page=true",
        ),
        (
            "tui_internal/tooltips.rs",
            "*New* Build faster with Codex.",
            "*New* Build faster with lemurclaw.",
        ),
        (
            "tui_internal/tooltips.rs",
            "Codex is included in your plan for free",
            "lemurclaw is included in your plan for free",
        ),
        // The filter must match the tooltip text above (lockstep).
        (
            "tui_internal/tooltips.rs",
            "line.contains(\"codex app\")",
            "line.contains(\"lemurclaw app\")",
        ),
        // tui_internal/tooltips.txt (loaded via include_str!, user-visible;
        // keep ~/.codex/config.toml, Discord URL, developers.openai.com URL,
        // community.openai.com URL untouched).
        (
            "tui_internal/tooltips.txt",
            "when Codex asks for confirmation",
            "when lemurclaw asks for confirmation",
        ),
        (
            "tui_internal/tooltips.txt",
            "ask Codex to use one",
            "ask lemurclaw to use one",
        ),
        (
            "tui_internal/tooltips.txt",
            "Run `codex app` to open the Desktop app",
            "Run `lemurclaw app` to open the Desktop app",
        ),
        (
            "tui_internal/tooltips.txt",
            "how Codex communicates",
            "how lemurclaw communicates",
        ),
        (
            "tui_internal/tooltips.txt",
            "`codex mcp add openaiDeveloperDocs --url https://developers.openai.com/mcp`",
            "`lemurclaw mcp add openaiDeveloperDocs --url https://developers.openai.com/mcp`",
        ),
        (
            "tui_internal/tooltips.txt",
            "Visit the Codex community forum: https://community.openai.com/c/codex/37",
            "Visit the lemurclaw community forum: https://community.openai.com/c/codex/37",
        ),
        (
            "tui_internal/tooltips.txt",
            "from Codex using `!`",
            "from lemurclaw using `!`",
        ),
        (
            "tui_internal/tooltips.txt",
            "See the Codex keymap documentation",
            "See the lemurclaw keymap documentation",
        ),
        (
            "tui_internal/tooltips.txt",
            "running `codex resume`",
            "running `lemurclaw resume`",
        ),
        // tui_internal/app/config_persistence.rs
        (
            "tui_internal/app/config_persistence.rs",
            "but Codex could not refresh the effective config: {err}",
            "but lemurclaw could not refresh the effective config: {err}",
        ),
        // tui_internal/app/input.rs
        (
            "tui_internal/app/input.rs",
            "before starting Codex.",
            "before starting lemurclaw.",
        ),
        // tui_internal/app/background_requests.rs — remediation messages only
        // (KEEP err.contains("codex plugins are disabled") and
        // err.contains("update codex") — they match remote server output).
        (
            "tui_internal/app/background_requests.rs",
            "Ask a workspace admin to enable Codex plugins or plugin sharing.",
            "Ask a workspace admin to enable lemurclaw plugins or plugin sharing.",
        ),
        (
            "tui_internal/app/background_requests.rs",
            "Update Codex, then try opening the shared plugin again.",
            "Update lemurclaw, then try opening the shared plugin again.",
        ),
        (
            "tui_internal/app/background_requests.rs",
            "Plugin sharing is disabled for this Codex session.",
            "Plugin sharing is disabled for this lemurclaw session.",
        ),
        // tui_internal/app/event_dispatch.rs — 2× (replace_all).
        (
            "tui_internal/app/event_dispatch.rs",
            "Codex can now safely edit files and execute commands in your computer",
            "lemurclaw can now safely edit files and execute commands in your computer",
        ),
        // tui_internal/external_agent_config_migration_flow.rs
        (
            "tui_internal/external_agent_config_migration_flow.rs",
            "Start Codex locally and run /import.",
            "Start lemurclaw locally and run /import.",
        ),
        (
            "tui_internal/external_agent_config_migration_flow.rs",
            "while Codex is connected to the local app-server daemon. Stop the daemon, restart Codex, and run /import.",
            "while lemurclaw is connected to the local app-server daemon. Stop the daemon, restart lemurclaw, and run /import.",
        ),
        // tui_internal/external_agent_config_migration/render.rs — 2×.
        (
            "tui_internal/external_agent_config_migration/render.rs",
            "Codex may add files to your current project folder.",
            "lemurclaw may add files to your current project folder.",
        ),
        // tui_internal/bottom_pane/hooks_browser_view.rs
        (
            "tui_internal/bottom_pane/hooks_browser_view.rs",
            "Right before Codex ends its turn",
            "Right before lemurclaw ends its turn",
        ),
        // tui_internal/app/thread_goal_actions.rs
        (
            "tui_internal/app/thread_goal_actions.rs",
            "Run `codex` to start a saved session, or `codex resume` / `/resume` to reopen one.",
            "Run `lemurclaw` to start a saved session, or `lemurclaw resume` / `/resume` to reopen one.",
        ),
        // tui_internal/bottom_pane/status_surface_preview.rs
        // (leave CodexVersion enum, gpt-5.2-codex model slugs untouched).
        (
            "tui_internal/bottom_pane/status_surface_preview.rs",
            "StatusSurfacePreviewItem::AppName => \"codex\"",
            "StatusSurfacePreviewItem::AppName => \"lemurclaw\"",
        ),

        // ===== Server cluster (lemurclaw-rs/server/src/) =====
        // app_server_daemon/update_loop.rs
        (
            "app_server_daemon/update_loop.rs",
            "standalone Codex updater exited with status {status}",
            "standalone lemurclaw updater exited with status {status}",
        ),
        // app_server_test_client/mod.rs (merged from lib.rs; leave --codex-bin
        // flag, codex_bin ident, SpawnCodex variant untouched).
        (
            "app_server_test_client/mod.rs",
            "author = \"Codex\", version, about = \"Bootstrap Codex app-server\"",
            "author = \"lemurclaw\", version, about = \"Bootstrap lemurclaw app-server\"",
        ),
        (
            "app_server_test_client/mod.rs",
            "started codex app-server",
            "started lemurclaw app-server",
        ),
        (
            "app_server_test_client/mod.rs",
            "[codex app-server exited: {status}]",
            "[lemurclaw app-server exited: {status}]",
        ),
        // app_server/mod.rs (merged from app-server/src/lib.rs) — SQLite state-db
        // recovery messages shown to the user. These mirror the CLI
        // state_db_recovery.rs strings already rewritten above. NOTE: leave the
        // `codex-app-server` telemetry/identifier name (OTEL_SERVICE_NAME) and
        // `codex-app-server-test-config.toml` test path untouched.
        (
            "app_server/mod.rs",
            "\"Codex rebuilt its local database.\"",
            "\"lemurclaw rebuilt its local database.\"",
        ),
        (
            "app_server/mod.rs",
            "\"Codex local database at {} appears damaged. Moving it into a backup folder so the app server can rebuild it from saved data.\"",
            "\"lemurclaw local database at {} appears damaged. Moving it into a backup folder so the app server can rebuild it from saved data.\"",
        ),
        (
            "app_server/mod.rs",
            "\"Moved damaged Codex local database file {} to {}\"",
            "\"Moved damaged lemurclaw local database file {} to {}\"",
        ),
        // chatgpt/chatgpt_client.rs (merged from chatgpt/src/chatgpt_client.rs)
        // — user-facing auth error messages. "Codex backend auth" appears 2×
        // (replace_all); the two `codex login` variants differ by backticks.
        (
            "chatgpt/chatgpt_client.rs",
            "ChatGPT backend requests require Codex backend auth",
            "ChatGPT backend requests require lemurclaw backend auth",
        ),
        (
            "chatgpt/chatgpt_client.rs",
            "ChatGPT account ID not available, please re-run `codex login`",
            "ChatGPT account ID not available, please re-run `lemurclaw login`",
        ),
        (
            "chatgpt/chatgpt_client.rs",
            "ChatGPT account ID not available, please re-run codex login",
            "ChatGPT account ID not available, please re-run lemurclaw login",
        ),
        // chatgpt/connectors.rs (merged from chatgpt/src/connectors.rs) — same
        // auth message family plus a connectors-specific variant.
        (
            "chatgpt/connectors.rs",
            "ChatGPT connectors require Codex backend auth",
            "ChatGPT connectors require lemurclaw backend auth",
        ),
        (
            "chatgpt/connectors.rs",
            "ChatGPT backend requests require Codex backend auth",
            "ChatGPT backend requests require lemurclaw backend auth",
        ),
        (
            "chatgpt/connectors.rs",
            "ChatGPT account ID not available, please re-run codex login",
            "ChatGPT account ID not available, please re-run lemurclaw login",
        ),
        // app_server_daemon/managed_install.rs — error messages about the
        // managed install. "managed Codex" appears 5× across binary/version
        // messages (replace_all). NOTE: the `codex`/`codex.exe` binary basename
        // and `codex_bin` ident below them are NOT touched (they resolve the
        // real on-disk binary).
        (
            "app_server_daemon/managed_install.rs",
            "managed Codex",
            "managed lemurclaw",
        ),
        // app_server_daemon/client.rs — notification title shown to the user.
        (
            "app_server_daemon/client.rs",
            "\"Codex App Server Daemon\"",
            "\"lemurclaw App Server Daemon\"",
        ),
        // memories_write/guard.rs (merged from memories/write/src/guard.rs) —
        // startup warning message.
        (
            "memories_write/guard.rs",
            "skipping memories startup because Codex rate limits are below the configured threshold",
            "skipping memories startup because lemurclaw rate limits are below the configured threshold",
        ),
    ];

    let mut changed = 0usize;
    for (rel, old, new) in edits {
        let path = src_dir.join(rel);
        if path.is_file() {
            let raw =
                fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
            if raw.contains(old) {
                fs::write(&path, raw.replace(old, new))
                    .with_context(|| format!("write {}", path.display()))?;
                changed += 1;
            }
        }
    }

    // Snapshots: replace ONLY the "Ask Codex to do anything" placeholder in
    // insta .snap files under the tui src tree (decision (a)). Do NOT touch
    // any other codex/Codex inside .snap files — they contain model slugs,
    // identifiers, etc. that must stay. This keeps tests passing without a
    // separate `cargo insta accept` step.
    let snap_old = "Ask Codex to do anything";
    let snap_new = "Ask lemurclaw to do anything";
    fn rewrite_snaps(dir: &Path, old: &str, new: &str, changed: &mut usize) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                rewrite_snaps(&path, old, new, changed)?;
            } else if path.extension().is_some_and(|e| e == "snap") {
                let raw = fs::read_to_string(&path)
                    .with_context(|| format!("read {}", path.display()))?;
                if raw.contains(old) {
                    fs::write(&path, raw.replace(old, new))
                        .with_context(|| format!("write {}", path.display()))?;
                    *changed += 1;
                }
            }
        }
        Ok(())
    }
    rewrite_snaps(src_dir, snap_old, snap_new, &mut changed)?;

    if changed > 0 {
        println!(
            "  ✓ Rewrote brand display text (Codex→lemurclaw) in {} files",
            changed
        );
    }
    Ok(())
}

/// Full-scope brand rewriter: Codex/codex → lemurclaw across the whole
/// `lemurclaw-rs/` tree, covering categories BEYOND display text — environment
/// variables (`CODEX_*` → `LEMURCLAW_*`), filesystem paths (`~/.codex` →
/// `~/.lemurclaw`, `/etc/codex/` → `/etc/lemurclaw/`), CLI flags
/// (`--codex-*` → `--lemurclaw-*`), internal protocol identifiers
/// (`codex://`, `CodexErrorInfo` type name, protobuf packages, internal
/// type/daemon/binary names), system prompts (model identity text), and
/// emit-only telemetry names.
///
/// ## Why a separate function from `rewrite_brand_display_text`
/// That function is an exact-string table (each `old` anchored with enough
/// context to be unambiguous within a file) because it targets `.rs` files
/// where brand words sit next to identifiers that must stay (`codex_home`,
/// `CodexAuth`). Most `codex`/`Codex` here is pure brand with no such
/// collisions, so a tree scan with a B-zone placeholder guard is cleaner and
/// more maintainable than hand-listing hundreds of pairs.
///
/// ## Why a scan, not merge-relative paths
/// Merge moves member `src/` under unpredictable module dirs (`core_internal/`,
/// `login/`, …), so fixed paths are fragile. Scanning `publish_root` and
/// editing any matching source file is robust to merge re-pathing.
///
/// ## Scope split (A-zone here; B-zone preserved)
/// The B-zone — strings sent to the OpenAI cloud and validated/consumed
/// there — is PRESERVED via placeholder protection. lemurclaw talks directly
/// to OpenAI's cloud (`chatgpt.com/codex-backend`), so renaming those would
/// break live integrations. B-zone = model slugs (`gpt-5.x-codex`), the
/// `originator` header values (`codex_cli_rs` etc.), the JWT audience
/// (`codex-app-server` in `agent-identity`), the `codex_exec` originator,
/// analytics `event_type` wire values, and the `codexErrorInfo` JSON field
/// name (on-wire, brand-irrelevant). Each is stashed before editing and
/// restored after, so a loose `\bCodex\b` match can never touch them.
pub(crate) fn rewrite_brand_full(publish_root: &Path) -> Result<()> {
    println!("\n🔄 Full-scope brand rewrite (Codex→lemurclaw) across lemurclaw-rs/…");

    // Files worth scanning. Skip target/ (walkdir already does), .snap
    // (regenerated by tests), .lock (handled by rename), and build artifacts.
    let wanted_exts: &[&str] = &["rs", "toml", "json", "ts", "md", "txt", "sh", "html", "py"];

    // B-zone tokens to protect: stash → rewrite → restore. Each is a literal
    // string that must survive unchanged. Order matters only for determinism.
    // (See function docstring for why each is preserved.)
    let protected: &[&str] = &[
        // Model slugs (real OpenAI API model names).
        "gpt-5.3-codex",
        "gpt-5.2-codex",
        "gpt-5.1-codex",
        "gpt-5-codex",
        "gpt-5.1-codex-max",
        "gpt-5.2-codex-sonic",
        // Originator header values (cloud gates first-party on these).
        "codex_cli_rs",
        "codex-tui",
        "codex_vscode",
        "codex_atlas",
        "codex_chatgpt_desktop",
        "codex_desktop",
        "codex-cli",
        "codex-app-server-sdk",
        "codex_sdk_ts",
        // JWT audience (cloud validates the `aud` claim).
        "codex-app-server",
        // codex_exec originator (outbound Originator header).
        "codex_exec",
        // Analytics event_type wire values (backend consumes by name).
        "codex_app_mentioned",
        "codex_app_used",
        "codex_thread_initialized",
        "codex_turn_event",
        "codex_turn_steer_event",
        "codex_goal_event",
        "codex_guardian_review",
        "codex_hook_run",
        "codex_review_event",
        "codex_command_execution_event",
        "codex_compaction_event",
        "codex_web_search_event",
        "codex_image_generation_event",
        "codex_mcp_tool_call_event",
        "codex_dynamic_tool_call_event",
        "codex_collab_agent_tool_call_event",
        "codex_file_change_event",
        "codex_plugin_used",
        "codex_plugin_enabled",
        "codex_plugin_disabled",
        "codex_plugin_installed",
        "codex_plugin_uninstalled",
        "codex_plugin_install_requested",
        "codex_plugin_install_failed",
        "codex_onboarding_external_agent_import_complete",
        "codex_onboarding_external_agent_import_failure",
        // On-wire JSON field name (brand-irrelevant; renaming changes wire).
        "codexErrorInfo",
        // Cloud URLs / OpenAI infrastructure.
        "openai/codex",
        "chatgpt.com/codex",
        "developers.openai.com/codex",
        "community.openai.com/c/codex",
        "@openai/codex",
        "com.openai.codex",
        "codex-backend",
    ];

    let mut files_scanned = 0usize;
    let mut files_changed = 0usize;
    let mut occ_total = 0usize;

    for entry in walkdir(publish_root)? {
        let path = entry.path();
        // Extension filter.
        let is_wanted = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| wanted_exts.contains(&e));
        if !is_wanted {
            continue;
        }
        // Skip snapshot dirs entirely (tests regenerate them).
        if path.components().any(|c| c.as_os_str() == "snapshots") {
            continue;
        }

        let raw = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue, // binary/symlink/non-utf8 — skip quietly.
        };
        files_scanned += 1;

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let new = rewrite_brand_full_text(&raw, ext, protected, &mut occ_total);
        if new != raw {
            fs::write(&path, &new).with_context(|| format!("write {}", path.display()))?;
            files_changed += 1;
        }
    }

    println!(
        "  ✓ Full brand rewrite: {} occurrences in {} files (scanned {})",
        occ_total, files_changed, files_scanned
    );

    // File renames: when a generated file is named after a type/identifier that
    // the content rewrite changed, the filename must follow or imports break.
    // E.g. `CodexErrorInfo.ts` whose content now exports `LemurclawErrorInfo`,
    // referenced by `index.ts` as `from "./LemurclawErrorInfo"`. Rename the
    // file to match. (Generated schemas are regenerated upstream; this keeps
    // the publish tree internally consistent.)
    let file_renames: &[(&str, &str)] = &[
        ("CodexErrorInfo.ts", "LemurclawErrorInfo.ts"),
        ("CodexErrorInfo.json", "LemurclawErrorInfo.json"),
        // Generated proto files named after the proto package. A4 rewrites the
        // package string (`codex.thread_config.v1` → `lemurclaw.…`) inside
        // `#[path = "..."]` attrs and SERVICE_NAME consts, so the generated
        // `.rs`/`.proto` files must follow or the `#[path]` include breaks.
        ("codex.thread_config.v1.rs", "lemurclaw.thread_config.v1.rs"),
        (
            "codex.thread_config.v1.proto",
            "lemurclaw.thread_config.v1.proto",
        ),
        (
            "codex.exec_server.relay.v1.rs",
            "lemurclaw.exec_server.relay.v1.rs",
        ),
        (
            "codex.exec_server.relay.v1.proto",
            "lemurclaw.exec_server.relay.v1.proto",
        ),
    ];
    let mut renamed = 0usize;
    for entry in walkdir(publish_root)? {
        let path = entry.path();
        let fname = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        let new_name = file_renames
            .iter()
            .find(|(old, _)| *old == fname)
            .map(|(_, new)| *new);
        if let Some(new_name) = new_name {
            let new_path = path.with_file_name(new_name);
            fs::rename(&path, &new_path)
                .with_context(|| format!("rename {} → {}", path.display(), new_path.display()))?;
            renamed += 1;
        }
    }
    if renamed > 0 {
        println!(
            "  ✓ Renamed {} generated schema files to match type names",
            renamed
        );
    }

    Ok(())
}

/// Apply the A-zone brand rewrites to a single file's text, with B-zone
/// `protected` tokens stashed via placeholders and restored afterwards.
/// `occ_count` accumulates the number of A-zone replacements made.
///
/// `ext` is the file extension (no dot): `.rs` files get the surgical
/// literal-table treatment only (identifiers must not be touched); non-`.rs`
/// prose files (`.md`, `.txt`, `.json`, `.ts`, `.html`) additionally get a
/// word-boundary `Codex`/`codex` → `lemurclaw` pass for brand/identity text.
fn rewrite_brand_full_text(
    raw: &str,
    ext: &str,
    protected: &[&str],
    occ_count: &mut usize,
) -> String {
    // 1. Stash B-zone tokens behind unique placeholders that contain no
    //    `codex`/`Codex` substring, so subsequent rewrites can't touch them.
    let mut s = String::with_capacity(raw.len());
    let mut stashed: Vec<String> = Vec::new();
    let mut cur = raw;
    let mut buf = String::new();
    for tok in protected {
        if !cur.contains(tok) {
            continue;
        }
        buf.clear();
        let mut last = 0;
        for (idx, _) in cur.match_indices(tok) {
            buf.push_str(&cur[last..idx]);
            let marker = format!("\x00BRAND_PROTECT_{}\x00", stashed.len());
            buf.push_str(&marker);
            stashed.push((*tok).to_string());
            last = idx + tok.len();
        }
        buf.push_str(&cur[last..]);
        s = buf.clone();
        cur = &s;
    }
    let working = if stashed.is_empty() {
        raw.to_string()
    } else {
        s
    };

    // 2. Apply A-zone rewrites. Each rule is (find, replace). Order matters:
    //    longer/more-specific patterns first so they win over generic ones.
    let rules: &[(&str, &str)] = &[
        // --- A1: environment variable VALUES (string literals) ---
        // These are the on-wire env-var names read at runtime. The constant
        // *identifiers* (CODEX_HOME_ENV_VAR) are already renamed by
        // source_rewrite's AST pass; only the quoted string literals remain.
        // Match the quoted form to stay surgical.
        (
            "\"CODEX_SANDBOX_NETWORK_DISABLED\"",
            "\"LEMURCLAW_SANDBOX_NETWORK_DISABLED\"",
        ),
        ("\"CODEX_SANDBOX\"", "\"LEMURCLAW_SANDBOX\""),
        ("\"CODEX_HOME\"", "\"LEMURCLAW_HOME\""),
        ("\"CODEX_API_KEY\"", "\"LEMURCLAW_API_KEY\""),
        ("\"CODEX_ACCESS_TOKEN\"", "\"LEMURCLAW_ACCESS_TOKEN\""),
        ("\"CODEX_SQLITE_HOME\"", "\"LEMURCLAW_SQLITE_HOME\""),
        ("\"CODEX_CA_CERTIFICATE\"", "\"LEMURCLAW_CA_CERTIFICATE\""),
        (
            "\"CODEX_PERMISSION_PROFILE\"",
            "\"LEMURCLAW_PERMISSION_PROFILE\"",
        ),
        ("\"CODEX_THREAD_ID\"", "\"LEMURCLAW_THREAD_ID\""),
        ("\"CODEX_ESCALATE_SOCKET\"", "\"LEMURCLAW_ESCALATE_SOCKET\""),
        (
            "\"CODEX_ROLLOUT_TRACE_ROOT\"",
            "\"LEMURCLAW_ROLLOUT_TRACE_ROOT\"",
        ),
        (
            "\"CODEX_CONNECTORS_TOKEN\"",
            "\"LEMURCLAW_CONNECTORS_TOKEN\"",
        ),
        (
            "\"CODEX_CODE_MODE_HOST_PATH\"",
            "\"LEMURCLAW_CODE_MODE_HOST_PATH\"",
        ),
        ("\"CODEX_EXEC_SERVER_URL\"", "\"LEMURCLAW_EXEC_SERVER_URL\""),
        (
            "\"CODEX_EXEC_SERVER_NOISE_AUTH_TOKEN\"",
            "\"LEMURCLAW_EXEC_SERVER_NOISE_AUTH_TOKEN\"",
        ),
        (
            "\"CODEX_AUTHAPI_BASE_URL\"",
            "\"LEMURCLAW_AUTHAPI_BASE_URL\"",
        ),
        (
            "\"CODEX_REFRESH_TOKEN_URL_OVERRIDE\"",
            "\"LEMURCLAW_REFRESH_TOKEN_URL_OVERRIDE\"",
        ),
        (
            "\"CODEX_REVOKE_TOKEN_URL_OVERRIDE\"",
            "\"LEMURCLAW_REVOKE_TOKEN_URL_OVERRIDE\"",
        ),
        (
            "\"CODEX_APP_SERVER_LOGIN_CLIENT_ID\"",
            "\"LEMURCLAW_APP_SERVER_LOGIN_CLIENT_ID\"",
        ),
        (
            "\"CODEX_INTERNAL_ORIGINATOR_OVERRIDE\"",
            "\"LEMURCLAW_INTERNAL_ORIGINATOR_OVERRIDE\"",
        ),
        (
            "\"CODEX_NETWORK_PROXY_ACTIVE\"",
            "\"LEMURCLAW_NETWORK_PROXY_ACTIVE\"",
        ),
        (
            "\"CODEX_NETWORK_ALLOW_LOCAL_BINDING\"",
            "\"LEMURCLAW_NETWORK_ALLOW_LOCAL_BINDING\"",
        ),
        (
            "\"CODEX_NETWORK_PROXY_CREDENTIAL_BROKER_ACTIVE\"",
            "\"LEMURCLAW_NETWORK_PROXY_CREDENTIAL_BROKER_ACTIVE\"",
        ),
        (
            "\"CODEX_NETWORK_PROXY_BROKERED_CREDENTIALS\"",
            "\"LEMURCLAW_NETWORK_PROXY_BROKERED_CREDENTIALS\"",
        ),
        (
            "\"CODEX_NETWORK_PROXY_ATTRIBUTION\"",
            "\"LEMURCLAW_NETWORK_PROXY_ATTRIBUTION\"",
        ),
        (
            "\"CODEX_NETWORK_POLICY_VIOLATION\"",
            "\"LEMURCLAW_NETWORK_POLICY_VIOLATION\"",
        ),
        (
            "\"CODEX_PROXY_GIT_SSH_COMMAND\"",
            "\"LEMURCLAW_PROXY_GIT_SSH_COMMAND\"",
        ),
        // Managed-install contract (JS shim + Rust; scattered, no constant).
        ("CODEX_MANAGED_BY_NPM", "LEMURCLAW_MANAGED_BY_NPM"),
        ("CODEX_MANAGED_BY_PNPM", "LEMURCLAW_MANAGED_BY_PNPM"),
        ("CODEX_MANAGED_BY_BUN", "LEMURCLAW_MANAGED_BY_BUN"),
        (
            "CODEX_MANAGED_PACKAGE_ROOT",
            "LEMURCLAW_MANAGED_PACKAGE_ROOT",
        ),
        // Cloud-tasks inline reads.
        (
            "CODEX_CLOUD_TASKS_BASE_URL",
            "LEMURCLAW_CLOUD_TASKS_BASE_URL",
        ),
        ("CODEX_CLOUD_TASKS_MODE", "LEMURCLAW_CLOUD_TASKS_MODE"),
        (
            "CODEX_CLOUD_TASKS_FORCE_INTERNAL",
            "LEMURCLAW_CLOUD_TASKS_FORCE_INTERNAL",
        ),
        // Remote auth token.
        ("CODEX_REMOTE_AUTH_TOKEN", "LEMURCLAW_REMOTE_AUTH_TOKEN"),
        // Misc inline env vars.
        ("CODEX_GITHUB_TOKEN", "LEMURCLAW_GITHUB_TOKEN"),
        ("CODEX_STARTING_DIFF", "LEMURCLAW_STARTING_DIFF"),
        // Generic catch-up for any remaining quoted CODEX_* test/dev vars
        // (CODEX_TEST_*, CODEX_APP_SERVER_*, CODEX_LOG, CODEX_BIN, …). The
        // quoted form keeps it to string literals, not identifiers.
        // NOTE: applied last via the loop below as a prefix rule.
    ];

    let mut out = working;
    for (find, repl) in rules {
        if out.contains(find) {
            let n = out.matches(find).count();
            *occ_count += n;
            out = out.replace(find, repl);
        }
    }
    // Generic quoted-prefix catch-up for remaining CODEX_* string literals
    // not enumerated above (test/dev/build markers). Anchored on the quoted
    // `"CODEX_` opener to stay within string literals.
    out = replace_quoted_prefix(&out, "\"CODEX_", "\"LEMURCLAW_", occ_count);

    // --- A2: filesystem paths ---
    // `.codex` directory basename → `.lemurclaw`, and `/etc/codex` →
    // `/etc/lemurclaw`. These are applied broadly because the `.codex` dir
    // name is always a path, never an identifier — but `com.openai.codex`
    // (the macOS bundle id / OpenAI infra) is B-zone-protected above, so the
    // `.codex` substring inside it is already stashed. We match `.codex`
    // when followed by a path-terminator (quote, slash, whitespace, `$`) so
    // we don't graze into `codex_home` etc. (which only exist in .rs anyway).
    //
    // Match `.codex"` (end of a quoted/raw-string path, any preceding char),
    // `.codex/` (path continuation), and `.codex` in `$HOME/.codex`-style.
    let path_rules: &[(&str, &str)] = &[
        // `.codex` at end of a quoted path segment (e.g. `r"...\.codex"`,
        // `join(".codex")`, `.expect("create .codex")`).
        (".codex\"", ".lemurclaw\""),
        // `/etc/codex` system-config dir (in strings AND comments).
        ("/etc/codex", "/etc/lemurclaw"),
        // `~/.codex` and `$HOME/.codex` (home references).
        ("~/.codex", "~/.lemurclaw"),
        // The home-dir `.push(".codex")` / `.join(".codex")` literals.
        ("(\".codex\")", "(\".lemurclaw\")"),
    ];
    for (find, repl) in path_rules {
        if out.contains(find) {
            let n = out.matches(find).count();
            *occ_count += n;
            out = out.replace(find, repl);
        }
    }

    // --- A3: CLI flags --codex-* → --lemurclaw-* ---
    // Process self-re-exec contract; both emitter and matcher are renamed.
    // The `--codex-` prefix is unambiguous (no identifier uses it).
    if out.contains("--codex-") {
        let n = out.matches("--codex-").count();
        *occ_count += n;
        out = out.replace("--codex-", "--lemurclaw-");
    }

    // --- A4: internal protocol identifiers ---
    let proto_rules: &[(&str, &str)] = &[
        // URL scheme (emit-side only in this repo).
        ("codex://", "lemurclaw://"),
        // Protobuf packages (local gRPC; both ends in-tree).
        ("codex.thread_config.v1", "lemurclaw.thread_config.v1"),
        (
            "codex.exec_server.relay.v1",
            "lemurclaw.exec_server.relay.v1",
        ),
        // Daemon client name (local Unix socket).
        ("codex_app_server_daemon", "lemurclaw_app_server_daemon"),
        // arg0 helper binary basenames.
        ("codex-linux-sandbox", "lemurclaw-linux-sandbox"),
        ("codex-execve-wrapper", "lemurclaw-execve-wrapper"),
    ];
    for (find, repl) in proto_rules {
        if out.contains(find) {
            let n = out.matches(find).count();
            *occ_count += n;
            out = out.replace(find, repl);
        }
    }
    // `CodexErrorInfo` TYPE name → `LemurclawErrorInfo` (not on-wire; only the
    // TS type name + JSON schema $ref def name + Rust enum name). The field
    // name `codexErrorInfo` is B-zone-protected above.
    if out.contains("CodexErrorInfo") {
        let n = out.matches("CodexErrorInfo").count();
        *occ_count += n;
        out = out.replace("CodexErrorInfo", "LemurclawErrorInfo");
    }

    // --- A6: emit-only telemetry names + OTEL service.name ---
    // `codex-app-server` as an OTEL service.name tag is emit-only (cloud
    // doesn't reject on it). The JWT audience of the same literal is
    // B-zone-protected above. Other `codex.*` metric names are emit-only too.
    let telem_rules: &[(&str, &str)] = &[
        // Metric name prefixes (emit-only; OTel backend just sees a new name).
        ("codex.thread.", "lemurclaw.thread."),
        ("codex.windows_sandbox.", "lemurclaw.windows_sandbox."),
        ("codex.apps.refresh.", "lemurclaw.apps.refresh."),
        ("codex.apps.installed.", "lemurclaw.apps.installed."),
        (
            "codex.cloud_config_bundle.",
            "lemurclaw.cloud_config_bundle.",
        ),
    ];
    for (find, repl) in telem_rules {
        if out.contains(find) {
            let n = out.matches(find).count();
            *occ_count += n;
            out = out.replace(find, repl);
        }
    }

    // --- A5: system prompt / brand-word identity text ---
    // Pure brand `\bCodex\b` → `lemurclaw` and `\bcodex\b` → `lemurclaw` in
    // prose/doc/prompt contexts. Applied LAST and ONLY to non-.rs files,
    // because .rs source is full of identifiers (`codex_home`, `CodexAuth`)
    // that must stay — those are handled by source_rewrite's AST pass, not
    // here. Doing this broadly on .rs would corrupt identifiers. Brand-word
    // rewrite for .rs *display* text is already covered by
    // rewrite_brand_display_text's exact-string table.
    //
    // Non-.rs prose files (prompts, tooltips.txt, README/docs, models.json
    // instructions_template) carry Codex as the product/model identity with
    // no identifier collisions, so a word-boundary replace is safe here.
    // B-zone tokens (model slugs, URLs, analytics names) are already stashed.
    let is_prose = match ext {
        "rs" => false,
        _ => true,
    };
    if is_prose {
        // Titlecase "Codex" → "lemurclaw" on word boundaries. Use the ASCII
        // boundary heuristic: Codex preceded by start/non-alnum and followed
        // by non-alnum (so "CodexAuth" / "CodexErrorInfo" — already protected
        // or identifier-adjacent — are NOT matched; only the standalone word).
        out = replace_word_boundary(&out, "Codex", "lemurclaw", occ_count);
        // lowercase "codex" → "lemurclaw" on word boundaries (e.g. "run codex",
        // "the codex cli"). Protects "codex_home"/"codex://" (the latter is
        // already rewritten above to lemurclaw://, and the former only appears
        // in .rs which is excluded here).
        out = replace_word_boundary(&out, "codex", "lemurclaw", occ_count);
    }

    // 3. Restore B-zone tokens.
    if stashed.is_empty() {
        return out;
    }
    let mut restored = out;
    for (i, tok) in stashed.iter().enumerate() {
        let marker = format!("\x00BRAND_PROTECT_{}\x00", i);
        if restored.contains(&marker) {
            restored = restored.replace(&marker, tok);
        }
    }
    restored
}

/// Replace `needle` with `repl` only where it appears as a standalone word
/// (bounded by non-alphanumeric `_`-excluding edges on both sides). This
/// prevents matching `needle` inside larger identifiers — e.g. `Codex` won't
/// match in `CodexAuth`, and `codex` won't match in `codex_home` or
/// `codex://`. Counts replacements into `occ_count`.
fn replace_word_boundary(s: &str, needle: &str, repl: &str, occ_count: &mut usize) -> String {
    if needle.is_empty() || !s.contains(needle) {
        return s.to_string();
    }
    // A word boundary char is anything that is NOT an ASCII alphanumeric.
    // (Underscore IS treated as a word char here, matching Rust ident rules,
    // so `codex_home` stays intact.)
    let is_boundary = |c: char| !c.is_ascii_alphanumeric() && c != '_';
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let nb = needle.as_bytes();
    let mut i = 0;
    let mut count = 0usize;
    while i < bytes.len() {
        if i + nb.len() <= bytes.len() && &bytes[i..i + nb.len()] == nb {
            // Check left boundary (start-of-string or boundary char before).
            let left_ok = i == 0 || s[..i].chars().next_back().is_some_and(is_boundary);
            // Check right boundary (end-of-string or boundary char after).
            let right_ok = if i + nb.len() == bytes.len() {
                true
            } else {
                s[i + nb.len()..].chars().next().is_some_and(is_boundary)
            };
            if left_ok && right_ok {
                out.push_str(repl);
                i += nb.len();
                count += 1;
                continue;
            }
        }
        // Advance by one char to stay UTF-8 safe.
        let ch = s[i..].chars().next().unwrap_or('_');
        out.push(ch);
        i += ch.len_utf8();
    }
    *occ_count += count;
    out
}

/// Replace every occurrence of a quoted string-literal opener prefix.
/// Walks `s` finding each `prefix` (e.g. `"CODEX_`) and, for the matching
/// close quote, rewrites the opener to `repl` (e.g. `"LEMURCLAW_`). Only the
/// opener prefix changes; the rest of the literal (the variable suffix) is
//  untouched. Counts replacements into `occ_count`.
fn replace_quoted_prefix(s: &str, prefix: &str, repl: &str, occ_count: &mut usize) -> String {
    if !s.contains(prefix) {
        return s.to_string();
    }
    // Count distinct string-literal occurrences of the opener.
    let n = s.matches(prefix).count();
    *occ_count += n;
    s.replace(prefix, repl)
}

/// Comment out `if let Some(tls) = tls.as_ref() { ... with_http_client ... }` blocks
/// and standalone `with_http_client` calls in otel source files. These pass a reqwest
/// Client to otel's with_http_client, which fails because otel's HttpClient trait is
/// only implemented for reqwest 0.12.
fn comment_out_otel_tls_blocks(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut modified = false;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Pattern A: `if let Some(tls) = tls.as_ref() {` where with_http_client
        // appears in the next few lines. Replace entire block with `let _ = tls;`.
        if trimmed.starts_with("if let Some(tls) = tls.as_ref() {")
            && i + 1 < lines.len()
            && lines[i + 1..]
                .iter()
                .take(5)
                .any(|l| l.contains("with_http_client"))
        {
            let indent = line.len() - line.trim_start().len();
            let indent_str: String = line.chars().take(indent).collect();
            // Find closing brace
            let mut brace_depth = 1;
            let mut j = i + 1;
            while j < lines.len() && brace_depth > 0 {
                for ch in lines[j].chars() {
                    match ch {
                        '{' => brace_depth += 1,
                        '}' => brace_depth -= 1,
                        _ => {}
                    }
                }
                if brace_depth == 0 {
                    break;
                }
                j += 1;
            }
            // Replace entire block with comment + `let _ = tls;`
            out.push_str(&format!(
                "{}// TODO(merge): Custom TLS client disabled — reqwest 0.13 (rmcp) and\n",
                indent_str
            ));
            out.push_str(&format!(
                "{}//                opentelemetry-http (reqwest 0.12) are incompatible.\n",
                indent_str
            ));
            out.push_str(&format!("{}let _ = tls;\n", indent_str));
            i = j + 1;
            modified = true;
        }
        // Pattern B: standalone `exporter_builder = exporter_builder.with_http_client(client);`
        // This appears for async clients where the client was just built unconditionally.
        // We also need to comment out the preceding `let client = ...build_async_http_client(...)?;`
        // which may span multiple lines. Since those lines were already emitted to `out`,
        // we remove them and replace with a comment.
        else if trimmed == "exporter_builder = exporter_builder.with_http_client(client);" {
            let indent = line.len() - line.trim_start().len();
            let indent_str: String = line.chars().take(indent).collect();

            // Walk backwards to find how many preceding lines form the `let client =` block
            let mut lines_to_remove = 0;
            if i > 0 {
                for k in (0..i).rev() {
                    if lines[k].contains("build_async_http_client") {
                        lines_to_remove = i - k;
                        break;
                    }
                    // Stop if we hit a line that's clearly not part of the expression
                    if !lines[k].trim().is_empty()
                        && !lines[k].trim().starts_with(")?;")
                        && !lines[k].contains("build_async_http_client")
                        && !lines[k].trim().ends_with(',')
                    {
                        break;
                    }
                }
            }

            // Remove the already-emitted lines from `out` by truncating
            for _ in 0..lines_to_remove {
                // Remove last line from out
                if let Some(pos) = out.rfind('\n') {
                    out.truncate(pos);
                }
            }

            out.push_str(&format!(
                "\n{}// TODO(merge): with_http_client disabled — reqwest 0.13 (rmcp) and\n",
                indent_str
            ));
            out.push_str(&format!(
                "{}//                opentelemetry-http (reqwest 0.12) are incompatible.\n",
                indent_str
            ));
            out.push_str(&format!("{}let _ = tls;\n", indent_str));
            i += 1;
            modified = true;
        } else {
            out.push_str(line);
            out.push('\n');
            i += 1;
        }
    }
    if modified {
        if !content.ends_with('\n') && out.ends_with('\n') {
            out.pop();
        }
        out
    } else {
        content.to_string()
    }
}

/// Recursively collect all paths under dir.
fn walkdir(dir: &Path) -> Result<Vec<std::fs::DirEntry>> {
    let mut result = Vec::new();
    fn walk(dir: &Path, result: &mut Vec<std::fs::DirEntry>) -> std::io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() && path.file_name() != Some(std::ffi::OsStr::new("target")) {
                walk(&path, result)?;
            } else {
                result.push(entry);
            }
        }
        Ok(())
    }
    walk(dir, &mut result).with_context(|| format!("walk {}", dir.display()))?;
    Ok(result)
}

/// Fix `include_dir!("$CARGO_MANIFEST_DIR/src/...")` paths in member modules.
/// After merge, a member crate's `src/` directory became `src/<module>/`, so
/// any `$CARGO_MANIFEST_DIR/src/` reference needs the module name inserted.
fn fix_include_dir_manifest_dir_paths(src_dir: &Path) -> Result<()> {
    fn process(dir: &Path, src_dir: &Path) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                process(&path, src_dir)?;
            } else if path.extension().is_some_and(|e| e == "rs") {
                let raw = fs::read_to_string(&path)?;
                // Only process files containing `$CARGO_MANIFEST_DIR/src/`
                if !raw.contains("$CARGO_MANIFEST_DIR/src/") {
                    continue;
                }
                // Determine the module name from the file path relative to src_dir.
                // path = src_dir/<module>/[...]/file.rs → module = first component.
                let rel = path.strip_prefix(src_dir).unwrap_or(&path);
                let module = rel
                    .components()
                    .next()
                    .and_then(|c| c.as_os_str().to_str())
                    .unwrap_or("");
                if module.is_empty() {
                    continue;
                }
                // Check if the referenced path exists at src/<module>/... but not
                // at src/... (i.e., the path needs the module inserted).
                let old_prefix = "$CARGO_MANIFEST_DIR/src/";
                let new_prefix = format!("$CARGO_MANIFEST_DIR/src/{}/", module);
                let mut out = raw.clone();
                // For each occurrence, check if the path resolves with the module
                // name inserted but not without it.
                let mut search_from = 0usize;
                while let Some(pos) = out[search_from..].find(old_prefix) {
                    let abs_pos = search_from + pos;
                    let after_start = abs_pos + old_prefix.len();
                    // Read to the closing quote.
                    let path_tail = out[after_start..]
                        .find('"')
                        .map(|end| out[after_start..after_start + end].to_string());
                    if let Some(path_tail) = path_tail {
                        let crate_root = src_dir.parent().unwrap_or(Path::new("."));
                        let without_module = crate_root.join("src").join(&path_tail);
                        let with_module = crate_root.join("src").join(module).join(&path_tail);
                        if !without_module.exists() && with_module.exists() {
                            // Replace: insert module name.
                            let end = after_start + path_tail.len();
                            out.replace_range(
                                abs_pos..end,
                                &format!("{}{}", new_prefix, path_tail),
                            );
                            // Continue searching after the replacement.
                            search_from = abs_pos + new_prefix.len() + path_tail.len();
                            continue;
                        }
                    }
                    // No fix needed — advance past this occurrence.
                    search_from = abs_pos + old_prefix.len();
                }
                if out != raw {
                    fs::write(&path, out)?;
                }
            }
        }
        Ok(())
    }
    process(src_dir, src_dir)
}
/// CamelCase/UPPER_CASE items (exec_server_protocol types/constants like
/// EXEC_METHOD, ExecParams). Lowercase submodule names (capabilities, models,
/// permissions) stay as `crate::protocol::` (member protocol submodules).
fn rewrite_protocol_refs_in_exec_server(dir: &Path) -> Result<()> {
    fn process(dir: &Path) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                process(&path)?;
            } else if path.extension().is_some_and(|e| e == "rs") {
                let raw = fs::read_to_string(&path)?;
                // Find all `crate::protocol::<item>` and check if <item> is
                // CamelCase or UPPER_CASE (exec_server_protocol item).
                let mut out = String::with_capacity(raw.len());
                let mut rest = raw.as_str();
                while let Some(pos) = rest.find("crate::protocol::") {
                    let after = &rest[pos + "crate::protocol::".len()..];
                    // Read the item name (identifier chars).
                    let item_end = after
                        .bytes()
                        .position(|b| !(b.is_ascii_alphanumeric() || b == b'_'))
                        .unwrap_or(after.len());
                    let item = &after[..item_end];
                    // Rewrite only if item starts with uppercase (CamelCase type or
                    // UPPER_CASE constant). Lowercase = member protocol submodule.
                    if item.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                        // Replace `crate::protocol::<UPPER>` with
                        // `crate::exec_server_protocol::<UPPER>`. The
                        // exec_server_protocol crate re-exports everything from
                        // its private `protocol` submodule via `pub use protocol::*`,
                        // so we must NOT include `protocol::` in the path (that
                        // would hit E0603: module `protocol` is private).
                        out.push_str(&rest[..pos]);
                        out.push_str("crate::exec_server_protocol::");
                    } else {
                        // Keep `crate::protocol::<lowercase>` as-is (member submodule).
                        out.push_str(&rest[..pos + "crate::protocol::".len()]);
                    }
                    out.push_str(item);
                    rest = &after[item_end..];
                }
                out.push_str(rest);
                if out != raw {
                    fs::write(&path, out)?;
                }
            }
        }
        Ok(())
    }
    process(dir)
}

/// Recursively replace a pattern in all .rs files under dir.
fn fix_pattern_in_tree(dir: &Path, old: &str, new: &str) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            fix_pattern_in_tree(&path, old, new)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            let raw = fs::read_to_string(&path)?;
            if raw.contains(old) {
                let rewritten = raw.replace(old, new);
                fs::write(&path, rewritten)?;
            }
        }
    }
    Ok(())
}

/// Rewrite `use lemurclaw_core::` → `use crate::` in .rs files, but ONLY in
/// `use` statements (not type positions where `crate::` is invalid mid-path).
fn fix_use_lemurclaw_core_in_tree(dir: &Path) -> Result<()> {
    fn process(dir: &Path) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                process(&path)?;
            } else if path.extension().is_some_and(|e| e == "rs") {
                let raw = fs::read_to_string(&path)?;
                // Only rewrite lines starting with `use lemurclaw_core::`
                // (possibly after whitespace or `pub `).
                let new: String = raw
                    .lines()
                    .map(|line| {
                        let trimmed = line.trim_start();
                        if trimmed.starts_with("use lemurclaw_core::") {
                            line.replacen("use lemurclaw_core::", "use crate::", 1)
                        } else {
                            line.to_string()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if new != raw {
                    fs::write(&path, new)?;
                }
            }
        }
        Ok(())
    }
    process(dir)
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Return the byte index of the next char boundary after `start`.
fn next_char_boundary(bytes: &[u8], start: usize) -> usize {
    let mut j = start + 1;
    while j < bytes.len() && (bytes[j] & 0xC0) == 0x80 {
        j += 1;
    }
    j
}

/// Rewrite every downstream (non-utils) crate in `lemurclaw-rs/`: `.rs` files get
/// `lemurclaw_utils::<sub>` path conversion, and each crate's `Cargo.toml`
/// collapses its `lemurclaw-utils-* = { workspace = true }` dep lines into a
/// single `lemurclaw-utils = { workspace = true }`.
///
/// Crates can be nested (e.g. `ext/items`, `memories/write`, `utils/cargo-bin`),
/// so this walks the whole tree, skipping the merged `utils/utils` crate
/// (handled by the intra-crate pass) and `target/`.
fn rewrite_downstream(publish_root: &Path, merged_dir: &Path, cluster: &Cluster) -> Result<()> {
    let mut rs_files = 0usize;
    let mut toml_files = 0usize;
    rewrite_downstream_dir(
        publish_root,
        merged_dir,
        cluster,
        &mut rs_files,
        &mut toml_files,
    )?;
    let _ = merged_dir;
    println!(
        "Rewrote downstream: {} .rs files, {} Cargo.toml dep-collapsed.",
        rs_files, toml_files
    );
    Ok(())
}

/// Recursive worker for `rewrite_downstream`.
fn rewrite_downstream_dir(
    dir: &Path,
    merged_dir: &Path,
    cluster: &Cluster,
    rs_files: &mut usize,
    toml_files: &mut usize,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        // Skip build artifacts and the merged crate (handled by intra-crate pass).
        if name == "target" || path == *merged_dir {
            continue;
        }
        if path.is_dir() {
            rewrite_downstream_dir(&path, merged_dir, cluster, rs_files, toml_files)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            let raw =
                fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
            let rewritten = rewrite_rs(&raw, RewriteScope::Downstream, cluster);
            if rewritten != raw {
                fs::write(&path, &rewritten)
                    .with_context(|| format!("write {}", path.display()))?;
                *rs_files += 1;
            }
        } else if name == "Cargo.toml" {
            // The workspace root (lemurclaw-rs/Cargo.toml) is already handled by
            // rewrite_publish_manifest; collapse is a no-op there. For all
            // other crate manifests, collapse member dep lines.
            collapse_deps_in_manifest(&path, cluster)?;
            *toml_files += 1;
        }
    }
    Ok(())
}

/// Collapse the cluster's member dep lines in a crate manifest into a single
/// `<merged_package> = { workspace = true }`. Standalone crates are preserved.
fn collapse_deps_in_manifest(path: &Path, cluster: &Cluster) -> Result<()> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let merge_pkgs: std::collections::HashSet<&str> =
        cluster.members.iter().map(|sc| sc.package).collect();
    let collapsed_line = format!("{} = {{ workspace = true }}", cluster.merged_package);
    let mut out = String::with_capacity(raw.len());
    let mut emitted = false;
    for line in raw.lines() {
        let trimmed = line.trim_start();
        if let Some(pkg) = extract_dep_key(trimmed) {
            // Detect member deps two ways:
            //  (1) dep key matches a member package name
            //  (2) the line has `package = "<member>"` (path-style alias, e.g.
            //      codex_windows_sandbox = { package = "lemurclaw-windows-sandbox", path = ... })
            let key_is_member = merge_pkgs.contains(pkg.as_str());
            let alias_is_member = trimmed
                .find("package = \"")
                .and_then(|i| {
                    let start = i + "package = \"".len();
                    trimmed[start..]
                        .find('"')
                        .map(|end| &trimmed[start..start + end])
                })
                .is_some_and(|alias| merge_pkgs.contains(alias));
            if key_is_member || alias_is_member {
                // A member package dep — collapse into the merged package.
                if !emitted {
                    let indent = &line[..line.len() - trimmed.len()];
                    out.push_str(&format!("{}{}\n", indent, collapsed_line));
                    emitted = true;
                }
                continue;
            }
            // The merged package's own pre-existing entry counts as the
            // collapsed line — keep the first occurrence, drop duplicates.
            if pkg == cluster.merged_package {
                if !emitted {
                    out.push_str(line);
                    out.push('\n');
                    emitted = true;
                }
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    if out != raw {
        fs::write(path, &out).with_context(|| format!("write {}", path.display()))?;
    }
    Ok(())
}

/// Extract the dep key (the part before ` = `) from a `key = value` manifest
/// line. Returns None if the line isn't a dep assignment.
fn extract_dep_key(trimmed: &str) -> Option<String> {
    let eq = trimmed.find(" = ")?;
    Some(trimmed[..eq].trim().to_string())
}
