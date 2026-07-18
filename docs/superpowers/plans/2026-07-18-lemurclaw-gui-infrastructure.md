# lemurclaw GUI 基础设施 实现计划(子项目 2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 填充 `lemurclaw-gui` crate,让 `lemurclaw --frontend gui` 能打开一个 wry 窗口(内嵌 React 空骨架),IPC 双向通信跑通:JS 发 ClientRequest → 经 request_handle 转发 → AppServerClient;AppServerClient 事件流 → 经 tao proxy → evaluate_script 推回 JS。完成标准:GUI 窗口能跑一轮 agent 对话(空白 UI 但事件流通,console 能看到 ServerNotification)。

**Architecture:** tao EventLoop + Window + wry WebView 在主线程。tokio task 跑 `AppServerClient::InProcess` 的 `next_event()` 循环,每个事件序列化为 JSON,经 `EventLoopProxy::send_event` 投递回主线程,主线程 `evaluate_script` 推给 React。反向:wry `ipc_handler` 收 JS 的 ClientRequest JSON,用 `request_handle()`(Clone)在独立 tokio task 发送(不需 `&mut client`)。WebView 非 Send,必须主线程调 evaluate_script。

**Tech Stack:** wry 0.54.2 + tao(最新,codex 无 wry/tao 需自己加)、React + TypeScript + Vite(`assets/`)、codex-app-server-client(InProcess)、codex 的 ts-rs 生成类型。

**Spec:** `docs/superpowers/specs/2026-07-18-lemurclaw-codex-gui-design.md`(§4 布局 + §5 传输 + §2.2 lemurclaw-gui 职责)

**前置事实(已核实):**
- codex-rs 无 wry/tao 依赖,需自己加(workspace.dependencies)
- `InProcessAppServerClient::start(InProcessClientStartArgs{...})` 是入口,19 字段无 Default
- `request_handle()` 返回 Clone 句柄(lib.rs:578),让 ipc_handler 在独立 task 发请求
- `next_event(&mut self) -> Option<AppServerEvent>` 在 AppServerClient enum 上(lib.rs:885)
- WebView 非 Send → evaluate_script 必须主线程 → tao EventLoopProxy 投递
- `AppServerEvent` 未 derive Serialize → 序列化内部 ServerNotification/ServerRequest(它们 serde 可序列化)
- ts 类型:`codex-rs/app-server-protocol/schema/typescript/` 整树(含 v2/),copy 进 assets
- 启动 InProcess 不需真 auth:用 `EnvironmentManager::default_for_tests()`(dev 路径)

---

## 范围说明

本计划只做 **GUI 基础设施**(子项目 2):打开窗口 + IPC 双向通 + 跑通一轮对话事件流。**不做** ~28 个 React 组件(scrollback/prompt/approval 等是子项目 3)。完成标准:GUI 窗口能跑一轮 agent 对话(空白 UI 但事件流通,console 能看到 ServerNotification)。

## 文件结构

| 文件 | 责任 |
|---|---|
| `codex-rs/Cargo.toml` | workspace.dependencies 加 wry + tao |
| `codex-rs/lemurclaw-gui/Cargo.toml` | 加 wry/tao/codex-app-server-client/serde_json/tokio/include_dir |
| `codex-rs/lemurclaw-gui/src/lib.rs` | gui 入口:run_gui()、构造 InProcess、tao loop + webview |
| `codex-rs/lemurclaw-gui/src/loop.rs` | tokio next_event 循环 + proxy.send_event |
| `codex-rs/lemurclaw-gui/src/ipc.rs` | ipc_handler:收 JSON → request_handle 转发 |
| `codex-rs/lemurclaw-gui/assets/package.json` | React + Vite + TypeScript |
| `codex-rs/lemurclaw-gui/assets/vite.config.ts` | Vite 配置 |
| `codex-rs/lemurclaw-gui/assets/index.html` | React 入口 HTML |
| `codex-rs/lemurclaw-gui/assets/src/main.tsx` | React 骨架:Transport 客户端 + 事件 console |
| `codex-rs/lemurclaw-gui/assets/src/transport.ts` | WryIpcTransport(window.ipc ↔ onEvent) |
| `codex-rs/lemurclaw-gui/assets/src/types/` | copy 自 codex ts-rs 生成 |
| `codex-rs/lemurclaw-gui/build.rs` | 检测 Node → npm run build → assets/dist |

