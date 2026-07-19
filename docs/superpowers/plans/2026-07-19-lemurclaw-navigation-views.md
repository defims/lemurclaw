# lemurclaw 导航与全屏视图 实现计划(子项目 4)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 给 lemurclaw-gui 加上 spec §6.1 列的 6 个导航/全屏 surface:**SessionPicker(侧栏常驻)+ ModelPicker + AgentPicker + Onboarding + TranscriptPager(加载历史 + 全屏)+ ThemePicker**。完成标准:用户能在 GUI 里切换历史会话、换模型、查看完整 transcript、首次启动走 onboarding、切主题;功能等价 codex TUI 的导航类 surface。

**Architecture:**
- **侧栏常驻布局**:扩 App.tsx,加一个右侧栏(spec §4.3 的"会话"区),SessionPicker 嵌在侧栏里常驻。其它 picker(ModelPicker / ThemePicker / AgentPicker)走模态/弹层。
- **AppServerClient 请求**:新增 `thread/list` / `thread/resume` / `thread/read` / `model/list` / `getAuthStatus` 的 typed wrapper(在 transport.ts 或 hooks 里),复用子项目 3 的 `send` + 既有 reducer。
- **TranscriptPager**:全屏 overlay,调 `thread/read { includeTurns: true }` 拿历史 turns,复用子项目 3 的 `threadItemToCell` + Scrollback 的 cell 渲染。
- **Onboarding**:首启动检测(`getAuthStatus` 返回未登录)+ 一个最小的 welcome + auth 链接引导。子项目 4 不做完整 trust 目录管理(留给后续),只做"识别未登录 + 提示用户跑 CLI 登录"的最小路径。
- **本地配置(ThemePicker)**:`localStorage` 存主题名,根 div 加 `data-theme` 属性,CSS 用 CSS variables 切换。

**Tech Stack:** React 18.3 + TypeScript 5.6 + Vite 5.4 + vitest 2.x(子项目 2/3 已就位)+ 既有 codex ts-rs 类型 + 子项目 3 的 ViewModel/Scrollback/cell 基础设施。

**Spec:** `docs/superpowers/specs/2026-07-18-lemurclaw-codex-gui-design.md`(§4.2 导航/onboarding/transcript 类 + §4.3 主窗口布局 + §6.1 子项目 4)
**前置:** 子项目 3 完成(见 `docs/superpowers/plans/2026-07-19-lemurclaw-core-conversation-ui.md`)。子项目 3 的 ViewModel reducer / Scrollback / cell 组件 / transport.send 都已就位。

---

## 范围说明

本计划做 **子项目 4:导航与全屏视图**(spec §6.1 第 5 块):
- ✅ 侧栏布局扩展(App.tsx + Sidebar.tsx)
- ✅ SessionPicker(侧栏常驻,thread/list + thread/resume)
- ✅ TranscriptPager(全屏 overlay,thread/read includeTurns)
- ✅ ModelPicker(模态,model/list)
- ✅ AgentPicker(侧栏 agent 区,占位 + 真实多 agent 状态)
- ✅ Onboarding(最小:getAuthStatus 检测 + welcome + 提示跑 CLI 登录)
- ✅ ThemePicker(localStorage + CSS variables)

**不做**(留给后续子项目):
- ❌ SettingsModal 系列(permissions/keymap/memories/skills/hooks/mcp/apps/plugins/experimental/statusline/title)— 子项目 5
- ❌ SlashPopup / MentionPopup / FileSearchPopup 的 fuzzy 交互 — 子项目 5
- ❌ 完整 trust 目录管理 / GUI 内的 OAuth 登录流 — 子项目 5+(Onboarding 只识别未登录并引导)
- ❌ webui 模式 — 子项目 6
- ❌ 键盘快捷键系统(只做最少的 Ctrl+T 开 transcript、Escape 关 modal)

**变更规模控制:** 每个任务独立 commit,单任务 < 300 行变更。计划共 9 个任务,建议分 2-3 个 PR(4.1-4.3 / 4.4-4.6 / 4.7-4.9)。

---

## 文件结构

新增/修改文件总览(每个文件单一职责,< 500 LoC):

| 文件 | 责任 | 任务 |
|---|---|---|
| `assets/src/transport.ts` | 加 typed request helper:`sendRequest(method, params) -> Promise<result>`(用 RequestId 等待匹配的 response)+ onResponse 回调 | 4.1 |
| `codex-rs/lemurclaw-gui/src/backend.rs` | handle_ipc 把 ClientRequest 的 JSON-RPC response 推回 JS(经 tao proxy + onResponse) | 4.1 |
| `codex-rs/lemurclaw-gui/src/lib.rs` | GuiEvent 加 Response 变体,onResponse 推送 | 4.1 |
| `assets/src/hooks/useRequest.ts` | 通用 typed request hook(loading/error/data) | 4.2 |
| `assets/src/hooks/useThreadList.ts` | thread/list 分页 hook | 4.2 |
| `assets/src/components/Sidebar.tsx` | 右侧栏容器(spec §4.3 "会话"区) | 4.3 |
| `assets/src/components/sidebar/SessionPicker.tsx` | 会话列表 + 切换(thread/resume) | 4.3 |
| `assets/src/components/TranscriptPager.tsx` | 全屏 transcript overlay(thread/read includeTurns) | 4.4 |
| `assets/src/components/ModelPicker.tsx` | 模型选择模态(model/list) | 4.5 |
| `assets/src/components/sidebar/AgentPanel.tsx` | 侧栏 agent 区(占位 + 真实状态) | 4.6 |
| `assets/src/components/Onboarding.tsx` | 首启动 onboarding 最小流 | 4.7 |
| `assets/src/hooks/useTheme.ts` + `themes.ts` | 主题 hook(localStorage + data-theme) | 4.8 |
| `assets/src/components/ThemePicker.tsx` | 主题选择模态 | 4.8 |
| `assets/src/app/App.tsx` | 装配侧栏 + modal host + onboarding gate + Ctrl+T 监听 | 4.9 |

---

## Task 4.1:typed request 通道(JS 发请求 + 收 response)

**目标:** SessionPicker / TranscriptPager / ModelPicker 都要发 ClientRequest 并等 codex 返回的 JSON-RPC response(不只是单向 ServerNotification)。当前 transport 只有 `send`(发出去就完事)+ `onEvent`(收 ServerNotification)。加一个 typed request 通道。

**核心设计:**
- JS 发 ClientRequest 时记下 `id`,backend 推回来的 JSON-RPC response envelope(`{jsonrpc:"2.0", id, result}` 或 `{jsonrpc:"2.0", id, error}`)用 id 匹配 pending promise。
- backend.rs::handle_ipc 现在调 `handle.request(req).await` 但**丢弃返回值**。改成把返回的 `RequestResult` 序列化为 JSON-RPC envelope,经 proxy 推回 JS。

**Files:**
- Modify: `codex-rs/lemurclaw-gui/src/backend.rs`(~40 行)
- Modify: `codex-rs/lemurclaw-gui/src/lib.rs`(~15 行)
- Modify: `assets/src/transport.ts`(~60 行)
- Create: `assets/src/transport.test.ts`(~60 行)

- [ ] **Step 1: backend.rs — handle_ipc 把 response 推回 JS**

Modify `codex-rs/lemurclaw-gui/src/backend.rs` 的 `handle_ipc`。在既有 ClientRequest 分支里,拿到 `value` 时先取 id,await `handle.request(req).await` 后把结果序列化推回。

需要给 `BackendHandles` 加一个 `proxy: EventLoopProxy<GuiEvent>` 字段(从 spawn 传进来;`EventLoopProxy` 是 Clone 的)。然后在 ClientRequest 分支:

```rust
Ok(req) => {
    // 在 from_value 之前从原始 value 取 id(每个 ClientRequest 变体的 id 字段都叫 id)
    let req_id_json = value.get("id").cloned().unwrap_or(serde_json::Value::Null);
    let proxy = proxy.clone();
    match handle.request(req).await {
        Ok(result) => {
            let envelope = match result {
                Ok(val) => serde_json::json!({
                    "jsonrpc": "2.0", "id": req_id_json, "result": val,
                }),
                Err(err) => serde_json::json!({
                    "jsonrpc": "2.0", "id": req_id_json,
                    "error": { "code": err.code, "message": err.message, "data": err.data },
                }),
            };
            if let Ok(json) = serde_json::to_string(&envelope) {
                if let Err(e) = proxy.send_event(GuiEvent::Response(json)) {
                    eprintln!("[lemurclaw] response proxy closed: {e}");
                }
            }
        }
        Err(e) => eprintln!("[lemurclaw] backend request failed: {e}"),
    }
}
```

> **实现要点:**
> 1. `value.get("id")` 在原始 `serde_json::Value` 上取 id(在 `from_value::<ClientRequest>` 之前,因为 enum 化之后取 id 麻烦)。`.cloned()` 拿到 `Value`(可能是 number 或 string)。`unwrap_or(Value::Null)` 兜底。
> 2. `RequestResult = Result<JsonRpcResult, JSONRPCErrorError>`(lib.rs:93),`JsonRpcResult` 就是 `serde_json::Value`(lib.rs:43)。
> 3. `JSONRPCErrorError` 字段:code(i64)/message(String)/data(Option<Value>),已在 Task 3.1 核实。
> 4. `proxy` 字段需要从 `spawn` 传进来。`spawn` 已经接收 proxy 参数(给 next_event loop 用)。同一个 proxy 可以发任意 GuiEvent 变体 —— 复用即可,不用加新字段。`handle_ipc` 的闭包要 move proxy clone(因为 closure 已经 move 了 handle)。

- [ ] **Step 2: lib.rs — GuiEvent 加 Response 变体 + 分发**

Modify `codex-rs/lemurclaw-gui/src/lib.rs`:

```rust
#[derive(Clone, Debug)]
enum GuiEvent {
    ServerEvent(String),
    Response(String),  // NEW: JSON-RPC response envelope from ClientRequest
}
```

在 `event_loop.run` 的 `Event::UserEvent` match 里加分支:
```rust
Event::UserEvent(GuiEvent::Response(json)) => {
    let escaped = escape_js_string(&json);
    let script = format!("window.__lemurclaw.onResponse(\"{escaped}\")");
    if let Err(e) = webview.evaluate_script(&script) {
        eprintln!("[lemurclaw] evaluate_script failed: {e}");
    }
}
```

init_script 的 `window.__lemurclaw` 安装也要加 onResponse stub:
```js
window.__lemurclaw = {
  onEvent: function(json) { console.log('[lemurclaw] onEvent (stub)', json); },
  onResponse: function(json) { console.log('[lemurclaw] onResponse (stub)', json); },
};
```

- [ ] **Step 3: transport.ts — 加 sendRequest + registerResponseHandler**

Modify `assets/src/transport.ts`,在文件末尾追加:

```ts
// ---------------------------------------------------------------------------
// Typed request channel: send a ClientRequest and await its JSON-RPC response.
//
// The Rust backend (backend.rs::handle_ipc) wraps every ClientRequest's
// RequestResult in a `{jsonrpc:"2.0", id, result|error}` envelope and pushes
// it back via window.__lemurclaw.onResponse. We match by id to settle the
// pending promise.

declare global {
  interface Window {
    ipc?: { postMessage: (s: string) => void };
    __lemurclaw?: {
      onEvent: (json: string) => void;
      onResponse?: (json: string) => void;
    };
  }
}

const pendingRequests = new Map<
  string | number,
  { resolve: (v: unknown) => void; reject: (e: Error) => void; timer: ReturnType<typeof setTimeout> }
>();
let nextRequestId = 1000; // avoid colliding with backend/server-assigned ids

/**
 * Send a ClientRequest and return a Promise that resolves with the
 * JSON-RPC `result`, or rejects with the error message.
 *
 * The id is assigned locally (monotonic from 1000) and the response is
 * matched by id via onResponse. Auto-rejects after 30s to avoid leaked
 * promises on dropped responses.
 */
export function sendRequest<T = unknown>(method: string, params: unknown): Promise<T> {
  const id = nextRequestId++;
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => {
      if (pendingRequests.has(id)) {
        pendingRequests.delete(id);
        reject(new Error(`request ${method} (id=${id}) timed out after 30s`));
      }
    }, 30_000);
    pendingRequests.set(id, { resolve: (v) => resolve(v as T), reject, timer });
    send({ method, id, params });
  });
}

/** Install the onResponse handler (called once by App on mount). Each
 *  JSON-RPC response envelope is matched against pendingRequests by id. */
export function registerResponseHandler(): void {
  if (!window.__lemurclaw) window.__lemurclaw = { onEvent: () => {} };
  window.__lemurclaw.onResponse = (json: string) => {
    let envelope: { id?: unknown; result?: unknown; error?: { code?: number; message?: string } };
    try {
      envelope = JSON.parse(json);
    } catch (e) {
      console.error('transport.onResponse: parse failed', e);
      return;
    }
    const id = envelope.id;
    if (id === undefined || (typeof id !== 'string' && typeof id !== 'number')) return;
    const pending = pendingRequests.get(id as string | number);
    if (!pending) return;
    clearTimeout(pending.timer);
    pendingRequests.delete(id as string | number);
    if (envelope.error) {
      pending.reject(new Error(envelope.error.message ?? `request failed (code ${envelope.error.code})`));
    } else {
      pending.resolve(envelope.result);
    }
  };
}
```

> **注:** 现有 transport.ts 里 `declare global` 已经声明了 `__lemurclaw?` 只有 `onEvent`。Step 3 把 onResponse 加进去(上面的 declare 块替换既有那个)。

- [ ] **Step 4: transport.test.ts**

Create `assets/src/transport.test.ts`:
```ts
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { sendRequest, registerResponseHandler, send } from './transport';

describe('sendRequest', () => {
  beforeEach(() => {
    vi.mocked(send).mockClear?.();
    registerResponseHandler();
  });

  it('resolves with result when matching response arrives', async () => {
    const p = sendRequest('thread/list', { limit: 5 });
    expect(send).toHaveBeenCalledWith(expect.objectContaining({
      method: 'thread/list', params: { limit: 5 },
    }));
    const sent = vi.mocked(send).mock.calls.at(-1)![0] as { id: number };
    const handler = (window.__lemurclaw as { onResponse: (s: string) => void }).onResponse;
    handler(JSON.stringify({
      jsonrpc: '2.0', id: sent.id,
      result: { data: [], nextCursor: null, backwardsCursor: null },
    }));
    await expect(p).resolves.toEqual({ data: [], nextCursor: null, backwardsCursor: null });
  });

  it('rejects when error response arrives', async () => {
    const p = sendRequest('model/list', {});
    const sent = vi.mocked(send).mock.calls.at(-1)![0] as { id: number };
    const handler = (window.__lemurclaw as { onResponse: (s: string) => void }).onResponse;
    handler(JSON.stringify({
      jsonrpc: '2.0', id: sent.id,
      error: { code: -32601, message: 'method not found' },
    }));
    await expect(p).rejects.toThrow('method not found');
  });

  it('ignores responses with unknown id (no crash)', () => {
    registerResponseHandler();
    const handler = (window.__lemurclaw as { onResponse: (s: string) => void }).onResponse;
    // 没 pending request 匹配 id=99999
    expect(() => handler(JSON.stringify({ jsonrpc: '2.0', id: 99999, result: {} }))).not.toThrow();
  });
});
```

- [ ] **Step 5: 验证 + commit**

```bash
cd /Users/def/lemurclaw/codex-rs
cargo check -p lemurclaw-gui
cargo clippy -p lemurclaw-gui
cargo fmt -p lemurclaw-gui
cd lemurclaw-gui/assets
npm test -- transport
npx tsc --noEmit
cd /Users/def/lemurclaw
git add codex-rs/lemurclaw-gui/src/backend.rs codex-rs/lemurclaw-gui/src/lib.rs \
        codex-rs/lemurclaw-gui/assets/src/transport.ts codex-rs/lemurclaw-gui/assets/src/transport.test.ts
git commit -m "feat(gui): typed request channel (sendRequest + onResponse matching by id)"
```

---

## Task 4.2:useRequest + useThreadList hooks

**目标:** SessionPicker / ModelPicker / TranscriptPager 共用的 React hooks。`useRequestLazy` 是通用的"发请求 + 等响应 + loading/error 状态";`useThreadList` 是 thread/list 的分页封装。

**Files:**
- Create: `assets/src/hooks/useRequest.ts`
- Create: `assets/src/hooks/useThreadList.ts`
- Create: `assets/src/hooks/__tests__/useRequest.test.ts`
- Create: `assets/src/hooks/__tests__/useThreadList.test.ts`

- [ ] **Step 1: useRequest.ts**

Create `assets/src/hooks/useRequest.ts`:
```ts
import { useState, useCallback } from 'react';
import { sendRequest } from '../transport';

interface RequestState<T> {
  data: T | null;
  loading: boolean;
  error: string | null;
}

/**
 * Lazy typed-request hook: returns a `run` function the caller invokes
 * explicitly (no auto-fire on mount). Most callers (SessionPicker refresh,
 * ModelPicker open) want to control when the request fires.
 */
export function useRequestLazy<T>() {
  const [state, setState] = useState<RequestState<T>>({ data: null, loading: false, error: null });

  const run = useCallback(async (method: string, params: unknown): Promise<T | null> => {
    setState((s) => ({ ...s, loading: true, error: null }));
    try {
      const data = await sendRequest<T>(method, params);
      setState({ data, loading: false, error: null });
      return data;
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setState({ data: null, loading: false, error: msg });
      return null;
    }
  }, []);

  return { ...state, run };
}
```

- [ ] **Step 2: useThreadList.ts**

Create `assets/src/hooks/useThreadList.ts`:
```ts
import { useState, useCallback, useEffect } from 'react';
import { sendRequest } from '../transport';
import type { Thread } from '../types/v2';
import type { ThreadListResponse } from '../types/v2/ThreadListResponse';

interface ThreadListState {
  threads: Thread[];
  loading: boolean;
  error: string | null;
  nextCursor: string | null;
}

/** Paginated thread list hook. Auto-loads first page on mount; `loadMore`
 *  appends the next page using the returned cursor. Used by SessionPicker. */
export function useThreadList(limit: number = 20) {
  const [state, setState] = useState<ThreadListState>({
    threads: [], loading: false, error: null, nextCursor: null,
  });

  const fetchPage = useCallback(async (cursor: string | null, replace: boolean) => {
    setState((s) => ({ ...s, loading: true, error: null }));
    try {
      const resp = await sendRequest<ThreadListResponse>('thread/list', { limit, cursor });
      setState((s) => ({
        threads: replace ? resp.data : [...s.threads, ...resp.data],
        loading: false, error: null, nextCursor: resp.nextCursor,
      }));
    } catch (e) {
      setState((s) => ({ ...s, loading: false, error: e instanceof Error ? e.message : String(e) }));
    }
  }, [limit]);

  useEffect(() => { fetchPage(null, true); }, [fetchPage]);

  const loadMore = useCallback(() => {
    if (state.nextCursor && !state.loading) fetchPage(state.nextCursor, false);
  }, [state.nextCursor, state.loading, fetchPage]);

  const refresh = useCallback(() => { fetchPage(null, true); }, [fetchPage]);

  return { ...state, loadMore, refresh };
}
```

- [ ] **Step 3: useRequest.test.ts**

Create `assets/src/hooks/__tests__/useRequest.test.ts`:
```ts
import { describe, it, expect, vi, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useRequestLazy } from '../useRequest';

vi.mock('../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../transport';

// NOTE: use `afterEach`, not `beforeEach`, for `mockReset()` here.
// Vitest 2.1.9 has a false-positive unhandled-rejection failure when
// `beforeEach(() => mockReset())` is combined with `mockRejectedValue`:
// the reset runs before the test body, and Vitest's mock/unhandled-rejection
// interaction attributes the rejected promise to the `new Error(...)` site
// even though the hook's `.catch` consumes it. `afterEach` runs cleanup
// *after* the test body, so the rejection is already settled. This applies
// to ALL transport-mocked tests in this subproject (Tasks 4.2/4.4/4.7/4.8).
describe('useRequestLazy', () => {
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('transitions through loading → data on success', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ ok: true } as never);
    const { result } = renderHook(() => useRequestLazy());
    expect(result.current.data).toBeNull();
    let p!: Promise<unknown>;
    act(() => { p = result.current.run('thread/list', {}); });
    expect(result.current.loading).toBe(true);
    await act(async () => { await p; });
    expect(result.current.loading).toBe(false);
    expect(result.current.data).toEqual({ ok: true });
    expect(result.current.error).toBeNull();
  });

  it('captures error on failure', async () => {
    vi.mocked(sendRequest).mockRejectedValue(new Error('boom'));
    const { result } = renderHook(() => useRequestLazy());
    let p!: Promise<unknown>;
    act(() => { p = result.current.run('thread/list', {}); });
    await act(async () => { await p; });
    expect(result.current.error).toBe('boom');
    expect(result.current.data).toBeNull();
  });
});
```

- [ ] **Step 4: useThreadList.test.ts**

Create `assets/src/hooks/__tests__/useThreadList.test.ts`:
```ts
import { describe, it, expect, vi, afterEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { useThreadList } from '../useThreadList';

vi.mock('../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../transport';

function makeThread(id: string) {
  return { id, sessionId: 's', forkedFromId: null, parentThreadId: null, preview: `t-${id}`,
    ephemeral: false, modelProvider: 'p', createdAt: 1, updatedAt: 1, recencyAt: null,
    status: { type: 'idle' }, path: null, cwd: { path: '/x' }, cliVersion: '0', source: 'Cli',
    threadSource: null, agentNickname: null, agentRole: null, gitInfo: null, name: null, turns: [] } as never;
}

describe('useThreadList', () => {
  // See useRequest.test.ts for why this is `afterEach` not `beforeEach`.
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('auto-loads first page on mount', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ data: [makeThread('1'), makeThread('2')], nextCursor: null, backwardsCursor: null });
    const { result } = renderHook(() => useThreadList());
    await waitFor(() => expect(result.current.threads).toHaveLength(2));
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('loadMore appends next page', async () => {
    vi.mocked(sendRequest)
      .mockResolvedValueOnce({ data: [makeThread('1')], nextCursor: 'cur', backwardsCursor: null })
      .mockResolvedValueOnce({ data: [makeThread('2')], nextCursor: null, backwardsCursor: null });
    const { result } = renderHook(() => useThreadList());
    await waitFor(() => expect(result.current.threads).toHaveLength(1));
    await act(async () => { await result.current.loadMore(); });
    expect(result.current.threads.map((t) => t.id)).toEqual(['1', '2']);
  });

  it('refresh replaces list', async () => {
    vi.mocked(sendRequest)
      .mockResolvedValueOnce({ data: [makeThread('1')], nextCursor: null, backwardsCursor: null })
      .mockResolvedValueOnce({ data: [makeThread('9')], nextCursor: null, backwardsCursor: null });
    const { result } = renderHook(() => useThreadList());
    await waitFor(() => expect(result.current.threads).toHaveLength(1));
    await act(async () => { await result.current.refresh(); });
    expect(result.current.threads.map((t) => t.id)).toEqual(['9']);
  });
});
```

- [ ] **Step 5: 验证 + commit**

