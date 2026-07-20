# lemurclaw 子项目 4 Follow-ups 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把子项目 4 final review 标记为 deferred 的 4 项收尾:TopBar 的 cwd + model 从 ConversationState 真实填充(替代当前的 null stub)、AgentPanel 从占位提示升级为真实 sub-agent 数据展示、styles.css 剩余硬编码颜色完整 var 化。完成后子项目 4 干净收尾,无遗留 TODO。

**Architecture:**
- **#1 (cwd + model):** ConversationState 加 `cwd: string | null` + `currentModel: string | null`。两条数据源:(a) reducer 从 `thread/started.params.thread.cwd` 抓 cwd、从 `model/rerouted.params.toModel` 抓 currentModel;(b) `turn/start` / `thread/resume` 的 JSON-RPC response(`ThreadStartResponse` / `ThreadResumeResponse` 都权威携带 `model` + `cwd`)—— 把这两个请求从 fire-and-forget `send` 升级为 `sendRequest`,在 useConversation 里捕获 response 喂回 reducer(经一个合成 `responseMeta` action)。
- **#2 (sub-agent):** ConversationState 加 `subAgents: SubAgentModel[]`。reducer 扫描 ThreadItem 把 `collabAgentToolCall.agentsStates` 提到有结构的数据。AgentPanel 渲染真实行替代 deferral hint。
- **#3 (CSS var-ification):** styles.css 剩余 ~50 处硬编码颜色分类处理 —— 中性色用既有变量;状态/类型色抽成新的 per-theme 变量(`--ok`/`--err`/`--warn`/`--exec-accent` 等)。

**Tech Stack:** React 18.3 + TypeScript 5.6 + Vite 5.4 + vitest 2.x + 既有 codex ts-rs 类型 + 子项目 4 的 ViewModel/reducer/transport/sendRequest 基础设施。

**Spec:** `docs/superpowers/specs/2026-07-18-lemurclaw-codex-gui-design.md`(§4.2 + §4.3 + §6.1)
**前置:** 子项目 4 完成(`docs/superpowers/plans/2026-07-19-lemurclaw-navigation-views.md` 9 个 task + 最终 review)。当前在 `main` 分支 `f64786993` 或更新。

---

## 范围说明

本计划做 **子项目 4 follow-ups**(非 spec §6.1 验收项,是 final review 发现的 polish):
- ✅ TopBar cwd 真实显示(从 `thread/started` + `turn/start`/`thread/resume` response 抓)
- ✅ TopBar model 真实显示(同上 + `model/rerouted` 同步)
- ✅ AgentPanel 真实 sub-agent 数据(collabAgentToolCall.agentsStates)
- ✅ styles.css 完整 var 化(所有硬编码颜色 → 主题变量)

**不做**(留给子项目 5+):
- ❌ SettingsModal 系列 / Composer popups / 杂项 surface
- ❌ TopBar 的"菜单"按钮下拉(只有 cwd + model 显示 + 既有 icon 按钮)
- ❌ AgentPanel 的 spawn/control UI(只读展示)
- ❌ webui 模式

**变更规模控制:** 5 个 task,每个独立 commit,单任务 < 300 行。总计 ~350 行。直接 main 上 5 个 commit(跟子项目 3/4 一致)。

---

## 关键事实(已核实,执行者无需重复调研)

1. **`Thread.cwd: AbsolutePathBuf`** — `types/v2/Thread.ts:50`。ts-rs 把 `AbsolutePathBuf` 序列化为 `string`(`types/AbsolutePathBuf.ts` 是 `string` alias),所以 `thread.cwd` 直接是 string,无需解包。
2. **`ThreadStartResponse` / `ThreadResumeResponse`** — 都有 `model: string` + `cwd: AbsolutePathBuf` 字段(已 `cat` 核实)。这是 ClientRequest 的 JSON-RPC response,**不进 reducer**(进 sendRequest 的 promise)。Task 5.2 把它接到 state。
3. **`ModelReroutedNotification`** — `{ threadId, turnId, fromModel, toModel, reason }`。ServerNotification 推送,reducer 可直接消费,让 currentModel 跟随后端 reroute。
4. **`ThreadItem.collabAgentToolCall`** — `{ id, tool, status, senderThreadId, receiverThreadIds[], prompt, model, reasoningEffort, agentsStates: { [threadId]: CollabAgentState } }`。`agentsStates` 是 sub-agent 当前状态 map —— **AgentPanel 行的主数据源**。
5. **`CollabAgentState`** — `{ status: CollabAgentStatus, message: string | null }`。`CollabAgentStatus` 是枚举(运行/完成/失败等),本计划里 `String(state.status)` 化给显示用。
6. **既有 reducer 签名** — `reducer(state, event: unknown)`,内部用 type guards narrow。所以新加的合成 action 不需要改 reducer 签名,只需加一个 guard。
7. **Vitest 2.1.9 规则** — 所有 transport-mocked 测试用 `afterEach`(NOT `beforeEach`)做 `mockReset`(子项目 4 Task 4.2 教训)。
8. **`sendRequest<T>(method, params): Promise<T>`** — 子项目 4 Task 4.1 已实现,从 `../transport` 导入。

---

## 文件结构

| 文件 | 责任 | 任务 |
|---|---|---|
| `assets/src/viewModel/types.ts` | ConversationState 加 `cwd` / `currentModel` / `subAgents` 字段;新增 `SubAgentModel` + `ResponseMetaAction` 类型 | 5.1, 5.2, 5.3 |
| `assets/src/viewModel/reducer.ts` | 消费 `model/rerouted` + 合成 `responseMeta`;扫描 collab items 填 `subAgents` | 5.1, 5.2, 5.3 |
| `assets/src/viewModel/reducer.test.ts` | 加 cwd/model/responseMeta/subAgent case 测试 | 5.1, 5.2, 5.3 |
| `assets/src/app/useConversation.ts` | `turn/start` / `thread/resume` 升级为 sendRequest,捕 response 喂 reducer;暴露 `startTurn` + `resumeThread` | 5.2 |
| `assets/src/components/Composer.tsx` | turn/start 改调 useConversation.startTurn(替代直接 send) | 5.2 |
| `assets/src/components/sidebar/SessionPicker.tsx` | thread/resume 改调 useConversation.resumeThread | 5.2 |
| `assets/src/components/ModelPicker.tsx` | turn/start 带 model override 改调 startTurn | 5.2 |
| `assets/src/components/sidebar/AgentPanel.tsx` | 渲染真实 sub-agent 行替代 deferral hint | 5.3 |
| `assets/src/components/sidebar/__tests__/AgentPanel.test.tsx` | sub-agent 行测试 | 5.3 |
| `assets/src/app/App.tsx` | TopBar 传 `state.cwd` / `state.currentModel`;Composer/SessionPicker/ModelPicker 传 startTurn/resumeThread | 5.2, 5.4 |
| `assets/src/app/__tests__/useConversation.test.ts` | 加 startTurn/resumeThread/responseMeta 测试 | 5.2 |
| `assets/src/styles.css` | 完整 var 化 + 新增状态/类型色变量 | 5.5 |

