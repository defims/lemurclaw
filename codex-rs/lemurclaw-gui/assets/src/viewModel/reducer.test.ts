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

  // ---- Edge cases (added per Task 3.4 code review) ---------------------

  it('item/*/delta for an unknown itemId silently no-ops (does not crash or invent a cell)', () => {
    // Codex's next_event contract guarantees item/started precedes any delta,
    // but if the contract is ever violated the reducer must degrade gracefully.
    const afterTurn = reducer(st(), { method: 'turn/started', params: { threadId: 't1', turn: EMPTY_TURN } });
    const afterDelta = reducer(afterTurn, {
      method: 'item/agentMessage/delta',
      params: { threadId: 't1', turnId: 'tu1', itemId: 'never-started', delta: 'orphan' },
    });
    // No cell added, state structure unchanged in items length.
    expect(afterDelta.turns[0].items).toHaveLength(0);
    // State reference changes (immutable update through turn rebuild) but no
    // data loss beyond the dropped delta.
    expect(afterDelta).not.toBe(afterTurn);
  });

  it('multiple pendingApprovals coexist; serverRequest/resolved removes only the matching one', () => {
    const base = {
      threadId: 't1', turnId: 'tu1', itemId: 'i1', startedAtMs: 1,
      environmentId: null, command: 'ls', cwd: { path: '/x' }, commandActions: null,
    };
    const afterFirst = reducer(st(), {
      method: 'item/commandExecution/requestApproval', id: 42, params: base,
    });
    const afterSecond = reducer(afterFirst, {
      method: 'item/commandExecution/requestApproval', id: 'abc', params: { ...base, itemId: 'i2' },
    });
    expect(afterSecond.pendingApprovals.map((a) => a.requestId)).toEqual([42, 'abc']);
    // Resolve the numeric one; the string one must remain.
    const afterResolveNum = reducer(afterSecond, {
      method: 'serverRequest/resolved',
      params: { requestId: 42 },
    });
    expect(afterResolveNum.pendingApprovals.map((a) => a.requestId)).toEqual(['abc']);
    // Resolve the string one by String-normalized match (RequestId = string|number).
    const afterResolveStr = reducer(afterResolveNum, {
      method: 'serverRequest/resolved',
      params: { requestId: 'abc' },
    });
    expect(afterResolveStr.pendingApprovals).toHaveLength(0);
  });

  it('hook/started then hook/completed coalesce by run.id into a single cell', () => {
    const afterTurn = reducer(st(), { method: 'turn/started', params: { threadId: 't1', turn: EMPTY_TURN } });
    const startedRun = {
      id: 'h1', eventName: 'PreToolUse', handlerType: 'command', executionMode: 'blocking',
      scope: 'session', sourcePath: { path: '/h/.codex/hook.sh' }, source: 'project',
      displayOrder: 0n, status: 'inProgress', statusMessage: null,
      startedAt: 1n, completedAt: null, durationMs: null, entries: [],
    };
    const afterStarted = reducer(afterTurn, {
      method: 'hook/started',
      params: { threadId: 't1', turnId: 'tu1', run: startedRun },
    });
    expect(afterStarted.turns[0].items).toHaveLength(1);
    expect(afterStarted.turns[0].items[0].kind).toBe('hook');

    const completedRun = { ...startedRun, status: 'completed', completedAt: 2n, durationMs: 1n };
    const afterCompleted = reducer(afterStarted, {
      method: 'hook/completed',
      params: { threadId: 't1', turnId: 'tu1', run: completedRun },
    });
    // Same cell (upsert by run.id), not a new one.
    expect(afterCompleted.turns[0].items).toHaveLength(1);
    const cell = afterCompleted.turns[0].items[0];
    if (cell.kind !== 'hook') throw new Error('expected hook cell');
    expect(cell.run.status).toBe('completed');
  });

  it('hook events with null turnId are dropped (no Scrollback anchor pre-first-turn)', () => {
    const before = st();
    const next = reducer(before, {
      method: 'hook/started',
      params: { threadId: 't1', turnId: null, run: {
        id: 'h1', eventName: 'PreToolUse', handlerType: 'command', executionMode: 'blocking',
        scope: 'session', sourcePath: { path: '/h/.codex/hook.sh' }, source: 'project',
        displayOrder: 0n, status: 'inProgress', statusMessage: null,
        startedAt: 1n, completedAt: null, durationMs: null, entries: [],
      } },
    });
    expect(next).toBe(before); // state unchanged (reference equality — reducer returns state as-is)
  });
});
