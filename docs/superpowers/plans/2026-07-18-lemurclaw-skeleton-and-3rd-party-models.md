# lemurclaw 骨架 + 三方模型 实现计划(子项目 0+1,fork 布局)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 fork(openai/codex)内加 lemurclaw 的 3 个 Rust crate(lemurclaw/lemurclaw-transport/lemurclaw-gui),让 `--frontend tui` 跑通 codex-tui;直接改 fork 内 codex-rs/core + model-provider-info 加回三方模型(Chat),使 `wire_api=chat` 跑通一个三方模型。

**Architecture:** lemurclaw = openai/codex fork。codex-rs workspace 在 `codex-rs/`。lemurclaw crate 作为 codex-rs workspace 新 member(用 `workspace=true` 引用 codex 依赖,共享 Cargo.lock/patch/lints)。三方模型改动是 fork 内直接 commit(非 patch),与 upstream 同步靠 git merge。**实测验证此布局零摩擦**(lemurclaw-transport 编译+测试通过)。

**Tech Stack:** Rust(edition 2024,codex workspace 设定)、cargo workspace、git fork。

**Spec:** `docs/superpowers/specs/2026-07-18-lemurclaw-codex-gui-design.md`(§2 fork 布局 + §3 三方模型)

**前置事实(已验证):**
- `cargo check -p lemurclaw-transport`(在 `codex-rs/` 下跑)能编译,作为 codex-rs workspace member 零摩擦。
- codex crate 模板:`codex-rs/app-server-protocol/Cargo.toml`(`version.workspace=true` / `edition.workspace=true` / `[lints] workspace=true` / 依赖 `workspace=true`)。
- TUI 入口:`codex_tui::run_main(cli, arg0_paths, loader_overrides, explicit_remote_endpoint) -> io::Result<AppExitInfo>`(codex-rs/tui/src/lib.rs:849)。main.rs 极薄,透传即可。
- 三方模型 patch 目标(已在 spec §3 核实):`codex-rs/model-provider-info/src/lib.rs`(WireApi 只 Responses)、`codex-rs/core/src/client.rs:1787`(match 只 Responses arm)、`codex-rs/core/src/lib.rs`(无 chat_completions/model_family mod)、`codex-rs/core/src/openai_tools.rs`(不存在)。
- 参照源:just-every/code 的 `code-rs/core/src/`(chat_completions.rs=1471,model_family.rs=595,openai_tools.rs=2981)。**lemurclaw fork 不含 code-rs**,需从 github.com/just-every/code 读或单独 clone参照。

---

## 范围说明

只覆盖子项目 0(骨架)+ 子项目 1(三方模型)。不涉及 GUI(wry/tao/React)、webui、surface 组件(子项目 2+)。子项目 1 完成标准:`wire_api=chat` 配置能跑通一个三方模型(OpenRouter/ollama)的完整对话。

## 文件结构

| 文件 | 责任 | 创建/修改 |
|---|---|---|
| `codex-rs/Cargo.toml` | workspace 根,加 3 个 lemurclaw member | 修改(子项目 0) |
| `codex-rs/lemurclaw-transport/` | Transport trait + JSON 编解码(无 wry) | 新增(子项目 0) |
| `codex-rs/lemurclaw/` | lib+bin:runtime + config,TUI 透传 | 新增(子项目 0) |
| `codex-rs/lemurclaw-gui/` | 占位空壳(子项目 2 填充) | 新增(子项目 0) |
| `squat/lemurclaw/` | crates.io 占名 crate(workspace 外) | 新增(子项目 0) |
| `codex-rs/model-provider-info/src/lib.rs` | WireApi 加 Chat 变体 + 撤销硬报错 | 修改(子项目 1) |
| `codex-rs/core/src/client.rs` | match 加 Chat arm | 修改(子项目 1) |
| `codex-rs/core/src/chat_completions.rs` | 新文件,从 code-rs 移植 | 新增(子项目 1) |
| `codex-rs/core/src/model_family.rs` | 新文件,从 code-rs 移植 | 新增(子项目 1) |
| `codex-rs/core/src/openai_tools.rs` | 新建/增补 create_tools_json_for_chat_completions_api | 新增(子项目 1) |
| `codex-rs/core/src/lib.rs` | 加 mod chat_completions; mod model_family; | 修改(子项目 1) |
| `docs/superpowers/third-party-model-adaptations.md` | 移植时记录 codex-rs vs code-rs 差异 | 新增(子项目 1) |

