import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { AppsPanel } from '../AppsPanel';

vi.mock('../../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../../transport';

describe('AppsPanel', () => {
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('lists apps from app/list', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      data: [
        {
          id: 'my-app',
          name: 'My App',
          description: 'An app',
          logoUrl: null, logoUrlDark: null, iconAssets: null, iconDarkAssets: null,
          distributionChannel: null, branding: null, appMetadata: null, labels: null,
          installUrl: null, isAccessible: true, isEnabled: true, pluginDisplayNames: [],
        } as never,
      ],
      nextCursor: null,
    });
    render(<AppsPanel />);
    await waitFor(() => expect(screen.getByText('My App')).toBeInTheDocument());
    expect(screen.getByText('An app')).toBeInTheDocument();
  });

  it('shows empty state when no apps registered', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ data: [], nextCursor: null });
    render(<AppsPanel />);
    await waitFor(() => expect(screen.getByText(/no apps registered/i)).toBeInTheDocument());
  });
});