---

## Task 2.1:加 wry + tao 依赖,跑通"打开空窗口"

**Files:** `codex-rs/Cargo.toml`、`codex-rs/lemurclaw-gui/Cargo.toml`、`src/lib.rs`

- [x] **Step 1: 加 wry + tao 到 workspace.dependencies**

Modify `codex-rs/Cargo.toml` `[workspace.dependencies]` 加:
```toml
wry = "0.54"
tao = "0.33"
include_dir = "0.7"
```
(具体 tao 版本以 crates.io 与 wry 0.54 兼容为准;wry 0.54 系列通常配 tao 0.30+。若版本冲突,以 cargo 错误指引调整。)

> **执行记录(2026-07-18):** 经查 crates.io,wry 0.54.2 实际 require `tao ^0.34`(非 0.33),故采用 **tao 0.34 + wry 0.54**。`include_dir` 已在工作区(0.7.4),无需重复添加。tao/wry 按字母序插入(tao 在 tar/tempfile 之间,wry 在 winapi-util/zip 之间)。

- [x] **Step 2: lemurclaw-gui/Cargo.toml 加依赖**

```toml
[dependencies]
lemurclaw-transport = { workspace = true }
codex-app-server-client = { workspace = true }
codex-app-server-protocol = { workspace = true }
codex-config = { workspace = true }
codex-arg0 = { workspace = true }
wry = { workspace = true }
tao = { workspace = true }
include_dir = { workspace = true }
anyhow = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
```

> **执行记录(2026-07-18):** 工作区配置了 cargo-shear(检测未用依赖),故仅加 Task 2.1 实际用到的最小集(`anyhow`/`tao`/`wry`)。其余依赖(codex-app-server-client、tokio、include_dir、serde、serde_json 等)推迟到实际 task 2.3/2.4 引入时按需添加,避免 cargo-shear 误报。

- [x] **Step 3: 写最小 run_gui:打开空窗口(不接 AppServerClient,先验证 wry/tao)**

`src/lib.rs`:
```rust
//! lemurclaw GUI:wry+tao webview + AppServerClient 驱动。
pub fn run_gui() -> anyhow::Result<()> {
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoop};
    use tao::window::WindowBuilder;
    let event_loop = EventLoop::new::<()>();
    let window = WindowBuilder::new().with_title("lemurclaw").build(&event_loop).unwrap();
    let _webview = wry::WebViewBuilder::new()
        .with_url("data:text/html,<html><body><h1>lemurclaw GUI</h1></body></html>")
        .build(&window)?;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent { event: WindowEvent::CloseRequested, .. } = event {
            *control_flow = ControlFlow::Exit;
        }
    });
}
```

- [x] **Step 4: lemurclaw/src/lib.rs 的 Gui arm 改为调 run_gui**

Modify `codex-rs/lemurclaw/src/lib.rs` Gui arm:从 `Err(anyhow!("not implemented"))` 改为 `lemurclaw_gui::run_gui()`(同步,与 TUI 的 arg0 类似,tao loop 自己接管)。

> **执行记录(2026-07-18):** 同时删除了过时的 `gui_frontend_returns_error` 测试(其断言的逻辑已不存在),并更新模块 doc 注释反映 GUI 已接 run_gui。webui stub 测试保留。`lemurclaw/Cargo.toml` 加 `lemurclaw-gui = { workspace = true }` 依赖。

- [x] **Step 5: 验证 `cargo check -p lemurclaw-gui` + `cargo check -p lemurclaw`**

Run(从 codex-rs/):
```bash
cargo check -p lemurclaw-gui
cargo check -p lemurclaw
```
Expected: 编译通过(wry/tao 拉入)。若版本冲突,调整 Step 1 的版本号。

> **执行记录(2026-07-18):** `cargo check -p lemurclaw-gui` ✅(9m54s,wry 0.54.2 + tao 0.34.8 拉入);`cargo check -p lemurclaw` ✅ exit 0;`cargo clippy -p lemurclaw-gui -p lemurclaw` ✅ 无新增 lint(仅 pre-existing lemurclaw-transport 的 async_fn_in_trait 警告);`cargo test -p lemurclaw --lib` ✅ 11 passed / 0 failed。

