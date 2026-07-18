# lemurclaw 骨架 + 四环 Patch 实现计划(子项目 0+1)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 搭建 lemurclaw Cargo workspace 骨架(5 crate + submodule + patcher),让 `--frontend tui` 跑通上游 grok-build,并落地四环可见性 patch(dispatch/execute/dispatch_task_result/acp_handler)+ webview IoC seam,使 Rust 侧能复用上游完整反馈循环。

**Architecture:** lemurclaw 独立 workspace(members 只列 `crates/*`,绝不包含 `vendor/`);`vendor/grok-build` 作为 git submodule 是它自己独立的 80-member workspace;pager 通过 path dep 引入时 cargo 向上找到上游根,在上游 workspace 上下文构建(规避"双根抢成员")。四环 patch 以 `patches/grok-build.patch` 形式 apply 到 submodule。

**Tech Stack:** Rust(edition 2021;上游 edition 2024,作为外部依赖不强制对齐)、cargo workspace、git submodule、git apply patch。上游 crate:`xai-grok-pager`、`xai-grok-shell`、`xai-grok-config`。

**Spec:** `docs/superpowers/specs/2026-07-17-lemurclaw-grok-build-gui-design.md`(子项目 0 + 1 + 章节 3 patch 契约)

---

## 范围说明

本计划**只覆盖子项目 0(骨架)和子项目 1(四环 patch)**。不涉及 GUI 前端(wry/tao/React)、不涉及 view-model 投影的具体字段实现、不涉及 ACP 连接的实际驱动(那些是子项目 2+)。子项目 1 的完成标准是:**patch 能 apply、cargo 能编译、四环函数从 lemurclaw crate 可见、一个最小单元测试调通 `dispatch`**。

## 关键约束(来自 spec 验证)

1. **workspace members 绝不含 `vendor/`** —— 否则 cargo 把上游 pager 纳入 lemurclaw workspace,导致上游 71 处 `workspace = true` 无法解析。
2. **四环 patch 目标的当前可见性(已核实)**:
   - `app/dispatch/mod.rs:52` → `pub(crate) use router::dispatch;`
   - `app/effects/mod.rs:34` → `pub(crate) fn execute(...)`
   - `app/dispatch/task_result.rs:172` → `pub(super) fn dispatch_task_result(...)`
   - `app/acp_handler/mod.rs:137` → `pub(crate) fn handle(...)`
3. **已 pub 无需 patch**:`AcpConnection{tx,rx}`、`connect`、`connect_via_leader`、`app::run(args, None)`、`PagerArgs`、`Action`/`Effect`/`TaskResult` 类型。
4. **submodule 锁定上游 main 当前 HEAD**(实现时取最新 commit,记入 `patches/README.md`)。

## 文件结构

本计划创建/修改的文件:

| 文件 | 责任 | 创建/修改 |
|---|---|---|
| `.gitmodules` | 登记上游 submodule | 修改(子项目 0) |
| `vendor/grok-build/` | 上游 submodule(@ 固定 commit) | 新增(子项目 0) |
| `Cargo.toml` | lemurclaw workspace root,members 只列 `crates/*` | 新增(子项目 0) |
| `crates/lemurclaw-config/Cargo.toml` | config crate 清单 | 新增(子项目 0) |
| `crates/lemurclaw-config/src/lib.rs` | RuntimeConfig + Frontend enum + CLI 解析骨架 | 新增(子项目 0) |
| `crates/lemurclaw-runtime/Cargo.toml` | runtime crate 清单 | 新增(子项目 0) |
| `crates/lemurclaw-runtime/src/lib.rs` | `run(config)` 入口,Tui/Gui 分流(此计划仅 Tui 桩) | 新增(子项目 0) |
| `crates/lemurclaw-gui-bridge/Cargo.toml` | gui-bridge crate 清单 | 新增(子项目 0,空壳) |
| `crates/lemurclaw-gui-bridge/src/lib.rs` | 占位(子项目 2 填充) | 新增(子项目 0) |
| `crates/lemurclaw-bin/Cargo.toml` | bin crate 清单 | 新增(子项目 0) |
| `crates/lemurclaw-bin/src/main.rs` | composition root:解析 CLI → 调 runtime::run | 新增(子项目 0) |
| `scripts/apply-patches.sh` | 对 submodule 应用 patch,幂等 | 新增(子项目 1) |
| `patches/grok-build.patch` | 四环可见性 + webview_hook patch | 新增(子项目 1) |
| `patches/README.md` | 锚点 commit + patch 意图 + rebase 指南 | 新增(子项目 1) |
| `crates/lemurclaw-runtime/tests/dispatch_smoke.rs` | 集成测试:调通 patch 后的 dispatch | 新增(子项目 1) |
| `README.md` | 项目说明(简短) | 修改(子项目 0) |

> **`lemurclaw-gui-frontend` 在此计划不创建** —— 它是子项目 2 的内容(React/TS 源码)。此计划只让 gui-bridge 成为空壳 crate,保证 workspace 编译通过。

---

## 子项目 0:基础骨架

### Task 0.1:添加 grok-build submodule 并锁定 commit

**Files:**
- Create: `.gitmodules`(由 `git submodule add` 自动生成)
- Create: `vendor/grok-build/`(submodule checkout)

- [ ] **Step 1: 添加上游为 submodule**

Run:
```bash
git submodule add https://github.com/xai-org/grok-build.git vendor/grok-build
```
Expected: `vendor/grok-build/` 检出 main HEAD,`.gitmodules` 生成,git status 显示新文件。

- [ ] **Step 2: 记录锁定的 commit SHA**

Run:
```bash
git -C vendor/grok-build rev-parse HEAD
```
Expected: 输出一个 40 字符 SHA(记下来,Task 1.x 的 `patches/README.md` 要用)。

- [ ] **Step 3: 验证 submodule 是独立 workspace**

