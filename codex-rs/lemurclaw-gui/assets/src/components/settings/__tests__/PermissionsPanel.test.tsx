import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { PermissionsPanel } from '../PermissionsPanel';

vi.mock('../../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../../transport';

describe('PermissionsPanel', () => {
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('lists permission profiles and marks disallowed ones disabled', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      data: [
        { id: 'default', description: 'Default', allowed: true },
        { id: 'restricted', description: 'Restricted', allowed: false },
      ],
      nextCursor: null,
    });
    render(<PermissionsPanel />);
    await waitFor(() => expect(screen.getByText('Default')).toBeInTheDocument());
    expect(screen.getByText('restricted').closest('.settings-list-item')).toHaveClass('settings-list-item-disabled');
  });

  it('shows error state', async () => {
    vi.mocked(sendRequest).mockRejectedValue(new Error('boom'));
    render(<PermissionsPanel />);
    await waitFor(() => expect(screen.getByText(/boom/)).toBeInTheDocument());
  });
});