- [x] **Step 6: Commit**

```bash
git add codex-rs/Cargo.toml codex-rs/lemurclaw-gui codex-rs/lemurclaw codex-rs/Cargo.lock
git commit -m "feat(gui): open empty wry+tao window (subproject 2 start)"
```

> **执行记录(2026-07-18):** committed as `e9e358744`(6 files changed, 1524 insertions, 94 deletions;大部分 +lines 来自 Cargo.lock 的 wry/tao 依赖树)。

---

## Task 2.2:React 骨架 + build.rs + ts 类型 copy

**Files:** `lemurclaw-gui/assets/*`、`lemurclaw-gui/build.rs`

- [x] **Step 1: 创建 React + Vite 骨架**

> **执行记录(2026-07-19):** scaffold 全部按 plan 创建,版本固定为 react ^18.3.1 / vite ^5.4.11 / @vitejs/plugin-react ^4.3.4 / typescript ^5.6.3(避免 ^18 这种宽约束带来的漂移)。额外加了 `.gitignore`(排除 node_modules/ + dist/),`index.html` 扩成多行可读版。

`assets/package.json`:
```json
{
  "name": "lemurclaw-gui-frontend",
  "private": true,
  "version": "0.0.0",
  "type": "module",
  "scripts": { "dev": "vite", "build": "vite build", "preview": "vite preview" },
  "dependencies": { "react": "^18", "react-dom": "^18" },
  "devDependencies": { "@types/react": "^18", "@types/react-dom": "^18", "@vitejs/plugin-react": "^4", "typescript": "^5", "vite": "^5" }
}
```
`assets/vite.config.ts`:
```ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
export default defineConfig({ plugins: [react()], build: { outDir: 'dist' } });
```
`assets/index.html`:
```html
<!doctype html><html><head><meta charset="utf-8"/><title>lemurclaw</title></head>
<body><div id="root"></div><script type="module" src="/src/main.tsx"></script></body></html>
```
`assets/tsconfig.json`:
```json
{ "compilerOptions": { "target": "ES2022", "module": "ESNext", "moduleResolution": "bundler", "jsx": "react-jsx", "strict": true, "esModuleInterop": true, "skipLibCheck": true } }
```

- [x] **Step 2: 写 React 骨架(Transport 客户端 + 事件 console)**

> **执行记录(2026-07-19)——关键偏离 plan:**
> 1. plan 的 `send({ kind: 'ready' })` **无效**(实测 codex `ClientRequest` discriminated union 用 `"method"` 不是 `"kind"`,且每条都要 `id: RequestId` + `params`)。已核实 `schema/typescript/ClientRequest.ts` + `InitializeParams.ts` + `ClientInfo.ts` + `InitializeCapabilities.ts`,改为发送合法 `initialize` 请求。
> 2. main.tsx 用 `useRef` 维护单调 seq 计数器(替代 plan 的嵌套 setState),React key 稳定 + 显示行号;事件缓冲上限 100(plan 是 50)。
> 3. transport.ts 加 `hasBridge()` helper 区分"wry 内 vs npm run dev 浏览器内",便于调试。
> 4. 边界类型保持 `unknown`,等子项目 3 React 组件层再收紧到具体 ServerNotification 变体。
> 5. 未导入 `React`(react-jsx 不需要 default import,且 tsconfig `noUnusedLocals: true` 会报 unused)。

`assets/src/transport.ts`:
```ts
// Transport:JS ↔ Rust。wry 注入 window.ipc(send)和 window.__lemurclaw.onEvent(recv)。
declare global {
  interface Window { ipc?: { postMessage: (s: string) => void }; __lemurclaw?: { onEvent: (json: string) => void } }
}
export function send(msg: unknown): void {
  window.ipc?.postMessage(JSON.stringify(msg));
}
export function onEvent(cb: (ev: unknown) => void): void {
  window.__lemurclaw = { onEvent: (json: string) => { try { cb(JSON.parse(json)); } catch (e) { console.error('parse event', e); } } };
}
```
`assets/src/main.tsx`:
```tsx
import React, { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { send, onEvent } from './transport';

function App() {
  const [events, setEvents] = useState<unknown[]>([]);
  useEffect(() => { onEvent((ev) => setEvents((p) => [...p.slice(-50), ev])); }, []);
  return (
    <div style={{ fontFamily: 'monospace', padding: 12 }}>
      <h2>lemurclaw GUI (skeleton)</h2>
      <button onClick={() => send({ kind: 'ready' })}>ready</button>
      <pre style={{ maxHeight: 400, overflow: 'auto', background: '#eee' }}>
        {events.map((e, i) => <div key={i}>{JSON.stringify(e)}</div>)}
      </pre>
    </div>
  );
}
createRoot(document.getElementById('root')!).render(<App />);
```

