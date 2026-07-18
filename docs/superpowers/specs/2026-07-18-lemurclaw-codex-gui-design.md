# lemurclaw 设计 spec:基于 codex 的可复用 agent runtime + 等价 TUI 的 GUI

- **日期:** 2026-07-18
- **状态:** Draft(待用户 review)
- **作者:** Long, Wei(brainstorming 协作产出)
- **上游:** lemurclaw = openai/codex 的 fork(远程 defims/lemurclaw forked from openai/codex)。codex-rs workspace 在仓库 `codex-rs/`。基座是 codex-rs(有 Windows 沙箱 + in_process API)。just-every/code 的 code-rs 作三方模型移植参照
- **前身:** grok-build 方案(已归档于 `docs/superpowers/archive/grok-build/`,放弃原因:沙箱不支持 Windows)

## 目标

把 codex 做成一个**可被其他项目复用的 agent runtime**,并提供**等价 TUI 功能的 GUI**:
- 启动时可配置使用 **TUI 或 GUI 或 WebUI**(`--frontend tui|gui|webui`)
- **TUI 复用** codex-rs/tui,零改动
- **GUI 自建**:React 前端 + wry webview,进程内
- **WebUI**:同 React 前端,纯 web + WebSocket 连本地 server
- runtime 可配置:agent 名称、工作目录、模型、frontend 选择等
- 跨平台,尤其 **Windows 沙箱**(codex-rs 的 `windows-sandbox-rs` 原生支持)

## 关键决策(已与用户确认)

| 维度 | 决定 |
|---|---|
| 上游基座 | **lemurclaw = openai/codex 的 fork**(远程 defims/lemurclaw forked from openai/codex)。codex-rs workspace 在仓库根的 `codex-rs/`。lemurclaw crate 直接加进该 workspace |
| Windows 沙箱 / in_process API | codex-rs 原生 ✅(fork 自带 windows-sandbox-rs;这是选 codex 而非 grok-build 的根本理由) |
| 集成方式 | **fork 模式**(非 submodule/path-dep)。lemurclaw crate 作为 codex-rs workspace 新 member,用 workspace=true 引用 codex 依赖,共享 Cargo.lock/patch/lints。实测零摩擦。详见 §2 |
| 三方模型 | **直接改 fork 内的 codex-rs/core + model-provider-info**(加回 Chat:WireApi 变体 + client.rs arm + 移植 chat_completions.rs + 依赖)。非 patch,是 fork commit。参照 just-every/code 的 code-rs 实现 |
| crate 结构 | **3 Rust crate,全在 codex-rs/ 内**:`lemurclaw`(lib+bin:runtime+config)+ `lemurclaw-transport`(独立,无 wry)+ `lemurclaw-gui`(wry+tao+shim+assets/)。transport 独立让 webui 模式不背 wry/tao 依赖 |
| 前端 | TUI 复用 codex-rs/tui;GUI 自建;`--frontend tui|gui|webui` |
| GUI 默认 | wry+React 进程内;WebUI = 同前端纯 web + WebSocket |
| 等效性 | 自动成立(TUI/GUI 同源消费 ServerNotification 事件流),无 patch |
| 范围 | 完整对等 TUI(功能等价,布局自由) |
| 发布 | git 仓库 + GitHub release 预编译二进制 + `cargo install --git` + 可选 npm;**额外发空占位 crate 到 crates.io 占名**(完整功能因 codex 依赖不发 crates.io,见 §6.5) |
| TS 类型 | build 时 copy codex-rs 的 ts-rs 生成(schema/typescript/) |
| 前端构建 | `lemurclaw-gui` 的 build.rs 自动跑 `npm run build`(检测到 Node) |

