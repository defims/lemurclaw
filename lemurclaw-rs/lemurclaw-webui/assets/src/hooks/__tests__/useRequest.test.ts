import { describe, it, expect, vi, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useRequestLazy } from '../useRequest';

vi.mock('../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../transport';

// NOTE: use `afterEach`, not `beforeEach`, for `mockReset()` here.
// Vitest 2.1.9 has a false-positive unhandled-rejection failure when
// `beforeEach(() => mockReset())` is combined with `mockRejectedValue`:
// the reset runs before the test body, and Vitest's mock/unhandled-rejection
// interaction attributes the rejected promise to the `new Error(...)` site
// even though the hook's `.catch` consumes it. `afterEach` runs cleanup
// *after* the test body, so the rejection is already settled. This rule
// applies to all transport-mocked tests in this subproject.
describe('useRequestLazy', () => {
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('transitions through loading → data on success', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ ok: true } as never);
    const { result } = renderHook(() => useRequestLazy());
    expect(result.current.data).toBeNull();
    let p!: Promise<unknown>;
    act(() => { p = result.current.run('thread/list', {}); });
    expect(result.current.loading).toBe(true);
    await act(async () => { await p; });
    expect(result.current.loading).toBe(false);
    expect(result.current.data).toEqual({ ok: true });
    expect(result.current.error).toBeNull();
  });

  it('captures error on failure', async () => {
    vi.mocked(sendRequest).mockRejectedValue(new Error('boom'));
    const { result } = renderHook(() => useRequestLazy());
    let p!: Promise<unknown>;
    act(() => { p = result.current.run('thread/list', {}); });
    await act(async () => { await p; });
    expect(result.current.error).toBe('boom');
    expect(result.current.data).toBeNull();
  });
});