```bash
cd /Users/def/lemurclaw/codex-rs/lemurclaw-gui/assets
npm test -- useRequest useThreadList
npx tsc --noEmit
cd /Users/def/lemurclaw
git add codex-rs/lemurclaw-gui/assets/src/hooks/
git commit -m "feat(gui): useRequest + useThreadList hooks"
```

---

## Task 4.3:Sidebar + SessionPicker(侧栏常驻)

**目标:** 扩 App.tsx 加右侧栏(spec §4.3 "会话"区),SessionPicker 嵌在侧栏里常驻。能拉历史会话列表 + 点切换(thread/resume)+ 当前会话高亮。

**Files:**
- Create: `assets/src/components/Sidebar.tsx`
- Create: `assets/src/components/sidebar/SessionPicker.tsx`
- Create: `assets/src/components/sidebar/__tests__/SessionPicker.test.tsx`
- Modify: `assets/src/app/App.tsx`(加侧栏装配)
- Modify: `assets/src/styles.css`(加 `.app-sidebar` + `.session-picker` 样式)

- [ ] **Step 1: Sidebar.tsx 容器**

Create `assets/src/components/Sidebar.tsx`:
```tsx
import type { ReactNode } from 'react';

interface SidebarSection {
  key: string;
  title: string;
  body: ReactNode;
}

interface Props {
  sections: SidebarSection[];
  collapsed?: boolean;
}

/** Right sidebar (spec §4.3 "会话/Agent/Plan" rail). Sections stack vertically;
 *  each has a title + body. Subproject 4 fills in SessionPicker (always) +
 *  AgentPanel (Task 4.6); Plan section is reserved for subproject 5+. */
export function Sidebar({ sections, collapsed = false }: Props) {
  if (collapsed) return null;
  return (
    <aside className="app-sidebar" data-testid="sidebar">
      {sections.map((s) => (
        <section key={s.key} className={`sidebar-section sidebar-${s.key}`}>
          <h3 className="sidebar-section-title">{s.title}</h3>
          <div className="sidebar-section-body">{s.body}</div>
        </section>
      ))}
    </aside>
  );
}
```

- [ ] **Step 2: SessionPicker.tsx**

Create `assets/src/components/sidebar/SessionPicker.tsx`:
```tsx
import { useThreadList } from '../../hooks/useThreadList';
import { send } from '../../transport';
import type { Thread } from '../../types/v2';

interface Props {
  /** Currently active thread id (for highlight). Null pre-first-thread. */
  activeThreadId: string | null;
  /** Override the default thread/resume dispatch (used by tests). */
  onSelect?: (thread: Thread) => void;
}

/** Session picker: lists past threads (thread/list, paginated), highlights
 *  the active one, and switches on click (thread/resume).
 *
 *  Lives in the sidebar (spec §4.3 "会话"区). Subproject 4 doesn't implement
 *  fuzzy search or rename — those land in subproject 5 (SettingsModal). */
export function SessionPicker({ activeThreadId, onSelect }: Props) {
  const { threads, loading, error, loadMore, refresh, nextCursor } = useThreadList(20);

  const handleSelect = (thread: Thread) => {
    if (onSelect) {
      onSelect(thread);
    } else {
      // Fire-and-forget thread/resume. The thread/started ServerNotification
      // (consumed by the reducer) will confirm the switch.
      send({
        method: 'thread/resume',
        id: Date.now(),
        params: { threadId: thread.id },
      });
    }
  };

  if (loading && threads.length === 0) {
    return <div className="session-picker session-picker-loading" data-testid="session-picker">loading…</div>;
  }
  if (error) {
    return (
      <div className="session-picker session-picker-error" data-testid="session-picker">
        <div>failed to load: {error}</div>
        <button onClick={refresh}>retry</button>
      </div>
    );
  }
  if (threads.length === 0) {
    return <div className="session-picker session-picker-empty" data-testid="session-picker">no sessions yet</div>;
  }

  return (
    <div className="session-picker" data-testid="session-picker">
      <ul className="session-list">
        {threads.map((t) => (
          <li
            key={t.id}
            className={`session-item${t.id === activeThreadId ? ' session-item-active' : ''}`}
          >
            <button onClick={() => handleSelect(t)} className="session-item-button">
              <span className="session-item-preview">{t.preview || t.name || '(untitled)'}</span>
              <span className="session-item-meta">
                {t.modelProvider} · {new Date((t.recencyAt ?? t.updatedAt) * 1000).toLocaleDateString()}
              </span>
            </button>
          </li>
        ))}
      </ul>
      {nextCursor && (
        <button onClick={loadMore} className="session-load-more" disabled={loading}>
          {loading ? 'loading…' : 'load more'}
        </button>
      )}
    </div>
  );
}
```

> **注:** `t.recencyAt` 和 `t.updatedAt` 在 Thread 类型里都是 Unix **秒**(codex 约定,已核实 Thread.ts),所以 `* 1000` 转 ms 给 Date。`recencyAt ?? updatedAt` 优先用 recency。

- [ ] **Step 3: SessionPicker.test.tsx**

Create `assets/src/components/sidebar/__tests__/SessionPicker.test.tsx`:
```tsx
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SessionPicker } from '../SessionPicker';

vi.mock('../../../hooks/useThreadList', () => ({ useThreadList: vi.fn() }));
vi.mock('../../../transport', () => ({ send: vi.fn() }));

import { useThreadList } from '../../../hooks/useThreadList';
import { send } from '../../../transport';
import type { Thread } from '../../../types/v2';

function makeThread(over: Partial<Thread> = {}): Thread {
  return {
    id: 't1', sessionId: 's', forkedFromId: null, parentThreadId: null,
    preview: 'hello', ephemeral: false, modelProvider: 'openai',
    createdAt: 1, updatedAt: 1, recencyAt: null, status: { type: 'idle' },
    path: null, cwd: { path: '/x' }, cliVersion: '0', source: 'Cli',
    threadSource: null, agentNickname: null, agentRole: null, gitInfo: null,
    name: null, turns: [], ...over,
  } as Thread;
}

describe('SessionPicker', () => {
  // `afterEach` for consistency with other transport-mocked tests (see Task 4.2).
  afterEach(() => {
    vi.mocked(useThreadList).mockReset();
    vi.mocked(send).mockClear();
  });

  it('renders threads and highlights active', () => {
    vi.mocked(useThreadList).mockReturnValue({
      threads: [makeThread({ id: 't1', preview: 'first' }), makeThread({ id: 't2', preview: 'second' })],
      loading: false, error: null, nextCursor: null, loadMore: vi.fn(), refresh: vi.fn(),
    } as never);
    render(<SessionPicker activeThreadId="t1" />);
    expect(screen.getByText('first')).toBeInTheDocument();
    expect(screen.getByText('second')).toBeInTheDocument();
    expect(screen.getByText('first').closest('.session-item')).toHaveClass('session-item-active');
  });

  it('clicking a session sends thread/resume', () => {
    vi.mocked(useThreadList).mockReturnValue({
      threads: [makeThread({ id: 't9', preview: 'click me' })],
      loading: false, error: null, nextCursor: null, loadMore: vi.fn(), refresh: vi.fn(),
    } as never);
    render(<SessionPicker activeThreadId={null} />);
    fireEvent.click(screen.getByText('click me'));
    expect(send).toHaveBeenCalledWith(expect.objectContaining({
      method: 'thread/resume', params: { threadId: 't9' },
    }));
  });

  it('shows empty state', () => {
    vi.mocked(useThreadList).mockReturnValue({
      threads: [], loading: false, error: null, nextCursor: null, loadMore: vi.fn(), refresh: vi.fn(),
    } as never);
    render(<SessionPicker activeThreadId={null} />);
    expect(screen.getByText('no sessions yet')).toBeInTheDocument();
  });

  it('shows error + retry calls refresh', () => {
    const refresh = vi.fn();
    vi.mocked(useThreadList).mockReturnValue({
      threads: [], loading: false, error: 'network down', nextCursor: null, loadMore: vi.fn(), refresh,
    } as never);
    render(<SessionPicker activeThreadId={null} />);
    expect(screen.getByText(/network down/)).toBeInTheDocument();
    fireEvent.click(screen.getByText('retry'));
    expect(refresh).toHaveBeenCalled();
  });
});
```

- [ ] **Step 4: App.tsx 装配侧栏**

Modify `assets/src/app/App.tsx`:
```tsx
import { useConversation } from './useConversation';
import { Scrollback } from '../components/Scrollback';
import { Composer } from '../components/Composer';
import { ApprovalCard } from '../components/ApprovalCard';
import { Sidebar } from '../components/Sidebar';
import { SessionPicker } from '../components/sidebar/SessionPicker';

/** Top-level GUI application. Spec §4.3 main column (scrollback + approvals
 *  + composer) + right sidebar (sessions / agent / plan rail). */
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
      <Sidebar
        sections={[
          { key: 'sessions', title: 'Sessions', body: <SessionPicker activeThreadId={threadId} /> },
        ]}
      />
    </div>
  );
}
```

- [ ] **Step 5: styles.css 加 sidebar 样式**

Append to `assets/src/styles.css`:
```css
.app-sidebar { width: 260px; flex-shrink: 0; border-left: 1px solid #ddd; background: #fff; padding: 8px; overflow-y: auto; }
.sidebar-section { margin-bottom: 16px; }
.sidebar-section-title { font-size: 11px; text-transform: uppercase; letter-spacing: 0.5px; color: #888; margin: 0 0 6px 0; }
.sidebar-section-body { display: flex; flex-direction: column; gap: 4px; }

.session-picker { display: flex; flex-direction: column; gap: 6px; }
.session-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 2px; }
.session-item { padding: 0; }
.session-item-button { width: 100%; text-align: left; background: none; border: none; cursor: pointer; padding: 6px 8px; border-radius: 4px; display: flex; flex-direction: column; gap: 2px; }
.session-item-button:hover { background: #f0f0f0; }
.session-item-active .session-item-button { background: #e8f0fe; }
.session-item-preview { font-size: 13px; color: #1a1a1a; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.session-item-meta { font-size: 11px; color: #888; }
.session-load-more { background: none; border: 1px solid #ddd; padding: 4px; border-radius: 4px; cursor: pointer; font-size: 12px; }
.session-picker-loading, .session-picker-empty, .session-picker-error { font-size: 13px; color: #888; padding: 8px; }
.session-picker-error button { margin-top: 4px; padding: 2px 8px; }
```

- [ ] **Step 6: 验证 + commit**

```bash
cd /Users/def/lemurclaw/codex-rs/lemurclaw-gui/assets
npm test -- SessionPicker
npx tsc --noEmit
cd /Users/def/lemurclaw
git add codex-rs/lemurclaw-gui/assets/src/components/Sidebar.tsx \
        codex-rs/lemurclaw-gui/assets/src/components/sidebar/ \
        codex-rs/lemurclaw-gui/assets/src/app/App.tsx \
        codex-rs/lemurclaw-gui/assets/src/styles.css
git commit -m "feat(gui): sidebar + SessionPicker (thread/list + thread/resume)"
```

---

## Task 4.4:TranscriptPager(全屏 overlay,加载历史)

**目标:** 用户能查看当前会话(或任意历史会话)的完整 transcript —— 调 `thread/read { threadId, includeTurns: true }` 拿全量 turns,用 Scrollback 的 cell renderer 在全屏 overlay 里渲染。codex TUI 的 Ctrl+T pager 等价。

**核心设计:**
- 全屏 fixed-position overlay,Esc 关闭。
- mount 时发 `thread/read { includeTurns: true }` 拿 thread.turns(每 turn 有 items: ThreadItem[])。
- 复用子项目 3 的 `threadItemToCell`(已经是 export 的)把 ThreadItem 转 CellModel,然后 Scrollback 风格渲染(但只读,不分 turn,纯平铺 + 单一 auto-scroll)。

