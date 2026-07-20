import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { ExperimentalPanel } from '../ExperimentalPanel';

vi.mock('../../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../../transport';

describe('ExperimentalPanel', () => {
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('lists features with toggle state matching enabled', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      data: [
        { name: 'feat-a', stage: 'beta', displayName: 'Feat A', description: 'desc', announcement: null, enabled: true, defaultEnabled: false },
        { name: 'feat-b', stage: 'alpha', displayName: null, description: null, announcement: null, enabled: false, defaultEnabled: false },
      ],
      nextCursor: null,
    });
    render(<ExperimentalPanel />);
    await waitFor(() => expect(screen.getByText('Feat A')).toBeInTheDocument());
    expect(screen.getByTestId('toggle-feat-a')).toBeChecked();
    expect(screen.getByTestId('toggle-feat-b')).not.toBeChecked();
  });

  it('toggling fires experimentalFeature/enablement/set', async () => {
    vi.mocked(sendRequest)
      .mockResolvedValueOnce({
        data: [{ name: 'feat-a', stage: 'beta', displayName: 'Feat A', description: null, announcement: null, enabled: false, defaultEnabled: false }],
        nextCursor: null,
      })
      .mockResolvedValueOnce({}); // enablement/set ack
    render(<ExperimentalPanel />);
    await waitFor(() => expect(screen.getByText('Feat A')).toBeInTheDocument());
    fireEvent.click(screen.getByTestId('toggle-feat-a'));
    await waitFor(() => {
      expect(sendRequest).toHaveBeenCalledWith('experimentalFeature/enablement/set', { enablement: { 'feat-a': true } });
    });
  });

  it('shows error state', async () => {
    vi.mocked(sendRequest).mockRejectedValue(new Error('boom'));
    render(<ExperimentalPanel />);
    await waitFor(() => expect(screen.getByText(/boom/)).toBeInTheDocument());
  });
});