---

## 子项目 0:基础骨架

### Task 0.1:创建 lemurclaw-transport crate 并加进 workspace

**Files:**
- Create: `codex-rs/lemurclaw-transport/Cargo.toml`、`src/lib.rs`
- Modify: `codex-rs/Cargo.toml`(加 member)

- [ ] **Step 1: 写 Cargo.toml(参照 codex-rs/app-server-protocol/Cargo.toml 模板)**

Create `codex-rs/lemurclaw-transport/Cargo.toml`:
```toml
[package]
name = "lemurclaw-transport"
version.workspace = true
edition.workspace = true
license = "MIT"

[lib]
name = "lemurclaw_transport"
path = "src/lib.rs"
doctest = false

[lints]
workspace = true

[dependencies]
codex-app-server-protocol = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
```

- [ ] **Step 2: 写 lib.rs(Transport trait + JSON 编解码 + 测试)**

Create `codex-rs/lemurclaw-transport/src/lib.rs`:
```rust
//! lemurclaw 传输抽象:让 GUI(wry 进程内)和 WebUI(WebSocket)共用同一套协议逻辑。
//! 此 crate 独立,不含 wry/tao——webui 模式只依赖它,不背 GUI 重依赖。

use codex_app_server_protocol::{ClientRequest, ServerNotification, ServerRequest};

/// 从后端到达前端的事件(包装 codex 的三类消息)。
#[derive(Debug, Clone)]
pub enum ServerEvent {
    Notification(ServerNotification),
    Request(ServerRequest),
}

/// 传输抽象。WryIpc(lemurclaw-gui)和 WebSocket(webui)各一实现。
pub trait Transport: Send {
    async fn send(&self, req: ClientRequest) -> std::io::Result<()>;
    async fn recv(&mut self) -> std::io::Result<Option<ServerEvent>>;
}

/// JSON 编码任意协议消息(codex JSON-RPC 格式)。
pub fn encode<T: serde::Serialize>(msg: &T) -> std::io::Result<String> {
    serde_json::to_string(msg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// JSON 解码任意协议消息。
pub fn decode<'de, T: serde::Deserialize<'de>>(json: &'de str) -> std::io::Result<T> {
    serde_json::from_str(json)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::ClientNotification;

    #[test]
    fn message_round_trip() {
        let msg = ClientNotification::Initialized;
        let json = encode(&msg).expect("encode");
        let back: ClientNotification = decode(&json).expect("decode");
        assert_eq!(serde_json::to_string(&back).unwrap(), json);
    }
}
```

- [ ] **Step 3: 加进 codex-rs workspace members**

Modify `codex-rs/Cargo.toml`:在 `members = [` 后插入 `"lemurclaw-transport",`。用 python 精确插入:
```bash
cd /Users/def/lemurclaw
python3 - << 'PY'
import pathlib
p = pathlib.Path("codex-rs/Cargo.toml")
s = p.read_text()
old = "members = [\n"
new = 'members = [\n    "lemurclaw-transport",\n'
assert old in s, "members = [ not found"
s = s.replace(old, new, 1)
p.write_text(s)
print("lemurclaw-transport added to members")
PY
```

- [ ] **Step 4: 跑测试验证**

Run:
```bash
cd /Users/def/lemurclaw/codex-rs
cargo test -p lemurclaw-transport
```
Expected: 1 test passed(`message_round_trip`)。

- [ ] **Step 5: Commit**

Run:
```bash
cd /Users/def/lemurclaw
git add codex-rs/lemurclaw-transport codex-rs/Cargo.toml codex-rs/Cargo.lock
git commit -m "feat(transport): add lemurclaw-transport crate to codex-rs workspace (Transport trait + JSON codec)"
```

---

