import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SessionPicker } from '../SessionPicker';

vi.mock('../../../hooks/useThreadList', () => ({ useThreadList: vi.fn() }));

import { useThreadList } from '../../../hooks/useThreadList';
import type { Thread } from '../../../types/v2';

function makeThread(over: Partial<Thread> = {}): Thread {
  return {
    id: 't1', sessionId: 's', forkedFromId: null, parentThreadId: null,
    preview: 'hello', ephemeral: false, modelProvider: 'openai',
    createdAt: 1, updatedAt: 1, recencyAt: null, status: { type: 'idle' },
    path: null, cwd: { path: '/x' }, cliVersion: '0', source: 'Cli',
    threadSource: null, agentNickname: null, agentRole: null, gitInfo: null,
    name: null, turns: [], ...over,
  } as Thread;
}

describe('SessionPicker', () => {
  // `afterEach` for consistency with other transport-mocked tests (see Task 4.2).
  // IMPORTANT: do NOT use beforeEach with mockReset — Vitest 2.1.9 has a
  // false-positive unhandled-rejection failure with that combination.
  afterEach(() => {
    vi.mocked(useThreadList).mockReset();
  });

  it('renders threads and highlights active', () => {
    vi.mocked(useThreadList).mockReturnValue({
      threads: [makeThread({ id: 't1', preview: 'first' }), makeThread({ id: 't2', preview: 'second' })],
      loading: false, error: null, nextCursor: null, loadMore: vi.fn(), refresh: vi.fn(),
    } as never);
    render(<SessionPicker activeThreadId="t1" onResume={vi.fn()} />);
    expect(screen.getByText('first')).toBeInTheDocument();
    expect(screen.getByText('second')).toBeInTheDocument();
    expect(screen.getByText('first').closest('.session-item')).toHaveClass('session-item-active');
  });

  it('clicking a session calls onResume with the thread id', () => {
    vi.mocked(useThreadList).mockReturnValue({
      threads: [makeThread({ id: 't9', preview: 'click me' })],
      loading: false, error: null, nextCursor: null, loadMore: vi.fn(), refresh: vi.fn(),
    } as never);
    const onResume = vi.fn();
    render(<SessionPicker activeThreadId={null} onResume={onResume} />);
    fireEvent.click(screen.getByText('click me'));
    expect(onResume).toHaveBeenCalledWith('t9');
  });

  it('shows empty state', () => {
    vi.mocked(useThreadList).mockReturnValue({
      threads: [], loading: false, error: null, nextCursor: null, loadMore: vi.fn(), refresh: vi.fn(),
    } as never);
    render(<SessionPicker activeThreadId={null} onResume={vi.fn()} />);
    expect(screen.getByText('no sessions yet')).toBeInTheDocument();
  });

  it('shows error + retry calls refresh', () => {
    const refresh = vi.fn();
    vi.mocked(useThreadList).mockReturnValue({
      threads: [], loading: false, error: 'network down', nextCursor: null, loadMore: vi.fn(), refresh,
    } as never);
    render(<SessionPicker activeThreadId={null} onResume={vi.fn()} />);
    expect(screen.getByText(/network down/)).toBeInTheDocument();
    fireEvent.click(screen.getByText('retry'));
    expect(refresh).toHaveBeenCalled();
  });
});
