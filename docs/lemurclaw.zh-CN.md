# Lemurclaw —— 架构与构建指南

Lemurclaw 是 [openai/codex](https://github.com/openai/codex) 的 fork,在 Codex agent
基础上新增了原生 GUI 栈和 crates.io 发布工具链。本文档覆盖 lemurclaw 区别于上游的
三个方面:

1. **GUI 栈** —— 四个新 crate,把 Codex 作为桌面窗口或浏览器应用运行。
2. **xtask 发布工具链** —— 把 `codex-*` 重命名为 `lemurclaw-*`、把 40+ 个 crate 合并
   成 6 个、改写品牌、并可选地去掉 V8 运行时以得到更小的二进制。
3. **品牌改写** —— 范围可控的 `Codex` → `lemurclaw` 改写,保护那些绝不能改的 OpenAI
   云端集成 token。

下面所有内容都与未修改的上游代码并存于源码树中。发布工具**绝不修改原始 `codex-rs/`
树**,所有产物都落在 `lemurclaw-rs/`(仅 `target/` 被 gitignore)。

> English: see [lemurclaw.md](./lemurclaw.md).

---

## 1. GUI 栈

四个 crate,都是 `codex-rs/Cargo.toml` 的成员:

| Crate | 角色 |
|-------|------|
| `lemurclaw` | launcher / 二进制。解析 `--frontend {tui\|gui\|webui}` 并分发。 |
| `lemurclaw-gui` | 通过 `wry` + `tao` 实现的原生桌面窗口,内嵌共享 React 应用。 |
| `lemurclaw-webui` | 通过 `axum` HTTP + WebSocket 实现的浏览器 UI,内嵌同一份 React 应用。 |
| `lemurclaw-transport` | 共享的传输类型(不带 `wry`/`tao` 重依赖)。 |

依赖方向:`lemurclaw` → `lemurclaw-gui` → `lemurclaw-webui`(叶子)。`lemurclaw-transport`
被三者共用。

### Launcher(``lemurclaw/src/``)

`main.rs` 解析 `Cli`(`config.rs`),调用 `run(config)`(`lib.rs:45`)。`Frontend` 枚举
选择前端:

- `Tui`(默认)→ 剥离 lemurclaw 专用 flag,把 argv 直接透传给 `codex_tui::run_main`,
  因此 Codex 的所有 flag 仍然可用。
- `Gui` → `lemurclaw_gui::run_gui()`。
- `Webui` → 拒绝非 loopback 绑定(WS 桥没有鉴权),然后
  `lemurclaw_webui::run_webui(host, port)`。

lemurclaw 自有(不透传给 TUI)的 CLI flag:
`--frontend`、`--agent-name`、`--cwd/-C`、`--model/-m`、`--yolo`、`--host`、`--port`。

### `lemurclaw-gui`(wry + tao)

`run_gui()`(`lemurclaw-gui/src/lib.rs:60`):

1. 在**主线程**上建 tao `EventLoop`(macOS/Windows 要求窗口在主线程)。
2. 在独立的 OS 线程上跑 tokio runtime + `InProcessAppServerClient`。
3. 创建 wry `WebView`:
   - 自定义 `lemurclaw://` 协议,serve 内嵌的 React `dist/`。
   - 初始化脚本注入 `window.__lemurclaw = { onEvent, onResponse }`。
   - IPC handler 把 JS `postMessage` 转成后端请求。
4. 事件循环通过 `evaluate_script("window.__lemurclaw.onEvent(...)")` 把后端 JSON 事件
   推回 JS。

线程拓扑:主线程持有 WebView;工作线程持有 tokio runtime。主 → runtime 用
`Handle::spawn`;runtime → 主 用 `EventLoopProxy::send_event`。

### `lemurclaw-webui`(axum + WebSocket)

`run_webui(host, port)`(`lemurclaw-webui/src/lib.rs:33`)启动一个 **16 MB 栈**的多线程
tokio runtime(Codex-core 有深递归,默认 2 MB 栈会崩溃),阻塞在 `serve()`
(`server.rs:68`):

- `GET /ws` —— 到同进程 AppServer 的 WebSocket 桥。
- `GET /`、`GET /{*path}` —— 内嵌的静态 React 资源。
- `GET /readyz`、`GET /healthz` —— 存活探针。

单连接模型:`InProcessAppServerClient` 放在 `Mutex` 后。首个 WS session 接收
`next_event()` 事件流;并发 session 能发请求,但收不到推送事件。

### 共享 React 前端(``lemurclaw-webui/assets/``)

React 18 + Vite 5 + TypeScript 5.6 + vitest。`vite.config.ts` 设 `base: './'`,因为应用
在合成的 `lemurclaw://app/` origin 下加载(绝对路径会失效)。入口:`src/main.tsx` →
`app/App.tsx`。关键组件:`Composer`、`ApprovalCard`、`Scrollback`、`Sidebar`、
`ModelPicker`、`DiffViewerModal`,外加 `cells/`、`composer/`、`settings/`、`sidebar/`
子目录。

### 前端 ↔ 后端传输(``assets/src/transport.ts``)

唯一的传输层。用 `hasBridge()` 自动探测宿主:

```typescript
export function hasBridge(): boolean {
  return typeof window.ipc !== 'undefined';
}
```

- **GUI(wry)模式**:Rust 注入 `window.ipc.postMessage`;`send()` 调用它。Rust 通过
  `window.__lemurclaw.onEvent`(推送事件)和 `.onResponse`(JSON-RPC 响应)回调。
- **浏览器(webui)模式**:没有 `window.ipc`;导入时开 WebSocket 连到
  `ws://<host>:<port>/ws`。socket 打开前的帧会入队。

入站分发(`handleInbound`):JSON-RPC 信封(`{jsonrpc:"2.0", id}`)按 `id` 匹配待决的
`sendRequest` Promise;其余都转发给 `onEventCb`。`sendRequest` 本地分配 id(从 1000 起),
30 秒超时。

### 同进程 AppServer(``lemurclaw-webui/src/codex_glue.rs``)

GUI 和 webui 都通过 `InProcessAppServerClient` 把 Codex AppServer 内嵌在同进程,而不是
走 socket:

- `build_dev_client()` 用 `EnvironmentManager::default_for_tests()`、
  `SessionSource::Cli`、`experimental_api: true`、`client_name: "lemurclaw"` 启动它。
- `serialize_server_event` / `parse_client_request` / `build_response_envelope` 构成两个
  后端共享的 wire 契约,因此 GUI 和 webui 说同一种 JSON-RPC 方言。

---

## 2. xtask 发布工具链

`xtask/` 是一个独立的 crate(不是 `codex-rs/` 的成员)。它绝不修改源码树 —— 所有输出都
落到 `lemurclaw-rs/`。命令:

| 命令 | 阶段 | 作用 |
|---------|-------|--------------|
| `verify size` | 0 | 报告最大几个 crate 的压缩 `.crate` 体积 vs 10 MB 上限。 |
| `verify patches` | 0 | 探测 `[patch.crates-io]` 的 fork 是否真的必需。 |
| `publish rename` | 1 | 把 `codex-rs/` 复制到 `lemurclaw-rs/`,在 manifest 和 `use` 语句里把 `codex-*` 重命名为 `lemurclaw-*`,从 lib+bin crate 丢弃 `[[bin]]`(只保留 lib),丢弃 `[patch.crates-io]`。最后跑 `cargo check --workspace` 验证。 |
| `publish fork {clone,prepare,publish,rewire}` | 1.5 | 把 8 个 git fork 发布为 `lemurclaw-*`(保留 `[lib].name` 使 `use ratatui::` 不变),并改写 workspace 引用它们。 |
| `publish bundle --target <X>` | 2-4 | 把一组 crate 合并成一个 mega-crate(每个原 crate 变成 `pub mod`)。target:`Utils`、`Core`(84-crate 闭包)、`Extensions`、`Server`、`Tui`、`Cli`。 |
| `publish rebrand-full` | — | 全树 `Codex` → `lemurclaw` 改写(见第 3 节)。可在已有的 `lemurclaw-rs/` 树上重复跑。 |
| `publish strip-v8` | — | 把 `code-mode` 替换成无 V8 的 stub,删除 `code-mode-host` + `v8-poc`,去掉 `v8`/`deno_core_icudata` 依赖,重新生成 lockfile。省约 120 MB。 |
| `publish restore-v8` | — | 撤销 `strip-v8`。 |

### 标准流水线

```bash
cd xtask
cargo run -q -- publish rename            # 阶段 1
cargo run -q -- publish bundle --target core        # 阶段 2/3
cargo run -q -- publish bundle --target extensions  # 阶段 4
cargo run -q -- publish bundle --target server
cargo run -q -- publish bundle --target tui
cargo run -q -- publish bundle --target cli
cargo run -q -- publish fork rewire      # 阶段 1.5
cd ../lemurclaw-rs && cargo check --workspace --offline
```

可选的去 V8(在 `rename` 之后、`bundle` 之前或之后跑):

```bash
cargo run -q -- publish strip-v8
```

耗时:`rename` 约 2 分钟,`bundle --target core` 约 2 分钟,首次
`cargo check --workspace --offline` 20-40 分钟(mega-crate 编译),之后 <1 分钟。

### 最终结构(完整 bundle 后)

| Crate | 内容 |
|-------|----------|
| `lemurclaw-core` | 合并 84+ crate(core、protocol、utils、…) |
| `lemurclaw-extensions` | 9 个扩展 crate(agent、connectors、goal、guardian、image-gen、mcp、memories、skills、web-search) |
| `lemurclaw-server` | 18 个 server 层 crate(app-server、transport、daemon、client、arg0、chatgpt、…) |
| `lemurclaw-tui` | tui 宿主 + ansi-escape + message-history |
| `lemurclaw` | cli 宿主 + cloud-tasks、exec、mcp-server、…(lib + bin,同名) |
| `lemurclaw-experimental-api-macros` | proc-macro(独立) |

外加 8 个发布到 crates.io 的 fork crate:`lemurclaw-{crossterm, ratatui,
tungstenite, tokio-tungstenite, nucleo, runfiles, ansi-to-tui, ratatui-macros}`。

### 从 `lemurclaw-rs/` 运行 GUI(推荐)

这是运行 lemurclaw 的推荐方式。`rename` + `strip-v8` 产出一个自包含的 `lemurclaw-rs/`
workspace,GUI crate 作为一等成员(launcher 保留 `[[bin]]`),不带 V8 依赖:

```bash
cd xtask
cargo run -q -- publish rename     # codex-rs/ → lemurclaw-rs/,重命名 crate
cargo run -q -- publish strip-v8   # 把 code-mode 替换成无 V8 的 stub
cd ../lemurclaw-rs
cargo build -p lemurclaw
./target/debug/lemurclaw --frontend webui --port 8080   # 浏览器
./target/debug/lemurclaw --frontend gui                 # 原生窗口
```

为什么用 `lemurclaw-rs/` 而不是 `codex-rs/`:GUI crate 作为 `codex-rs/` 成员存在是为了开发
方便,但在那里编译 launcher 会拉入完整 V8 运行时(约 120 MB 下载)。`lemurclaw-rs/` 是
lemurclaw 自包含的 workspace;rename + strip-v8 给你一个不带 V8、能跑 TUI/GUI/WebUI 的
二进制。

`strip-v8` 后从 `lemurclaw-rs/` 构建的 `lemurclaw` 能驱动 TUI/GUI/WebUI 前端做正常对话,只
缺 V8 代码执行(stub 在真正请求 code-mode cell 时返回错误)。

---

## 3. 品牌改写

`rewrite_brand_full`(`xtask/src/bundle.rs:3948`)在 `lemurclaw-rs/` 树里把
`Codex`/`codex` 改写成 `lemurclaw`。它把工作分成 **A-zone**(改写)和 **B-zone**(保护,
保持不变)。

### A-zone —— 改写

| 类别 | 例子 |
|----------|----------|
| 环境变量**值**(引号字符串字面量) | `"CODEX_HOME"` → `"LEMURCLAW_HOME"`、`"CODEX_API_KEY"` → `"LEMURCLAW_API_KEY"`,40+ 条。常量*标识符*(`CODEX_HOME_ENV_VAR`)由 `source_rewrite` 的 AST pass 改。 |
| 文件系统路径 | `~/.codex` → `~/.lemurclaw`、`/etc/codex` → `/etc/lemurclaw`、`(".codex")` → `(".lemurclaw")` |
| CLI flag | `--codex-*` → `--lemurclaw-*`(emitter 和 matcher 两端都改,保证 self-re-exec 一致) |
| 协议 / 二进制标识 | `codex://` → `lemurclaw://`、protobuf 包 `codex.thread_config.v1` → `lemurclaw.…`、daemon client 名、arg0 helper 二进制名 |
| emit-only telemetry / OTEL | `codex.thread.*`、`codex.windows_sandbox.*`、OTEL `service.name` |
| 类型名(非 on-wire) | `CodexErrorInfo` → `LemurclawErrorInfo` |
| 散文品牌词 | 在非 `.rs` 文件里(prompts、tooltips、README、models.json)`\bCodex\b` / `\bcodex\b` → `lemurclaw` |

`source_rewrite.rs` 用 `syn` 做 AST 驱动的 Rust 标识符改写(`use codex_foo` →
`use lemurclaw_foo`),带逐行的兜底。展示文本(CLI `--help`、TUI 屏幕文字、错误、
banner)由 `rewrite_brand_display_text` 用精确的全串字面量配对处理。

### B-zone —— 保护(绝不能改)

Lemurclaw 直连 OpenAI 云端(`chatgpt.com/codex-backend`)。这些 token 在改写前被藏到
占位符后面、改写后还原,所以松散的 `\bCodex\b` 匹配永远碰不到它们:

- **模型 slug**(真实 API 模型名):`gpt-5.x-codex`、`gpt-5.x-codex-max`、…
- **originator header 值**:`codex_cli_rs`、`codex-tui`、`codex_vscode`、
  `codex_atlas`、`codex_chatgpt_desktop`、…
- **JWT audience**:`codex-app-server`(云端校验 `aud` claim)
- **`codex_exec` originator**(出站 header)
- **analytics `event_type` wire 值**:`codex_app_mentioned`、`codex_turn_event`、…
- **on-wire JSON 字段名**:`codexErrorInfo`
- **云端 URL / 基础设施**:`chatgpt.com/codex`、`com.openai.codex`、`codex-backend`、…

完整的替换表和范围规则见 `xtask-brand-rewrite-spec.md`。

---

## 4. V8 / code-mode

`code-mode` 是唯一链接 `librusty_v8`(约 120 MB)的 crate。它提供同进程的 JavaScript
执行沙箱。

`publish strip-v8` 把 `code-mode` 替换成无 V8 的 stub,后者重新导出相同的 protocol 类型
和 provider 类型签名(`InProcessCodeModeSessionProvider`、
`ProcessOwnedCodeModeSessionProvider`、`WebSocketCodeModeSessionProvider`),因此
**下游代码无需改动即可编译** —— 不用 `#[cfg]` 门控,不用 Cargo feature。各 provider 的
`create_session` 在运行时返回错误;上游在 `Arc<dyn CodeModeSessionProvider>` trait 边界
后持有它们,所以这是一个干净的接缝。

**GUI/TUI/WebUI 对话路径完全不碰 V8。**`code-mode` 是 feature-gated、默认关闭的能力
(`Feature::CodeMode: default_enabled = false`)。launcher 和 GUI 后端通过普通 JSON-RPC
和 AppServer 通信,从不直接 import `code-mode`。V8 只在真正执行 code-mode cell 时惰性
初始化(`OnceCell`)。所以 `strip-v8` 构建完全支持正常的聊天、审批、diff、thread 操作
—— 只是失去可选的代码执行沙箱。

---

## 5. 仓库布局(lemurclaw 新增)

```
.
├── codex-rs/
│   ├── lemurclaw/            # launcher(lib + bin)
│   ├── lemurclaw-gui/        # wry + tao 桌面窗口
│   ├── lemurclaw-webui/      # axum + WebSocket 浏览器 UI
│   │   └── assets/           # 共享 React/Vite/TS 前端
│   ├── lemurclaw-transport/  # 共享传输类型
│   └── …                     # 上游 codex-* crate,未修改
├── xtask/                    # 发布工具链(独立 crate)
│   └── src/{main,rename,bundle,manifest,source_rewrite,strip_v8,forks,verify}.rs
├── lemurclaw-rs/              # xtask 输出(仅 target/ 被 gitignore)
├── lemurclaw-rs.forks/        # 发布用 fork 克隆(gitignored)
├── docs/lemurclaw.zh-CN.md   # 本文档
├── xtask-brand-rewrite-spec.md
└── .agents/skills/lemurclaw-upstream-sync/SKILL.md
```

`.gitignore` 新增:`/lemurclaw-rs/target/`、`/lemurclaw-rs.forks/`、`/.zcode/`。

---

## 6. 故障排查

完整已知问题表见 `.agents/skills/lemurclaw-upstream-sync/SKILL.md`(bundle 后 workspace
重复项、fork rewire 别名、合并后修复错误、二进制 target 的 `main.rs` 恢复、嵌套
`Cargo.toml` 清理,以及 crates.io 发布顺序)。该 skill 是上游同步后重跑流水线的权威
运行手册。
