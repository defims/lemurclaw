import { describe, it, expect, vi, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';

// Capture the onEvent callback so tests can feed it events, the same way the
// Rust backend's evaluate_script would. We install a fresh capture per test.
let onEventCb: ((ev: unknown) => void) | null = null;
vi.mock('../../transport', () => ({
  onEvent: (cb: (ev: unknown) => void) => {
    onEventCb = cb;
  },
  send: vi.fn(),
  registerResponseHandler: vi.fn(),
}));

import { useConversation } from '../useConversation';
import { send } from '../../transport';

function emit(ev: unknown): void {
  if (!onEventCb) throw new Error('onEvent not installed');
  act(() => {
    onEventCb!(ev);
  });
}

describe('useConversation', () => {
  afterEach(() => {
    vi.mocked(send).mockClear();
    onEventCb = null;
  });

  it('captures the threadId from the first thread-carrying event', () => {
    const { result } = renderHook(() => useConversation());
    emit({ method: 'turn/started', params: { threadId: 't1', turn: { id: 'tu1', items: [] } } });
    expect(result.current.threadId).toBe('t1');
  });

  it('tracks the latest threadId when a second thread/started arrives (session switch)', () => {
    // Regression guard: an earlier version guarded with `!threadIdRef.current`,
    // which pinned the ref to the first thread and broke session switching —
    // the SessionPicker active highlight, the composer's threadId, and the
    // interrupt callback all stayed stuck on the old thread.
    const { result } = renderHook(() => useConversation());
    emit({ method: 'turn/started', params: { threadId: 't1', turn: { id: 'tu1', items: [] } } });
    expect(result.current.threadId).toBe('t1');
    // Simulate thread/resume → thread/started for a different thread.
    emit({ method: 'turn/started', params: { threadId: 't2', turn: { id: 'tu2', items: [] } } });
    expect(result.current.threadId).toBe('t2');
  });

  it('interrupt uses the current (post-switch) threadId', () => {
    const { result } = renderHook(() => useConversation());
    emit({ method: 'turn/started', params: { threadId: 't1', turn: { id: 'tu1', items: [] } } });
    emit({ method: 'turn/started', params: { threadId: 't2', turn: { id: 'tu2', items: [] } } });

    act(() => {
      result.current.interrupt();
    });

    expect(send).toHaveBeenCalledWith(
      expect.objectContaining({
        method: 'turn/interrupt',
        params: { threadId: 't2', turnId: 'tu2' },
      }),
    );
  });
});
