# Task: Add post-merge brand-text rewriter to xtask (Codex → lemurclaw display text)

> Self-contained spec for a new session. Read this whole file before coding.
> Context: a prior session manually renamed user-facing display text in
> `publish/` (a generated, gitignored tree). Now automate it in `xtask/` so
> every `xtask publish bundle` run re-applies the renames automatically.

## Goal

Add a new post-merge fixup to `xtask/src/bundle.rs` that rewrites
**user-visible display text** in the merged source tree: `Codex`/`codex` →
`lemurclaw`. This must run as part of `post_merge_fixups()` for the `cli`,
`tui`, and `server` clusters (display text lives in all three).

## Background: how post_merge_fixups works (already exists, read this)

- `xtask/src/bundle.rs:995` calls `post_merge_fixups(&merged_dir.join("src"), cluster)?`.
  → The fixup receives `src_dir` = the merged crate's `src/` directory. All
  display-text files live under `src/`, so scope matches.
- `post_merge_fixups(src_dir, cluster)` at `bundle.rs:2236` dispatches by
  `cluster.name` → `post_merge_fixups_{core,extensions,server,tui,cli}`.
- Existing pattern (see `fix_otel_dangling_assignments` at `bundle.rs:2566`):
  `fs::read_to_string` → `str::replace(old, new)` → compare → `fs::write`.
- `fix_pattern_in_tree(dir, old, new)` at `bundle.rs:3180` walks a tree and
  replaces in `.rs` files. **Do NOT reuse it blindly** — see "Safety" below.
- Imports already present: `use std::fs;`, `use std::path::{Path, PathBuf};`,
  `use anyhow::{Context, Result};` (line 20–23).
- xtask currently compiles (`cargo check` in `xtask/` → only a dead-code
  warning, no errors).

## CRITICAL: Scope rules (do NOT deviate)

This rewriter touches **only user-facing display text**. Many `codex`/`Codex`
occurrences MUST be preserved. Apply replacements as **exact full-string-literal
`str::replace`** (not regex, not blind token replacement). The exact pairs are
listed in "The replacement table" below.

### MUST PRESERVE (never rewrite these) — they appear inside display strings too
- Environment variables: `CODEX_HOME`, `CODEX_API_KEY`, `CODEX_ACCESS_TOKEN`,
  `CODEX_MANAGED_PACKAGE_ROOT`, `CODEX_MANAGED_BY_NPM`, and all other
  `CODEX_*` identifiers.
- Paths: `~/.codex`, `~/.codex/config.toml`, `/etc/codex/`, `.codex/`, `CODEX_HOME`.
- CLI flag names: `--codex-home`, `--codex-bin`, `--run-as-windows-sandbox`,
  `--codex-run-as-fs-helper`, `--codex-run-as-apply-patch`.
- URL scheme: `codex://`.
- arg0 helper binary basenames: `codex-linux-sandbox`, `codex-execve-wrapper`.
- Type/identifier names: `CodexAuth`, `CodexHome`, `CodexHomeUserInstructionsProvider`,
  `CodexStatus`, `CodexFeedback`, `CodexPackageLayout`, `CodexVersion`,
  `codex_home`, `codex_self_exe`, `codex_linux_sandbox_exe`, `codex_exec`,
  `enable_codex_api_key_env`.
- Telemetry/metric names: `codex.thread.fork`, `codex.windows_sandbox.*`,
  `codex.apps.refresh.duration_ms`, `codex_exec` originator strings.
- OpenAI infrastructure: `com.openai.codex`, `Codex.app`, `Codex.dmg`,
  `codex-app-prod`, `github.com/openai/codex`, `@openai/codex`,
  `chatgpt.com/codex`, `community.openai.com/c/codex/37`,
  `developers.openai.com/codex`, `oaistatic.com`.
- Model slugs: `gpt-5.x-codex`, `gpt-5.1-codex`, `gpt-4.1-codex`, `codex-auto-`.
- System prompts (`prompt.md`, `gpt_*_prompt.md`, `base_instructions/*.md`)
  — NOT display text, leave untouched (their "You are Codex" is intentional).
