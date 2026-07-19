# lemurclaw 核心对话循环 UI 实现计划(子项目 3)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `lemurclaw-gui` 的 React 前端从"事件 console 骨架"(子项目 2 产物)升级到功能等价 codex TUI 主体的完整对话 UI:Scrollback + 9 类 history cell(user / assistant / reasoning / command-exec / file-change / mcp / plan / hook / web-search)+ composer(发 TurnStart)+ ApprovalCard(exec / patch / mcp-elicitation 决策回传)。完成标准:`lemurclaw --frontend gui` 能跑一轮完整 agent 对话(用户输入 → 流式 assistant 回复 → 工具调用 + 审批 → patch 展示 → plan 渲染),功能等价 TUI 主体。

**Architecture:**
- **ViewModel 投影层**(`assets/src/viewModel/`):把 ServerNotification 流增量折叠成 `ConversationState`(turns → items → 渲染结构)。React 组件只读 ViewModel,不直接吃 raw 事件。增量更新(append delta、replace item on ItemCompleted)。
- **Cell 组件层**(`assets/src/components/cells/`):每类 ThreadItem 一个组件,纯函数 `(model) => JSX`,易测。
- **ApprovalCard** 消费 `ServerRequest` 流,Rust 后端扩展 `resolve_server_request` 后,JS 发约定 envelope,backend 转 codex 协议 resolve。
- **Rust 后端**(`lemurclaw-gui/src/backend.rs`):新增 `resolve_server_request` 通道,需在 `codex-rs/app-server-client/src/lib.rs` 给 `InProcessAppServerRequestHandle` 加 2 个纯加法方法(fork commit,逆 upstream 风险极低)。
- **测试**:vitest + @testing-library/react + jsdom,组件单测 + ViewModel reducer 单测。`npm test` 跑。

**Tech Stack:** React 18.3 + TypeScript 5.6 + Vite 5.4(已就位)+ vitest 2.x + @testing-library/react 16 + jsdom(新增 devDeps)+ 既有 codex ts-rs 类型(`assets/src/types/`)。

**Spec:** `docs/superpowers/specs/2026-07-18-lemurclaw-codex-gui-design.md`(§4 surface 矩阵 + §4.3 布局 + §1.4 审批流)
**前置:** 子项目 2(GUI 基础设施)已完成,见 `docs/superpowers/plans/2026-07-18-lemurclaw-gui-infrastructure.md`。

---

## 范围说明

本计划做 **子项目 3:核心对话循环**(spec §6.1 第 4 块):
- ✅ ViewModel 投影层 + reducer(规范化 ServerNotification)
- ✅ Scrollback 容器 + 9 类 history cell(7 主类 + hook/web-search 轻量)
- ✅ Composer(发 `turn/start`,Enter 发送,Shift+Enter 换行)
- ✅ ApprovalCard(exec / patch / mcp-elicitation / permissions / tool-input,决策回传)
- ✅ Rust 后端扩展 resolve 通道
- ✅ vitest + @testing-library/react 测试基础设施

**不做**(留给后续子项目):
- ❌ SessionPicker / ModelPicker / AgentPicker / Onboarding / TranscriptPager(子项目 4)
- ❌ SettingsModal 系列 / statusline / diff viewer 独立页 / /usage / /status(子项目 5)
- ❌ SlashPopup / MentionPopup / FileSearchPopup 的 fuzzy 交互(子项目 3 只做 composer 朴素 textarea + 占位提示;完整 popup 在子项目 4/5)
- ❌ webui 模式(子项目 6)

**变更规模控制:** 每个任务(Task 3.1 - 3.9)独立 commit,单任务目标 < 300 行变更,符合 AGENTS.md 800 行总变更 guideline。计划共 9 个任务,建议分 2-3 个 PR(Task 3.1-3.3 / 3.4-3.6 / 3.7-3.9)。

---

## 文件结构

新增/修改文件总览(每个文件单一职责,< 500 LoC):

| 文件 | 责任 | 任务 |
|---|---|---|
| `codex-rs/app-server-client/src/lib.rs` | 给 `InProcessAppServerRequestHandle` 加 `resolve_server_request` / `reject_server_request`(纯加法,fork commit) | 3.1 |
| `codex-rs/lemurclaw-gui/src/backend.rs` | `BackendHandles` 加 resolve 通道;`handle_ipc` 区分 ClientRequest vs resolve envelope | 3.1 |
| `codex-rs/lemurclaw-gui/assets/package.json` | 加 vitest + testing-library + jsdom devDeps | 3.2 |
| `codex-rs/lemurclaw-gui/assets/vitest.config.ts` | vitest 配置(jsdom 环境) | 3.2 |
| `codex-rs/lemurclaw-gui/assets/src/types/guards.ts` | ServerNotification / ServerRequest 的 type guards + discriminated union narrowing 辅助 | 3.3 |
| `codex-rs/lemurclaw-gui/assets/src/types/guards.test.ts` | guards 单测 | 3.3 |
| `codex-rs/lemurclaw-gui/assets/src/viewModel/types.ts` | `ConversationState` / `CellModel` / `PendingApproval` 等 ViewModel 类型 | 3.4 |
| `codex-rs/lemurclaw-gui/assets/src/viewModel/reducer.ts` | reducer:`(state, ServerNotification) => state`,增量折叠事件流 | 3.4 |
| `codex-rs/lemurclaw-gui/assets/src/viewModel/reducer.test.ts` | reducer 单测(每类事件一个 case) | 3.4 |
| `codex-rs/lemurclaw-gui/assets/src/components/cells/UserMessageCell.tsx` | user message cell | 3.5 |
| `codex-rs/lemurclaw-gui/assets/src/components/cells/AgentMessageCell.tsx` | assistant message cell(streaming + final) | 3.5 |
| `codex-rs/lemurclaw-gui/assets/src/components/cells/ReasoningCell.tsx` | reasoning cell(summary/content delta) | 3.5 |
| `codex-rs/lemurclaw-gui/assets/src/components/cells/CommandExecCell.tsx` | exec cell(command + aggregated output + exit code) | 3.5 |
| `codex-rs/lemurclaw-gui/assets/src/components/cells/FileChangeCell.tsx` | patch cell(changes + diff + apply status) | 3.6 |
| `codex-rs/lemurclaw-gui/assets/src/components/cells/McpToolCell.tsx` | mcp tool call cell | 3.6 |
| `codex-rs/lemurclaw-gui/assets/src/components/cells/PlanCell.tsx` | plan cell(plan delta + final) | 3.6 |
| `codex-rs/lemurclaw-gui/assets/src/components/cells/HookCell.tsx` | hook cell(started/completed/entries) | 3.6 |
| `codex-rs/lemurclaw-gui/assets/src/components/cells/WebSearchCell.tsx` | web search cell(轻量) | 3.6 |
| `codex-rs/lemurclaw-gui/assets/src/components/cells/__tests__/*.test.tsx` | 每 cell 一个组件测试 | 3.5/3.6 |
| `codex-rs/lemurclaw-gui/assets/src/components/Scrollback.tsx` | Scrollback 容器:渲染 CellModel 列表,auto-scroll | 3.7 |
| `codex-rs/lemurclaw-gui/assets/src/components/Composer.tsx` | composer:textarea + 发送(发 `turn/start`) | 3.7 |
| `codex-rs/lemurclaw-gui/assets/src/components/ApprovalCard.tsx` | 审批卡:exec/patch/mcp-elicitation,决策回传 | 3.8 |
| `codex-rs/lemurclaw-gui/assets/src/transport.ts` | 加 `resolveServerRequest` / `rejectServerRequest` helper(发约定 envelope) | 3.8 |
| `codex-rs/lemurclaw-gui/assets/src/app/App.tsx` | 顶层 App:wires onEvent → reducer,renders Scrollback + Composer + ApprovalCard 队列 | 3.9 |
| `codex-rs/lemurclaw-gui/assets/src/app/useConversation.ts` | reducer wiring hook | 3.9 |
| `codex-rs/lemurclaw-gui/assets/src/main.tsx` | 简化:只 createRoot + render `<App/>`(业务逻辑挪到 app/App.tsx) | 3.9 |
| `codex-rs/lemurclaw-gui/assets/src/styles.css` | 基础布局样式(主对话区 + composer + 审批卡) | 3.9 |

---

## Task 3.1:Rust 后端扩展 resolve 通道

**目标:** 让 JS 能回传审批决策。当前 `InProcessAppServerRequestHandle` 只有 `request()`,审批要 `resolve_server_request()`。先给 codex-rs/app-server-client 加 2 个纯加法方法(fork commit),再在 lemurclaw-gui/src/backend.rs 暴露 resolve 通道,transport.ts 端用约定 envelope。

**Files:**
- Modify: `codex-rs/app-server-client/src/lib.rs`(加 2 个方法到 `impl InProcessAppServerRequestHandle`,~50 行)
- Modify: `codex-rs/lemurclaw-gui/src/backend.rs`(`handle_ipc` 区分 envelope + 新增 resolve 路径,~80 行)

- [ ] **Step 1: 给 InProcessAppServerRequestHandle 加 resolve/reject 方法**

Modify `codex-rs/app-server-client/src/lib.rs`,在 `impl InProcessAppServerRequestHandle` 块内(line 766-808,`request_typed` 之后)插入 2 个方法。完全镜像 `InProcessAppServerClient` 上的同名方法(line 663-716),只是 `self.command_tx` 来源不同(handle 自己的 field)。

```rust
    /// Resolves a pending server request by ID.
    ///
    /// Mirror of [`InProcessAppServerClient::resolve_server_request`]; lets
    /// call sites that only hold a request handle (e.g. an ipc handler that
    /// cannot own the full client) still respond to `ServerRequest`s.
    pub async fn resolve_server_request(
        &self,
        request_id: RequestId,
        result: JsonRpcResult,
    ) -> IoResult<()> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(ClientCommand::ResolveServerRequest {
                request_id,
                result,
                response_tx,
            })
            .await
            .map_err(|_| {
                IoError::new(
                    ErrorKind::BrokenPipe,
                    "in-process app-server worker channel is closed",
                )
            })?;
        response_rx.await.map_err(|_| {
            IoError::new(
                ErrorKind::BrokenPipe,
                "in-process app-server resolve channel is closed",
            )
        })?
    }

    /// Rejects a pending server request by ID with a JSON-RPC error.
    ///
    /// Mirror of [`InProcessAppServerClient::reject_server_request`].
    pub async fn reject_server_request(
        &self,
        request_id: RequestId,
        error: JSONRPCErrorError,
    ) -> IoResult<()> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(ClientCommand::RejectServerRequest {
                request_id,
                error,
                response_tx,
            })
            .await
            .map_err(|_| {
                IoError::new(
                    ErrorKind::BrokenPipe,
                    "in-process app-server worker channel is closed",
                )
            })?;
        response_rx.await.map_err(|_| {
            IoError::new(
                ErrorKind::BrokenPipe,
                "in-process app-server reject channel is closed",
            )
        })?
    }
```

> **注:** `RequestId` / `JsonRpcResult` / `IoError` / `IoResult` / `JSONRPCErrorError` / `ClientCommand` / `oneshot` 都已在 lib.rs 顶部 import,无需新加。这是 codex fork commit(纯加法,不改既有逻辑)。AGENTS.md "Never add CODEX_SANDBOX_*" 等约束不触及。

- [ ] **Step 2: 验证 codex 侧编译**

```bash
cd /Users/def/lemurclaw/codex-rs
cargo check -p codex-app-server-client
```
Expected: 编译通过。若失败,通常是 import 名错(核实 `RequestId` 在 lib.rs:42、`JsonRpcResult` 在 lib.rs:43 import)。

- [ ] **Step 3: backend.rs 加 resolve 通道**

Modify `codex-rs/lemurclaw-gui/src/backend.rs`。修改 `BackendHandles::handle_ipc`(替换既有 line 58-73)以区分三种 envelope,并新增 module-level `ResolveKind` enum + `spawn_resolve` helper。

约定 envelope(JS 发):
```
{ "__resolve": <request_id>, "result": <json_value> }
{ "__reject":  <request_id>, "error": { "code": <num>, "message": "<str>" } }
```
其它 JSON 走既有 `ClientRequest` 路径。

替换 `handle_ipc`:
```rust
    /// Forward a raw JSON body from JS to the backend. Three shapes:
    ///   - `{"__resolve": id, "result": {...}}` → resolve a pending
    ///     ServerRequest (ApprovalCard accept).
    ///   - `{"__reject": id, "error": {code, message}}` → reject a pending
    ///     ServerRequest (ApprovalCard decline/cancel).
    ///   - any other JSON → ClientRequest (turn/start, thread/list, ...).
    /// All deserialization happens on the backend runtime so malformed bodies
    /// only log, never block the UI thread.
    pub fn handle_ipc(&self, body: &str) {
        let parsed: serde_json::Result<serde_json::Value> = serde_json::from_str(body);
        let handle = self.request_handle.clone();
        self.handle.spawn(async move {
            let value = match parsed {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[lemurclaw] ipc body not valid JSON: {e}");
                    return;
                }
            };
            if let Some(req_id) = value.get("__resolve") {
                if let Some(result) = value.get("result") {
                    spawn_resolve(&handle, req_id.clone(), result.clone(), ResolveKind::Resolve).await;
                }
            } else if let Some(req_id) = value.get("__reject") {
                if let Some(error) = value.get("error") {
                    spawn_resolve(&handle, req_id.clone(), error.clone(), ResolveKind::Reject).await;
                }
            } else {
                match serde_json::from_value::<ClientRequest>(value) {
                    Ok(req) => {
                        if let Err(e) = handle.request(req).await {
                            eprintln!("[lemurclaw] backend request failed: {e}");
                        }
                    }
                    Err(e) => eprintln!("[lemurclaw] ipc body not a valid ClientRequest: {e}"),
                }
            }
        });
    }
```

