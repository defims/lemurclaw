# Lemurclaw — Architecture & Build Guide

Lemurclaw is a fork of [openai/codex](https://github.com/openai/codex) that adds a
native GUI stack and a crates.io publishing toolkit on top of the Codex agent.
This document covers the three things that distinguish lemurclaw from upstream:

1. **GUI stack** — four new crates that run Codex as a desktop window or browser app.
2. **xtask publishing toolkit** — renames `codex-*` → `lemurclaw-*`, merges 40+ crates
   into 6, rewrites the brand, and strips the V8 runtime for a smaller binary.
3. **Brand rewrite** — a scoped `Codex` → `lemurclaw` rewrite that protects the
   OpenAI cloud integration tokens that must not change.

Everything below lives in the source tree alongside unmodified upstream code. The
original `codex-rs/` tree is never modified by the publishing tooling — all output
lands in `lemurclaw-rs/` (only `target/` is gitignored).

> 简体中文:见 [lemurclaw.zh-CN.md](./lemurclaw.zh-CN.md)。

---

## 1. GUI stack

Four crates, all members of `codex-rs/Cargo.toml`:

| Crate | Role |
|-------|------|
| `lemurclaw` | The launcher / binary. Parses `--frontend {tui\|gui\|webui}` and dispatches. |
| `lemurclaw-gui` | Native desktop window via `wry` + `tao`, embedding the shared React app. |
| `lemurclaw-webui` | Browser UI via `axum` HTTP + WebSocket, embedding the same React app. |
| `lemurclaw-transport` | Shared transport types (no heavy `wry`/`tao` dependency). |

Dependency direction: `lemurclaw` → `lemurclaw-gui` → `lemurclaw-webui` (the leaf).
`lemurclaw-transport` is shared by all three.

### Launcher (`lemurclaw/src/`)

`main.rs` parses `Cli` (`config.rs`) and calls `run(config)` (`lib.rs:45`). The
`Frontend` enum selects the frontend:

- `Tui` (default) → strips lemurclaw-only flags and passes argv straight to
  `codex_tui::run_main`, so every Codex flag still works.
- `Gui` → `lemurclaw_gui::run_gui()`.
- `Webui` → rejects non-loopback binds (the WS bridge has no auth), then
  `lemurclaw_webui::run_webui(host, port)`.

CLI flags owned by lemurclaw (not forwarded to the TUI):
`--frontend`, `--agent-name`, `--cwd/-C`, `--model/-m`, `--yolo`, `--host`, `--port`.

### `lemurclaw-gui` (wry + tao)

`run_gui()` (`lemurclaw-gui/src/lib.rs:60`):

1. Builds a tao `EventLoop` on the **main thread** (macOS/Windows require the
   window on the main thread).
2. Spawns a separate OS thread for the tokio runtime + `InProcessAppServerClient`.
3. Creates a wry `WebView` with:
   - A custom `lemurclaw://` protocol that serves the embedded React `dist/`.
   - An initialization script that injects `window.__lemurclaw = { onEvent, onResponse }`.
   - An IPC handler that turns JS `postMessage` into backend requests.
4. The event loop pushes backend JSON events back into JS via
   `evaluate_script("window.__lemurclaw.onEvent(...")`.

Thread topology: main thread owns the WebView; a worker thread owns the tokio
runtime. Main → runtime uses `Handle::spawn`; runtime → main uses
`EventLoopProxy::send_event`.

### `lemurclaw-webui` (axum + WebSocket)

`run_webui(host, port)` (`lemurclaw-webui/src/lib.rs:33`) starts a multi-thread
tokio runtime with a **16 MB stack** (Codex-core has deep recursion that crashes
the default 2 MB stack) and blocks on `serve()` (`server.rs:68`):

- `GET /ws` — WebSocket bridge to the in-process AppServer.
- `GET /`, `GET /{*path}` — embedded static React assets.
- `GET /readyz`, `GET /healthz` — liveness probes.

Single-connection model: `InProcessAppServerClient` sits behind a `Mutex`. The
first WS session receives the `next_event()` event stream; concurrent sessions
can issue requests but won't receive push events.

### Shared React frontend (`lemurclaw-webui/assets/`)

React 18 + Vite 5 + TypeScript 5.6 + vitest. `vite.config.ts` sets `base: './'`
because the app loads under the synthetic `lemurclaw://app/` origin (absolute
paths would break). Entry: `src/main.tsx` → `app/App.tsx`. Key components:
`Composer`, `ApprovalCard`, `Scrollback`, `Sidebar`, `ModelPicker`,
`DiffViewerModal`, plus `cells/`, `composer/`, `settings/`, `sidebar/` subtrees.

### Frontend ↔ backend transport (`assets/src/transport.ts`)

The single transport layer. It auto-detects the host with `hasBridge()`:

```typescript
export function hasBridge(): boolean {
  return typeof window.ipc !== 'undefined';
}
```

- **GUI (wry) mode**: Rust injects `window.ipc.postMessage`; `send()` calls it.
  Rust calls back via `window.__lemurclaw.onEvent` (push events) and
  `.onResponse` (JSON-RPC responses).
- **Browser (webui) mode**: no `window.ipc`; on import it opens a WebSocket to
  `ws://<host>:<port>/ws`. Frames are queued until the socket opens.

Inbound dispatch (`handleInbound`): JSON-RPC envelopes (`{jsonrpc:"2.0", id}`) are
matched to pending `sendRequest` Promises by `id`; everything else is forwarded to
`onEventCb`. `sendRequest` assigns local ids starting at 1000 with a 30 s timeout.

### In-process AppServer (`lemurclaw-webui/src/codex_glue.rs`)

Both GUI and webui embed the Codex AppServer in-process via
`InProcessAppServerClient`, not over a socket:

- `build_dev_client()` starts it with `EnvironmentManager::default_for_tests()`,
  `SessionSource::Cli`, `experimental_api: true`, `client_name: "lemurclaw"`.
- `serialize_server_event` / `parse_client_request` / `build_response_envelope`
  form the wire contract shared by both backends, so GUI and webui speak the
  same JSON-RPC dialect.

---

## 2. xtask publishing toolkit

`xtask/` is a standalone crate (not a member of `codex-rs/`). It never edits the
source tree — all output goes to `lemurclaw-rs/`. Commands:

| Command | Phase | What it does |
|---------|-------|--------------|
| `verify size` | 0 | Reports compressed `.crate` size for the largest crates vs the 10 MB limit. |
| `verify patches` | 0 | Probes whether the `[patch.crates-io]` forks are actually required. |
| `publish rename` | 1 | Copies `codex-rs/` → `lemurclaw-rs/`, renames `codex-*` → `lemurclaw-*` in manifests and `use` statements, drops `[[bin]]` from lib+bin crates (kept as libs), drops `[patch.crates-io]`. Runs `cargo check --workspace` to verify. |
| `publish fork {clone,prepare,publish,rewire}` | 1.5 | Publishes the 8 git forks as `lemurclaw-*` (preserving `[lib].name` so `use ratatui::` is unchanged) and rewires the workspace to reference them. |
| `publish bundle --target <X>` | 2-4 | Merges a cluster of crates into one mega-crate (each former crate becomes `pub mod`). Targets: `Utils`, `Core` (84-crate closure), `Extensions`, `Server`, `Tui`, `Cli`. |
| `publish rebrand-full` | — | Full-scope `Codex` → `lemurclaw` rewrite (see §3). Can be re-run on an existing `lemurclaw-rs/` tree. |
| `publish strip-v8` | — | Replaces `code-mode` with a V8-free stub, removes `code-mode-host` + `v8-poc`, drops `v8`/`deno_core_icudata` deps, regenerates the lockfile. Saves ~120 MB. |
| `publish restore-v8` | — | Undoes `strip-v8`. |

### Standard pipeline

```bash
cd xtask
cargo run -q -- publish rename            # Phase 1
cargo run -q -- publish bundle --target core        # Phase 2/3
cargo run -q -- publish bundle --target extensions  # Phase 4
cargo run -q -- publish bundle --target server
cargo run -q -- publish bundle --target tui
cargo run -q -- publish bundle --target cli
cargo run -q -- publish fork rewire      # Phase 1.5
cd ../lemurclaw-rs && cargo check --workspace --offline
```

Optional V8 removal (run after `rename`, before/after `bundle`):

```bash
cargo run -q -- publish strip-v8
```

Timing: `rename` ~2 min, `bundle --target core` ~2 min, first
`cargo check --workspace --offline` 20-40 min (mega-crate compile), <1 min after.

### Final structure (after full bundle)

| Crate | Contents |
|-------|----------|
| `lemurclaw-core` | 84+ crates merged (core, protocol, utils, …) |
| `lemurclaw-extensions` | 9 extension crates (agent, connectors, goal, guardian, image-gen, mcp, memories, skills, web-search) |
| `lemurclaw-server` | 18 server-layer crates (app-server, transport, daemon, client, arg0, chatgpt, …) |
| `lemurclaw-tui` | tui host + ansi-escape + message-history |
| `lemurclaw` | cli host + cloud-tasks, exec, mcp-server, … (lib + bin, same name) |
| `lemurclaw-experimental-api-macros` | proc-macro (standalone) |

Plus 8 fork crates published to crates.io: `lemurclaw-{crossterm, ratatui,
tungstenite, tokio-tungstenite, nucleo, runfiles, ansi-to-tui, ratatui-macros}`.

### Running the GUI from `lemurclaw-rs/` (recommended)

This is the recommended way to run lemurclaw. `rename` + `strip-v8` produces a
self-contained `lemurclaw-rs/` workspace with the GUI crates as first-class members
(the launcher keeps its `[[bin]]`) and no V8 dependency:

```bash
cd xtask
cargo run -q -- publish rename     # codex-rs/ → lemurclaw-rs/, renames crates
cargo run -q -- publish strip-v8   # replace code-mode with a V8-free stub
cd ../lemurclaw-rs
cargo build -p lemurclaw
./target/debug/lemurclaw --frontend webui --port 8080   # browser
./target/debug/lemurclaw --frontend gui                 # native window
```

Why `lemurclaw-rs/` over `codex-rs/`: the GUI crates are members of `codex-rs/` for
development convenience, but building the launcher there pulls in the full V8
runtime (~120 MB download). `lemurclaw-rs/` is the self-contained lemurclaw workspace;
rename + strip-v8 yields a binary that runs the TUI/GUI/WebUI frontends without V8.

A `lemurclaw-rs/`-built `lemurclaw` after `strip-v8` runs the TUI/GUI/WebUI frontends
for normal conversation; it only lacks V8 code execution (the stub returns an
error if something actually requests a code-mode cell).

---

## 3. Brand rewrite

`rewrite_brand_full` (`xtask/src/bundle.rs:3948`) rewrites `Codex`/`codex` →
`lemurclaw` across the `lemurclaw-rs/` tree. It splits the work into an **A-zone**
(rewritten) and a **B-zone** (protected, left untouched).

### A-zone — rewritten

| Category | Examples |
|----------|----------|
| Env var **values** (quoted string literals) | `"CODEX_HOME"` → `"LEMURCLAW_HOME"`, `"CODEX_API_KEY"` → `"LEMURCLAW_API_KEY"`, 40+ entries. The constant *identifiers* (`CODEX_HOME_ENV_VAR`) are rewritten by the `source_rewrite` AST pass. |
| Filesystem paths | `~/.codex` → `~/.lemurclaw`, `/etc/codex` → `/etc/lemurclaw`, `(".codex")` → `(".lemurclaw")` |
| CLI flags | `--codex-*` → `--lemurclaw-*` (both emitter and matcher, for self-re-exec) |
| Protocol / binary identifiers | `codex://` → `lemurclaw://`, protobuf packages `codex.thread_config.v1` → `lemurclaw.…`, daemon client name, arg0 helper binary names |
| Emit-only telemetry / OTEL | `codex.thread.*`, `codex.windows_sandbox.*`, OTEL `service.name` |
| Type names (non-on-wire) | `CodexErrorInfo` → `LemurclawErrorInfo` |
| Prose brand words | `\bCodex\b` / `\bcodex\b` → `lemurclaw` in non-`.rs` files (prompts, tooltips, README, models.json) |

`source_rewrite.rs` does the AST-driven Rust identifier rewrite (`use codex_foo`
→ `use lemurclaw_foo`) using `syn`, with a surgical line-level fallback. Display
text (CLI `--help`, TUI screen text, errors, banners) is handled by
`rewrite_brand_display_text` using exact full-string-literal pairs.

### B-zone — protected (must not change)

Lemurclaw talks directly to OpenAI's cloud (`chatgpt.com/codex-backend`). These
tokens are stashed behind placeholders before editing and restored after, so a
loose `\bCodex\b` match can never touch them:

- **Model slugs** (real API model names): `gpt-5.x-codex`, `gpt-5.x-codex-max`, …
- **Originator header values**: `codex_cli_rs`, `codex-tui`, `codex_vscode`,
  `codex_atlas`, `codex_chatgpt_desktop`, …
- **JWT audience**: `codex-app-server` (the cloud validates the `aud` claim)
- **`codex_exec` originator** (outbound header)
- **Analytics `event_type` wire values**: `codex_app_mentioned`, `codex_turn_event`, …
- **On-wire JSON field name**: `codexErrorInfo`
- **Cloud URLs / infrastructure**: `chatgpt.com/codex`, `com.openai.codex`,
  `codex-backend`, …

The full replacement table and scope rules live in `xtask-brand-rewrite-spec.md`.

---

## 4. V8 / code-mode

`code-mode` is the only crate that links `librusty_v8` (~120 MB). It provides an
in-process JavaScript execution sandbox.

`publish strip-v8` replaces `code-mode` with a V8-free stub that re-exports the
same protocol types and provider type signatures (`InProcessCodeModeSessionProvider`,
`ProcessOwnedCodeModeSessionProvider`, `WebSocketCodeModeSessionProvider`) so
**downstream code compiles unchanged** — no `#[cfg]` gates, no Cargo features.
The providers' `create_session` returns an error at runtime; upstream holds them
behind an `Arc<dyn CodeModeSessionProvider>` trait boundary, so this is a clean
seam.

**The GUI/TUI/WebUI conversation path never touches V8.** `code-mode` is a
feature-gated, default-off capability (`Feature::CodeMode: default_enabled =
false`). The launcher and GUI backends talk to the AppServer over plain JSON-RPC
and never import `code-mode` directly. V8 initializes lazily (`OnceCell`) only
when a code-mode cell is actually executed. So a `strip-v8` build fully supports
normal chat, approval, diff, and thread operations — it only loses the optional
code execution sandbox.

---

## 5. Repository layout (lemurclaw additions)

```
.
├── codex-rs/
│   ├── lemurclaw/            # launcher (lib + bin)
│   ├── lemurclaw-gui/        # wry + tao desktop window
│   ├── lemurclaw-webui/      # axum + WebSocket browser UI
│   │   └── assets/           # shared React/Vite/TS frontend
│   ├── lemurclaw-transport/  # shared transport types
│   └── …                     # upstream codex-* crates, unmodified
├── xtask/                    # publishing toolkit (standalone crate)
│   └── src/{main,rename,bundle,manifest,source_rewrite,strip_v8,forks,verify}.rs
├── lemurclaw-rs/              # xtask output (only target/ gitignored)
├── lemurclaw-rs.forks/        # fork clones for publishing (gitignored)
├── docs/lemurclaw.md         # this document
├── xtask-brand-rewrite-spec.md
└── .agents/skills/lemurclaw-upstream-sync/SKILL.md
```

`.gitignore` additions: `/lemurclaw-rs/target/`, `/lemurclaw-rs.forks/`, `/.zcode/`.

---

## 6. Troubleshooting

See `.agents/skills/lemurclaw-upstream-sync/SKILL.md` for the full known-issues
table (workspace duplicates after bundle, fork rewire aliases, post-merge fixup
errors, the binary-target `main.rs` recovery, nested `Cargo.toml` cleanup, and
the crates.io publish order). That skill is the authoritative runbook for
re-running the pipeline after an upstream sync.