> **分页 known-limitation:** codex 协议下没有 ClientRequest 方法返回分页 turns(`TurnsPage`/`ThreadResumeInitialTurnsPageParams` 类型存在但无对应 RPC,已核实 `ClientRequest.ts`)。`thread/read` 一次拿全量 `thread.turns`。对典型 agent 会话(< 500 items)够用;超大历史的真分页需等 codex upstream 开放,本 task 不实现。

**Files:**
- Create: `assets/src/components/TranscriptPager.tsx`
- Create: `assets/src/components/__tests__/TranscriptPager.test.tsx`
- Modify: `assets/src/styles.css`(加 `.transcript-pager` 全屏样式)
- Modify: `assets/src/app/App.tsx`(加 Ctrl+T 监听 + overlay 装配)— **推迟到 Task 4.9 统一装配,本 task 只做组件**

- [ ] **Step 1: TranscriptPager.tsx**

Create `assets/src/components/TranscriptPager.tsx`:
```tsx
import { useEffect, useState } from 'react';
import { sendRequest } from '../transport';
import type { Thread } from '../types/v2';
import type { CellModel } from '../viewModel/types';
import { threadItemToCell } from '../viewModel/reducer';

interface Props {
  /** Thread to load. The pager fetches its turns on mount. */
  threadId: string;
  /** Close handler (Esc or backdrop click). */
  onClose: () => void;
}

interface ReadState {
  loading: boolean;
  error: string | null;
  cells: CellModel[];
  thread: Thread | null;
}

/** Full-screen transcript pager (codex TUI's Ctrl+T equivalent).
 *
 *  Loads the thread's full turn history via `thread/read { includeTurns: true }`
 *  and renders all items flat (no turn boundaries) using the same cell
 *  components as Scrollback. Read-only — no input, no approvals.
 *
 *  Close: Esc key or backdrop click. */
export function TranscriptPager({ threadId, onClose }: Props) {
  const [state, setState] = useState<ReadState>({ loading: true, error: null, cells: [], thread: null });

  useEffect(() => {
    let cancelled = false;
    setState({ loading: true, error: null, cells: [], thread: null });
    sendRequest<{ thread: Thread }>('thread/read', { threadId, includeTurns: true })
      .then((resp) => {
        if (cancelled) return;
        const cells = resp.thread.turns.flatMap((t) => t.items.map(threadItemToCell));
        setState({ loading: false, error: null, cells, thread: resp.thread });
      })
      .catch((e) => {
        if (cancelled) return;
        setState({ loading: false, error: e instanceof Error ? e.message : String(e), cells: [], thread: null });
      });
    return () => { cancelled = true; };
  }, [threadId]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  return (
    <div className="transcript-pager-overlay" data-testid="transcript-pager" onClick={onClose}>
      <div className="transcript-pager-content" onClick={(e) => e.stopPropagation()}>
        <header className="transcript-pager-header">
          <span className="transcript-pager-title">
            transcript · {state.thread?.name ?? state.thread?.preview ?? threadId}
          </span>
          <button className="transcript-pager-close" onClick={onClose} aria-label="close">✕</button>
        </header>
        <div className="transcript-pager-body">
          {state.loading && <div className="transcript-pager-loading">loading…</div>}
          {state.error && <div className="transcript-pager-error">failed: {state.error}</div>}
          {!state.loading && !state.error && state.cells.length === 0 && (
            <div className="transcript-pager-empty">(no items in transcript)</div>
          )}
          {!state.loading && !state.error && state.cells.length > 0 && (
            <FlatScrollback cells={state.cells} />
          )}
        </div>
      </div>
    </div>
  );
}

/** Read-only flat cell list. Reuses Scrollback's CellRenderer indirectly by
 *  importing the cells directly. Keeping this minimal (no auto-scroll, no
 *  turn structure) — the pager is for review, not interaction. */
function FlatScrollback({ cells }: { cells: CellModel[] }) {
  // Lazy import to avoid a cycle: Scrollback.tsx imports lots of cells; we
  // only need the renderer. Inline a minimal dispatcher instead.
  return (
    <div className="transcript-pager-cells">
      {cells.map((c, i) => (
        <div key={`${c.kind}-${i}`} className="transcript-pager-cell">
          <pre>{JSON.stringify(c, null, 2)}</pre>
        </div>
      ))}
    </div>
  );
}
```

> **实现注:** FlatScrollback 起步用 `JSON.stringify(c)` 渲染 cell。这跟子项目 3 的 Scrollback 用的真 cell 组件(UserMessageCell 等)不等价。**这是 Task 4.4 的最小实现**;真正的"复用 Scrollback 的 CellRenderer"作为 Task 4.4 的 Step 2 优化(把 Scrollback 的 `CellRenderer` 抽成共享 export,TranscriptPager 调它)。先做 Step 1 + 测试,Step 2 优化可以延后到子项目 4 收尾。

- [ ] **Step 2: 抽 Scrollback 的 CellRenderer 共享(可选优化)**

如果 Step 1 的 JSON.stringify 不够好,在 Task 4.4 Step 1 之后做这个优化:

Modify `assets/src/components/Scrollback.tsx` 把 `CellRenderer` 从内部 fn 改成 export:
```tsx
export function CellRenderer({ cell }: { cell: CellModel }) { ... }
```

Then modify `TranscriptPager.tsx` 的 FlatScrollback:
```tsx
import { CellRenderer } from './Scrollback';

function FlatScrollback({ cells }: { cells: CellModel[] }) {
  return (
    <div className="transcript-pager-cells">
      {cells.map((c, i) => <CellRenderer key={`${c.kind}-${i}`} cell={c} />)}
    </div>
  );
}
```

> 这个 Step 2 跟 Scrollback 测试不冲突(CellRenderer 的行为没变,只是 export 了)。如果选 Step 2,记得加 TranscriptPager 的测试验证 cell 正确渲染。

- [ ] **Step 3: TranscriptPager.test.tsx**

Create `assets/src/components/__tests__/TranscriptPager.test.tsx`:
```tsx
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { TranscriptPager } from '../TranscriptPager';

vi.mock('../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../transport';

const TURN_WITH_ITEMS = {
  id: 'tu1', status: 'completed' as const, itemsView: 'full' as const,
  error: null, startedAt: 1, completedAt: 2, durationMs: 1000,
  items: [
    { type: 'userMessage', id: 'u1', clientId: null, content: [{ type: 'text', text: 'hi', text_elements: [] }] },
    { type: 'agentMessage', id: 'a1', text: 'hello', phase: 'final_answer', memoryCitation: null },
  ],
};

const FULL_THREAD = {
  id: 't1', sessionId: 's', forkedFromId: null, parentThreadId: null, preview: 'hello thread',
  ephemeral: false, modelProvider: 'openai', createdAt: 1, updatedAt: 1, recencyAt: null,
  status: { type: 'idle' }, path: null, cwd: { path: '/x' }, cliVersion: '0', source: 'Cli',
  threadSource: null, agentNickname: null, agentRole: null, gitInfo: null, name: null,
  turns: [TURN_WITH_ITEMS],
};

describe('TranscriptPager', () => {
  // See useRequest.test.ts (Task 4.2) for why this is `afterEach` not `beforeEach`.
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('loads thread on mount and renders cells', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ thread: FULL_THREAD });
    render(<TranscriptPager threadId="t1" onClose={vi.fn()} />);
    await waitFor(() => expect(screen.queryByText('loading…')).toBeNull());
    // Step 1 用 JSON.stringify,验证 cells 有内容
    expect(screen.getByText(/transcript-pager-cell/)).toBeInTheDocument();
  });

  it('Esc calls onClose', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ thread: FULL_THREAD });
    const onClose = vi.fn();
    render(<TranscriptPager threadId="t1" onClose={onClose} />);
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });

  it('backdrop click calls onClose', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ thread: FULL_THREAD });
    const onClose = vi.fn();
    const { container } = render(<TranscriptPager threadId="t1" onClose={onClose} />);
    fireEvent.click(container.querySelector('.transcript-pager-overlay')!);
    expect(onClose).toHaveBeenCalled();
  });

  it('shows error state', async () => {
    vi.mocked(sendRequest).mockRejectedValue(new Error('network down'));
    render(<TranscriptPager threadId="t1" onClose={vi.fn()} />);
    await waitFor(() => expect(screen.getByText(/network down/)).toBeInTheDocument());
  });
});
```

> **注:** Step 1 的 JSON.stringify 渲染方式让测试只能断言"cells container 存在",不好断言具体内容。如果做了 Step 2(用 CellRenderer),测试可以断言 `screen.getByTestId('user-message')`。两者择一。

- [ ] **Step 4: styles.css 加 transcript-pager 样式**

Append to `assets/src/styles.css`:
```css
.transcript-pager-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.4); display: flex; align-items: center; justify-content: center; z-index: 1000; }
.transcript-pager-content { width: 90vw; height: 90vh; background: #fff; border-radius: 8px; display: flex; flex-direction: column; overflow: hidden; }
.transcript-pager-header { display: flex; justify-content: space-between; align-items: center; padding: 8px 12px; border-bottom: 1px solid #ddd; }
.transcript-pager-title { font-weight: bold; font-size: 14px; }
.transcript-pager-close { background: none; border: none; cursor: pointer; font-size: 16px; color: #888; }
.transcript-pager-body { flex: 1; overflow-y: auto; padding: 12px; }
.transcript-pager-loading, .transcript-pager-empty, .transcript-pager-error { color: #888; padding: 12px; }
.transcript-pager-cells { display: flex; flex-direction: column; gap: 8px; }
.transcript-pager-cell { padding: 8px; background: #f8f8f8; border-radius: 4px; font-family: 'SF Mono', Menlo, monospace; font-size: 12px; white-space: pre-wrap; }
```

- [ ] **Step 5: 验证 + commit**

```bash
cd /Users/def/lemurclaw/codex-rs/lemurclaw-gui/assets
npm test -- TranscriptPager
npx tsc --noEmit
cd /Users/def/lemurclaw
git add codex-rs/lemurclaw-gui/assets/src/components/TranscriptPager.tsx \
        codex-rs/lemurclaw-gui/assets/src/components/__tests__/TranscriptPager.test.tsx \
        codex-rs/lemurclaw-gui/assets/src/styles.css \
        codex-rs/lemurclaw-gui/assets/src/components/Scrollback.tsx  # if Step 2 done
git commit -m "feat(gui): TranscriptPager (full-screen overlay, thread/read includeTurns)"
```

---

## Task 4.5:ModelPicker(模态)

**目标:** 用户能查看可用模型列表并切换。模态打开时调 `model/list`,选中后发 `thread/start` 带 model override(或者发一个新的"切换当前 thread 模型"的请求 —— codex 协议下,切换已存在 thread 的模型靠 thread/metadata/update 或下个 turn 用 `turn/start { model }` override,这里采用后者更简单)。

**Files:**
- Create: `assets/src/components/ModelPicker.tsx`
- Create: `assets/src/components/__tests__/ModelPicker.test.tsx`
- Modify: `assets/src/styles.css`(加 `.modal` + `.model-picker` 样式)

- [ ] **Step 1: 先核实 Model 类型字段**

```bash
cat codex-rs/lemurclaw-gui/assets/src/types/v2/Model.ts
```
预期字段(参照 codex 协议):id/name/provider 之类。如果实际字段名不同,Step 2 的渲染要相应调整。

- [ ] **Step 2: ModelPicker.tsx**