### Task 0.2:创建 lemurclaw crate(lib+bin,Tui 透传 + Gui/Webui 桩)

**Files:**
- Create: `codex-rs/lemurclaw/Cargo.toml`、`src/lib.rs`、`src/main.rs`、`src/config.rs`
- Modify: `codex-rs/Cargo.toml`(加 member)

- [ ] **Step 1: 写 Cargo.toml**

Create `codex-rs/lemurclaw/Cargo.toml`:
```toml
[package]
name = "lemurclaw"
version.workspace = true
edition.workspace = true
license = "MIT"

[lib]
name = "lemurclaw"
path = "src/lib.rs"

[[bin]]
name = "lemurclaw"
path = "src/main.rs"

[lints]
workspace = true

[dependencies]
lemurclaw-transport = { workspace = true }
codex-tui = { workspace = true }
codex-arg0 = { workspace = true }
codex-config = { workspace = true }
clap = { workspace = true, features = ["derive"] }
serde = { workspace = true }
tokio = { workspace = true }
```

同时,在 `codex-rs/Cargo.toml` 的 `[workspace.dependencies]` 加一行:
```toml
lemurclaw-transport = { path = "lemurclaw-transport" }
```

- [ ] **Step 2: 写 config.rs(Frontend + RuntimeConfig + Cli)**

Create `codex-rs/lemurclaw/src/config.rs`:
```rust
//! lemurclaw 配置层:Frontend + RuntimeConfig + CLI。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
#[value(rename_all = "lowercase")]
pub enum Frontend {
    #[default]
    Tui,
    Gui,
    Webui,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeConfig {
    pub agent_name: String,
    pub frontend: Frontend,
    pub cwd: Option<std::path::PathBuf>,
    pub model: Option<String>,
    pub yolo: bool,
}

#[derive(clap::Parser, Debug)]
#[command(name = "lemurclaw", version, about = "Cross-platform agent runtime + TUI/GUI/WebUI")]
pub struct Cli {
    #[arg(long, value_enum, default_value_t = Frontend::Tui)]
    pub frontend: Frontend,
    #[arg(long, default_value = "lemurclaw")]
    pub agent_name: String,
    #[arg(long)]
    pub cwd: Option<std::path::PathBuf>,
    #[arg(long)]
    pub model: Option<String>,
    #[arg(long, default_value_t = false)]
    pub yolo: bool,
}

impl From<Cli> for RuntimeConfig {
    fn from(c: Cli) -> Self {
        RuntimeConfig {
            agent_name: c.agent_name,
            frontend: c.frontend,
            cwd: c.cwd,
            model: c.model,
            yolo: c.yolo,
        }
    }
}
```

- [ ] **Step 3: 写 lib.rs(run + Tui 透传 + Gui/Webui 桩)**

Create `codex-rs/lemurclaw/src/lib.rs`:
```rust
//! lemurclaw:跨平台 agent runtime + TUI/GUI/WebUI。
//! TUI 复用 codex-tui;GUI/WebUI 由 lemurclaw-gui 提供(子项目 2+)。

pub mod config;
pub use config::{Cli, Frontend, RuntimeConfig};
pub use lemurclaw_transport as transport;

pub struct ExitInfo(pub i32);

pub async fn run(config: RuntimeConfig) -> std::io::Result<ExitInfo> {
    match config.frontend {
        config::Frontend::Tui => run_tui().await,
        config::Frontend::Gui => Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "GUI frontend not implemented yet (subproject 2+); use --frontend tui",
        )),
        config::Frontend::Webui => Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "WebUI frontend not implemented yet (subproject 6); use --frontend tui",
        )),
    }
}

/// TUI 模式:透传到 codex_tui::run_main。
async fn run_tui() -> std::io::Result<ExitInfo> {
    use clap::Parser;
    let result = codex_arg0::arg0_dispatch_or_else(|arg0_paths| async move {
        let cli = codex_tui::Cli::parse();
        codex_tui::run_main(
            cli,
            arg0_paths,
            codex_config::LoaderOverrides::default(),
            None,
        )
        .await
    })
    .await;
    match result {
        Ok(exit_info) => Ok(ExitInfo(exit_info.exit_code())),
        Err(e) => Err(e),
    }
}
```

