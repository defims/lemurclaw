import { useCallback, useEffect, useState } from 'react';
import { sendRequest } from '../../transport';
import { SettingsListPicker, type LoadState } from '../SettingsListPicker';
import type { PluginListResponse } from '../../types/v2/PluginListResponse';
import type { PluginSummary } from '../../types/v2/PluginSummary';

/** Plugin list (`plugin/list`) with an `uninstall` action per installed plugin.
 *
 *  The response is `{ marketplaces: PluginMarketplaceEntry[] }` where each
 *  marketplace has a `plugins: PluginSummary[]` array — we flatten across all
 *  marketplaces into one row per plugin. Uninstall calls `plugin/uninstall`
 *  with the plugin's `id` (not name) then refetches. Install-from-marketplace
 *  is out of scope for this batch. */
export function PluginsPanel() {
  const [state, setState] = useState<LoadState<PluginSummary>>({
    loading: true, error: null, items: [],
  });

  const fetchAll = useCallback(() => {
    let cancelled = false;
    setState({ loading: true, error: null, items: [] });
    return sendRequest<PluginListResponse>('plugin/list', {})
      .then((resp) => {
        if (cancelled) return;
        const all = resp.marketplaces.flatMap((m) => m.plugins ?? []);
        setState({ loading: false, error: null, items: all });
      })
      .catch((e) => {
        if (!cancelled) setState({ loading: false, error: e instanceof Error ? e.message : String(e), items: [] });
      })
      .finally(() => { cancelled = true; });
  }, []);

  useEffect(() => { fetchAll(); }, [fetchAll]);

  const uninstall = async (pluginId: string) => {
    await sendRequest('plugin/uninstall', { pluginId });
    fetchAll();
  };

  return (
    <SettingsListPicker
      state={state}
      getId={(p) => p.id}
      renderLabel={(p) => p.name}
      renderSub={(p) => p.installed ? 'installed' : 'available'}
      renderAction={(p) =>
        p.installed ? (
          <button className="settings-action-button" onClick={() => uninstall(p.id)}>uninstall</button>
        ) : null
      }
      emptyText="(no plugins in marketplace)"
    />
  );
}
