import type { SettingsSurface } from '../settings/SettingsModal';

/** The kind of wire side-effect a slash command produces. */
export type SlashCommandCategory =
  | 'sendTurn'
  | 'openSettings'
  | 'openModal'
  | 'localAction'
  | 'notImplemented';

/** Local-only Composer actions (no turn, no server round-trip). */
export type LocalAction = 'clear' | 'new' | 'quit';

/** Existing top-level modals. Kept in sync with App.tsx's ModalKind union. */
export type ModalKind = 'model' | 'theme' | 'transcript' | 'settings';

/** Discriminated union returned by every command's dispatch(). The Composer/
 *  App switches on `kind` to route the side effect. */
export type SlashCommandResult =
  | { kind: 'sendTurn'; input: unknown[] }
  | { kind: 'openSettings'; surface: SettingsSurface }
  | { kind: 'openModal'; modal: ModalKind }
  | { kind: 'localAction'; action: LocalAction }
  | { kind: 'notImplemented'; message: string };

/** Context passed to every dispatch(): the callbacks a command needs to
 *  trigger UI side effects without knowing about App's internal state. */
export interface SlashCommandContext {
  threadId: string | null;
  openSettings: (surface: SettingsSurface) => void;
  openModal: (modal: ModalKind) => void;
  localAction: (action: LocalAction) => void;
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
