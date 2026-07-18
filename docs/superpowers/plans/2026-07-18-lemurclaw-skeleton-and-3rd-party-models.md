# lemurclaw 骨架 + 三方模型 Patch 实现计划(子项目 0+1)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 搭建 lemurclaw Cargo workspace 骨架(4 crate + submodule + patcher),让 `--frontend tui` 跑通 codex-rs/tui,并落地三方模型 patch(WireApi::Chat + chat_completions 移植),使 `wire_api=chat` 配置能跑通一个三方模型。

**Architecture:** lemurclaw 独立 workspace(members 只列 `crates/*`);`~/code` 整体作 submodule 挂 `vendor/code/`,codex-rs 作 path 依赖(cargo 向上找 codex-rs 自己的 workspace 根解析);**lemurclaw 根 Cargo.toml 必须逐字复制 codex-rs 的 `[patch.crates-io]`**(否则 ratatui fork 特性缺失,编译失败)。三方模型 patch 以 `patches/` apply 到 `vendor/code/codex-rs/`。

**Tech Stack:** Rust(edition 2024,与 codex-rs 对齐)、cargo workspace、git submodule、git apply patch。上游 crate:codex-tui、codex-app-server-client、codex-config、codex-core、codex-model-provider-info。

**Spec:** `docs/superpowers/specs/2026-07-18-lemurclaw-codex-gui-design.md`(子项目 0 + 1 + 章节 3 patch 契约)

---

## 范围说明

本计划**只覆盖子项目 0(骨架)和子项目 1(三方模型 patch)**。不涉及 GUI(wry/tao/React)、不涉及 webui WebSocket、不涉及 surface 组件(子项目 2+)。子项目 1 完成标准:**patch 能 apply、cargo 能编译、`wire_api=chat` 配置能跑通一个三方模型(如 OpenRouter)的一轮对话**。

## 关键约束(勘察验证)

1. **workspace members 绝不含 `vendor/`**——否则 cargo 把 codex-rs crate 纳入 lemurclaw workspace,`workspace=true` 解析失败。
2. **⚠️ 必须复制 `[patch.crates-io]`** 到 lemurclaw 根 Cargo.toml(4 条 + 1 个 ssh patch 块),否则 ratatui/crossterm 用 crates.io 版本,缺 fork 特性(`unstable-backend-writer` 等),codex-tui 编译失败。这是最大的集成风险。
3. **TUI 入口**:`codex_tui::run_main(cli, arg0_paths, loader_overrides, explicit_remote_endpoint) -> io::Result<AppExitInfo>`(lib.rs:849)。main.rs 极薄(~30 行),透传即可。
4. **patch 目标当前状态(已核实)**:
   - `model-provider-info/src/lib.rs`:`WireApi` 只有 `Responses`(L57-61);`Display`(L63-70)只处理 Responses;`Deserialize`(L72-84)硬报错 chat
   - `core/src/client.rs:1787-1825`:match 只有 Responses arm
   - `core/src/lib.rs`:无 `chat_completions`/`model_family` mod
   - `core/src/openai_tools.rs`:**不存在**(需新建,但只摘 chat 相关函数,不整文件搬 code-rs 的 2981 行)
5. **code-rs 参照源**(port from,均确认存在):`code-rs/core/src/chat_completions.rs`(1471)、`model_family.rs`(595)、`model_provider_info.rs`(1154)、`openai_tools.rs`(2981,只摘 `create_tools_json_for_chat_completions_api` 等 chat 相关函数)。
6. **submodule 锁定 ~/code 当前 HEAD**(实现时取最新 commit,记入 patches/README.md)。

## 文件结构

| 文件 | 责任 | 创建/修改 |
|---|---|---|
| `.gitmodules` | 登记 ~/code submodule | 修改(子项目 0) |
| `vendor/code/` | submodule(@ 固定 commit) | 新增(子项目 0) |
| `Cargo.toml` | workspace root + **[patch.crates-io] 复制** | 新增(子项目 0) |
| `crates/lemurclaw/Cargo.toml` | lib+bin crate 清单 | 新增(子项目 0) |
| `crates/lemurclaw/src/lib.rs` | run() / Frontend / RuntimeConfig | 新增(子项目 0) |
| `crates/lemurclaw/src/main.rs` | composition root | 新增(子项目 0) |
| `crates/lemurclaw/src/config.rs` | 配置层 + CLI 转换 | 新增(子项目 0) |
| `crates/lemurclaw-transport/Cargo.toml` | transport crate 清单 | 新增(子项目 0) |
| `crates/lemurclaw-transport/src/lib.rs` | Transport trait + JSON 编解码 | 新增(子项目 0) |
| `crates/lemurclaw-gui/Cargo.toml` | gui crate 清单 | 新增(子项目 0,空壳) |
| `crates/lemurclaw-gui/src/lib.rs` | 占位(子项目 2 填充) | 新增(子项目 0) |
| `squat/lemurclaw/Cargo.toml` | crates.io 占名 crate | 新增(子项目 0) |
| `squat/lemurclaw/src/lib.rs` | 占位空壳 | 新增(子项目 0) |
| `scripts/apply-patches.sh` | 幂等 apply patch 到 submodule | 新增(子项目 1) |
| `patches/codex-rs-third-party-models.patch` | 三方模型 patch | 新增(子项目 1) |
| `patches/README.md` | 锚点 commit + rebase 指南 | 新增(子项目 1) |
| `crates/lemurclaw/tests/tui_smoke.rs` | TUI 透传冒烟测试 | 新增(子项目 0) |
| `README.md` | 项目说明 | 修改(子项目 0) |

---

## 子项目 0:基础骨架

### Task 0.1:添加 ~/code submodule 并锁定 commit

**Files:**
- Create: `.gitmodules`、`vendor/code/`

- [ ] **Step 1: 添加 ~/code 为 submodule**

Run:
```bash
git submodule add https://github.com/just-every/code.git vendor/code
```
Expected: `vendor/code/` 检出 main HEAD,`.gitmodules` 生成。

- [ ] **Step 2: 记录锁定的 commit SHA**

Run:
```bash
git -C vendor/code rev-parse HEAD
```
Expected: 40 字符 SHA(记下来,Task 1.x 的 patches/README 要用)。

- [ ] **Step 3: 验证 codex-rs 是独立 workspace + 含 [patch]**

Run:
```bash
test -f vendor/code/codex-rs/Cargo.toml && \
grep -q "\[workspace\]" vendor/code/codex-rs/Cargo.toml && \
grep -q "nornagon/ratatui" vendor/code/codex-rs/Cargo.toml && \
echo "OK: codex-rs is workspace with ratatui patch"
```
Expected: `OK: codex-rs is workspace with ratatui patch`