- [x] **Step 3: copy ts 类型(整个 typescript/ 树含 v2/)**

```bash
mkdir -p codex-rs/lemurclaw-gui/assets/src/types
cp -R codex-rs/app-server-protocol/schema/typescript/* codex-rs/lemurclaw-gui/assets/src/types/
```
(后续 React 代码 `import type { ClientRequest, ServerNotification } from './types/...'`)

> **执行记录(2026-07-19):** 已 copy,共 **616 个 `.ts` 文件**(含 `v2/` 子树)。这些是 codex ts-rs 生成产物(头部 `GENERATED CODE! DO NOT MODIFY BY HAND!`),commit 进仓库便于离线构建 + 子项目 3 直接 import。

- [x] **Step 4: 写 build.rs(检测 Node → npm run build)**

`lemurclaw-gui/build.rs`:
```rust
use std::path::PathBuf;
use std::process::Command;
fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let assets = manifest.join("assets");
    let dist = assets.join("dist");
    println!("cargo:rerun-if-changed={}", assets.display());
    if !dist.exists() {
        if Command::new("npm").arg("--version").output().is_ok() {
            let ok = Command::new("npm").args(["install"]).current_dir(&assets).status().map(|s| s.success()).unwrap_or(false)
                && Command::new("npm").args(["run", "build"]).current_dir(&assets).status().map(|s| s.success()).unwrap_or(false);
            if !ok { println!("cargo:warning=npm build failed; assets/dist missing"); }
        } else {
            println!("cargo:warning=Node not found; assets/dist missing (GUI will be blank)");
        }
    }
}
```

> **执行记录(2026-07-19)——偏离 plan:**
> 1. `env!("CARGO_MANIFEST_DIR")` 是**编译期宏**(返回的是引用 build.rs 的 crate 的 manifest,在 build.rs 上下文里能用,但若 build.rs 编译期没设置会编译失败)。改用 `env::var("CARGO_MANIFEST_DIR")`(运行期 env,Cargo 在 build.rs 执行时保证已设置)。需 `use std::env;`。
> 2. `dist.exists()` 检查改为 `dist.join("index.html").exists()`(更精确——空 dist 目录不该被当作"已构建")。
> 3. 分步检查 `npm` 可用性 / `npm install` / `npm run build` 成功,每步失败都发具体 `cargo:warning`(plan 是合并检查 + 一句泛化 warning)。
> 4. `cargo:rerun-if-changed` 拆细到 `package.json`/`index.html`/`vite.config.ts`/`tsconfig.json`/`src/`(plan 笼统整个 assets/,会因 node_modules/ 触发频繁重建)。

- [x] **Step 5: 首次构建前端**

```bash
cd codex-rs/lemurclaw-gui/assets
npm install
npm run build
ls dist/  # 确认 index.html + assets/ 生成
cd ../../..
```

> **执行记录(2026-07-19):**
> - `npm install` ✅(67 packages,29s;2 vulnerabilities 但都是 dev-only 的 eslint/vite 依赖,不影响产物)
> - `npx tsc --noEmit` ✅(0 errors;期间修过一处 unused `seq` state,改用 useRef)
> - `npm run build` ✅(`dist/index.html` 0.32kB + `dist/assets/index-*.js` 143.94kB,1.17s)
> - **关键修正:`vite.config.ts` 加 `base: './'`** —— 否则 index.html 输出绝对路径 `/assets/...`,在 Task 2.3 的 `file://` 加载下会找不到文件(绝对路径会解析到文件系统根)。改后输出 `./assets/index-*.js`,file:// 可加载。
> - `cargo check -p lemurclaw-gui` ✅(build.rs 编译通过,触发了一次 npm install + build 已存在的 dist 不重建)