Create `assets/src/components/ModelPicker.tsx`:
```tsx
import { useEffect, useState } from 'react';
import { sendRequest, send } from '../transport';
import type { Model } from '../types/v2';
import type { ModelListResponse } from '../types/v2/ModelListResponse';

interface Props {
  /** Active thread id. Sent as thread/start's threadId when switching. */
  threadId: string | null;
  /** Currently selected model id (for highlight). Optional. */
  currentModel?: string | null;
  onClose: () => void;
}

interface LoadState {
  loading: boolean;
  error: string | null;
  models: Model[];
}

/** Modal model picker. On open, calls `model/list` to enumerate available
 *  models; selecting one fires a `turn/start` with `model` override on the
 *  active thread (codex doesn't have a dedicated "switch model mid-thread"
 *  method — the override takes effect on the next turn).
 *
 *  Close: Esc, backdrop click, or ✕ button. */
export function ModelPicker({ threadId, currentModel, onClose }: Props) {
  const [state, setState] = useState<LoadState>({ loading: true, error: null, models: [] });

  useEffect(() => {
    let cancelled = false;
    setState({ loading: true, error: null, models: [] });
    sendRequest<ModelListResponse>('model/list', {})
      .then((resp) => {
        if (!cancelled) setState({ loading: false, error: null, models: resp.data });
      })
      .catch((e) => {
        if (!cancelled) setState({ loading: false, error: e instanceof Error ? e.message : String(e), models: [] });
      });
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') { e.preventDefault(); onClose(); }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  const handlePick = (model: Model) => {
    if (!threadId) return;
    // Switch model via next-turn override. Empty input means "no new user
    // message, just switch model" — codex treats empty input as a no-op turn,
    // so we add a minimal steering placeholder if needed. For now, send an
    // empty text; user can type in Composer after.
    send({
      method: 'turn/start',
      id: Date.now(),
      params: {
        threadId,
        input: [{ type: 'text', text: '', text_elements: [] }],
        model: model.id,
      },
    });
    onClose();
  };

  return (
    <div className="modal-overlay" data-testid="model-picker" onClick={onClose}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()}>
        <header className="modal-header">
          <span className="modal-title">select model</span>
          <button className="modal-close" onClick={onClose} aria-label="close">✕</button>
        </header>
        <div className="modal-body">
          {state.loading && <div className="modal-loading">loading…</div>}
          {state.error && <div className="modal-error">failed: {state.error}</div>}
          {!state.loading && !state.error && state.models.length === 0 && (
            <div className="modal-empty">no models configured</div>
          )}
          {!state.loading && !state.error && state.models.length > 0 && (
            <ul className="model-list">
              {state.models.map((m) => (
                <li
                  key={m.id}
                  className={`model-item${m.id === currentModel ? ' model-item-active' : ''}`}
                >
                  <button onClick={() => handlePick(m)} className="model-item-button" disabled={!threadId}>
                    <span className="model-item-name">{m.displayName || m.id}</span>
                    <span className="model-item-id">{m.id}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
}
```

> **注:** `Model` 的字段已核实(`types/v2/Model.ts`):有 `id`(模型标识)、`displayName`(用户可读名)、`description`、`isDefault`、`hidden` 等。**没有 `name` 字段** —— 渲染用 `m.displayName || m.id`(fallback 到 id 因为 displayName 不应为空但兜底)。Step 1 的 cat 核实可跳过(已确认)。
>
> **handlePick 的简化:** 真正的"切模型"理想做法是不发 turn,只更新 thread 的 model。但 codex 协议没有现成的 `thread/model/set` 方法,只有 `thread/metadata/update`(改 metadata,不改运行时 model)和 `turn/start { model }`(下个 turn 用这个 model)。本 task 用后者(发空 input turn)。这是个 known limitation —— 真实切换需要用户在 Composer 里再输入一句话触发 turn。文档化即可。

- [ ] **Step 3: ModelPicker.test.tsx**

Create `assets/src/components/__tests__/ModelPicker.test.tsx`:
```tsx
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ModelPicker } from '../ModelPicker';

vi.mock('../../transport', () => ({ sendRequest: vi.fn(), send: vi.fn() }));
import { sendRequest, send } from '../../transport';

describe('ModelPicker', () => {
  // See useRequest.test.ts (Task 4.2) for why this is `afterEach` not `beforeEach`.
  afterEach(() => {
    vi.mocked(sendRequest).mockReset();
    vi.mocked(send).mockClear();
  });

  it('loads models and renders list', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      data: [
        { id: 'gpt-4', displayName: 'GPT-4' } as never,
        { id: 'gpt-3.5-turbo', displayName: 'GPT-3.5' } as never,
      ],
      nextCursor: null,
    });
    render(<ModelPicker threadId="t1" onClose={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('GPT-4')).toBeInTheDocument());
    expect(screen.getByText('GPT-3.5')).toBeInTheDocument();
  });

  it('picking a model sends turn/start with model override', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      data: [{ id: 'claude-3', displayName: 'Claude 3' } as never], nextCursor: null,
    });
    const onClose = vi.fn();
    render(<ModelPicker threadId="t1" onClose={onClose} />);
    await waitFor(() => expect(screen.getByText('Claude 3')).toBeInTheDocument());
    fireEvent.click(screen.getByText('Claude 3'));
    expect(send).toHaveBeenCalledWith(expect.objectContaining({
      method: 'turn/start',
      params: expect.objectContaining({
        threadId: 't1',
        model: 'claude-3',
      }),
    }));
    expect(onClose).toHaveBeenCalled();
  });

  it('disables picking when threadId is null', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      data: [{ id: 'm1', displayName: 'M1' } as never], nextCursor: null,
    });
    render(<ModelPicker threadId={null} onClose={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('M1')).toBeInTheDocument());
    expect(screen.getByText('M1').closest('button')).toBeDisabled();
  });

  it('Esc closes', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ data: [], nextCursor: null });
    const onClose = vi.fn();
    render(<ModelPicker threadId="t1" onClose={onClose} />);
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });
});
```

- [ ] **Step 4: styles.css 加 modal 样式**

Append to `assets/src/styles.css`:
```css
.modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.4); display: flex; align-items: center; justify-content: center; z-index: 900; }
.modal-content { width: 480px; max-width: 90vw; max-height: 80vh; background: #fff; border-radius: 8px; display: flex; flex-direction: column; overflow: hidden; }
.modal-header { display: flex; justify-content: space-between; align-items: center; padding: 8px 12px; border-bottom: 1px solid #ddd; }
.modal-title { font-weight: bold; font-size: 14px; }
.modal-close { background: none; border: none; cursor: pointer; font-size: 16px; color: #888; }
.modal-body { padding: 12px; overflow-y: auto; flex: 1; }
.modal-loading, .modal-empty, .modal-error { color: #888; padding: 12px; }

.model-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 4px; }
.model-item-button { width: 100%; text-align: left; background: none; border: 1px solid #eee; cursor: pointer; padding: 8px; border-radius: 4px; display: flex; flex-direction: column; gap: 2px; }
.model-item-button:hover:not(:disabled) { background: #f0f0f0; }
.model-item-button:disabled { opacity: 0.5; cursor: not-allowed; }
.model-item-active .model-item-button { border-color: #4a90e2; background: #e8f0fe; }
.model-item-name { font-size: 14px; }
.model-item-id { font-size: 11px; color: #888; font-family: 'SF Mono', Menlo, monospace; }
```

- [ ] **Step 5: 验证 + commit**

```bash
cd /Users/def/lemurclaw/codex-rs/lemurclaw-gui/assets
npm test -- ModelPicker
npx tsc --noEmit
cd /Users/def/lemurclaw
git add codex-rs/lemurclaw-gui/assets/src/components/ModelPicker.tsx \
        codex-rs/lemurclaw-gui/assets/src/components/__tests__/ModelPicker.test.tsx \
        codex-rs/lemurclaw-gui/assets/src/styles.css
git commit -m "feat(gui): ModelPicker modal (model/list + turn/start model override)"
```

---

## Task 4.6:AgentPanel(侧栏 agent 区)

**目标:** spec §4.3 右侧栏的"Agent"区。展示当前 thread 的多 agent 状态(主 agent + 任何 sub-agent)。codex 的 collab agent 状态来自 `ThreadItem.collabAgentToolCall` 和 `subAgentActivity`(子项目 3 已映射到 CellModel 的 `generic` kind,本 task 把它们提到一个有结构的 ViewModel 切片)。

**核心设计:**
- 因为 lemurclaw-gui 当前还没有"多 agent 控制"功能(那是更后续的子项目),Task 4.6 做**最小占位 + 真实数据展示**:从 ConversationState 抽出 collab/subAgent items,渲染一个"Agent"区显示主 agent 状态 + sub-agent 列表(如果有)。**没有 spawn/control UI**。
- 如果 ConversationState 没有 collab/subAgent items(常见情况),显示"main agent"(当前 thread status)。

**Files:**
- Create: `assets/src/components/sidebar/AgentPanel.tsx`
- Create: `assets/src/components/sidebar/__tests__/AgentPanel.test.tsx`
- Modify: `assets/src/app/App.tsx`(侧栏 sections 加 agent)

- [ ] **Step 1: AgentPanel.tsx**

Create `assets/src/components/sidebar/AgentPanel.tsx`:
```tsx
import type { ConversationState } from '../../viewModel/types';

interface Props {
  state: ConversationState;
}

/** Sidebar "Agent" section. Shows main agent status + any sub-agent activity
 *  observed in the current conversation.
 *
 *  Subproject 4 minimal: this is read-only — no spawn/control UI. The multi-
 *  agent control surface (spawn, message, interrupt sub-agents) is reserved
 *  for a later subproject. */
export function AgentPanel({ state }: Props) {
  // Collect sub-agent activity by scanning turns[].items for collab/subAgent
  // cells. These currently map to CellModel 'generic' (Task 3.4 fallback),
  // so we re-scan raw state — actually we should look at the raw thread items
  // not the CellModel. For Task 4.6 minimal, we just show main agent + count.
  const mainStatus = state.status;
  const mainLabel = mainStatus === null
    ? '(not started)'
    : mainStatus.type === 'active'
      ? `active · ${(mainStatus.activeFlags ?? []).join(', ') || 'working'}`
      : mainStatus.type;

  return (
    <div className="agent-panel" data-testid="agent-panel">
      <div className="agent-row agent-row-main">
        <span className="agent-name">main</span>
        <span className="agent-status">{mainLabel}</span>
      </div>
      {/* Sub-agent rows deferred — lemurclaw-gui doesn't yet surface
          collab/subAgent items with structure (they go to CellModel 'generic'
          in Task 3.4). Add rows here once Task 3.4's reducer exports a typed
          sub-agent view. */}
      <div className="agent-empty-hint">sub-agent control deferred to a later subproject</div>
    </div>
  );
}
```

> **实现注:** 起步只渲染"main agent + status"。这跟 spec §4.3 的"Agent / ● main 运行中 / ○ sub 空闲"对照,主 agent 部分对齐,sub-agent 是占位。把 collab/subAgent 提升到有结构的 ViewModel 是个 follow-up(需要改 Task 3.4 的 reducer,不属于子项目 4 范围)。

- [ ] **Step 2: AgentPanel.test.tsx**

Create `assets/src/components/sidebar/__tests__/AgentPanel.test.tsx`:
```tsx
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { AgentPanel } from '../AgentPanel';
import { initialState } from '../../../viewModel/types';
import type { ConversationState } from '../../../viewModel/types';

describe('AgentPanel', () => {
  it('shows not-started when status is null', () => {
    render(<AgentPanel state={initialState} />);
    expect(screen.getByText('(not started)')).toBeInTheDocument();
  });

  it('shows idle status', () => {
    const state: ConversationState = { ...initialState, status: { type: 'idle' } };
    render(<AgentPanel state={state} />);
    expect(screen.getByText('idle')).toBeInTheDocument();
  });

  it('shows active status with flags', () => {
    const state: ConversationState = {
      ...initialState,
      status: { type: 'active', activeFlags: ['executing'] } as never,
    };
    render(<AgentPanel state={state} />);
    expect(screen.getByText(/active/)).toBeInTheDocument();
    expect(screen.getByText(/executing/)).toBeInTheDocument();
  });

  it('shows deferral hint for sub-agents', () => {
    render(<AgentPanel state={initialState} />);
    expect(screen.getByText(/sub-agent control deferred/)).toBeInTheDocument();
  });
});
```

