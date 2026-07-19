import { describe, it, expect } from 'vitest';
import { isServerNotification, isServerRequest, hasMethod } from './guards';

describe('guards', () => {
  it('isServerNotification accepts a method+params envelope', () => {
    expect(isServerNotification({ method: 'error', params: { message: 'x' } })).toBe(true);
  });

  it('isServerRequest accepts method+params+id, isServerNotification rejects it', () => {
    const sr = {
      method: 'item/commandExecution/requestApproval',
      id: 5,
      params: { threadId: 't' },
    };
    expect(isServerRequest(sr)).toBe(true);
    expect(isServerNotification(sr)).toBe(false);
  });

  it('isServerNotification rejects non-objects', () => {
    expect(isServerNotification(null)).toBe(false);
    expect(isServerNotification('hello')).toBe(false);
    expect(isServerNotification(42)).toBe(false);
  });

  it('hasMethod narrows to the picked variant', () => {
    const ev = {
      method: 'turn/started',
      params: {
        threadId: 't',
        turn: { id: 'tu', items: [], itemsView: 'full', status: 'inProgress', error: null, startedAt: null, completedAt: null, durationMs: null },
      },
    } as const;
    expect(hasMethod(ev as never, 'turn/started')).toBe(true);
  });
});