- Pet name `"codex"` / `DEFAULT_PET_ID` / spritesheet filename — not display text.
- Remote-server error-match substrings in `background_requests.rs`:
  `err.contains("codex plugins are disabled")`, `err.contains("update codex")`
  — these match cloud-server output, keep verbatim. (Only the remediation
  *messages* shown to the user are rewritten.)
- AGENTS.md-forbidden sandbox constants (not display text anyway):
  `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR`, `CODEX_SANDBOX_ENV_VAR`,
  `CODEX_SANDBOX=seatbelt`, `CODEX_SANDBOX_NETWORK_DISABLED=1`.

### Why exact-string replace (not regex/tree-walk) is required
A generic `codex ` → `lemurclaw ` walk would corrupt `codex_home`,
`codex_exec`, `codex-linux-sandbox`, `codex://`, etc. The safe approach is a
list of **exact, full-context string literals** where each `old` includes
enough surrounding text to be unambiguous. Apply with `str::replace` per file
(only on the specific files where each string appears).

## Implementation

### 1. Add `rewrite_brand_display_text(src_dir: &Path) -> Result<()>`

Place it near the other `fix_*` helpers (e.g. after `fix_reqwest_tls_built_in_root_certs`
around `bundle.rs:2894`, before `add_reexport_module`). Structure:

```rust
/// Rewrite user-facing display text: Codex/codex → lemurclaw.
///
/// IMPORTANT: only exact full-string-literal replacements from the table
/// below. Do NOT use a generic token walk — it would corrupt env vars
/// (CODEX_*), paths (~/.codex), flag names (--codex-home), identifiers
/// (codex_home, CodexAuth), telemetry names (codex.thread.*), model slugs
/// (gpt-5.x-codex), and OpenAI infrastructure URLs. Each `old` pair is
/// anchored with surrounding context so it only matches display text.
fn rewrite_brand_display_text(src_dir: &Path) -> Result<()> {
    // Each entry: (relative path under src_dir, old, new). Use "" for path
    // to mean "search every .rs and the listed text files in the tree".
    // For safety, target specific files where the string appears.
    let edits: &[(&str, &str, &str)] = &[
        // ... entries from the table below ...
    ];
    for (rel, old, new) in edits {
        let path = src_dir.join(rel);
        if path.is_file() {
            let raw = fs::read_to_string(&path)?;
            if raw.contains(old) {
                fs::write(&path, raw.replace(old, new))?;
            }
        }
    }
    Ok(())
}
```

Design choice rationale (put in a comment): target specific files rather than
walking the tree, because (a) exact-string replace is idempotent and safe only
when the string is unambiguous, and (b) most `codex`/`Codex` in any given file
is an identifier/path that must be preserved — file-scoped replace still risks
false hits, so keep `old` strings long enough to be unambiguous within the file.

### 2. Call it from the three relevant cluster fixups

- `post_merge_fixups_cli` (`bundle.rs:2695`, currently a no-op `Ok(())`):
  call `rewrite_brand_display_text(src_dir)?;` then `Ok(())`.
- `post_merge_fixups_tui` (`bundle.rs:2635`): append the call before `Ok(())`.
- `post_merge_fixups_server` (`bundle.rs:2630`, currently no-op): call it too
  (server has `app_server_daemon/update_loop.rs` + `app_server_test_client`).

Note: `post_merge_fixups_server` currently takes `_src_dir`. Change the param
to `src_dir` (remove the underscore) since it's now used.

### 3. The replacement table

Each row: (relative path under `src/`, old, new). Paths are relative to the
merged crate `src/`. When the same file lives in different clusters, the call
only runs in that cluster's fixup, so the path is cluster-relative.

