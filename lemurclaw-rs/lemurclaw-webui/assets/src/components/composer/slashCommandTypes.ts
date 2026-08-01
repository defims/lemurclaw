import type { SettingsSurface } from '../settings/SettingsModal';

/** The kind of wire side-effect a slash command produces. */
export type SlashCommandCategory =
  | 'sendTurn'
  | 'openSettings'
  | 'openModal'
  | 'localAction'
  | 'sendRequest'      // fire a ClientRequest RPC, ignore response (Stage 2)
  | 'showResponse'     // fire RPC, display response in a modal (Stage 3 follow-up)
  | 'notImplemented'   // planned for future, currently surfaces a message
  | 'notApplicable';   // GUI will never support (e.g. /vim /title /pets)

/** Local-only Composer actions (no turn, no server round-trip). */
export type LocalAction = 'clear' | 'new' | 'quit' | 'copy' | 'raw';

/** Existing top-level modals. Kept in sync with App.tsx's ModalKind union. */
export type ModalKind = 'model' | 'theme' | 'transcript' | 'settings' | 'diff';

/** Discriminated union returned by every command's dispatch(). The Composer/
 *  App switches on `kind` to route the side effect. */
export type SlashCommandResult =
  | { kind: 'sendTurn'; input: unknown[] }
  | { kind: 'openSettings'; surface: SettingsSurface }
  | { kind: 'openModal'; modal: ModalKind }
  | { kind: 'localAction'; action: LocalAction }
  | { kind: 'sendRequest'; method: string; params: unknown }
  | { kind: 'showResponse'; method: string; params: unknown; title: string }
  | { kind: 'notImplemented'; message: string }
  | { kind: 'notApplicable'; message: string };

/** Context passed to every dispatch(): the callbacks a command needs to
 *  trigger UI side effects without knowing about App's internal state. */
export interface SlashCommandContext {
  threadId: string | null;
  openSettings: (surface: SettingsSurface) => void;
  openModal: (modal: ModalKind) => void;
  localAction: (action: LocalAction) => void;
  /** Fire a JSON-RPC request to the backend. Response is ignored (Stage 2
   *  surfaces these commands fire-and-forget; a future stage may display
   *  responses in a modal). Returns a Promise so callers can await if needed. */
  sendRequest: (method: string, params: unknown) => Promise<unknown>;
}

/** One entry in the slash command catalog. */
export interface SlashCommand {
  /** WITHOUT leading "/". Must be lowercase, no spaces. */
  name: string;
  /** One-line description shown in the popup. */
  description: string;
  /** Which kind of result dispatch returns (documentation/test aid). */
  category: SlashCommandCategory;
  /** Optional availability predicate; absent means always available. */
  available?: (ctx: SlashCommandContext) => boolean;
  /** Compute the side effect for this command given user args. */
  dispatch: (args: string, ctx: SlashCommandContext) => SlashCommandResult;
}