- [ ] **Step 4: Commit**

Run:
```bash
git add .gitmodules vendor/code
git commit -m "chore: add just-every/code as submodule at vendor/ (codex-rs base)"
```

---

### Task 0.2:创建 lemurclaw workspace root(含 [patch] 复制)

**Files:**
- Create: `Cargo.toml`

- [ ] **Step 1: 写 workspace root Cargo.toml(含从 codex-rs 复制的 [patch])**

先读取 codex-rs 的 patch 段,确认内容:
```bash
sed -n '/\[patch.crates-io\]/,/^\[/{/^\[patch/p;/^\[/q;p}' vendor/code/codex-rs/Cargo.toml
```
Expected: 输出 4 条 patch(crossterm/ratatui/tokio-tungstenite/tungstenite)+ 可能的 ssh patch 块。

Create `Cargo.toml`:
```toml
# lemurclaw workspace root
# CRITICAL: members 只列 crates/*。绝不含 vendor/,否则 cargo 把 codex-rs crate
# 纳入本 workspace,导致其 workspace=true 依赖解析失败。
# 同时不含 squat/(占位 crate 独立发布,不在 workspace 内,且与 crates/lemurclaw 同名会冲突)。
[workspace]
resolver = "2"
members = [
    "crates/lemurclaw",
    "crates/lemurclaw-transport",
    "crates/lemurclaw-gui",
]

[workspace.package]
edition = "2024"
license = "MIT"
version = "0.0.1"

# codex-rs crate 通过 path dep 引用。cargo 向上找 workspace 根时停在
# vendor/code/codex-rs/Cargo.toml,在 codex-rs workspace 上下文构建。
[workspace.dependencies]
codex-tui = { path = "vendor/code/codex-rs/tui" }
codex-core = { path = "vendor/code/codex-rs/core" }
codex-app-server = { path = "vendor/code/codex-rs/app-server" }
codex-app-server-client = { path = "vendor/code/codex-rs/app-server-client" }
codex-app-server-protocol = { path = "vendor/code/codex-rs/app-server-protocol" }
codex-config = { path = "vendor/code/codex-rs/config" }
codex-model-provider-info = { path = "vendor/code/codex-rs/model-provider-info" }
codex-arg0 = { path = "vendor/code/codex-rs/arg0" }
tokio = { version = "1", features = ["full"] }
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }

# CRITICAL: 必须逐字复制 codex-rs 的 [patch.crates-io]。
# [patch] 在调用 cargo 的 workspace 根(lemurclaw)生效,不是嵌套 codex-rs 根。
# 不复制 → 用 crates.io 的 ratatui 0.29,缺 fork 特性(unstable-backend-writer 等)→ 编译失败。
# 下面的内容从 vendor/code/codex-rs/Cargo.toml 的 [patch] 段逐字复制(含 rev pin):
[patch.crates-io]
crossterm = { git = "https://github.com/nornagon/crossterm", rev = "87db8bfa6dc99427fd3b071681b07fc31c6ce995" }
ratatui = { git = "https://github.com/nornagon/ratatui", rev = "9b2ad1298408c45918ee9f8241a6f95498cdbed2" }
tokio-tungstenite = { git = "https://github.com/openai-oss-forks/tokio-tungstenite", rev = "0e5b2d73aa18dd9f0a50ee9ff199d5aef7594186" }
tungstenite = { git = "https://github.com/openai-oss-forks/tungstenite-rs", rev = "4fffad30fe373adbdcffab9545e9e9bf4f2fc19f" }

[patch."ssh://git@github.com/openai-oss-forks/tungstenite-rs.git"]
tungstenite = { git = "https://github.com/openai-oss-forks/tungstenite-rs", rev = "4fffad30fe373adbdcffab9545e9e9bf4f2fc19f" }
```

> **注:** 上面的 rev pin 值来自勘察时的 codex-rs/Cargo.toml。Task 0.1 Step 3 已确认 codex-rs 含这些 patch。若 codex-rs 的 pin 在你锁定 commit 时不同,以 `sed` 输出的实际值为准(替换上面的 rev)。

- [ ] **Step 2: 验证 [patch] 与 codex-rs 一致**

Run:
```bash
diff <(sed -n '/\[patch.crates-io\]/,/^$/p' vendor/code/codex-rs/Cargo.toml) \
     <(sed -n '/\[patch.crates-io\]/,/^$/p' Cargo.toml) && echo "patch matches" || echo "MISMATCH — fix rev pins"
```
Expected: `patch matches`(若 MISMATCH,用 codex-rs 的实际 rev 替换)。

- [ ] **Step 3: Commit**

Run:
```bash
git add Cargo.toml
git commit -m "chore: add lemurclaw workspace root with codex-rs [patch] replication"
```

---

### Task 0.3:创建 lemurclaw-transport crate

**Files:**
- Create: `crates/lemurclaw-transport/Cargo.toml`、`src/lib.rs`

- [ ] **Step 1: 写 Cargo.toml(纯协议层,无 wry/tao)**

Create `crates/lemurclaw-transport/Cargo.toml`:
```toml
[package]
name = "lemurclaw-transport"
edition.workspace = true
license.workspace = true
version.workspace = true

[dependencies]
codex-app-server-protocol = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
```

- [ ] **Step 2: 写失败的测试(Transport trait 可定义 + JSON round-trip)**

Create `crates/lemurclaw-transport/src/lib.rs`:
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
/// 发送:前端 → 后端(ClientRequest)。
/// 接收:后端 → 前端(ServerEvent)。
///
/// 用 async fn(edition 2024 原生支持)。实现是具体类型(WryIpcTransport/WebSocketTransport),
/// 不需要 `Box<dyn Transport>`,故无 dyn 兼容问题。
pub trait Transport: Send {
    /// 发一个 ClientRequest 到后端。
    async fn send(&self, req: ClientRequest) -> std::io::Result<()>;
    /// 收下一个后端事件。
    async fn recv(&mut self) -> std::io::Result<Option<ServerEvent>>;
}

