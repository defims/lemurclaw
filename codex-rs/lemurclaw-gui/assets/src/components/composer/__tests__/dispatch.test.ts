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
});
