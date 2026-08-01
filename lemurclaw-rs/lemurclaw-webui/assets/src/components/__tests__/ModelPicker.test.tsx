import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ModelPicker } from '../ModelPicker';

vi.mock('../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../transport';

describe('ModelPicker', () => {
  // See useRequest.test.ts (Task 4.2) for why this is `afterEach` not `beforeEach`.
  // CRITICAL: do NOT switch to beforeEach — Vitest 2.1.9 false-positives.
  afterEach(() => {
    vi.mocked(sendRequest).mockReset();
  });

  it('loads models and renders list', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      data: [
        { id: 'gpt-4', displayName: 'GPT-4' } as never,
        { id: 'gpt-3.5-turbo', displayName: 'GPT-3.5' } as never,
      ],
      nextCursor: null,
    });
    render(<ModelPicker threadId="t1" onClose={vi.fn()} startTurn={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('GPT-4')).toBeInTheDocument());
    expect(screen.getByText('GPT-3.5')).toBeInTheDocument();
  });

  it('picking a model calls startTurn with empty input + model override', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      data: [{ id: 'claude-3', displayName: 'Claude 3' } as never], nextCursor: null,
    });
    const onClose = vi.fn();
    const startTurn = vi.fn();
    render(<ModelPicker threadId="t1" onClose={onClose} startTurn={startTurn} />);
    await waitFor(() => expect(screen.getByText('Claude 3')).toBeInTheDocument());
    fireEvent.click(screen.getByText('Claude 3'));
    expect(startTurn).toHaveBeenCalledWith(
      [{ type: 'text', text: '', text_elements: [] }],
      'claude-3',
    );
    expect(onClose).toHaveBeenCalled();
  });

  it('disables picking when threadId is null', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      data: [{ id: 'm1', displayName: 'M1' } as never], nextCursor: null,
    });
    render(<ModelPicker threadId={null} onClose={vi.fn()} startTurn={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('M1')).toBeInTheDocument());
    expect(screen.getByText('M1').closest('button')).toBeDisabled();
  });

  it('Esc closes', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ data: [], nextCursor: null });
    const onClose = vi.fn();
    render(<ModelPicker threadId="t1" onClose={onClose} startTurn={vi.fn()} />);
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });

  it('backdrop click closes', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ data: [], nextCursor: null });
    const onClose = vi.fn();
    const { container } = render(<ModelPicker threadId="t1" onClose={onClose} startTurn={vi.fn()} />);
    fireEvent.click(container.querySelector('.modal-overlay')!);
    expect(onClose).toHaveBeenCalled();
  });

  it('shows error state', async () => {
    vi.mocked(sendRequest).mockRejectedValue(new Error('model service down'));
    render(<ModelPicker threadId="t1" onClose={vi.fn()} startTurn={vi.fn()} />);
    await waitFor(() => expect(screen.getByText(/model service down/)).toBeInTheDocument());
  });

  it('shows empty state when no models configured', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ data: [], nextCursor: null });
    render(<ModelPicker threadId="t1" onClose={vi.fn()} startTurn={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('no models configured')).toBeInTheDocument());
  });
});