---

## Task 5.1:reducer 抓 cwd + model(thread/started + model/rerouted)

**目标:** ConversationState 持有 cwd + currentModel,由 reducer 从既有 ServerNotification 填充。**本 task 只做 reducer 层**(发送方升级在 Task 5.2)。`thread/started.params.thread.cwd` 是日常来源;`model/rerouted` 让 currentModel 跟随后端 reroute。

**Files:**
- Modify: `assets/src/viewModel/types.ts`(~15 行)
- Modify: `assets/src/viewModel/reducer.ts`(~25 行)
- Modify: `assets/src/viewModel/reducer.test.ts`(~40 行)

- [ ] **Step 1: types.ts —— ConversationState 加 cwd + currentModel 字段**

Modify `assets/src/viewModel/types.ts`,在 `ConversationState` interface 里(在 `status` 之后、`pendingApprovals` 之前)加:

```ts
  /** Working directory of the active thread, from thread/started's thread.cwd
   *  (and turn/start / thread/resume responses — Task 5.2). Null pre-thread/started.
   *  Displayed in TopBar. */
  cwd: string | null;
  /** Current model id for the active thread. Set from model/rerouted's toModel
   *  (and from the turn/start / thread/resume response — Task 5.2). Null until
   *  the first reroute or a sendRequest-backed start. */
  currentModel: string | null;
```

更新 `initialState`:
```ts
export const initialState: ConversationState = {
  turns: [],
  activeTurnId: null,
  status: null,
  cwd: null,
  currentModel: null,
  pendingApprovals: [],
};
```

- [ ] **Step 2: reducer.ts —— thread/started 抓 cwd,model/rerouted 抓 currentModel**

Modify `assets/src/viewModel/reducer.ts` 的 `applyNotification` switch。

`thread/started` 分支(当前是 `return { ...state, status: n.params.thread.status, activeTurnId: null };`)改为:
```ts
    case 'thread/started': {
      const thread = n.params.thread;
      const cwdRaw = thread.cwd as unknown;
      return {
        ...state,
        status: thread.status,
        activeTurnId: null,
        cwd: typeof cwdRaw === 'string' ? cwdRaw : String(cwdRaw ?? ''),
      };
    }
```

新增 `model/rerouted` 分支(建议在 `serverRequest/resolved` 之后):
```ts
    case 'model/rerouted':
      // Backend rerouted the model (e.g. chosen model unavailable → fallback).
      // toModel is the now-current model id; fromModel is informational only.
      return { ...state, currentModel: n.params.toModel };
```

> **实现注:** `thread.cwd` 兜底用 `typeof === 'string' ? : String(...)` —— ts-rs 标 string,但 AbsolutePathBuf 偶尔在不同 codex 版本里形态不同,双路径兜底防御。

- [ ] **Step 3: reducer.test.ts —— 加 cwd + model/rerouted 测试**

在既有 `describe('reducer')` 块里(贴在 `turn/completed` 测试之后)加:

```ts
  it('thread/started captures cwd from thread.cwd', () => {
    const thread = { ...FULL_THREAD, cwd: '/home/user/proj' } as never;
    const next = reducer(st(), { method: 'thread/started', params: { thread } });
    expect(next.cwd).toBe('/home/user/proj');
  });

  it('model/rerouted sets currentModel to toModel', () => {
    const afterTurn = reducer(st(), { method: 'thread/started', params: { thread: FULL_THREAD } });
    const next = reducer(afterTurn, {
      method: 'model/rerouted',
      params: { threadId: 't1', turnId: 'tu1', fromModel: 'gpt-4', toModel: 'gpt-4o', reason: 'unavailable' },
    });
    expect(next.currentModel).toBe('gpt-4o');
  });
```

> **注:** `FULL_THREAD` / `EMPTY_TURN` 已在既有测试文件定义。如果 `reason: 'unavailable'` 类型报错(`ModelRerouteReason` 枚举),`cat types/v2/ModelRerouteReason.ts` 核实合法值后调整。

- [ ] **Step 4: 验证**

```bash
cd /Users/def/lemurclaw/codex-rs/lemurclaw-gui/assets
npm test -- reducer
npx tsc --noEmit
```
Expected: 既有 reducer test + 2 个新 case 全过;tsc 0 errors。

- [ ] **Step 5: Commit**

```bash
cd /Users/def/lemurclaw
git add codex-rs/lemurclaw-gui/assets/src/viewModel/types.ts \
        codex-rs/lemurclaw-gui/assets/src/viewModel/reducer.ts \
        codex-rs/lemurclaw-gui/assets/src/viewModel/reducer.test.ts
git commit -m "feat(gui): reducer captures cwd + currentModel (follow-up #1)"
```

---

## Task 5.2:useConversation 用 sendRequest 发 turn/start + thread/resume,捕 response 喂 state

**目标:** useConversation 成为 turn/start + thread/resume 的发送方,用 sendRequest 拿 JSON-RPC response(`ThreadStartResponse` / `ThreadResumeResponse` 权威携带 `model` + `cwd`),经一个合成 `responseMeta` action 把 response 喂回 reducer。Composer / SessionPicker / ModelPicker 不再直接 `send`,改调 useConversation 暴露的 `startTurn` / `resumeThread`。

