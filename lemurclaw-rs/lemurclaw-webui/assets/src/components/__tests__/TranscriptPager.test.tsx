import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { TranscriptPager } from '../TranscriptPager';

vi.mock('../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../transport';

const TURN_WITH_ITEMS = {
  id: 'tu1', status: 'completed' as const, itemsView: 'full' as const,
  error: null, startedAt: 1, completedAt: 2, durationMs: 1000,
  items: [
    { type: 'userMessage', id: 'u1', clientId: null, content: [{ type: 'text', text: 'hi', text_elements: [] }] },
    { type: 'agentMessage', id: 'a1', text: 'hello', phase: 'final_answer', memoryCitation: null },
  ],
};

const FULL_THREAD = {
  id: 't1', sessionId: 's', forkedFromId: null, parentThreadId: null, preview: 'hello thread',
  ephemeral: false, modelProvider: 'openai', createdAt: 1, updatedAt: 1, recencyAt: null,
  status: { type: 'idle' }, path: null, cwd: { path: '/x' }, cliVersion: '0', source: 'Cli',
  threadSource: null, agentNickname: null, agentRole: null, gitInfo: null, name: null,
  turns: [TURN_WITH_ITEMS],
};

describe('TranscriptPager', () => {
  // See useRequest.test.ts (Task 4.2) for why this is `afterEach` not `beforeEach`.
  // CRITICAL: do NOT switch to beforeEach — Vitest 2.1.9 false-positives.
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('loads thread on mount and renders cells via the shared CellRenderer', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ thread: FULL_THREAD } as never);
    render(<TranscriptPager threadId="t1" onClose={vi.fn()} />);
    // The userMessage cell renders with data-testid="user-message" (UserMessageCell).
    // The agentMessage cell renders with data-testid="agent-message" (AgentMessageCell).
    // These assertions confirm the real cell components are used (not a JSON dump).
    await waitFor(() => expect(screen.getByTestId('user-message')).toBeInTheDocument());
    expect(screen.getByTestId('agent-message')).toBeInTheDocument();
    expect(screen.getByTestId('user-message')).toHaveTextContent('hi');
    expect(screen.getByTestId('agent-message')).toHaveTextContent('hello');
  });

  it('Esc calls onClose', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ thread: FULL_THREAD } as never);
    const onClose = vi.fn();
    render(<TranscriptPager threadId="t1" onClose={onClose} />);
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });

  it('backdrop click calls onClose', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ thread: FULL_THREAD } as never);
    const onClose = vi.fn();
    const { container } = render(<TranscriptPager threadId="t1" onClose={onClose} />);
    fireEvent.click(container.querySelector('.transcript-pager-overlay')!);
    expect(onClose).toHaveBeenCalled();
  });

  it('shows error state', async () => {
    vi.mocked(sendRequest).mockRejectedValue(new Error('network down'));
    render(<TranscriptPager threadId="t1" onClose={vi.fn()} />);
    await waitFor(() => expect(screen.getByText(/network down/)).toBeInTheDocument());
  });
});
