import type { SlashCommand } from './slashCommandTypes';

/** Hardcoded slash command catalog for Stage 1 (16 commands).
 *
 *  Why hardcoded: the app-server wire protocol exposes NO command metadata
 *  (verified: searched app-server-protocol for `available_commands` /
 *  `SlashCommand` — absent). Spec §4.2's "available_commands (Initialize)"
 *  data-source claim is incorrect. So the catalog mirrors the subset of
 *  codex TUI's SlashCommand enum that applies to a web GUI.
 *
 *  Inapplicable-to-web commands (/vim /title /statusline /pets /ps /stop
 *  /keymap /sandbox-* /ide /copy /raw /debug-* /test-approval /rollout) are
 *  intentionally absent: typing them in the popup shows no match and Enter
 *  sends them as plain text (server-side no-op). They land in Stage 2 with
 *  explicit "not available in GUI" stubs if we want popup-visible messages.
 *
 *  Order matters — it's the display order in the popup when the user types
 *  just "/" (matches TUI's "DO NOT ALPHA-SORT" convention: frequent first). */
const INIT_PROMPT =
  'Create an AGENTS.md file in the repository root with instructions for working in this codebase.';

export const SLASH_COMMANDS: SlashCommand[] = [
  // sendTurn — submit a turn with a synthesized message
  {
    name: 'init',
    description: 'Create an AGENTS.md file with instructions for Codex',
    category: 'sendTurn',
    dispatch: () => ({
      kind: 'sendTurn',
      input: [{ type: 'text', text: INIT_PROMPT, text_elements: [] }],
    }),
  },
  {
    name: 'review',
    description: 'Review my current changes and find issues',
    category: 'sendTurn',
    dispatch: (args) => ({
      kind: 'sendTurn',
      input: [{
        type: 'text',
        text: args ? `Review this: ${args}` : 'Review my current changes and find issues',
        text_elements: [],
      }],
    }),
  },
  {
    name: 'compact',
    description: 'Summarize conversation to prevent hitting the context limit',
    category: 'sendTurn',
    dispatch: () => ({
      kind: 'sendTurn',
      input: [{ type: 'text', text: 'Compact the conversation', text_elements: [] }],
    }),
  },

  // openModal — reuse existing top-level pickers
  {
    name: 'model',
    description: 'Choose what model and reasoning effort to use',
    category: 'openModal',
    dispatch: (_args, ctx) => { ctx.openModal('model'); return { kind: 'openModal', modal: 'model' }; },
  },
  {
    name: 'theme',
    description: 'Choose a syntax highlighting theme',
    category: 'openModal',
    dispatch: (_args, ctx) => { ctx.openModal('theme'); return { kind: 'openModal', modal: 'theme' }; },
  },

  // openSettings — map directly to SettingsModal surfaces
  {
    name: 'permissions',
    description: 'Choose what Codex is allowed to do',
    category: 'openSettings',
    dispatch: (_args, ctx) => { ctx.openSettings('permissions'); return { kind: 'openSettings', surface: 'permissions' }; },
  },
  {
    name: 'memories',
    description: 'Configure memory use and generation',
    category: 'openSettings',
    dispatch: (_args, ctx) => { ctx.openSettings('memories'); return { kind: 'openSettings', surface: 'memories' }; },
  },
  {
    name: 'skills',
    description: 'Use skills to improve how Codex performs specific tasks',
    category: 'openSettings',
    dispatch: (_args, ctx) => { ctx.openSettings('skills'); return { kind: 'openSettings', surface: 'skills' }; },
  },
  {
    name: 'hooks',
    description: 'View and manage lifecycle hooks',
    category: 'openSettings',
    dispatch: (_args, ctx) => { ctx.openSettings('hooks'); return { kind: 'openSettings', surface: 'hooks' }; },
  },
  {
    name: 'mcp',
    description: 'List configured MCP tools',
    category: 'openSettings',
    dispatch: (_args, ctx) => { ctx.openSettings('mcp'); return { kind: 'openSettings', surface: 'mcp' }; },
  },
  {
    name: 'apps',
    description: 'Manage apps',
    category: 'openSettings',
    dispatch: (_args, ctx) => { ctx.openSettings('apps'); return { kind: 'openSettings', surface: 'apps' }; },
  },
  {
    name: 'plugins',
    description: 'Browse plugins',
    category: 'openSettings',
    dispatch: (_args, ctx) => { ctx.openSettings('plugins'); return { kind: 'openSettings', surface: 'plugins' }; },
  },
  {
    name: 'experimental',
    description: 'Toggle experimental features',
    category: 'openSettings',
    dispatch: (_args, ctx) => { ctx.openSettings('experimental'); return { kind: 'openSettings', surface: 'experimental' }; },
  },

  // localAction — pure client-side ops
  {
    name: 'clear',
    description: 'Clear the terminal and start a new chat',
    category: 'localAction',
    dispatch: (_args, ctx) => { ctx.localAction('clear'); return { kind: 'localAction', action: 'clear' }; },
  },
  {
    name: 'new',
    description: 'Start a new chat during a conversation',
    category: 'localAction',
    dispatch: (_args, ctx) => { ctx.localAction('new'); return { kind: 'localAction', action: 'new' }; },
  },

  // notImplemented — explicit "later" stub (5-C will fill /diff)
  {
    name: 'diff',
    description: 'Show git diff (including untracked files)',
    category: 'notImplemented',
    dispatch: () => ({ kind: 'notImplemented', message: 'diff viewer coming in subproject 5-C' }),
  },
];
