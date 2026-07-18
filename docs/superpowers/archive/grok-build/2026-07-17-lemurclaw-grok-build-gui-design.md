# lemurclaw 设计 spec:基于 grok-build 的可复用 agent runtime + 等价 TUI 的 GUI

- **日期:** 2026-07-17
- **状态:** Draft(待用户 review)
- **作者:** Long, Wei(brainstorming 协作产出)
- **上游:** `xai-org/grok-build`(Apache-2.0),以 git submodule 引入

## 目标

把 `xai-org/grok-build` 做成一个**可被其他项目引用的 agent runtime crate**,并提供**等价 TUI 功能的 GUI**:
- runtime 启动时可配置使用 **TUI 或 GUI**(二选一,默认 TUI)
- **TUI 复用上游** `xai-grok-pager`,不重写
- **GUI 自建**:React 前端 + wry webview + 进程内 IPC
- runtime 可配置:agent 名称、工作目录、模型、frontend 选择等
- 其他项目 `cargo add lemurclaw-runtime` 后可嵌入

## 关键决策(已与用户确认)

| 维度 | 决定 |
|---|---|
| 上游 | `xai-org/grok-build`(Apache-2.0),git submodule,放弃 claurst clean-room |
| crate 形态 | 可复用的 agent runtime crate |
| IPC | 进程内,wry 原生桥(`ipc_handler` + `evaluate_script`) |
| GUI 前端 | React(生态成熟),Vite 构建,`include_dir!` 嵌入 |
| GUI 范围 | 完整对等 TUI(追平上游 ~50 个 view 的功能) |
| TUI 来源 | 复用上游 `xai-grok-pager`,不重写 |
| 与上游关系 | submodule 依赖,作 path 依赖,不合并 workspace |
| 实现路线 | 方案 B → 修正为**路 1**(四环全暴露 patch),复用上游完整反馈循环保证等效 |
| patch 深度 | 暴露 dispatch + execute + dispatch_task_result + acp_handler 四环 |

> **诚实记录:** 章节 3 的 patch 契约经历了修正。初版 B1 只计划暴露 `dispatch`,承诺"GUI 行为自动与 TUI 对齐"。在对上游源码的勘察中发现:上游反馈循环 5 环中有 4 环(类型 + 4 个行为函数中的全部)是 `pub(crate)` 或更严,只暴露 dispatch 不够——GUI 拿到 Effect 却无法用上游方式执行。用户据此选择路 1:扩大 patch 到四环全暴露,真正复用完整循环,以"共享同一份代码"保证等效。

---

## 设计章节 1:架构总览与数据流

### 1.1 三层模型

```
┌─────────────────────────────────────────────────────────┐
│                    lemurclaw binary                      │
│  (composition root: parse config, pick frontend, boot)   │
└────────────┬────────────────────────────┬───────────────┘
             │ --frontend=tui             │ --frontend=gui
             ▼                            ▼
   ┌──────────────────┐         ┌─────────────────────┐
   │  上游 TUI 前端     │         │   GUI 前端(自建)     │
   │  xai-grok-pager  │         │  wry + React        │
   │  app::run        │         │  (patch 暴露的状态机) │
   │  (原封不动)       │         │                     │
   └────────┬─────────┘         └──────────┬──────────┘
            │                              │
            │     共享同一份 patch 后的      │
            │      ┌────────────────┐      │
            └─────►│  agent runtime │◄─────┘
                   │  (上游 submodule │
                   │   + 四环 patch)   │
                   │  dispatch/effects│
                   │  经 ACP 通信      │
                   └────────────────┘
```

### 1.2 关键原则

- **前端可切换,运行时单例。** 启动时 `--frontend tui|gui` 二选一(默认 tui),同一进程同一 runtime。
- **TUI 模式零改动:** 转调上游 `xai_grok_pager::app::run`,体验与上游一致。
- **GUI 模式复用上游纯逻辑,自建渲染:** 经 patch 暴露的四环驱动 AppView;view 的视觉层在 React 重写(view-model 是 ratatui widget 形状,无法直接喂 DOM)。
- **GUI 与 agent 通信仍经 ACP:** `xai_grok_pager::acp::connect` 拿 `{tx, rx}`,与上游 TUI 自用机制一致。
- **配置层统一:** agent 名称、工作目录、frontend 选择等,通过 `lemurclaw-config` 封装上游 `xai_grok_shell::config`,两个前端共用。

### 1.3 GUI 数据流(单向)