/// JSON 编码 ClientRequest(codex 协议格式)。
pub fn encode_request(req: &ClientRequest) -> std::io::Result<String> {
    serde_json::to_string(req)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// JSON 解码 ClientRequest。
pub fn decode_request(json: &str) -> std::io::Result<ClientRequest> {
    serde_json::from_str(json)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::ClientRequest;

    #[test]
    fn request_round_trip() {
        // 用最简单的 ClientRequest 变体做 round-trip。具体变体以 codex 协议为准。
        // 这里测 Initialize(协议握手第一个请求)。
        let req = ClientRequest::Initialize(Default::default());
        let json = encode_request(&req).expect("encode");
        let back = decode_request(&json).expect("decode");
        // 比较序列化结果(枚举直接 == 可能不 impl,用 JSON 比对)
        assert_eq!(serde_json::to_string(&back).unwrap(), json);
    }
}
```

> **注:** `ClientRequest::Initialize(Default::default())` 是否成立取决于 codex 协议的 InitializeParams 是否 impl Default。若不成立,Task 0.3 Step 3 会编译失败——届时改为构造一个真实 InitializeParams(参考 codex-rs/app-server/src/in_process.rs 的 InitializeParams 字段)。

- [ ] **Step 3: 运行测试验证通过**

Run:
```bash
cargo test -p lemurclaw-transport
```
Expected: 1 test passed。若 `Initialize(Default::default())` 不成立,按上面注释修正构造方式,重跑直到通过。

- [ ] **Step 4: Commit**

Run:
```bash
git add crates/lemurclaw-transport
git commit -m "feat(transport): add lemurclaw-transport crate (Transport trait + JSON codec, no wry)"
```

---

### Task 0.4:创建 lemurclaw crate(lib+bin,Tui 透传 + Gui/Webui 桩)

**Files:**
- Create: `crates/lemurclaw/Cargo.toml`、`src/lib.rs`、`src/main.rs`、`src/config.rs`

- [ ] **Step 1: 写 Cargo.toml**

Create `crates/lemurclaw/Cargo.toml`:
```toml
[package]
name = "lemurclaw"
edition.workspace = true
license.workspace = true
version.workspace = true

[lib]
name = "lemurclaw"
path = "src/lib.rs"

[[bin]]
name = "lemurclaw"
path = "src/main.rs"

[dependencies]
lemurclaw-transport = { path = "../lemurclaw-transport" }
# TUI 透传所需:
codex-tui = { workspace = true }
codex-arg0 = { workspace = true }
codex-config = { workspace = true }
clap = { workspace = true }
serde = { workspace = true }
tokio = { workspace = true }
anyhow = { workspace = true }
```

- [ ] **Step 2: 写 config.rs(RuntimeConfig + Frontend + CLI)**

Create `crates/lemurclaw/src/config.rs`:
```rust
//! lemurclaw 配置层:RuntimeConfig + Frontend + CLI 解析。

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

/// lemurclaw CLI(顶层)。
#[derive(clap::Parser, Debug)]
#[command(name = "lemurclaw", version, about = "Cross-platform agent runtime + TUI/GUI/WebUI")]
pub struct Cli {
    /// 前端选择
    #[arg(long, value_enum, default_value_t = Frontend::Tui)]
    pub frontend: Frontend,

    /// agent 名称
    #[arg(long, default_value = "lemurclaw")]
    pub agent_name: String,

    /// 工作目录
    #[arg(long)]
    pub cwd: Option<std::path::PathBuf>,

    /// 模型
    #[arg(long)]
    pub model: Option<String>,

    /// yolo 模式
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontend_default_is_tui() {
        assert_eq!(Frontend::default(), Frontend::Tui);
    }

    #[test]
    fn frontend_serde_lowercase() {
        assert_eq!(serde_json::to_string(&Frontend::Gui).unwrap(), "\"gui\"");
        assert_eq!(
            serde_json::from_str::<Frontend>("\"webui\"").unwrap(),
            Frontend::Webui
        );
    }
}
```

- [ ] **Step 3: 写 lib.rs(run + Tui 透传 + Gui/Webui 桩)**

Create `crates/lemurclaw/src/lib.rs`:
```rust
//! lemurclaw:跨平台 agent runtime + TUI/GUI/WebUI。
//! TUI 复用 codex-rs/tui;GUI/WebUI 由 lemurclaw-gui 提供(子项目 2+)。

pub mod config;
pub use config::{Cli, Frontend, RuntimeConfig};

pub use lemurclaw_transport as transport;

/// 运行时退出信息(包装 codex 的 AppExitInfo)。
pub struct ExitInfo(pub i32);

/// 启动 runtime。根据 config.frontend 分流。
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
/// 复制 codex-rs/tui/src/main.rs 的 ~30 行调用模式。
async fn run_tui() -> std::io::Result<ExitInfo> {
    use clap::Parser;
    // codex_tui 的 Cli 与 codex 的 TopCli 结构。这里用 arg0_dispatch_or_else 包裹,
    // 与 codex-tui main.rs 一致(透传,不拦截)。
    let result = codex_arg0::arg0_dispatch_or_else(|arg0_paths| async move {
        let top_cli = codex_tui::Cli::parse();
        codex_tui::run_main(
            top_cli,
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

> **注:** 上面假设了 `codex_tui::Cli`(勘察确认 lib.rs:219 导出)、`codex_arg0::arg0_dispatch_or_else`、`codex_config::LoaderOverrides`、`AppExitInfo::exit_code()`。若 Task 0.4 Step 5 编译失败,逐一核实这些符号(见 Step 5 的诊断)。

- [ ] **Step 4: 写 main.rs(composition root)**

Create `crates/lemurclaw/src/main.rs`:
```rust
//! lemurclaw composition root:解析 CLI → run。

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

- [ ] **Step 5: 尝试编译(预期可能失败,记录现象)**

Run:
```bash
cargo check -p lemurclaw 2>&1 | tail -30
```
Expected: 可能成功,也可能因以下失败:
- `codex_tui::Cli` 不存在或签名不同 → 核实 `vendor/code/codex-rs/tui/src/lib.rs:219` 附近实际导出
- `codex_arg0::arg0_dispatch_or_else` 不存在 → 核实 `vendor/code/codex-rs/arg0/src/lib.rs`
- `LoaderOverrides` 在 codex_config 还是别处 → `grep -rn "LoaderOverrides" vendor/code/codex-rs/config/src/`
- `AppExitInfo::exit_code()` 方法名 → `grep -n "fn exit_code" vendor/code/codex-rs/tui/src/`

记录输出。若成功 → Step 6;若失败 → 根据上面诊断修正 lib.rs 的 run_tui,重跑直到通过。

> **说明:** Task 0.4 目标是结构就位 + TUI 透传编译通过。首次跑可能因 codex API 细节失败,这是正常的"首次集成"摩擦,迭代修正。

- [ ] **Step 6: Commit**

Run:
```bash
git add crates/lemurclaw
git commit -m "feat(lemurclaw): add lib+bin crate with TUI pass-through to codex_tui::run_main"
```

---

### Task 0.5:创建 lemurclaw-gui 空壳 crate

**Files:**
- Create: `crates/lemurclaw-gui/Cargo.toml`、`src/lib.rs`

- [ ] **Step 1: 写 Cargo.toml(最小依赖,wry/tao 子项目 2 加)**

Create `crates/lemurclaw-gui/Cargo.toml`:
```toml
[package]
name = "lemurclaw-gui"
edition.workspace = true
license.workspace = true
version.workspace = true

[dependencies]
lemurclaw-transport = { path = "../lemurclaw-transport" }
anyhow = { workspace = true }
```

- [ ] **Step 2: 写占位 lib.rs**

Create `crates/lemurclaw-gui/src/lib.rs`:
```rust
//! lemurclaw GUI(wry+tao + AppServerClient 驱动 + ipc shim)。
//! 子项目 2 填充。此版本为空壳,保证 workspace 编译。
```

- [ ] **Step 3: 验证编译**

Run:
```bash
cargo check -p lemurclaw-gui
```
Expected: 编译通过。

- [ ] **Step 4: Commit**

Run:
```bash
git add crates/lemurclaw-gui
git commit -m "feat(gui): add empty lemurclaw-gui crate (filled in subproject 2)"
```

---

### Task 0.6:创建 squat/lemurclaw 占位 crate(crates.io 占名)

**Files:**
- Create: `squat/lemurclaw/Cargo.toml`、`src/lib.rs`、`README.md`

> **关键:** `squat/` **不在** workspace members 里(与 `crates/lemurclaw` 同名会冲突)。它是独立 crate,单独 `cargo publish`。

- [ ] **Step 1: 写 Cargo.toml(无 codex 依赖,纯占名)**

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

- [ ] **Step 2: 写占位 lib.rs**

Create `squat/lemurclaw/src/lib.rs`:
```rust
//! lemurclaw:跨平台 agent runtime + TUI/GUI/WebUI。
//!
//! 此 crate 是 crates.io 上的占位壳(完整功能依赖 codex,发不了 crates.io)。
//! 完整功能从 git 构建:
//! ```text
//! cargo install --git https://github.com/defims/lemurclaw
//! ```
//! 或其他项目依赖:
//! ```text
//! lemurclaw = { git = "https://github.com/defims/lemurclaw" }
//! ```
```

- [ ] **Step 3: 写 README.md**

Create `squat/lemurclaw/README.md`:
```markdown
# lemurclaw

Cross-platform agent runtime + TUI/GUI/WebUI, based on codex.

## Status

This crate is a **name placeholder** on crates.io. The full functionality
depends on [codex](https://github.com/openai/codex) whose Rust crates are not
published to crates.io, so the full lemurclaw cannot be published here either.

## Get the full lemurclaw

Install the binary:
```
cargo install --git https://github.com/defims/lemurclaw
```

Or depend on it as a library:
```toml
lemurclaw = { git = "https://github.com/defims/lemurclaw" }
```

See https://github.com/defims/lemurclaw for source, builds, and docs.
```

- [ ] **Step 4: 验证占位 crate 能 publish(dry-run)**

Run:
```bash
cd squat/lemurclaw && cargo publish --dry-run 2>&1 | tail -8
```
Expected: `Uploading lemurclaw v0.0.1 ... warning: aborting upload due to dry run`(成功;真实 publish 需要 `cargo login` + API token,留作首次发布时手动执行)。

- [ ] **Step 5: Commit(占位 crate 源码进 git,真实 publish 后续手动)**

Run:
```bash
cd ../..
git add squat/
git commit -m "chore: add squat/lemurclaw placeholder crate for crates.io name reservation"
```

---

### Task 0.7:验证 workspace 整体 + TUI 冒烟测试

**Files:**
- Create: `crates/lemurclaw/tests/tui_smoke.rs`

- [ ] **Step 1: 写 TUI 冒烟测试(验证 codex_tui 符号可链接)**

Create `crates/lemurclaw/tests/tui_smoke.rs`:
```rust
//! 子项目 0 完成标准:验证 codex_tui::run_main 符号可从 lemurclaw 链接。
//! 不实际启动 TUI(那需要终端),只验证符号可见 + Frontend enum 工作。

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

/// 验证 codex_tui 的关键符号可链接(run_main 是 async fn,取函数指针证明可见)。
#[test]
fn codex_tui_run_main_is_linked() {
    let _fn: fn(
        codex_tui::Cli,
        codex_arg0::Arg0DispatchPaths,
        codex_config::LoaderOverrides,
        Option<codex_app_server_client::RemoteAppServerEndpoint>,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = std::io::Result<codex_tui::AppExitInfo>> + Send>,
    > = |a, b, c, d| Box::pin(codex_tui::run_main(a, b, c, d));
}
```

> **注:** `run_main` 的确切签名以 Task 0.4 Step 5 编译通过的版本为准。若 `RemoteAppServerEndpoint` 的路径不对(可能在 codex_app_server_client 或别的 crate),按编译错误修正。此测试的目的是"符号可链接",签名以实际为准。

- [ ] **Step 2: 跑测试**

Run:
```bash
cargo test -p lemurclaw --test tui_smoke
```
Expected: 2 tests passed。若符号路径不对,按编译错误修正(参考 Task 0.4 Step 5 诊断)。

- [ ] **Step 3: 验证 workspace metadata 完整**

Run:
```bash
cargo metadata --no-deps --format-version 1 > /dev/null && echo "OK: workspace valid"
```
Expected: `OK: workspace valid`。

- [ ] **Step 4: Commit**

Run:
```bash
git add crates/lemurclaw/tests/tui_smoke.rs
git commit -m "test(lemurclaw): TUI smoke test (codex_tui symbols linkable + Frontend enum)"
```

---

### Task 0.8:更新 README

**Files:**
- Modify: `README.md`

- [ ] **Step 1: 写 README**

Modify `README.md`(替换原 `# lemurclaw` 单行):
```markdown
# lemurclaw

Cross-platform agent runtime + TUI/GUI/WebUI, based on [codex](https://github.com/openai/codex).

## Status

In development (subprojects 0+1: skeleton + third-party model patch).

## Architecture

- **TUI**: reuses `codex-rs/tui`, zero changes.
- **GUI** (planned): React + wry webview, in-process IPC.
- **WebUI** (planned): same React frontend, pure web + WebSocket.
- `--frontend tui|gui|webui` selects at startup.

Windows sandbox supported via codex-rs/windows-sandbox-rs.

## Build

```bash
git submodule update --init          # fetch vendor/code (codex-rs base)
scripts/apply-patches.sh             # apply third-party model patch (subproject 1)
cargo build -p lemurclaw
```

## Install (once released)

```
cargo install --git https://github.com/defims/lemurclaw
```

Full functionality depends on codex (not on crates.io), so this project is
distributed via git + GitHub release binaries, not crates.io. See
`docs/superpowers/specs/2026-07-18-lemurclaw-codex-gui-design.md`.
```

- [ ] **Step 2: Commit**

Run:
```bash
git add README.md
git commit -m "docs: update README with architecture + build + install instructions"
```

---

## 子项目 0 完成标准

- [ ] 4 crate 结构就位(lemurclaw lib+bin / lemurclaw-transport / lemurclaw-gui)+ squat 占位
- [ ] workspace root 的 members 只列 `crates/*`,不含 `vendor/`、不含 `squat/`
- [ ] **[patch.crates-io] 从 codex-rs 逐字复制到根 Cargo.toml**(Task 0.2 Step 2 验证一致)
- [ ] submodule 在 `vendor/code/` 且 codex-rs 是独立 workspace
- [ ] `--frontend gui`/`webui` 返回明确的"未实现"错误(不 panic)
- [ ] `cargo test -p lemurclaw --test tui_smoke` 通过(codex_tui 符号可链接)
- [ ] `squat/lemurclaw` 的 `cargo publish --dry-run` 通过

---

## 子项目 1:三方模型 Patch(WireApi::Chat + chat_completions 移植)

> **重要说明:** 子项目 1 是 codex 方案最重、风险最高的部分。patch 逆 upstream(#7782 故意删 Chat),且 `chat_completions.rs` 依赖 15 个 codex-rs core 的 `crate::` 类型,移植需逐一适配(不是 copy)。本计划把它拆成小步,每步可独立验证。

### Task 1.1:编写 apply-patches 脚本

**Files:**
- Create: `scripts/apply-patches.sh`

- [ ] **Step 1: 写幂等 apply 脚本**

Create `scripts/apply-patches.sh`:
```bash
#!/usr/bin/env bash
# 对 vendor/code/codex-rs/ 应用 lemurclaw 维护的三方模型 patch。
# 幂等:已 apply 则跳过;不能干净 apply 则报错(提示 rebase)。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
VENDOR="$ROOT/vendor/code/codex-rs"
PATCH_DIR="$ROOT/patches"

if [ ! -d "$VENDOR/.git" ] && [ ! -f "$VENDOR/.git" ]; then
  echo "ERROR: $VENDOR is not a git checkout (submodule not initialized?)" >&2
  exit 1
fi

shopt -s nullglob
patches=( "$PATCH_DIR"/*.patch )
if [ ${#patches[@]} -eq 0 ]; then
  echo "No patches in $PATCH_DIR; nothing to apply."
  exit 0
fi

cd "$VENDOR"
for p in "${patches[@]}"; do
  name="$(basename "$p")"
  if git apply --check "$p" 2>/dev/null; then
    echo "Applying $name ..."
    git apply "$p"
    echo "  applied."
  elif git apply --check --reverse "$p" 2>/dev/null; then
    echo "$name already applied; skipping."
  else
    echo "ERROR: $name does not apply cleanly (neither forward nor reverse)." >&2
    echo "  Likely upstream drifted. Rebase the patch against current codex-rs." >&2
    exit 1
  fi
done

echo "All patches applied."
```

- [ ] **Step 2: 赋可执行权限 + 验证(无 patch 时 nothing to apply)**

Run:
```bash
chmod +x scripts/apply-patches.sh
scripts/apply-patches.sh
```
Expected: `No patches in patches/; nothing to apply.`

- [ ] **Step 3: Commit**

Run:
```bash
git add scripts/apply-patches.sh
git commit -m "chore: add idempotent apply-patches.sh for codex-rs submodule patches"
```

---

### Task 1.2:生成 patch Part 1 —— WireApi::Chat(model-provider-info crate)

**Files:**
- (修改 submodule 工作区,导出进 patch)
- Create: `patches/codex-rs-third-party-models.patch`(本 task 生成第一版,后续 task 追加)

- [ ] **Step 1: 改 model-provider-info 的 WireApi(加 Chat + ResponsesWebsocket)+ Display + Deserialize**

参照 `vendor/code/code-rs/core/src/model_provider_info.rs:73-84`(fork 的 inlined 版本)。在 submodule 内改:

```bash
cd vendor/code/codex-rs
F=model-provider-info/src/lib.rs

# 1) WireApi 枚举加变体。当前(L57-61):
#    pub enum WireApi { #[default] Responses, }
#    改为加 Chat + ResponsesWebsocket,默认改 Chat(参照 code-rs)。
#    用 python 精确替换(避免 sed 缩进问题):
python3 - << 'PY'
import re, pathlib
p = pathlib.Path("model-provider-info/src/lib.rs")
s = p.read_text()
# 替换枚举体
old = """pub enum WireApi {
    /// The Responses API exposed by OpenAI at `/v1/responses`.
    #[default]
    Responses,
}"""
new = """pub enum WireApi {
    /// The Responses API exposed by OpenAI at `/v1/responses`.
    Responses,
    /// Responses API over WebSocket.
    ResponsesWebsocket,
    /// The Chat Completions API (`/v1/chat/completions`), used by third-party providers.
    #[default]
    Chat,
}"""
assert old in s, "WireApi enum body not found verbatim — check upstream drift"
s = s.replace(old, new)
p.write_text(s)
print("WireApi enum updated")
PY

# 2) Display impl(L63-70)加 Chat + ResponsesWebsocket arm
python3 - << 'PY'
import pathlib
p = pathlib.Path("model-provider-info/src/lib.rs")
s = p.read_text()
old = """        WireApi::Responses => write!(f, "responses"),"""
new = """        WireApi::Responses => write!(f, "responses"),
        WireApi::ResponsesWebsocket => write!(f, "responses-websocket"),
        WireApi::Chat => write!(f, "chat"),"""
assert old in s, "Display Responses arm not found"
s = s.replace(old, new)
p.write_text(s)
print("Display impl updated")
PY

# 3) Deserialize(L72-84)撤销 chat 硬报错,加 chat + responses-websocket
python3 - << 'PY'
import pathlib
p = pathlib.Path("model-provider-info/src/lib.rs")
s = p.read_text()
old = """        match value.as_str() {
            "responses" => Ok(Self::Responses),
            "chat" => Err(serde::de::Error::custom(CHAT_WIRE_API_REMOVED_ERROR)),
            _ => Err(serde::de::Error::unknown_variant(&value, &["responses"])),
        }"""
new = """        match value.as_str() {
            "responses" => Ok(Self::Responses),
            "responses-websocket" => Ok(Self::ResponsesWebsocket),
            "chat" => Ok(Self::Chat),
            _ => Err(serde::de::Error::unknown_variant(
                &value,
                &["responses", "responses-websocket", "chat"],
            )),
        }"""
assert old in s, "Deserialize match not found verbatim"
s = s.replace(old, new)
p.write_text(s)
print("Deserialize impl updated")
PY
cd ../..
```

- [ ] **Step 2: 验证改动(枚举 + Display + Deserialize 三处)**

Run:
```bash
cd vendor/code/codex-rs
grep -nA6 'pub enum WireApi' model-provider-info/src/lib.rs | head -10
grep -n 'Chat => write!' model-provider-info/src/lib.rs
grep -n '"chat" => Ok(Self::Chat)' model-provider-info/src/lib.rs
git diff --stat
cd ../..
```
Expected: 枚举含 3 变体(Responses/ResponsesWebsocket/Chat);Display 含 Chat arm;Deserialize 含 chat=>Ok;diff 改动 1 文件约 6-8 行。

- [ ] **Step 3: 验证 model-provider-info crate 编译**

Run:
```bash
cd vendor/code/codex-rs
cargo check -p codex-model-provider-info 2>&1 | tail -10
cd ../..
```
Expected: 编译通过(枚举改动自洽)。若失败,看错误修正 python 脚本(可能是 `CHAT_WIRE_API_REMOVED_ERROR` 常量变成 unused warning,不阻断;若报错则保留常量不删)。

- [ ] **Step 4: 导出 patch 到 lemurclaw 仓库**

Run:
```bash
cd vendor/code/codex-rs
git diff > ../../../patches/codex-rs-third-party-models.patch
git checkout -- .          # 撤销工作区改动(稍后用 apply-patches.sh 正式应用)
cd ../..
ls -la patches/codex-rs-third-party-models.patch
```
Expected: patch 文件创建,约 30-40 行。

- [ ] **Step 5: 写 patches/README.md**

Create `patches/README.md`:
```markdown
# codex-rs patches

lemurclaw 维护的对 `vendor/code/codex-rs/` 的 patch。

## 锚点 commit

- 上游仓库:`https://github.com/just-every/code.git`(含 codex-rs 上游镜像)
- codex-rs 锁定 commit:`<填入 Task 0.1 Step 2 记录的 SHA>`
- 锁定日期:2026-07-18

## patch 内容

`codex-rs-third-party-models.patch` 给 codex-rs 加回三方模型(Chat Completions)支持。
upstream 在 #7782 故意删了 Chat,lemurclaw 逆向加回。参照 `vendor/code/code-rs/`(fork)的实现。

### 改动点
1. `model-provider-info/src/lib.rs`:`WireApi` 加 `Chat`+`ResponsesWebsocket` 变体;Display/Deserialize 加对应 arm;撤销 chat 硬报错。
2. `core/src/client.rs`:分发 match 加 Chat arm(调 stream_chat_completions)。
3. `core/src/chat_completions.rs`(新文件):从 code-rs 移植,适配 codex-rs 类型。
4. `core/src/model_family.rs`(新文件):从 code-rs 移植。
5. `core/src/openai_tools.rs`(新文件或增补):含 create_tools_json_for_chat_completions_api。
6. `core/src/lib.rs`:加 `mod chat_completions; mod model_family;`(+ openai_tools 若新建)。

## 应用

```bash
scripts/apply-patches.sh   # 幂等
```

## 持久化策略

patch 在构建时应用(apply-patches.sh),submodule 工作区保持指向上游 commit。
不提交 submodule 内的改动。CI/本地构建前必须运行 apply-patches.sh。

## rebase 流程(codex-rs mirror 刷新时)

```bash
git -C vendor/code checkout <new-commit>
# 重新基于新 commit 生成 patch:进 submodule 手动重做改动,git diff 导出
scripts/apply-patches.sh   # 验证
cargo test                 # 验证 lemurclaw + codex 上游测试
git add vendor/code patches/
git commit -m "chore: bump codex-rs to <new-commit>; rebase third-party-model patch"
```

## 风险

- **逆 upstream**:#7782 故意删 Chat,每次 mirror 刷 client.rs/model-provider-info 冲突概率高。
- **chat_completions.rs 依赖闭包**:15 个 crate:: 类型,codex-rs vs code-rs 分歧逐一适配。
- `effects::execute` 类的签名变更同理:client.rs 的 ModelClient 若重构,patch Part 2 重写。
```

> **注:** Task 1.2 Step 5 的 `<填入...>` 是执行时填的具体 SHA(从 Task 0.1 Step 2 记录取),非设计占位符。

- [ ] **Step 6: Commit patch + README**

Run:
```bash
git add patches/
git commit -m "feat(patch): add WireApi::Chat to codex-rs model-provider-info (patch Part 1)"
```

---

### Task 1.3:应用 patch Part 1 并验证幂等

**Files:**
- (验证已 apply 的 submodule)

- [ ] **Step 1: 应用 patch**

Run:
```bash
scripts/apply-patches.sh
```
Expected: `Applying codex-rs-third-party-models.patch ... applied. All patches applied.`

- [ ] **Step 2: 再次运行验证幂等**

Run:
```bash
scripts/apply-patches.sh
```
Expected: `codex-rs-third-party-models.patch already applied; skipping. All patches applied.`

- [ ] **Step 3: 验证 WireApi 现在含 Chat**

Run:
```bash
grep -A8 'pub enum WireApi' vendor/code/codex-rs/model-provider-info/src/lib.rs
```
Expected: 3 变体(Responses/ResponsesWebsocket/Chat),Chat 是 `#[default]`。

无需 commit(patch 文件已提交;submodule 改动不提交,靠构建时 apply)。

---

### Task 1.4:移植 chat_completions.rs + model_family.rs + openai_tools.rs(patch Part 2-5,最重)

> **这是子项目 1 的核心工作,预计最大工作量。** 分多个子步,每步验证编译。

**Files:**
- (新增文件进 submodule core/src/,导出进 patch)
- Modify: `patches/codex-rs-third-party-models.patch`

- [ ] **Step 1: copy model_family.rs(code-rs → codex-rs,该文件相对独立)**

Run:
```bash
cp vendor/code/code-rs/core/src/model_family.rs vendor/code/codex-rs/core/src/model_family.rs
```
在 `vendor/code/codex-rs/core/src/lib.rs` 加模块声明。先找到合适的 mod 声明位置:
```bash
grep -n '^mod client;' vendor/code/codex-rs/core/src/lib.rs
```
在该行附近插入 `mod model_family;`(用编辑器或 sed)。

- [ ] **Step 2: 核实 model_family.rs 的依赖,逐个适配**

Run:
```bash
grep -nE '^use (crate|codex_|super)' vendor/code/codex-rs/core/src/model_family.rs | head -20
```
Expected: 列出它的 `use` 依赖。逐一核实这些类型在 codex-rs core 是否存在、签名是否一致:
- `code_protocol::openai_models::ModelInfo` → codex-rs 里是 `codex_protocol::...`(改名)
- 其他 `code_*` → 对应 `codex_*`

修正 model_family.rs 的 `use` 路径(把 `code_*` 改成 `codex_*`,适配签名差异)。

- [ ] **Step 3: 验证 model_family.rs 编译**

Run:
```bash
cd vendor/code/codex-rs
cargo check -p codex-core 2>&1 | grep -E 'error|model_family' | head -20
cd ../..
```
Expected: 无 error(或 error 只来自尚未移植的 chat_completions)。修正 use 路径直到 model_family 相关 error 清零。

- [ ] **Step 4: copy chat_completions.rs + 适配 15 个 crate:: 依赖**

Run:
```bash
cp vendor/code/code-rs/core/src/chat_completions.rs vendor/code/codex-rs/core/src/chat_completions.rs
```
在 lib.rs 加 `mod chat_completions;`(同 Step 1 方式)。

- [ ] **Step 5: 核实并适配 chat_completions.rs 的 15 个 use**

勘察列出的依赖(已确认):
```
use crate::auth::AuthManager;
use crate::ModelProviderInfo;       // 注意:codex-rs 里在独立 crate model-provider-info
use crate::client_common::{Prompt, ResponseEvent, ResponseStream, replace_image_payloads_for_model, rewrite_image_generation_calls_for_input};
use crate::debug_logger::DebugLogger;
use crate::error::{CodexErr, Result, RetryLimitReachedError, UnexpectedResponseError};
use crate::model_family::ModelFamily;   // 已在 Step 1 移植
use crate::openai_tools::create_tools_json_for_chat_completions_api;  // 下一步新建
use crate::util::backoff;
```

逐一核实(用 grep):
```bash
cd vendor/code/codex-rs
for sym in "auth::AuthManager" "client_common::Prompt" "client_common::ResponseEvent" "client_common::ResponseStream" "debug_logger::DebugLogger" "error::CodexErr" "error::RetryLimitReachedError" "util::backoff"; do
  echo "=== $sym ==="
  grep -rn "$sym" core/src/ | head -2
done
cd ../..
```
Expected: 大部分在 codex-rs core 存在。不存在的(如某些 fork-only helper)需在 chat_completions.rs 里重实现或从 code-rs 摘过来。

**关键适配点:**
- `crate::ModelProviderInfo` → codex-rs 里 ModelProviderInfo 在 `codex_model_provider_info` crate(不在 core)。chat_completions.rs 顶部加 `use codex_model_provider_info::ModelProviderInfo;`(core 已依赖该 crate)。
- `replace_image_payloads_for_model` / `rewrite_image_generation_calls_for_input` → 核实 codex-rs client_common 是否有同名;无则从 code-rs 摘函数进来。

手动编辑 `vendor/code/codex-rs/core/src/chat_completions.rs` 修正所有 use 路径。

- [ ] **Step 6: 新建 openai_tools.rs(只摘 chat 相关函数)**

codex-rs 无 openai_tools.rs,code-rs 的有 2981 行(含大量无关函数)。**只摘 `create_tools_json_for_chat_completions_api` 及其依赖**:
```bash
# 看 code-rs 里这个函数的定义和它依赖的 helper
grep -n 'create_tools_json_for_chat_completions_api\|fn ' vendor/code/code-rs/core/src/openai_tools.rs | head -30
```
在 `vendor/code/codex-rs/core/src/` 新建 `openai_tools.rs`,**手动**写入 `create_tools_json_for_chat_completions_api` 及其直接依赖的 helper(从 code-rs 摘,不要整文件 copy)。在 lib.rs 加 `mod openai_tools;`。

> **这是最耗时的步骤**——要理解 code-rs 的函数依赖闭包,只摘 chat 相关部分。预计 4-8 小时专注工作。

- [ ] **Step 7: 迭代编译修正**

Run:
```bash
cd vendor/code/codex-rs
cargo check -p codex-core 2>&1 | grep '^error' | head -20
cd ../..
```
循环:看 error → 修正 use 路径/补 helper/适配签名 → 重跑。直到 `cargo check -p codex-core` 通过(或仅剩 client.rs 尚未加 Chat arm 的 error,下一步处理)。

> **预期:** 这一步可能需要多轮。每个 error 都是真实的"codex-rs vs code-rs 类型分歧",要逐一判断:是改名(`code_*`→`codex_*`)、签名变化、还是 fork-only 需补实现。记录每个修正到 patches/README 的"已知适配点"。

- [ ] **Step 8: 加 client.rs 的 Chat 分发 arm(patch Part 2)**

找到 client.rs:1787 的 match 块:
```bash
grep -n 'match wire_api' vendor/code/codex-rs/core/src/client.rs
```
在该 match 加 Chat arm(参照 code-rs `core/src/client.rs:612-707`):
```rust
// 在 match wire_api { 的 Responses arm 之后加:
            WireApi::Chat => {
                let response_stream = chat_completions::stream_chat_completions(...);
                // 通过 mpsc + AggregatedChatStream 桥接回 ResponseStream
                // (参照 code-rs client.rs:656-707 的桥接逻辑)
            }
            WireApi::ResponsesWebsocket => {
                // 参照 code-rs(若 lemurclaw 不需要 ws 可暂返回未实现)
                return Err(CodexErr::Other("ResponsesWebsocket not yet supported".into()));
            }
```
具体参数以 `stream_chat_completions` 的签名(在 chat_completions.rs)为准。参照 code-rs client.rs 的 Chat arm 逐行适配。

- [ ] **Step 9: 验证 codex-core 编译通过**

Run:
```bash
cd vendor/code/codex-rs
cargo check -p codex-core 2>&1 | tail -10
cd ../..
```
Expected: 编译通过(或仅 warning)。若 error,继续 Step 7 的迭代修正循环。

- [ ] **Step 10: 重新导出完整 patch(含所有新文件 + 改动)**

Run:
```bash
cd vendor/code/codex-rs
git diff > ../../../patches/codex-rs-third-party-models.patch
git checkout -- .
cd ../..
```
> **注意:** `git diff` 对**新文件**(chat_completions.rs/model_family.rs/openai_tools.rs)默认不包含——它们是 untracked。需要 `git add -N`(intent-to-add)让它们进 diff:
```bash
cd vendor/code/codex-rs
git add -N core/src/chat_completions.rs core/src/model_family.rs core/src/openai_tools.rs
git diff > ../../../patches/codex-rs-third-party-models.patch
git checkout -- . && git reset HEAD core/src/chat_completions.rs core/src/model_family.rs core/src/openai_tools.rs 2>/dev/null
# 清理:撤销 intent-to-add,删掉工作区新文件(apply-patches 会重新创建)
rm -f core/src/chat_completions.rs core/src/model_family.rs core/src/openai_tools.rs
cd ../..
```

- [ ] **Step 11: 验证完整 patch 能干净 apply + 重新生成文件**

Run:
```bash
scripts/apply-patches.sh
ls vendor/code/codex-rs/core/src/chat_completions.rs && echo "chat_completions.rs created"
ls vendor/code/codex-rs/core/src/model_family.rs && echo "model_family.rs created"
ls vendor/code/codex-rs/core/src/openai_tools.rs && echo "openai_tools.rs created"
cd vendor/code/codex-rs && cargo check -p codex-core 2>&1 | tail -5
cd ../..
```
Expected: 三个文件创建;codex-core 编译通过。

- [ ] **Step 12: Commit patch**

Run:
```bash
git add patches/codex-rs-third-party-models.patch
git commit -m "feat(patch): port chat_completions + model_family + openai_tools + client.rs Chat arm (Parts 2-5)"
```

---

### Task 1.5:端到端验证 —— wire_api=chat 跑通一个三方模型

**Files:**
- Create: `tests/third-party-model.md`(测试用配置说明,非代码)

> **这是子项目 1 的完成标准。** 验证 patch 后能用 Chat 协议跑通一个三方模型(如 OpenRouter)。

- [ ] **Step 1: 写一个三方模型 provider 配置**

准备一个 codex config(如 `~/.codex/config.toml` 或临时),配 OpenRouter(或任何 OpenAI 兼容 Chat API):
```toml
# 示例:OpenRouter(需 OPENROUTER_API_KEY 环境变量)
[model_providers.openrouter]
name = "openrouter"
base_url = "https://openrouter.ai/api/v1"
env_key = "OPENROUTER_API_KEY"
wire_api = "chat"    # ← 关键:patch 后此值不再报错

[model]
name = "anthropic/claude-3.5-sonnet"   # OpenRouter 上的模型
provider = "openrouter"
```

- [ ] **Step 2: 应用 patch + 跑 lemurclaw TUI 对话**

Run:
```bash
scripts/apply-patches.sh
cargo run -p lemurclaw -- --frontend tui
# 在 TUI 里输入一句话,如"say hi",确认收到回复
```
Expected: TUI 启动,发 prompt 收到流式回复。若报 `wire_api = "chat"` 错误 → patch 未生效(回查 apply);若报网络/auth 错 → provider 配置问题;若收到回复 → ✅ 三方模型跑通。

> **替代(无 API key 时):** 用本地 ollama(`base_url = "http://localhost:11434/v1"`, `wire_api="chat"`)测,无需外部 key。

- [ ] **Step 3: 记录验证结果到 patches/README.md**

在 `patches/README.md` 末尾加:
```markdown
## 验证

- 锚点 commit 下,patch 应用后 `cargo check -p codex-core`:通过(执行时确认)
- 三方模型端到端:用 <OpenRouter/ollama/...> + `wire_api=chat` 跑通一轮对话(执行时确认)
- 已知适配点(chat_completions 移植时 codex-rs vs code-rs 的差异):
  - <执行时填写:如 ModelProviderInfo 路径改 codex_model_provider_info::...>
  - <执行时填写:其他>
```

- [ ] **Step 4: Commit README 更新**

Run:
```bash
git add patches/README.md
git commit -m "docs(patch): record third-party model end-to-end verification"
```

---

### Task 1.6:验证上游 codex 测试不被 patch 破坏

**Files:**
- (无新文件;跑 codex 上游测试)

- [ ] **Step 1: 应用 patch 后跑 codex-core 单元测试**

Run:
```bash
scripts/apply-patches.sh
cd vendor/code/codex-rs
cargo test -p codex-core --lib 2>&1 | tail -15
cd ../..
```
Expected: 上游测试全通过(patch 改的是加 Chat 支持,不应破坏 Responses 路径的测试)。若有失败,排查是否 patch 误改了别的。

> **时间预期:** codex-core 很大,编译+测试可能数十分钟。可限 `--lib` 或 `cargo test -p codex-core --lib model_provider` 缩小范围。

- [ ] **Step 2: 若有测试失败,排查**

可见性/分发改动理论上不破坏 Responses 路径。若失败,很可能是 chat_completions 移植引入的副作用(如 lib.rs mod 声明影响别的)——回查 `git diff` in submodule 确认只动了目标。

- [ ] **Step 3: 记录到 patches/README.md**

在 patches/README.md 验证段补:
```markdown
- codex 上游测试 `cargo test -p codex-core --lib`:<通过/结果摘要,执行时填>
```

- [ ] **Step 4: Commit**

Run:
```bash
git add patches/README.md
git commit -m "docs(patch): record codex-core upstream test results"
```

---

## 子项目 1 完成标准

- [ ] `patches/codex-rs-third-party-models.patch` 含完整三方模型支持(WireApi 变体 + chat_completions.rs + model_family.rs + openai_tools.rs 新文件 + client.rs Chat arm + lib.rs mod 声明)
- [ ] `scripts/apply-patches.sh` 幂等(已 apply 跳过)
- [ ] `patches/README.md` 记录锚点 commit、改动点、rebase 流程、验证结果、已知适配点
- [ ] `cargo check -p codex-core`(patch 后)通过
- [ ] **`wire_api=chat` 配置跑通一个三方模型(OpenRouter/ollama)的完整对话**(端到端)
- [ ] `cargo test -p codex-core --lib`(上游)不被 patch 破坏

---

## 后续(不在本计划范围)

- **子项目 2:** GUI 基础设施 —— 填充 `lemurclaw-gui`(wry+tao + AppServerClient + ipc shim + tao proxy)+ React 骨架(assets/)+ Transport TS 接口。
- **子项目 3+:** 核心对话循环、导航、配置模态、webui、发布。
- 各自独立 spec → plan → 实现循环。
