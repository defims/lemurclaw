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
  sendRequest: vi.fn(),
}));

import { useConversation } from '../useConversation';
import { send, sendRequest } from '../../transport';

function emit(ev: unknown): void {
  if (!onEventCb) throw new Error('onEvent not installed');
  act(() => {
    onEventCb!(ev);
  });
}

describe('useConversation', () => {
  afterEach(() => {
    vi.mocked(send).mockClear();
    vi.mocked(sendRequest).mockReset();
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

  it('startTurn dispatches responseMeta with cwd + model from response', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ model: 'gpt-4o', cwd: '/proj' } as never);
    const { result } = renderHook(() => useConversation());
    emit({ method: 'turn/started', params: { threadId: 't1', turn: { id: 'tu1', items: [] } } });
    await act(async () => {
      await result.current.startTurn([{ type: 'text', text: 'hi', text_elements: [] }]);
    });
    expect(result.current.state.cwd).toBe('/proj');
    expect(result.current.state.currentModel).toBe('gpt-4o');
  });

  it('resumeThread sets threadId and dispatches responseMeta', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ model: 'claude-3', cwd: '/other' } as never);
    const { result } = renderHook(() => useConversation());
    emit({ method: 'turn/started', params: { threadId: 't1', turn: { id: 'tu1', items: [] } } });
    await act(async () => { await result.current.resumeThread('t2'); });
    expect(result.current.threadId).toBe('t2');
    expect(result.current.state.cwd).toBe('/other');
    expect(result.current.state.currentModel).toBe('claude-3');
  });
});