```
React 组件
   │  1. 用户输入/点击 → 构造 Action(纯数据)
   │  2. wry ipc_handler 收到 Action JSON
   ▼
[Rust] lemurclaw-gui-bridge
   │  3. 反序列化为上游 Action 类型
   │  4. dispatch::dispatch(&mut app, action) → Vec<Effect>     环①
   │  5. effects::execute(Effect, ...) → spawn 异步任务          环②
   │     (经 AcpAgentTx 发 ACP 请求 / 文件操作 / ...)
   ▼
agent runtime (patch 后) ──ACP──► grok agent
   │                              │
   │  6. ACP rx 收 session/update  │
   │     → acp_handler::handle      环④
   │     / TaskComplete → dispatch_task_result  环③
   │     → dispatch 更新 AppView
   ▼
[Rust] view-model 投影
   │  7. 经 minimal_api facade 读 AppView 字段
   │  8. 投影成 React 友好的 JSON(剔除 ratatui widget 形状)
   │  9. tao proxy 把 ViewModel 投递回主线程
   │ 10. 主线程 evaluate_script("window.__lemurclaw.dispatch(json)")
   ▼
React 收到新状态 → 重渲染
```

---

## 设计章节 2:Workspace 与 Crate 结构

### 2.1 总体结构

lemurclaw 独立 Cargo workspace;`xai-org/grok-build` 作为 git submodule 挂在 `vendor/grok-build/`,作 path 依赖引入,**不合并 workspace**(各自独立,上游内部 path 依赖自锁版本)。

```
lemurclaw/
├── .gitmodules                    # 指向 xai-org/grok-build @ 固定 commit
├── Cargo.toml                     # 手写 workspace root
├── vendor/grok-build/             # submodule,上游原样(不改)
├── patches/
│   ├── grok-build.patch           # 四环 patch(git format-patch 风格)
│   └── README.md                  # 锚点 commit + patch 意图 + rebase 指南
├── scripts/apply-patches.sh       # cd vendor/grok-build && git apply
├── crates/
│   ├── lemurclaw-runtime/         # 核心:可复用 agent runtime crate
│   ├── lemurclaw-gui-bridge/      # GUI Rust 侧:wry+tao + 四环驱动 + 投影
│   ├── lemurclaw-gui-frontend/    # React 源码 + Vite 构建产物(include_dir 嵌入)
│   ├── lemurclaw-config/          # 配置层(封装上游 config)
│   └── lemurclaw-bin/             # composition root 二进制
└── docs/superpowers/              # specs + plans
```

### 2.2 各 crate 职责

**`lemurclaw-runtime`(对外发布的核心 crate)**
暴露:
- `enum Frontend { Tui, Gui }`
- `pub async fn run(config: RuntimeConfig) -> Result<ExitCode>`
- `RuntimeConfig { agent_name, cwd, model, frontend, permission_mode, yolo, ... }`

Tui 路径转调 `xai_grok_pager::app::run`;Gui 路径转调 `lemurclaw_gui_bridge::run`。**不含 wry/ratatui**(由前端 crate 拉入,保持可嵌入)。embedding 用例:
```rust
lemurclaw_runtime::run(RuntimeConfig {
    agent_name: "my-bot".into(),
    cwd: Some("/code".into()),
    frontend: Frontend::Gui,
    ..Default::default()
}).await?;
```

**`lemurclaw-gui-bridge`**
wry+tao 集成、`ipc_handler` 收 Action、调四环驱动 AppView、`project_view` 投影、tao proxy 跨线程推 ViewModel。依赖 `wry`+`tao`、patch 后的 `xai-grok-pager`、`lemurclaw-config`、`tokio`。**不依赖 ratatui**。

**`lemurclaw-gui-frontend`**
React/TS 源码 + `dist/` 构建产物。`build.rs` 在 Node 可用时触发 `npm run build`,否则用预构建 `dist/`。无 Rust 依赖,由 gui-bridge `include_dir!` 嵌入。

**`lemurclaw-config`**
封装上游 `xai_grok_shell::config` + lemurclaw 配置项;CLI(clap)→ RuntimeConfig;配置文件读写(`~/.lemurclaw/config.toml` 或项目级 `.lemurclaw.toml`)。

**`lemurclaw-bin`**
薄二进制,等价上游 `xai-grok-pager-bin`:解析 CLI、装 crash handler/telemetry/allocator、调 `lemurclaw_runtime::run`。产物名 `lemurclaw`。

### 2.3 依赖图

```
lemurclaw-bin → lemurclaw-runtime ─┬─ xai-grok-pager (patched, vendored)
                                   ├─ xai-grok-shell / xai-grok-config (vendored)
                                   ├─ lemurclaw-config
                                   └─ [frontend]
                                        ├─ tui:  xai-grok-pager::app::run
                                        └─ gui:  lemurclaw-gui-bridge ─┬─ wry + tao
                                                                      ├─ xai-grok-pager (patched)
                                                                      └─ lemurclaw-gui-frontend (include_dir)
```