**Files:**
- Modify: `assets/src/viewModel/types.ts`(~10 行,加 `ResponseMetaAction`)
- Modify: `assets/src/viewModel/reducer.ts`(~20 行,加 `responseMeta` 分支)
- Modify: `assets/src/viewModel/reducer.test.ts`(~15 行)
- Modify: `assets/src/app/useConversation.ts`(~45 行)
- Modify: `assets/src/components/Composer.tsx`(~10 行)
- Modify: `assets/src/components/sidebar/SessionPicker.tsx`(~10 行)
- Modify: `assets/src/components/ModelPicker.tsx`(~10 行)
- Modify: `assets/src/app/App.tsx`(~5 行)
- Modify: `assets/src/app/__tests__/useConversation.test.ts`(~25 行)
- Modify: `assets/src/components/__tests__/Composer.test.tsx`、`ModelPicker.test.tsx`、`sidebar/__tests__/SessionPicker.test.tsx`(更新 mock/断言)

- [ ] **Step 1: types.ts —— ResponseMetaAction**

在 `assets/src/viewModel/types.ts` 文件末尾(`initialState` 之后)加:

```ts
/**
 * Synthetic action dispatched by useConversation when a turn/start or
 * thread/resume ClientRequest's JSON-RPC response arrives. The response
 * (ThreadStartResponse / ThreadResumeResponse) carries authoritative cwd +
 * model that the reducer can't get from ServerNotifications alone. This is
 * NOT a ServerNotification — it's a local dispatch used to fold response
 * data into ConversationState.
 */
export interface ResponseMetaAction {
  kind: 'responseMeta';
  cwd: string | null;
  model: string | null;
}
```

- [ ] **Step 2: reducer.ts —— 加 responseMeta 分支**

Modify `assets/src/viewModel/reducer.ts`。当前 `reducer` 函数末尾是:
```ts
  // Unknown event shape (e.g. backend's `{lagged, skipped}` envelope).
  return state;
}
```

在 `isServerRequest` 分支之后、unknown 注释之前插入:
```ts
  if (isResponseMetaAction(event)) {
    return applyResponseMeta(state, event);
  }
```

加 import(在既有 types import 那行追加 `ResponseMetaAction`):
```ts
import type { ConversationState, TurnModel, CellModel, PendingApproval, ApprovalKind, ResponseMetaAction } from './types';
```

在文件末尾(`applyHookEvent` 之后或任意 module-level fn 位置)加:
```ts
function isResponseMetaAction(x: unknown): x is ResponseMetaAction {
  if (typeof x !== 'object' || x === null) return false;
  return (x as { kind?: unknown }).kind === 'responseMeta';
}

function applyResponseMeta(state: ConversationState, action: ResponseMetaAction): ConversationState {
  // Only overwrite when the response carries a non-null value — lets later
  // responses update cwd/model without clobbering with nulls on partial fails.
  const next = { ...state };
  if (action.cwd !== null) next.cwd = action.cwd;
  if (action.model !== null) next.currentModel = action.model;
  return next;
}
```

- [ ] **Step 3: reducer.test.ts —— responseMeta 测试**

加 case:
```ts
  it('responseMeta action folds cwd + model into state', () => {
    const next = reducer(st(), { kind: 'responseMeta', cwd: '/proj', model: 'gpt-4o' });
    expect(next.cwd).toBe('/proj');
    expect(next.currentModel).toBe('gpt-4o');
  });

  it('responseMeta with null fields preserves existing cwd/model', () => {
    const after = reducer(st(), { kind: 'responseMeta', cwd: '/proj', model: 'gpt-4o' });
    const next = reducer(after, { kind: 'responseMeta', cwd: null, model: null });
    expect(next.cwd).toBe('/proj');  // not clobbered
    expect(next.currentModel).toBe('gpt-4o');
  });
```

- [ ] **Step 4: useConversation.ts —— 暴露 startTurn + resumeThread**

Modify `assets/src/app/useConversation.ts`。加 sendRequest import:
```ts
import { onEvent, send, registerResponseHandler, sendRequest } from '../transport';
```

在 `interrupt` 定义之后、`return` 之前,加两个 callback:
```ts
  const startTurn = useCallback(async (input: unknown[], modelOverride?: string): Promise<void> => {
    const threadId = threadIdRef.current;
    if (!threadId) return;
    const params: Record<string, unknown> = { threadId, input };
    if (modelOverride) params.model = modelOverride;
    try {
      const resp = await sendRequest<{ model: string; cwd: string }>('turn/start', params);
      dispatch({ kind: 'responseMeta', model: resp.model, cwd: resp.cwd });
    } catch (e) {
      console.error('useConversation.startTurn failed', e);
    }
  }, []);

  const resumeThread = useCallback(async (threadId: string): Promise<void> => {
    try {
      const resp = await sendRequest<{ model: string; cwd: string }>('thread/resume', { threadId });
      // Switch threadIdRef immediately so the composer + sidebar highlight
      // follow the switch before the response comes back.
      threadIdRef.current = threadId;
      dispatch({ kind: 'responseMeta', model: resp.model, cwd: resp.cwd });
    } catch (e) {
      console.error('useConversation.resumeThread failed', e);
    }
  }, []);

  return { state, threadId: threadIdRef.current, interrupt, startTurn, resumeThread };
```

> **注:** catch 里只 log,不阻塞。responseMeta 只在成功时 dispatch,失败时 cwd/model 保持上次值。

- [ ] **Step 5: Composer.tsx —— 改调 startTurn**

Modify `assets/src/components/Composer.tsx`。Props 加 `startTurn`:
```ts
interface Props {
  threadId: string | null;
  turnActive: boolean;
  onInterrupt: () => void;
  startTurn: (input: unknown[], modelOverride?: string) => Promise<void>;
}
```

`submit` 改为(删掉既有的 `send({ method: 'turn/start', ... })` 块):
```ts
  const submit = useCallback(() => {
    const trimmed = text.trim();
    if (!trimmed || !threadId) return;
    startTurn([{ type: 'text', text: trimmed, text_elements: [] }]);
    setText('');
  }, [text, threadId, startTurn]);
```

解构 props 加 `startTurn`,删掉不再用的 `send` import + `seqRef`(如果 seqRef 只用于 clientUserMessageId,现在省略了 —— `TurnStartParams.clientUserMessageId` 是可选)。

- [ ] **Step 6: SessionPicker.tsx —— 改调 resumeThread**

Modify `assets/src/components/sidebar/SessionPicker.tsx`。Props 加 `onResume`:
```ts
interface Props {
  activeThreadId: string | null;
  onSelect?: (thread: Thread) => void;
  onResume?: (threadId: string) => void;
}
```

