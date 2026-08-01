---
name: lemurclaw-upstream-sync
description: Sync lemurclaw fork with upstream codex-rs updates. Use when the user says they updated/merged upstream, wants to re-run the publish pipeline, needs to publish to crates.io, or mentions lemurclaw Phase 1-5, bundle, rename, fork publish, or upstream sync. Also use when compilation errors appear after re-running the pipeline that need post-merge fixups.
---

# Lemurclaw Upstream Sync

This skill handles the full pipeline for syncing the lemurclaw fork (a codex-rs fork that renames `codex-*` to `lemurclaw-*` and merges 40+ crates into 6) with upstream codex-rs changes.

## When to use

- User merged upstream codex-rs changes and needs to re-run the pipeline
- User wants to publish lemurclaw crates to crates.io
- Compilation errors after pipeline run (post-merge fixups needed)
- User mentions Phase 1-5, bundle, rename, fork, or upstream sync

## Architecture Overview

The lemurclaw project is a codex-rs fork that:
1. Renames all `codex-*` crates to `lemurclaw-*` (Phase 1)
2. Publishes 4 git fork crates (crossterm/ratatui/tungstenite/tokio-tungstenite) + nucleo + runfiles + ansi-to-tui + ratatui-macros (Phase 1.5)
3. Merges 40+ standalone crates into 6 mega-crates (Phase 2-4)
4. Publishes everything to crates.io (Phase 5)

### Final crate structure (6 crates)

| Crate | Contents | Depends on |
|-------|----------|------------|
| `lemurclaw-core` | 84+ crates merged (core + protocol + utils + ...) | experimental-api-macros |
| `lemurclaw-extensions` | 9 ext crates (agent, connectors, goal, guardian, image-gen, mcp, memories, skills, web-search) | core |
| `lemurclaw-server` | 18 server-layer crates (app-server, transport, daemon, client, arg0, chatgpt, home, uds, linux-sandbox, ...) | core, extensions |
| `lemurclaw-tui` | 3 crates (tui host + ansi-escape + message-history) | core, server |
| `lemurclaw` | 9 crates (cli host + cloud-tasks, exec, mcp-server, ...) - **lib + bin same name** | core, server, tui |
| `lemurclaw-experimental-api-macros` | proc-macro (standalone) | none |

### Fork crates published to crates.io

| Fork crate | Original | Version |
|------------|----------|---------|
| lemurclaw-crossterm | crossterm (nornagon fork) | 0.28.1 |
| lemurclaw-ratatui | ratatui (nornagon fork) | 0.29.0 |
| lemurclaw-tungstenite | tungstenite (openai-oss-forks) | 0.27.0 |
| lemurclaw-tokio-tungstenite | tokio-tungstenite (openai-oss-forks) | 0.28.0 |
| lemurclaw-nucleo | nucleo (helix-editor) | 0.5.0 |
| lemurclaw-runfiles | runfiles (dzbarsky/rules_rust) | 0.1.0 |
| lemurclaw-ansi-to-tui | ansi-to-tui | 7.0.0 |
| lemurclaw-ratatui-macros | ratatui-macros | 0.6.1 |

## Pipeline execution order

**Always run in this exact order.** Each step depends on the previous.

```bash
cd xtask

# Phase 1: rename codex-* → lemurclaw-*
cargo run -q -- publish rename

# Phase 2/3: merge 84+ crates into lemurclaw-core
cargo run -q -- publish bundle --target core

# Phase 4: merge remaining clusters
cargo run -q -- publish bundle --target extensions
cargo run -q -- publish bundle --target server
cargo run -q -- publish bundle --target tui
cargo run -q -- publish bundle --target cli

# Phase 1.5: rewire to use published forks (removes [patch] sections, adds package aliases)
cargo run -q -- publish fork rewire

# Validate
cd ../lemurclaw-rs && cargo check --workspace --offline
```

### Timing

- `publish rename`: ~2 min (plus first compile of xtask ~30 sec)
- `bundle --target core`: ~2 min (82 member migrations + rewrites)
- Each Phase 4 bundle: ~30 sec
- `cargo check --workspace --offline`: **20-40 min first time** (mega-crate compilation), <1 min incremental
- v8 compile/download can be very slow or fail if no network

## Known issues and manual fixes

### 1. Workspace Cargo.toml duplicates