crate 名前缀统一 `lemurclaw-`。

---

## 设计章节 3:上游 Patch 契约 —— 四环全暴露(路 1)

### 3.1 等效性的真实基础

GUI 复用上游**完整反馈循环**(不重写执行体),行为与 TUI 一致由"共享同一份代码"保证,不靠人工对齐。

上游反馈循环(GUI 必须完整复用):
```
        ┌─────────────────────────────────────────────────┐
        │                                                  ▼
  dispatch::dispatch    effects::execute    JoinSet.spawn
  (Action→Vec<Effect>)  (Effect→spawn任务)     (异步执行)
  环① 同步纯逻辑         环② 异步执行            │
        │                                          │
        │                                          ▼
        │                              tasks.join_next()
        │                                          │
        │                                          ▼
        └──── Action::TaskComplete ◄── dispatch_task_result (环③)
              (结果回灌成新 Action)     (TaskResult→Vec<Effect>)
                                                   ▲
                                                   │
                            acp_handler::handle (环④,入站 ACP session/update → 状态变更)
```

### 3.2 勘察事实(决定性)

上游 `Action`/`Effect`/`TaskResult` **类型**已是 `pub`;但四个**行为函数**全是 `pub(crate)` 或更严:

| 环节 | 位置 | 当前可见性 |
|---|---|---|
| `dispatch::dispatch` | `dispatch/mod.rs` L52 `pub(crate) use router::dispatch;` | `pub(crate)` |
| `effects::execute` | `effects/mod.rs` | `pub(crate)` |
| `dispatch::dispatch_task_result` | `dispatch/task_result.rs` | `pub(super)` |
| `acp_handler::handle` | `acp_handler/mod.rs` | `pub(crate)` |

只暴露 dispatch(原 B1)不够——GUI 拿到 Effect 却无法用上游方式执行。故选路 1:四环全暴露。

### 3.3 Patch 内容(四部分)

**Part 1 — 暴露反馈循环四环(可见性,~4 处)**
- `dispatch/mod.rs`:`pub(crate) use router::dispatch;` → `pub`
- `effects/mod.rs`:`pub(crate) fn execute(...)` → `pub`
- `dispatch/task_result.rs`:`pub(super) fn dispatch_task_result(...)` → `pub`
- `acp_handler/mod.rs`:`pub(crate) fn handle(msg, app) -> bool` → `pub`

类型(`Action`/`Effect`/`TaskResult`)已 `pub`,无需改。`AgentSession.acp_tx` 字段已 `pub`,GUI 能拿到 ACP 发送句柄。

**Part 2 — 暴露 `AppView` 构造与读路径(~10-20 处)**
`AppView` 结构体已 `pub`,但构造/字段多为 `pub(crate)`。做法:
- 复用 `minimal_api` facade 读字段(已有 ~70 个 `pub fn`,零 patch)
- `minimal_api` 未覆盖的 GUI 必需字段,在 patch 里增补 `pub fn`
- 暴露构造入口 `pub fn new_app_view(args, acp_connection) -> AppView`(或复用上游构造路径改 pub)

**Part 3 — 新增 webview IoC seam(新文件,仿 `minimal/hook.rs`)**
`app/webview_hook.rs`,与 `minimal/hook.rs` 同构,但**不带 `PagerTerminal`**:
```rust
pub type WebViewRenderFn = fn(&AppView) -> serde_json::Value;
pub struct WebViewHooks { pub render: WebViewRenderFn }
static HOOKS: OnceLock<WebViewHooks> = OnceLock::new();
pub fn install(h: WebViewHooks) { let _ = HOOKS.set(h); }
pub fn hooks() -> Option<&'static WebViewHooks> { HOOKS.get() }
```
fn 指针 + `OnceLock`,与上游 `MinimalHooks` 风格一致,避免 cargo cycle。区别于 `minimal_hook` 的 `fn(&mut AppView, &mut PagerTerminal)`(crossterm 绑定)。

**Part 4 — 增补 `minimal_api` 读字段(增量,~5-15 条)**
按 GUI 实际实现需求迭代增补,初始预计对话/工具/Plan/diff/scrollback 的读路径。这部分**迭代式增长**,实现到哪个 view 就补哪个字段的读路径。

### 3.4 明确不改

| 不改 | 原因 |
|---|---|
| `MvpAgent` 字段/方法 | 最敏感、最易变;agent 通信走 ACP(经已 pub 的 `AgentSession.acp_tx`) |
| `event_loop::run` | 绑死 crossterm 输入 + ratatui 终端;GUI 自己有 wry 事件循环 |
| `views/*`、`scrollback/*`(ratatui widgets) | ratatui 形状,React 重写 |
| `PagerTerminal`/`CrosstermBackend` | 渲染后端,GUI 不走 |
| 上游根 `Cargo.toml`(生成物) | 避免与生成器冲突 |