`handleSelect` 改为:
```ts
  const handleSelect = (thread: Thread) => {
    if (onSelect) {
      onSelect(thread);
    } else if (onResume) {
      onResume(thread.id);
    }
  };
```

删掉 `send` import(如果不再用)。

- [ ] **Step 7: ModelPicker.tsx —— 改调 startTurn(带 model override)**

Modify `assets/src/components/ModelPicker.tsx`。Props 加 `startTurn`:
```ts
interface Props {
  threadId: string | null;
  currentModel?: string | null;
  onClose: () => void;
  startTurn: (input: unknown[], modelOverride?: string) => Promise<void>;
}
```

`handlePick` 改为:
```ts
  const handlePick = (model: Model) => {
    if (!threadId) return;
    // Switch model via next-turn override. Empty input = no new user message;
    // codex treats this as a no-op turn, model still takes effect next turn.
    startTurn([{ type: 'text', text: '', text_elements: [] }], model.id);
    onClose();
  };
```

删掉 `send` import(如果不再用)。

- [ ] **Step 8: App.tsx —— 接线**

Modify `assets/src/app/App.tsx`。useConversation 解构加 `startTurn, resumeThread`:
```ts
const { state, threadId, interrupt, startTurn, resumeThread } = useConversation();
```

传给组件:
```tsx
<Composer threadId={threadId} turnActive={turnActive} onInterrupt={interrupt} startTurn={startTurn} />
<SessionPicker activeThreadId={threadId} onResume={resumeThread} />
<ModelPicker threadId={threadId} onClose={() => setModal('none')} startTurn={startTurn} />
```

- [ ] **Step 9: useConversation.test.ts —— 加 response 路径测试**

Modify `assets/src/app/__tests__/useConversation.test.ts`。mock 加 `sendRequest`:
```ts
vi.mock('../../transport', () => ({
  onEvent: (cb: (ev: unknown) => void) => { onEventCb = cb; },
  send: vi.fn(),
  registerResponseHandler: vi.fn(),
  sendRequest: vi.fn(),
}));
import { send } from '../../transport';
import { sendRequest } from '../../transport';
```

加测试:
```ts
  it('startTurn dispatches responseMeta with cwd + model from response', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ model: 'gpt-4o', cwd: '/proj' } as never);
    const { result } = renderHook(() => useConversation());
    emit({ method: 'turn/started', params: { threadId: 't1', turn: { id: 'tu1', items: [] } } });
    await act(async () => {
      await result.current.startTurn([{ type: 'text', text: 'hi', text_elements: [] }]);
    });
    expect(result.current.state.cwd).toBe('/proj');
    expect(result.current.state.currentModel).toBe('gpt-4o');
  });

  it('resumeThread sets threadId and dispatches responseMeta', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ model: 'claude-3', cwd: '/other' } as never);
    const { result } = renderHook(() => useConversation());
    emit({ method: 'turn/started', params: { threadId: 't1', turn: { id: 'tu1', items: [] } } });
    await act(async () => { await result.current.resumeThread('t2'); });
    expect(result.current.threadId).toBe('t2');
    expect(result.current.state.cwd).toBe('/other');
    expect(result.current.state.currentModel).toBe('claude-3');
  });
```

- [ ] **Step 10: 更新 Composer/ModelPicker/SessionPicker 既有测试**

这 3 个测试之前 mock `send` 并断言 `send({ method: 'turn/start', ... })`。现在组件改调 prop(`startTurn`/`onResume`),所以:
- 测试里传 mock 函数(`startTurn={vi.fn()}` / `onResume={vi.fn()}`)
- 断言改成断言 mock 被调用,而不是断言 `send`

例:Composer.test.tsx 的 "Enter sends a turn/start" 测试,断言从 `expect(send).toHaveBeenCalledWith(expect.objectContaining({ method: 'turn/start', ... }))` 改成 `expect(startTurn).toHaveBeenCalledWith([{ type: 'text', text: 'hello', text_elements: [] }])`。

- [ ] **Step 11: 验证 + commit**

```bash
cd /Users/def/lemurclaw/codex-rs/lemurclaw-gui/assets
npm test
npx tsc --noEmit
cd /Users/def/lemurclaw
git add codex-rs/lemurclaw-gui/assets/src/viewModel/types.ts \
        codex-rs/lemurclaw-gui/assets/src/viewModel/reducer.ts \
        codex-rs/lemurclaw-gui/assets/src/viewModel/reducer.test.ts \
        codex-rs/lemurclaw-gui/assets/src/app/useConversation.ts \
        codex-rs/lemurclaw-gui/assets/src/app/__tests__/useConversation.test.ts \
        codex-rs/lemurclaw-gui/assets/src/components/Composer.tsx \
        codex-rs/lemurclaw-gui/assets/src/components/__tests__/Composer.test.tsx \
        codex-rs/lemurclaw-gui/assets/src/components/sidebar/SessionPicker.tsx \
        codex-rs/lemurclaw-gui/assets/src/components/sidebar/__tests__/SessionPicker.test.tsx \
        codex-rs/lemurclaw-gui/assets/src/components/ModelPicker.tsx \
        codex-rs/lemurclaw-gui/assets/src/components/__tests__/ModelPicker.test.tsx \
        codex-rs/lemurclaw-gui/assets/src/app/App.tsx
git commit -m "feat(gui): sendRequest-backed turn/start + thread/resume feed cwd/model into state (follow-up #1)"
```

> **关键:** Step 10 是本 task 最 fiddly 的部分 —— 3 个组件测试要更新。如果某个测试因 prop 形状改动难修,优先保证 `npm test` 全绿(测试可以简化但不能删行为覆盖)。

---

## Task 5.3:AgentPanel 真实 sub-agent 数据(collabAgentToolCall.agentsStates)

**目标:** ConversationState 加 `subAgents: SubAgentModel[]`,reducer 扫描 ThreadItem 把 `collabAgentToolCall.agentsStates` 提到有结构的数据。AgentPanel 渲染真实行替代 deferral hint。CellModel 里的 collab/subAgent 仍保留 `generic` fallback(给 Scrollback 一个最小 cell),AgentPanel 单独从 state.subAgents 读。