After running bundles, `lemurclaw-rs/Cargo.toml` may have duplicate member entries (e.g. `"cli"` listed twice). **Always check and fix:**

```toml
# CORRECT - no duplicates, no v8-poc
members = [
    "core",
    "server",
    "tui",
    "cli",
    "ext/extensions",
    "codex-experimental-api-macros",
]
```

Also remove `lemurclaw-v8-poc` from `[workspace.dependencies]` and delete `lemurclaw-rs/v8-poc/` directory.

### 2. Fork rewire package aliases

After `fork rewire`, workspace deps get `package = "lemurclaw-X"` aliases. These work for local dev with `[patch.crates-io]` but **break cargo publish** (patch removed, aliases point to crates.io forks). For local dev, keep both patches AND aliases. For publishing, see Phase 5 below.

### 3. Post-merge fixup errors

The xtask has automated fixups in `post_merge_fixups_{core,extensions,server,tui,cli}()`. If new errors appear after upstream sync:

**Core fixups** (in `xtask/src/bundle.rs`):
- `add_core_root_reexports()` - 49 `pub use core_internal::*` in lib.rs
- `add_core_member_reexports()` - re-exports in config/sandboxing/skills/connectors/windows_sandbox modules
- `fix_otel_dangling_assignments()` - cleanup otel code broken by reqwest 0.13 upgrade
- `fix_reqwest_tls_built_in_root_certs()` - `tls_built_in_root_certs` → `tls_certs_only` (both async AND blocking)

**TUI fixups**:
- Fix `tui_internal/frames.rs` include_str! paths (`../frames/` -> `frames/`)
- Add missing `arboard` and `libc` deps to `tui/Cargo.toml`
- Add lib.rs re-exports (ComposerInput, ComposerAction, AppExitInfo, Cli, etc.)

**Brand display-text rewrite** (`rewrite_brand_display_text()` - runs in cli/tui/server fixups):
- Rewrites user-visible display text `Codex`/`codex` -> `lemurclaw` (CLI `--help`, TUI screen
  text, error messages, doctor output, login hints, banners, tooltips.txt).
- Uses exact full-string-literal `str::replace` pairs (NOT a generic token walk) to avoid
  corrupting env vars (`CODEX_*`), paths (`~/.codex`), flag names (`--codex-home`), identifiers
  (`codex_home`, `CodexAuth`), telemetry names (`codex.thread.*`), model slugs (`gpt-5.x-codex`),
  and OpenAI infrastructure URLs (`com.openai.codex`, `github.com/openai/codex`).
- Command examples inside display text ARE rewritten (`codex login` -> `lemurclaw login`) since
  `bin_name` is already `lemurclaw`; env-var names in the same string are preserved.
- Full replacement table + scope rules: see `xtask-brand-rewrite-spec.md` at repo root.
- If upstream adds new "Codex" display strings, add new pairs to the table in that function.

**If new fixups are needed**: add them to the corresponding `post_merge_fixups_*()` function in `xtask/src/bundle.rs`, then manually apply to current `lemurclaw-rs/` tree.

### 4. Binary target (main.rs)

The `cli` crate has both lib AND bin named `lemurclaw`. After bundle, `main.rs` is lost (migrated into `cli_internal/`). Must manually:

1. Copy `codex-rs/cli/src/main.rs` → `lemurclaw-rs/cli/src/main.rs`
2. Apply rename: `codex_*` → `lemurclaw_*`, `codex-` → `lemurclaw-`
3. Fix binary crate refs: `crate::cli_internal::` → `lemurclaw::cli_internal::` (bin references lib)
4. Fix member crate refs: `lemurclaw_exec::` → `crate::exec::` (or `lemurclaw_server::arg0::` for server members)
5. Add `#[path]` declarations for binary-only modules (app_cmd, doctor, mcp_cmd, etc.)
6. Fix over-renamed identifiers: `lemurclaw_home` → `codex_home`, `lemurclaw_linux_sandbox_exe` → `codex_linux_sandbox_exe`
7. Add `[[bin]] name = "lemurclaw"` to `cli/Cargo.toml`
8. Remove `cli_internal/Cargo.toml` (residual from migration - prevents packaging)

### 5. Nested Cargo.toml files block packaging