Run:
```bash
test -f vendor/grok-build/Cargo.toml && grep -q "\[workspace\]" vendor/grok-build/Cargo.toml && echo "OK: upstream is own workspace"
```
Expected: `OK: upstream is own workspace`

- [ ] **Step 4: Commit**

Run:
```bash
git add .gitmodules vendor/grok-build
git commit -m "chore: add xai-org/grok-build as submodule at vendor/"
```

---

### Task 0.2:创建 lemurclaw workspace root

**Files:**
- Create: `Cargo.toml`(workspace root)

- [ ] **Step 1: 写 workspace root Cargo.toml**

Create `Cargo.toml`:
```toml
# lemurclaw workspace root
# CRITICAL: members 只列 crates/*。绝不能包含 vendor/,否则 cargo 会把
# vendor/grok-build 的上游 pager crate 纳入本 workspace,导致上游 crate 内
# 71 处 `workspace = true` 无法解析("双根抢成员"错误)。
[workspace]
resolver = "2"
members = [
    "crates/lemurclaw-config",
    "crates/lemurclaw-runtime",
    "crates/lemurclaw-gui-bridge",
    "crates/lemurclaw-bin",
]

[workspace.package]
edition = "2021"
license = "MIT"
version = "0.0.1"

# 上游 crate 通过 path dep 引用。cargo 向上查找 workspace 根时会停在
# vendor/grok-build/Cargo.toml(那是 pager 所属的 workspace),从而在
# 上游 workspace 上下文构建 pager,正确解析其 `workspace = true` 依赖。
[workspace.dependencies]
xai-grok-pager = { path = "vendor/grok-build/crates/codegen/xai-grok-pager" }
xai-grok-shell = { path = "vendor/grok-build/crates/codegen/xai-grok-shell" }
xai-grok-config = { path = "vendor/grok-build/crates/codegen/xai-grok-config" }
tokio = { version = "1", features = ["full"] }
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }
```

- [ ] **Step 2: 验证 workspace root 可识别(此时 members 还不存在,会报错——预期)**

Run:
```bash
cargo metadata --no-deps --format-version 1 > /dev/null 2>&1; echo "exit=$?"
```
Expected: 非 0 退出码,报错指向 members 目录不存在(正常,后续 task 创建它们)。

- [ ] **Step 3: Commit**

Run:
```bash
git add Cargo.toml
git commit -m "chore: add lemurclaw workspace root (members exclude vendor/)"
```

---

### Task 0.3:创建 lemurclaw-config crate

**Files:**
- Create: `crates/lemurclaw-config/Cargo.toml`
- Create: `crates/lemurclaw-config/src/lib.rs`

- [ ] **Step 1: 写 Cargo.toml**

Create `crates/lemurclaw-config/Cargo.toml`:
```toml
[package]
name = "lemurclaw-config"
edition.workspace = true
license.workspace = true
version.workspace = true

[dependencies]
clap = { workspace = true }
serde = { workspace = true }
```

- [ ] **Step 2: 写失败的测试(Frontend enum 解析)**

Create `crates/lemurclaw-config/src/lib.rs`:
```rust
//! lemurclaw 配置层:RuntimeConfig + Frontend 选择 + CLI 解析。

use serde::{Deserialize, Serialize};

/// 前端选择:TUI(复用上游)或 GUI(自建 React/wry)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Frontend {
    #[default]
    Tui,
    Gui,
}

/// runtime 启动配置。其他项目嵌入时构造此结构传给 `lemurclaw_runtime::run`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub agent_name: String,
    pub cwd: Option<std::path::PathBuf>,
    pub model: Option<String>,
    pub frontend: Frontend,
    pub yolo: bool,
    pub permission_mode: Option<String>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            agent_name: "lemurclaw".to_string(),
            cwd: None,
            model: None,
            frontend: Frontend::default(),
            yolo: false,
            permission_mode: None,
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
        let gui = serde_json::to_string(&Frontend::Gui).unwrap();
        assert_eq!(gui, "\"gui\"");
        let parsed: Frontend = serde_json::from_str("\"tui\"").unwrap();
        assert_eq!(parsed, Frontend::Tui);
    }

    #[test]
    fn runtime_config_default_agent_name() {
        let cfg = RuntimeConfig::default();
        assert_eq!(cfg.agent_name, "lemurclaw");
        assert_eq!(cfg.frontend, Frontend::Tui);
    }
}
```

- [ ] **Step 3: 运行测试验证通过**

Run:
```bash
cargo test -p lemurclaw-config
```
Expected: 3 tests passed。

- [ ] **Step 4: Commit**

Run:
```bash
git add crates/lemurclaw-config
git commit -m "feat(config): add lemurclaw-config crate (RuntimeConfig + Frontend)"
```

---

### Task 0.4:创建 lemurclaw-runtime crate(Tui 桩 + Gui 未实现)

**Files:**
- Create: `crates/lemurclaw-runtime/Cargo.toml`
- Create: `crates/lemurclaw-runtime/src/lib.rs`

- [ ] **Step 1: 写 Cargo.toml**

Create `crates/lemurclaw-runtime/Cargo.toml`:
```toml
[package]
name = "lemurclaw-runtime"
edition.workspace = true
license.workspace = true
version.workspace = true

[dependencies]
lemurclaw-config = { path = "../lemurclaw-config" }
# 上游 pager:子项目 1 apply patch 后才能完整编译;此处先声明,编译验证留到 Task 1.x
xai-grok-pager = { workspace = true }
tokio = { workspace = true }
anyhow = { workspace = true }
```

- [ ] **Step 2: 写 lib.rs(Tui 转调上游,Gui 返回未实现错误)**