**CLI cluster** (`cli/src/`):
```
main.rs
  "/// Codex CLI"                                          → "/// lemurclaw CLI"
  'override_usage = "codex [OPTIONS] [PROMPT]\n       codex [OPTIONS] <COMMAND> [ARGS]"'
    → 'override_usage = "lemurclaw [OPTIONS] [PROMPT]\n       lemurclaw [OPTIONS] <COMMAND> [ARGS]"'
  "the generic `codex` command name that users run."       → "the generic `lemurclaw` command name that users run."
  "/// Run Codex non-interactively."                       → "/// Run lemurclaw non-interactively."
  "/// Manage external MCP servers for Codex."             → "/// Manage external MCP servers for lemurclaw."
  "/// Manage Codex plugins."                              → "/// Manage lemurclaw plugins."
  "/// Start Codex as an MCP server (stdio)."              → "/// Start lemurclaw as an MCP server (stdio)."
  "/// Update Codex to the latest version."                → "/// Update lemurclaw to the latest version."
  "/// Diagnose local Codex installation"                  → "/// Diagnose local lemurclaw installation"
  "/// Run commands within a Codex-provided sandbox."      → "/// Run commands within a lemurclaw-provided sandbox."
  "/// Apply the latest diff produced by Codex agent"      → "/// Apply the latest diff produced by lemurclaw agent"
  "/// [EXPERIMENTAL] Browse tasks from Codex Cloud"       → "/// [EXPERIMENTAL] Browse tasks from lemurclaw Cloud"
  "/// [internal] Generate internal JSON Schema artifacts for Codex tooling."
    → "/// [internal] Generate internal JSON Schema artifacts for lemurclaw tooling."
  "this version of Codex."                                 → "this version of lemurclaw."   (replace_all on this one)
  "printenv OPENAI_API_KEY | codex login --with-api-key"   → "printenv OPENAI_API_KEY | lemurclaw login --with-api-key"
  "printenv CODEX_ACCESS_TOKEN | codex login --with-access-token"
    → "printenv CODEX_ACCESS_TOKEN | lemurclaw login --with-access-token"
  "Updating Codex via `{cmd_str}`..."                      → "Updating lemurclaw via `{cmd_str}`..."
  "Please restart Codex."                                  → "Please restart lemurclaw."
  "`codex update` is not available in debug builds. Install a release build of Codex to use this command."
    → "`lemurclaw update` is not available in debug builds. Install a release build of lemurclaw to use this command."
  "Could not detect the Codex installation method."        → "Could not detect the lemurclaw installation method."
  "Codex executable path is not configured"                → "lemurclaw executable path is not configured"
  "run `codex login` or set CODEX_API_KEY"                 → "run `lemurclaw login` or set CODEX_API_KEY"
  "Codex's interactive TUI may not work in this terminal." → "lemurclaw's interactive TUI may not work in this terminal."
  "failed to move damaged Codex local database files"      → "failed to move damaged lemurclaw local database files"
  "`codex sandbox` is not supported on this operating system"
    → "`lemurclaw sandbox` is not supported on this operating system"
  "--profile only applies to runtime commands and `codex mcp`: `codex`, `codex exec`, `codex review`, `codex resume`, `codex archive`, `codex delete`, `codex unarchive`, `codex fork`, `codex mcp`, `codex sandbox`, and `codex debug prompt-input`."
    → (same with every `codex` → `lemurclaw`)
  "not `codex {subcommand}`"                               → "not `lemurclaw {subcommand}`"   (2 occurrences)
  "`--strict-config` is not supported for `codex {subcommand}`"
    → "`--strict-config` is not supported for `lemurclaw {subcommand}`"
exec/cli.rs
  'override_usage = "codex exec [OPTIONS] [PROMPT]\n       codex exec [OPTIONS] <COMMAND> [ARGS]"'
    → 'override_usage = "lemurclaw exec [OPTIONS] [PROMPT]\n       lemurclaw exec [OPTIONS] <COMMAND> [ARGS]"'
  "/// Allow running Codex outside a Git repository."      → "/// Allow running lemurclaw outside a Git repository."
exec/event_processor_with_human_output.rs
  "OpenAI Codex v{VERSION}"                                → "OpenAI lemurclaw v{VERSION}"
  `"codex".style(self.italic).style(self.magenta)`         → `"lemurclaw".style(self.italic).style(self.magenta)`  (2)
exec/mod.rs
  "Error finding codex home: {err}"                        → "Error finding lemurclaw home: {err}"
cli_internal/login.rs
  "Use `codex login --device-auth` instead."               → "Use `lemurclaw login --device-auth` instead."
  "printenv OPENAI_API_KEY | codex login --with-api-key"   → "printenv OPENAI_API_KEY | lemurclaw login --with-api-key"
  "printenv CODEX_ACCESS_TOKEN | codex login --with-access-token"
    → "printenv CODEX_ACCESS_TOKEN | lemurclaw login --with-access-token"
cli_internal/sandbox_setup.rs
  "`codex sandbox setup` currently requires --elevated"    → "`lemurclaw sandbox setup` currently requires --elevated"
  (NOTE: `--codex-home` flag and `--user` are NOT touched.)
cli_internal/mcp_cmd.rs
  'override_usage = "codex mcp add [OPTIONS] <NAME> (--url <URL> | -- <COMMAND>...)"'
    → 'override_usage = "lemurclaw mcp add ..."'   (keep the rest identical)
  "Run `codex mcp login {name}` to login."                 → "Run `lemurclaw mcp login {name}` to login."
  "Try `codex mcp add my-tool -- my-command`."             → "Try `lemurclaw mcp add my-tool -- my-command`."
  "remove: codex mcp remove {}"                            → "remove: lemurclaw mcp remove {}"
cli_internal/marketplace_cmd.rs   (all bin_name + after_help)
  'bin_name = "codex plugin marketplace"'                  → 'bin_name = "lemurclaw plugin marketplace"'
  'bin_name = "codex plugin marketplace add"'              → 'bin_name = "lemurclaw plugin marketplace add"'
  'bin_name = "codex plugin marketplace list"'             → 'bin_name = "lemurclaw plugin marketplace list"'
  'bin_name = "codex plugin marketplace upgrade"'          → 'bin_name = "lemurclaw plugin marketplace upgrade"'
  'bin_name = "codex plugin marketplace remove"'           → 'bin_name = "lemurclaw plugin marketplace remove"'
  In after_help strings, replace every `codex plugin marketplace` → `lemurclaw plugin marketplace`
  (the add/upgrade/remove examples each list multiple `codex plugin marketplace ...` lines).
  "List plugin marketplaces Codex is currently considering" → "List plugin marketplaces lemurclaw is currently considering"
cli_internal/plugin_cmd.rs   (bin_name + after_help)
  'bin_name = "codex plugin"'                              → 'bin_name = "lemurclaw plugin"'
  'bin_name = "codex plugin add"'                          → 'bin_name = "lemurclaw plugin add"'
  'bin_name = "codex plugin list"'                         → 'bin_name = "lemurclaw plugin list"'
  'bin_name = "codex plugin remove"'                       → 'bin_name = "lemurclaw plugin remove"'
  In after_help, every `codex plugin add/list/remove ...` → `lemurclaw plugin ...`
cli_internal/state_db_recovery.rs
  "Codex couldn't start because its local database appears to be damaged." → "lemurclaw couldn't start ..."
  "Moving the damaged local database aside so Codex can rebuild it from saved data." → "...so lemurclaw can rebuild..."
  "Codex rebuilt its local database."                      → "lemurclaw rebuilt its local database."
  "Codex detected a damaged local database, moved it into a backup folder" → "lemurclaw detected a damaged local database, moved it into a backup folder"
  "Run `codex doctor` to check your setup"                 → "Run `lemurclaw doctor` to check your setup"
  "Codex couldn't start because another Codex process is using its local data."
    → "lemurclaw couldn't start because another lemurclaw process is using its local data."
  "Quit any other copies of Codex that may still be running" → "Quit any other copies of lemurclaw that may still be running"
cli_internal/doctor/output.rs
  bold("Codex Doctor", options)                            → bold("lemurclaw Doctor", options)
  "Run codex doctor without --summary for detailed diagnostics." → "Run lemurclaw doctor without --summary for detailed diagnostics."
cli_internal/doctor/background.rs
  "Run codex app-server daemon version for more details."  → "Run lemurclaw app-server daemon version for more details."
cli_internal/doctor/git.rs
  "so Codex can inspect Git metadata."                     → "so lemurclaw can inspect Git metadata."
  "so Codex can inspect repository metadata."              → "so lemurclaw can inspect repository metadata."
  "the bundled Git executable Codex resolves first."       → "the bundled Git executable lemurclaw resolves first."
cli_internal/doctor/runtime.rs
  "repair the bundled Codex package."                      → "repair the bundled lemurclaw package."
cli_internal/doctor/updates.rs
  "Reinstall or update Codex so the JS shim provides CODEX_MANAGED_PACKAGE_ROOT."
    → "Reinstall or update lemurclaw so the JS shim provides CODEX_MANAGED_PACKAGE_ROOT."  (NOTE: keep CODEX_MANAGED_PACKAGE_ROOT)
cli_internal/doctor/mod.rs
  "then rerun codex doctor."                               → "then rerun lemurclaw doctor."
  "failed to load Codex config"                            → "failed to load lemurclaw config"
  "Run codex login again or provide a supported auth env var." → "Run lemurclaw login again or provide a supported auth env var."
  "no Codex credentials were found"                        → "no lemurclaw credentials were found"
  "Run codex login or provide an API key through a supported auth env var." → "Run lemurclaw login or provide an API key through a supported auth env var."
  "Fix auth storage access or run codex login again."      → "Fix auth storage access or run lemurclaw login again."
  "Reinstall or update Codex so the JS shim provides CODEX_MANAGED_PACKAGE_ROOT."
    → "Reinstall or update lemurclaw so the JS shim provides CODEX_MANAGED_PACKAGE_ROOT."  (keep CODEX_MANAGED_PACKAGE_ROOT)
  (NOTE: leave `"PATH codex entries: ..."` and `@openai/codex` npm refs untouched.)
cli_internal/doctor/thread_inventory.rs
  "Start Codex with no state DB present so startup backfill can create it from rollout files."
    → "Start lemurclaw with no state DB present so startup backfill can create it from rollout files."  (2 occurrences: .remedy and .remediation)
cloud_tasks/mod.rs
  "Please run 'codex login' to sign in with ChatGPT, then re-run 'codex cloud'."
    → "Please run 'lemurclaw login' to sign in with ChatGPT, then re-run 'lemurclaw cloud'."  (2)
  "run `codex cloud` to list available environments"       → "run `lemurclaw cloud` to list available environments"
  "run `codex cloud` to pick the desired environment id"   → "run `lemurclaw cloud` to pick the desired environment id"
  format!("codex cloud list --cursor='{cursor}'")          → format!("lemurclaw cloud list --cursor='{cursor}'")
```

