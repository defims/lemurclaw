import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { PluginsPanel } from '../PluginsPanel';

vi.mock('../../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../../transport';

// Helper: build a PluginListResponse-shaped object with one marketplace
// containing the given plugins.
function listResponse(plugins: Array<{ id: string; name: string; installed: boolean; enabled: boolean }>) {
  return {
    marketplaces: [
      {
        name: 'default',
        path: null,
        interface: null,
        plugins: plugins.map((p) => ({
          id: p.id, remotePluginId: null, version: null, localVersion: null, name: p.name,
          shareContext: null, source: 'marketplace', installed: p.installed, enabled: p.enabled,
          installPolicy: 'allow', installPolicySource: null, mustShowInstallationInterstitial: null,
          authPolicy: 'never', availability: 'available', interface: null, keywords: [],
        })),
      },
    ],
    marketplaceLoadErrors: [],
    featuredPluginIds: [],
  };
}

describe('PluginsPanel', () => {
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('lists plugins flattened from marketplaces and offers uninstall on installed ones', async () => {
    vi.mocked(sendRequest).mockResolvedValue(listResponse([
      { id: 'p1', name: 'Plugin One', installed: true, enabled: true },
      { id: 'p2', name: 'Plugin Two', installed: false, enabled: false },
    ]) as never);
    render(<PluginsPanel />);
    await waitFor(() => expect(screen.getByText('Plugin One')).toBeInTheDocument());
    expect(screen.getByText('Plugin Two')).toBeInTheDocument();
    // Only installed plugins get an uninstall button.
    expect(screen.getByRole('button', { name: /uninstall/i })).toBeInTheDocument();
  });

  it('uninstall button calls plugin/uninstall with pluginId and refetches', async () => {
    vi.mocked(sendRequest)
      .mockResolvedValueOnce(listResponse([{ id: 'p1', name: 'Plugin One', installed: true, enabled: true }]) as never)
      .mockResolvedValueOnce({}) // plugin/uninstall ack
      .mockResolvedValueOnce(listResponse([]) as never); // refetch -> empty
    render(<PluginsPanel />);
    await waitFor(() => expect(screen.getByText('Plugin One')).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: /uninstall/i }));
    await waitFor(() => expect(sendRequest).toHaveBeenCalledWith('plugin/uninstall', { pluginId: 'p1' }));
    await waitFor(() => expect(screen.queryByText('Plugin One')).not.toBeInTheDocument());
  });

  it('shows empty state when no plugins in any marketplace', async () => {
    vi.mocked(sendRequest).mockResolvedValue(listResponse([]) as never);
    render(<PluginsPanel />);
    await waitFor(() => expect(screen.getByText(/no plugins/i)).toBeInTheDocument());
  });
});
