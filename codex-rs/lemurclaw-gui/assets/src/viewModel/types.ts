// ViewModel types: the shape React components read. The reducer (reducer.ts)
// is the single place that produces these from raw ServerNotification /
// ServerRequest events. Components never see raw events.
//
// Design rules:
// - All fields nullable-by-default at construction; reducer fills them in.
// - CellModel is a discriminated union on `kind`; each variant maps 1:1 to a
//   ThreadItem.type.
// - Timestamps are ms since epoch where applicable (matching codex's *AtMs).

import type { MessagePhase } from '../types/MessagePhase';
import type { PatchChangeKind, CommandExecutionStatus, CommandExecutionSource, PatchApplyStatus, McpToolCallStatus, DynamicToolCallStatus, ThreadStatus, TurnStatus, HookRunSummary, CommandAction, FileUpdateChange } from '../types/v2';
import type { RequestId } from '../types/RequestId';
import type { ServerRequest } from '../types/ServerRequest';
import type { FuzzyFileSearchResult } from '../types/FuzzyFileSearchResult';

export interface ConversationState {
  /** Ordered turns (oldest first). Each turn owns an ordered items list. */
  turns: TurnModel[];
  /** Active turn id (from most recent turn/started), null pre-first-turn or after turn/completed. */
  activeTurnId: string | null;
  /** Current thread status, or null pre-thread/started. */
  status: ThreadStatusModel | null;
  /** Working directory of the active thread, from thread/started's thread.cwd
   *  (and turn/start / thread/resume responses — Task 5.2). Null pre-thread/started.
   *  Displayed in TopBar. */
  cwd: string | null;
  /** Current model id for the active thread. Set from model/rerouted's toModel
   *  (and from the turn/start / thread/resume response — Task 5.2). Null until
   *  the first reroute or a sendRequest-backed start. */
  currentModel: string | null;
  /** Sub-agent rows for the sidebar AgentPanel. Derived from
   *  collabAgentToolCall items' agentsStates. Empty when no sub-agents
   *  observed in the active thread. */
  subAgents: SubAgentModel[];
  /** Pending ServerRequests awaiting user decision (ApprovalCard queue). */
  pendingApprovals: PendingApproval[];
  /** Latest turn-level unified diff from core's TurnDiffTracker. Null until
   *  the first turn/diff/updated notification arrives. Source for
   *  <DiffViewerModal> (subproject 5-C). */
  turnDiff: { turnId: string; diff: string } | null;
  /** Active fuzzy file search session (subproject 5-E Stage 3). Null when
   *  no @-mention popup is open. The Composer owns the lifecycle (start /
   *  update / stop); the reducer only mirrors the server's sessionUpdated
   *  pushes so the popup re-renders with new results. */
  fuzzySession: { sessionId: string; query: string; files: FuzzyFileSearchResult[] } | null;
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
  cwd: null,
  currentModel: null,
  subAgents: [],
  pendingApprovals: [],
  turnDiff: null,
  fuzzySession: null,
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

export function summarizeCommandActions(actions: Array<CommandAction> | null | undefined): string {
  if (!actions || actions.length === 0) return '';
  // CommandAction.name only exists on the `read` variant; fall back to type tag.
  return actions.map((a) => ('name' in a && a.name ? a.name : a.type)).join(' | ');
}

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