在 `backend.rs` 顶部 import 之后,`BackendHandles` 之前新增 enum + helper:
```rust
/// Distinguishes the two envelope kinds handled in `handle_ipc`.
enum ResolveKind {
    Resolve,
    Reject,
}

/// Resolve or reject a pending ServerRequest on the backend runtime. Pulls
/// the RequestId / JsonRpcResult / JSONRPCErrorError types from the protocol
/// crate; falls back to a safe null/error payload on malformed input so a bad
/// JS envelope never kills the worker.
async fn spawn_resolve(
    handle: &InProcessAppServerRequestHandle,
    request_id: serde_json::Value,
    payload: serde_json::Value,
    kind: ResolveKind,
) {
    let req_id = match serde_json::from_value::<codex_app_server_protocol::RequestId>(request_id) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("[lemurclaw] resolve: bad request_id: {e}");
            return;
        }
    };
    let result = match kind {
        ResolveKind::Resolve => {
            let json_result: codex_app_server_protocol::Result =
                serde_json::from_value(payload).unwrap_or_else(|_| Ok(serde_json::Value::Null));
            handle.resolve_server_request(req_id, json_result).await
        }
        ResolveKind::Reject => {
            let err: codex_app_server_protocol::JSONRPCErrorError =
                serde_json::from_value(payload).unwrap_or_else(|e| {
                    codex_app_server_protocol::JSONRPCErrorError {
                        code: -32000,
                        message: format!("malformed reject payload: {e}"),
                        data: None,
                    }
                });
            handle.reject_server_request(req_id, err).await
        }
    };
    if let Err(e) = result {
        eprintln!("[lemurclaw] resolve/reject failed: {e}");
    }
}
```

> **注:**
> - `codex_app_server_protocol::RequestId` / `JSONRPCErrorError` / `Result`(JsonRpcResult 别名)都是 pub 的(已核实 lib.rs:42-43 + protocol crate 导出)。
> - `InProcessAppServerRequestHandle` 已在 backend.rs:24 import,Step 1 加的 `resolve_server_request` / `reject_server_request` 方法可直接调。
> - `spawn_resolve` 是 module-level fn(非 `BackendHandles` 方法),因为 clippy 对 `&self` 方法里再 `&self.handle` 借用会报 `needless_pass_by_ref_value`。module-level 接 `&InProcessAppServerRequestHandle` 最干净。
> - `JSONRPCErrorError` 的字段名(code/message/data)若与协议不符,cat `codex-rs/app-server-protocol/src/protocol/common.rs` 核实后调整。

- [ ] **Step 4: 验证 lemurclaw-gui 编译**

```bash
cd /Users/def/lemurclaw/codex-rs
cargo check -p lemurclaw-gui
```
Expected: 通过。常见错误:
- `JsonRpcResult` 名字报错 → 用全路径 `codex_app_server_protocol::Result`
- `JSONRPCErrorError` 字段名报错 → cat common.rs 核实字段
- clippy `borrow_deref_ref` → `spawn_resolve` 第一参数改 `&InProcessAppServerRequestHandle` 已经是引用,应无此错

- [ ] **Step 5: clippy + fmt**

```bash
cd /Users/def/lemurclaw/codex-rs
cargo clippy -p lemurclaw-gui -p codex-app-server-client
cargo fmt -p lemurclaw-gui -p codex-app-server-client
```
Expected: 无新 lint warning。

- [ ] **Step 6: Commit**

```bash
git add codex-rs/app-server-client/src/lib.rs codex-rs/lemurclaw-gui/src/backend.rs
git commit -m "feat(gui): expose resolve/reject server-request channel for ApprovalCard (subproject 3)"
```

---

## Task 3.2:vitest 测试基础设施

**目标:** 给前端加 vitest + @testing-library/react + jsdom。组件单测 + reducer 单测都用它跑。

**Files:**
- Modify: `codex-rs/lemurclaw-gui/assets/package.json`
- Create: `codex-rs/lemurclaw-gui/assets/vitest.config.ts`
- Create: `codex-rs/lemurclaw-gui/assets/src/__tests__/setup.ts`
- Create: `codex-rs/lemurclaw-gui/assets/src/__tests__/smoke.test.ts`

- [ ] **Step 1: 加 devDeps + scripts 到 package.json**

Modify `codex-rs/lemurclaw-gui/assets/package.json`,替换全文为:
```json
{
  "name": "lemurclaw-gui-frontend",
  "private": true,
  "version": "0.0.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview",
    "test": "vitest run",
    "test:watch": "vitest"
  },
  "dependencies": {
    "react": "^18.3.1",
    "react-dom": "^18.3.1"
  },
  "devDependencies": {
    "@testing-library/jest-dom": "^6.6.3",
    "@testing-library/react": "^16.1.0",
    "@types/react": "^18.3.12",
    "@types/react-dom": "^18.3.1",
    "@vitejs/plugin-react": "^4.3.4",
    "jsdom": "^25.0.1",
    "typescript": "^5.6.3",
    "vite": "^5.4.11",
    "vitest": "^2.1.8"
  }
}
```

> **版本固定理由:** 跟子项目 2 的策略一致(避免 `^18` 宽约束漂移)。vitest 2.1.x 配 vite 5.4.x 已知兼容;@testing-library/react 16 配 React 18;jsdom 25 是 vitest 2 当前推荐。

- [ ] **Step 2: 创建 vitest.config.ts**

Create `codex-rs/lemurclaw-gui/assets/vitest.config.ts`:
```ts
import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

// Vitest config. Separate from vite.config.ts so production build (vite build)
// is not affected by test-only setup (jsdom globals, testing-library
// matchers). Reuses the same React plugin for JSX transform.
export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/__tests__/setup.ts'],
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
    exclude: ['node_modules', 'dist', 'src/types/**'],
  },
});
```

- [ ] **Step 3: 创建 setup 文件 + smoke test**

Create `codex-rs/lemurclaw-gui/assets/src/__tests__/setup.ts`:
```ts
import '@testing-library/jest-dom/vitest';
```

Create `codex-rs/lemurclaw-gui/assets/src/__tests__/smoke.test.ts`:
```ts
import { describe, it, expect } from 'vitest';

describe('vitest smoke', () => {
  it('runs', () => {
    expect(1 + 1).toBe(2);
  });
});
```

- [ ] **Step 4: 安装 + 跑 smoke test**

```bash
cd /Users/def/lemurclaw/codex-rs/lemurclaw-gui/assets
npm install
npm test
```
Expected: `vitest run` 通过 1 个 test(`vitest smoke > runs`)。若 `@testing-library/jest-dom/vitest` import 失败,确认 `@testing-library/jest-dom` 已在 devDeps(Step 1)。

- [ ] **Step 5: 验证 build 未被破坏**

```bash
npm run build
ls dist/
```
Expected: `dist/index.html` + `dist/assets/*.js` 生成照旧(vitest.config.ts 不影响 vite build)。

- [ ] **Step 6: 验证 build.rs 仍能触发 npm install + build**

`codex-rs/lemurclaw-gui/build.rs` 已在 dist 不存在时跑 `npm install && npm run build`,devDeps 加了之后首次构建会装上。验证:
```bash
cd /Users/def/lemurclaw/codex-rs
rm -rf lemurclaw-gui/assets/dist
cargo check -p lemurclaw-gui 2>&1 | tail -5
ls lemurclaw-gui/assets/dist/index.html
```
Expected: build.rs 触发 npm install(装 vitest 等)+ npm run build,生成 dist/index.html。

- [ ] **Step 7: Commit**

```bash
git add codex-rs/lemurclaw-gui/assets/package.json codex-rs/lemurclaw-gui/assets/package-lock.json \
        codex-rs/lemurclaw-gui/assets/vitest.config.ts codex-rs/lemurclaw-gui/assets/src/__tests__/
git commit -m "test(gui): add vitest + testing-library infrastructure"
```

---

## Task 3.3:ServerNotification type guards

**目标:** ViewModel reducer 要把 `unknown`(transport.ts 当前传出)安全 narrow 到具体 ServerNotification 变体。type guards 文件集中管理 narrowing 逻辑,reducer 和组件都引用。

**Files:**
- Create: `codex-rs/lemurclaw-gui/assets/src/types/guards.ts`
- Create: `codex-rs/lemurclaw-gui/assets/src/types/guards.test.ts`

- [ ] **Step 1: 实现 guards.ts**

Create `codex-rs/lemurclaw-gui/assets/src/types/guards.ts`:
```ts
// Type guards for narrowing the `unknown` event payloads the transport layer
// hands us (see transport.ts:onEvent). The wire shape is the codex
// `ServerNotification` / `ServerRequest` discriminated union — each variant
// carries `method` (string tag) + `params`. ServerRequest additionally carries
// `id`.

import type { ServerNotification } from './ServerNotification';
import type { ServerRequest } from './ServerRequest';

// Re-export for caller convenience.
export type { ServerNotification, ServerRequest };

/** Common envelope shape shared by ServerNotification and ServerRequest. */
interface Envelope {
  method?: unknown;
  params?: unknown;
  id?: unknown;
}

/** True if x looks like a ServerNotification envelope (method + params, no id). */
export function isServerNotification(x: unknown): x is ServerNotification {
  if (typeof x !== 'object' || x === null) return false;
  const env = x as Envelope;
  return typeof env.method === 'string' && 'params' in env && !('id' in env);
}

/** True if x looks like a ServerRequest envelope (method + params + id). */
export function isServerRequest(x: unknown): x is ServerRequest {
  if (typeof x !== 'object' || x === null) return false;
  const env = x as Envelope;
  return (
    typeof env.method === 'string' &&
    'params' in env &&
    'id' in env &&
    (typeof env.id === 'string' || typeof env.id === 'number')
  );
}

/** Narrow a ServerNotification to a specific `method`. */
export function hasMethod<T extends ServerNotification['method']>(
  x: ServerNotification,
  method: T,
): x is Extract<ServerNotification, { method: T }> {
  return x.method === method;
}

/** Narrow a ServerRequest to a specific `method`. */
export function hasServerRequestMethod<T extends ServerRequest['method']>(
  x: ServerRequest,
  method: T,
): x is Extract<ServerRequest, { method: T }> {
  return x.method === method;
}
```

> **注:** `ServerNotification` / `ServerRequest` 在顶层 `types/`(非 v2/)—— 它们是聚合 discriminated union(子项目 2 已 copy)。顶层 ServerNotification 有 ~70 个变体,ServerRequest 有 10 个。

- [ ] **Step 2: 写 guards 单测**

Create `codex-rs/lemurclaw-gui/assets/src/types/guards.test.ts`:
```ts
import { describe, it, expect } from 'vitest';
import { isServerNotification, isServerRequest, hasMethod } from './guards';

describe('guards', () => {
  it('isServerNotification accepts a method+params envelope', () => {
    expect(isServerNotification({ method: 'error', params: { message: 'x' } })).toBe(true);
  });

  it('isServerRequest accepts method+params+id, isServerNotification rejects it', () => {
    const sr = {
      method: 'item/commandExecution/requestApproval',
      id: 5,
      params: { threadId: 't' },
    };
    expect(isServerRequest(sr)).toBe(true);
    expect(isServerNotification(sr)).toBe(false);
  });

  it('isServerNotification rejects non-objects', () => {
    expect(isServerNotification(null)).toBe(false);
    expect(isServerNotification('hello')).toBe(false);
    expect(isServerNotification(42)).toBe(false);
  });

  it('hasMethod narrows to the picked variant', () => {
    const ev = {
      method: 'turn/started',
      params: {
        threadId: 't',
        turn: { id: 'tu', items: [], itemsView: 'full', status: 'inProgress', error: null, startedAt: null, completedAt: null, durationMs: null },
      },
    } as const;
    expect(hasMethod(ev as never, 'turn/started')).toBe(true);
  });
});
```

- [ ] **Step 3: 跑测试 + 类型检查**

```bash
cd /Users/def/lemurclaw/codex-rs/lemurclaw-gui/assets
npm test -- guards
npx tsc --noEmit
```
Expected: 4 个 test 通过,0 TS errors。

- [ ] **Step 4: Commit**

```bash
git add codex-rs/lemurclaw-gui/assets/src/types/guards.ts codex-rs/lemurclaw-gui/assets/src/types/guards.test.ts
git commit -m "feat(gui): ServerNotification/Request type guards"
```

---

## Task 3.4:ViewModel 投影层 + reducer

**目标:** 把 ServerNotification 流增量折叠成 `ConversationState`,React 组件只读 ViewModel。这是整个子项目 3 的核心数据层。

**核心设计:**
- `ConversationState` = `{ turns: TurnModel[]; activeTurnId: string | null; status: ThreadStatusModel | null; pendingApprovals: PendingApproval[] }`
- `TurnModel` = `{ id: string; status: TurnStatus; items: CellModel[]; startedAt: number | null; completedAt: number | null }`
- `CellModel` 是 discriminated union,每类 cell 一个变体(`{ kind: 'agentMessage'; itemId: string; text: string; phase: MessagePhase | null }` 等)。
- `reducer(state, event) → state`:对 `turn/started` 加 turn;对 `item/started` 加 cell;对 `item/agentMessage/delta` 找到对应 cell append delta;对 `item/completed` 替换 cell(权威快照);对 `ServerRequest::*Approval` 加 pendingApproval;对 `serverRequest/resolved` 移除 pendingApproval。
- itemId 索引 cell(reducer 内部 `turns[].items.find(itemId)`,O(n),agent 对话 < 500 items 可接受)。

**Files:**
- Create: `codex-rs/lemurclaw-gui/assets/src/viewModel/types.ts`
- Create: `codex-rs/lemurclaw-gui/assets/src/viewModel/reducer.ts`
- Create: `codex-rs/lemurclaw-gui/assets/src/viewModel/reducer.test.ts`

- [ ] **Step 1: 实现 ViewModel 类型(types.ts)**