Create `crates/lemurclaw-runtime/src/lib.rs`:
```rust
//! lemurclaw agent runtime:可被其他项目嵌入的入口。
//!
//! 根据 `RuntimeConfig.frontend` 分流:
//! - Tui:转调上游 `xai_grok_pager::app::run`(复用上游,零改动)
//! - Gui:子项目 2+ 实现,此版本返回未实现错误

use lemurclaw_config::{Frontend, RuntimeConfig};

/// runtime 返回的退出码包装。
pub struct ExitCode(pub i32);

/// 启动 runtime。embedding 用例:
/// ```no_run
/// # use lemurclaw_config::{Frontend, RuntimeConfig};
/// # async fn run() -> Result<(), anyhow::Error> {
/// lemurclaw_runtime::run(RuntimeConfig {
///     frontend: Frontend::Tui,
///     ..Default::default()
/// }).await?;
/// # Ok(())
/// # }
/// ```
pub async fn run(config: RuntimeConfig) -> anyhow::Result<ExitCode> {
    match config.frontend {
        Frontend::Tui => run_tui(config).await,
        Frontend::Gui => Err(anyhow::anyhow!(
            "GUI frontend not implemented yet (subproject 2+); use --frontend tui"
        )),
    }
}

async fn run_tui(config: RuntimeConfig) -> anyhow::Result<ExitCode> {
    // 构造上游 PagerArgs:此版本传最小参数,真实参数映射在后续子项目完善。
    // 上游 app::run 签名:pub async fn run(args: PagerArgs, bg_update_rx: Option<...>) -> Result<bool>
    // PagerArgs 是 clap 派生类型,这里用 try_parse_from 从 argv 构造。
    let mut argv: Vec<std::ffi::OsString> = vec!["lemurclaw".into()];
    if let Some(cwd) = &config.cwd {
        argv.push("--cwd".into());
        argv.push(cwd.as_os_str().to_os_string());
    }
    if let Some(model) = &config.model {
        argv.push("--model".into());
        argv.push(model.clone().into());
    }
    if config.yolo {
        argv.push("--yolo".into());
    }
    let args = xai_grok_pager::app::PagerArgs::try_parse_from(argv)
        .map_err(|e| anyhow::anyhow!("failed to parse PagerArgs: {e}"))?;
    let ok = xai_grok_pager::app::run(args, None).await?;
    Ok(ExitCode(if ok { 0 } else { 1 }))
}

// 引入 clap::Parser trait 以便 try_parse_from 可用(上游 PagerArgs 实现了 Parser)。
use clap::Parser;
```

- [ ] **Step 3: 尝试编译(预期失败:patch 未 apply 前 pager 可能可编译,也可能因某些 transitive 问题失败——记录现象)**

Run:
```bash
cargo check -p lemurclaw-runtime 2>&1 | tail -20
```
Expected: 可能成功(pager 本身完整),也可能因上游 edition 2024 / 某些依赖问题失败。**记录输出**:
- 若成功 → 继续 Step 4
- 若失败 → 记录错误,这是预期的"骨架先就位,编译验证留到 patch 后"。不阻塞,进入 Step 4。

> **说明:** Task 0.4 的目标是 crate 结构就位,不是编译通过。完整编译验证在子项目 1 patch apply 后(Task 1.5)。PagerArgs 的真实字段(`--cwd`/`--model`/`--yolo` 是否存在)以 Task 1.5 验证为准,若不存在则在此 task 修正 argv 构造。

- [ ] **Step 4: Commit(即使编译未通过,结构已就位)**

Run:
```bash
git add crates/lemurclaw-runtime
git commit -m "feat(runtime): add lemurclaw-runtime crate (Tui pass-through to upstream, Gui stub)"
```

---

### Task 0.5:创建 lemurclaw-gui-bridge 空壳 crate

**Files:**
- Create: `crates/lemurclaw-gui-bridge/Cargo.toml`
- Create: `crates/lemurclaw-gui-bridge/src/lib.rs`

- [ ] **Step 1: 写 Cargo.toml(最小依赖,子项目 2 再加 wry/tao)**

Create `crates/lemurclaw-gui-bridge/Cargo.toml`:
```toml
[package]
name = "lemurclaw-gui-bridge"
edition.workspace = true
license.workspace = true
version.workspace = true

[dependencies]
lemurclaw-config = { path = "../lemurclaw-config" }
anyhow = { workspace = true }
```

- [ ] **Step 2: 写占位 lib.rs**

Create `crates/lemurclaw-gui-bridge/src/lib.rs`:
```rust
//! lemurclaw GUI 桥接层(Rust 侧)。
//!
//! 子项目 2 填充:wry+tao 集成、ipc_handler、四环驱动 AppView、
//! project_view 投影、tao proxy 推 ViewModel。此版本为空壳,保证 workspace 编译。
```

- [ ] **Step 3: 验证编译**

Run:
```bash
cargo check -p lemurclaw-gui-bridge
```
Expected: 编译通过(无依赖问题)。

- [ ] **Step 4: Commit**

Run:
```bash
git add crates/lemurclaw-gui-bridge
git commit -m "feat(gui-bridge): add empty lemurclaw-gui-bridge crate (filled in subproject 2)"
```

---

### Task 0.6:创建 lemurclaw-bin composition root

**Files:**
- Create: `crates/lemurclaw-bin/Cargo.toml`
- Create: `crates/lemurclaw-bin/src/main.rs`

- [ ] **Step 1: 写 Cargo.toml**

Create `crates/lemurclaw-bin/Cargo.toml`:
```toml
[package]
name = "lemurclaw-bin"
edition.workspace = true
license.workspace = true
version.workspace = true

[[bin]]
name = "lemurclaw"
path = "src/main.rs"

[dependencies]
lemurclaw-config = { path = "../lemurclaw-config" }
lemurclaw-runtime = { path = "../lemurclaw-runtime" }
clap = { workspace = true }
tokio = { workspace = true }
anyhow = { workspace = true }
```

- [ ] **Step 2: 写 main.rs**

Create `crates/lemurclaw-bin/src/main.rs`:
```rust
//! lemurclaw composition root:解析 CLI → 构造 RuntimeConfig → 调 runtime::run。