> **重要更正记录(brainstorming 过程中的事实澄清):**
> 1. **~/code 是 just-every/code fork,非 openai/codex 原版。** 内部含 `codex-rs/`(上游镜像,有 Windows 沙箱)和 `code-rs/`(fork,无 Windows 沙箱但有三方模型)。
> 2. **Windows 沙箱只在 codex-rs 侧,三方模型只在 code-rs 侧,且 codex-rs 无 trait 注入点**(`ModelClient` 是具体类型,session 硬连)。要"两者都要",必须把一侧的东西搬到另一侧。
> 3. 用户选**基座 codex-rs + patch 三方模型进去**(逆 upstream,因 upstream #7782 故意删了 Chat)。
> 4. **依赖 codex 的 crate 发不了 crates.io**(实测 `cargo publish` 在打包阶段因 codex-core 不在 crates.io 索引而失败;codex 有 `[patch.crates-io] ratatui=fork` 在单 crate 失效)。发布走 git+二进制。
> 5. **集成方式转向 fork(2026-07-18):** 原"独立 workspace + submodule + path dep codex"经实测推翻(codex 127-crate workspace + alpha 依赖 rama 漂移 + cargo workspace 根约束,详见 `notes/2026-07-18-integration-blocker-findings.md`)。**转向 fork**:lemurclaw 仓库 = openai/codex fork,crate 直接加进 codex-rs workspace。这是行业对 codex 类(不发 crate 的巨型 workspace)上游的通用做法(rust-analyzer/helix/zed 类)。三方模型从"patch submodule"变为"fork 内直接改"。

---

## 设计章节 1:架构总览与数据流

### 1.1 三层模型

```
┌─────────────────────────────────────────────────────────┐
│                    lemurclaw binary                      │
│  (composition root: parse config, pick frontend, boot)   │
└────────────┬────────────────────────────┬───────────────┘
             │ --frontend=tui             │ --frontend=gui [默认]
             │                            │ --frontend=webui
             ▼                            ▼
   ┌──────────────────┐         ┌─────────────────────┐
   │  TUI 前端(复用)   │         │   GUI 前端(自建)     │
   │  codex-rs/tui    │         │  wry+React [gui]    │
   │  原封不动         │         │  纯web+WebSocket    │
   └────────┬─────────┘         └──────────┬──────────┘
            │                              │
            │   共用 AppServerClient       │
            │   ┌──────────────────┐       │
            └──►│ InProcess 变体    │◄──────┘ (gui 默认)
                │ Remote/WebSocket  │◄────── (webui)
                └────────┬─────────┘
                         │ 同一套 ClientRequest/ServerNotification 协议
                         ▼
              codex-rs app-server (in_process::start)
                         │
                         ▼
              codex-rs core (patch 后,含三方模型 Chat 支持)
                         │ + Windows 沙箱(原生)
```

### 1.2 关键原则

- **前端三选一,运行时单例。** 启动 `--frontend tui|gui|webui`(默认 gui),同一进程同一 runtime。
- **TUI 零改动:** 转调 codex-rs/tui 的入口,体验与上游一致。
- **GUI/WebUI 复用同一前端代码**:React 业务逻辑共用,只换 Transport 实现(WryIpc vs WebSocket)。
- **等效性自动成立:** TUI 和 GUI/WebUI 都是 `AppServerClient` 的消费者,消费同一份 `ServerNotification` 事件流。GUI 不是"复刻"TUI 行为,而是平行的另一个客户端。**无 patch 保证等效,无状态机复用难题。**

### 1.3 GUI 数据流(默认 wry 进程内模式)

```
React
  │ 1. 用户输入 → 构造 ClientRequest JSON (TurnStart + UserInput)
  │ 2. window.ipc.postMessage(json) → wry ipc_handler
  ▼
[Rust] lemurclaw-gui
  │ 3. serde_json 反序列化为 ClientRequest (codex_app_server_protocol 类型)
  │ 4. app_server_client.request(ClientRequest::TurnStart{...}).await
  │ 5. AppServerClient (InProcess 变体) 内部转发到 in_process handle
  ▼
codex-rs core (patch 后,三方模型可用)
  │ 6. 跑 agent turn → 流式产出事件
  │ 7. 经 in_process channel 回流
  ▼
[Rust] lemurclaw-gui 的 next_event 循环
  │ 8. app_server_client.next_event() → AppServerEvent::ServerNotification(...)
  │    - AgentMessageDelta(流式 token)
  │    - ItemCompleted(工具调用/消息完成)
  │    - TurnCompleted(turn 结束)
  │    - ServerRequest::CommandExecutionRequestApproval(审批)
  │ 9. serialize_outgoing_message(event) → JSON (codex 现成的序列化)
  │ 10. tao proxy 投递到主线程
  │ 11. 主线程 webview.evaluate_script("window.__lemurclaw.onEvent(json)")
  ▼
React 收到事件 → 更新 ViewModel → 重渲染
```

**关键:第 3/9 步的 JSON 编解码是 codex 现成的**(`forward_incoming_message`/`serialize_outgoing_message`),lemurclaw-gui 的 shim 只是薄转发。

### 1.4 审批流

agent 想跑命令时 → `ServerRequest::CommandExecutionRequestApproval`(带 command/cwd)→ 到达 React → 渲染审批卡 → 用户决定 → `resolve_server_request(id, {decision: Accept|Decline|Cancel})`。codex 协议原生,lemurclaw 只转发。

---

## 设计章节 2:Workspace 与 Crate 结构

> **架构转向记录(2026-07-18):** 原方案"独立 workspace + submodule + path dep codex"经实测推翻(codex 127-crate workspace + alpha 依赖 rama + cargo workspace 根约束,详见 `notes/2026-07-18-integration-blocker-findings.md`)。**转向:lemurclaw 仓库本身就是 openai/codex 的 fork**(远程 `defims/lemurclaw` 已改为 fork from openai/codex)。lemurclaw 的 crate 直接加进 codex-rs workspace,像 codex 自己的 crate 一样工作。**实测验证零摩擦**(lemurclaw-transport 作为 codex-rs member 编译+测试通过)。这符合行业通用做法:codex 不发 crates.io,所有 codex 下游(rust-analyzer/helix/zed 类)都是 fork 模式。

### 2.1 目录结构

lemurclaw = openai/codex fork。codex-rs workspace 在 `codex-rs/`(仓库根下)。lemurclaw 的 3 个 Rust crate 作为**新 member** 加进 codex-rs workspace。

```
lemurclaw/                         # = openai/codex fork(远程 defims/lemurclaw)
├── codex-rs/                      # codex 的 Rust workspace(lemurclaw crate 加进这里)
│   ├── Cargo.toml                 # workspace 根,members 含 lemurclaw-*(直接改)
│   ├── Cargo.lock                 # codex 的 lockfile,lemurclaw crate 共享(零 alpha 漂移)
│   ├── core/                      # codex-core(三方模型改动直接在此,非 patch)
│   ├── model-provider-info/       # WireApi::Chat 改动直接在此
│   ├── tui/                       # codex-tui(TUI 复用)
│   ├── app-server*/               # in_process API + 协议
│   ├── ...                        # codex 其余 ~120 crate
│   ├── lemurclaw/                 # ← 新增:lib+bin,runtime + config
│   │   ├── Cargo.toml             # workspace=true 引用 codex 依赖
│   │   └── src/{lib.rs, main.rs, config.rs}
│   ├── lemurclaw-transport/       # ← 新增:Transport trait + JSON 编解码(无 wry/tao)
│   │   └── src/lib.rs
│   └── lemurclaw-gui/             # ← 新增:wry+tao+shim + assets/
│       ├── Cargo.toml
│       ├── build.rs               # npm run build → assets/dist
│       ├── src/lib.rs
│       └── assets/{package.json, src/, dist/}
├── docs/superpowers/              # lemurclaw 自己的 brainstorming 产出(spec/plan/notes)
├── AGENTS.md / README.md / ...    # codex 原有(lemurclaw 可改 README)
└── (codex 其余:codex-cli, sdk, docs, ...)
```

**关键变化(vs 原方案):**
- ❌ 不再有 `vendor/` submodule(lemurclaw 就是 codex 本身)
- ❌ 不再有 `patches/` + `apply-patches.sh`(改动直接在 fork 里,跟 upstream 同步靠 git merge)
- ❌ 不再有 `[patch.crates-io]` 复制问题(共享 codex 的)
- ✅ lemurclaw crate 用 `workspace=true` 引用 codex 依赖(就像 codex 自己的 crate)
- ✅ 共享 codex 的 Cargo.lock → 零 alpha 漂移
- ✅ 共享 codex 的 [workspace.lints] / [patch] / workspace.dependencies
- ✅ 三方模型改动直接改 codex-rs/core + model-provider-info(fork 内改,非 patch)

### 2.2 crate 职责

**`lemurclaw`(lib + bin,在 `codex-rs/lemurclaw/`)** —— runtime + config。
- `src/lib.rs`:`pub async fn run(config: RuntimeConfig) -> Result<ExitCode>`、`enum Frontend { Tui, Gui, Webui }`。
  - Tui → 调 `codex_tui::run_main`(同 workspace,直接引用)
  - Gui → 调 `lemurclaw_gui::run`(in_process + wry)
  - Webui → 起 codex app-server WebSocket server + serve assets + 打印 URL(只依赖 lemurclaw-transport)
- `src/main.rs`:clap 解析 CLI → `RuntimeConfig` → `lemurclaw::run`。
- `src/config.rs`:RuntimeConfig + 配置文件读写 + 封装 codex-config。
- Cargo.toml:`version.workspace=true`/`edition.workspace=true`/`[lints] workspace=true`,依赖用 `workspace=true`(codex-tui、codex-config、lemurclaw-transport 等)。
- **不含 wry/tao/ratatui**(GUI 在 lemurclaw-gui,TUI 用 codex-tui)。

**`lemurclaw-transport`(在 `codex-rs/lemurclaw-transport/`)** —— Transport 抽象 + JSON 编解码,**无 wry/tao/ratatui**。
- `src/lib.rs`:`pub trait Transport`(async send/recv)+ JSON 编解码辅助。
- 依赖:`codex-app-server-protocol`(workspace=true)、serde、serde_json、tokio。
- **核心价值:webui 模式只依赖此 crate,不拉 wry/tao。**
- **实测验证(2026-07-18):作为 codex-rs member 编译+测试通过,零摩擦。**

**`lemurclaw-gui`(在 `codex-rs/lemurclaw-gui/`)** —— GUI 整块(wry+tao+shim+前端资源)。
- wry+tao 集成、AppServerClient(InProcess)、ipc_handler 收 JSON → ClientRequest、next_event 循环 → serialize → tao proxy → evaluate_script。
- 实现 `WryIpcTransport`(满足 `lemurclaw_transport::Transport`)。
- 依赖 `lemurclaw-transport`(workspace=true)+ wry + tao。
- `assets/`:React 源码 + Vite dist。`build.rs` 检测 Node 时 `npm run build`。
- build 时 copy codex-rs/app-server-protocol/schema/typescript/ 的 ts-rs 类型进 `assets/src/types/`。
- **不依赖 ratatui。**

### 2.3 依赖关系(同 workspace 内)

```
codex-rs workspace(共享 Cargo.lock / [patch] / lints / workspace.dependencies)
  │
  ├─ lemurclaw (bin+lib) ─┬─ codex-tui {workspace=true}        [tui 路径]
  │                       ├─ codex-config {workspace=true}
  │                       ├─ lemurclaw-transport {workspace=true}  [webui 路径只要这个]
  │                       └─ lemurclaw-gui {workspace=true} ─┬─ wry + tao  [gui 路径]
  │                                                            ├─ codex-app-server-client
  │                                                            ├─ lemurclaw-transport
  │                                                            └─ assets/ (include_dir)
  │
  └─ codex 原有 crate(core/model-provider-info/tui/app-server*/...)
       (三方模型改动:直接改 core + model-provider-info,fork 内修改非 patch)
```

### 2.4 upstream 同步(fork 流程)

lemurclaw 是 openai/codex fork,跟 upstream 同步用标准 git 流程:
```bash
git remote add upstream https://github.com/openai/codex.git  # 一次性
git fetch upstream
git merge upstream/main           # 合并上游改动(冲突时手动解决)
# lemurclaw crate 在 codex-rs/Cargo.toml 的 members 声明、codex-rs/lemurclaw-*/ 可能冲突
# 三方模型改动(core/model-provider-info)若 upstream 改了同处,手动 rebase
```
不再需要 patch apply 脚本——所有 lemurclaw 改动都是 fork 内的直接 commit,merge upstream 时 git 处理。

### 2.5 关键约束(fork 布局下)

- lemurclaw crate 的 Cargo.toml 用 `workspace=true` 引用 codex 的依赖/版本/lints(参照 codex-rs/app-server-protocol/Cargo.toml 模板)。
- lemurclaw crate 加进 `codex-rs/Cargo.toml` 的 `[workspace].members`(直接改 fork)。
- 在 `codex-rs/` 目录下跑 cargo(`cargo check -p lemursclaw-transport`),不在仓库根。
- lemurclaw 的改动(新 crate + 三方模型)都是 fork commit,与 upstream 同步靠 git merge。

---

## 设计章节 3:三方模型改动契约(codex 方案的核心)

> **转向记录:** 原方案"patch submodule"已改为"fork 内直接改"(因集成方式转向 fork)。改动内容相同,但载体从 `patches/*.patch` + apply 脚本变为 fork 内的直接 git commit。与 upstream 同步靠 git merge(冲突手动解决),不再靠 patch reapply。

### 3.1 为什么必须改 core(勘察事实)

codex-rs **无 trait 注入点**(已核实):
- `ModelClient` 是具体类型(非 `Box<dyn>`),session 硬连到它(`session.rs:1117`)。
- `WireApi` 枚举只有 `Responses` 一个变体(`model-provider-info/src/lib.rs:55-61`)。
- `wire_api="chat"` 反序列化时**硬报错**(`lib.rs:72-84`,`CHAT_WIRE_API_REMOVED_ERROR` 指向 discussion #7782)。
- `client.rs:1787-1825` 分发 match 只有 `Responses` arm。
- `chat_completions.rs`(1471行)在 codex-rs **不存在**,只在 code-rs 有。

**结论:** 三方模型必须直接改 codex-rs 的 core + model-provider-info,无法外部注入。勘察同时证明 fork(code-rs)自己也是"直接改 core"实现的。fork 布局下,lemurclaw 就在 codex-rs 里,直接改这些文件即可。

### 3.2 改动内容(直接改 fork 内的 codex-rs/)

| Part | 类型 | 内容 |
|---|---|---|
| 1 | 改 fork 内文件 | `codex-rs/model-provider-info/src/lib.rs`:`WireApi` 加 `Chat`+`ResponsesWebsocket` 变体;反序列化撤销 `chat` 硬报错 |
| 2 | 改 fork 内文件 | `codex-rs/core/src/client.rs`:分发 match 加 `Chat` + `ResponsesWebsocket` arm |
| 3 | 新文件 | `codex-rs/core/src/chat_completions.rs`(从 code-rs 移植,1471行,适配 codex-rs 类型差异) |
| 4 | 新文件 | `codex-rs/core/src/model_family.rs`(从 code-rs 移植,codex-rs 独有) |
| 5 | 改/新建 fork 内文件 | `codex-rs/core/src/openai_tools.rs`:增补 `create_tools_json_for_chat_completions_api`(codex-rs 无则新建) |
| 6 | 改 fork 内文件 | `codex-rs/core/src/lib.rs`:加 `mod chat_completions; mod model_family;` |
| 7 | 改 fork 内文件 | provider 注册表增补(可选,首版可只提供配置机制不给默认 provider) |

**参照源:** just-every/code 的 `code-rs/core/src/`(lemurclaw 需单独 clone just-every/code 作为参照,或从 GitHub 直接读)。lemurclaw fork 本身不含 code-rs。

### 3.3 chat_completions.rs 的 15 个 crate:: 依赖(移植须逐一适配)

勘察列出的依赖:`auth::AuthManager`、`ModelProviderInfo`、`client_common::{Prompt,ResponseEvent,ResponseStream,replace_image_payloads_for_model,rewrite_image_generation_calls_for_input}`、`debug_logger::DebugLogger`、`error::{CodexErr,Result,RetryLimitReachedError,UnexpectedResponseError}`、`model_family::ModelFamily`、`openai_tools::create_tools_json_for_chat_completions_api`、`util::backoff`。

其中 fork-only(codex-rs 无,需移植):`create_tools_json_for_chat_completions_api`、`ModelFamily`、`chat_completions.rs` 本身、code-rs 版的 `model_provider_info`(codex-rs 是独立 crate,code-rs 内联进 core——**结构差异要适配**)。

**`AggregatedChatStream` 定义在 chat_completions.rs 内部**(L1073),随文件一起移植。

### 3.4 明确不改

| 不改 | 原因 |
|---|---|
| codex-rs app-server 协议 | 公开,直接用 |
| codex-rs in_process API | 公开 |
| codex-rs 沙箱(sandboxing/windows-sandbox-rs) | 原生支持 Windows,不动 |
| codex-rs TUI | 复用,不改 |

> 注:codex-rs 根 Cargo.toml 的 workspace 声明**会改**(加 lemurclaw-* member),这是 fork 布局必需。

### 3.5 维护(fork 流程)

三方模型改动是 fork 内的直接 git commit(非 patch 文件)。与 upstream 同步:
```bash
git fetch upstream
git merge upstream/main
# 若 upstream 改了 core/client.rs 或 model-provider-info 同处,手动解决冲突
cargo test -p codex-core --lib   # 验证 upstream 测试不破
cargo test -p codex-tui          # 验证 TUI
```
不再需要 apply-patches.sh——改动已在 fork 历史里,merge 时 git 处理。

### 3.6 诚实风险

| 风险 | 严重度 | 说明 |
|---|---|---|
| **upstream 主动对抗 Chat** | **高** | #7782 故意删,每次 mirror 刷 client.rs/model-provider-info 冲突概率高 |
| **chat_completions 移植依赖闭包** | **高** | 15 个 crate:: 类型,codex-rs vs code-rs 分歧要逐一适配,首次 2-3 周 |
| **upstream 重构 ModelClient** | 中 | 若改 client.rs 结构,Part 2 重写 |
| **codex-rs mirror 刷新频率** | 中 | fork 活跃(持续更新),patch 维护税持续 |

> **关键张力(写明供未来维护者知晓):** 选"基座 codex-rs + patch 三方模型"是逆 upstream 决定。每次 mirror 刷新都在跟 upstream 抢 client.rs 和 model-provider-info 的控制权。对比"基座 code-rs + 移植 Windows 沙箱"(沙箱是独立 crate,搬一次基本完事),本方案的维护税更高。用户已知晓并接受(基座 codex-rs 既有 Win 沙箱又有 in_process API,patch 只补三方模型)。

---

## 设计章节 4:GUI 布局 + codex TUI 组件映射

### 4.1 设计原则:功能等价,布局自由

- **功能覆盖**:~35-40 个 codex TUI surface 每个在 GUI 都有功能等价(清单即验收单,不漏)。
- **组件粒度**:多个 TUI surface 可共用一个通用 React 组件(codex TUI 本身大量复用 `ListSelectionView`/`MultiSelectPicker`),但要能映射回每个 surface。
- **布局**:用 GUI 原生(主对话区 + 侧栏 + 模态 + 全屏路由),不强制复刻 TUI 的流式 + 底部 overlay 结构。

### 4.2 codex TUI surface → GUI 组件映射(验收矩阵)

| 类别 | codex TUI surface | GUI 组件 | 数据源(ServerNotification/Request) |
|---|---|---|---|
| **对话** | history_cell: user/assistant/reasoning/agent msg | `<Scrollback>` + 子 cells | `AgentMessageDelta`/`ItemCompleted`/`ReasoningSummaryTextDelta` |
| | exec/unified-exec cell | `<ExecBlock>` | `CommandExecutionOutputDelta`/`ProcessExited` |
| | patch cell | `<DiffBlock>` | `FileChangePatchUpdated`/`TurnDiffUpdated` |
| | mcp tool call cell | `<McpToolBlock>` | `ItemCompleted`(MCP) |
| | plan cell(plan mode) | `<PlanBlock>` | `PlanDelta` |
| | web search cell | `<WebSearchBlock>` | `ItemCompleted`(web search) |
| | hook cell | `<HookBlock>` | `HookStarted`/`HookCompleted` |
| | transcript overlay(Ctrl+T pager) | `<TranscriptPager>` | 全量 history 回放 |
| **输入** | composer textarea | `<PromptInput>` | 本地状态 |
| | command popup(斜杠) | `<SlashPopup>` | `available_commands`(Initialize) |
| | skill/mention popup | `<MentionPopup>` | skills 列表 |
| | file search popup(@文件) | `<FileSearchPopup>` | `FuzzyFileSearchSession*` |
| **审批** | ApprovalOverlay(exec/file/perm/mcp) | `<ApprovalCard>` | `ServerRequest::*Approval`/`ToolRequestUserInput` |
| | McpServerElicitationOverlay | `<McpElicitation>` | `McpServerElicitationRequest` |
| | AppLinkView(Desktop handoff) | `<AppLinkModal>` | elicitation |
| **导航/选择** | resume picker(会话) | `<SessionPicker>` | Thread 列表 |
| | model picker | `<ModelPicker>`(通用 ListSelect) | `Initialize.models` |
| | agent/subagent picker | `<AgentPicker>` | 多 agent 状态 |
| | theme/keymap picker | `<SettingsListPicker>`(通用) | 本地配置 |
| **配置/管理** | permissions/keymap/memories/skills/hooks/mcp/apps/plugins/experimental/statusline/title | `<SettingsModal>`(通用,复用) | 各自配置 |
| | feedback view | `<FeedbackModal>` | — |
| | custom prompt view | `<CustomPromptModal>` | — |
| | Claude Code import | `<ImportModal>` | `ExternalAgentConfigImport*` |
| **onboarding** | welcome/sign-in/trust | `<Onboarding>` | auth flow |
| **状态/通知** | status indicator | `<StatusBar>` | `ThreadStatusChanged`/token usage |
| | update available / deprecation | `<Notice>` | `Warning`/`DeprecationNotice`/`UpdateAvailable` |

**关键观察:** codex 22 个 BottomPaneView 大多是"列表选择/多选/表单",GUI 用 2-3 个通用组件(`<SettingsListPicker>`/`<MultiSelect>`/`<SettingsModal>`)覆盖。bespoke React 组件约 **18-22 个**(含通用)。

### 4.3 主对话窗口布局(GUI 原生,不复刻 TUI 流式)

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  lemurclaw — [~/code/proj]  grok-4.5 ⏷  ⚙                                       │  顶栏(目录+模型+菜单)
├────────────────────────────────────────────────────┬─────────────────────────┤
│                                                    │  ◐ 侧栏(可折叠)         │
│  ▎user                                             │  ───────────────────    │
│  帮我重构 parser                                    │  会话                    │
│                                                    │   ▸ 当前: refactor       │
│  ▎assistant  ▎reasoning: 分析依赖…                  │   ⏚ 历史 (12)            │
│  我来读取相关文件…                                   │  ───────────────────    │
│                                                    │  Agent                   │
│  ┌─ $ cargo check ────────────────────────┐       │   ● main  运行中         │
│  │ warning: unused import...               │       │   ○ sub   空闲           │
│  │ ┊ (tap 展开)                            │       │  ───────────────────    │
│  └─ [✓ 已批准]                              │       │  Plan                    │
│                                                    │   1. 读 parser.rs ✓      │
│  ┌─ 📝 patch src/parser.rs ────────────────┐      │   2. 提取调用点 →        │
│  │  - fn parse(input) {                    │      │   3. 重写                │
│  │  + fn parse(input, opts) {              │      │  ───────────────────    │
│  │  ┊ 12 行 diff    [接受] [拒绝]           │      │  Token 4.2k / 128k       │
│  └──────────────────────────────────────────┘      │                         │
├────────────────────────────────────────────────────┴─────────────────────────┤
│  / 斜杠命令  ·  @ 文件/skill  ·  ⌘K 命令面板                                      │  输入辅助
├──────────────────────────────────────────────────────────────────────────────┤
│  ▎ > 输入消息…                                                    [Enter 发送] │  composer
└──────────────────────────────────────────────────────────────────────────────┘
```

- **左主对话 + 右侧栏**:对话占主体,侧栏(会话/Agent/Plan/Token)可折叠
- **工具调用/diff 内联**对话流(折叠块,点击展开)
- **审批内联**对话流(高亮卡片,非总是弹窗)
- codex 的 `/agent` `/subagents` 是模式切换(非常驻),侧栏 Agent 区可选浮出;Plan mode 是 codex 核心特性(`PlanDelta` 流),侧栏 Plan 区常驻

### 4.4 响应式

- 宽屏(>1200px):主对话 + 右侧栏常驻
- 中屏(800-1200):侧栏按需 drawer
- 窄屏(<800):全宽,侧栏/审批全屏 modal

---

## 设计章节 5:传输抽象 + TS 类型复用

### 5.1 Transport 抽象(独立 crate `lemurclaw-transport`)

```
React (TS)                          Rust
  │                                   │
  │  Transport 接口(TS)               │  trait Transport (Rust, in lemurclaw-transport crate)
  │  - send(ClientRequest)            │  - send(ClientRequest)
  │  - onEvent(ServerEvent)           │  - recv() -> ServerEvent
  │                                   │
  ├─ WryIpcTransport (gui 默认)       ├─ WryIpcTransport (in lemurclaw-gui, impl trait)
  │   window.ipc.postMessage          │   ipc_handler 收 JSON
  │   window.__lemurclaw.onEvent      │   evaluate_script 推 JSON
  │                                   │
  └─ WebSocketTransport (webui)       └─ (后端起 ws server)
      new WebSocket(ws://localhost)       codex app-server --listen ws://
```

两套传输说同一套协议(codex 的 ClientRequest/ServerNotification JSON),React 业务逻辑完全共用,只换 Transport 实现。`lemurclaw-transport` 独立成 crate,**让 webui 模式只依赖它(无 wry/tao)**,gui 模式才拉 `lemurclaw-gui`。

### 5.2 Rust 侧(gui 默认模式)

```rust
// lemurclaw-transport crate
pub trait Transport: Send {
    async fn send(&self, req: ClientRequest) -> io::Result<()>;
    async fn recv(&mut self) -> Option<ServerEvent>;
}
// WryIpcTransport 在 lemurclaw-gui 实现(impl lemurclaw_transport::Transport):
//   - send: ipc_handler 收 JSON → 反序列化 ClientRequest → app_server_client.request/notify
//   - recv: app_server_client.next_event() → serialize → tao proxy → evaluate_script
```

**集成点用 `AppServerClient`(codex 官方推荐,InProcess 变体)**:封装 in_process handle + worker task + 背压,而非裸 InProcessClientHandle。

### 5.3 TS 类型复用

`codex-rs/app-server-protocol/schema/typescript/` 有 ts-rs 生成的 92 个 `.ts` 类型文件。**build 时 copy** 进 `codex-rs/lemurclaw-gui/assets/src/types/`(build.rs 或 Vite 预构建步骤)。保证 Rust↔JS 边界类型安全。

### 5.4 句柄持有与线程(wry 固有约束)

`WebView` 方法签名 `&self` 但内部用 `Rc`(非 Arc),**非 Send**,绑主线程。tokio 的 next_event 循环(在 worker 线程)通过 **tao `EventLoopProxy`** 投递 JSON 回主线程,主线程用 `&webview` 调 evaluate_script。无 Arc/Mutex。

### 5.5 webui 模式

`--frontend webui`:
1. lemurclaw 起 codex app-server 的 WebSocket server(`--listen ws://127.0.0.1:PORT`,codex 原生支持)
2. lemurclaw-gui 的 assets 构建为静态资源,由同进程 axum serve
3. 打印 `http://localhost:PORT` 让用户浏览器访问
4. 浏览器里的 React 用 WebSocketTransport 连 ws://

前端代码与 gui 模式完全共用(只 Transport 实例化不同)。

### 5.6 前端构建

`lemurclaw-gui/build.rs`:检测到 Node 可用时跑 `npm install && npm run build`,生成 `assets/dist/`;无 Node 时用预构建 dist(仓库提交)。同时 copy codex-rs ts 类型进 `assets/src/types/`。

---

## 设计章节 6:范围分解 + 测试 + 发布

### 6.1 子项目分解

```
子项目 0:基础骨架
    │ 在 fork(codex-rs workspace)内加 3 个 lemurclaw crate(lemurclaw/lemurclaw-transport/lemurclaw-gui 空壳)
    │ 改 codex-rs/Cargo.toml 加 members;在 codex-rs/ 下跑 cargo
    │ 产出:可构建,--frontend tui 跑通 codex-tui
    ▼
子项目 1:三方模型改动(最重,独占)
    │ 直接改 fork 内 codex-rs/:WireApi::Chat + 反序列化 + client.rs arm + chat_completions.rs(移植)
    │ + model_family.rs + openai_tools 增补 + provider 注册表(参照 just-every/code 的 code-rs)
    │ 产出:wire_api=chat 配置能跑通一个三方模型(如 OpenRouter)
    ▼
子项目 2:GUI 基础设施(gui 模式)
    │ lemurclaw-gui(wry+tao + AppServerClient + ipc shim + tao proxy)
    │ + lemurclaw transport trait + WryIpc 实现
    │ + assets React 骨架 + Transport TS 接口 + ts 类型 copy
    │ 产出:空 React 页面收 ServerNotification、发 ClientRequest,跑通一轮对话
    ▼
子项目 3:核心对话循环(矩阵:对话+输入+审批)
    │ Scrollback + history cells(user/assistant/reasoning/exec/patch/mcp/plan/websearch)
    │ + composer + slash/@/file-search popup + ApprovalCard
    │ 产出:GUI 跑完整 agent 对话(含工具调用+审批),功能等价 TUI 主体
    ▼
子项目 4:导航与全屏视图
    │ SessionPicker + ModelPicker + AgentPicker + Onboarding + TranscriptPager + ThemePicker
    ▼
子项目 5:配置与管理模态
    │ SettingsModal 系列 + statusline/title + diff viewer + /usage + /status
    ▼
子项目 6:webui 模式
    │ runtime 的 webui 路径(起 ws server + serve assets + 打印 URL)+ WebSocketTransport
    │ 产出:--frontend webui 在浏览器跑
    ▼
子项目 7:发布就绪
    │ GitHub release 预编译二进制(mac/win/linux)+ cargo install --git + 可选 npm
    │ + surface 覆盖矩阵全验收 + 文档 + Linux libwebkit2gtk 依赖说明
    │ + 双前端录放测试套件
```

**第一个实现计划聚焦子项目 0+1**(骨架 + 三方模型改动)。子项目 1 是最重、风险最高部分(逆 upstream #7782 + chat_completions 移植)。fork 布局下不再需要 patch apply 机制。

### 6.2 等效性验证(比 grok-build 简单)

等效性**自动成立**(TUI/GUI 同源消费 ServerNotification 事件流):
1. **协议契约(自动):** GUI 消费的事件流与 TUI 完全一致(都是 AppServerClient.next_event())。无需双前端录放对比——同源。只需验证 Transport 不丢/不篡改事件。
2. **surface 覆盖矩阵(清单验收):** 每个 TUI surface 在 GUI 有等价,逐个核对(§4.2 矩阵)。
3. **无头截图(可选):** 关键场景(对话/审批/diff)GUI 截图人工 review,确认功能正确(不要求视觉一致)。

### 6.3 测试

| 层 | 内容 |
|---|---|
| Rust 单元 | Transport JSON 编解码 round-trip;config 解析 |
| Rust 集成 | patch 不破上游:`cargo test -p codex-core/codex-tui`(vendor/patched);三方模型:mock Chat endpoint 测流式 |
| Rust 集成 | lemurclaw-gui:喂录制事件序列,断言 evaluate_script 推出的 JSON |
| React 单元 | 组件 @testing-library/react,给定 ServerNotification 渲染正确;Transport mock |
| 端到端 | 三前端(tui/gui/webui)跑同一组对话脚本,确认都能完成 turn |

### 6.4 风险登记

| 风险 | 严重度 | 说明 |
|---|---|---|
| **三方模型改动逆 upstream** | **高** | #7782 故意删 Chat,每次 merge upstream/main 时 client.rs/model-provider-info 冲突 |
| **chat_completions 移植依赖闭包** | **高** | 15 个 crate:: 类型,首次移植 2-3 周;codex-rs vs code-rs 分歧逐一适配 |
| **upstream 重构 ModelClient** | 中 | 若改 client.rs 结构,client.rs Chat arm 重写 |
| **fork 与 upstream 同步** | 中 | lemurclaw crate 的 members 声明、三方模型改动,merge upstream 时可能冲突,手动解决 |
| Linux webview 依赖 | 低 | wry 固有,文档说明 libwebkit2gtk-4.1 |
| surface 覆盖遗漏 | 中 | 靠覆盖矩阵清单验收控制 |

### 6.5 发布(实测验证:依赖 codex 的 crate 发不了 crates.io)

**实测结论**(本 brainstorming 中用 `cargo publish --dry-run` 验证):
- lib crate 含 codex 依赖 → `no matching package codex-core found` 失败
- bin crate 含 codex 依赖 → 同样失败(打包阶段重定向到 crates.io 索引)
- 根因:openai 不发 codex Rust crate;codex 有 `[patch.crates-io] ratatui=fork` 在单 crate/workspace 外失效;vendor 全合并 codex(150 crate)不现实

**发布策略(采纳行业通用做法,与 openai/codex、just-every/code 一致):**

| 产物 | 渠道 | 说明 |
|---|---|---|
| `lemurclaw` 单二进制 | GitHub release 预编译 | mac/win/linux;Linux 注明 libwebkit2gtk-4.1 |
| `cargo install --git` | git 仓库 | 从源码编译装 bin |
| 库复用 | git 仓库 | 其他项目 `lemurclaw = { git = "url" }` |
| 可选 npm 包 | npm | `@lemurclaw/cli`,postinstall 下载二进制(模仿 codex) |
| ~~完整 crate 发 crates.io~~ | 不发 | 实测不可行(codex 依赖) |
| **`lemurclaw` 占位 crate** | **crates.io** | **占名用空壳 crate(README + 空 lib.rs,无 codex 依赖),description/repository 指向 git 仓库。实测 `cargo publish --dry-run` 通过。用户 `cargo add lemurclaw` 拿到占位壳,文档引导走 git 获得完整功能** |

**占位 crate 物理位置:** 放在 fork 仓库的 `squat/lemurclaw/`(codex-rs workspace 外独立 crate)。**关键约束**:占位 crate 和真实 `codex-rs/lemurclaw/` 都叫 `lemurclaw`,不能在同一 Cargo workspace 共存。所以 `squat/` **不在** codex-rs workspace 的 members 里,它是独立 crate,单独 `cd squat/lemurclaw && cargo publish` 发布。

```
lemurclaw/                         # = openai/codex fork
├── codex-rs/                      # codex workspace(真实 lemurclaw crate 在这)
│   ├── Cargo.toml                 # members 含 lemurclaw/lemurclaw-transport/lemurclaw-gui
│   ├── lemurclaw/                 # 真实 runtime crate(含 codex,发不了 crates.io)
│   ├── lemurclaw-transport/
│   ├── lemurclaw-gui/
│   └── ... (codex 原有 crate)
├── squat/                         # ← 占位 crate 区,codex-rs workspace 外
│   └── lemurclaw/                 # 空壳,同名,独立 cargo publish
│       ├── Cargo.toml             # name="lemurclaw",无 codex 依赖,只依赖 serde(可选)
│       ├── lib.rs                 # 占位(可放 Frontend enum 等无 codex 的纯类型,或空)
│       └── README.md              # 引导:完整功能走 cargo install --git
└── (codex fork 其余内容)
```

**占位 crate 说明:** crates.io 上的 `lemurclaw` 与 fork 里的 `codex-rs/lemurclaw/`(完整 runtime)**同名但内容不同**——前者是纯占名空壳(无 codex,能 publish),后者是完整实现(走 git)。这是 crates.io 允许的占名模式,确保包名不被他人抢注,同时完整功能不受 codex 依赖阻塞。未来若 codex upstream 发 crates.io,可逐步把真实功能迁入 crates.io 上的 `lemurclaw`。

### 6.6 YAGNI 排除

- ❌ 自建 TUI(复用 codex-rs/tui)
- ❌ 移动端
- ❌ 远程多客户端(webui 是本地浏览器)
- ❌ crates.io 发布完整功能(实测不可行)
- ❌ 自动跟随 upstream main(手动 git merge upstream)
- ❌ 增量事件推送(MVP 全量)
- ❌ 视觉复刻 TUI(功能等价,布局自由)
- ❌ vendor 全 codex 发 crates.io(150 crate 不现实)
- ❌ submodule + patch apply 机制(已转向 fork 直接改)

---

## 附录:决策溯源

1. **grok-build → codex 转向:** grok-build 沙箱(`xai-grok-sandbox`+`nix`)只支持 Unix,不支持 Windows。转向 codex(codex-rs 有 `windows-sandbox-rs`:JobObjects+WFP+restricted token,37/41 smoketest 通过)。
2. **Win 沙箱 + 三方模型都在 codex 生态:** codex-rs(openai/codex)有 Win 沙箱无三方模型;just-every/code 的 code-rs 有三方模型无 Win 沙箱。codex-rs 无 trait 注入点(`ModelClient` 具体类型),三方模型必须直接改 core。用户选**基于 codex-rs + 直接改三方模型**(逆 upstream #7782)。
3. **crate 结构:** 3 Rust crate(`lemurclaw` lib+bin + `lemurclaw-transport` 独立无 wry + `lemurclaw-gui` wry+assets)。transport 独立让 webui 不背 wry。
4. **crates.io 完整功能不可行,占名可行:** 实测 `cargo publish` 对 codex 依赖失败(含 optional+git+version 都失败)。完整功能走 git+二进制。**占名**:额外发空占位 crate(无 codex 依赖)到 crates.io 抢占 `lemurclaw` 名,实测通过。
5. **等效性自动成立:** TUI/GUI 同源消费 ServerNotification 事件流(经 AppServerClient),无需 patch 状态机(对比 grok-build 方案要 patch 8 处可见性)。
6. **集成方式转向 fork(2026-07-18):** 原"独立 workspace + submodule + path dep codex"经实测推翻(codex 127-crate workspace + alpha 依赖 rama 漂移 + cargo workspace 根约束,详见 `notes/2026-07-18-integration-blocker-findings.md`)。**转向 fork**:lemurclaw = openai/codex fork,crate 直接加进 codex-rs workspace,共享 Cargo.lock/patch/lints,实测零摩擦。三方模型从 patch 变 fork 直接改。这是 codex 类(不发 crate 的巨型 workspace)上游的通用做法。