**TUI cluster** (`tui/src/`):
```
tui_internal/slash_command.rs
  "create an AGENTS.md file with instructions for Codex"   → "...instructions for lemurclaw"
  "exit Codex"                                             → "exit lemurclaw"
  "use skills to improve how Codex performs specific tasks" → "...how lemurclaw performs specific tasks"
  "choose a communication style for Codex"                 → "...style for lemurclaw"
  "choose what Codex is allowed to do"                     → "choose what lemurclaw is allowed to do"
  "log out of Codex"                                       → "log out of lemurclaw"
tui_internal/history_cell/session.rs
  " - create an AGENTS.md file with instructions for Codex" → "...instructions for lemurclaw"
  " - choose what Codex is allowed to do"                  → "...what lemurclaw is allowed to do"
  Span::from("OpenAI Codex").bold()                        → Span::from("OpenAI lemurclaw").bold()
  format!("OpenAI Codex (v{})", self.version)              → format!("OpenAI lemurclaw (v{})", self.version)
  comment ">_ OpenAI Codex (vX)"                           → ">_ OpenAI lemurclaw (vX)"
tui_internal/status/card.rs
  Span::from("OpenAI Codex").bold()                        → Span::from("OpenAI lemurclaw").bold()
  "API key configured (run codex login to use ChatGPT)"    → "API key configured (run lemurclaw login to use ChatGPT)"
tui_internal/history_cell/approvals.rs   (sentence-fragment spans)
  " codex to run "                                          → " lemurclaw to run "            (3, replace_all)
  " codex network access to "                              → " lemurclaw network access to " (4, replace_all)
  " codex to always run commands that start with "         → " lemurclaw to always run commands that start with "
  " Codex network access to "                              → " lemurclaw network access to " (1)
  " for codex to run "                                     → " for lemurclaw to run "
  " before codex could run "                               → " before lemurclaw could run "
  " before codex could access "                            → " before lemurclaw could access "
  " the request for codex network access to "              → " the request for lemurclaw network access to "
  " for codex to apply "                                   → " for lemurclaw to apply "
  " before codex could apply "                             → " before lemurclaw could apply "
tui_internal/bottom_pane/approval_overlay.rs
  "No, and tell Codex what to do differently"              → "No, and tell lemurclaw what to do differently" (5: 2 prod labels + 3 test strings; replace_all to keep tests passing)
  "✔ You approved codex to run"                            → "✔ You approved lemurclaw to run"  (test; change to keep passing)
tui_internal/bottom_pane/mod.rs, chat_composer.rs, chat_composer/slash_input.rs, chat_composer/history_search.rs, keymap_setup.rs
  "Ask Codex to do anything"                               → "Ask lemurclaw to do anything"   (replace_all across these files + the 52 .snap files — see "Snapshots" below)
tui_internal/model_migration.rs
  "Codex just got an upgrade. Introducing {target_display_name}."
    → "lemurclaw just got an upgrade. Introducing {target_display_name}."
  "Choose how you'd like Codex to proceed."                → "Choose how you'd like lemurclaw to proceed."
  (NOTE: do NOT touch "gpt-5.x-codex" model slugs or "Codex-optimized" test fixture strings? — see decision below.)
tui_internal/bottom_pane/list_selection_view.rs
  "Optimized for Codex. Balance of reasoning quality and coding ability." → "Optimized for lemurclaw. Balance of reasoning quality and coding ability."  (these are in TEST fixtures L2406/2488; see decision)
  "Optimized for Codex. Cheaper, faster, but less capable." → "Optimized for lemurclaw. Cheaper, faster, but less capable."  (test fixtures L2416/2498)
  (NOTE: leave "gpt-5.1-codex" / "gpt-4.1-codex" name: fields untouched.)
tui_internal/bottom_pane/memories_settings_view.rs
  "Choose how Codex uses and creates memories."             → "Choose how lemurclaw uses and creates memories."
  "for the current Codex home."                            → "for the current lemurclaw home."
tui_internal/keymap_setup/debug.rs
  "Tip: Codex can only inspect keys your terminal sends."  → "Tip: lemurclaw can only inspect keys your terminal sends."
  "your terminal is not sending that key to Codex."        → "...that key to lemurclaw."
  "Press any key to see what Codex receives."              → "Press any key to see what lemurclaw receives."
tui_internal/keymap_setup/picker.rs
  "See the key Codex detects and any shortcuts assigned to it." → "See the key lemurclaw detects and any shortcuts assigned to it."
tui_internal/pets/image_protocol.rs
  "Run Codex outside tmux to use pets."                    → "Run lemurclaw outside tmux to use pets."
  "Run Codex outside Zellij to use pets."                  → "Run lemurclaw outside Zellij to use pets."
  "or run Codex outside tmux."                             → "or run lemurclaw outside tmux."
tui_internal/tooltips.rs
  "Run 'codex app' or visit https://chatgpt.com/codex?app-landing-page=true"
    → "Run 'lemurclaw app' or visit https://chatgpt.com/codex?app-landing-page=true"  (keep the URL)
  "*New* Build faster with Codex."                         → "*New* Build faster with lemurclaw."
  "Codex is included in your plan for free"                → "lemurclaw is included in your plan for free"
  line.contains("codex app")                               → line.contains("lemurclaw app")   (the filter, must match the tooltip text above)
tui_internal/tooltips.txt   (loaded via include_str!, user-visible)
  "when Codex asks for confirmation"                       → "when lemurclaw asks for confirmation"
  "ask Codex to use one"                                   → "ask lemurclaw to use one"
  "Run `codex app` to open the Desktop app"                → "Run `lemurclaw app` to open the Desktop app"
  "how Codex communicates"                                 → "how lemurclaw communicates"
  "`codex mcp add openaiDeveloperDocs --url https://developers.openai.com/mcp`"
    → "`lemurclaw mcp add openaiDeveloperDocs --url https://developers.openai.com/mcp`"  (keep URL)
  "Visit the Codex community forum: https://community.openai.com/c/codex/37"
    → "Visit the lemurclaw community forum: https://community.openai.com/c/codex/37"  (keep URL)
  "from Codex using `!`"                                   → "from lemurclaw using `!`"
  "See the Codex keymap documentation"                     → "See the lemurclaw keymap documentation"
  "running `codex resume`"                                 → "running `lemurclaw resume`"
  (NOTE: leave `~/.codex/config.toml`, Discord URL untouched.)
