<p align="center"><strong>Codex CLI</strong> is a coding agent from OpenAI that runs locally on your computer.
<p align="center">
  <img src="https://github.com/openai/codex/blob/main/.github/codex-cli-splash.png" alt="Codex CLI splash" width="80%" />
</p>
</br>
If you want Codex in your code editor (VS Code, Cursor, Windsurf), <a href="https://developers.openai.com/codex/ide">install in your IDE.</a>
</br>If you want the desktop app experience, run <code>codex app</code> or visit <a href="https://chatgpt.com/codex?app-landing-page=true">the Codex App page</a>.
</br>If you are looking for the <em>cloud-based agent</em> from OpenAI, <strong>Codex Web</strong>, go to <a href="https://chatgpt.com/codex">chatgpt.com/codex</a>.</p>

---

# Lemurclaw

> Lemurclaw is a fork of openai/codex that adds a **native GUI stack** and a
> **crates.io publishing toolkit** on top of the Codex agent. The original Codex
> TUI, agent core, and cloud integration are unchanged.
>
> 简体中文说明见 [README.zh-CN.md](./README.zh-CN.md)。

## What's different from upstream

- **GUI stack** — run Codex as a native desktop window (`--frontend gui`) or a
  browser app (`--frontend webui`), in addition to the original TUI. The three
  frontends share one React/Vite/TypeScript UI and talk to an in-process
  AppServer over JSON-RPC.
- **xtask publishing toolkit** — rename `codex-*` → `lemurclaw-*`, merge 40+
  crates into 6 mega-crates, rewrite the brand, and optionally strip the V8
  runtime (~120 MB smaller binary).
- The original `codex-rs/` source tree is **never modified** by the tooling; all
  publish output lands in `lemurclaw-rs/` (only `target/` is gitignored).

See [docs/lemurclaw.md](./docs/lemurclaw.md) for the full architecture, build,
and brand-rewrite reference.

## Quickstart — build and run the GUI

The lemurclaw binary is built from a generated `lemurclaw-rs/` workspace, which renames
the upstream `codex-*` crates to `lemurclaw-*` and strips the V8 runtime. This is
the recommended path: it produces a ~120 MB smaller binary that doesn't need to
download `librusty_v8`, and the GUI conversation path never uses V8 anyway
(code-mode is an optional, default-off feature).

Prerequisites: Rust (stable), Node.js (for the frontend build).

```shell
# 1. Generate the lemurclaw-rs/ workspace and strip V8 (run from the repo root)
cd xtask
cargo run -q -- publish rename     # codex-rs/ → lemurclaw-rs/, renames crates
cargo run -q -- publish strip-v8   # replace code-mode with a V8-free stub

# 2. Build and run — from lemurclaw-rs/
cd ../lemurclaw-rs
cargo build -p lemurclaw

# Browser UI (axum + WebSocket) — then open http://127.0.0.1:8080
./target/debug/lemurclaw --frontend webui --port 8080

# Native desktop window (wry + tao)
./target/debug/lemurclaw --frontend gui

# TUI (same as upstream codex)
./target/debug/lemurclaw
```

The `lemurclaw-webui` build script automatically runs `npm install && npm run
build` to produce the embedded React `dist/`. If Node is missing it falls back
to a committed `dist/` with a warning.

> **Why `lemurclaw-rs/` and not `codex-rs/`?** The GUI crates live as members of the
> `codex-rs/` workspace for development convenience, but building the launcher
> there pulls in the full V8 runtime (a ~120 MB download). `lemurclaw-rs/` is the
> self-contained lemurclaw workspace — rename + strip-v8 gives you a binary that
> runs the TUI/GUI/WebUI frontends without V8.

### Frontend development

For React/Vite HMR while developing the shared UI, edit under
`codex-rs/lemurclaw-webui/assets/` (the source the lemurclaw-rs build embeds):

```shell
cd codex-rs/lemurclaw-webui/assets
npm install
npm run dev    # Vite dev server with HMR
npm run test   # vitest
```

## Publishing to crates.io

The full pipeline (rename → bundle into 6 crates → rewire forks → publish) is
documented in [docs/lemurclaw.md](./docs/lemurclaw.md#2-xtask-publishing-toolkit)
and the runbook at
[`.agents/skills/lemurclaw-upstream-sync/SKILL.md`](./.agents/skills/lemurclaw-upstream-sync/SKILL.md).

---

# Codex CLI (upstream)

## Quickstart

### Installing and running Codex CLI

Run the following on Mac or Linux to install Codex CLI:

```shell
curl -fsSL https://chatgpt.com/codex/install.sh | sh
```

Run the following on Windows to install Codex CLI:

```shell
powershell -ExecutionPolicy ByPass -c "irm https://chatgpt.com/codex/install.ps1 | iex"
```

The standalone installers download from `https://releases.openai.com/codex` by default and fall back to GitHub Releases if a metadata or asset download is unavailable. To force GitHub Releases, set `CODEX_INSTALLER_USE_RELEASES_OPENAI_COM` to `false` (`0` and `no` are also accepted):

```shell
curl -fsSL https://chatgpt.com/codex/install.sh | CODEX_INSTALLER_USE_RELEASES_OPENAI_COM=false sh
```

```powershell
$env:CODEX_INSTALLER_USE_RELEASES_OPENAI_COM='false'; irm https://chatgpt.com/codex/install.ps1 | iex
```

Codex CLI can also be installed via the following package managers:

```shell
# Install using npm
npm install -g @openai/codex
```

```shell
# Install using Homebrew
brew install --cask codex
```

Then simply run `codex` to get started.

<details>
<summary>You can also go to the <a href="https://github.com/openai/codex/releases/latest">latest GitHub Release</a> and download the appropriate binary for your platform.</summary>

Each GitHub Release contains many executables, but in practice, you likely want one of these:

- macOS
  - Apple Silicon/arm64: `codex-aarch64-apple-darwin.tar.gz`
  - x86_64 (older Mac hardware): `codex-x86_64-apple-darwin.tar.gz`
- Linux
  - x86_64: `codex-x86_64-unknown-linux-musl.tar.gz`
  - arm64: `codex-aarch64-unknown-linux-musl.tar.gz`

Each archive contains a single entry with the platform baked into the name (e.g., `codex-x86_64-unknown-linux-musl`), so you likely want to rename it to `codex` after extracting it.

</details>

### Using Codex with your ChatGPT plan

Run `codex` and select **Sign in with ChatGPT**. We recommend signing into your ChatGPT account to use Codex as part of your Plus, Pro, Business, Edu, or Enterprise plan. [Learn more about what's included in your ChatGPT plan](https://help.openai.com/en/articles/11369540-codex-in-chatgpt).

You can also use Codex with an API key, but this requires [additional setup](https://developers.openai.com/codex/auth#sign-in-with-an-api-key).

## Docs

- [**Codex Documentation**](https://developers.openai.com/codex)
- [**Contributing**](./docs/contributing.md)
- [**Installing & building**](./docs/install.md)
- [**Open source fund**](./docs/open-source-fund.md)

This repository is licensed under the [Apache-2.0 License](LICENSE).