After bundle, each member module may have a residual `Cargo.toml` (e.g. `core/src/core_internal/Cargo.toml`). These cause `cargo package` to exclude the directory. **Delete all nested Cargo.toml files** before publishing:

```bash
find lemurclaw-rs/ -name Cargo.toml -not -path "*/target/*" | while read f; do
  # Keep only crate-root Cargo.toml files (depth 2 from lemurclaw-rs/)
  depth=$(echo "$f" | tr -cd '/' | wc -c)
  if [ "$depth" -gt 3 ]; then
    rm "$f"
  fi
done
```

## Phase 5: Publishing to crates.io

### Prerequisites
- `~/.cargo/credentials.toml` with valid token
- All fork crates already published (see Fork crates table above)

### Publish order (strict dependency order)

```bash
cd lemurclaw-rs

# 1. proc-macro (no deps)
cd codex-experimental-api-macros && cargo publish --allow-dirty && cd ..

# 2. core (depends on macros)
cd core && cargo publish --allow-dirty --no-verify && cd ..

# 3. extensions (depends on core)
cd ext/extensions && cargo publish --allow-dirty --no-verify && cd ../..

# 4. server (depends on core, extensions)
cd server && cargo publish --allow-dirty --no-verify && cd ..

# 5. tui (depends on core, server)
cd tui && cargo publish --allow-dirty --no-verify && cd ..

# 6. lemurclaw (depends on core, server, tui)
cd cli && cargo publish --allow-dirty --no-verify && cd ..
```

### Key publish flags
- `--allow-dirty`: forks have uncommitted Cargo.toml changes
- `--no-verify`: skips recompilation (`.DS_Store` build artifacts cause failures, and mega-crate recompile takes 20+ min)

### Version requirements

All workspace deps need explicit `version` for crates.io:
```toml
lemurclaw-core = { path = "core", version = "0.0.1" }
lemurclaw-extensions = { path = "ext/extensions", version = "0.0.1" }
# ... etc
```

### Fork deps with package aliases

Third-party crates (ansi-to-tui, ratatui-macros) depend on standard `ratatui`. Use `package = "lemurclaw-X"` aliases in workspace deps:
```toml
ratatui = { version = "0.29.0", package = "lemurclaw-ratatui" }
ansi-to-tui = { version = "7.0.0", package = "lemurclaw-ansi-to-tui" }
ratatui-macros = { version = "0.6.1", package = "lemurclaw-ratatui-macros" }
```

### Publishing new fork crates

If upstream adds a new git dependency that's API-incompatible with crates.io:

1. Clone the git repo to `lemurclaw-rs.forks/<name>/`
2. Rename package: `name = "lemurclaw-<name>"`
3. If it depends on ratatui/crossterm/etc., change those deps to use `package = "lemurclaw-X"` aliases
4. `git init && git add -A && git commit -m "init"`
5. `cargo publish --allow-dirty`
6. Update workspace Cargo.toml with the alias

## Quick troubleshooting

| Error | Fix |
|-------|-----|
| `file not found for module core_internal` | Delete `core/src/core_internal/Cargo.toml` |
| `no matching package named lemurclaw-X` | Fork not published yet, or run `fork rewire` |
| `tls_built_in_root_certs not found` | reqwest 0.13 API change, see `fix_reqwest_tls_built_in_root_certs()` |
| `unreachable_patterns` lint error | Remove `_ => unreachable!()` catch-all OR add `_ => tools` fallback |
| `unused_mut` on exporter_builder | Remove `mut` (with_http_client was commented out) |
| `cannot find module or crate lemurclaw_cli` | Bin references lib via `lemurclaw::` not `crate::` |
| unicode-width version conflict | Third-party crate uses standard ratatui, need fork alias |
| `.DS_Store` in build output | Use `--no-verify` flag |
| `dependency X does not specify a version` | Add `version = "0.0.1"` to workspace path deps |

## Key files

- `xtask/src/bundle.rs` (~3100 LoC) - all merge logic + post-merge fixups
- `xtask/src/forks.rs` - Phase 1.5 fork clone/prepare/publish/rewire
- `xtask/src/rename.rs` - Phase 1 rename
- `xtask/src/main.rs` - CLI dispatch
- `lemurclaw-rs/Cargo.toml` - workspace manifest (manually maintained after bundles)
- `lemurclaw-rs/core/src/lib.rs` - crate-root re-exports (auto-generated by fixups)