- [x] **Step 6: Commit**

```bash
git add codex-rs/lemurclaw-gui
git commit -m "feat(gui): React+Vite skeleton + build.rs + ts types copy"
```

> **执行记录(2026-07-19):** committed as `0527e6b23`(625 files changed:assets/ 全树含 616 个 ts 类型 + 7 个手写源 + build.rs + package-lock.json;node_modules/ + dist/ 被 .gitignore 排除,0 污染)。

---

## Task 2.3:webview 加载 dist + ipc_handler 双向通信

**Files:** `lemurclaw-gui/src/lib.rs`

- [x] **Step 1: webview 加载 dist(经 include_dir)+ 注入 onEvent 桥 + ipc_handler**

> **执行记录(2026-07-19)——偏离 plan 多处:**
> 1. **`CARGO_MANIFEST_DIR` 是编译期 env,运行时未设置。** plan 用 `std::env::var("CARGO_MANIFEST_DIR")` 在运行时读会拿不到值。改用 `env!("CARGO_MANIFEST_DIR")`(编译期宏,把路径烘焙进二进制),+ 几个相对路径 fallback。
> 2. **`EventLoop::<T>::new()` 不存在** —— tao 0.34 中 `EventLoop::new()` 只构造 `EventLoop<()>`,自定义 user event 要走 `EventLoopBuilder::<T>::with_user_event().build()`(查 tao 源码 `event_loop.rs:81/192` 确认)。
> 3. **JSON 注入 JS 字符串需 escape。** plan 直接 `format!("...onEvent({})", json)` 把裸 JSON 拼进 JS 函数调用(JSON 不是合法 JS 表达式,且 `</script>`/U+2028/U+2029 会破坏)。改为 `escape_js_string` + `format!("window.__lemurclaw.onEvent(\"{escaped}\")")`,前端 transport.ts 的 onEvent 接收 string 再 JSON.parse。
> 4. **workspace lint `expect_used = "deny"` 阻止 `.expect()`** —— build.rs 里 `env::var(...).expect(...)` 编译失败,改用 `match` + cargo:warning + early return(build script 不该 panic,会 fail 整个构建)。
> 5. **`GuiEvent::ServerEvent` 在 task 2.3 暂未被构造**(构造在 task 2.4 的 next_event 循环),加 `#[allow(dead_code)]` + 注释说明 task 2.4 会移除。
> 6. 验证 API:wry 0.54 `WebView::evaluate_script(&self, js: &str) -> Result<()>`(line 2011)、`with_ipc_handler: Fn(Request<String>) + 'static`、`req.body() -> &String`(查 wry 源码 + examples/custom_titlebar.rs:214-215 确认);tao 0.34 `EventLoopProxy::send_event(event: T) -> Result<(), EventLoopClosed<T>>`、`Event::UserEvent(T)`。

`src/lib.rs` 升级 run_gui:
```rust
use include_dir::{include_dir, Dir};
static ASSETS: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/dist");
// 注:include_dir 宏不支持 $CARGO_MANIFEST_DIR 直接展开;用 build.rs OUT_DIR 或运行时读
// 简化:运行时读 dist 路径(开发期),或用 include_dir! 的相对编译期路径
```
**实际实现**(include_dir 宏需字面路径,改用 build.rs emit 路径或运行时 env):
```rust
pub fn run_gui() -> anyhow::Result<()> {
    use tao::event::{Event, WindowEvent, StartCause};
    use tao::event_loop::{ControlFlow, EventLoop};
    use tao::window::WindowBuilder;
    // 自定义 user event:后端 → 前端的事件 JSON
    #[derive(Clone)]
    enum GuiEvent { ServerEvent(String) }
    let event_loop = EventLoop::<GuiEvent>::new();
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new().with_title("lemurclaw").build(&event_loop)?;
    let dist = std::env::var("CARGO_MANIFEST_DIR").map(|d| format!("{d}/lemurclaw-gui/assets/dist/index.html")).unwrap_or_default();
    // 启动 AppServerClient(下一步 2.4 实现);这里先桩
    let webview = wry::WebViewBuilder::new()
        .with_url(&format!("file://{}", dist))  // 开发期:file:// 加载 dist
        .with_initialization_script(&format!(
            "window.__lemurclaw = {{ onEvent: function(json) {{ console.log('onEvent stub', json); }} }};"
        ))
        .with_ipc_handler(move |request| {
            // JS → Rust:收到 JSON,转发到 AppServerClient(下一步实现)
            println!("ipc_handler received: {}", request.body());
        })
        .build(&window)?;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) => { /* 启动后端 task(下一步) */ }
            Event::UserEvent(GuiEvent::ServerEvent(json)) => {
                let _ = webview.evaluate_script(&format!("window.__lemurclaw.onEvent({})", json));
            }
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    });
}
```
> **注:** `with_url(file://...)` 是开发期简化;生产期应通过自定义 protocol 或 include_dir 嵌入。本 task 用 file:// 验证加载,后续优化。

