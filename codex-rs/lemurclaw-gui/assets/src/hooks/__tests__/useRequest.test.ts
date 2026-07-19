import { describe, it, expect, vi } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useRequestLazy } from '../useRequest';

vi.mock('../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../transport';

describe('useRequestLazy', () => {
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