**Files:**
- Modify: `assets/src/viewModel/types.ts`(~20 行)
- Modify: `assets/src/viewModel/reducer.ts`(~35 行)
- Modify: `assets/src/viewModel/reducer.test.ts`(~35 行)
- Modify: `assets/src/components/sidebar/AgentPanel.tsx`(~35 行)
- Modify: `assets/src/components/sidebar/__tests__/AgentPanel.test.tsx`(~25 行)
- Modify: `assets/src/styles.css`(~8 行)

- [ ] **Step 1: types.ts —— SubAgentModel + state 字段**

在 `assets/src/viewModel/types.ts` 加(`ResponseMetaAction` 之后):
```ts
/** One row in the sidebar AgentPanel's sub-agent list. Derived from
 *  collabAgentToolCall.agentsStates (the per-agent status map). */
export interface SubAgentModel {
  /** Thread id of the sub-agent (key in agentsStates). */
  threadId: string;
  /** Status string from CollabAgentState.status (e.g. 'running', 'completed').
   *  Kept as string for display simplicity. */
  status: string;
  /** Optional message from the sub-agent's last state update. */
  message: string | null;
}
```

ConversationState 加字段(在 `currentModel` 之后、`pendingApprovals` 之前):
```ts
  /** Sub-agent rows for the sidebar AgentPanel. Derived from
   *  collabAgentToolCall items' agentsStates. Empty when no sub-agents
   *  observed in the active thread. */
  subAgents: SubAgentModel[];
```

`initialState` 加 `subAgents: []`(在 `pendingApprovals` 之前)。

- [ ] **Step 2: reducer.ts —— extractSubAgents + merge**

Modify `assets/src/viewModel/reducer.ts`。加 import:
```ts
import type { ConversationState, TurnModel, CellModel, PendingApproval, ApprovalKind, ResponseMetaAction, SubAgentModel } from './types';
```

加 module-level helper(任意位置,建议 `threadItemToCell` 附近):
```ts
/** Extract sub-agent rows from a turn's items' collabAgentToolCall.agentsStates.
 *  Later items override earlier (last write wins per threadId). */
function extractSubAgents(items: ThreadItem[]): SubAgentModel[] {
  const agents = new Map<string, SubAgentModel>();
  for (const item of items) {
    if (item.type !== 'collabAgentToolCall') continue;
    for (const [threadId, state] of Object.entries(item.agentsStates)) {
      if (!state) continue;
      agents.set(threadId, {
        threadId,
        status: String(state.status),
        message: state.message,
      });
    }
  }
  return Array.from(agents.values());
}

/** Merge by threadId — incoming overrides existing. */
function mergeSubAgents(existing: SubAgentModel[], incoming: SubAgentModel[]): SubAgentModel[] {
  const map = new Map(existing.map((a) => [a.threadId, a]));
  for (const a of incoming) map.set(a.threadId, a);
  return Array.from(map.values());
}
```

修改 `applyTurnStarted`、`applyTurnCompleted`、`applyItemEvent` 三个函数,在 return 前重算 subAgents。模板(以 `applyTurnStarted` 为例,其它两个同样模式):
```ts
function applyTurnStarted(state: ConversationState, turn: Turn): ConversationState {
  const turnModel: TurnModel = { /* 既有,不动 */ };
  const subAgents = extractSubAgents(turn.items);
  return {
    ...state,
    turns: [...state.turns, turnModel],
    activeTurnId: turn.id,
    subAgents: mergeSubAgents(state.subAgents, subAgents),
  };
}
```

`applyTurnCompleted` 同样(在 turns map 之后加 `extractSubAgents(turn.items)` + `mergeSubAgents`)。

`applyItemEvent` 在更新该 turn 的 items 之后,rescan 那个 turn:
```ts
function applyItemEvent(state: ConversationState, turnId: string, item: ThreadItem): ConversationState {
  const cell = threadItemToCell(item);
  const turns = state.turns.map((t) => {
    if (t.id !== turnId) return t;
    /* 既有 upsert item 逻辑,生成 updatedItems */
    return { ...t, items: updatedItems };
  });
  const affected = turns.find((t) => t.id === turnId);
  const newSubAgents = affected ? extractSubAgents(affected.items) : [];
  return { ...state, turns, subAgents: mergeSubAgents(state.subAgents, newSubAgents) };
}
```

- [ ] **Step 3: reducer.test.ts —— subAgent 提取测试**

加 3 个 case:
```ts
  it('collabAgentToolCall items populate state.subAgents from agentsStates', () => {
    const afterTurn = reducer(st(), { method: 'turn/started', params: {
      threadId: 't1',
      turn: {
        ...EMPTY_TURN,
        items: [{
          type: 'collabAgentToolCall', id: 'col1',
          tool: 'spawn' as never, status: 'running' as never,
          senderThreadId: 't1', receiverThreadIds: ['sub1'],
          prompt: null, model: null, reasoningEffort: null,
          agentsStates: { sub1: { status: 'running' as never, message: 'working' } },
        } as never],
      },
    } });
    expect(afterTurn.subAgents).toHaveLength(1);
    expect(afterTurn.subAgents[0].threadId).toBe('sub1');
    expect(afterTurn.subAgents[0].status).toBe('running');
    expect(afterTurn.subAgents[0].message).toBe('working');
  });

  it('item/started with a collabAgentToolCall updates subAgents', () => {
    const afterTurn = reducer(st(), { method: 'turn/started', params: { threadId: 't1', turn: EMPTY_TURN } });
    const afterItem = reducer(afterTurn, {
      method: 'item/started',
      params: {
        threadId: 't1', turnId: 'tu1', startedAtMs: 5,
        item: {
          type: 'collabAgentToolCall', id: 'col1',
          tool: 'spawn' as never, status: 'running' as never,
          senderThreadId: 't1', receiverThreadIds: ['sub1'],
          prompt: null, model: null, reasoningEffort: null,
          agentsStates: { sub1: { status: 'idle' as never, message: null } },
        } as never,
      },
    });
    expect(afterItem.subAgents).toHaveLength(1);
    expect(afterItem.subAgents[0].threadId).toBe('sub1');
  });

  it('multiple collabAgentToolCall items merge sub-agents by threadId (last wins)', () => {
    const afterTurn = reducer(st(), { method: 'turn/started', params: {
      threadId: 't1',
      turn: {
        ...EMPTY_TURN,
        items: [
          { type: 'collabAgentToolCall', id: 'c1', tool: 'spawn' as never, status: 'running' as never, senderThreadId: 't1', receiverThreadIds: ['a','b'], prompt: null, model: null, reasoningEffort: null, agentsStates: { a: { status: 'running' as never, message: null }, b: { status: 'idle' as never, message: null } } } as never,
          { type: 'collabAgentToolCall', id: 'c2', tool: 'spawn' as never, status: 'running' as never, senderThreadId: 't1', receiverThreadIds: ['b','c'], prompt: null, model: null, reasoningEffort: null, agentsStates: { b: { status: 'completed' as never, message: 'done' }, c: { status: 'running' as never, message: null } } } as never,
        ],
      },
    } });
    expect(afterTurn.subAgents.map((s) => s.threadId).sort()).toEqual(['a','b','c']);
    const b = afterTurn.subAgents.find((s) => s.threadId === 'b')!;
    expect(b.status).toBe('completed');
  });
```

