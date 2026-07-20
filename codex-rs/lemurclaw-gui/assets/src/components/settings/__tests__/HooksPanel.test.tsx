import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { HooksPanel } from '../HooksPanel';

vi.mock('../../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../../transport';

describe('HooksPanel', () => {
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('lists hook entries grouped by cwd', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      data: [
        { cwd: '/repo', hooks: [{ event: 'PreToolUse', command: 'echo hi' } as never], warnings: [], errors: [] },
      ],
    });
    render(<HooksPanel />);
    await waitFor(() => expect(screen.getByText('/repo')).toBeInTheDocument());
    expect(screen.getByText(/1 hook/)).toBeInTheDocument();
  });

  it('shows empty state when no hooks configured', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ data: [] });
    render(<HooksPanel />);
    await waitFor(() => expect(screen.getByText(/no hooks configured/i)).toBeInTheDocument());
  });
});