> **ThreadStatus.active.activeFlags 字段类型:** spec 里是 `Array<ThreadActiveFlag>`,ThreadActiveFlag 可能是 string 或 enum。如果 TS 报错,cast `as never` 在 test fixture 里,或在 AgentPanel 里 `String(...)` 化。`activeFlags ?? []` 兼容 null/undefined。

- [ ] **Step 3: App.tsx 侧栏 sections 加 agent**

Modify `assets/src/app/App.tsx` 的 Sidebar sections:
```tsx
<Sidebar
  sections={[
    { key: 'sessions', title: 'Sessions', body: <SessionPicker activeThreadId={threadId} /> },
    { key: 'agents', title: 'Agent', body: <AgentPanel state={state} /> },
  ]}
/>
```

加 import:`import { AgentPanel } from '../components/sidebar/AgentPanel';`

- [ ] **Step 4: styles.css 加 agent-panel 样式**

Append to `assets/src/styles.css`:
```css
.agent-panel { display: flex; flex-direction: column; gap: 4px; }
.agent-row { display: flex; justify-content: space-between; align-items: center; padding: 4px 8px; border-radius: 4px; background: #f8f8f8; font-size: 12px; }
.agent-row-main { background: #e8f0fe; }
.agent-name { font-weight: 500; }
.agent-status { color: #666; font-size: 11px; }
.agent-empty-hint { font-size: 11px; color: #aaa; padding: 4px 8px; font-style: italic; }
```

- [ ] **Step 5: 验证 + commit**

```bash
cd /Users/def/lemurclaw/codex-rs/lemurclaw-gui/assets
npm test -- AgentPanel
npx tsc --noEmit
cd /Users/def/lemurclaw
git add codex-rs/lemurclaw-gui/assets/src/components/sidebar/AgentPanel.tsx \
        codex-rs/lemurclaw-gui/assets/src/components/sidebar/__tests__/AgentPanel.test.tsx \
        codex-rs/lemurclaw-gui/assets/src/app/App.tsx \
        codex-rs/lemurclaw-gui/assets/src/styles.css
git commit -m "feat(gui): AgentPanel sidebar section (main agent status, sub-agent deferred)"
```

---

## Task 4.7:Onboarding(最小:auth 检测 + welcome)

**目标:** 首启动检测(getAuthStatus)如果未登录,显示一个 welcome 屏 + 引导用户去 CLI 跑 `codex login`(或对应的登录命令)。**子项目 4 不做 GUI 内的 OAuth 流程**(那是 codex-api 的 WebSocket auth,复杂),只做检测 + 引导。

**Files:**
- Create: `assets/src/components/Onboarding.tsx`
- Create: `assets/src/components/__tests__/Onboarding.test.tsx`
- Modify: `assets/src/styles.css`(加 `.onboarding` 样式)

- [ ] **Step 1: getAuthStatus 返回结构(已核实)**

`types/GetAuthStatusResponse.ts`(在顶层 types/,非 v2/):
```ts
export type GetAuthStatusResponse = {
  authMethod: AuthMode | null,   // null = 未配置 auth;非 null = 已配置
  authToken: string | null,
  requiresOpenaiAuth: boolean | null,
};
```
`AuthMode`(`types/AuthMode.ts`):`"apikey" | "chatgpt" | "chatgptAuthTokens" | "headers" | "agentIdentity" | "personalAccessToken" | "bedrockApiKey"`。

**判定逻辑(本 task 用):** `authMethod !== null` → 已配置某种 auth(可能是 apikey/chatgpt/...)→ 视为 "authed",放行。`authMethod === null && requiresOpenaiAuth === true` → 需要 OpenAI auth 但没配 → "unauthed",显示引导。`authMethod === null && requiresOpenaiAuth !== true` → 既没配 auth 也不要求 → 放行(本地 only 模式)。Step 1 的 cat 可跳过(已核实)。

- [ ] **Step 2: Onboarding.tsx**

Create `assets/src/components/Onboarding.tsx`:
```tsx
import { useEffect, useState } from 'react';
import { sendRequest } from '../transport';

interface Props {
  /** Children to render once authed (or once user dismisses the prompt). */
  children: React.ReactNode;
}

type Phase = 'checking' | 'authed' | 'unauthed' | 'dismissed';

/** Minimal onboarding gate. On mount, calls `getAuthStatus`. If the response
 *  indicates no auth method is configured AND OpenAI auth is required, shows a
 *  welcome screen directing the user to the CLI to run `codex login`. Otherwise
 *  renders children directly.
 *
 *  Subproject 4 deliberately does NOT implement in-GUI OAuth — that's codex-
 *  api's WebSocket auth flow and is far heavier. The CLI handles login; the
 *  GUI just detects the result. */
export function Onboarding({ children }: Props) {
  const [phase, setPhase] = useState<Phase>('checking');
  const [authMethod, setAuthMethod] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    // Shape verified against types/GetAuthStatusResponse.ts:
    //   { authMethod: AuthMode | null, authToken: string | null, requiresOpenaiAuth: boolean | null }
    sendRequest<{ authMethod: string | null; requiresOpenaiAuth: boolean | null }>('getAuthStatus', {})
      .then((resp) => {
        if (cancelled) return;
        setAuthMethod(resp.authMethod);
        // "authed" = some auth method is configured (apikey/chatgpt/...).
        // "unauthed" = no method AND the server says OpenAI auth is required.
        // Otherwise (no method, not required) pass through — local-only mode.
        const authed = resp.authMethod !== null || resp.requiresOpenaiAuth !== true;
        setPhase(authed ? 'authed' : 'unauthed');
      })
      .catch(() => {
        // If getAuthStatus fails entirely, assume authed — don't block the UI
        // on a protocol issue. The user can still use apikey-configured setups.
        if (!cancelled) setPhase('authed');
      });
    return () => { cancelled = true; };
  }, []);

  if (phase === 'checking') {
    return <div className="onboarding onboarding-checking" data-testid="onboarding-checking">checking auth…</div>;
  }
  if (phase === 'authed' || phase === 'dismissed') {
    return <>{children}</>;
  }
  // phase === 'unauthed'
  return (
    <div className="onboarding onboarding-unauthed" data-testid="onboarding-unauthed">
      <div className="onboarding-card">
        <h2>welcome to lemurclaw</h2>
        <p>
          you're not signed in. lemurclaw needs a configured model provider
          {authMethod ? ` (current mode: ${authMethod})` : ''}.
        </p>
        <p>
          open a terminal in this project and run:
        </p>
        <pre className="onboarding-cmd">codex login</pre>
        <p>
          or set <code>OPENAI_API_KEY</code> in your environment, then restart lemurclaw.
        </p>
        <p className="onboarding-dismiss-hint">
          already configured? <button onClick={() => setPhase('dismissed')} className="onboarding-dismiss">continue anyway</button>
        </p>
      </div>
    </div>
  );
}
```

> **实现注:**
> 1. `getAuthStatus` 返回 shape 已核实(Step 1):`{authMethod, authToken, requiresOpenaiAuth}`。判定逻辑:有 authMethod → authed;无 authMethod 但 requiresOpenaiAuth=true → unauthed;无 authMethod 且不要求 → authed(本地模式放行)。
> 2. `codex login` 是 codex CLI 的命令(lemurclaw 复用),用户在系统终端跑,不是 GUI 里。
> 3. "continue anyway" 让用户能跳过 —— 应对 auth 检测误报或者用户用 apikey 配置(没走 OAuth)的情况。
> 4. 不显示 children 直到 authed/dismissed —— 用 Onboarding 包 App 的主内容做 gate(Task 4.9 装配)。

- [ ] **Step 3: Onboarding.test.tsx**

Create `assets/src/components/__tests__/Onboarding.test.tsx`:
```tsx
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { Onboarding } from '../Onboarding';

vi.mock('../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../transport';

describe('Onboarding', () => {
  // See useRequest.test.ts (Task 4.2) for why this is `afterEach` not `beforeEach`.
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('renders children when authed (authMethod set)', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ authMethod: 'chatgpt', authToken: null, requiresOpenaiAuth: false } as never);
    render(<Onboarding><div data-testid="app-content">app</div></Onboarding>);
    await waitFor(() => expect(screen.getByTestId('app-content')).toBeInTheDocument());
    expect(screen.queryByTestId('onboarding-unauthed')).toBeNull();
  });

  it('renders children when no authMethod but OpenAI auth not required (local mode)', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ authMethod: null, authToken: null, requiresOpenaiAuth: false } as never);
    render(<Onboarding><div data-testid="app-content">app</div></Onboarding>);
    await waitFor(() => expect(screen.getByTestId('app-content')).toBeInTheDocument());
  });

  it('shows welcome screen when unauthed (no method + requiresOpenaiAuth)', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ authMethod: null, authToken: null, requiresOpenaiAuth: true } as never);
    render(<Onboarding><div data-testid="app-content">app</div></Onboarding>);
    await waitFor(() => expect(screen.getByTestId('onboarding-unauthed')).toBeInTheDocument());
    expect(screen.queryByTestId('app-content')).toBeNull();
    expect(screen.getByText(/codex login/)).toBeInTheDocument();
  });

  it('"continue anyway" dismisses and shows children', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ authMethod: null, authToken: null, requiresOpenaiAuth: true } as never);
    render(<Onboarding><div data-testid="app-content">app</div></Onboarding>);
    await waitFor(() => expect(screen.getByTestId('onboarding-unauthed')).toBeInTheDocument());
    fireEvent.click(screen.getByText('continue anyway'));
    expect(screen.getByTestId('app-content')).toBeInTheDocument();
  });

  it('on getAuthStatus error, renders children (fail open)', async () => {
    vi.mocked(sendRequest).mockRejectedValue(new Error('boom'));
    render(<Onboarding><div data-testid="app-content">app</div></Onboarding>);
    await waitFor(() => expect(screen.getByTestId('app-content')).toBeInTheDocument());
  });
});
```

- [ ] **Step 4: styles.css 加 onboarding 样式**

Append to `assets/src/styles.css`:
```css
.onboarding { display: flex; align-items: center; justify-content: center; height: 100vh; padding: 24px; }
.onboarding-checking { color: #888; font-size: 14px; }
.onboarding-card { max-width: 480px; background: #fff; padding: 24px; border-radius: 8px; border: 1px solid #ddd; }
.onboarding-card h2 { margin: 0 0 12px 0; }
.onboarding-card p { margin: 8px 0; line-height: 1.5; }
.onboarding-cmd { background: #f4f4f4; padding: 8px 12px; border-radius: 4px; font-family: 'SF Mono', Menlo, monospace; font-size: 13px; }
.onboarding-dismiss-hint { margin-top: 16px; font-size: 12px; color: #888; }
.onboarding-dismiss { background: none; border: none; color: #4a90e2; cursor: pointer; text-decoration: underline; padding: 0; font: inherit; }
```

- [ ] **Step 5: 验证 + commit**

```bash
cd /Users/def/lemurclaw/codex-rs/lemurclaw-gui/assets
npm test -- Onboarding
npx tsc --noEmit
cd /Users/def/lemurclaw
git add codex-rs/lemurclaw-gui/assets/src/components/Onboarding.tsx \
        codex-rs/lemurclaw-gui/assets/src/components/__tests__/Onboarding.test.tsx \
        codex-rs/lemurclaw-gui/assets/src/styles.css
git commit -m "feat(gui): Onboarding minimal (getAuthStatus + welcome + CLI login hint)"
```

---

## Task 4.8:ThemePicker(本地配置 + CSS variables)