Create `codex-rs/lemurclaw-gui/assets/src/viewModel/types.ts`:
```ts
// ViewModel types: the shape React components read. The reducer (reducer.ts)
// is the single place that produces these from raw ServerNotification /
// ServerRequest events. Components never see raw events.
//
// Design rules:
// - All fields nullable-by-default at construction; reducer fills them in.
// - CellModel is a discriminated union on `kind`; each variant maps 1:1 to a
//   ThreadItem.type.
// - Timestamps are ms since epoch where applicable (matching codex's *AtMs).

import type { MessagePhase, PatchChangeKind, CommandExecutionStatus, CommandExecutionSource, PatchApplyStatus, McpToolCallStatus, DynamicToolCallStatus, ThreadStatus, TurnStatus, HookRunSummary, CommandAction, FileUpdateChange } from '../types/v2';
import type { RequestId } from '../types';
import type { ServerRequest } from '../types/ServerRequest';

export interface ConversationState {
  /** Ordered turns (oldest first). Each turn owns an ordered items list. */
  turns: TurnModel[];
  /** Active turn id (from most recent turn/started), null pre-first-turn or after turn/completed. */
  activeTurnId: string | null;
  /** Current thread status, or null pre-thread/started. */
  status: ThreadStatusModel | null;
  /** Pending ServerRequests awaiting user decision (ApprovalCard queue). */
  pendingApprovals: PendingApproval[];
}

export type ThreadStatusModel = ThreadStatus;

export interface TurnModel {
  id: string;
  status: TurnStatus;
  items: CellModel[];
  startedAt: number | null;
  completedAt: number | null;
}

export type CellModel =
  | { kind: 'userMessage'; itemId: string; text: string }
  | { kind: 'agentMessage'; itemId: string; text: string; phase: MessagePhase | null }
  | { kind: 'reasoning'; itemId: string; summary: string[]; content: string[] }
  | {
      kind: 'commandExecution';
      itemId: string;
      command: string;
      cwd: string;
      status: CommandExecutionStatus;
      source: CommandExecutionSource;
      aggregatedOutput: string;
      exitCode: number | null;
      durationMs: number | null;
    }
  | {
      kind: 'fileChange';
      itemId: string;
      changes: Array<{ path: string; kind: PatchChangeKind; diff: string }>;
      status: PatchApplyStatus;
    }
  | {
      kind: 'mcpToolCall';
      itemId: string;
      server: string;
      tool: string;
      status: McpToolCallStatus;
      arguments: unknown;
      progress: string[];
      result: unknown;
      error: string | null;
    }
  | { kind: 'dynamicToolCall'; itemId: string; tool: string; status: DynamicToolCallStatus; arguments: unknown; progress: string[] }
  | { kind: 'plan'; itemId: string; text: string }
  | { kind: 'hook'; run: HookRunSummary }
  | { kind: 'webSearch'; itemId: string; query: string; status: string }
  | { kind: 'imageGeneration'; itemId: string; prompt: string }
  | { kind: 'sleep'; itemId: string; durationMs: number }
  | { kind: 'generic'; itemId: string; rawType: string };

/** Pending ServerRequest awaiting user decision (approval/elicitation). */
export interface PendingApproval {
  /** ServerRequest.id (RequestId). Used to resolve/reject via transport. */
  requestId: RequestId;
  /** Discriminator for ApprovalCard rendering. */
  kind: ApprovalKind;
  /** Original ServerRequest envelope (kept for advanced fields). */
  raw: ServerRequest;
}

export type ApprovalKind =
  | 'commandExecution'
  | 'fileChange'
  | 'mcpElicitation'
  | 'permissions'
  | 'toolUserInput'
  | 'generic';

export const initialState: ConversationState = {
  turns: [],
  activeTurnId: null,
  status: null,
  pendingApprovals: [],
};

// Helpers (re-exported for components that want typed local copies) --------

export interface FileChangeEntry {
  path: string;
  kind: PatchChangeKind;
  diff: string;
}

export function toFileChangeEntry(c: FileUpdateChange): FileChangeEntry {
  return { path: c.path, kind: c.kind, diff: c.diff };
}

export function summarizeCommandActions(actions: CommandAction[] | null | undefined): string {
  if (!actions || actions.length === 0) return '';
  return actions.map((a) => a.name || a.type).join(' | ');
}
```

- [ ] **Step 2: 写 reducer.test.ts(关键 case 的测试,先 fail)**

Create `codex-rs/lemurclaw-gui/assets/src/viewModel/reducer.test.ts`:
```ts
import { describe, it, expect } from 'vitest';
import { reducer } from './reducer';
import { initialState } from './types';
import type { ConversationState } from './types';

function st(over: Partial<ConversationState> = {}): ConversationState {
  return { ...initialState, ...over };
}

const FULL_THREAD = {
  id: 't1', sessionId: 's', forkedFromId: null, parentThreadId: null,
  preview: '', ephemeral: false, modelProvider: 'p', createdAt: 1,
  updatedAt: 1, recencyAt: null, status: { type: 'idle' }, path: null,
  cwd: { path: '/x' }, cliVersion: '0', source: 'Cli', threadSource: null,
  agentNickname: null, agentRole: null, gitInfo: null, name: null, turns: [],
} as const;

const EMPTY_TURN = {
  id: 'tu1', items: [], itemsView: 'full' as const, status: 'inProgress' as const,
  error: null, startedAt: 1, completedAt: null, durationMs: null,
} as const;

describe('reducer', () => {
  it('thread/started sets status (idle) and clears activeTurnId', () => {
    const next = reducer(st(), { method: 'thread/started', params: { thread: FULL_THREAD } });
    expect(next.status).toEqual({ type: 'idle' });
    expect(next.activeTurnId).toBeNull();
  });

  it('turn/started adds a new turn and sets activeTurnId', () => {
    const next = reducer(st(), { method: 'turn/started', params: { threadId: 't1', turn: EMPTY_TURN } });
    expect(next.turns).toHaveLength(1);
    expect(next.turns[0].id).toBe('tu1');
    expect(next.activeTurnId).toBe('tu1');
  });

  it('item/started adds a cell derived from the ThreadItem', () => {
    const afterTurn = reducer(st(), { method: 'turn/started', params: { threadId: 't1', turn: EMPTY_TURN } });
    const afterStart = reducer(afterTurn, {
      method: 'item/started',
      params: {
        threadId: 't1', turnId: 'tu1', startedAtMs: 5,
        item: { type: 'agentMessage', id: 'i1', text: '', phase: null, memoryCitation: null },
      },
    });
    expect(afterStart.turns[0].items).toHaveLength(1);
    expect(afterStart.turns[0].items[0].kind).toBe('agentMessage');
  });

  it('item/agentMessage/delta appends to the streaming cell', () => {
    const afterTurn = reducer(st(), { method: 'turn/started', params: { threadId: 't1', turn: EMPTY_TURN } });
    const afterStart = reducer(afterTurn, {
      method: 'item/started',
      params: { threadId: 't1', turnId: 'tu1', startedAtMs: 5, item: { type: 'agentMessage', id: 'i1', text: '', phase: null, memoryCitation: null } },
    });
    const afterDelta = reducer(afterStart, {
      method: 'item/agentMessage/delta',
      params: { threadId: 't1', turnId: 'tu1', itemId: 'i1', delta: 'Hello' },
    });
    const cell = afterDelta.turns[0].items.find((c) => c.kind === 'agentMessage');
    expect(cell && cell.kind === 'agentMessage' && cell.text).toBe('Hello');
  });

  it('item/completed replaces the streaming cell with the authoritative snapshot', () => {
    const afterTurn = reducer(st(), { method: 'turn/started', params: { threadId: 't1', turn: EMPTY_TURN } });
    const afterStart = reducer(afterTurn, {
      method: 'item/started', params: { threadId: 't1', turnId: 'tu1', startedAtMs: 5, item: { type: 'agentMessage', id: 'i1', text: '', phase: null, memoryCitation: null } },
    });
    const afterDelta = reducer(afterStart, {
      method: 'item/agentMessage/delta', params: { threadId: 't1', turnId: 'tu1', itemId: 'i1', delta: 'Hel' },
    });
    const afterComplete = reducer(afterDelta, {
      method: 'item/completed',
      params: {
        threadId: 't1', turnId: 'tu1', completedAtMs: 9,
        item: { type: 'agentMessage', id: 'i1', text: 'Hello world', phase: 'final_answer', memoryCitation: null },
      },
    });
    const cell = afterComplete.turns[0].items.find((c) => c.kind === 'agentMessage');
    expect(cell && cell.kind === 'agentMessage' && cell.text).toBe('Hello world');
    expect(cell && cell.kind === 'agentMessage' && cell.phase).toBe('final_answer');
  });

  it('ServerRequest commandExecution adds a pendingApproval', () => {
    const next = reducer(st(), {
      method: 'item/commandExecution/requestApproval', id: 42,
      params: { threadId: 't1', turnId: 'tu1', itemId: 'i1', startedAtMs: 1, environmentId: null, command: 'ls', cwd: { path: '/x' }, commandActions: null },
    });
    expect(next.pendingApprovals).toHaveLength(1);
    expect(next.pendingApprovals[0].kind).toBe('commandExecution');
    expect(next.pendingApprovals[0].requestId).toBe(42);
  });

  it('serverRequest/resolved removes the matching pendingApproval', () => {
    const afterReq = reducer(st(), {
      method: 'item/commandExecution/requestApproval', id: 42,
      params: { threadId: 't1', turnId: 'tu1', itemId: 'i1', startedAtMs: 1, environmentId: null, command: 'ls', cwd: { path: '/x' }, commandActions: null },
    });
    const afterResolved = reducer(afterReq, {
      method: 'serverRequest/resolved',
      params: { requestId: 42 },
    });
    expect(afterResolved.pendingApprovals).toHaveLength(0);
  });

  it('turn/completed marks the active turn completed', () => {
    const afterTurn = reducer(st(), { method: 'turn/started', params: { threadId: 't1', turn: EMPTY_TURN } });
    const afterComplete = reducer(afterTurn, {
      method: 'turn/completed',
      params: { threadId: 't1', turn: { ...EMPTY_TURN, status: 'completed', completedAt: 2, durationMs: 1000 } },
    });
    expect(afterComplete.turns[0].status).toBe('completed');
    expect(afterComplete.activeTurnId).toBeNull();
  });
});
```

> **注:** Task 3.4 Step 5 实现前先 `cat codex-rs/lemurclaw-gui/assets/src/types/v2/ServerRequestResolvedNotification.ts` 核实 `requestId` 字段名(假设为 `requestId: RequestId`)。

- [ ] **Step 3: 跑测试,确认它们都 fail(reducer 未实现)**

```bash
npm test -- reducer
```
Expected: 全部 fail(import error / reducer 未定义)。

- [ ] **Step 4: 实现 reducer.ts**