tui_internal/app/config_persistence.rs
  "but Codex could not refresh the effective config: {err}" → "but lemurclaw could not refresh the effective config: {err}"
tui_internal/app/input.rs
  "before starting Codex."                                 → "before starting lemurclaw."
tui_internal/app/background_requests.rs   (remediation messages only — NOT the err.contains() matchers)
  "Ask a workspace admin to enable Codex plugins or plugin sharing." → "...enable lemurclaw plugins or plugin sharing."  (2, replace_all)
  "Update Codex, then try opening the shared plugin again." → "Update lemurclaw, then try opening the shared plugin again."  (2, replace_all)
  "Plugin sharing is disabled for this Codex session."     → "Plugin sharing is disabled for this lemurclaw session."  (2, replace_all)
  KEEP: err.contains("codex plugins are disabled"), err.contains("update codex") — these match remote server output.
tui_internal/app/event_dispatch.rs
  "Codex can now safely edit files and execute commands in your computer" → "lemurclaw can now safely edit files and execute commands in your computer"  (2, replace_all)
tui_internal/external_agent_config_migration_flow.rs
  "Start Codex locally and run /import."                   → "Start lemurclaw locally and run /import."
  "while Codex is connected to the local app-server daemon. Stop the daemon, restart Codex, and run /import."
    → "while lemurclaw is connected to the local app-server daemon. Stop the daemon, restart lemurclaw, and run /import."