> **注:** 此 step 假设 `codex_tui::Cli`、`codex_arg0::arg0_dispatch_or_else`、`codex_config::LoaderOverrides`、`AppExitInfo::exit_code()` 这些符号。Step 5 若编译失败,逐一核实(参考 spec 前置事实 + grep codex-rs 源码)。

- [ ] **Step 4: 写 main.rs**

Create `codex-rs/lemurclaw/src/main.rs`:
```rust
use clap::Parser;
use lemurclaw::{config::Cli, run};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let cli = Cli::parse();
    let config = cli.into();
    let exit = run(config).await?;
    std::process::exit(exit.0);
}
```

- [ ] **Step 5: 加进 workspace members + 编译验证**

加 member(同 Task 0.1 Step 3 方式,加 `"lemurclaw",` 到 members)。

Run:
```bash
cd /Users/def/lemurclaw/codex-rs
cargo check -p lemurclaw 2>&1 | tail -15
```
Expected: 编译通过,或因 codex API 细节失败。若失败,诊断:
- `codex_tui::Cli` 不存在 → `grep -n "pub struct Cli\|pub use.*Cli" codex-rs/tui/src/lib.rs`
- `codex_arg0::arg0_dispatch_or_else` → `grep -n "arg0_dispatch_or_else" codex-rs/arg0/src/lib.rs`
- `LoaderOverrides` → `grep -rn "LoaderOverrides" codex-rs/config/src/`
- `exit_code()` → `grep -n "fn exit_code" codex-rs/tui/src/`

按错误修正 lib.rs 的 run_tui,重跑直到通过。

- [ ] **Step 6: Commit**

Run:
```bash
cd /Users/def/lemurclaw
git add codex-rs/lemurclaw codex-rs/Cargo.toml codex-rs/Cargo.lock
git commit -m "feat(lemurclaw): add lib+bin crate with TUI pass-through to codex_tui::run_main"
```

---

### Task 0.3:创建 lemurclaw-gui 空壳 crate

**Files:**
- Create: `codex-rs/lemurclaw-gui/Cargo.toml`、`src/lib.rs`
- Modify: `codex-rs/Cargo.toml`(加 member + workspace dep)

- [ ] **Step 1: 写 Cargo.toml + lib.rs**

Create `codex-rs/lemurclaw-gui/Cargo.toml`:
```toml
[package]
name = "lemurclaw-gui"
version.workspace = true
edition.workspace = true
license = "MIT"

[lib]
name = "lemurclaw_gui"
path = "src/lib.rs"

[lints]
workspace = true

[dependencies]
lemurclaw-transport = { workspace = true }
```

Create `codex-rs/lemurclaw-gui/src/lib.rs`:
```rust
//! lemurclaw GUI(wry+tao + AppServerClient 驱动 + ipc shim)。
//! 子项目 2 填充。此版本为空壳。
```

- [ ] **Step 2: 加进 workspace(members + workspace.dependencies 加 `lemurclaw-gui = { path = "lemurclaw-gui" }`)**

同前方式加 member `"lemurclaw-gui",` + workspace dep 行。

- [ ] **Step 3: 验证编译**

Run:
```bash
cd /Users/def/lemurclaw/codex-rs
cargo check -p lemurclaw-gui
```
Expected: 通过。

- [ ] **Step 4: Commit**

Run:
```bash
cd /Users/def/lemurclaw
git add codex-rs/lemurclaw-gui codex-rs/Cargo.toml codex-rs/Cargo.lock
git commit -m "feat(gui): add empty lemurclaw-gui crate (filled in subproject 2)"
```

---

### Task 0.4:创建 squat/lemurclaw 占位 crate(crates.io 占名)

**Files:**
- Create: `squat/lemurclaw/Cargo.toml`、`src/lib.rs`、`README.md`

> **关键:** `squat/` **不在** codex-rs workspace(codex-rs/Cargo.toml 的 members 不含它)。独立 crate,单独 publish。

- [ ] **Step 1: 写 squat/lemurclaw(Cargo.toml + 空 lib.rs + README)**

