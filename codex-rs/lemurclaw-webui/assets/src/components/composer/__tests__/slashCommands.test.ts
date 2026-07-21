import { describe, it, expect } from 'vitest';
import { SLASH_COMMANDS } from '../slashCommands';

describe('SLASH_COMMANDS catalog', () => {
  it('has the full TUI parity catalog (Stage 1 + Stage 2)', () => {
    // 55 = all codex TUI SlashCommand variants (Stage 1's 16 + Stage 2's 39).
    expect(SLASH_COMMANDS.length).toBe(55);
  });

  it('every command has non-empty name, description, and a dispatch fn', () => {
    for (const cmd of SLASH_COMMANDS) {
      expect(cmd.name.length).toBeGreaterThan(0);
      expect(cmd.description.length).toBeGreaterThan(0);
      expect(typeof cmd.dispatch).toBe('function');
    }
  });

  it('names are unique', () => {
    const names = SLASH_COMMANDS.map((c) => c.name);
    expect(new Set(names).size).toBe(names.length);
  });

  it('names are lowercase kebab-case with no spaces', () => {
    for (const cmd of SLASH_COMMANDS) {
      expect(cmd.name).toMatch(/^[a-z][a-z0-9-]*$/);
    }
  });

  it('every command declares a valid category', () => {
    const valid = new Set([
      'sendTurn', 'openSettings', 'openModal', 'localAction',
      'sendRequest', 'showResponse', 'notImplemented', 'notApplicable',
    ]);
    for (const cmd of SLASH_COMMANDS) {
      expect(valid.has(cmd.category)).toBe(true);
    }
  });
});