Create `codex-rs/lemurclaw-gui/assets/src/viewModel/reducer.ts`:
```ts
// ViewModel reducer: fold a single ServerNotification or ServerRequest into
// the ConversationState. Pure function. Called from App.tsx's onEvent handler.
//
// Strategy:
// - turn/started → append a new TurnModel, set activeTurnId
// - item/started → upsert a CellModel derived from ThreadItem (initial state)
// - item/*/delta → find the cell by itemId in the active turn, stream-append
// - item/completed → upsert cell with the authoritative ThreadItem snapshot
// - ServerRequest::*Approval/*Elicitation → push PendingApproval
// - serverRequest/resolved → drop matching PendingApproval
// - turn/completed → mark active turn completed, clear activeTurnId
// - thread/status/changed → update status
//
// Item lookup: O(n) over active turn items. Conversation lengths in normal
// agent runs are < 500 items; if this becomes hot, switch to a Map index.

import type { ConversationState, TurnModel, CellModel, PendingApproval, ApprovalKind } from './types';
import { initialState } from './types';
import { isServerNotification, isServerRequest } from '../types/guards';
import type { ServerNotification, ServerRequest } from '../types/ServerRequest';
import type { ThreadItem, Turn, HookRunSummary } from '../types/v2';

export function reducer(state: ConversationState, event: unknown): ConversationState {
  if (isServerNotification(event)) {
    return applyNotification(state, event);
  }
  if (isServerRequest(event)) {
    return applyServerRequest(state, event);
  }
  // Unknown event shape (e.g. backend's `{lagged, skipped}` envelope).
  return state;
}

function applyNotification(state: ConversationState, n: ServerNotification): ConversationState {
  switch (n.method) {
    case 'thread/started':
      return { ...state, status: n.params.thread.status, activeTurnId: null };
    case 'thread/status/changed':
      return { ...state, status: n.params.status };
    case 'turn/started':
      return applyTurnStarted(state, n.params.turn);
    case 'turn/completed':
      return applyTurnCompleted(state, n.params.turn);
    case 'item/started':
      return applyItemEvent(state, n.params.turnId, n.params.item);
    case 'item/completed':
      return applyItemEvent(state, n.params.turnId, n.params.item);
    case 'item/agentMessage/delta':
      return applyItemDelta(state, n.params.turnId, n.params.itemId, (cell) => {
        if (cell.kind !== 'agentMessage') return cell;
        return { ...cell, text: cell.text + n.params.delta };
      });
    case 'item/reasoning/summaryTextDelta':
      return applyItemDelta(state, n.params.turnId, n.params.itemId, (cell) => {
        if (cell.kind !== 'reasoning') return cell;
        const summary = [...cell.summary];
        summary[n.params.summaryIndex] = (summary[n.params.summaryIndex] ?? '') + n.params.delta;
        return { ...cell, summary };
      });
    case 'item/reasoning/textDelta':
      return applyItemDelta(state, n.params.turnId, n.params.itemId, (cell) => {
        if (cell.kind !== 'reasoning') return cell;
        const content = [...cell.content];
        content[n.params.contentIndex] = (content[n.params.contentIndex] ?? '') + n.params.delta;
        return { ...cell, content };
      });
    case 'item/commandExecution/outputDelta':
      return applyItemDelta(state, n.params.turnId, n.params.itemId, (cell) => {
        if (cell.kind !== 'commandExecution') return cell;
        return { ...cell, aggregatedOutput: cell.aggregatedOutput + n.params.delta };
      });
    case 'item/fileChange/patchUpdated':
      return applyItemDelta(state, n.params.turnId, n.params.itemId, (cell) => {
        if (cell.kind !== 'fileChange') return cell;
        return { ...cell, changes: n.params.changes.map((c) => ({ path: c.path, kind: c.kind, diff: c.diff })) };
      });
    case 'item/mcpToolCall/progress':
      return applyItemDelta(state, n.params.turnId, n.params.itemId, (cell) => {
        if (cell.kind !== 'mcpToolCall' && cell.kind !== 'dynamicToolCall') return cell;
        return { ...cell, progress: [...cell.progress, n.params.message] };
      });
    case 'item/plan/delta':
      // EXPERIMENTAL per upstream type. Concatenate deltas; ItemCompleted
      // will replace with the authoritative plan text.
      return applyItemDelta(state, n.params.turnId, n.params.itemId, (cell) => {
        if (cell.kind !== 'plan') return cell;
        return { ...cell, text: cell.text + n.params.delta };
      });
    case 'hook/started':
    case 'hook/completed':
      // Hook cells are tied to a turn (turnId may be null pre-first-turn).
      // If null, drop the event (no Scrollback anchor yet — subproject 3
      // limitation). Otherwise upsert a hook cell on the named turn.
      if (n.params.turnId === null) return state;
      return applyHookEvent(state, n.params.turnId, n.params.run);
    case 'serverRequest/resolved':
      return {
        ...state,
        pendingApprovals: state.pendingApprovals.filter(
          (a) => String(a.requestId) !== String(n.params.requestId),
        ),
      };
    default:
      // Many notification methods (rawResponseItem/completed, model/*,
      // thread/realtime/*, account/*, ...) are not yet rendered in subproject
      // 3. Silently ignore; they remain visible in the dev tools console.
      return state;
  }
}

function applyServerRequest(state: ConversationState, r: ServerRequest): ConversationState {
  const kind = approvalKindFor(r);
  const approval: PendingApproval = { requestId: r.id, kind, raw: r };
  return { ...state, pendingApprovals: [...state.pendingApprovals, approval] };
}

function approvalKindFor(r: ServerRequest): ApprovalKind {
  switch (r.method) {
    case 'item/commandExecution/requestApproval': return 'commandExecution';
    case 'item/fileChange/requestApproval': return 'fileChange';
    case 'mcpServer/elicitation/request': return 'mcpElicitation';
    case 'item/permissions/requestApproval': return 'permissions';
    case 'item/tool/requestUserInput': return 'toolUserInput';
    default: return 'generic';
  }
}

function applyTurnStarted(state: ConversationState, turn: Turn): ConversationState {
  const turnModel: TurnModel = {
    id: turn.id,
    status: turn.status,
    items: turn.items.map(threadItemToCell),
    startedAt: turn.startedAt,
    completedAt: turn.completedAt,
  };
  return { ...state, turns: [...state.turns, turnModel], activeTurnId: turn.id };
}

function applyTurnCompleted(state: ConversationState, turn: Turn): ConversationState {
  const turns = state.turns.map((t) =>
    t.id === turn.id
      ? { ...t, status: turn.status, completedAt: turn.completedAt, items: turn.items.map(threadItemToCell) }
      : t,
  );
  return { ...state, turns, activeTurnId: null };
}

function applyItemEvent(state: ConversationState, turnId: string, item: ThreadItem): ConversationState {
  const cell = threadItemToCell(item);
  const turns = state.turns.map((t) => {
    if (t.id !== turnId) return t;
    const existingIdx = t.items.findIndex((c) => 'itemId' in c && c.itemId === item.id);
    if (existingIdx >= 0) {
      const items = [...t.items];
      items[existingIdx] = cell;
      return { ...t, items };
    }
    return { ...t, items: [...t.items, cell] };
  });
  return { ...state, turns };
}

function applyItemDelta(state: ConversationState, turnId: string, itemId: string, mutate: (cell: CellModel) => CellModel): ConversationState {
  const turns = state.turns.map((t) => {
    if (t.id !== turnId) return t;
    const items = t.items.map((c) => ('itemId' in c && c.itemId === itemId ? mutate(c) : c));
    return { ...t, items };
  });
  return { ...state, turns };
}

function applyHookEvent(state: ConversationState, turnId: string, run: HookRunSummary): ConversationState {
  // Upsert by run.id (HookRunSummary.id is unique per run).
  const turns = state.turns.map((t) => {
    if (t.id !== turnId) return t;
    const existingIdx = t.items.findIndex((c) => c.kind === 'hook' && c.run.id === run.id);
    if (existingIdx >= 0) {
      const items = [...t.items];
      items[existingIdx] = { kind: 'hook', run };
      return { ...t, items };
    }
    return { ...t, items: [...t.items, { kind: 'hook', run }] };
  });
  return { ...state, turns };
}

/** Map a raw ThreadItem (from upstream types) to our CellModel. Centralizes
 *  the upstream → ViewModel translation so components stay dumb. */
export function threadItemToCell(item: ThreadItem): CellModel {
  switch (item.type) {
    case 'userMessage': {
      const text = item.content
        .filter((c) => c.type === 'text')
        .map((c) => (c as { type: 'text'; text: string }).text)
        .join('\n');
      return { kind: 'userMessage', itemId: item.id, text };
    }
    case 'agentMessage':
      return { kind: 'agentMessage', itemId: item.id, text: item.text, phase: item.phase };
    case 'reasoning':
      return { kind: 'reasoning', itemId: item.id, summary: item.summary, content: item.content };
    case 'commandExecution':
      return {
        kind: 'commandExecution',
        itemId: item.id,
        command: item.command,
        cwd: typeof item.cwd === 'string' ? item.cwd : (item.cwd as { path?: string }).path ?? String(item.cwd),
        status: item.status,
        source: item.source,
        aggregatedOutput: item.aggregatedOutput ?? '',
        exitCode: item.exitCode,
        durationMs: item.durationMs,
      };
    case 'fileChange':
      return {
        kind: 'fileChange',
        itemId: item.id,
        changes: item.changes.map((c) => ({ path: c.path, kind: c.kind, diff: c.diff })),
        status: item.status,
      };
    case 'mcpToolCall':
      return {
        kind: 'mcpToolCall',
        itemId: item.id,
        server: item.server,
        tool: item.tool,
        status: item.status,
        arguments: item.arguments,
        progress: [],
        result: item.result,
        error: item.error ? JSON.stringify(item.error) : null,
      };
    case 'dynamicToolCall':
      return {
        kind: 'dynamicToolCall',
        itemId: item.id,
        tool: item.tool,
        status: item.status,
        arguments: item.arguments,
        progress: [],
      };
    case 'plan':
      return { kind: 'plan', itemId: item.id, text: item.text };
    case 'webSearch': {
      const q = (item as unknown as { query?: string }).query ?? '';
      return { kind: 'webSearch', itemId: item.id, query: q, status: 'completed' };
    }
    case 'imageGeneration':
      return { kind: 'imageGeneration', itemId: item.id, prompt: (item as unknown as { prompt?: string }).prompt ?? '' };
    case 'sleep':
      return { kind: 'sleep', itemId: item.id, durationMs: 0 };
    case 'hookPrompt':
    case 'collabAgentToolCall':
    case 'subAgentActivity':
    case 'imageView':
    case 'enteredReviewMode':
    case 'exitedReviewMode':
    case 'contextCompaction':
      return { kind: 'generic', itemId: 'id' in item ? item.id : '', rawType: item.type };
  }
}
```

> **实现注:**
> 1. `cwd` 处理:LegacyAppPathString 可能是 branded string 或 `{ path }`。代码用 `typeof item.cwd === 'string' ? ... : item.cwd.path ?? String(...)` 双路径兜底。
> 2. `ThreadItem` 的 webSearch/imageGeneration/sleep 是 intersection(`& WebSearchItem`),TS narrowing 后访问额外字段需要 cast(已用 `as unknown as`)。
> 3. switch 必须覆盖所有 ThreadItem.type 变体(否则 TS 报 non-exhaustive);generic fallback 处理未渲染的 7 个变体。

- [ ] **Step 5: 跑测试 + 类型检查,修字段路径问题**

```bash
cd /Users/def/lemurclaw/codex-rs/lemurclaw-gui/assets
npm test -- reducer
npx tsc --noEmit
```
Expected: 8 个 reducer test 通过。常见修法:
- `LegacyAppPathString` 不是 `{path}` 也不是 string(可能是 opaque branded)→ 改 `cwd: String(item.cwd as unknown as string)`
- `ServerRequestResolvedNotification.requestId` 字段名不同 → cat 文件核实后调整 reducer + test
- ThreadItem 变体不全 → switch 补 default 兜底(但 TS 会报 switch 不 exhaustive;补齐所有变体即可)

- [ ] **Step 6: Commit**

```bash
git add codex-rs/lemurclaw-gui/assets/src/viewModel/
git commit -m "feat(gui): ViewModel reducer for ServerNotification stream"
```

---

## Task 3.5:基础 cell 组件(user / agent / reasoning / command-exec)

**目标:** 实现最常用的 4 类 cell 组件 + 组件单测。

**Files:**
- Create: `codex-rs/lemurclaw-gui/assets/src/components/cells/UserMessageCell.tsx`
- Create: `codex-rs/lemurclaw-gui/assets/src/components/cells/AgentMessageCell.tsx`
- Create: `codex-rs/lemurclaw-gui/assets/src/components/cells/ReasoningCell.tsx`
- Create: `codex-rs/lemurclaw-gui/assets/src/components/cells/CommandExecCell.tsx`
- Create: 4 个对应 `__tests__/*.test.tsx`

- [ ] **Step 1: UserMessageCell.tsx + 测试**

Create `codex-rs/lemurclaw-gui/assets/src/components/cells/UserMessageCell.tsx`:
```tsx
import type { CellModel } from '../../viewModel/types';

interface Props {
  model: Extract<CellModel, { kind: 'userMessage' }>;
}

/** User-authored message cell. Renders the plain text; subproject 3 does not
 *  handle image/audio/skill/mention inputs (those land in the composer). */
export function UserMessageCell({ model }: Props) {
  return (
    <div className="cell cell-user" data-testid="user-message">
      <div className="cell-role">user</div>
      <div className="cell-body">
        <pre className="cell-text">{model.text}</pre>
      </div>
    </div>
  );
}
```

Create `codex-rs/lemurclaw-gui/assets/src/components/cells/__tests__/UserMessageCell.test.tsx`:
```tsx
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { UserMessageCell } from '../UserMessageCell';

describe('UserMessageCell', () => {
  it('renders the message text', () => {
    render(<UserMessageCell model={{ kind: 'userMessage', itemId: 'u1', text: 'hello world' }} />);
    expect(screen.getByTestId('user-message')).toHaveTextContent('hello world');
  });
});
```

- [ ] **Step 2: AgentMessageCell.tsx + 测试**

Create `codex-rs/lemurclaw-gui/assets/src/components/cells/AgentMessageCell.tsx`:
```tsx
import type { CellModel } from '../../viewModel/types';

interface Props {
  model: Extract<CellModel, { kind: 'agentMessage' }>;
}

/** Assistant message cell. Renders streamed + final text identically (the
 *  reducer already keeps the latest snapshot). `phase: 'final_answer'` gets a
 *  subtle visual marker; null or 'commentary' renders without it. */
export function AgentMessageCell({ model }: Props) {
  const isFinal = model.phase === 'final_answer';
  return (
    <div className={`cell cell-agent${isFinal ? ' cell-agent-final' : ''}`} data-testid="agent-message">
      <div className="cell-role">assistant{isFinal ? '' : ' · thinking'}</div>
      <div className="cell-body">
        <pre className="cell-text">{model.text}</pre>
      </div>
    </div>
  );
}
```

Create `codex-rs/lemurclaw-gui/assets/src/components/cells/__tests__/AgentMessageCell.test.tsx`:
```tsx
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { AgentMessageCell } from '../AgentMessageCell';

describe('AgentMessageCell', () => {
  it('renders streamed text with thinking marker when phase is null', () => {
    render(<AgentMessageCell model={{ kind: 'agentMessage', itemId: 'a1', text: 'let me think', phase: null }} />);
    expect(screen.getByTestId('agent-message')).toHaveTextContent('let me think');
    expect(screen.getByTestId('agent-message')).toHaveTextContent('thinking');
  });

  it('marks final_answer without the thinking suffix', () => {
    render(<AgentMessageCell model={{ kind: 'agentMessage', itemId: 'a1', text: 'done', phase: 'final_answer' }} />);
    const cell = screen.getByTestId('agent-message');
    expect(cell).toHaveTextContent('done');
    expect(cell).not.toHaveTextContent('thinking');
    expect(cell.className).toContain('cell-agent-final');
  });
});
```

- [ ] **Step 3: ReasoningCell.tsx + 测试**

Create `codex-rs/lemurclaw-gui/assets/src/components/cells/ReasoningCell.tsx`:
```tsx
import { useState } from 'react';
import type { CellModel } from '../../viewModel/types';

interface Props {
  model: Extract<CellModel, { kind: 'reasoning' }>;
}

/** Reasoning cell. Collapsed by default (summary only); click to expand and
 *  show full content array. */
export function ReasoningCell({ model }: Props) {
  const [expanded, setExpanded] = useState(false);
  const hasContent = model.content.length > 0 || model.summary.length > 0;
  return (
    <div className="cell cell-reasoning" data-testid="reasoning">
      <button
        className="cell-reasoning-toggle"
        onClick={() => setExpanded((e) => !e)}
        aria-expanded={expanded}
      >
        reasoning {expanded ? '▾' : '▸'}
      </button>
      {hasContent ? (
        <div className="cell-body">
          {model.summary.map((s, i) => (
            <pre key={`s${i}`} className="cell-text cell-reasoning-summary">{s}</pre>
          ))}
          {expanded &&
            model.content.map((c, i) => (
              <pre key={`c${i}`} className="cell-text cell-reasoning-content">{c}</pre>
            ))}
        </div>
      ) : (
        <div className="cell-body cell-empty">(empty)</div>
      )}
    </div>
  );
}
```