- [ ] **Step 4: AgentPanel.tsx —— 渲染真实行**

替换 `assets/src/components/sidebar/AgentPanel.tsx` 全文:
```tsx
import type { ConversationState } from '../../viewModel/types';

interface Props {
  state: ConversationState;
}

/** Sidebar "Agent" section. Shows main agent status + one row per sub-agent
 *  observed in collabAgentToolCall items (via state.subAgents). Read-only —
 *  no spawn/control UI (deferred to a later subproject). */
export function AgentPanel({ state }: Props) {
  const mainStatus = state.status;
  const mainLabel =
    mainStatus === null
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
      {state.subAgents.length === 0 ? (
        <div className="agent-empty-hint">no sub-agents active</div>
      ) : (
        <ul className="agent-sub-list" data-testid="agent-sub-list">
          {state.subAgents.map((s) => (
            <li key={s.threadId} className={`agent-row agent-row-sub agent-row-sub-${s.status}`}>
              <span className="agent-name">{s.threadId}</span>
              <span className="agent-status">{s.status}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
```

- [ ] **Step 5: AgentPanel.test.tsx —— 更新测试**

Modify `assets/src/components/sidebar/__tests__/AgentPanel.test.tsx`。既有 4 个 case 保留(not-started / idle / active+flags / deferral-hint)。**deferral-hint case 改文案**:之前断言 `/sub-agent control deferred/`,现在断言 `/no sub-agents active/`(空状态文案改了)。加 2 个新 case:
```ts
  it('renders sub-agent rows from state.subAgents', () => {
    const state: ConversationState = {
      ...initialState,
      status: { type: 'idle' },
      subAgents: [
        { threadId: 'sub1', status: 'running', message: null },
        { threadId: 'sub2', status: 'completed', message: 'done' },
      ],
    };
    render(<AgentPanel state={state} />);
    expect(screen.getByText('sub1')).toBeInTheDocument();
    expect(screen.getByText('sub2')).toBeInTheDocument();
    expect(screen.getByText('running')).toBeInTheDocument();
    expect(screen.getByText('completed')).toBeInTheDocument();
  });

  it('shows "no sub-agents active" when subAgents is empty', () => {
    render(<AgentPanel state={initialState} />);
    expect(screen.getByText(/no sub-agents active/)).toBeInTheDocument();
  });
```

> **注:** 既有"shows deferral hint for sub-agents"测试要么删,要么改名 + 改文案匹配。建议改:测试名 "shows empty hint when no sub-agents",断言 `/no sub-agents active/`。

- [ ] **Step 6: styles.css —— sub-agent 行样式**

Append(在 AgentPanel 既有 `.agent-*` 规则之后):
```css
.agent-sub-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 2px; }
.agent-row-sub { background: var(--cell-bg); padding: 3px 8px; font-size: 11px; }
.agent-row-sub-running .agent-status { color: #2eb872; }  /* TODO Task 5.5: var(--ok) */
.agent-row-sub-completed .agent-status { color: #2eb872; }  /* TODO Task 5.5: var(--ok) */
.agent-row-sub-failed .agent-status { color: #d9534f; }  /* TODO Task 5.5: var(--err) */
```

> **注:** 本 task 先用硬编码色,Task 5.5(完整 var 化)再替换成变量。TODO 注释提醒。

- [ ] **Step 7: 验证 + commit**

```bash
cd /Users/def/lemurclaw/codex-rs/lemurclaw-gui/assets
npm test -- reducer AgentPanel
npx tsc --noEmit
cd /Users/def/lemurclaw
git add codex-rs/lemurclaw-gui/assets/src/viewModel/types.ts \
        codex-rs/lemurclaw-gui/assets/src/viewModel/reducer.ts \
        codex-rs/lemurclaw-gui/assets/src/viewModel/reducer.test.ts \
        codex-rs/lemurclaw-gui/assets/src/components/sidebar/AgentPanel.tsx \
        codex-rs/lemurclaw-gui/assets/src/components/sidebar/__tests__/AgentPanel.test.tsx \
        codex-rs/lemurclaw-gui/assets/src/styles.css
git commit -m "feat(gui): AgentPanel real sub-agent data from collabAgentToolCall (follow-up #2)"
```

---

## Task 5.4:TopBar 接 state.cwd + state.currentModel

**目标:** TopBar 不再传 null,从 state 读真实 cwd + currentModel。极小改动 —— TopBar 组件本身不改(Props 已经是 `string | null`),只改 App.tsx 传参。

**Files:**
- Modify: `assets/src/app/App.tsx`(~2 行)

- [ ] **Step 1: App.tsx —— TopBar 传真实 cwd/model**

Modify `assets/src/app/App.tsx`。当前:
```tsx
<TopBar
  cwd={null /* TODO: surface thread.cwd — needs reducer extension */}
  model={null /* TODO: surface current model — needs reducer extension */}
  ...
/>
```

改为:
```tsx
<TopBar
  cwd={state.cwd}
  model={state.currentModel}
  ...
/>
```

> **注:** Task 5.1 + 5.2 已让 state.cwd / state.currentModel 填充。如果用户从未发过 turn/start,currentModel 可能仍 null —— TopBar 显示 "(no model)",符合预期。

- [ ] **Step 2: 验证 + commit**