tui_internal/external_agent_config_migration/render.rs
  "Codex may add files to your current project folder."    → "lemurclaw may add files to your current project folder."  (2, replace_all)
tui_internal/bottom_pane/hooks_browser_view.rs
  "Right before Codex ends its turn"                       → "Right before lemurclaw ends its turn"
tui_internal/app/thread_goal_actions.rs
  "Run `codex` to start a saved session, or `codex resume` / `/resume` to reopen one."
    → "Run `lemurclaw` to start a saved session, or `lemurclaw resume` / `/resume` to reopen one."
tui_internal/bottom_pane/status_surface_preview.rs
  StatusSurfacePreviewItem::AppName => "codex",            => "lemurclaw",
  (NOTE: leave CodexVersion enum, "gpt-5.2-codex" model slugs untouched.)
```

**Server cluster** (`server/src/`):
```
app_server_daemon/update_loop.rs
  "standalone Codex updater exited with status {status}"   → "standalone lemurclaw updater exited with status {status}"
app_server_test_client/mod.rs
  author = "Codex", about = "Bootstrap Codex app-server"   → author = "lemurclaw", about = "Bootstrap lemurclaw app-server"
  "started codex app-server"                               → "started lemurclaw app-server"
  "[codex app-server exited: {status}]"                    → "[lemurclaw app-server exited: {status}]"  (2, replace_all)
  (NOTE: leave `--codex-bin` flag, `codex_bin` ident, SpawnCodex variant untouched.)
