import { describe, it, expect, vi } from 'vitest';
import { SLASH_COMMANDS } from '../slashCommands';
import { dispatchSlashCommand } from '../dispatch';
import type { SlashCommandContext } from '../slashCommandTypes';

function makeCtx(overrides: Partial<SlashCommandContext> = {}): SlashCommandContext {
  return {
    threadId: 't1',
    openSettings: vi.fn(),
    openModal: vi.fn(),
    localAction: vi.fn(),
    sendRequest: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

function cmd(name: string) {
  const c = SLASH_COMMANDS.find((x) => x.name === name);
  if (!c) throw new Error(`no command named ${name}`);
  return c;
}

describe('dispatchSlashCommand', () => {
  describe('sendTurn', () => {
    it('/init sends an AGENTS.md prompt', () => {
      const r = dispatchSlashCommand(cmd('init'), '', makeCtx());
      expect(r.kind).toBe('sendTurn');
      if (r.kind !== 'sendTurn') return;
      expect(r.input[0]).toMatchObject({ type: 'text', text: expect.stringContaining('AGENTS.md') });
    });

    it('/review without args sends default review prompt', () => {
      const r = dispatchSlashCommand(cmd('review'), '', makeCtx());
      expect(r.kind).toBe('sendTurn');
      if (r.kind !== 'sendTurn') return;
      expect(r.input[0]).toMatchObject({ type: 'text', text: 'Review my current changes and find issues' });
    });

    it('/review with args includes the args in the prompt', () => {
      const r = dispatchSlashCommand(cmd('review'), 'the foo bar module', makeCtx());
      expect(r.kind).toBe('sendTurn');
      if (r.kind !== 'sendTurn') return;
      expect(r.input[0]).toMatchObject({ type: 'text', text: 'Review this: the foo bar module' });
    });

    it('/compact sends a compact prompt', () => {
      const r = dispatchSlashCommand(cmd('compact'), '', makeCtx());
      expect(r.kind).toBe('sendTurn');
      if (r.kind !== 'sendTurn') return;
      expect(r.input[0]).toMatchObject({ type: 'text', text: expect.stringContaining('Compact') });
    });
  });

  describe('openModal', () => {
    it('/model calls ctx.openModal("model") and returns matching result', () => {
      const openModal = vi.fn();
      const r = dispatchSlashCommand(cmd('model'), '', makeCtx({ openModal }));
      expect(openModal).toHaveBeenCalledWith('model');
      expect(r).toEqual({ kind: 'openModal', modal: 'model' });
    });

    it('/theme calls ctx.openModal("theme") and returns matching result', () => {
      const openModal = vi.fn();
      const r = dispatchSlashCommand(cmd('theme'), '', makeCtx({ openModal }));
      expect(openModal).toHaveBeenCalledWith('theme');
      expect(r).toEqual({ kind: 'openModal', modal: 'theme' });
    });
  });

  describe('openSettings', () => {
    it.each([
      'permissions', 'memories', 'skills', 'hooks', 'mcp', 'apps', 'plugins', 'experimental',
    ])('/%s calls ctx.openSettings(%s) and returns matching result', (name) => {
      const openSettings = vi.fn();
      const r = dispatchSlashCommand(cmd(name), '', makeCtx({ openSettings }));
      expect(openSettings).toHaveBeenCalledWith(name);
      expect(r).toEqual({ kind: 'openSettings', surface: name });
    });
  });

  describe('localAction', () => {
    it.each(['clear', 'new'])('/%s calls ctx.localAction(%s) and returns matching result', (name) => {
      const localAction = vi.fn();
      const r = dispatchSlashCommand(cmd(name), '', makeCtx({ localAction }));
      expect(localAction).toHaveBeenCalledWith(name);
      expect(r).toEqual({ kind: 'localAction', action: name });
    });
  });

  describe('diff viewer (openModal)', () => {
    it('/diff calls ctx.openModal("diff") and returns matching result', () => {
      const openModal = vi.fn();
      const r = dispatchSlashCommand(cmd('diff'), '', makeCtx({ openModal }));
      expect(openModal).toHaveBeenCalledWith('diff');
      expect(r).toEqual({ kind: 'openModal', modal: 'diff' });
    });
  });

  // ----- Stage 2 coverage -----
  describe('Stage 2: session lifecycle (sendRequest)', () => {
    it.each([
      ['archive', 'thread/archive'],
      ['delete', 'thread/delete'],
      ['fork', 'thread/fork'],
    ])('/%s fires ctx.sendRequest(%s) with the active threadId', (name, method) => {
      const sendRequest = vi.fn().mockResolvedValue(undefined);
      const r = dispatchSlashCommand(cmd(name), '', makeCtx({ sendRequest }));
      expect(sendRequest).toHaveBeenCalledWith(method, { threadId: 't1' });
      expect(r.kind).toBe('sendRequest');
    });

    it('/rename with no args returns notApplicable usage hint', () => {
      const r = dispatchSlashCommand(cmd('rename'), '', makeCtx());
      expect(r.kind).toBe('notApplicable');
    });

    it('/rename with args fires thread/name/set', () => {
      const sendRequest = vi.fn().mockResolvedValue(undefined);
      dispatchSlashCommand(cmd('rename'), 'new name', makeCtx({ sendRequest }));
      expect(sendRequest).toHaveBeenCalledWith('thread/name/set', { threadId: 't1', name: 'new name' });
    });

    it('/resume returns notImplemented pointing at the sidebar', () => {
      const r = dispatchSlashCommand(cmd('resume'), '', makeCtx());
      expect(r.kind).toBe('notImplemented');
    });

    it('/logout fires account/logout', () => {
      const sendRequest = vi.fn().mockResolvedValue(undefined);
      dispatchSlashCommand(cmd('logout'), '', makeCtx({ sendRequest }));
      expect(sendRequest).toHaveBeenCalledWith('account/logout', {});
    });
  });

  describe('Stage 2: server-side queries (sendRequest)', () => {
    it('/status fires account/rateLimits/read', () => {
      const sendRequest = vi.fn().mockResolvedValue(undefined);
      dispatchSlashCommand(cmd('status'), '', makeCtx({ sendRequest }));
      expect(sendRequest).toHaveBeenCalledWith('account/rateLimits/read', {});
    });

    it('/usage fires account/usage/read with range', () => {
      const sendRequest = vi.fn().mockResolvedValue(undefined);
      dispatchSlashCommand(cmd('usage'), 'daily', makeCtx({ sendRequest }));
      expect(sendRequest).toHaveBeenCalledWith('account/usage/read', { range: 'daily' });
    });

    it('/debug-config fires config/read', () => {
      const sendRequest = vi.fn().mockResolvedValue(undefined);
      dispatchSlashCommand(cmd('debug-config'), '', makeCtx({ sendRequest }));
      expect(sendRequest).toHaveBeenCalledWith('config/read', {});
    });

    it('/feedback with no args returns notApplicable usage hint', () => {
      const r = dispatchSlashCommand(cmd('feedback'), '', makeCtx());
      expect(r.kind).toBe('notApplicable');
    });

    it('/feedback with args fires feedback/upload', () => {
      const sendRequest = vi.fn().mockResolvedValue(undefined);
      dispatchSlashCommand(cmd('feedback'), 'this is broken', makeCtx({ sendRequest }));
      expect(sendRequest).toHaveBeenCalledWith('feedback/upload', { message: 'this is broken' });
    });

    it('/import fires externalAgentConfig/import', () => {
      const sendRequest = vi.fn().mockResolvedValue(undefined);
      dispatchSlashCommand(cmd('import'), '', makeCtx({ sendRequest }));
      expect(sendRequest).toHaveBeenCalledWith('externalAgentConfig/import', {});
    });
  });

  describe('Stage 2: thread goal / plan / approve', () => {
    it('/goal clear fires thread/goal/clear', () => {
      const sendRequest = vi.fn().mockResolvedValue(undefined);
      dispatchSlashCommand(cmd('goal'), 'clear', makeCtx({ sendRequest }));
      expect(sendRequest).toHaveBeenCalledWith('thread/goal/clear', { threadId: 't1' });
    });

    it('/goal <text> fires thread/goal/set', () => {
      const sendRequest = vi.fn().mockResolvedValue(undefined);
      dispatchSlashCommand(cmd('goal'), 'ship 5-E', makeCtx({ sendRequest }));
      expect(sendRequest).toHaveBeenCalledWith('thread/goal/set', { threadId: 't1', goalDraft: 'ship 5-E' });
    });

    it('/plan returns sendTurn with plan text', () => {
      const r = dispatchSlashCommand(cmd('plan'), 'redesign the diff viewer', makeCtx());
      expect(r.kind).toBe('sendTurn');
    });

    it('/approve fires review/start with auto_review_retry target', () => {
      const sendRequest = vi.fn().mockResolvedValue(undefined);
      dispatchSlashCommand(cmd('approve'), '', makeCtx({ sendRequest }));
      expect(sendRequest).toHaveBeenCalledWith('review/start', { target: 'auto_review_retry' });
    });
  });

  describe('Stage 2: turn-prefix slash passthrough', () => {
    it.each(['side', 'btw', 'agent', 'subagents', 'personality'])(
      '/%s sends a turn with the literal slash text (server-side interpretation)',
      (name) => {
        // makeCtx with threadId is the default; the dispatch just builds a
        // turn input — we assert it contains the slash text.
        const r = dispatchSlashCommand(cmd(name), 'foo bar', makeCtx());
        expect(r.kind).toBe('sendTurn');
        if (r.kind !== 'sendTurn') return;
        const text = (r.input[0] as { text: string }).text;
        expect(text).toContain(`/${name}`);
        expect(text).toContain('foo bar');
      },
    );
  });

  describe('Stage 2: localAction additions', () => {
    it.each([
      ['copy', 'copy'],
      ['raw', 'raw'],
      ['quit', 'quit'],
      ['exit', 'quit'], // alias maps to quit
    ])('/%s calls ctx.localAction(%s)', (name, expected) => {
      const localAction = vi.fn();
      const r = dispatchSlashCommand(cmd(name), '', makeCtx({ localAction }));
      expect(localAction).toHaveBeenCalledWith(expected);
      expect(r).toEqual({ kind: 'localAction', action: expected as never });
    });
  });

  describe('Stage 2: notImplemented stubs', () => {
    it.each(['ide', 'app'])('/%s returns notImplemented', (name) => {
      const r = dispatchSlashCommand(cmd(name), '', makeCtx());
      expect(r.kind).toBe('notImplemented');
    });
  });

  describe('Stage 3: /mention points users at the @ popup', () => {
    it('/mention returns notApplicable pointing at the @ popup', () => {
      const r = dispatchSlashCommand(cmd('mention'), '', makeCtx());
      expect(r.kind).toBe('notApplicable');
      if (r.kind !== 'notApplicable') return;
      expect(r.message).toMatch(/@.*composer/i);
    });
  });

  describe('Stage 2: notApplicable (TUI-only / debug-only)', () => {
    it.each([
      'vim', 'keymap', 'title', 'statusline', 'pets', 'ps', 'stop',
      'setup-default-sandbox', 'sandbox-add-read-dir',
      'rollout', 'test-approval', 'debug-m-drop', 'debug-m-update',
    ])('/%s returns notApplicable', (name) => {
      const r = dispatchSlashCommand(cmd(name), '', makeCtx());
      expect(r.kind).toBe('notApplicable');
    });
  });
});