- [x] **Step 2: 验证编译 + 手动跑窗口加载 React**

```bash
cd codex-rs
cargo run -p lemurclaw -- --frontend gui
```
Expected: 窗口打开,显示 "lemurclaw GUI (skeleton)" + ready 按钮。点 ready 看 stdout "ipc_handler received"。

> **执行记录(2026-07-19):**
> - `cargo check -p lemurclaw-gui` ✅ Finished(dev profile, 3.73s)
> - `cargo clippy -p lemurclaw-gui` ✅ Finished(2m06s,无新 lint)
> - `cargo clippy -p lemurclaw` ✅(3m43s,仅 pre-existing codex-api/codex-core 警告)
> - **手动跑窗口留给用户**(沙箱无 display)。CLI 是 `cargo run -p lemurclaw -- --frontend gui`(注:`--frontend` 是 lemurclaw 的 clap flag 不是位置参数;`--` 是 cargo run 分隔符;webui stub 仍返回错)。期望:窗口打开 → 加载 React 骨架 → console 显示 "bridge: injected (wry)" → 点 "send initialize" → 终端 stdout 出现 `[lemurclaw] ipc_handler received: {"method":"initialize",...}`。

- [x] **Step 3: Commit**

```bash
git add codex-rs/lemurclaw-gui
git commit -m "feat(gui): webview loads React dist + ipc_handler + tao proxy skeleton"
```

> **执行记录(2026-07-19):** committed as `250f36bc1`(2 files changed, 146 insertions, 22 deletions;build.rs + src/lib.rs)。

---

## Task 2.4:接 AppServerClient InProcess + next_event 循环

**Files:** `lemurclaw-gui/src/lib.rs`、`src/loop.rs`、`src/ipc.rs`

- [x] **Step 1: 构造 InProcessAppServerClient(参照 tui/src/lib.rs:553-574)**

> **执行记录(2026-07-19)——偏离 plan:**
> 1. **plan 写 `state_db: None`,但实际 InProcess 启动需要 state_db。** 查 tui 生产路径(tui/src/lib.rs:515 + 286-303)嵌入式 target 必须 `state_db::try_init(config)` 初始化。我用 `codex_rollout::state_db::try_init(&config).await` 拿到 handle 后传 `Some(state_db)`(参照 app-server-client test helper lib.rs:1006-1008)。
> 2. **Config 构造用 `ConfigBuilder::default().build()`**(从 `codex_app_server_client::legacy_core::config::ConfigBuilder`,这是 tui 同款 re-export 路径)。比 test helper 的 `build_test_config_for_codex_home` 简单——直接走生产路径,从 codex_home env 自动加载。
> 3. **模块拆分**:新增 `src/backend.rs`(213 行)承载 backend 逻辑,`lib.rs` 只做 tao 编排。符合 AGENTS.md "避免大模块、新功能用新模块" 原则。
> 4. **线程拓扑**:tokio `Runtime::new()` + `std::thread::spawn` 独立线程跑 runtime(不用 `runtime.spawn`,因为我们要这个线程长期 own runtime,且主线程要给 tao)。`ipc_handler`(同步)用 `tokio::runtime::Handle`(Clone + Send + Sync)在 runtime 上 spawn one-shot task 转发 `ClientRequest`。
> 5. **`BackendHandles` 不 `pub use`** —— `spawn()` 返回给 lib.rs 用即可,无需 expose 成 crate API(避免 API 表面膨胀)。
> 6. **依赖**:lemurclaw-gui/Cargo.toml 加 `codex-app-server-client`/`codex-app-server-protocol`/`codex-arg0`/`codex-config`/`codex-protocol`/`codex-rollout`/`tokio`/`serde_json`/`serde`(全 `workspace = true`)。`codex-exec-server` 不需要直接依赖——`EnvironmentManager` 从 `codex_app_server_client` re-export 拿到。
> 7. **导入坑**:`ClientRequest`/`ConfigWarningNotification`/`LoaderOverrides`/`SessionSource` 在 `codex_app_server_client` 里是**私有 use**(非 pub re-export),必须直接从源 crate 导入(`codex_app_server_protocol`、`codex_config`、`codex_protocol`)。编译器提示的 `codex_app_server_protocol::protocol::v2::config::ConfigWarningNotification` 是错的(`protocol` 模块私有)——顶层 `codex_app_server_protocol::ConfigWarningNotification` 才对(v2/mod.rs:35 `pub use config::*` + lib.rs:42 `pub use protocol::v2::*` 链路)。