```

### 4. Snapshots (.snap files) — DECISION NEEDED

`"Ask Codex to do anything"` also appears in 52 insta `.snap` files under
`tui/src/.../snapshots/`. The prior session updated them in-place so tests pass.
**Question for the new session's user**: should the xtask rewriter also touch
`.snap` files? Two options:
  (a) Yes — also replace `Ask Codex to do anything` → `Ask lemurclaw to do anything`
      in `**/snapshots/*.snap` under the tui src tree. Risk: `.snap` files are
      test artifacts; regenerating via `cargo insta accept` is the canonical
      path. But the strings are stable, so a direct replace is fine.
  (b) No — leave `.snap` alone; the test suite will fail until `cargo insta
      accept` is run after bundle. Document this in the fixup's println.

Recommend (a) for the placeholder only (single, unambiguous string). Do NOT
touch any other `codex` inside `.snap` files (they contain model slugs,
identifiers, etc. that must stay).

## Decisions the prior session made (apply the same here)

1. **Command-example strings inside display text ARE rewritten** (`codex login`
   → `lemurclaw login`) because `bin_name` is already `lemurclaw` and the
   examples must be copy-pasteable.
2. **Env-var names / paths / flag names inside display text are PRESERVED**
   even when they sit in the same string as a brand word. E.g.
   `"...run `lemurclaw login` or set CODEX_API_KEY"` keeps `CODEX_API_KEY`.
3. **`OpenAI Codex` → `OpenAI lemurclaw`** in banners/titles (keeps the
   "OpenAI" prefix since it's the company).
4. **Test fixture strings that mirror display text** (approval_overlay,
   list_selection_view "Optimized for Codex") ARE rewritten so tests keep
   passing — but only the exact display-text phrases, never model slugs.
5. **`tooltips.rs` filter** `line.contains("codex app")` is updated in lockstep
   with the tooltip text `"Run 'codex app'"` → both become `lemurclaw app`.
6. **Remote-server error matchers** (`err.contains("codex plugins are disabled")`,
   `err.contains("update codex")`) are PRESERVED (they match cloud output).

## Verification

After implementing:
1. `cd xtask && cargo check` — must compile.
2. `cd xtask && cargo run -q -- publish bundle --target tui` then
   `cd ../publish && cargo build --bin lemurclaw` — must compile.
3. `./target/debug/lemurclaw --help | head -3` → shows `lemurclaw CLI` +
   `Usage: lemurclaw`.
4. `./target/debug/lemurclaw exec --help | grep Usage` → `lemurclaw exec`.
5. `./target/debug/lemurclaw login --help` → `OPENAI_API_KEY | lemurclaw login`
   AND `CODEX_ACCESS_TOKEN | lemurclaw login` (env vars preserved, command
   rewritten).
6. Grep audit (no hits expected):
   `grep -rnE '"[^"]*[Cc]odex[^"]*"' publish/cli/src publish/server/src \
     publish/tui/src --include='*.rs' | grep -vE 'CODEX_|codex_home|CodexAuth|...'`
7. Spot-check preserved items: `--codex-home` flag still in sandbox_setup.rs;
   `codex://` still in history_ui.rs; `@openai/codex` still in doctor.

## Files to edit
- `xtask/src/bundle.rs` — add `rewrite_brand_display_text`, wire into
  `post_merge_fixups_{cli,tui,server}` (un-underscore `server`'s `_src_dir`).

## Notes
- `publish/` is gitignored (generated tree). This fixup makes the brand rename
  survive `xtask publish bundle` regeneration.
- If upstream adds NEW display-text strings with "Codex" later, this table
  won't catch them — add new pairs as they surface. Consider printing a warning
  if, after the fixup, any `println!`/`eprintln!`/`bail!`/`Span::from(` line
  still contains a standalone `Codex`/`codex` word (heuristic audit) — optional.
- Keep each `old` string long enough to be unambiguous within its file. When a
  phrase appears multiple times identically in one file, use `replace_all`
  semantics (call `raw.replace(old, new)` which replaces all; or `replacen` for
  count-limited).