### 3.5 GUI 的循环体(对照 TUI 的 `event_loop::run`)

GUI crate 自己写 `gui_loop`,与 TUI 的 `event_loop::run` 结构对仗,**差别仅在:输入源(crossterm→wry)、渲染(`draw(terminal)`→投影推 React)**:
```rust
let mut app = new_app_view(args, acp_connection);   // Part 2 构造
let mut tasks = JoinSet::new();
loop {
    tokio::select! {
        action = rx_gui_action.recv() => {           // 来自 wry ipc_handler
            for eff in dispatch::dispatch(action, &mut app) {        // 环①
                effects::execute(eff, &mut tasks, &app.acp_tx, ...); // 环②
            }
        }
        Some(jr) = tasks.join_next() => {
            let effs = dispatch::dispatch(Action::TaskComplete(jr.unwrap()), &mut app);
            for eff in effs { effects::execute(eff, &mut tasks, ...); }
            // TaskResult 经 dispatch 内部走 dispatch_task_result 环③
        }
        msg = acp_rx.recv() => { acp_handler::handle(msg, &mut app); } // 环④
    }
    let vm = project_view(&app);                     // Part 2/4 读路径
    proxy.send(VmEvent(vm));                         // tao proxy → 主线程
}
// 主线程 EventLoop::user_event:
//   app.webview.as_ref().unwrap().evaluate_script(
//     &format!("window.__lemurclaw.dispatch({})", serde_json::to_string(&vm)?))
```

### 3.6 Patch 体量与维护

**体量:** 初始 ~100 行,随实现增长至 ~150-200。

**载体:** `patches/grok-build.patch`(git format-patch 风格)+ `patches/README.md`(锚点 commit + 意图 + rebase 指南)+ `scripts/apply-patches.sh`(`cd vendor/grok-build && git apply ../../patches/*.patch`)。

**bump 上游流程:** `git -C vendor/grok-build checkout <new-commit>` → `scripts/apply-patches.sh`(冲突人工调整 patch)→ `cargo test` 验证 → 提交 submodule 新 commit + 更新后的 patch。初始锁定上游当前 main HEAD(实现时取最新稳定 commit,记入 `patches/README.md`)。

### 3.7 诚实风险声明

`effects::execute` 签名含 `JoinSet`/`AcpAgentTx`/`SessionFlags`/`progress_tx`——比 `dispatch` 复杂得多,是四环中**最易变、rebase 冲突最高频**的点。patch 面大于原 B1(~80→~150-200),已由用户明确接受(等效性优先)。

---

## 设计章节 4:GUI 布局

### 4.1 50 view → 7 区域

| 区域 | 上游 view | GUI 对应 |
|---|---|---|
| **A 对话主体** | `agent.rs`、`scrollback/*`(block/blocks/entry/layout/render/selection/sticky) | 中央对话流 + 工具调用块 + diff 块 |
| **B 输入区** | `prompt_widget/*`、`prompt_suggestion`、`completion_dropdown`、`slash_dropdown`、`context_bar`、`shortcuts_bar` | 底部输入框 + 斜杠下拉 + 上下文 + 快捷键栏 |
| **C 状态栏** | `status_bar`、`status_blocks`、`credit_bar` | 底部状态栏(模型/目录/额度) |
| **D 编排面板** | `subagent_catalog_pane`、`tasks_pane`、`queue_pane`、`todo_pane`、`timeline`、`roster`、`agents_modal`、`subagent` | 右侧/弹出:子 agent、任务、队列、todo、时间线 |
| **E 模态/审批** | `plan_approval_view`、`permission_view`、`question_view` | 模态:Plan 审批、权限确认、问题应答 |
| **F 导航/选择** | `dashboard/*`、`welcome/*`、`session_picker`、`history_search`、`file_search/*`、`project_picker/*` | 全屏视图:仪表盘、欢迎、会话/文件/项目选择 |
| **G 配置/管理** | `settings_modal`、`mcps_modal`、`extensions_modal`、`memory_modal`、`import_claude_modal`、`block_viewer`、`diff`、`rewind`、`jump`、`picker`、`overlay` | 模态:设置/MCP/插件/记忆;查看器:diff/rewind/block |