在 run_gui 启动时(StartCause::Init 或之前 async),构造 InProcessClientStartArgs(19 字段)。开发期用 EnvironmentManager::default_for_tests()。参照核实的 tui 模式 + test helper。

具体 InProcessClientStartArgs 构造代码(参照 tui/src/lib.rs:553):
```rust
// 在 tokio runtime 内:
let client = InProcessAppServerClient::start(InProcessClientStartArgs {
    arg0_paths: Arg0DispatchPaths::default(),
    config: Arc::new(config),
    cli_overrides: Vec::new(),
    loader_overrides: LoaderOverrides::default(),
    strict_config: false,
    cloud_config_bundle: CloudConfigBundleLoader::default(),
    feedback: CodexFeedback::new(),
    log_db: None,
    state_db: None,
    environment_manager: Arc::new(EnvironmentManager::default_for_tests()),
    config_warnings: Vec::new(),
    session_source: SessionSource::Cli,
    enable_codex_api_key_env: false,
    client_name: "lemurclaw-gui".to_string(),
    client_version: env!("CARGO_PKG_VERSION").to_string(),
    experimental_api: true,
    mcp_server_openai_form_elicitation: false,
    opt_out_notification_methods: Vec::new(),
    channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
}).await?;
```
> **注:** Config 构造也需参照 tui/test helper。先最小化,能 start 即可。

- [x] **Step 2: 启动 tokio task 跑 next_event 循环,经 proxy 投递**

```rust
let request_handle = client.request_handle();  // Clone,给 ipc_handler 用
std::thread::spawn(move || {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async move {
        let mut client = client;
        while let Some(event) = client.next_event().await {
            // AppServerEvent 未 derive Serialize;序列化内部 payload
            let json = match event {
                AppServerEvent::ServerNotification(n) => serde_json::to_string(&n).unwrap_or_default(),
                AppServerEvent::ServerRequest(r) => serde_json::to_string(&r).unwrap_or_default(),
                AppServerEvent::Lagged { skipped } => format!(r#"{{"lagged":{skipped}}}"#),
                AppServerEvent::Disconnected { message } => format!(r#"{{"disconnected":{message:?}}}"#),
            };
            let _ = proxy.send_event(GuiEvent::ServerEvent(json));
        }
    });
});
```

- [x] **Step 3: ipc_handler 用 request_handle 转发 ClientRequest**

```rust
.with_ipc_handler(move |request| {
    let body = request.body().to_string();
    let handle = request_handle.clone();
    tokio::spawn(async move {
        if let Ok(req) = serde_json::from_str::<ClientRequest>(&body) {
            let _ = handle.request(req).await;
        }
    });
})
```
> **注:** ipc_handler 是同步闭包,内部 spawn tokio task(需 runtime handle,全局或 leaked)。这是 wry 的固有限制。