**目标:** 用户能切主题(light / dark / 高对比)。纯客户端 —— `localStorage` 存主题名,根 div 加 `data-theme` 属性,CSS 用 CSS variables 切换。

**Files:**
- Create: `assets/src/themes.ts`
- Create: `assets/src/hooks/useTheme.ts`
- Create: `assets/src/components/ThemePicker.tsx`
- Create: `assets/src/components/__tests__/ThemePicker.test.tsx`
- Modify: `assets/src/styles.css`(把硬编码颜色抽成 CSS variables + 加 `[data-theme]` 规则)

- [ ] **Step 1: themes.ts**

Create `assets/src/themes.ts`:
```ts
// Theme definitions for the GUI. Each theme is a map of CSS variable name →
// value, applied to the root element via `data-theme="<name>"`. The actual
// variable → CSS rule mapping lives in styles.css ([data-theme="..."] blocks);
// this file just enumerates the available theme names + metadata for the
// picker UI.

export type ThemeName = 'light' | 'dark' | 'high-contrast';

export interface ThemeMeta {
  name: ThemeName;
  label: string;
  description: string;
}

export const THEMES: ThemeMeta[] = [
  { name: 'light', label: 'Light', description: 'default bright theme' },
  { name: 'dark', label: 'Dark', description: 'low-light theme' },
  { name: 'high-contrast', label: 'High contrast', description: 'maximum text/background separation' },
];

export const DEFAULT_THEME: ThemeName = 'light';
```

- [ ] **Step 2: useTheme.ts**

Create `assets/src/hooks/useTheme.ts`:
```ts
import { useState, useCallback, useEffect } from 'react';
import { DEFAULT_THEME, type ThemeName } from '../themes';

const STORAGE_KEY = 'lemurclaw.theme';

function readStored(): ThemeName {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    if (v === 'light' || v === 'dark' || v === 'high-contrast') return v;
  } catch { /* localStorage unavailable */ }
  return DEFAULT_THEME;
}

/** Theme hook: reads/writes localStorage + sets `data-theme` on document root.
 *  The CSS uses `[data-theme="..."]` blocks to swap CSS variables. */
export function useTheme() {
  const [theme, setThemeState] = useState<ThemeName>(() => readStored());

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    try { localStorage.setItem(STORAGE_KEY, theme); } catch { /* ignore */ }
  }, [theme]);

  const setTheme = useCallback((t: ThemeName) => setThemeState(t), []);
  return { theme, setTheme };
}
```

- [ ] **Step 3: ThemePicker.tsx**

Create `assets/src/components/ThemePicker.tsx`:
```tsx
import { THEMES, type ThemeName } from '../themes';

interface Props {
  current: ThemeName;
  onPick: (t: ThemeName) => void;
  onClose: () => void;
}

/** Modal theme picker. Lists THEMES, highlights the current one, calls
 *  onPick when a theme is selected. Close: Esc, backdrop, ✕. */
export function ThemePicker({ current, onPick, onClose }: Props) {
  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') { e.preventDefault(); onClose(); }
  };

  return (
    <div className="modal-overlay" data-testid="theme-picker" onClick={onClose}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()} onKeyDown={onKey} tabIndex={-1}>
        <header className="modal-header">
          <span className="modal-title">select theme</span>
          <button className="modal-close" onClick={onClose} aria-label="close">✕</button>
        </header>
        <div className="modal-body">
          <ul className="theme-list">
            {THEMES.map((t) => (
              <li key={t.name} className={`theme-item${t.name === current ? ' theme-item-active' : ''}`}>
                <button onClick={() => onPick(t.name)} className="theme-item-button">
                  <span className="theme-item-name">{t.label}</span>
                  <span className="theme-item-desc">{t.description}</span>
                </button>
              </li>
            ))}
          </ul>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: ThemePicker.test.tsx**

Create `assets/src/components/__tests__/ThemePicker.test.tsx`:
```tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ThemePicker } from '../ThemePicker';

describe('ThemePicker', () => {
  it('lists all themes and highlights current', () => {
    render(<ThemePicker current="dark" onPick={vi.fn()} onClose={vi.fn()} />);
    expect(screen.getByText('Light')).toBeInTheDocument();
    expect(screen.getByText('Dark')).toBeInTheDocument();
    expect(screen.getByText('High contrast')).toBeInTheDocument();
    expect(screen.getByText('Dark').closest('.theme-item')).toHaveClass('theme-item-active');
  });

  it('picking a theme calls onPick + onClose (via caller)', () => {
    const onPick = vi.fn();
    const onClose = vi.fn();
    render(<ThemePicker current="light" onPick={onPick} onClose={onClose} />);
    fireEvent.click(screen.getByText('Dark'));
    expect(onPick).toHaveBeenCalledWith('dark');
  });

  it('Esc closes', () => {
    const onClose = vi.fn();
    render(<ThemePicker current="light" onPick={vi.fn()} onClose={onClose} />);
    fireEvent.keyDown(screen.getByTestId('theme-picker').querySelector('.modal-content')!, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });

  it('backdrop click closes', () => {
    const onClose = vi.fn();
    const { container } = render(<ThemePicker current="light" onPick={vi.fn()} onClose={onClose} />);
    fireEvent.click(container.querySelector('.modal-overlay')!);
    expect(onClose).toHaveBeenCalled();
  });
});
```

- [ ] **Step 5: styles.css — 把硬编码颜色抽成 CSS variables**

这是本 task 最重的部分 —— **不重写整个 styles.css**,只把最影响视觉的根变量抽出来(`--bg` / `--fg` / `--border` / `--accent` 等),在 styles.css **开头**加 `[data-theme]` 块定义,然后把现有规则里**少数高 visibility 的颜色**改成 `var(--xxx)`。其余硬编码颜色保留(子项目 4 不做完整主题化,只做"能切 light/dark 主色调")。

在 `assets/src/styles.css` **最顶部**插入:
```css
:root, [data-theme="light"] {
  --bg: #fafafa;
  --fg: #1a1a1a;
  --border: #ddd;
  --accent: #4a90e2;
  --accent-bg: #e8f0fe;
  --muted: #888;
  --cell-bg: #fff;
  --hover-bg: #f0f0f0;
}
[data-theme="dark"] {
  --bg: #1a1a1a;
  --fg: #e0e0e0;
  --border: #333;
  --accent: #5b9bf5;
  --accent-bg: #2a3a4f;
  --muted: #888;
  --cell-bg: #252525;
  --hover-bg: #2f2f2f;
}
[data-theme="high-contrast"] {
  --bg: #000;
  --fg: #fff;
  --border: #fff;
  --accent: #ffff00;
  --accent-bg: #333;
  --muted: #ccc;
  --cell-bg: #111;
  --hover-bg: #222;
}

/* 最影响视觉的根 rules 改用 var() */
html, body, #root { background: var(--bg); color: var(--fg); }
body { background: var(--bg); color: var(--fg); }
.app-sidebar { border-left-color: var(--border); background: var(--cell-bg); }
.cell { background: var(--cell-bg); border-color: var(--border); }
.cell-role, .session-item-meta, .agent-status { color: var(--muted); }
.session-item-active .session-item-button { background: var(--accent-bg); }
.composer { background: var(--cell-bg); border-top-color: var(--border); }
```

> **注:** 不要试图把整个 styles.css 全 var 化(那是子项目 5+ 的工作)。只改上面这些最影响"切主题后第一眼感受"的规则。

- [ ] **Step 6: styles.css 加 theme-list 样式**

```css
.theme-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 4px; }
.theme-item-button { width: 100%; text-align: left; background: none; border: 1px solid var(--border); cursor: pointer; padding: 8px; border-radius: 4px; display: flex; flex-direction: column; gap: 2px; }
.theme-item-button:hover { background: var(--hover-bg); }
.theme-item-active .theme-item-button { border-color: var(--accent); background: var(--accent-bg); }
.theme-item-name { font-size: 14px; }
.theme-item-desc { font-size: 11px; color: var(--muted); }
```

- [ ] **Step 7: 验证 + commit**

```bash
cd /Users/def/lemurclaw/codex-rs/lemurclaw-gui/assets
npm test -- ThemePicker
npx tsc --noEmit
cd /Users/def/lemurclaw
git add codex-rs/lemurclaw-gui/assets/src/themes.ts \
        codex-rs/lemurclaw-gui/assets/src/hooks/useTheme.ts \
        codex-rs/lemurclaw-gui/assets/src/components/ThemePicker.tsx \
        codex-rs/lemurclaw-gui/assets/src/components/__tests__/ThemePicker.test.tsx \
        codex-rs/lemurclaw-gui/assets/src/styles.css
git commit -m "feat(gui): ThemePicker (localStorage + CSS variables, light/dark/high-contrast)"
```

---

## Task 4.9:App 装配(modal host + onboarding gate + Ctrl+T + 顶栏)

**目标:** 把所有子项目 4 组件装配到 App:Onboarding gate 包主内容 + 顶栏(目录/模型/主题/菜单按钮)+ Ctrl+T 开 TranscriptPager + modal host(ThemePicker / ModelPicker)+ 注册 response handler(Task 4.1)。

**Files:**
- Modify: `assets/src/app/App.tsx`(主要改动)
- Modify: `assets/src/app/useConversation.ts`(加 registerResponseHandler 调用)
- Create: `assets/src/components/TopBar.tsx`
- Modify: `assets/src/styles.css`(加 `.app-topbar` 样式)

- [ ] **Step 1: TopBar.tsx**

Create `assets/src/components/TopBar.tsx`:
```tsx
interface Props {
  cwd: string | null;
  model: string | null;
  onOpenModelPicker: () => void;
  onOpenThemePicker: () => void;
  onOpenTranscript: () => void;
}

/** Top bar (spec §4.3 "顶栏 目录+模型+菜单"). Shows cwd + current model +
 *  buttons for model picker, theme picker, transcript pager. */
export function TopBar({ cwd, model, onOpenModelPicker, onOpenThemePicker, onOpenTranscript }: Props) {
  return (
    <header className="app-topbar" data-testid="topbar">
      <span className="topbar-cwd">{cwd ?? '(no cwd)'}</span>
      <button className="topbar-button" onClick={onOpenModelPicker}>
        <span className="topbar-model">{model ?? '(no model)'}</span> ⏷
      </button>
      <div className="topbar-spacer" />
      <button className="topbar-icon-button" onClick={onOpenTranscript} aria-label="transcript" title="transcript (Ctrl+T)">
        📜
      </button>
      <button className="topbar-icon-button" onClick={onOpenThemePicker} aria-label="theme" title="theme">
        🎨
      </button>
    </header>
  );
}
```

- [ ] **Step 2: useConversation.ts — 注册 response handler**

Modify `assets/src/app/useConversation.ts` 的 useEffect 加一行 `registerResponseHandler()`:
```ts
import { onEvent, send, registerResponseHandler } from '../transport';

// 在既有 useEffect 里:
useEffect(() => {
  registerResponseHandler();  // NEW: 为 sendRequest 装响应路由
  onEvent((ev) => { /* 既有逻辑 */ });
}, []);
```

- [ ] **Step 3: App.tsx 全装配**

Modify `assets/src/app/App.tsx`:
```tsx
import { useEffect, useState } from 'react';
import { useConversation } from './useConversation';
import { useTheme } from '../hooks/useTheme';
import { Scrollback } from '../components/Scrollback';
import { Composer } from '../components/Composer';
import { ApprovalCard } from '../components/ApprovalCard';
import { Sidebar } from '../components/Sidebar';
import { SessionPicker } from '../components/sidebar/SessionPicker';
import { AgentPanel } from '../components/sidebar/AgentPanel';
import { TopBar } from '../components/TopBar';
import { Onboarding } from '../components/Onboarding';
import { TranscriptPager } from '../components/TranscriptPager';
import { ModelPicker } from '../components/ModelPicker';
import { ThemePicker } from '../components/ThemePicker';