### 4.2 主对话窗口 mockup

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  lemurclaw — my-agent  ⏷  [~/code/proj]  ⚙                                    │  顶栏(标题+agent名+目录+菜单)
├────────────────────────────────────────────────────┬─────────────────────────┤
│                                                    │  ◐ 编排面板(可折叠)     │
│  ▎user  14:23                                      │  ───────────────────    │
│  帮我重构这个函数                                    │  子 agent (3)            │
│                                                    │   ● researcher  运行中   │
│  ▎assistant                                        │   ○ coder       等待     │
│  我来分析这个函数的结构…                              │   ○ reviewer    空闲     │
│                                                    │  ───────────────────    │
│  ┌─ 🔧 edit_file ──────────────────────────┐      │  任务队列                 │
│  │ src/parser.rs                            │      │   ▶ 1/3 解析依赖         │
│  │  - fn parse(input) {                     │      │   ⬚ 2/3 重写            │
│  │  + fn parse(input, opts) {               │      │   ⬚ 3/3 验证            │
│  │  ┊  15 行 diff                           │      │  ───────────────────    │
│  └─ [接受] [拒绝] [查看完整diff] ──────────────┘      │  Todo                    │
│                                                    │  [x] 定位函数            │
│  ▎assistant  ▎thinking…                             │  [x] 识别模式            │
│                                                    │  [ ] 提取参数            │
│                                                    │  [ ] 重写                │
├────────────────────────────────────────────────────┴─────────────────────────┤
│  @context: 3 files · 📎 context_bar  ·  / 斜杠命令                              │  上下文/斜杠
├──────────────────────────────────────────────────────────────────────────────┤
│  ▎ > 输入消息…                                                    [Enter 发送] │  输入框
├──────────────────────────────────────────────────────────────────────────────┤
│  grok-4.5 · ~/code/proj · 42% credit · ⏎发送 ⌘K命令 ⌘/斜杠 · Ctrl+C中断        │  状态栏
└──────────────────────────────────────────────────────────────────────────────┘
```

### 4.3 关键交互形态

**审批卡片(Plan Mode,对话流内,非弹窗):**
```
  ┌─ 📋 Plan 审批 ────────────────────────────────┐
  │ 1. 读取 src/parser.rs, src/tokenizer.rs        │
  │ 2. 提取 parse() 的 3 处调用点                    │
  │ 3. 重写 parse(input, opts) 并更新调用点          │
  │ 4. 跑 cargo test 验证                           │
  │                                    [拒绝] [批准] │
  └─────────────────────────────────────────────────┘
```

**权限确认(对话流内):**
```
  ┌─ 🔐 权限请求 ──────────────────────────────────┐
  │ assistant 想执行: rm -rf target/                │
  │ 工作目录: ~/code/proj                           │
  │                          [拒绝] [本次允许] [总是] │
  └─────────────────────────────────────────────────┘
```

**子 agent 视图(D 区展开/弹出):**
```
  ┌─ 子 agent: researcher ─────────────── [×] ─┐
  │ ▎ researcher  正在搜索: "parse 函数调用点"    │
  │ ▎ researcher  找到 3 处,正在读取上下文…       │
  │ ▎ researcher  ✅ 完成,返回 3 个调用点          │
  └─────────────────────────────────────────────┘
```

### 4.4 导航路由(F 区——独立全屏视图)

```
启动 → welcome(首次) / dashboard(后续)
         │
         ├─ 新会话 → 主对话窗口
         ├─ 选择会话 → session_picker → 主对话窗口
         ├─ 选择项目 → project_picker → 新会话
         └─ 历史搜索 → history_search → 跳转会话
```

### 4.5 响应式策略

- **宽屏(>1200px)**:主对话 + 右侧编排面板(D 区常驻)
- **中屏(800-1200px)**:主对话,D 区按需浮出(drawer)
- **窄屏(<800px)**:主对话全宽,D/E 区全屏 modal

### 4.6 设计要点

- **左主右辅**:对话占主体(A 区),编排面板(D 区)右侧、可折叠
- **工具调用内联**:diff/edit/search 作为对话流内折叠块,点击展开
- **审批内联**:Plan/权限/问题(E 区)以对话流内高亮卡片,与 TUI inline 审批一致
- **F 区独立路由**:dashboard/welcome/session-picker 整屏替换主窗口,不叠加

---

## 设计章节 5:IPC 协议 + View-Model 投影

### 5.1 IPC 传输机制(wry 原生,无 WebSocket/端口/子进程)

| 方向 | wry 原语 | 用法 |
|---|---|---|
| React → Rust | `WebViewBuilder::with_ipc_handler(closure)` | JS 调 `window.ipc.postMessage(str)`,Rust 闭包收 |
| Rust → React | `WebView::evaluate_script(&self, js)` | Rust 执行 `window.__lemurclaw.dispatch(<json>)` |

`evaluate_script` 是 wry 唯一的 Rust→JS 通道,**已是最原生的机制**,无更原生的"window 挂载"。

### 5.2 句柄持有(核实 wry 0.55.1 源码后)

`WebView` 方法签名是 `&self`(只需共享引用),但内部用 `Rc` 非 `Arc`,**非 `Send`**,绑主线程。持有方式与 wry 官方文档示例一致:
```rust
struct GuiApp {
    window: Option<tao::window::Window>,
    webview: Option<wry::WebView>,                 // 主线程持有,字段
    agent_handle: tokio::task::JoinHandle<()>,     // agent 循环在 tokio 线程
    proxy: tao::event_loop::EventLoopProxy<VmEvent>, // 跨线程投递回主线程
}
```
tokio agent 循环 dispatch 产生 `ViewModel` → `proxy.send(VmEvent(vm))` → tao 主循环 `user_event` 回调 → `app.webview.as_ref().unwrap().evaluate_script(...)`。**无 Arc/Mutex**——句柄就存在结构体字段里;proxy 解决的是 `WebView` 非 `Send` 的线程归属问题,不是句柄获取问题。

### 5.3 消息格式(单一信封,双向复用)

```typescript
type IpcMessage =
  | { kind: "action";   action: Action }       // React → Rust:用户动作
  | { kind: "viewmodel"; vm: ViewModel }        // Rust → React:状态投影
  | { kind: "event";    event: RuntimeEvent }   // Rust → React:一次性事件
  | { kind: "ready" }                           // React → Rust:前端就绪
  | { kind: "log";      level, msg }            // 双向:诊断
```

### 5.4 Action 协议(React → Rust)

上游 `Action` enum 的**用户可触发子集**(~30 个),React 不实现任何业务逻辑,只产生上游认识的 Action:
```typescript
type Action =
  // 会话
  | { t: "CreateSession"; cwd: string; model?: string }
  | { t: "LoadSession"; sessionId: string }
  | { t: "Prompt"; text: string }
  | { t: "Interrupt" }
  | { t: "Quit" }
  // 审批
  | { t: "ApprovePlan" } | { t: "RejectPlan" }
  | { t: "ApprovePermission"; mode: "once"|"always" } | { t: "RejectPermission" }
  | { t: "AnswerQuestion"; text: string }
  // 输入区
  | { t: "SetSlashCommand"; cmd: string }
  | { t: "AttachContext"; files: string[] }
  // 导航
  | { t: "Navigate"; route: Route }
  | { t: "OpenModal"; modal: ModalKind }
  | { t: "CloseModal" }
  // 配置
  | { t: "SetAgentName"; name: string }
  | { t: "SetWorkingDir"; path: string }
```
内部 Action(`TaskComplete` 等)对前端不可见——等效性的前端侧保证。

### 5.5 ViewModel 协议(Rust → React)

AppView 经 `minimal_api` 投影成 React 友好 JSON,**剔除 ratatui widget 形状**:
```typescript
type ViewModel = {
  session:    { id: string; cwd: string; agentName: string; model: string } | null
  scrollback: ScrollbackBlock[]
  turn:       { state: "idle"|"thinking"|"tool"|"done"; interruptible: boolean }
  pendingPlan?:       { steps: PlanStep[] } | null
  pendingPermission?: { tool: string; args: string; cwd: string } | null
  pendingQuestion?:   { prompt: string } | null
  agents:   AgentInfo[]
  tasks:    TaskInfo[]
  todos:    TodoItem[]
  timeline: TimelineEntry[]
  input:    { text: string; contextFiles: string[]; slashOpen: boolean; suggestions: Suggestion[] }
  status:   { model: string; cwd: string; creditPct: number; shortcuts: Shortcut[] }
  route:    Route
  modal:    ModalKind | null
}

type ScrollbackBlock =
  | { kind: "user";      text: string; ts: number }
  | { kind: "assistant"; text: string; ts: number }
  | { kind: "thinking";  text: string }
  | { kind: "tool";      tool: string; args: any; result?: any; status: "running"|"done"|"error" }
  | { kind: "diff";      file: string; hunks: Hunk[]; accepted?: boolean }
  | { kind: "error";     text: string }
```

投影函数 `project_view(&AppView) -> ViewModel` 是纯读、无副作用;每个 `project_*` 调 `minimal_api` 已有 `pub fn`,未覆盖字段在 patch Part 4 增补。

### 5.6 更新策略

| 阶段 | 策略 | 理由 |
|---|---|---|
| 初始 / route 切换 | 全量推 ViewModel | React 首次渲染需要完整状态 |
| 每次 dispatch 后 | 全量推 ViewModel(MVP) | 简单,正确性优先;view-model 通常 <50KB,桌面 webview 可接受 |
| 优化期(可选,子项目 8) | 增量 diff 推送 | Rust 维护前一份 ViewModel,diff 后只推变化的路径 |

### 5.7 渲染节流

Rust 侧 **16ms(60fps)节流**:dispatch 多次只在下一帧合并推一次完整 ViewModel;React 侧用 `useSyncExternalStore` 接入,自动批处理。

### 5.8 错误处理

- **agent 错误融入 scrollback**:作为 `ScrollbackBlock::error` 出现,与 TUI 一致(TUI 也把错误显示在 scrollback)。
- **IPC 层自身错误**(反序列化失败等):用 `RuntimeEvent::Error`。

### 5.9 安全

- `ipc_handler` 用 serde 反序列化上游 `Action`,类型不匹配直接丢弃(React 无法构造非法 Action)。
- `evaluate_script` 的 json 经 serde 严格序列化;用户文本中的 HTML 由 React 默认转义处理。
- 前端资源 `include_dir!` 嵌入,无 eval、无远程加载。

---

## 设计章节 6:范围分解 + 测试 + 等效性验证

### 6.1 8 子项目(各自独立 spec→plan→实现循环)

```
子项目 0:基础骨架(patcher + workspace + runtime crate 空壳 + bin)
    │   产出:可构建、submodule + patch 可 apply、--frontend tui 跑通上游
    ▼
子项目 1:四环 patch + view-model 投影基础
    │   产出:dispatch/execute/task_result/acp_handler 暴露,project_view 框架,
    │        Rust 侧能构造 AppView 并 dump 出 ViewModel JSON
    ▼
子项目 2:GUI 基础设施(wry+tao + tao proxy + React 骨架 + IPC 信封)
    │   产出:空 React 页面能收 ViewModel、能发 Action、循环跑通
    ▼
子项目 3:核心对话循环(A 区 + B 区 + C 区)
    │   scrollback + prompt + 工具调用块 + diff 块 + 状态栏
    │   产出:可用 GUI 跑一轮完整 agent 对话,行为对等 TUI
    ▼
子项目 4:编排面板(D 区)
    │   subagent roster / tasks / queue / todo / timeline
    ▼
子项目 5:审批与模态(E 区)
    │   Plan 审批 / 权限确认 / 问题应答
    ▼
子项目 6:导航全屏视图(F 区)
    │   dashboard / welcome / session-picker / file-search / project-picker / history-search
    ▼
子项目 7:配置与管理模态(G 区)
    │   settings / mcps / extensions / memory / import-claude / block-viewer / rewind / diff 查看器
    ▼
子项目 8:发布就绪
    │   crates.io 发布 runtime crate、文档、双前端录放测试套件、增量投影优化
```

**本 spec 覆盖整个架构(子项目 0-8 的蓝图)。** 第一个实现计划聚焦**子项目 0+1**(基础骨架 + patch),因为它们是后续一切的前提。

### 6.2 view 对等映射(~50 view → ~28 React 组件)

多上游 view 合并到一个组件:

| 区 | React 组件 | 上游 view(参考) | ViewModel 字段 |
|---|---|---|---|
| A | `<Scrollback>` | `scrollback/blocks/*` | `scrollback: ScrollbackBlock[]` |
| A | `<ToolCallBlock>` | `tool_usage`、`agent.rs` 内联 | `scrollback[].kind=tool` |
| A | `<DiffBlock>` | `diff.rs`、`block_viewer` | `scrollback[].kind=diff` |
| B | `<PromptInput>` | `prompt_widget/*` | `input.text` |
| B | `<SlashDropdown>` | `slash_dropdown`、`slash` | `input.slashOpen`、`suggestions` |
| B | `<ContextBar>` | `context_bar` | `input.contextFiles` |
| B | `<ShortcutsBar>` | `shortcuts_bar` | `status.shortcuts` |
| C | `<StatusBar>` | `status_bar`、`status_blocks`、`credit_bar` | `status` |
| D | `<AgentRoster>` | `roster`、`agents_modal` | `agents` |
| D | `<TasksPane>` | `tasks_pane`、`queue_pane` | `tasks` |
| D | `<TodoPane>` | `todo_pane` | `todos` |
| D | `<Timeline>` | `timeline` | `timeline` |
| D | `<SubagentView>` | `subagent`、`subagent_catalog_pane` | `agents[].detail` |
| E | `<PlanApproval>` | `plan_approval_view` | `pendingPlan` |
| E | `<PermissionPrompt>` | `permission_view` | `pendingPermission` |
| E | `<QuestionView>` | `question_view` | `pendingQuestion` |
| F | `<Dashboard>` | `dashboard/*` | `route=dashboard` |
| F | `<Welcome>` | `welcome/*` | `route=welcome` |
| F | `<SessionPicker>` | `session_picker`、`history_search` | `route=session-picker` |
| F | `<FileSearch>` | `file_search/*` | `modal=file-search` |
| F | `<ProjectPicker>` | `project_picker/*` | `route=project-picker` |
| G | `<SettingsModal>` | `settings_modal` | `modal=settings` |
| G | `<McpsModal>` | `mcps_modal` | `modal=mcps` |
| G | `<ExtensionsModal>` | `extensions_modal` | `modal=extensions` |
| G | `<MemoryModal>` | `memory_modal` | `modal=memory` |
| G | `<ImportClaudeModal>` | `import_claude_modal` | `modal=import-claude` |
| G | `<BlockViewer>` / `<Rewind>` / `<Jump>` / `<Picker>` / `<Overlay>` | 同名 | route/modal |

### 6.3 等效性验证三层

1. **循环体结构对照(静态):** `gui_loop` 与 `event_loop::run` 的 `tokio::select!` 分支逐行 diff,确认调同一组 patch 后的函数(`dispatch`/`execute`/`dispatch_task_result`/`acp_handler::handle`)、顺序一致。防"复用外表、实际重写逻辑"。
2. **ViewModel 快照录放(行为):** 录一组 ACP 事件序列(流式 token、工具调用、Plan、权限、错误、多 turn),同序列分别驱动 TUI 与 GUI,比对投影出的 ViewModel 关键字段快照。数据在 `tests/fixtures/acp-recordings/`。自动捕获行为分叉。
3. **无头 webview 截图(用户可感):** 驱动 GUI 完成对话,关键帧截图与 TUI 视觉对比(人工或像素差异阈值)。成本高,只覆盖核心场景。

### 6.4 Rust 侧测试

| 层 | 内容 |
|---|---|
| 单元 | `project_*` 投影函数:构造 mock `AppView`,断言投影 JSON 字段正确 |
| 单元 | patch 不破坏上游:`cargo test -p xai-grok-pager`(在 vendor/patched 上跑上游原测试) |
| 集成 | `gui_loop` 驱动:喂录制的 ACP 序列,断言产出的 ViewModel 快照 |
| 集成 | `apply-patches.sh` 幂等性:bump submodule 后 patch 能干净 reapply |

### 6.5 React 侧测试

| 层 | 内容 |
|---|---|
| 组件 | 各 `<Component>` 用 `@testing-library/react`,断言给定 ViewModel 渲染正确 |
| IPC 客户端 | mock `window.ipc`,断言用户交互产生正确的 Action 消息 |
| 快照 | 关键组件的渲染快照(`jest` snapshot),防回归 |

### 6.6 风险登记

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| `effects::execute` 签名随上游变更,patch rebase 冲突 | 高 | 中 | patch README 记录锚点;CI 在 bump 时跑全测试;冲突时优先适配签名而非重写 |
| `minimal_api` 不覆盖的字段需大量增补 | 中 | 中 | Part 4 按需增补,控制范围;投影只取 GUI 真正需要的字段 |
| React 重渲染性能(大 scrollback + 全量推送) | 中 | 中 | 16ms 节流 + `useSyncExternalStore`;子项目 8 做增量投影 |
| webview 跨平台差异(WKWebView/WebView2/WebKitGTK) | 中 | 低 | MVP 先锁单一平台(macOS WKWebView),其余平台后续 |
| 上游 `xai-grok-pager` 版本快速迭代(API 破坏) | 中 | 高 | 锁定 submodule commit;release 节奏滞后上游 |
| "完整对等"工作量被低估 | 高 | 高 | 分 8 子项目;子项目 3(核心对话)成功后再评估后续节奏 |

### 6.7 不在范围内(YAGNI)

- ❌ 自建 TUI(复用上游)
- ❌ 移动端(只桌面 webview)
- ❌ 远程/多客户端(进程内单实例)
- ❌ 自定义主题编辑器(读上游主题即可)
- ❌ 增量 ViewModel diff 推送(MVP 全量,子项目 8 再优化)
- ❌ Tauri 整套方案(用裸 wry+tao)
- ❌ 自动跟随上游 main(手动 bump + rebase)

---

## 附录:决策溯源

1. **claurst → grok-build 转向:** 仓库原有 clean-room 设置目标是 claurst(GPL-3.0),与本次需求(grok-build,Apache-2.0)矛盾。用户决定转向 grok-build、放弃 claurst(未写代码,无损)。
2. **方案 A/B/C → B1 → 路 1:** 勘察发现上游反馈循环 5 环中 4 环 `pub(crate)` 锁住,原 B1 小 patch 不能保证等效。用户选路 1:四环全暴露,真正复用完整循环。
3. **wry 句柄:** 用户追问"Rust→React 能否复用 wry window 挂载"。核实源码后:`evaluate_script` 是唯一 Rust→JS 通道;句柄存结构体字段;tao proxy 解决的是 `WebView` 非 `Send` 的线程归属,非句柄获取。