> **执行记录(2026-07-19):** 实现略偏离 plan——不全局 leak runtime,而是 `BackendHandles` struct 持有 `tokio::runtime::Handle` + `InProcessAppServerRequestHandle`(后者 derive Clone)。`handle_ipc(&self, body)` 在 `self.handle` 上 spawn,把 `serde_json::from_str::<ClientRequest>` 也放 task 里(主线程不阻塞 + 坏 body 只 log 不炸)。lib.rs 的 ipc_handler 闭包直接调 `backend.handle_ipc(request.body())`(传 `&str`,免一次 `to_string()`)。

- [x] **Step 4: 验证编译**

```bash
cd codex-rs
cargo check -p lemurclaw-gui
```
Expected: 通过。InProcessClientStartArgs 字段/Config 构造若有问题,逐个核实(参照 test helper start_test_client_with_capacity lib.rs:1000)。

> **执行记录(2026-07-19):** 迭代过程发现并修了 5 个编译错误:
> 1. `ClientRequest`/`ConfigWarningNotification`/`LoaderOverrides`/`SessionSource` 在 app-server-client 是私有 use → 改从源 crate 导入。
> 2. `codex_protocol` 不在 deps → Cargo.toml 加 `codex-protocol`。
> 3. `ConfigWarningNotification` 编译器提示路径错(`protocol` 模块私有) → 用顶层 `codex_app_server_protocol::ConfigWarningNotification`。
> 4. `client.next_event()` 返回 `InProcessServerEvent`(非 `AppServerEvent`,因为用的是 `InProcessAppServerClient` 不是 `AppServerClient` enum)→ 改 `serialize_event` 入参类型 + 去掉 `Disconnected` arm(`InProcessServerEvent` 无此变体,3 个变体:ServerRequest/ServerNotification/Lagged)。
> 5. clippy `redundant_clone = "deny"`(proxy.clone() 多余,因为 spawn 直接 move proxy 即可)+ `escape_json_string` 因 Disconnected 删除变 unused(整体删掉)。
>
> 最终:`cargo check -p lemurclaw-gui` ✅ Finished(5.98s);`cargo clippy -p lemurclaw-gui` ✅ 无新 lint;`cargo check -p lemurclaw` ✅(1m12s,验证调用方未破)。

- [x] **Step 5: Commit**

```bash
git add codex-rs/lemurclaw-gui codex-rs/Cargo.lock
git commit -m "feat(gui): wire AppServerClient InProcess + next_event loop + ipc via request_handle"
```

> **执行记录(2026-07-19):** committed as `91a7d4f4f`(4 files changed:backend.rs 新增 213 行;lib.rs 重构 77 行变化 + 拆 backend;Cargo.toml 加 9 行 deps;Cargo.lock +9 行)。

---

## Task 2.5:端到端验证 —— GUI 跑通一轮对话

**Files:** (无代码改动,验证)

- [ ] **Step 1: 启动 GUI(需 provider config,复用 Task 1.4 的 codex config)**

```bash
cd codex-rs
cargo run -p lemurclaw -- --frontend gui
```
Expected: 窗口打开。点 ready(发 ClientRequest 如 Initialize)。在 React console 看到事件流(AgentMessageDelta/ItemCompleted 等)。

- [ ] **Step 2: 若有 OpenRouter/ollama config,发一个 TurnStart 看 console 收到流式回复**

(复用 Task 1.4 的 provider config;若环境无 provider,至少验证 Initialize 握手事件流通)

- [ ] **Step 3: 记录验证结果 + Commit**

在 adaptations 文档或新 GUI doc 记录 GUI 基础设施完成。

---

## 子项目 2 完成标准

- [ ] `cargo check -p lemurclaw-gui` 通过
- [ ] `lemurclaw --frontend gui` 打开 wry 窗口,加载 React 骨架
- [ ] IPC 双向通:JS 发 ClientRequest → Rust 收到(ipc_handler);Rust 推 ServerEvent → JS 收到(console)
- [ ] AppServerClient InProcess 启动,next_event 循环经 tao proxy 推事件到前端
- [ ] 至少一次 agent 对话握手(Initialize)事件流可见

## 后续(子项目 3+)

- React 组件:Scrollback/PromptInput/ApprovalCard/各 history cell(~28 组件)
- ViewModel 投影层(规范化 ServerNotification 成 React 友好结构)
- 16ms 节流、错误处理、完整对话 UI
- webui 模式(子项目 6)