```bash
cd /Users/def/lemurclaw/codex-rs/lemurclaw-gui/assets
npm test                    # App integration tests 应该仍过
npx tsc --noEmit
cd /Users/def/lemurclaw
git add codex-rs/lemurclaw-gui/assets/src/app/App.tsx
git commit -m "feat(gui): TopBar shows real cwd + model from ConversationState (follow-up #1 wiring)"
```

---

## Task 5.5:styles.css 完整 var 化

**目标:** 把剩余 ~50 处硬编码颜色全 var 化。中性色用既有变量;状态/类型色抽成新的 per-theme 变量。让 dark/high-contrast 主题真正完整生效(目前只有 8 个根变量 var 化,切 dark 时大部分 cell 还是白底)。

**分类(基于 grep 调研,执行时按文件实际为准):**
- **中性色 → 既有变量:** `#fff`→`--cell-bg`,`#888`→`--muted`,`#ddd`→`--border`,`#f4f4f4`→新`--code-bg`,`#f8f8f8`→新`--cell-bg-alt`,`#1a1a1a`→`--fg`,`#ccc`→新`--border-strong`,`#eee`→新`--border-weak`,`#aaa`→新`--muted-weak`,`#666`/`#999`→新`--muted-strong`
- **状态色 → 新变量:** `#2eb872`→`--ok`,`#d9534f`→`--err`,`#b8a02e`→`--warn`,`#ffe082`→`--warn-border`,`#fff8e1`→`--warn-bg`
- **类型色 → 新变量:** `#6c757d`→`--exec-accent`,`#9b59b6`→`--patch-accent`,`#e67e22`→`--mcp-accent`,`#3498db`+`#f0f8ff`→`--plan-accent`+`--plan-bg`,`#95a5a6`→`--hook-accent`,`#f0fff4`→`--agent-final-bg`

**Files:**
- Modify: `assets/src/styles.css`(~80 行)

- [ ] **Step 1: 三个 theme block 各加新变量**

在 styles.css 顶部既有 `:root, [data-theme="light"]` / `[data-theme="dark"]` / `[data-theme="high-contrast"]` 三个 block 里,在既有 8 个变量之后**各加**以下变量(light 用原色值,dark/high-contrast 调亮/调饱和以保对比度)。light 版本:
```css
  /* Status + neutral variants + cell accents (Task 5.5) */
  --ok: #2eb872;
  --err: #d9534f;
  --warn: #b8a02e;
  --warn-bg: #fff8e1;
  --warn-border: #ffe082;
  --muted-strong: #666;
  --muted-weak: #aaa;
  --border-strong: #ccc;
  --border-weak: #eee;
  --code-bg: #f4f4f4;
  --cell-bg-alt: #f8f8f8;
  --exec-accent: #6c757d;
  --patch-accent: #9b59b6;
  --mcp-accent: #e67e22;
  --plan-accent: #3498db;
  --plan-bg: #f0f8ff;
  --hook-accent: #95a5a6;
  --agent-final-bg: #f0fff4;
```

`[data-theme="dark"]` 版本(色值调亮以适配暗背景,以下为建议起点,执行者可微调):
```css
  --ok: #4ec98a;
  --err: #e0655f;
  --warn: #d4b04e;
  --warn-bg: #3a3520;
  --warn-border: #5a5028;
  --muted-strong: #999;
  --muted-weak: #666;
  --border-strong: #444;
  --border-weak: #2a2a2a;
  --code-bg: #2a2a2a;
  --cell-bg-alt: #1f1f1f;
  --exec-accent: #8a9398;
  --patch-accent: #b06fd0;
  --mcp-accent: #e89b4e;
  --plan-accent: #4fa8da;
  --plan-bg: #1a2838;
  --hook-accent: #a5b0b6;
  --agent-final-bg: #1a3024;
```

`[data-theme="high-contrast"]` 版本(最大对比):
```css
  --ok: #00ff00;
  --err: #ff5555;
  --warn: #ffff00;
  --warn-bg: #333300;
  --warn-border: #ffff00;
  --muted-strong: #ddd;
  --muted-weak: #aaa;
  --border-strong: #fff;
  --border-weak: #ccc;
  --code-bg: #1a1a1a;
  --cell-bg-alt: #0a0a0a;
  --exec-accent: #ccc;
  --patch-accent: #ff00ff;
  --mcp-accent: #ffaa00;
  --plan-accent: #00ffff;
  --plan-bg: #001a33;
  --hook-accent: #aaa;
  --agent-final-bg: #003300;
```

- [ ] **Step 2: 替换所有硬编码颜色**

按 `grep -n "background: #\|color: #\|border.*: #\|border-color: #" styles.css` 的结果,**逐条规则**把硬编码色替换成对应变量。替换映射(前=当前硬编码值,后=变量名):