Create `codex-rs/lemurclaw-gui/assets/src/components/cells/__tests__/ReasoningCell.test.tsx`:
```tsx
import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ReasoningCell } from '../ReasoningCell';

describe('ReasoningCell', () => {
  it('shows summary by default, hides content until expanded', () => {
    render(<ReasoningCell model={{ kind: 'reasoning', itemId: 'r1', summary: ['short'], content: ['long detail'] }} />);
    expect(screen.getByText('short')).toBeInTheDocument();
    expect(screen.queryByText('long detail')).toBeNull();
    fireEvent.click(screen.getByRole('button'));
    expect(screen.getByText('long detail')).toBeInTheDocument();
  });

  it('renders empty marker when no content', () => {
    render(<ReasoningCell model={{ kind: 'reasoning', itemId: 'r1', summary: [], content: [] }} />);
    expect(screen.getByTestId('reasoning')).toHaveTextContent('(empty)');
  });
});
```

- [ ] **Step 4: CommandExecCell.tsx + 测试**

Create `codex-rs/lemurclaw-gui/assets/src/components/cells/CommandExecCell.tsx`:
```tsx
import { useState } from 'react';
import type { CellModel } from '../../viewModel/types';

type Model = Extract<CellModel, { kind: 'commandExecution' }>;

interface Props {
  model: Model;
}

/** Command-execution cell. Shows command + cwd + status; output collapsible. */
export function CommandExecCell({ model }: Props) {
  const [expanded, setExpanded] = useState(false);
  const statusLabel = labelForStatus(model.status, model.exitCode);
  return (
    <div className={`cell cell-exec cell-exec-${model.status}`} data-testid="exec">
      <div className="cell-exec-header">
        <code className="cell-exec-command">$ {model.command}</code>
        <span className="cell-exec-status">{statusLabel}</span>
        <button onClick={() => setExpanded((e) => !e)} aria-expanded={expanded}>
          {expanded ? 'hide output' : 'show output'}
        </button>
      </div>
      <div className="cell-exec-cwd">{model.cwd}</div>
      {expanded && model.aggregatedOutput && (
        <pre className="cell-exec-output" data-testid="exec-output">{model.aggregatedOutput}</pre>
      )}
    </div>
  );
}

function labelForStatus(status: Model['status'], exitCode: number | null): string {
  switch (status) {
    case 'inProgress': return 'running';
    case 'completed': return exitCode === 0 ? '✓ exit 0' : `✓ exit ${exitCode}`;
    case 'failed': return `✗ exit ${exitCode}`;
    case 'declined': return 'declined';
  }
}
```

Create `codex-rs/lemurclaw-gui/assets/src/components/cells/__tests__/CommandExecCell.test.tsx`:
```tsx
import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { CommandExecCell } from '../CommandExecCell';

describe('CommandExecCell', () => {
  it('shows command + status, hides output until expanded', () => {
    render(<CommandExecCell model={{ kind: 'commandExecution', itemId: 'e1', command: 'cargo build', cwd: '/proj', status: 'completed', source: 'agent', aggregatedOutput: 'Compiling...', exitCode: 0, durationMs: 1234 }} />);
    expect(screen.getByText(/cargo build/)).toBeInTheDocument();
    expect(screen.getByText('✓ exit 0')).toBeInTheDocument();
    expect(screen.queryByText('Compiling...')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: /show output/ }));
    expect(screen.getByTestId('exec-output')).toHaveTextContent('Compiling...');
  });

  it('marks failed status with non-zero exit', () => {
    render(<CommandExecCell model={{ kind: 'commandExecution', itemId: 'e2', command: 'false', cwd: '/x', status: 'failed', source: 'agent', aggregatedOutput: '', exitCode: 1, durationMs: 10 }} />);
    expect(screen.getByText('✗ exit 1')).toBeInTheDocument();
  });
});
```

- [ ] **Step 5: 跑测试 + 类型检查**

```bash
cd /Users/def/lemurclaw/codex-rs/lemurclaw-gui/assets
npm test -- cells/__tests__
npx tsc --noEmit
```
Expected: 7 个 test 通过(UserMessageCell 1 + AgentMessageCell 2 + ReasoningCell 2 + CommandExecCell 2)。

- [ ] **Step 6: Commit**

```bash
git add codex-rs/lemurclaw-gui/assets/src/components/cells/
git commit -m "feat(gui): user/agent/reasoning/exec cell components + tests"
```

---

## Task 3.6:剩余 cell 组件(fileChange / mcp / plan / hook / webSearch)

**目标:** 补齐 spec §4.2 矩阵里的剩余 cell 类型。

**Files:**
- Create: `codex-rs/lemurclaw-gui/assets/src/components/cells/FileChangeCell.tsx`
- Create: `codex-rs/lemurclaw-gui/assets/src/components/cells/McpToolCell.tsx`
- Create: `codex-rs/lemurclaw-gui/assets/src/components/cells/PlanCell.tsx`
- Create: `codex-rs/lemurclaw-gui/assets/src/components/cells/HookCell.tsx`
- Create: `codex-rs/lemurclaw-gui/assets/src/components/cells/WebSearchCell.tsx`
- Create: 5 个对应 `__tests__/*.test.tsx`

- [ ] **Step 1: FileChangeCell.tsx + 测试**

Create `codex-rs/lemurclaw-gui/assets/src/components/cells/FileChangeCell.tsx`:
```tsx
import { useState } from 'react';
import type { CellModel } from '../../viewModel/types';

type Model = Extract<CellModel, { kind: 'fileChange' }>;

interface Props {
  model: Model;
}

/** File-change (patch) cell. Lists changed files with +/- markers; each
 *  file's diff is collapsible. Apply status shown as a badge. */
export function FileChangeCell({ model }: Props) {
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const statusBadge = labelForPatchStatus(model.status);
  return (
    <div className={`cell cell-patch cell-patch-${model.status}`} data-testid="patch">
      <div className="cell-patch-header">
        <span className="cell-patch-title">📝 patch</span>
        <span className="cell-patch-status">{statusBadge}</span>
        <span className="cell-patch-count">{model.changes.length} file(s)</span>
      </div>
      <ul className="cell-patch-files">
        {model.changes.map((c, i) => {
          const key = `${c.path}:${i}`;
          const isOpen = open[key] ?? false;
          return (
            <li key={key} className={`cell-patch-file cell-patch-file-${c.kind.type}`}>
              <button
                className="cell-patch-file-toggle"
                aria-expanded={isOpen}
                onClick={() => setOpen((o) => ({ ...o, [key]: !o[key] }))}
              >
                <span className="cell-patch-kind">{kindMarker(c.kind.type)}</span>
                <code>{c.path}</code>
              </button>
              {isOpen && c.diff && <pre className="cell-patch-diff">{c.diff}</pre>}
            </li>
          );
        })}
      </ul>
    </div>
  );
}

function labelForPatchStatus(status: Model['status']): string {
  switch (status) {
    case 'inProgress': return 'applying';
    case 'completed': return 'applied';
    case 'failed': return 'failed';
    case 'declined': return 'declined';
  }
}

function kindMarker(t: 'add' | 'delete' | 'update'): string {
  switch (t) {
    case 'add': return '+';
    case 'delete': return '-';
    case 'update': return '~';
  }
}
```

Create `codex-rs/lemurclaw-gui/assets/src/components/cells/__tests__/FileChangeCell.test.tsx`:
```tsx
import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { FileChangeCell } from '../FileChangeCell';

describe('FileChangeCell', () => {
  it('lists files with kind markers and hides diff until clicked', () => {
    render(<FileChangeCell model={{
      kind: 'fileChange', itemId: 'p1', status: 'completed',
      changes: [
        { path: 'src/a.rs', kind: { type: 'add' }, diff: '+new' },
        { path: 'src/b.rs', kind: { type: 'update', move_path: null }, diff: '-old\n+new' },
      ],
    }} />);
    expect(screen.getByText('src/a.rs')).toBeInTheDocument();
    expect(screen.getByText('src/b.rs')).toBeInTheDocument();
    expect(screen.queryByText('+new')).toBeNull();
    fireEvent.click(screen.getByText('src/a.rs'));
    expect(screen.getByText('+new')).toBeInTheDocument();
    expect(screen.getByText('applied')).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: McpToolCell.tsx + 测试**

Create `codex-rs/lemurclaw-gui/assets/src/components/cells/McpToolCell.tsx`:
```tsx
import { useState } from 'react';
import type { CellModel } from '../../viewModel/types';

interface Props {
  model: Extract<CellModel, { kind: 'mcpToolCall' }>;
}

/** MCP tool-call cell. Shows server/tool + status; args + progress + result
 *  collapsible. */
export function McpToolCell({ model }: Props) {
  const [expanded, setExpanded] = useState(false);
  return (
    <div className={`cell cell-mcp cell-mcp-${model.status}`} data-testid="mcp">
      <div className="cell-mcp-header">
        <span className="cell-mcp-title">🔧 {model.server}.{model.tool}</span>
        <span className="cell-mcp-status">{model.status}</span>
        <button aria-expanded={expanded} onClick={() => setExpanded((e) => !e)}>
          {expanded ? 'less' : 'more'}
        </button>
      </div>
      {model.progress.length > 0 && (
        <ul className="cell-mcp-progress">
          {model.progress.map((p, i) => <li key={i}>{p}</li>)}
        </ul>
      )}
      {expanded && (
        <div className="cell-mcp-detail">
          <pre className="cell-mcp-args">{JSON.stringify(model.arguments, null, 2)}</pre>
          {model.error && <pre className="cell-mcp-error">{model.error}</pre>}
          {model.result != null && <pre className="cell-mcp-result">{JSON.stringify(model.result, null, 2)}</pre>}
        </div>
      )}
    </div>
  );
}
```

Create `codex-rs/lemurclaw-gui/assets/src/components/cells/__tests__/McpToolCell.test.tsx`:
```tsx
import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { McpToolCell } from '../McpToolCell';

