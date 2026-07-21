import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { PluginsPanel } from '../PluginsPanel';

vi.mock('../../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../../transport';

// Build a PluginListResponse with one local marketplace (path present) and one
// remote marketplace (path null). Plugins inherit their marketplace's context.
function listResponse(opts: {
  local?: Array<{ id: string; name: string; installed: boolean }>;
  remote?: Array<{ id: string; name: string; installed: boolean }>;
}) {
  const mk = (plugins: Array<{ id: string; name: string; installed: boolean }>) =>
    plugins.map((p) => ({
      id: p.id, remotePluginId: null, version: null, localVersion: null, name: p.name,
      shareContext: null, source: 'marketplace', installed: p.installed, enabled: p.installed,
      installPolicy: 'allow', installPolicySource: null, mustShowInstallationInterstitial: null,
      authPolicy: 'never', availability: 'available', interface: null, keywords: [],
    }));
  return {
    marketplaces: [
      {
        name: 'local-market',
        path: { path: '/home/.codex/marketplaces/local.json' },
        interface: null,
        plugins: mk(opts.local ?? []),
      },
      {
        name: 'remote-market',
        path: null,
        interface: null,
        plugins: mk(opts.remote ?? []),
      },
    ],
    marketplaceLoadErrors: [],
    featuredPluginIds: [],
  };
}

describe('PluginsPanel', () => {
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('lists plugins flattened across local + remote marketplaces', async () => {
    vi.mocked(sendRequest).mockResolvedValue(listResponse({
      local: [{ id: 'p1', name: 'Local Plugin', installed: true }],
      remote: [{ id: 'p2', name: 'Remote Plugin', installed: false }],
    }) as never);
    render(<PluginsPanel />);
    await waitFor(() => expect(screen.getByText('Local Plugin')).toBeInTheDocument());
    expect(screen.getByText('Remote Plugin')).toBeInTheDocument();
  });

  it('installed plugins get uninstall, uninstalled get install', async () => {
    vi.mocked(sendRequest).mockResolvedValue(listResponse({
      local: [{ id: 'p1', name: 'One', installed: true }],
      remote: [{ id: 'p2', name: 'Two', installed: false }],
    }) as never);
    render(<PluginsPanel />);
    await waitFor(() => expect(screen.getByText('One')).toBeInTheDocument());
    expect(screen.getByRole('button', { name: 'uninstall' })).toBeInTheDocument();
    // exact match on "install" so it doesn't also match "uninstall"
    expect(screen.getByRole('button', { name: (n) => n === 'install' })).toBeInTheDocument();
  });

  it('uninstall calls plugin/uninstall with pluginId and refetches', async () => {
    vi.mocked(sendRequest)
      .mockResolvedValueOnce(listResponse({ local: [{ id: 'p1', name: 'One', installed: true }] }) as never)
      .mockResolvedValueOnce({}) // plugin/uninstall ack
      .mockResolvedValueOnce(listResponse({}) as never); // refetch -> empty
    render(<PluginsPanel />);
    await waitFor(() => expect(screen.getByText('One')).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: /uninstall/i }));
    await waitFor(() => expect(sendRequest).toHaveBeenCalledWith('plugin/uninstall', { pluginId: 'p1' }));
    await waitFor(() => expect(screen.queryByText('One')).not.toBeInTheDocument());
  });

  it('install on local marketplace uses marketplacePath selector', async () => {
    vi.mocked(sendRequest)
      .mockResolvedValueOnce(listResponse({ local: [{ id: 'p1', name: 'LocalOne', installed: false }] }) as never)
      .mockResolvedValueOnce({ authPolicy: 'never', appsNeedingAuth: [] }) // install ack
      .mockResolvedValueOnce(listResponse({ local: [{ id: 'p1', name: 'LocalOne', installed: true }] }) as never);
    render(<PluginsPanel />);
    await waitFor(() => expect(screen.getByText('LocalOne')).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: /install/i }));
    await waitFor(() => {
      expect(sendRequest).toHaveBeenCalledWith('plugin/install', {
        marketplacePath: { path: '/home/.codex/marketplaces/local.json' },
        pluginName: 'LocalOne',
      });
    });
    // After refetch the button flips to uninstall.
    await waitFor(() => expect(screen.getByRole('button', { name: /uninstall/i })).toBeInTheDocument());
  });

  it('install on remote marketplace uses remoteMarketplaceName selector', async () => {
    vi.mocked(sendRequest)
      .mockResolvedValueOnce(listResponse({ remote: [{ id: 'p2', name: 'RemoteOne', installed: false }] }) as never)
      .mockResolvedValueOnce({ authPolicy: 'never', appsNeedingAuth: [] })
      .mockResolvedValueOnce(listResponse({ remote: [{ id: 'p2', name: 'RemoteOne', installed: true }] }) as never);
    render(<PluginsPanel />);
    await waitFor(() => expect(screen.getByText('RemoteOne')).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: /install/i }));
    await waitFor(() => {
      expect(sendRequest).toHaveBeenCalledWith('plugin/install', {
        remoteMarketplaceName: 'remote-market',
        pluginName: 'RemoteOne',
      });
    });
  });

  it('shows empty state when no plugins in any marketplace', async () => {
    vi.mocked(sendRequest).mockResolvedValue(listResponse({}) as never);
    render(<PluginsPanel />);
    await waitFor(() => expect(screen.getByText(/no plugins/i)).toBeInTheDocument());
  });
});