| 当前值 | 替换为 | 涉及规则(grep 定位) |
|---|---|---|
| `#fff` | `var(--cell-bg)` | `.cell`(已改 4.8)、`.composer`、`.approval`、`.approval-buttons button`、`.composer-send color` |
| `#888` | `var(--muted)` | `.scrollback-placeholder`、`.cell-role`(已改)、`.cell-mcp-progress`、`.cell-websearch`、`.approval-cwd`、`.model-item-id`、`.session-item-meta`(已改)、`.onboarding-dismiss-hint` |
| `#666` | `var(--muted-strong)` | `.cell-exec-status`、`.agent-status` |
| `#999` | `var(--muted-strong)` | `.cell-exec-cwd` |
| `#aaa` | `var(--muted-weak)` | `.agent-empty-hint`、`.composer-send:disabled` |
| `#ddd` | `var(--border)` | `.cell-reasoning-content border-top`、`.composer border-top`(已改)、`.app-sidebar border-left`(已改)、`.session-load-more border` |
| `#ccc` | `var(--border-strong)` | `.approval-buttons button border` |
| `#eee` | `var(--border-weak)` | `.cell border`(已改 4.8 为 --border,保持)、`.model-item-button border` |
| `#f4f4f4` | `var(--code-bg)` | `.cell-exec-output`、`.cell-patch-diff`、`.approval-command`、`.onboarding-cmd` |
| `#f8f8f8` | `var(--cell-bg-alt)` | `.cell-hook`、`.transcript-pager-cell` |
| `#1a1a1a` | `var(--fg)` | `.session-item-preview` |
| `#2eb872` | `var(--ok)` | `.cell-agent border-left`、`.cell-exec-completed .cell-exec-status`、`.cell-patch-file-add .cell-patch-kind`、`.approval-buttons button:first-child`、`.agent-row-sub-running/-completed .agent-status`(Task 5.3 临时硬编码) |
| `#d9534f` | `var(--err)` | `.cell-exec-failed .cell-exec-status`、`.cell-patch-file-delete .cell-patch-kind`、`.composer-interrupt`、`.agent-row-sub-failed .agent-status` |
| `#b8a02e` | `var(--warn)` | `.cell-reasoning border-left`、`.cell-reasoning-toggle color`、`.cell-patch-file-update .cell-patch-kind` |
| `#4a90e2` | `var(--accent)` | `.cell-user border-left`、`.composer-send bg`、`.approval-buttons button:nth-child(2)`、`.onboarding-dismiss`、`.model-item-active .model-item-button border-color`(已改) |
| `#fff8e1` | `var(--warn-bg)` | `.approvals-queue bg` |
| `#ffe082` | `var(--warn-border)` | `.approvals-queue border-top`、`.approval border` |
| `#6c757d` | `var(--exec-accent)` | `.cell-exec border-left` |
| `#9b59b6` | `var(--patch-accent)` | (patch 没单独 border 规则,可能 grep 不到 —— 跳过) |
| `#e67e22` | `var(--mcp-accent)` | `.cell-mcp border-left` |
| `#3498db` + `#f0f8ff` | `var(--plan-accent)` + `var(--plan-bg)` | `.cell-plan border-left` + `bg` |
| `#95a5a6` | `var(--hook-accent)` | `.cell-hook border-left` |
| `#f0fff4` | `var(--agent-final-bg)` | `.cell-agent-final bg` |
| `#fffdf3` | `var(--cell-bg-alt)` | `.cell-reasoning bg`(近似,微色差可接受) |

> **执行策略:** 用 Edit 工具逐条改(每条规则一个 Edit)。不要试图一次重写整个 styles.css —— 单 Edit 太大易错。改完每批跑一次 `npm run build` 确认没语法错。

- [ ] **Step 3: 验证 + commit**

```bash
cd /Users/def/lemurclaw/codex-rs/lemurclaw-gui/assets
npm test                    # CSS 改动不应让测试失败
npx tsc --noEmit
npm run build               # 确认 build 仍产出 dist/
# 确认无残留硬编码(应只剩 theme block 里的变量定义本身):
grep -c "background: #\|color: #\|border.*: #" styles.css   # 期望:接近 0(只剩 [data-theme] block 内的变量值)
cd /Users/def/lemurclaw
git add codex-rs/lemurclaw-gui/assets/src/styles.css
git commit -m "feat(gui): full styles.css var-ification (follow-up #3)

Var-ifies remaining ~50 hardcoded colors. Adds status (--ok/--err/--warn/
--warn-bg/--warn-border), neutral-variant (--muted-strong/--muted-weak/
--border-strong/--border-weak/--code-bg/--cell-bg-alt), and cell-type
(--exec-accent/--mcp-accent/--plan-accent/--plan-bg/--hook-accent/
--agent-final-bg) variables to each theme block. Dark/high-contrast
palettes now fully differentiate."
```

---

## 子项目 4 Follow-ups 完成标准

- [ ] `npm test` 全绿(89 + 新增 ~12 = ~101)
- [ ] `npx tsc --noEmit` 0 errors
- [ ] `npm run build` 产出 dist/
- [ ] **Follow-up #1:** TopBar 显示真实 cwd + model(替代 null stub)
- [ ] **Follow-up #2:** AgentPanel 渲染 sub-agent 行(替代 deferral hint)
- [ ] **Follow-up #3:** styles.css 所有硬编码颜色 var 化,dark/high-contrast 完整生效
- [ ] **Reducer 完整性:** state.cwd / currentModel / subAgents 都由 reducer + sendRequest response 填充

---

## 后续(子项目 5+)

- SettingsModal 系列(permissions/keymap/memories/skills/hooks/mcp/apps/plugins/experimental/statusline/title)
- Composer popups(SlashPopup / MentionPopup / FileSearchPopup 的 fuzzy 交互)
- 杂项 surface(statusline / diff viewer / /usage / /status)
- webui 模式(子项目 6)
- 发布就绪(子项目 7)

---

## 实现备注

1. **Task 5.2 是最 fiddly 的** —— Composer/ModelPicker/SessionPicker 的既有测试都 mock `send`,改调 `startTurn`/`onResume` prop 后需要更新 mock + 断言。预期这个 task 花最多时间在测试调整上。
2. **AbsolutePathBuf 形态:** ts-rs 把 `AbsolutePathBuf` 序列化为 `string`(已核实)。`LegacyAppPathString` 是 branded string —— 不同类型,本计划遇到的是 AbsolutePathBuf(Thread.cwd)。
3. **ModelRerouteReason 枚举值:** 测试 fixture 用 `'unavailable'`,若 tsc 报错 `cat types/v2/ModelRerouteReason.ts` 核实合法值。
4. **CellModel 'generic' fallback 保留:** Task 5.3 把 collab/subAgent 提到 state.subAgents,但 `threadItemToCell` 仍把它们映射到 `generic` cell(给 Scrollback 最小显示)。AgentPanel 从 state.subAgents 读结构化数据,Scrollback 从 CellModel 读最小展示,两者互补。
5. **CSS var 化的判断:** `#fffdf3`(reasoning bg)用 `var(--cell-bg-alt)` 近似 —— 微小色差可接受。若要精确加专用 `--reasoning-bg` 变量。
6. **sendRequest 类型:** `sendRequest<{model: string; cwd: string}>('turn/start', params)` —— 是 ThreadStartResponse 的结构子集。TS 不报错(结构性子集)。
7. **Task 5.3 的 status 字段 stringification:** `String(state.status)` 把 `CollabAgentStatus` 枚举值转 string。CSS class 用 `agent-row-sub-${status}` —— 如果 status 含非字母数字字符(不太可能,枚举值通常是标识符),需要 sanitize。先按标识符假设。
8. **本计划用分多次 Write/Edit 落盘** —— 避免子项目 3 计划写时的 "单次 Write 超长被中断" 教训(见排查记录)。

---