use clap::Parser;
use lemurclaw_config::{Frontend, RuntimeConfig};

/// lemurclaw:基于 grok-build 的可复用 agent runtime + 等价 TUI 的 GUI。
#[derive(Parser, Debug)]
#[command(name = "lemurclaw", version, about)]
struct Cli {
    /// 前端选择:tui(复用上游)或 gui(自建,子项目 2+)
    #[arg(long, value_enum, default_value_t = Frontend::Tui)]
    frontend: Frontend,

    /// agent 名称
    #[arg(long, default_value = "lemurclaw")]
    agent_name: String,

    /// 工作目录
    #[arg(long)]
    cwd: Option<std::path::PathBuf>,

    /// 模型
    #[arg(long)]
    model: Option<String>,

    /// yolo 模式(跳过权限确认)
    #[arg(long, default_value_t = false)]
    yolo: bool,
}

impl From<Cli> for RuntimeConfig {
    fn from(cli: Cli) -> Self {
        RuntimeConfig {
            agent_name: cli.agent_name,
            cwd: cli.cwd,
            model: cli.model,
            frontend: cli.frontend,
            yolo: cli.yolo,
            permission_mode: None,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config: RuntimeConfig = cli.into();
    let exit = lemurclaw_runtime::run(config).await?;
    std::process::exit(exit.0);
}
```

> **注:** `Frontend` 需派生 `clap::ValueEnum` 才能作 `value_enum`。Task 0.3 的 `Frontend` 定义未加 `ValueEnum`。**下一步先回去补**。

- [ ] **Step 3: 给 lemurclaw-config 的 Frontend 加 ValueEnum 派生**

Modify `crates/lemurclaw-config/Cargo.toml` 的 `[dependencies]`:
```toml
clap = { workspace = true, features = ["derive"] }
```

Modify `crates/lemurclaw-config/src/lib.rs` 的 `Frontend` enum:
```rust
use clap::ValueEnum;

/// 前端选择:TUI(复用上游)或 GUI(自建 React/wry)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, ValueEnum)]
#[serde(rename_all = "lowercase")]
#[value(rename_all = "lowercase")]
pub enum Frontend {
    #[default]
    Tui,
    Gui,
}
```

- [ ] **Step 4: 验证 bin 编译(此阶段 runtime 可能仍未通过——看 Task 0.4 现象)**

Run:
```bash
cargo check -p lemurclaw-bin 2>&1 | tail -20
```
Expected: 若 Task 0.4 的 runtime 通过则 bin 也通过;否则同样记录现象,不阻塞。

- [ ] **Step 5: Commit**

Run:
```bash
git add crates/lemurclaw-bin crates/lemurclaw-config
git commit -m "feat(bin): add lemurclaw-bin composition root; Frontend derives ValueEnum"
```

---

### Task 0.7:更新 README + 验证 workspace 元数据

**Files:**
- Modify: `README.md`

- [ ] **Step 1: 写 README**

Modify `README.md`(替换原有的 `# lemurclaw` 单行):
```markdown
# lemurclaw

基于 [xai-org/grok-build](https://github.com/xai-org/grok-build) 的可复用 agent runtime crate + 等价 TUI 的 GUI。

## 状态

搭建中(子项目 0+1:骨架 + 四环 patch)。

## 架构

- **TUI 前端**:复用上游 `xai-grok-pager`,零改动。
- **GUI 前端**(进行中):React + wry webview,进程内 IPC。
- **runtime crate**:`lemurclaw-runtime`,其他项目可 `cargo add` 嵌入。

详见 `docs/superpowers/specs/2026-07-17-lemurclaw-grok-build-gui-design.md`。

## 构建

```bash
git submodule update --init          # 拉取 vendor/grok-build
scripts/apply-patches.sh             # 应用四环 patch(子项目 1)
cargo build -p lemurclaw-bin
```
```

- [ ] **Step 2: 验证 workspace 元数据完整**

Run:
```bash
cargo metadata --no-deps --format-version 1 > /dev/null && echo "OK: workspace metadata valid"
```
Expected: 若 Task 0.4 runtime 编译通过,此处输出 `OK`;否则记录错误(留待 Task 1.5 修复)。

- [ ] **Step 3: Commit**

Run:
```bash
git add README.md
git commit -m "docs: update README with architecture overview and build steps"
```

---

## 子项目 0 完成标准

- [ ] 5 个 crate 结构就位(config/runtime/gui-bridge/bin)
- [ ] workspace root Cargo.toml 的 members 只列 `crates/*`,不含 `vendor/`
- [ ] submodule 在 `vendor/grok-build/` 且是独立 workspace
- [ ] `--frontend gui` 返回明确的"未实现"错误(不 panic)
- [ ] 若 runtime 编译通过,`cargo metadata` 成功;若未通过,错误已记录(子项目 1 修复)

---

## 子项目 1:四环 Patch + dispatch 冒烟测试

### Task 1.1:编写 apply-patches 脚本

**Files:**
- Create: `scripts/apply-patches.sh`

- [ ] **Step 1: 写脚本(幂等:已 apply 则跳过)**

Create `scripts/apply-patches.sh`:
```bash
#!/usr/bin/env bash
# 对 vendor/grok-build/ 应用 lemurclaw 维护的 patch。
# 幂等:若 patch 已应用(git apply --check 失败但代码已是目标状态),跳过。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
VENDOR="$ROOT/vendor/grok-build"
PATCH_DIR="$ROOT/patches"

if [ ! -d "$VENDOR/.git" ]; then
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
  else
    # 检查是否已是目标状态(已 apply 过):reverse 能 check 说明正向已应用
    if git apply --check --reverse "$p" 2>/dev/null; then
      echo "$name already applied; skipping."
    else
      echo "ERROR: $name does not apply cleanly (neither forward nor reverse)." >&2
      echo "  Likely upstream drifted. Rebase the patch." >&2
      exit 1
    fi
  fi
done

echo "All patches applied."
```

- [ ] **Step 2: 赋可执行权限**

Run:
```bash
chmod +x scripts/apply-patches.sh
```

- [ ] **Step 3: 验证脚本可运行(此时 patches/ 为空,应输出 nothing to apply)**

Run:
```bash
scripts/apply-patches.sh
```
Expected: `No patches in patches/; nothing to apply.`

- [ ] **Step 4: Commit**

Run:
```bash
git add scripts/apply-patches.sh
git commit -m "chore: add idempotent apply-patches.sh for submodule patches"
```

---

### Task 1.2:生成四环可见性 patch

**Files:**
- Create: `patches/grok-build.patch`
- Create: `patches/README.md`

- [ ] **Step 1: 在 submodule 工作区做八处可见性修改**

> **关键:** 共 **8 处**,分两类。
>
> **A. 模块可见性(3 处,`app/mod.rs`):** `dispatch`/`effects`/`acp_handler` 三个模块当前是私有 `mod`(已核实 `app/mod.rs:24/26/29`)。即使把函数改 pub,外部 crate 访问 `xai_grok_pager::app::dispatch::dispatch` 仍被模块私有挡住。必须先暴露模块。
>
> **B. 函数/子模块/重导出可见性(5 处):** dispatch re-export、task_result 子模块、execute、dispatch_task_result、handle。

Run(在 submodule 内直接编辑,稍后用 git diff 导出 patch):
```bash
cd vendor/grok-build
PAGER=crates/codegen/xai-grok-pager/src

# === A. 模块可见性(app/mod.rs)3 处 ===

# 修改 1: app/mod.rs:24  mod acp_handler;  →  pub mod acp_handler;
sed -i.bak 's/^mod acp_handler;$/pub mod acp_handler;/' $PAGER/app/mod.rs

# 修改 2: app/mod.rs:26  mod dispatch;  →  pub mod dispatch;
sed -i.bak 's/^mod dispatch;$/pub mod dispatch;/' $PAGER/app/mod.rs

# 修改 3: app/mod.rs:29  mod effects;  →  pub mod effects;
sed -i.bak 's/^mod effects;$/pub mod effects;/' $PAGER/app/mod.rs

# === B. 函数/子模块/re-export 可见性 5 处 ===

# 修改 4: dispatch/mod.rs:52  pub(crate) use router::dispatch;  →  pub use
sed -i.bak 's/^pub(crate) use router::dispatch;$/pub use router::dispatch;/' $PAGER/app/dispatch/mod.rs

# 修改 5: dispatch/mod.rs:34  mod task_result;  →  pub mod task_result;
sed -i.bak 's/^mod task_result;$/pub mod task_result;/' $PAGER/app/dispatch/mod.rs

# 修改 6: effects/mod.rs:34  pub(crate) fn execute  →  pub fn
sed -i.bak 's/^pub(crate) fn execute(/pub fn execute(/' $PAGER/app/effects/mod.rs

# 修改 7: task_result.rs:172  pub(super) fn dispatch_task_result  →  pub fn
sed -i.bak 's/^pub(super) fn dispatch_task_result(/pub fn dispatch_task_result(/' \
  $PAGER/app/dispatch/task_result.rs

# 修改 8: acp_handler/mod.rs:137  pub(crate) fn handle  →  pub fn
sed -i.bak 's/^pub(crate) fn handle(/pub fn handle(/' $PAGER/app/acp_handler/mod.rs

# 清理 sed 备份
find $PAGER -name '*.bak' -delete
```

- [ ] **Step 2: 验证八处修改都命中(app/mod.rs 3 处 + dispatch/mod.rs 2 处 + 其余各 1 处,共 4 文件 8 行)**

Run:
```bash
cd vendor/grok-build
git diff --stat
```
Expected: 4 个文件;`app/mod.rs` 改动 3 行,`dispatch/mod.rs` 改动 2 行,`effects/mod.rs`/`task_result.rs`/`acp_handler/mod.rs` 各 1 行。总计 8 行 `+8 -8`。

- [ ] **Step 3: 若某处未命中(grep 0 行),手动核实该行**

Run:
```bash
cd vendor/grok-build
grep -n 'pub mod acp_handler' $PAGER/app/mod.rs
grep -n 'pub mod dispatch' $PAGER/app/mod.rs
grep -n 'pub mod effects' $PAGER/app/mod.rs
grep -n 'pub use router::dispatch' $PAGER/app/dispatch/mod.rs
grep -n 'pub mod task_result' $PAGER/app/dispatch/mod.rs
grep -n 'pub fn execute' $PAGER/app/effects/mod.rs
grep -n 'pub fn dispatch_task_result' $PAGER/app/dispatch/task_result.rs
grep -n 'pub fn handle' $PAGER/app/acp_handler/mod.rs
```
Expected: 8 条各命中一行。若某条未命中(仍是 `pub(crate)`/`pub(super)`/`mod`),说明 sed 未匹配(可能是缩进/空格差异),手动编辑该行,记录实际文本供后续 rebase 参考。

- [ ] **Step 4: 导出 patch 到 lemurclaw 仓库**

Run:
```bash
cd vendor/grok-build
git diff > ../../patches/grok-build.patch
git checkout -- .   # 撤销 submodule 工作区修改(稍后用 apply-patches.sh 正式应用)
cd ../..
```

- [ ] **Step 5: 验证 patch 内容含 4 处修改**

Run:
```bash
grep -c '^-' patches/grok-build.patch   # 删除行数
grep -c '^+' patches/grok-build.patch   # 新增行数
grep 'pub use router::dispatch' patches/grok-build.patch
grep 'pub fn execute' patches/grok-build.patch
grep 'pub fn dispatch_task_result' patches/grok-build.patch
grep 'pub fn handle' patches/grok-build.patch
```
Expected: 删除/新增各约 4 行;四条 grep 都命中。

- [ ] **Step 6: 写 patches/README.md**

Create `patches/README.md`:
```markdown
# grok-build patches

lemurclaw 维护的对上游 `xai-org/grok-build` 的可见性 patch。

## 锚点 commit

- 上游仓库:`https://github.com/xai-org/grok-build.git`
- 锁定 commit:`<填入 Task 0.1 Step 2 记录的 SHA>`
- 锁定日期:2026-07-17

## patch 内容

`grok-build.patch` 暴露上游反馈循环四环(见 spec 章节 3),让 lemurclaw GUI crate
能复用上游完整的 dispatch→execute→task_result→acp_handler 循环,保证 TUI/GUI 行为等效。
共 8 处可见性改动 + 1 个新文件(webview_hook)+ 1 处 mod 声明:

**A. 模块可见性(app/mod.rs):**
1. `app/mod.rs:24` `mod acp_handler;` → `pub mod acp_handler;`
2. `app/mod.rs:26` `mod dispatch;` → `pub mod dispatch;`
3. `app/mod.rs:29` `mod effects;` → `pub mod effects;`

**B. 函数/子模块/re-export:**
4. `app/dispatch/mod.rs:52` `pub(crate) use router::dispatch;` → `pub use`
5. `app/dispatch/mod.rs:34` `mod task_result;` → `pub mod task_result;`(否则函数 pub 也访问不到)
6. `app/effects/mod.rs:34` `pub(crate) fn execute(...)` → `pub fn`
7. `app/dispatch/task_result.rs:172` `pub(super) fn dispatch_task_result(...)` → `pub fn`
8. `app/acp_handler/mod.rs:137` `pub(crate) fn handle(...)` → `pub fn`

**C. 新增(子项目 1 Task 1.4):**
9. 新文件 `app/webview_hook.rs`(镜像 `minimal/hook.rs`,不带 PagerTerminal)
10. `app/mod.rs` 加 `pub mod webview_hook;`

## 设计意图

顺上游 `minimal` 前端先例(`minimal_api` + `minimal_hook`),为 webview GUI 增加
第三个前端。不修改 `MvpAgent`、渲染层(`views/*`/`scrollback/*`)、`event_loop::run`、
上游根 Cargo.toml。

## 应用

```bash
scripts/apply-patches.sh   # 幂等
```

## rebase 流程(上游 bump 时)

```bash
git -C vendor/grok-build checkout <new-commit>
scripts/apply-patches.sh   # 若失败:手动调整 patches/grok-build.patch
cargo test                 # 验证 lemurclaw 与上游原测试都通过
git add vendor/grok-build patches/
git commit -m "chore: bump grok-build to <new-commit>; rebase patch"
```

## 风险

`effects::execute` 签名(含 `JoinSet`/`AcpAgentTx`/`SessionFlags`/`progress_tx`)是四环中
最易变的,rebase 冲突最高频。冲突时优先适配签名而非重写。
```

> **注:** Task 1.2 Step 6 的 `<填入...>` 占位符是**执行时填**的具体 SHA,不是计划的占位符——执行者从 Task 0.1 Step 2 的记录里取实际值替换。这是允许的(具体运行时数据,非设计 TBD)。

- [ ] **Step 7: Commit patch 文件**

Run:
```bash
git add patches/
git commit -m "feat(patch): add 4-loop visibility patch for grok-build (dispatch/execute/task_result/acp_handler)"
```

---

### Task 1.3:应用 patch 并验证可见性

**Files:**
- (无新文件;验证已 apply 的 submodule)

- [ ] **Step 1: 应用 patch**

Run:
```bash
scripts/apply-patches.sh
```
Expected: `Applying grok-build.patch ... applied. All patches applied.`

- [ ] **Step 2: 再次运行验证幂等**

Run:
```bash
scripts/apply-patches.sh
```
Expected: `grok-build.patch already applied; skipping. All patches applied.`

- [ ] **Step 3: 验证八处现在是 pub**

Run:
```bash
PAGER=vendor/grok-build/crates/codegen/xai-grok-pager/src
grep -n '^pub mod acp_handler' $PAGER/app/mod.rs
grep -n '^pub mod dispatch' $PAGER/app/mod.rs
grep -n '^pub mod effects' $PAGER/app/mod.rs
grep -n '^pub use router::dispatch' $PAGER/app/dispatch/mod.rs
grep -n '^pub mod task_result' $PAGER/app/dispatch/mod.rs
grep -n '^pub fn execute' $PAGER/app/effects/mod.rs
grep -n '^pub fn dispatch_task_result' $PAGER/app/dispatch/task_result.rs
grep -n '^pub fn handle' $PAGER/app/acp_handler/mod.rs
```
Expected: 八条各命中一行,都已是 `pub`/`pub mod`/`pub use`。

- [ ] **Step 4: Commit(应用 patch 不改变 submodule commit,只是工作区改动;但需确保 .gitmodules 的 ignore 配置或记录)**

> **关键决策点:** patch apply 改的是 submodule 工作区,不是 commit。两种持久化策略:
> - (a) 让 apply-patches.sh 在每次 build 前自动运行(build.rs 或 CI 调用)
> - (b) 在 submodule 内 commit 改动(产生 lemurclaw 私有 fork commit)
>
> **本计划选 (a)**:submodule 保持干净(指向上游 commit),patch 在构建时应用。这保证 bump 上游只需改 submodule 指针 + rebase patch。无需此 task 的 git commit(patch 文件已在 Task 1.2 提交)。

- [ ] **Step 4(替代):记录策略决策到 patches/README.md(若 Task 1.2 未含)**

若 Task 1.2 Step 6 的 README 已说明 rebase 流程,跳过。否则补充一段:
```bash
# 在 patches/README.md 末尾追加:
## 持久化策略

patch 在构建时应用(apply-patches.sh),submodule 工作区保持指向上游 commit。
不提交 submodule 内的改动。CI / 本地构建前必须运行 apply-patches.sh。
```

无需 commit(README 已在 Task 1.2 提交;若补充了则 `git add patches/README.md && git commit --amend --no-edit`)。

---

### Task 1.4:添加 webview IoC seam(spec 章节 3 Part 3)

**Files:**
- (修改 submodule 工作区,导出进 patch)
- Modify: `patches/grok-build.patch`

- [ ] **Step 1: 在 submodule 创建 webview_hook.rs(镜像 minimal/hook.rs)**

Run(创建新文件):
```bash
cat > vendor/grok-build/crates/codegen/xai-grok-pager/src/app/webview_hook.rs << 'EOF'
//! Inversion-of-control seam for the optional webview (React/wry) render mode.
//!
//! 镜像 `crate::minimal::hook` 的结构,但**不带 `PagerTerminal`**——webview 后端
//! 不渲染到终端。lemurclaw-gui-bridge 在启动时调用 `install`,注册一个把
//! `AppView` 投影成 JSON(供 React 消费)的回调。
//!
//! 与 minimal_hook 一样用 fn 指针 + `OnceLock`,避免 cargo cycle
//! (lemurclaw 依赖 pager,pager 不能反向依赖 lemurclaw)。

use std::sync::OnceLock;

use crate::app::app_view::AppView;

/// 把 AppView 投影成 React 友好的 JSON(由 lemurclaw-gui-bridge 实现)。
pub type WebViewRenderFn = fn(&AppView) -> serde_json::Value;

/// lemurclaw-gui-bridge 安装的 hook 集合。
#[derive(Clone, Copy)]
pub struct WebViewHooks {
    /// 投影 AppView → JSON。由 GUI 的 gui_loop 在每次 dispatch 后调用。
    pub render: WebViewRenderFn,
}

static HOOKS: OnceLock<WebViewHooks> = OnceLock::new();

/// 安装 webview hook。幂等:首次调用生效。
pub fn install(hooks: WebViewHooks) {
    let _ = HOOKS.set(hooks);
}

/// 已安装的 hook(若 lemurclaw-gui-bridge 已接线)。
pub fn hooks() -> Option<&'static WebViewHooks> {
    HOOKS.get()
}
EOF
```

- [ ] **Step 2: 在 app/mod.rs 注册 webview_hook 模块**

Run:
```bash
# 找到 app/mod.rs 里 pub mod 声明区,加入 webview_hook
grep -n 'pub mod acp_handler' vendor/grok-build/crates/codegen/xai-grok-pager/src/app/mod.rs | head -1
```
Expected: 输出类似 `NNN: pub mod acp_handler;`。

在该行后插入(用 sed 或手动编辑):
```rust
pub mod webview_hook;
```

Run(示例 sed,需替换 NNN):
```bash
# 在 acp_handler 行后插入 webview_hook 模块声明
sed -i.bak '/^pub mod acp_handler;/a pub mod webview_hook;' \
  vendor/grok-build/crates/codegen/xai-grok-pager/src/app/mod.rs
find vendor/grok-build/crates/codegen/xai-grok-pager/src/app -name '*.bak' -delete
```

- [ ] **Step 3: 验证 pager crate 仍能编译(patch 累积了 8 处可见性 + webview_hook 新文件 + mod 声明)**

Run:
```bash
cd vendor/grok-build
cargo check -p xai-grok-pager 2>&1 | tail -15
```
Expected: 编译通过(或仅有 warning)。webview_hook 引用 `serde_json::Value`,确认 pager 已依赖 serde_json(它已依赖,见 ACP 序列化)。若报 serde_json 未声明,在 pager Cargo.toml 加 `serde_json = { workspace = true }`(记入 patch)。

- [ ] **Step 4: 把 webview_hook 改动并入 patch**

Run:
```bash
cd vendor/grok-build
git diff > ../../patches/grok-build.patch
git checkout -- .
cd ../..
```

- [ ] **Step 5: 重新 apply 验证完整 patch(8 处可见性 + webview_hook 新文件 + webview_hook mod 声明,共 ~10 处)**

Run:
```bash
scripts/apply-patches.sh
ls vendor/grok-build/crates/codegen/xai-grok-pager/src/app/webview_hook.rs && echo "webview_hook.rs exists"
grep -n 'pub mod webview_hook' vendor/grok-build/crates/codegen/xai-grok-pager/src/app/mod.rs
```
Expected: 文件存在;mod 声明命中。

- [ ] **Step 6: Commit 更新的 patch**

Run:
```bash
git add patches/grok-build.patch
git commit -m "feat(patch): add webview_hook IoC seam (mirrors minimal_hook, no PagerTerminal)"
```

---

### Task 1.5:验证 lemurclaw-runtime 完整编译

**Files:**
- (无新文件;验证 Task 0.4 的 runtime 现在能编译)

- [ ] **Step 1: 应用 patch 后编译 runtime**

Run:
```bash
scripts/apply-patches.sh
cargo check -p lemurclaw-runtime 2>&1 | tail -30
```
Expected: 编译通过。若失败,记录错误类型:
- PagerArgs 字段不对(`--cwd`/`--model`/`--yolo` 不存在)→ 修正 Task 0.4 Step 2 的 argv 构造,查 PagerArgs 实际字段
- 类型不匹配 → 核实 `app::run` 返回 `Result<bool>`,ExitCode 构造正确
- 其他 → 单独处理

- [ ] **Step 2: 若 PagerArgs 字段错误,核实实际 CLI 定义**

Run:
```bash
grep -nE '(long|short) =' vendor/grok-build/crates/codegen/xai-grok-pager/src/app/cli.rs | head -20
```
Expected: 看到实际的 `#[arg(long)]` 字段名。据此修正 Task 0.4 的 argv。

- [ ] **Step 3: 修正 runtime 后重新编译直到通过**

(根据 Step 1/2 的具体错误,迭代修正 `crates/lemurclaw-runtime/src/lib.rs`,每次 `cargo check -p lemurclaw-runtime` 验证。)

- [ ] **Step 4: Commit(若有修正)**

Run:
```bash
git add crates/lemurclaw-runtime
git commit -m "fix(runtime): align PagerArgs construction with upstream CLI fields"
```

---

### Task 1.6:dispatch 冒烟测试(子项目 1 完成标准)

**Files:**
- Create: `crates/lemurclaw-runtime/tests/dispatch_smoke.rs`

- [ ] **Step 1: 写冒烟测试(验证 patch 后的 dispatch 从外部 crate 可见且可调)**

Create `crates/lemurclaw-runtime/tests/dispatch_smoke.rs`:
```rust
//! 子项目 1 完成标准:验证 patch 后的反馈循环四环从外部 crate 可见。
//! 不驱动真实 agent(那需要 ACP 连接),只验证符号可见性——这是 patch 生效的证明。

use xai_grok_pager::app::{
    acp_handler,
    dispatch,
    effects,
    webview_hook,
};

/// 验证四环函数的可见性(patch 已把它们改 pub)。
/// 若 patch 未应用,这些 `use` 会编译失败(pub(crate) 不可跨 crate)。
#[test]
fn four_loop_symbols_are_public() {
    // 取函数指针即可证明可见性,无需调用(调用需要真实 AppView/Effect/...)。
    let _dispatch: fn(
        xai_grok_pager::app::actions::Action,
        &mut xai_grok_pager::app::app_view::AppView,
    ) -> Vec<xai_grok_pager::app::actions::Effect> = dispatch::dispatch;

    let _execute = effects::execute;
    let _handle = acp_handler::handle;

    // webview_hook 模块及其 install/hooks 公开可见
    let _install = webview_hook::install;
    let _hooks = webview_hook::hooks;
}

/// 验证 dispatch_task_result 已 pub(从 dispatch 子模块提升到 pub)。
/// 它在 dispatch::task_result 路径下,patch 把 pub(super) → pub。
#[test]
fn dispatch_task_result_is_public() {
    // dispatch_task_result 在 dispatch/task_result.rs,patch 后为 pub。
    // 通过 dispatch 模块的 task_result 子模块访问。
    let _fn = dispatch::task_result::dispatch_task_result;
}
```

> **注:** `task_result` 模块可见性已在 Task 1.2(修改 2)一并 patch(`mod task_result;` → `pub mod task_result;`),所以此测试的 `dispatch::task_result::dispatch_task_result` 访问应直接通过。

- [ ] **Step 2: 运行测试验证通过**

Run:
```bash
scripts/apply-patches.sh
cargo test -p lemurclaw-runtime --test dispatch_smoke
```
Expected: 2 tests passed。若失败提示 `task_result` 不可见,回查 Task 1.2 Step 2 的 grep 是否命中 `pub mod task_result`(可能 sed 未匹配,需手动核实 `dispatch/mod.rs:34`)。

- [ ] **Step 3: Commit 测试**

Run:
```bash
git add crates/lemurclaw-runtime/tests/dispatch_smoke.rs
git commit -m "test(runtime): dispatch smoke test proving 4-loop symbols are public post-patch"
```

---

### Task 1.7:验证上游原测试不被 patch 破坏

**Files:**
- (无新文件;跑上游测试套件)

- [ ] **Step 1: 应用 patch 后跑上游 pager 单元测试**

Run:
```bash
scripts/apply-patches.sh
cd vendor/grok-build
cargo test -p xai-grok-pager --lib 2>&1 | tail -20
cd ../..
```
Expected: 上游原测试全通过(patch 只改可见性,不改行为,不应破坏任何测试)。

> **时间预期:** 上游 pager crate 很大,编译 + 测试可能耗时数十分钟。若 CI 时间敏感,可限制为 `--lib --quiet` 或只跑 dispatch 相关:`cargo test -p xai-grok-pager --lib dispatch`。

- [ ] **Step 2: 若有测试失败,排查是否 patch 引起**

(可见性改动理论上不破坏测试。若失败,很可能是 patch 误改了别的——回查 `git diff` in submodule 确认只动了目标行。)

- [ ] **Step 3: 记录测试结果到 patches/README.md**

在 `patches/README.md` 末尾追加:
```markdown
## 验证

- 锚点 commit 下,patch 应用后上游 `cargo test -p xai-grok-pager --lib`:<通过/结果摘要,执行时填>
- lemurclaw `cargo test -p lemurclaw-runtime --test dispatch_smoke`:2 passed
```

- [ ] **Step 4: Commit README 更新**

Run:
```bash
git add patches/README.md
git commit -m "docs(patch): record upstream test verification results"
```

---

## 子项目 1 完成标准

- [ ] `patches/grok-build.patch` 含 8 处可见性(3 处 app/mod.rs 模块 + dispatch re-export + task_result 子模块 + execute + dispatch_task_result + handle)+ webview_hook 新文件 + app/mod.rs 的 webview_hook mod 声明(共 ~10 处 diff)
- [ ] `scripts/apply-patches.sh` 幂等(已 apply 时跳过)
- [ ] `patches/README.md` 记录锚点 commit、意图、rebase 流程、验证结果
- [ ] `cargo test -p lemurclaw-runtime --test dispatch_smoke` 通过(2 tests)
- [ ] `cargo test -p xai-grok-pager --lib`(上游)不被 patch 破坏
- [ ] `cargo check -p lemurclaw-runtime` 通过(runtime 能引用 patch 后的上游符号)

---

## 后续(不在本计划范围)

- **子项目 2:** GUI 基础设施——填充 `lemurclaw-gui-bridge`(wry+tao 集成、tao proxy、ipc_handler)、创建 `lemurclaw-gui-frontend`(React 骨架)、IpcMessage 信封、空 ViewModel 循环跑通。
- **子项目 3+:** view-model 投影具体字段、~28 React 组件、ACP 录放测试。
- 各自独立的 spec → plan → 实现循环。