type ModalKind = 'none' | 'model' | 'theme' | 'transcript';

export function App() {
  const { state, threadId, interrupt } = useConversation();
  const { theme, setTheme } = useTheme();
  const [modal, setModal] = useState<ModalKind>('none');
  const turnActive = state.activeTurnId !== null;

  // Esc closes any open modal (modals also handle Esc themselves, this is a
  // safety net for focus issues).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && modal !== 'none') {
        setModal('none');
      }
      // Ctrl+T (or Cmd+T on mac) opens transcript pager.
      // Note: Cmd+T is browser-new-tab in many browsers — for the wry
      // webview context there's no browser chrome, so this is safe. If we
      // ever run in a real browser (webui mode), revisit.
      if ((e.ctrlKey || e.metaKey) && e.key === 't') {
        e.preventDefault();
        setModal('transcript');
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [modal]);

  return (
    <Onboarding>
      <div className="app-root">
        <TopBar
          cwd={null /* TODO: surface thread.cwd — needs reducer extension to capture from thread/started */}
          model={null /* TODO: surface current model — needs reducer extension */}
          onOpenModelPicker={() => setModal('model')}
          onOpenThemePicker={() => setModal('theme')}
          onOpenTranscript={() => setModal('transcript')}
        />
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
        <Sidebar
          sections={[
            { key: 'sessions', title: 'Sessions', body: <SessionPicker activeThreadId={threadId} /> },
            { key: 'agents', title: 'Agent', body: <AgentPanel state={state} /> },
          ]}
        />
      </div>

      {modal === 'transcript' && threadId && (
        <TranscriptPager threadId={threadId} onClose={() => setModal('none')} />
      )}
      {modal === 'model' && (
        <ModelPicker threadId={threadId} onClose={() => setModal('none')} />
      )}
      {modal === 'theme' && (
        <ThemePicker current={theme} onPick={(t) => { setTheme(t); }} onClose={() => setModal('none')} />
      )}
    </Onboarding>
  );
}
```

> **实现注:**
> 1. `cwd` 和 `model` 在 TopBar 里先留 null(TODO 注释)。ConversationState 目前没存 thread.cwd / currentModel —— 这是 Task 4.9 的 known limitation,真实数据需要扩 reducer(Task 3.4)抓 `thread/started.params.thread.cwd`。可作为 Task 4.9 的可选 follow-up 或留子项目 5。
> 2. TranscriptPager 在 modal 里需要 `threadId`,所以只在 `threadId !== null` 时渲染。否则点 Ctrl+T 不弹(或者弹一个"please start a conversation first"提示 —— 简化起见不弹)。
> 3. Ctrl+T 用 `e.ctrlKey || e.metaKey`(mac 用 Cmd)。wry webview 里没浏览器 chrome,Cmd+T 不会开新 tab,安全。webui 模式(子项目 6)要 revisit。
> 4. Onboarding 包整个 app-root,未登录时 gate 住,登录后显示。

- [ ] **Step 4: styles.css 加 topbar 样式**

Append to `assets/src/styles.css`:
```css
.app-root { display: flex; flex-direction: column; height: 100vh; }
.app-topbar { display: flex; align-items: center; gap: 8px; padding: 6px 12px; border-bottom: 1px solid var(--border); background: var(--cell-bg); flex-shrink: 0; }
.app-topbar ~ .app-main { flex: 1; display: flex; min-height: 0; }  /* 在 topbar 下面 */
/* 重写既有 .app-root flex:之前是 row(main + sidebar),现在改成 column(topbar + row) */
.app-root > .app-main { flex: 1; display: flex; min-height: 0; }

.topbar-cwd { font-size: 12px; color: var(--muted); font-family: 'SF Mono', Menlo, monospace; }
.topbar-button { background: none; border: 1px solid var(--border); padding: 4px 8px; border-radius: 4px; cursor: pointer; font-size: 12px; color: var(--fg); }
.topbar-model { font-weight: 500; }
.topbar-spacer { flex: 1; }
.topbar-icon-button { background: none; border: none; cursor: pointer; font-size: 16px; padding: 4px; }
```

> **关键 layout 调整:** 既有 `.app-root` 是 `display: flex; height: 100vh` 给 main + sidebar 横排。现在加了 topbar,改成 column:topbar 在顶,下面是 main+sidebar 的 row。这要改既有 `.app-root` 规则的 `flex-direction`,加 topbar 之后整体变 column。
>
> **测试影响:** 既有 Scrollback 测试用 `render(<Scrollback state={...} />)` 不涉及 App 的 layout,不受影响。Composer 测试同理。App 本身没测试(子项目 3 没写 App 测试)。如果 layout 改动让某个既有测试 fail,补一个 layout 烟测或调整 CSS selector。

- [ ] **Step 5: 验证全栈**

```bash
cd /Users/def/lemurclaw/codex-rs/lemurclaw-gui/assets
npm test                    # 全部测试应该通过(子项目 3 + 4 累计约 60+)
npx tsc --noEmit            # 类型检查
npm run build               # 生产构建
ls dist/                    # 确认 dist/index.html + assets/*.js + *.css 生成
cd /Users/def/lemurclaw/codex-rs
cargo check -p lemurclaw-gui
cargo clippy -p lemurclaw-gui
cargo fmt -p lemurclaw-gui
```

- [ ] **Step 6: 端到端手动验证(用户执行,沙箱无 display)**

```bash
cd /Users/def/lemurclaw/codex-rs
cargo run -p lemurclaw -- --frontend gui
```
验证:
1. 首启动:Onboarding 显示(如果未 authed),点 "continue anyway" 进主界面
2. 主界面:顶栏 + Scrollback + 侧栏(SessionPicker 列历史会话)+ Composer
3. 点顶栏 📜 或 Ctrl+T:TranscriptPager 全屏弹出,Esc 关
4. 点顶栏模型按钮:ModelPicker 弹出,选中后发 turn/start(下个 turn 用新模型)
5. 点顶栏 🎨:ThemePicker 弹出,切 dark/high-contrast 立即生效
6. 侧栏 Sessions:点历史会话发 thread/resume,Scrollback 切换到该会话

- [ ] **Step 7: Commit + 子项目 4 完成记录**

```bash
cd /Users/def/lemurclaw
git add codex-rs/lemurclaw-gui/assets/src/components/TopBar.tsx \
        codex-rs/lemurclaw-gui/assets/src/app/App.tsx \
        codex-rs/lemurclaw-gui/assets/src/app/useConversation.ts \
        codex-rs/lemurclaw-gui/assets/src/styles.css
git commit -m "feat(gui): assemble TopBar + modal host + onboarding gate + Ctrl+T (subproject 4 complete)"
```

---

## 子项目 4 完成标准

- [ ] `npm test` 全绿(约 60+ vitest,子项目 3 的 40 + 子项目 4 新增 ~20)
- [ ] `npx tsc --noEmit` 0 errors
- [ ] `cargo check -p lemurclaw-gui` + `cargo clippy -p lemurclaw-gui` 通过
- [ ] `npm run build` 产出 dist/
- [ ] **功能等价 codex TUI 导航类 surface:**
  - [ ] SessionPicker 侧栏常驻(thread/list + thread/resume + 当前高亮 + 分页)
  - [ ] TranscriptPager 全屏(thread/read includeTurns + Esc 关 + Ctrl+T 快捷键)
  - [ ] ModelPicker 模态(model/list + 选中发 turn/start override)
  - [ ] AgentPanel 侧栏(main agent status,sub-agent 占位)
  - [ ] Onboarding(getAuthStatus + welcome + CLI login 引导 + continue anyway 跳过)
  - [ ] ThemePicker(localStorage + CSS variables + light/dark/high-contrast)
  - [ ] 顶栏布局(cwd + 模型按钮 + transcript/theme icon)
  - [ ] typed request 通道(sendRequest + onResponse id 匹配)

---

## 后续(子项目 5+)

- SettingsModal 系列(permissions/keymap/memories/skills/hooks/mcp/apps/plugins/experimental/statusline/title)
- SlashPopup / MentionPopup / FileSearchPopup 的 fuzzy 交互
- 完整 trust 目录管理 / GUI 内 OAuth 流
- AgentPicker 真实多 agent 控制(spawn/message/interrupt)— 需要先扩 reducer 抓 collab/subAgent items
- TopBar 的 cwd/model 字段从 ConversationState 真实填充(需扩 Task 3.4 reducer)
- webui 模式(子项目 6)
- 发布就绪(子项目 7)

---

## 实现备注(给执行者的注意事项)

1. **typed request 通道(Task 4.1)是核心** —— 整个子项目 4 的 picker 都靠 `sendRequest` 等待 codex 的 JSON-RPC response。Task 4.1 必须先做,且必须可靠(30s 超时 + id 匹配 + reject 路径)。task 4.1 的 transport.test.ts 是关键回归保护。
2. **backend.rs 改动(Task 4.1)** —— `handle_ipc` 当前在 ClientRequest 分支只 await `handle.request(req).await` 然后丢弃返回。改成把返回值序列化推回 JS。注意 `value.get("id")` 要在 `from_value::<ClientRequest>` **之前**取(因为 enum 化之后取 id 麻烦)。
3. **ThreadListResponse 的 nextCursor + backwardsCursor** —— `thread/list` 返回 `nextCursor`(向前翻页用)+ `backwardsCursor`(向后翻页,反向 sort 用)。本计划只用 nextCursor(forward 分页),backwardsCursor 忽略。如果后续要双向翻页,扩 useThreadList。
4. **Thread.recencyAt / Thread.updatedAt 是 Unix 秒不是毫秒** —— `new Date(seconds * 1000)`。SessionPicker 已正确处理。
5. **ModelPicker 的"切模型"用 turn/start override,不是 thread/metadata/update** —— codex 协议下,切换运行时 model 靠下个 turn 的 override。`thread/metadata/update` 只改 metadata,不影响 model。这是 known limitation,文档化即可。
6. **Onboarding 的 getAuthStatus 返回 shape** —— Task 4.7 Step 1 先 cat 文件核实。代码用了双 shape 兼容兜底,实际接入时简化。
7. **ThemePicker 不全主题化 styles.css** —— Task 4.8 Step 5 只把最影响视觉的根变量抽出来(`--bg/--fg/--border/--accent/--cell-bg`)。把整个 styles.css 全 var 化是子项目 5+ 的工作。这样切主题后第一眼能看出来差别,但不追求像素级完美。
8. **App.tsx layout 调整(Task 4.9 Step 4)** —— 加 topbar 后 `.app-root` 从 row 变 column(topbar 在顶,下面 main+sidebar)。注意既有 Scrollback/Composer/cell 测试不涉及 App layout,应该不受影响。如果有 layout 相关测试 fail,补烟测。
9. **Task 4.4 的 TranscriptPager FlatScrollback** —— 起步用 `JSON.stringify(cell)` 渲染最小可用。真正的 cell 组件复用是 Step 2 可选优化(export Scrollback 的 CellRenderer)。建议做 Step 2,体验差距大。
10. **AGENTS.md 800 行变更 guideline** —— 单 task < 300 行,9 个 task 总计约 1500-2000 行(含测试)。分 2-3 个 PR:Task 4.1-4.3 / 4.4-4.6 / 4.7-4.9。

---

## 执行交接

计划已就绪。建议执行方式:**superpowers:subagent-driven-development**(跟子项目 3 一致)。每个 Task 一个新 subagent + 两阶段 review(spec compliance + code quality)。Task 4.1 是基础设施(typed request 通道),必须先做且做对 —— 优先级最高,可能需要更强大的模型(opus 或最强可用)。其余 task(4.2-4.9)大多机械,标准模型即可。
