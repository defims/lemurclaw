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

import type { ConversationState, TurnModel, CellModel, PendingApproval, ApprovalKind, ResponseMetaAction } from './types';
import { isServerNotification, isServerRequest } from '../types/guards';
import type { ServerNotification } from '../types/ServerNotification';
import type { ServerRequest } from '../types/ServerRequest';
import type { ThreadItem, Turn, HookRunSummary, UserInput } from '../types/v2';

export function reducer(state: ConversationState, event: unknown): ConversationState {
  if (isServerNotification(event)) {
    return applyNotification(state, event);
  }
  if (isServerRequest(event)) {
    return applyServerRequest(state, event);
  }
  if (isResponseMetaAction(event)) {
    return applyResponseMeta(state, event);
  }
  // Unknown event shape (e.g. backend's `{lagged, skipped}` envelope).
  return state;
}

function applyNotification(state: ConversationState, n: ServerNotification): ConversationState {
  switch (n.method) {
    case 'thread/started': {
      const thread = n.params.thread;
      // AbsolutePathBuf is a ts-rs `string` alias at the wire level, so
      // thread.cwd is already a string. Trusting the type (no defensive
      // coercion) — a previous version tried `String(cwdRaw ?? '')` for
      // schema drift, but that degrades the realistic { path: '/x' } drift
      // shape to "[object Object]", which is worse than crashing loudly.
      return {
        ...state,
        status: thread.status,
        activeTurnId: null,
        cwd: thread.cwd,
      };
    }
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
    case 'model/rerouted':
      // Backend rerouted the model (e.g. chosen model unavailable → fallback).
      // toModel is the now-current model id; fromModel is informational only.
      return { ...state, currentModel: n.params.toModel };
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
  const hookCell: CellModel = { kind: 'hook', run };
  const turns = state.turns.map((t) => {
    if (t.id !== turnId) return t;
    const existingIdx = t.items.findIndex((c) => c.kind === 'hook' && c.run.id === run.id);
    if (existingIdx >= 0) {
      const items: CellModel[] = [...t.items];
      items[existingIdx] = hookCell;
      return { ...t, items };
    }
    const items: CellModel[] = [...t.items, hookCell];
    return { ...t, items };
  });
  return { ...state, turns };
}

/** Map a raw ThreadItem (from upstream types) to our CellModel. Centralizes
 *  the upstream → ViewModel translation so components stay dumb. */
export function threadItemToCell(item: ThreadItem): CellModel {
  switch (item.type) {
    case 'userMessage': {
      const text = item.content
        .filter((c: UserInput): c is Extract<UserInput, { type: 'text' }> => c.type === 'text')
        .map((c) => c.text)
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
        cwd: item.cwd,
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
    case 'webSearch':
      return { kind: 'webSearch', itemId: item.id, query: item.query, status: 'completed' };
    case 'imageGeneration':
      // ImageGenerationItem has `revisedPrompt: string | null`; use it as the
      // display prompt. Subproject 3 only needs a best-effort label.
      return { kind: 'imageGeneration', itemId: item.id, prompt: item.revisedPrompt ?? '' };
    case 'sleep':
      return { kind: 'sleep', itemId: item.id, durationMs: item.durationMs };
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
