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

  // openModal — full-screen diff viewer (5-C)
  {
    name: 'diff',
    description: 'Show git diff (including untracked files)',
    category: 'openModal',
    dispatch: (_args, ctx) => { ctx.openModal('diff'); return { kind: 'openModal', modal: 'diff' }; },
  },

  // ============================================================
  // Stage 2 additions (subproject 5-E Stage 2): session control,
  // server queries, turn-prefix commands, local-only ops, and
  // explicit "GUI doesn't support this" stubs for TUI-only commands.
  // ============================================================

  // ----- sendRequest: session lifecycle (fire-and-forget; Stage 2 ignores
  // the response, a future stage may show a confirmation toast) -----
  {
    name: 'archive',
    description: 'Archive this session and exit',
    category: 'sendRequest',
    dispatch: (_args, ctx) => {
      if (!ctx.threadId) return { kind: 'notApplicable', message: 'no active thread' };
      ctx.sendRequest('thread/archive', { threadId: ctx.threadId });
      return { kind: 'sendRequest', method: 'thread/archive', params: { threadId: ctx.threadId } };
    },
  },
  {
    name: 'delete',
    description: 'Permanently delete this session and exit',
    category: 'sendRequest',
    dispatch: (_args, ctx) => {
      if (!ctx.threadId) return { kind: 'notApplicable', message: 'no active thread' };
      ctx.sendRequest('thread/delete', { threadId: ctx.threadId });
      return { kind: 'sendRequest', method: 'thread/delete', params: { threadId: ctx.threadId } };
    },
  },
  {
    name: 'fork',
    description: 'Fork the current chat',
    category: 'sendRequest',
    dispatch: (_args, ctx) => {
      if (!ctx.threadId) return { kind: 'notApplicable', message: 'no active thread' };
      ctx.sendRequest('thread/fork', { threadId: ctx.threadId });
      return { kind: 'sendRequest', method: 'thread/fork', params: { threadId: ctx.threadId } };
    },
  },
  {
    name: 'rename',
    description: 'Rename the current thread (usage: /rename <new name>)',
    category: 'sendRequest',
    dispatch: (args, ctx) => {
      if (!ctx.threadId) return { kind: 'notApplicable', message: 'no active thread' };
      const name = args.trim();
      if (!name) return { kind: 'notApplicable', message: 'usage: /rename <new name>' };
      ctx.sendRequest('thread/name/set', { threadId: ctx.threadId, name });
      return { kind: 'sendRequest', method: 'thread/name/set', params: { threadId: ctx.threadId, name } };
    },
  },
  {
    name: 'resume',
    description: 'Resume a saved chat (usage: /resume <id-or-name>)',
    category: 'notImplemented',
    dispatch: () => ({ kind: 'notImplemented', message: '/resume picker coming in a future stage (use the Sessions sidebar for now)' }),
  },
  {
    name: 'logout',
    description: 'Log out of Codex',
    category: 'sendRequest',
    dispatch: (_args, ctx) => {
      ctx.sendRequest('account/logout', {});
      return { kind: 'sendRequest', method: 'account/logout', params: {} };
    },
  },

  // ----- sendRequest: server-side queries (response ignored in Stage 2;
  // future stage renders the response in a modal) -----
  {
    name: 'status',
    description: 'Show current session configuration and token usage',
    category: 'sendRequest',
    dispatch: (_args, ctx) => {
      ctx.sendRequest('account/rateLimits/read', {});
      return { kind: 'sendRequest', method: 'account/rateLimits/read', params: {} };
    },
  },
  {
    name: 'usage',
    description: 'View account usage (usage daily|weekly|cumulative)',
    category: 'sendRequest',
    dispatch: (args, ctx) => {
      ctx.sendRequest('account/usage/read', { range: args.trim() || 'cumulative' });
      return { kind: 'sendRequest', method: 'account/usage/read', params: { range: args.trim() || 'cumulative' } };
    },
  },
  {
    name: 'debug-config',
    description: 'Show config layers and requirement sources for debugging',
    category: 'sendRequest',
    dispatch: (_args, ctx) => {
      ctx.sendRequest('config/read', {});
      return { kind: 'sendRequest', method: 'config/read', params: {} };
    },
  },
  {
    name: 'feedback',
    description: 'Send logs to maintainers (feedback <message>)',
    category: 'sendRequest',
    dispatch: (args, ctx) => {
      const message = args.trim();
      if (!message) return { kind: 'notApplicable', message: 'usage: /feedback <message>' };
      ctx.sendRequest('feedback/upload', { message });
      return { kind: 'sendRequest', method: 'feedback/upload', params: { message } };
    },
  },
  {
    name: 'import',
    description: 'Import setup and chats from Claude Code',
    category: 'sendRequest',
    dispatch: (_args, ctx) => {
      ctx.sendRequest('externalAgentConfig/import', {});
      return { kind: 'sendRequest', method: 'externalAgentConfig/import', params: {} };
    },
  },

  // ----- sendRequest: thread goal + plan + agent (turn-prefix or thread
  // settings; Stage 2 fires the RPC, response ignored) -----
  {
    name: 'goal',
    description: 'Set/clear/edit/pause/resume a long-running task goal',
    category: 'sendRequest',
    dispatch: (args, ctx) => {
      if (!ctx.threadId) return { kind: 'notApplicable', message: 'no active thread' };
      const a = args.trim();
      // /goal clear → thread/goal/clear; /goal <text> → thread/goal/set
      if (a === 'clear') {
        ctx.sendRequest('thread/goal/clear', { threadId: ctx.threadId });
        return { kind: 'sendRequest', method: 'thread/goal/clear', params: { threadId: ctx.threadId } };
      }
      ctx.sendRequest('thread/goal/set', { threadId: ctx.threadId, goalDraft: a });
      return { kind: 'sendRequest', method: 'thread/goal/set', params: { threadId: ctx.threadId, goalDraft: a } };
    },
  },
  {
    name: 'plan',
    description: 'Switch to Plan mode (plan <text> to plan a specific task)',
    category: 'sendTurn',
    dispatch: (args) => ({
      kind: 'sendTurn',
      input: [{
        type: 'text',
        text: args.trim() ? `Plan: ${args.trim()}` : 'Switch to Plan mode',
        text_elements: [],
      }],
    }),
  },
  {
    name: 'approve',
    description: 'Approve one retry of a recent auto-review denial',
    category: 'sendRequest',
    dispatch: (_args, ctx) => {
      ctx.sendRequest('review/start', { target: 'auto_review_retry' });
      return { kind: 'sendRequest', method: 'review/start', params: { target: 'auto_review_retry' } };
    },
  },

  // ----- sendTurn: side conversation + agent switching (server treats the
  // slash text as a user message) -----
  {
    name: 'side',
    description: 'Start a side conversation (side <text>)',
    category: 'sendTurn',
    dispatch: (args) => ({
      kind: 'sendTurn',
      input: [{ type: 'text', text: `/side ${args}`.trim(), text_elements: [] }],
    }),
  },
  {
    name: 'btw',
    description: 'Alias for /side',
    category: 'sendTurn',
    dispatch: (args) => ({
      kind: 'sendTurn',
      input: [{ type: 'text', text: `/btw ${args}`.trim(), text_elements: [] }],
    }),
  },
  {
    name: 'agent',
    description: 'Switch the active agent thread',
    category: 'sendTurn',
    dispatch: (args) => ({
      kind: 'sendTurn',
      input: [{ type: 'text', text: `/agent ${args}`.trim(), text_elements: [] }],
    }),
  },
  {
    name: 'subagents',
    description: 'Alias for /agent',
    category: 'sendTurn',
    dispatch: (args) => ({
      kind: 'sendTurn',
      input: [{ type: 'text', text: `/subagents ${args}`.trim(), text_elements: [] }],
    }),
  },
  {
    name: 'personality',
    description: 'Choose a communication style for Codex',
    category: 'sendTurn',
    dispatch: (args) => ({
      kind: 'sendTurn',
      input: [{ type: 'text', text: `/personality ${args}`.trim(), text_elements: [] }],
    }),
  },

  // ----- localAction: GUI-local ops -----
  {
    name: 'copy',
    description: 'Copy last agent response as markdown',
    category: 'localAction',
    dispatch: (_args, ctx) => { ctx.localAction('copy'); return { kind: 'localAction', action: 'copy' }; },
  },
  {
    name: 'raw',
    description: 'Toggle raw scrollback mode for copy-friendly output',
    category: 'localAction',
    dispatch: (_args, ctx) => { ctx.localAction('raw'); return { kind: 'localAction', action: 'raw' }; },
  },
  {
    name: 'quit',
    description: 'Exit Codex',
    category: 'localAction',
    dispatch: (_args, ctx) => { ctx.localAction('quit'); return { kind: 'localAction', action: 'quit' }; },
  },
  {
    name: 'exit',
    description: 'Alias for /quit',
    category: 'localAction',
    dispatch: (_args, ctx) => { ctx.localAction('quit'); return { kind: 'localAction', action: 'quit' }; },
  },

  // ----- notImplemented: planned for future stages -----
  {
    name: 'mention',
    description: 'Mention a file (type @ in the composer)',
    category: 'notApplicable',
    dispatch: () => ({ kind: 'notApplicable', message: 'Type @ in the composer to mention a file — the popup opens automatically' }),
  },
  {
    name: 'ide',
    description: 'Include current selection / open files from your IDE',
    category: 'notImplemented',
    dispatch: () => ({ kind: 'notImplemented', message: 'IDE context handoff not yet supported in lemurclaw-gui' }),
  },
  {
    name: 'app',
    description: 'Continue this session in the Desktop app (macOS/Windows)',
    category: 'notImplemented',
    dispatch: () => ({ kind: 'notImplemented', message: 'Desktop handoff not yet supported in lemurclaw-gui' }),
  },

  // ----- notApplicable: TUI-only commands GUI will never support -----
  {
    name: 'vim',
    description: 'Toggle Vim mode for the composer',
    category: 'notApplicable',
    dispatch: () => ({ kind: 'notApplicable', message: 'Vim mode is a terminal-composer feature; not applicable in the web GUI' }),
  },
  {
    name: 'keymap',
    description: 'Remap TUI shortcuts',
    category: 'notApplicable',
    dispatch: () => ({ kind: 'notApplicable', message: 'TUI keymap customization is not exposed by the app-server API' }),
  },
  {
    name: 'title',
    description: 'Configure terminal title items',
    category: 'notApplicable',
    dispatch: () => ({ kind: 'notApplicable', message: 'terminal title is a TUI-only concept' }),
  },
  {
    name: 'statusline',
    description: 'Configure status line items',
    category: 'notApplicable',
    dispatch: () => ({ kind: 'notApplicable', message: 'status line is a TUI-only concept (not in app-server config/read)' }),
  },
  {
    name: 'pets',
    description: 'Choose or hide the terminal pet',
    category: 'notApplicable',
    dispatch: () => ({ kind: 'notApplicable', message: 'terminal pets are TUI-only' }),
  },
  {
    name: 'ps',
    description: 'List background terminals',
    category: 'notApplicable',
    dispatch: () => ({ kind: 'notApplicable', message: 'background terminals are TUI-only' }),
  },
  {
    name: 'stop',
    description: 'Stop all background terminals',
    category: 'notApplicable',
    dispatch: () => ({ kind: 'notApplicable', message: 'background terminals are TUI-only' }),
  },
  {
    name: 'setup-default-sandbox',
    description: 'Set up elevated agent sandbox (Windows-only)',
    category: 'notApplicable',
    dispatch: () => ({ kind: 'notApplicable', message: 'Windows-only sandbox setup not supported in this build' }),
  },
  {
    name: 'sandbox-add-read-dir',
    description: 'Let sandbox read a directory (Windows-only)',
    category: 'notApplicable',
    dispatch: () => ({ kind: 'notApplicable', message: 'Windows-only sandbox setup not supported in this build' }),
  },
  {
    name: 'rollout',
    description: 'Print the rollout file path (debug only)',
    category: 'notApplicable',
    dispatch: () => ({ kind: 'notApplicable', message: 'debug-only TUI command' }),
  },
  {
    name: 'test-approval',
    description: 'Test approval request (debug only)',
    category: 'notApplicable',
    dispatch: () => ({ kind: 'notApplicable', message: 'debug-only TUI command' }),
  },
  {
    name: 'debug-m-drop',
    description: 'DO NOT USE',
    category: 'notApplicable',
    dispatch: () => ({ kind: 'notApplicable', message: 'debug-only; do not use' }),
  },
  {
    name: 'debug-m-update',
    description: 'DO NOT USE',
    category: 'notApplicable',
    dispatch: () => ({ kind: 'notApplicable', message: 'debug-only; do not use' }),
  },
];


