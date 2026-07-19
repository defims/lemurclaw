import { describe, it, expect, vi } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { useThreadList } from '../useThreadList';

vi.mock('../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../transport';

function makeThread(id: string) {
  return { id, sessionId: 's', forkedFromId: null, parentThreadId: null, preview: `t-${id}`,
    ephemeral: false, modelProvider: 'p', createdAt: 1, updatedAt: 1, recencyAt: null,
    status: { type: 'idle' }, path: null, cwd: { path: '/x' }, cliVersion: '0', source: 'Cli',
    threadSource: null, agentNickname: null, agentRole: null, gitInfo: null, name: null, turns: [] } as never;
}

describe('useThreadList', () => {
  it('auto-loads first page on mount', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ data: [makeThread('1'), makeThread('2')], nextCursor: null, backwardsCursor: null });
    const { result } = renderHook(() => useThreadList());
    await waitFor(() => expect(result.current.threads).toHaveLength(2));
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('loadMore appends next page', async () => {
    vi.mocked(sendRequest)
      .mockResolvedValueOnce({ data: [makeThread('1')], nextCursor: 'cur', backwardsCursor: null })
      .mockResolvedValueOnce({ data: [makeThread('2')], nextCursor: null, backwardsCursor: null });
    const { result } = renderHook(() => useThreadList());
    await waitFor(() => expect(result.current.threads).toHaveLength(1));
    await act(async () => { await result.current.loadMore(); });
    expect(result.current.threads.map((t) => t.id)).toEqual(['1', '2']);
  });

  it('refresh replaces list', async () => {
    vi.mocked(sendRequest)
      .mockResolvedValueOnce({ data: [makeThread('1')], nextCursor: null, backwardsCursor: null })
      .mockResolvedValueOnce({ data: [makeThread('9')], nextCursor: null, backwardsCursor: null });
    const { result } = renderHook(() => useThreadList());
    await waitFor(() => expect(result.current.threads).toHaveLength(1));
    await act(async () => { await result.current.refresh(); });
    expect(result.current.threads.map((t) => t.id)).toEqual(['9']);
  });
});