describe('McpToolCell', () => {
  it('shows server.tool + status, reveals args on expand', () => {
    render(<McpToolCell model={{
      kind: 'mcpToolCall', itemId: 'm1', server: 'fs', tool: 'read',
      status: 'completed', arguments: { path: '/x' }, progress: ['reading'],
      result: { content: 'hi' }, error: null,
    }} />);
    expect(screen.getByText('fs.read')).toBeInTheDocument();
    expect(screen.getByText('reading')).toBeInTheDocument();
    expect(screen.queryByText('"path"')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'more' }));
    expect(screen.getByText(/"path"/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 3: PlanCell.tsx + 测试**

Create `codex-rs/lemurclaw-gui/assets/src/components/cells/PlanCell.tsx`:
```tsx
import type { CellModel } from '../../viewModel/types';

interface Props {
  model: Extract<CellModel, { kind: 'plan' }>;
}

/** Plan cell. Renders the plan text verbatim (subproject 3 does not implement
 *  live plan editing — thread/goal/set is deferred). */
export function PlanCell({ model }: Props) {
  return (
    <div className="cell cell-plan" data-testid="plan">
      <div className="cell-plan-header">📋 plan</div>
      <pre className="cell-plan-text">{model.text || '(empty plan)'}</pre>
    </div>
  );
}
```

Create `codex-rs/lemurclaw-gui/assets/src/components/cells/__tests__/PlanCell.test.tsx`:
```tsx
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { PlanCell } from '../PlanCell';

describe('PlanCell', () => {
  it('renders plan text', () => {
    render(<PlanCell model={{ kind: 'plan', itemId: 'pl1', text: '1. do thing\n2. done' }} />);
    expect(screen.getByTestId('plan')).toHaveTextContent('1. do thing');
  });
});
```

- [ ] **Step 4: HookCell.tsx + 测试**

Create `codex-rs/lemurclaw-gui/assets/src/components/cells/HookCell.tsx`:
```tsx
import { useState } from 'react';
import type { CellModel } from '../../viewModel/types';

interface Props {
  model: Extract<CellModel, { kind: 'hook' }>;
}

/** Hook cell. Shows hook event + handler + status; entries collapsible.
 *
 *  NOTE: HookRunSummary fields marked `bigint` in ts-rs types are actually
 *  serialized by codex as JSON numbers (serde i64 → number). We render them
 *  through Number()/String() defensively in case an engine preserves bigint. */
export function HookCell({ model }: Props) {
  const [expanded, setExpanded] = useState(false);
  const r = model.run;
  return (
    <div className={`cell cell-hook cell-hook-${r.status}`} data-testid="hook">
      <div className="cell-hook-header">
        <span className="cell-hook-title">🪝 {r.eventName} · {r.handlerType}</span>
        <span className="cell-hook-status">{r.status}</span>
        <span className="cell-hook-scope">{r.scope}</span>
        <button aria-expanded={expanded} onClick={() => setExpanded((e) => !e)}>
          {expanded ? 'hide' : `${r.entries.length} entries`}
        </button>
      </div>
      {r.statusMessage && <div className="cell-hook-message">{r.statusMessage}</div>}
      {expanded && r.entries.length > 0 && (
        <ul className="cell-hook-entries">
          {r.entries.map((e, i) => <li key={i}><pre>{JSON.stringify(e)}</pre></li>)}
        </ul>
      )}
    </div>
  );
}
```

Create `codex-rs/lemurclaw-gui/assets/src/components/cells/__tests__/HookCell.test.tsx`:
```tsx
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { HookCell } from '../HookCell';

// HookRunSummary uses bigint in ts-rs types, but the wire shape is JSON
// number (codex serializes i64 → number). Test with a plain-number fixture
// cast through `never` to satisfy TS.
const run = {
  id: 'h1', eventName: 'PreToolUse', handlerType: 'command', executionMode: 'blocking',
  scope: 'session', sourcePath: { path: '/h/.codex/hook.sh' }, source: 'project',
  displayOrder: 0, status: 'completed', statusMessage: null,
  startedAt: 1, completedAt: 2, durationMs: 1, entries: [{ stream: 'stdout', line: 'ok' }],
} as never;

describe('HookCell', () => {
  it('shows event + status, entry count button', () => {
    render(<HookCell model={{ kind: 'hook', run }} />);
    expect(screen.getByText(/PreToolUse/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /entries/ })).toBeInTheDocument();
  });
});
```

- [ ] **Step 5: WebSearchCell.tsx + 测试**

Create `codex-rs/lemurclaw-gui/assets/src/components/cells/WebSearchCell.tsx`:
```tsx
import type { CellModel } from '../../viewModel/types';

interface Props {
  model: Extract<CellModel, { kind: 'webSearch' }>;
}

/** Web-search cell. Subproject 3 lightweight: shows query + status; full
 *  result rendering (citations, snippets) deferred. */
export function WebSearchCell({ model }: Props) {
  return (
    <div className="cell cell-websearch" data-testid="websearch">
      <span className="cell-websearch-icon">🌐</span>
      <span className="cell-websearch-query">{model.query || '(web search)'}</span>
      <span className="cell-websearch-status">{model.status}</span>
    </div>
  );
}
```

Create `codex-rs/lemurclaw-gui/assets/src/components/cells/__tests__/WebSearchCell.test.tsx`:
```tsx
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { WebSearchCell } from '../WebSearchCell';

describe('WebSearchCell', () => {
  it('renders query + status', () => {
    render(<WebSearchCell model={{ kind: 'webSearch', itemId: 'w1', query: 'rust async', status: 'completed' }} />);
    expect(screen.getByTestId('websearch')).toHaveTextContent('rust async');
    expect(screen.getByTestId('websearch')).toHaveTextContent('completed');
  });
});
```

- [ ] **Step 6: 跑测试 + 类型检查**

```bash
cd /Users/def/lemurclaw/codex-rs/lemurclaw-gui/assets
npm test -- cells/__tests__
npx tsc --noEmit
```
Expected: 全部新增 test 通过(5 个文件,各 1 个 test = 5 个)。

- [ ] **Step 7: Commit**

```bash
git add codex-rs/lemurclaw-gui/assets/src/components/cells/
git commit -m "feat(gui): fileChange/mcp/plan/hook/webSearch cell components + tests"
```

---

## Task 3.7:Scrollback 容器 + Composer

**目标:** 把 cells 拼成 Scrollback 滚动列表 + 实现 Composer 发 `turn/start`。

**Files:**
- Create: `codex-rs/lemurclaw-gui/assets/src/components/Scrollback.tsx`
- Create: `codex-rs/lemurclaw-gui/assets/src/components/Composer.tsx`
- Create: `codex-rs/lemurclaw-gui/assets/src/components/__tests__/Scrollback.test.tsx`
- Create: `codex-rs/lemurclaw-gui/assets/src/components/__tests__/Composer.test.tsx`

- [ ] **Step 1: Scrollback.tsx**

Create `codex-rs/lemurclaw-gui/assets/src/components/Scrollback.tsx`:
```tsx
import { useEffect, useRef } from 'react';
import type { ConversationState, CellModel } from '../viewModel/types';
import { UserMessageCell } from './cells/UserMessageCell';
import { AgentMessageCell } from './cells/AgentMessageCell';
import { ReasoningCell } from './cells/ReasoningCell';
import { CommandExecCell } from './cells/CommandExecCell';
import { FileChangeCell } from './cells/FileChangeCell';
import { McpToolCell } from './cells/McpToolCell';
import { PlanCell } from './cells/PlanCell';
import { HookCell } from './cells/HookCell';
import { WebSearchCell } from './cells/WebSearchCell';

interface Props {
  state: ConversationState;
}

/** Scrollback: the main conversation region. Renders every turn's items in
 *  order, auto-scrolls to the bottom when new content arrives (unless the
 *  user has scrolled up to read history — detected via scroll position). */
export function Scrollback({ state }: Props) {
  const bottomRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const stickToBottomRef = useRef(true);

  const onScroll = () => {
    const el = containerRef.current;
    if (!el) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
    stickToBottomRef.current = atBottom;
  };

  useEffect(() => {
    if (stickToBottomRef.current) {
      bottomRef.current?.scrollIntoView({ block: 'end' });
    }
  });

  if (state.turns.length === 0) {
    return (
      <div className="scrollback scrollback-empty" ref={containerRef}>
        <div className="scrollback-placeholder">send a message to start</div>
        <div ref={bottomRef} />
      </div>
    );
  }

  return (
    <div className="scrollback" ref={containerRef} onScroll={onScroll}>
      {state.turns.flatMap((turn) =>
        turn.items.map((cell) => <CellRenderer key={cellKey(cell)} cell={cell} />),
      )}
      <div ref={bottomRef} />
    </div>
  );
}

function CellRenderer({ cell }: { cell: CellModel }) {
  switch (cell.kind) {
    case 'userMessage': return <UserMessageCell model={cell} />;
    case 'agentMessage': return <AgentMessageCell model={cell} />;
    case 'reasoning': return <ReasoningCell model={cell} />;
    case 'commandExecution': return <CommandExecCell model={cell} />;
    case 'fileChange': return <FileChangeCell model={cell} />;
    case 'mcpToolCall': return <McpToolCell model={cell} />;
    case 'plan': return <PlanCell model={cell} />;
    case 'hook': return <HookCell model={cell} />;
    case 'webSearch': return <WebSearchCell model={cell} />;
    case 'dynamicToolCall':
    case 'imageGeneration':
    case 'sleep':
    case 'generic':
      // Subproject 3 renders these as a minimal placeholder; full coverage in
      // later subprojects.
      return (
        <div className="cell cell-generic" data-testid="generic-cell">
          <pre>{cell.kind === 'generic' ? cell.rawType : cell.kind}</pre>
        </div>
      );
  }
}

function cellKey(cell: CellModel): string {
  if (cell.kind === 'hook') return `hook:${cell.run.id}`;
  return `${cell.kind}:${cell.itemId}`;
}
```

- [ ] **Step 2: Scrollback 单测**

Create `codex-rs/lemurclaw-gui/assets/src/components/__tests__/Scrollback.test.tsx`:
```tsx
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { Scrollback } from '../Scrollback';
import { initialState } from '../../viewModel/types';
import type { ConversationState } from '../../viewModel/types';

describe('Scrollback', () => {
  it('shows placeholder when empty', () => {
    render(<Scrollback state={initialState} />);
    expect(screen.getByText('send a message to start')).toBeInTheDocument();
  });

  it('renders a user message cell for a userMessage item', () => {
    const state: ConversationState = {
      ...initialState,
      turns: [{
        id: 'tu1', status: 'inProgress', startedAt: 1, completedAt: null,
        items: [{ kind: 'userMessage', itemId: 'u1', text: 'hi' }],
      }],
    };
    render(<Scrollback state={state} />);
    expect(screen.getByTestId('user-message')).toHaveTextContent('hi');
  });

  it('renders mixed cells in order', () => {
    const state: ConversationState = {
      ...initialState,
      turns: [{
        id: 'tu1', status: 'inProgress', startedAt: 1, completedAt: null,
        items: [
          { kind: 'userMessage', itemId: 'u1', text: 'hi' },
          { kind: 'agentMessage', itemId: 'a1', text: 'hello', phase: null },
        ],
      }],
    };
    render(<Scrollback state={state} />);
    expect(screen.getByTestId('user-message')).toBeInTheDocument();
    expect(screen.getByTestId('agent-message')).toBeInTheDocument();
  });
});
```

- [ ] **Step 3: Composer.tsx**

Create `codex-rs/lemurclaw-gui/assets/src/components/Composer.tsx`:
```tsx
import { useState, useRef, useCallback } from 'react';
import { send } from '../transport';

interface Props {
  /** Active thread id, or null before thread/started. When null, the composer
   *  is disabled. */
  threadId: string | null;
  /** True while a turn is in progress; disables send + shows interrupt button. */
  turnActive: boolean;
  /** Send `turn/interrupt` for the active turn. No-op if turnActive is false. */
  onInterrupt: () => void;
}

/** Composer: textarea + send button. Enter sends (shift+enter newline).
 *  Sends a `turn/start` ClientRequest with a single text UserInput.
 *  Subproject 3 limitation: no slash popup, no @-mention popup, no file
 *  upload — those land in subproject 4. */
export function Composer({ threadId, turnActive, onInterrupt }: Props) {
  const [text, setText] = useState('');
  const seqRef = useRef(1);

  const submit = useCallback(() => {
    const trimmed = text.trim();
    if (!trimmed || !threadId) return;
    const clientUserMessageId = `c${seqRef.current++}`;
    send({
      method: 'turn/start',
      id: seqRef.current++,
      params: {
        threadId,
        clientUserMessageId,
        input: [{ type: 'text', text: trimmed, text_elements: [] }],
      },
    });
    setText('');
  }, [text, threadId]);

  const onKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      submit();
    }
  };

  const disabled = !threadId || turnActive;

  return (
    <div className="composer" data-testid="composer">
      <textarea
        className="composer-input"
        data-testid="composer-input"
        placeholder={threadId ? 'type a message…  (Enter to send, Shift+Enter for newline)' : 'starting up…'}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={onKeyDown}
        disabled={!threadId}
        rows={3}
      />
      <div className="composer-actions">
        {turnActive ? (
          <button className="composer-interrupt" onClick={onInterrupt} data-testid="composer-interrupt">
            interrupt
          </button>
        ) : (
          <button className="composer-send" onClick={submit} disabled={disabled} data-testid="composer-send">
            send
          </button>
        )}
      </div>
    </div>
  );
}
```

> **注:** `seqRef` 同时用于 ClientRequest `id`(单调整数,符合 `RequestId = string | number`)和 `clientUserMessageId`(字符串前缀)。`turn/start` 的 `params` 形态以 `types/v2/TurnStartParams.ts` 为准(已核实:threadId + input 必填,input 是 `UserInput[]`,UserInput.text 变体要求 `text_elements` 字段)。

- [ ] **Step 4: Composer 单测(用 mock transport)**

Create `codex-rs/lemurclaw-gui/assets/src/components/__tests__/Composer.test.tsx`:
```tsx
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Composer } from '../Composer';

// Mock transport.send so tests can assert outbound ClientRequest shape.
vi.mock('../../transport', () => ({
  send: vi.fn(),
}));

import { send } from '../../transport';

describe('Composer', () => {
  beforeEach(() => vi.mocked(send).mockClear());

  it('disables send when threadId is null', () => {
    render(<Composer threadId={null} turnActive={false} onInterrupt={() => {}} />);
    expect(screen.getByTestId('composer-send')).toBeDisabled();
    expect(screen.getByTestId('composer-input')).toBeDisabled();
  });

  it('Enter sends a turn/start with the typed text', () => {
    render(<Composer threadId="t1" turnActive={false} onInterrupt={() => {}} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    fireEvent.change(ta, { target: { value: 'hello' } });
    fireEvent.keyDown(ta, { key: 'Enter' });
    expect(send).toHaveBeenCalledTimes(1);
    const req = vi.mocked(send).mock.calls[0][0] as { method: string; params: { threadId: string; input: Array<{ type: string; text: string }> } };
    expect(req.method).toBe('turn/start');
    expect(req.params.threadId).toBe('t1');
    expect(req.params.input[0].text).toBe('hello');
  });

  it('Shift+Enter does NOT send', () => {
    render(<Composer threadId="t1" turnActive={false} onInterrupt={() => {}} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    fireEvent.change(ta, { target: { value: 'hello' } });
    fireEvent.keyDown(ta, { key: 'Enter', shiftKey: true });
    expect(send).not.toHaveBeenCalled();
  });

  it('shows interrupt button while turnActive and calls onInterrupt', () => {
    const onInterrupt = vi.fn();
    render(<Composer threadId="t1" turnActive={true} onInterrupt={onInterrupt} />);
    expect(screen.queryByTestId('composer-send')).toBeNull();
    fireEvent.click(screen.getByTestId('composer-interrupt'));
    expect(onInterrupt).toHaveBeenCalled();
  });
});
```

- [ ] **Step 5: 跑测试 + 类型检查**

```bash
cd /Users/def/lemurclaw/codex-rs/lemurclaw-gui/assets
npm test -- Scrollback Composer
npx tsc --noEmit
```
Expected: 7 个 test 通过(Scrollback 3 + Composer 4)。

- [ ] **Step 6: Commit**

```bash
git add codex-rs/lemurclaw-gui/assets/src/components/Scrollback.tsx \
        codex-rs/lemurclaw-gui/assets/src/components/Composer.tsx \
        codex-rs/lemurclaw-gui/assets/src/components/__tests__/
git commit -m "feat(gui): Scrollback container + Composer (turn/start)"
```

---

## Task 3.8:ApprovalCard + transport.resolveServerRequest

**目标:** 用户对 ServerRequest 做决策,经 transport.resolveServerRequest → ipc envelope → backend → codex resolve。

**Files:**
- Modify: `codex-rs/lemurclaw-gui/assets/src/transport.ts`(加 `resolveServerRequest` / `rejectServerRequest`)
- Create: `codex-rs/lemurclaw-gui/assets/src/components/ApprovalCard.tsx`
- Create: `codex-rs/lemurclaw-gui/assets/src/components/__tests__/ApprovalCard.test.tsx`

- [ ] **Step 1: 扩展 transport.ts**

Modify `codex-rs/lemurclaw-gui/assets/src/transport.ts`,在文件末尾(已有 `send` / `onEvent` / `hasBridge` 之后)追加:

```ts
/**
 * Resolve a pending ServerRequest by id. Used by ApprovalCard's [accept] /
 * [approve] buttons. Sends the `__resolve` envelope consumed by
 * `backend.rs::handle_ipc`.
 *
 * `result` is the JSON-RPC result payload — its shape depends on the
 * ServerRequest method (see the `*ApprovalResponse` types in
 * `assets/src/types/v2/`). For exec/file approvals, a `{ decision: "accept" }`
 * object; for mcp elicitation, a `{ value: ... }` object.
 */
export function resolveServerRequest(requestId: string | number, result: unknown): void {
  send({ __resolve: requestId, result });
}

/**
 * Reject a pending ServerRequest by id. Used by ApprovalCard's [decline] /
 * [cancel] buttons. `code` defaults to -32000 (JSON-RPC server error);
 * `message` is required.
 */
export function rejectServerRequest(
  requestId: string | number,
  message: string,
  code: number = -32000,
): void {
  send({ __reject: requestId, error: { code, message } });
}
```

> **注:** 不改 `send` 既有签名(`unknown` 入参)—— envelope 也是 unknown,通过 ipc_handler 在 backend 区分。TS 层不强类型 envelope 是 intentional(backend 是 envelope 的 single source of truth)。

- [ ] **Step 2: ApprovalCard.tsx**

Create `codex-rs/lemurclaw-gui/assets/src/components/ApprovalCard.tsx`:
```tsx
import type { PendingApproval } from '../viewModel/types';
import { resolveServerRequest, rejectServerRequest } from '../transport';

interface Props {
  approval: PendingApproval;
}

/** ApprovalCard: renders a pending ServerRequest as a card with decision
 *  buttons. Dispatch shape depends on `approval.kind`:
 *  - commandExecution: [run once] [always this session] [decline]
 *  - fileChange:       [apply once] [always this session] [decline]
 *  - mcpElicitation:   text input + [submit] [cancel]
 *  - permissions:      [allow] [deny]
 *  - toolUserInput:    text input + [submit] [cancel]
 *  - generic:          [resolve] [cancel]
 *
 *  Decision → transport.resolveServerRequest / rejectServerRequest. */
export function ApprovalCard({ approval }: Props) {
  switch (approval.kind) {
    case 'commandExecution':
      return <ExecApproval approval={approval} />;
    case 'fileChange':
      return <FileChangeApproval approval={approval} />;
    case 'mcpElicitation':
    case 'toolUserInput':
      return <ElicitationApproval approval={approval} />;
    case 'permissions':
      return <PermissionsApproval approval={approval} />;
    case 'generic':
    default:
      return <GenericApproval approval={approval} />;
  }
}

function ExecApproval({ approval }: { approval: PendingApproval }) {
  const params = approval.raw.params as {
    command?: string | null;
    cwd?: { path?: string } | string | null;
    commandActions?: Array<{ type: string; name?: string }> | null;
    reason?: string | null;
  };
  const cwdStr = typeof params.cwd === 'string' ? params.cwd : params.cwd?.path ?? '';
  const actions = (params.commandActions ?? []).map((a) => a.name ?? a.type).join(' | ');
  return (
    <div className="approval approval-exec" data-testid="approval-exec">
      <div className="approval-title">🛡 command approval</div>
      <div className="approval-detail">
        <code className="approval-command">$ {params.command ?? '(no command)'}</code>
        {cwdStr && <div className="approval-cwd">{cwdStr}</div>}
        {actions && <div className="approval-actions-summary">{actions}</div>}
        {params.reason && <div className="approval-reason">{params.reason}</div>}
      </div>
      <div className="approval-buttons">
        <button onClick={() => resolveServerRequest(approval.requestId, { decision: 'accept' })}>run once</button>
        <button onClick={() => resolveServerRequest(approval.requestId, { decision: 'acceptForSession' })}>always this session</button>
        <button onClick={() => rejectServerRequest(approval.requestId, 'user declined')}>decline</button>
      </div>
    </div>
  );
}

function FileChangeApproval({ approval }: { approval: PendingApproval }) {
  const params = approval.raw.params as {
    changes?: Array<{ path: string; kind: { type: string }; diff?: string }> | null;
  };
  const changes = params.changes ?? [];
  return (
    <div className="approval approval-patch" data-testid="approval-patch">
      <div className="approval-title">📝 file change approval</div>
      <ul className="approval-files">
        {changes.map((c, i) => (
          <li key={i} className={`approval-file approval-file-${c.kind.type}`}>
            <code>{c.path}</code>
          </li>
        ))}
      </ul>
      <div className="approval-buttons">
        <button onClick={() => resolveServerRequest(approval.requestId, { decision: 'accept' })}>apply once</button>
        <button onClick={() => resolveServerRequest(approval.requestId, { decision: 'acceptForSession' })}>always this session</button>
        <button onClick={() => rejectServerRequest(approval.requestId, 'user declined')}>decline</button>
      </div>
    </div>
  );
}

function ElicitationApproval({ approval }: { approval: PendingApproval }) {
  // MCP elicitation + tool/user-input both want a free-form value. Subproject
  // 3 keeps it as a plain text field; structured JSON-Schema elicitation UI
  // (multi-field forms) is deferred.
  const submit = (value: string) =>
    resolveServerRequest(approval.requestId, { value });
  const cancel = () =>
    rejectServerRequest(approval.requestId, 'user cancelled');
  return (
    <InlineInputApproval
      testId="approval-elicitation"
      title={approval.kind === 'mcpElicitation' ? '🔌 mcp elicitation' : '❓ tool input requested'}
      submitLabel="submit"
      onSubmit={submit}
      onCancel={cancel}
    />
  );
}

function PermissionsApproval({ approval }: { approval: PendingApproval }) {
  return (
    <div className="approval approval-permissions" data-testid="approval-permissions">
      <div className="approval-title">🔒 permission request</div>
      <pre className="approval-raw">{JSON.stringify(approval.raw.params, null, 2)}</pre>
      <div className="approval-buttons">
        <button onClick={() => resolveServerRequest(approval.requestId, { decision: 'accept' })}>allow</button>
        <button onClick={() => rejectServerRequest(approval.requestId, 'user denied')}>deny</button>
      </div>
    </div>
  );
}

function GenericApproval({ approval }: { approval: PendingApproval }) {
  return (
    <div className="approval approval-generic" data-testid="approval-generic">
      <div className="approval-title">server request: {approval.raw.method}</div>
      <pre className="approval-raw">{JSON.stringify(approval.raw.params, null, 2)}</pre>
      <div className="approval-buttons">
        <button onClick={() => resolveServerRequest(approval.requestId, {})}>resolve</button>
        <button onClick={() => rejectServerRequest(approval.requestId, 'cancelled')}>cancel</button>
      </div>
    </div>
  );
}

function InlineInputApproval(props: {
  testId: string;
  title: string;
  submitLabel: string;
  onSubmit: (value: string) => void;
  onCancel: () => void;
}) {
  const submit = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const data = new FormData(e.currentTarget);
    props.onSubmit(String(data.get('value') ?? ''));
  };
  return (
    <form className="approval approval-elicitation" data-testid={props.testId} onSubmit={submit}>
      <div className="approval-title">{props.title}</div>
      <input name="value" className="approval-input" />
      <div className="approval-buttons">
        <button type="submit">{props.submitLabel}</button>
        <button type="button" onClick={props.onCancel}>cancel</button>
      </div>
    </form>
  );
}
```

> **注:** ApprovalCard 的 `params` cast 成精简形状(只取渲染需要的字段);完整字段在 `approval.raw.params` 里,需要时(`GenericApproval`)dump 出来。decision value 的字面值(`'accept'` / `'acceptForSession'`)严格匹配 `CommandExecutionApprovalDecision` / `FileChangeApprovalDecision` union(已核实 Task 3.1 前置勘查)。

- [ ] **Step 3: ApprovalCard 单测**

Create `codex-rs/lemurclaw-gui/assets/src/components/__tests__/ApprovalCard.test.tsx`:
```tsx
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ApprovalCard } from '../ApprovalCard';

vi.mock('../../transport', () => ({
  resolveServerRequest: vi.fn(),
  rejectServerRequest: vi.fn(),
}));

import { resolveServerRequest, rejectServerRequest } from '../../transport';
import type { PendingApproval } from '../../viewModel/types';

describe('ApprovalCard', () => {
  beforeEach(() => {
    vi.mocked(resolveServerRequest).mockClear();
    vi.mocked(rejectServerRequest).mockClear();
  });

  it('exec approval sends accept decision on [run once]', () => {
    const approval: PendingApproval = {
      requestId: 42,
      kind: 'commandExecution',
      raw: {
        method: 'item/commandExecution/requestApproval', id: 42,
        params: { threadId: 't1', turnId: 'tu1', itemId: 'i1', startedAtMs: 1, environmentId: null, command: 'ls', cwd: { path: '/x' }, commandActions: null },
      } as never,
    };
    render(<ApprovalCard approval={approval} />);
    fireEvent.click(screen.getByText('run once'));
    expect(resolveServerRequest).toHaveBeenCalledWith(42, { decision: 'accept' });
  });

  it('exec approval decline sends reject', () => {
    const approval: PendingApproval = {
      requestId: 99,
      kind: 'commandExecution',
      raw: { method: 'item/commandExecution/requestApproval', id: 99, params: { threadId: 't', turnId: 'tu', itemId: 'i', startedAtMs: 1, environmentId: null, command: 'rm', cwd: { path: '/' }, commandActions: null } } as never,
    };
    render(<ApprovalCard approval={approval} />);
    fireEvent.click(screen.getByText('decline'));
    expect(rejectServerRequest).toHaveBeenCalledWith(99, 'user declined');
  });

  it('patch approval renders file list and sends acceptForSession', () => {
    const approval: PendingApproval = {
      requestId: 7,
      kind: 'fileChange',
      raw: { method: 'item/fileChange/requestApproval', id: 7, params: { threadId: 't', turnId: 'tu', itemId: 'i', startedAtMs: 1, changes: [{ path: 'a.rs', kind: { type: 'add' }, diff: '+x' }] } } as never,
    };
    render(<ApprovalCard approval={approval} />);
    expect(screen.getByText('a.rs')).toBeInTheDocument();
    fireEvent.click(screen.getByText('always this session'));
    expect(resolveServerRequest).toHaveBeenCalledWith(7, { decision: 'acceptForSession' });
  });

  it('elicitation submits the typed value', () => {
    const approval: PendingApproval = {
      requestId: 'abc',
      kind: 'mcpElicitation',
      raw: { method: 'mcpServer/elicitation/request', id: 'abc', params: { threadId: 't', turnId: 'tu', itemId: 'i', requestedSchema: {} } } as never,
    };
    render(<ApprovalCard approval={approval} />);
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'my response' } });
    fireEvent.click(screen.getByText('submit'));
    expect(resolveServerRequest).toHaveBeenCalledWith('abc', { value: 'my response' });
  });
});
```

- [ ] **Step 4: 跑测试 + 类型检查**

```bash
cd /Users/def/lemurclaw/codex-rs/lemurclaw-gui/assets
npm test -- ApprovalCard
npx tsc --noEmit
```
Expected: 4 个 test 通过。

- [ ] **Step 5: Commit**

```bash
git add codex-rs/lemurclaw-gui/assets/src/transport.ts \
        codex-rs/lemurclaw-gui/assets/src/components/ApprovalCard.tsx \
        codex-rs/lemurclaw-gui/assets/src/components/__tests__/ApprovalCard.test.tsx
git commit -m "feat(gui): ApprovalCard (exec/patch/mcp-elicitation/permissions) + transport.resolve"
```

---

## Task 3.9:顶层 App 装配 + 样式 + 端到端验证

**目标:** 把所有组件装配到顶层 App,简化 main.tsx,加基础样式,跑一次真实 GUI 验证完整对话。

**Files:**
- Create: `codex-rs/lemurclaw-gui/assets/src/app/useConversation.ts`
- Create: `codex-rs/lemurclaw-gui/assets/src/app/App.tsx`
- Create: `codex-rs/lemurclaw-gui/assets/src/styles.css`
- Modify: `codex-rs/lemurclaw-gui/assets/src/main.tsx`(简化)

- [ ] **Step 1: useConversation hook**

Create `codex-rs/lemurclaw-gui/assets/src/app/useConversation.ts`:
```ts
import { useEffect, useReducer, useRef, useCallback } from 'react';
import { onEvent, send } from '../transport';
import { reducer } from '../viewModel/reducer';
import { initialState } from '../viewModel/types';

/** Wires the transport's onEvent stream into the ViewModel reducer.
 *  Returns the live ConversationState + the captured thread id (for the
 *  composer) + an `interrupt` callback for the active turn.
 *
 *  The thread id is captured opportunistically from any event that carries
 *  `params.threadId` (most ServerNotifications do) and held in a ref so the
 *  composer doesn't re-render on every event. */
export function useConversation() {
  const [state, dispatch] = useReducer(reducer, initialState);
  const threadIdRef = useRef<string | null>(null);

  useEffect(() => {
    onEvent((ev) => {
      const maybeThreadId = (ev as { params?: { threadId?: string } })?.params?.threadId;
      if (maybeThreadId && !threadIdRef.current) {
        threadIdRef.current = maybeThreadId;
      }
      dispatch(ev);
    });
  }, []);

  const interrupt = useCallback(() => {
    const threadId = threadIdRef.current;
    const turnId = state.activeTurnId;
    if (!threadId || !turnId) return;
    send({
      method: 'turn/interrupt',
      id: Date.now(),
      params: { threadId, turnId },
    });
  }, [state.activeTurnId]);

  return { state, threadId: threadIdRef.current, interrupt };
}
```

- [ ] **Step 2: App.tsx**

Create `codex-rs/lemurclaw-gui/assets/src/app/App.tsx`:
```tsx
import { useConversation } from './useConversation';
import { Scrollback } from '../components/Scrollback';
import { Composer } from '../components/Composer';
import { ApprovalCard } from '../components/ApprovalCard';

/** Top-level GUI application. Wires the transport stream into the ViewModel
 *  reducer via `useConversation`, then lays out the main conversation region
 *  (Scrollback), the approval queue (overlay above composer), and the input
 *  (Composer).
 *
 *  Layout follows spec §4.3: vertical main column. The right rail (sessions /
 *  agent / plan sidebar) is reserved for subproject 4. */
export function App() {
  const { state, threadId, interrupt } = useConversation();
  const turnActive = state.activeTurnId !== null;

  return (
    <div className="app-root">
      <main className="app-main">
        <div className="app-scrollback">
          <Scrollback state={state} />
        </div>
        {state.pendingApprovals.length > 0 && (
          <div className="approvals-queue" data-testid="approvals-queue">
            {state.pendingApprovals.map((a) => (
              <ApprovalCard key={String(a.requestId)} approval={a} />
            ))}
          </div>
        )}
        <Composer threadId={threadId} turnActive={turnActive} onInterrupt={interrupt} />
      </main>
    </div>
  );
}
```

- [ ] **Step 3: styles.css**

Create `codex-rs/lemurclaw-gui/assets/src/styles.css`:
```css
/* lemurclaw GUI base styles. Subproject 3 keeps it intentionally plain —
   visual polish is out of scope. Layout follows spec §4.3: vertical main
   column (scrollback + approvals + composer). */

* { box-sizing: border-box; }
html, body, #root { height: 100%; margin: 0; }
body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; color: #1a1a1a; background: #fafafa; font-size: 14px; }

.app-root { display: flex; height: 100vh; }
.app-main { flex: 1; display: flex; flex-direction: column; min-width: 0; }
.app-scrollback { flex: 1; overflow: hidden; display: flex; }
.scrollback { flex: 1; overflow-y: auto; padding: 12px; }
.scrollback-empty { display: flex; align-items: center; justify-content: center; }
.scrollback-placeholder { color: #888; }

.cell { margin-bottom: 10px; padding: 8px 10px; border-radius: 6px; background: #fff; border: 1px solid #eee; }
.cell-role { font-size: 11px; color: #888; margin-bottom: 4px; text-transform: uppercase; letter-spacing: 0.5px; }
.cell-text { margin: 0; font-family: 'SF Mono', Menlo, monospace; font-size: 13px; white-space: pre-wrap; word-wrap: break-word; }
.cell-user { border-left: 3px solid #4a90e2; }
.cell-agent { border-left: 3px solid #2eb872; }
.cell-agent-final { background: #f0fff4; }
.cell-reasoning { border-left: 3px solid #b8a02e; background: #fffdf3; }
.cell-reasoning-toggle { background: none; border: none; cursor: pointer; color: #b8a02e; font-size: 12px; padding: 0; }
.cell-reasoning-summary { opacity: 0.9; font-size: 12px; }
.cell-reasoning-content { opacity: 0.7; font-size: 12px; border-top: 1px dashed #ddd; margin-top: 4px; padding-top: 4px; }

.cell-exec { border-left: 3px solid #6c757d; }
.cell-exec-header { display: flex; align-items: center; gap: 8px; }
.cell-exec-command { font-family: 'SF Mono', Menlo, monospace; font-size: 13px; flex: 1; }
.cell-exec-status { font-size: 11px; color: #666; }
.cell-exec-cwd { font-size: 11px; color: #999; margin-top: 2px; }
.cell-exec-output { margin-top: 6px; padding: 6px; background: #f4f4f4; font-size: 12px; max-height: 300px; overflow: auto; }
.cell-exec-completed .cell-exec-status { color: #2eb872; }
.cell-exec-failed .cell-exec-status { color: #d9534f; }

.cell-patch { border-left: 3px solid #9b59b6; }
.cell-patch-header { display: flex; align-items: center; gap: 8px; margin-bottom: 4px; }
.cell-patch-files { list-style: none; padding: 0; margin: 0; }
.cell-patch-file { padding: 2px 0; }
.cell-patch-file-toggle { background: none; border: none; cursor: pointer; padding: 0; font-size: 13px; }
.cell-patch-kind { display: inline-block; width: 16px; font-weight: bold; }
.cell-patch-file-add .cell-patch-kind { color: #2eb872; }
.cell-patch-file-delete .cell-patch-kind { color: #d9534f; }
.cell-patch-file-update .cell-patch-kind { color: #b8a02e; }
.cell-patch-diff { margin: 4px 0; padding: 6px; background: #f4f4f4; font-size: 12px; max-height: 300px; overflow: auto; }

.cell-mcp { border-left: 3px solid #e67e22; }
.cell-mcp-header { display: flex; align-items: center; gap: 8px; }
.cell-mcp-progress { margin: 4px 0 0 0; padding-left: 16px; font-size: 12px; color: #888; }

.cell-plan { border-left: 3px solid #3498db; background: #f0f8ff; }
.cell-plan-text { font-size: 13px; white-space: pre-wrap; }

.cell-hook { border-left: 3px solid #95a5a6; background: #f8f8f8; font-size: 12px; }
.cell-websearch { padding: 4px 10px; color: #888; font-size: 12px; }

.composer { border-top: 1px solid #ddd; padding: 8px; background: #fff; display: flex; flex-direction: column; gap: 6px; }
.composer-input { width: 100%; resize: vertical; min-height: 48px; padding: 6px; font-family: inherit; font-size: 14px; border: 1px solid #ccc; border-radius: 4px; }
.composer-actions { display: flex; justify-content: flex-end; gap: 6px; }
.composer-send, .composer-interrupt { padding: 6px 16px; border: none; border-radius: 4px; cursor: pointer; font-size: 13px; }
.composer-send { background: #4a90e2; color: #fff; }
.composer-send:disabled { background: #aaa; cursor: not-allowed; }
.composer-interrupt { background: #d9534f; color: #fff; }

.approvals-queue { padding: 8px; background: #fff8e1; border-top: 1px solid #ffe082; max-height: 40vh; overflow-y: auto; }
.approval { background: #fff; border: 1px solid #ffe082; border-radius: 6px; padding: 8px 10px; margin-bottom: 6px; }
.approval-title { font-weight: bold; margin-bottom: 4px; font-size: 13px; }
.approval-detail { margin-bottom: 6px; font-size: 13px; }
.approval-command { font-family: 'SF Mono', Menlo, monospace; background: #f4f4f4; padding: 2px 4px; border-radius: 3px; }
.approval-cwd { font-size: 11px; color: #888; margin-top: 2px; }
.approval-files { list-style: none; padding: 0; margin: 0 0 6px 0; }
.approval-buttons { display: flex; gap: 6px; flex-wrap: wrap; }
.approval-buttons button { padding: 4px 12px; border: 1px solid #ccc; border-radius: 4px; background: #fff; cursor: pointer; font-size: 12px; }
.approval-buttons button:first-child { background: #2eb872; color: #fff; border-color: #2eb872; }
.approval-buttons button:nth-child(2) { background: #4a90e2; color: #fff; border-color: #4a90e2; }
.approval-input { width: 100%; padding: 4px; margin-bottom: 6px; border: 1px solid #ccc; border-radius: 4px; }
.approval-raw { font-size: 11px; max-height: 150px; overflow: auto; background: #f4f4f4; padding: 4px; }
```

- [ ] **Step 4: 简化 main.tsx**

Modify `codex-rs/lemurclaw-gui/assets/src/main.tsx`,替换全部内容为:
```tsx
import { createRoot } from 'react-dom/client';
import './styles.css';
import { App } from './app/App';

const rootEl = document.getElementById('root');
if (!rootEl) {
  throw new Error('lemurclaw-gui: #root element not found in index.html');
}
createRoot(rootEl).render(<App />);
```

- [ ] **Step 5: index.html 无需改动**

`import './styles.css'` 在 main.tsx 里,Vite 构建时会把它打进 bundle。index.html 仍指向 `/src/main.tsx`(子项目 2 已设)。

- [ ] **Step 6: 跑全部前端测试 + 类型检查 + build**

```bash
cd /Users/def/lemurclaw/codex-rs/lemurclaw-gui/assets
npm test                    # 全部 vitest
npx tsc --noEmit            # 类型检查
npm run build               # 生产构建
ls dist/                    # 确认 index.html + assets/*.js 生成
```
Expected: 全部 vitest test 通过(约 25+ 个),tsc 0 errors,build 产出 dist/index.html + dist/assets/index-*.js。

- [ ] **Step 7: 验证 Rust 侧编译 + clippy + fmt**

```bash
cd /Users/def/lemurclaw/codex-rs
cargo check -p lemurclaw-gui
cargo clippy -p lemurclaw-gui
cargo fmt -p lemurclaw-gui
```
Expected: 通过。build.rs 检测到 dist/ 存在不会重新跑 npm。

- [ ] **Step 8: 端到端手动验证(用户执行)**

> **沙箱限制:** 本环境无 display,无法实际打开 GUI 窗口。手动验证留给用户。

```bash
cd /Users/def/lemurclaw/codex-rs
cargo run -p lemurclaw -- --frontend gui
```
Expected(用户验证):
1. 窗口打开,显示空白 scrollback("send a message to start") + composer
2. 在 composer 输入消息 → Enter → 发送 `turn/start`
3. 看到 `turn/started` → agent 开始回复 → `agentMessage/delta` 流式渲染 → `item/completed` 定稿
4. 若 agent 调命令 → `item/commandExecution/requestApproval` → ApprovalCard 出现 → 点 [run once] → `serverRequest/resolved` → 命令运行 → `commandExecution/outputDelta` → exit
5. 若 agent 改文件 → patch cell 渲染 diff
6. 若 plan mode → plan cell 渲染
7. 整轮 `turn/completed` → composer 重新启用

- [ ] **Step 9: Commit + 子项目 3 完成记录**

```bash
git add codex-rs/lemurclaw-gui/assets/src/app/ \
        codex-rs/lemurclaw-gui/assets/src/main.tsx \
        codex-rs/lemurclaw-gui/assets/src/styles.css
git commit -m "feat(gui): assemble App + styles + main wiring (subproject 3 complete)"
```

参照子项目 2 的 `docs(plan): record Task 2.5 completion + ...` 模式,在后续 docs commit 记录子项目 3 完成状态(包括 Step 8 手动验证的真实结果或已知 caveat)。

---

## 子项目 3 完成标准

- [ ] `npm test` 全绿(约 25+ vitest)
- [ ] `npx tsc --noEmit` 0 errors
- [ ] `cargo check -p lemurclaw-gui` + `cargo clippy -p lemurclaw-gui` 通过
- [ ] `npm run build` 产出 dist/
- [ ] **功能等价 TUI 主体(spec §6.1):**
  - [ ] Scrollback 渲染 9 类 cell(user/agent/reasoning/exec/patch/mcp/plan/hook/webSearch)
  - [ ] Composer 发 `turn/start`,Enter 发送 / Shift+Enter 换行 / turn 进行中显示 interrupt
  - [ ] ApprovalCard 处理 commandExecution/fileChange/mcpElicitation/permissions 决策,回传 codex
  - [ ] Rust 后端 resolve/reject 通道通(纯加法扩展 InProcessAppServerRequestHandle)
  - [ ] ViewModel reducer 增量折叠 ServerNotification 流(streaming + complete + approval lifecycle)
- [ ] **surface 覆盖矩阵(spec §4.2 对话/输入/审批三类):**
  - [ ] history_cell: user/assistant/reasoning/exec/patch/mcp/plan/websearch/hook(8/8 对话类)
  - [ ] composer textarea(输入类基础;slash/mention/file popup 留给子项目 4)
  - [ ] ApprovalOverlay(exec/file/mcp/perm)(审批类基础)

---

## 后续(子项目 4+)

- SessionPicker / ModelPicker / AgentPicker(导航类 surface)
- Onboarding / TranscriptPager / ThemePicker
- SlashPopup / MentionPopup / FileSearchPopup 的 fuzzy 交互(composer 增强)
- SettingsModal 系列(配置管理)
- webui 模式(子项目 6)
- 双前端录放测试套件(子项目 7)

---

## 实现备注(给执行者的注意事项)

1. **bigint 字段:** HookRunSummary 含 `bigint` 字段(displayOrder/startedAt 等)。codex 后端用 serde_json 序列化 i64 → JSON number(不是字符串),所以 JS 运行时拿到的是 number。但 ts-rs 生成的 TS 类型标 `bigint`,会导致 React render 报错或 TS 类型矛盾。Task 3.6 测试 fixture 用 `as never` cast 绕过 TS;运行时无影响(JSON.parse 出来是 number)。若运行时 react 报 bigint 渲染错,在 HookCell 里显式 `Number(r.durationMs)` 转换显示字段。
2. **LegacyAppPathString:** codex 的路径包装。可能是 branded string 或 `{ path }` 形态。Task 3.4 reducer 实现 cwd 字段时代码已双路径兜底(`typeof === 'string' ? : .path ?? String()`);若仍 TS 报错,改 `String(item.cwd as unknown as string)` 强转。
3. **ServerRequestResolvedNotification.requestId:** 字段名假设为 `requestId`(string|number)。Task 3.4 实现前 `cat codex-rs/lemurclaw-gui/assets/src/types/v2/ServerRequestResolvedNotification.ts` 核实。
4. **resolve payload shape:** ApprovalCard 发的 `result`(如 `{ decision: 'accept' }`)必须匹配对应 `*ApprovalResponse.ts` 类型。Task 3.8 实现前 `cat codex-rs/lemurclaw-gui/assets/src/types/v2/CommandExecutionRequestApprovalResponse.ts` + `FileChangeRequestApprovalResponse.ts` 核实 `decision` 字段值(已勘查:CommandExecutionApprovalDecision 有 `accept` / `acceptForSession` / `decline` / `cancel` + 两个对象变体;FileChangeApprovalDecision 有 `accept` / `acceptForSession` / `decline` / `cancel`)。
5. **codex fork 改动(Task 3.1):** 给 InProcessAppServerRequestHandle 加方法是 codex fork commit,逆 upstream 风险极低(纯加法,不动既有逻辑)。但 merge upstream 时若 openai 改了 ClientCommand 或 InProcessAppServerRequestHandle 结构,可能冲突,手动解决。
6. **build.rs 不重跑 npm test:** build.rs 只在 dist/ 不存在时跑 npm install + build,不跑 npm test。测试是开发者本地 `npm test` 跑,不阻塞 cargo build。若 CI 要跑前端测试,在 CI script 里加 `cd lemurclaw-gui/assets && npm test`(超出本计划范围)。
7. **范围已用户确认:** 子项目 3 全做 7 类 cell + hook/webSearch 轻量(共 9 类 cell variant 在 CellModel 里),ApprovalCard 支持 exec/patch/mcp/perm/tool-input/generic 6 类决策。完整覆盖 spec §4.2 对话/输入/审批三类 surface。
8. **执行计划超大文件的处理教训:** 本计划文件总计约 1700 行。Write 一次性写整文件会超 tool call payload 上限,Edit 追加大段内容同样。每个 Task(< 1500 行内容)单独 Edit/Write 是安全粒度。后续若写更大计划,直接按 Task 分多份 `.md` 或一开始就按这个粒度追加。
