import { describe, it, expect } from 'vitest';
import { SLASH_COMMANDS } from '../slashCommands';

describe('SLASH_COMMANDS catalog', () => {
  it('has the expected stage-1 size', () => {
    // 16 = 3 sendTurn + 3 openModal (incl /diff from 5-C) + 8 openSettings + 2 localAction
    expect(SLASH_COMMANDS.length).toBe(16);
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
    const valid = new Set(['sendTurn', 'openSettings', 'openModal', 'localAction', 'notImplemented']);
    for (const cmd of SLASH_COMMANDS) {
      expect(valid.has(cmd.category)).toBe(true);
    }
  });
});
