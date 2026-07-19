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

export function summarizeCommandActions(actions: Array<CommandAction> | null | undefined): string {
  if (!actions || actions.length === 0) return '';
  // CommandAction.name only exists on the `read` variant; fall back to type tag.
  return actions.map((a) => ('name' in a && a.name ? a.name : a.type)).join(' | ');
}