Create `squat/lemurclaw/Cargo.toml`:
```toml
[package]
name = "lemurclaw"
version = "0.0.1"
edition = "2021"
authors = ["Long, Wei <lostaim@gmail.com>"]
description = "Cross-platform agent runtime + TUI/GUI/WebUI (placeholder; full functionality via git)"
license = "MIT"
repository = "https://github.com/defims/lemurclaw"
homepage = "https://github.com/defims/lemurclaw"

[lib]
name = "lemurclaw"
path = "src/lib.rs"
```

Create `squat/lemurclaw/src/lib.rs`:
```rust
//! lemurclaw:跨平台 agent runtime + TUI/GUI/WebUI。
//!
//! 此 crate 是 crates.io 上的占位壳(完整功能依赖 codex,发不了 crates.io)。
//! 完整功能从 git 构建:`cargo install --git https://github.com/defims/lemurclaw`
```

Create `squat/lemurclaw/README.md`:
```markdown
# lemurclaw

Cross-platform agent runtime + TUI/GUI/WebUI, based on codex.

## Status

This crate is a **name placeholder** on crates.io. Full functionality depends
on [codex](https://github.com/openai/codex) whose Rust crates are not on
crates.io, so the full lemurclaw cannot be published here.

## Get the full lemurclaw

```
cargo install --git https://github.com/defims/lemurclaw
```

See https://github.com/defims/lemurclaw.
```

- [ ] **Step 2: 验证 publish dry-run**

Run:
```bash
cd /Users/def/lemurclaw/squat/lemurclaw
cargo publish --dry-run 2>&1 | tail -5
```
Expected: `Uploading lemurclaw v0.0.1 ... warning: aborting upload due to dry run`(成功)。

- [ ] **Step 3: Commit**

Run:
```bash
cd /Users/def/lemurclaw
git add squat/
git commit -m "chore: add squat/lemurclaw placeholder crate for crates.io name reservation"
```

---

### Task 0.5:TUI 冒烟测试 + workspace 验证

**Files:**
- Create: `codex-rs/lemurclaw/tests/tui_smoke.rs`

- [ ] **Step 1: 写冒烟测试(验证 codex_tui 符号可链接 + Frontend 工作)**

Create `codex-rs/lemurclaw/tests/tui_smoke.rs`:
```rust
use lemurclaw::{Frontend, RuntimeConfig};

#[test]
fn frontend_enum_works() {
    let cfg = RuntimeConfig {
        agent_name: "test".into(),
        frontend: Frontend::Tui,
        ..Default::default()
    };
    assert_eq!(cfg.frontend, Frontend::Tui);
}

/// 验证 codex_tui::run_main 符号可链接(不实际启动 TUI)。
/// 签名以 Task 0.2 Step 5 编译通过的版本为准;若不符按编译错误修正。
#[test]
fn codex_tui_run_main_linked() {
    // 取函数指针证明符号可见(run_main 是 async,用 cast)
    let _ = codex_tui::run_main as fn(
        codex_tui::Cli,
        codex_arg0::Arg0DispatchPaths,
        codex_config::LoaderOverrides,
        Option<codex_app_server_client::RemoteAppServerEndpoint>,
    ) -> _;
}
```

> **注:** 上面的类型签名以实际为准(codex_tui::Cli / Arg0DispatchPaths / LoaderOverrides / RemoteAppServerEndpoint 的确切路径)。若编译失败,按错误修正引用路径。测试目的是"符号可链接"。

- [ ] **Step 2: 跑测试**

Run:
```bash
cd /Users/def/lemurclaw/codex-rs
cargo test -p lemurclaw --test tui_smoke
```
Expected: 2 tests passed。签名不符则按错误修正。

- [ ] **Step 3: Commit**

Run:
```bash
cd /Users/def/lemurclaw
git add codex-rs/lemurclaw/tests/tui_smoke.rs codex-rs/Cargo.lock
git commit -m "test(lemurclaw): TUI smoke test (codex_tui symbols linkable + Frontend enum)"
```

---

## 子项目 0 完成标准

- [ ] 3 个 lemurclaw crate 在 codex-rs/(lemurclaw lib+bin / lemurclaw-transport / lemurclaw-gui 空壳)
- [ ] codex-rs/Cargo.toml 的 members 含这 3 个 + workspace.dependencies 含 lemurclaw-transport/lemurclaw-gui
- [ ] `cargo check` 全部通过(在 codex-rs/ 下)
- [ ] `--frontend gui`/`webui` 返回明确"未实现"错误
- [ ] `cargo test -p lemurclaw --test tui_smoke` 通过
- [ ] squat/lemurclaw 的 `cargo publish --dry-run` 通过

---

## 子项目 1:三方模型改动(直接改 fork 内 codex-rs/)

> **最重、风险最高的部分。** 逆 upstream #7782(故意删 Chat),chat_completions.rs 依赖 15 个 crate:: 类型需逐一适配。fork 布局下直接改文件(非 patch),与 upstream 同步靠 git merge。

### Task 1.1:WireApi::Chat(model-provider-info)

**Files:**
- Modify: `codex-rs/model-provider-info/src/lib.rs`

参照源:just-every/code 的 `code-rs/core/src/model_provider_info.rs:73-84`。需从 GitHub 读(github.com/just-every/code,raw)。

- [ ] **Step 1: 改 WireApi 枚举(加 Chat + ResponsesWebsocket)+ Display + Deserialize**

Modify `codex-rs/model-provider-info/src/lib.rs`(用 python3 精确替换,参照 spec §3.2 Part 1):
- 枚举(L54-61):`pub enum WireApi { Responses, ResponsesWebsocket, #[default] Chat }`
- Display(L63-70):加 ResponsesWebsocket/Chat arm
- Deserialize(L72-84):撤销 chat 硬报错,加 chat/responses-websocket → Ok

具体 python 替换脚本(三条 assert + replace)在 spec §3.2 已定。执行后验证:
```bash
cd /Users/def/lemurclaw/codex-rs
grep -A8 'pub enum WireApi' model-provider-info/src/lib.rs
cargo check -p codex-model-provider-info 2>&1 | tail -5
```
Expected: 枚举 3 变体;crate 编译通过(`CHAT_WIRE_API_REMOVED_ERROR` 变 unused warning,不阻断)。

- [ ] **Step 2: Commit**

Run:
```bash
cd /Users/def/lemurclaw
git add codex-rs/model-provider-info codex-rs/Cargo.lock
git commit -m "feat(models): add WireApi::Chat + ResponsesWebsocket to model-provider-info (re-add Chat support)"
```

---

### Task 1.2:移植 model_family.rs(相对独立)

**Files:**
- Create: `codex-rs/core/src/model_family.rs`
- Modify: `codex-rs/core/src/lib.rs`(加 mod)、`codex-rs/core/Cargo.toml`(若需新依赖)

参照源:just-every/code 的 `code-rs/core/src/model_family.rs`(595 行)。

- [ ] **Step 1: 从 just-every/code raw 取 model_family.rs,放进 codex-rs/core/src/**

Run:
```bash
cd /Users/def/lemurclaw
curl -sSL https://raw.githubusercontent.com/just-every/code/main/code-rs/core/src/model_family.rs \
  -o codex-rs/core/src/model_family.rs
wc -l codex-rs/core/src/model_family.rs
```
Expected: ~595 行。

- [ ] **Step 2: 在 core/src/lib.rs 加 `mod model_family;`**

找合适位置(如 `mod client;` 附近)加 `mod model_family;`。

- [ ] **Step 3: 核实并适配 use 路径(code_* → codex_*)**

Run:
```bash
cd /Users/def/lemurclaw/codex-rs
grep -nE "^use (crate|code_|codex_|super)" core/src/model_family.rs | head -20
cargo check -p codex-core 2>&1 | grep -E "error|model_family" | head -15
```
逐一修正:`code_protocol::*` → `codex_protocol::*`,其他 `code_*` → `codex_*`,适配签名差异。循环直到 model_family 相关 error 清零。

- [ ] **Step 4: Commit**

Run:
```bash
cd /Users/def/lemurclaw
git add codex-rs/core codex-rs/Cargo.lock
git commit -m "feat(models): port model_family.rs from just-every/code (adapt code_*→codex_* namespaces)"
```

---

### Task 1.3:移植 chat_completions.rs + 新建 openai_tools.rs(最重)

**Files:**
- Create: `codex-rs/core/src/chat_completions.rs`、`codex-rs/core/src/openai_tools.rs`
- Modify: `codex-rs/core/src/lib.rs`(加 mod)、`codex-rs/core/src/client.rs`(加 Chat arm)

参照源:just-every/code 的 `code-rs/core/src/chat_completions.rs`(1471行)+ `openai_tools.rs`(2981行,只摘 chat 相关函数)。

- [ ] **Step 1: 取 chat_completions.rs**

Run:
```bash
cd /Users/def/lemurclaw
curl -sSL https://raw.githubusercontent.com/just-every/code/main/code-rs/core/src/chat_completions.rs \
  -o codex-rs/core/src/chat_completions.rs
wc -l codex-rs/core/src/chat_completions.rs
```
在 core/src/lib.rs 加 `mod chat_completions;`。

- [ ] **Step 2: 核实并适配 15 个 use(spec §3.3 列出)**

勘察确认的依赖:`auth::AuthManager`、`ModelProviderInfo`(codex-rs 在独立 crate,需 `use codex_model_provider_info::ModelProviderInfo`)、`client_common::{Prompt,ResponseEvent,ResponseStream,replace_image_payloads_for_model,rewrite_image_generation_calls_for_input}`、`debug_logger::DebugLogger`、`error::*`、`model_family::ModelFamily`(Task 1.2 已移植)、`openai_tools::create_tools_json_for_chat_completions_api`(下一步)、`util::backoff`。

逐一核实 codex-rs 是否有同名:
```bash
cd /Users/def/lemurclaw/codex-rs
for sym in "auth::AuthManager" "client_common::Prompt" "client_common::ResponseEvent" "client_common::ResponseStream" "debug_logger::DebugLogger" "error::CodexErr" "error::RetryLimitReachedError" "util::backoff"; do
  echo "=== $sym ==="
  grep -rn "$sym" core/src/ | head -2
done
```
不存在的(fork-only helper)需在 chat_completions.rs 重实现或从 code-rs 摘函数进来。手动编辑修正所有 use 路径。

- [ ] **Step 3: 新建 openai_tools.rs(只摘 chat 相关函数)**

codex-rs 无 openai_tools.rs。从 code-rs 的 2981 行只摘 `create_tools_json_for_chat_completions_api` 及其直接依赖 helper(不要整文件)。

Run(先看函数定义与依赖):
```bash
curl -sSL https://raw.githubusercontent.com/just-every/code/main/code-rs/core/src/openai_tools.rs | grep -n "fn create_tools_json_for_chat_completions_api\|^fn \|^pub" | head -30
```
手动在 `codex-rs/core/src/openai_tools.rs` 写入 `create_tools_json_for_chat_completions_api` 及其依赖(从 code-rs 摘,适配 codex-rs 类型)。在 lib.rs 加 `mod openai_tools;`。

> **最耗时步骤**(4-8 小时),需理解 code-rs 函数依赖闭包。

- [ ] **Step 4: 迭代编译修正**

Run:
```bash
cd /Users/def/lemurclaw/codex-rs
cargo check -p codex-core 2>&1 | grep '^error' | head -20
```
循环:看 error → 修正 use/补 helper/适配签名 → 重跑。每个 error 是真实的 codex-rs vs code-rs 差异,逐一判断。记录到 `docs/superpowers/third-party-model-adaptations.md`。

直到 `cargo check -p codex-core` 通过(或仅剩 client.rs 未加 Chat arm 的 error,下一步处理)。

- [ ] **Step 5: 加 client.rs 的 Chat 分发 arm**

找到 client.rs 的 `match wire_api`(约 L1787):
```bash
cd /Users/def/lemurclaw/codex-rs
grep -n 'match wire_api' core/src/client.rs
```
加 Chat arm(参照 code-rs `code-rs/core/src/client.rs:612-707`,通过 mpsc + AggregatedChatStream 桥接回 ResponseStream)。具体参数以 `stream_chat_completions` 签名为准。ResponsesWebsocket arm 可暂返回未实现错误。

- [ ] **Step 6: 验证 codex-core 编译通过**

Run:
```bash
cd /Users/def/lemurclaw/codex-rs
cargo check -p codex-core 2>&1 | tail -5
```
Expected: 编译通过(或仅 warning)。

- [ ] **Step 7: Commit + 写 adaptations 记录**

Run:
```bash
cd /Users/def/lemurclaw
git add codex-rs/core codex-rs/Cargo.lock docs/superpowers/third-party-model-adaptations.md
git commit -m "feat(models): port chat_completions + openai_tools + client.rs Chat arm from just-every/code"
```

---

### Task 1.4:端到端验证 —— wire_api=chat 跑通一个三方模型

**Files:** (无代码改动,验证)

- [ ] **Step 1: 写三方模型 provider 配置**

准备 codex config(如 `~/.codex/config.toml` 或临时),配 OpenRouter 或本地 ollama:
```toml
[model_providers.openrouter]
name = "openrouter"
base_url = "https://openrouter.ai/api/v1"
env_key = "OPENROUTER_API_KEY"
wire_api = "chat"    # ← patch 后此值不再报错

[model]
name = "anthropic/claude-3.5-sonnet"
provider = "openrouter"
```
(无 API key 用本地 ollama:`base_url = "http://localhost:11434/v1"`, `wire_api="chat"`)

- [ ] **Step 2: 跑 lemurclaw TUI 对话**

Run:
```bash
cd /Users/def/lemurclaw/codex-rs
cargo run -p lemurclaw -- --frontend tui
# TUI 里输入"say hi",确认收到回复
```
Expected: TUI 启动,发 prompt 收到流式回复。若报 `wire_api=chat` 错 → 改动未生效;收到回复 → ✅ 三方模型跑通。

- [ ] **Step 3: 记录验证结果**

在 `docs/superpowers/third-party-model-adaptations.md` 末尾加验证段(用的 provider + 结果)。

- [ ] **Step 4: Commit**

Run:
```bash
cd /Users/def/lemurclaw
git add docs/superpowers/third-party-model-adaptations.md
git commit -m "docs(models): record wire_api=chat end-to-end verification"
```

---

### Task 1.5:验证 codex 上游测试不被破坏

**Files:** (无代码改动)

- [ ] **Step 1: 跑 codex-core 单元测试**

Run:
```bash
cd /Users/def/lemurclaw/codex-rs
cargo test -p codex-core --lib 2>&1 | tail -10
```
Expected: 上游测试全通过(加 Chat 不应破坏 Responses 路径)。失败则排查 patch 是否误改别的。

> 时间预期:codex-core 大,编译+测试数十分钟。可缩 `--lib model_provider`。

- [ ] **Step 2: 记录 + Commit**

在 adaptations.md 补测试结果。Commit:
```bash
git add docs/superpowers/third-party-model-adaptations.md
git commit -m "docs(models): record codex-core upstream test results"
```

---

## 子项目 1 完成标准

- [ ] WireApi 含 Chat + ResponsesWebsocket 变体(model-provider-info)
- [ ] chat_completions.rs + model_family.rs + openai_tools.rs 在 codex-rs/core/(从 code-rs 移植,适配 codex-rs)
- [ ] client.rs 的 wire_api match 含 Chat arm
- [ ] `cargo check -p codex-core` 通过
- [ ] **`wire_api=chat` 配置跑通一个三方模型完整对话**(端到端)
- [ ] `cargo test -p codex-core --lib` 不被破坏
- [ ] `third-party-model-adaptations.md` 记录所有适配点 + 验证结果

---

## 后续(不在本计划范围)

- **子项目 2:** GUI 基础设施 —— 填充 lemurclaw-gui(wry+tao + AppServerClient + ipc shim + tao proxy)+ React 骨架(assets/)
- **子项目 3+:** 核心对话循环、导航、配置模态、webui、发布
- 各自独立 spec → plan → 实现循环
