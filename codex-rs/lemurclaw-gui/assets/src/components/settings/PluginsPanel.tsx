import { useCallback, useEffect, useState } from 'react';
import { sendRequest } from '../../transport';
import { SettingsListPicker, type LoadState } from '../SettingsListPicker';
import type { PluginListResponse } from '../../types/v2/PluginListResponse';
import type { PluginSummary } from '../../types/v2/PluginSummary';
import type { PluginInstallParams } from '../../types/v2/PluginInstallParams';
import type { AbsolutePathBuf } from '../../types/AbsolutePathBuf';

/** Plugin list (`plugin/list`) with per-row install / uninstall actions.
 *
 *  The response is `{ marketplaces: PluginMarketplaceEntry[] }` where each
 *  marketplace has a `plugins: PluginSummary[]` array. We flatten across
 *  marketplaces into one row per plugin, keeping the marketplace context
 *  (path for local, name for remote) so install can address it.
 *
 *  - Installed plugins get an `uninstall` button → `plugin/uninstall { pluginId }`.
 *  - Uninstalled plugins get an `install` button → `plugin/install` with
 *    `{ marketplacePath }` (local) or `{ remoteMarketplaceName }` (remote)
 *    plus `{ pluginName }`.
 *  Both actions refetch the list on success. No optimistic update — install
 *  can require auth (PluginInstallResponse.appsNeedingAuth), which is out of
 *  scope here; we just refetch and let the server's `installed` flag settle. */
interface PluginRow {
  /** PluginSummary.id — the stable plugin identifier used for uninstall. */
  id: string;
  /** PluginSummary.name — the human/plugin name used for install. */
  name: string;
  installed: boolean;
  /** Local marketplace file path (PluginMarketplaceEntry.path). Null for
   *  remote-only catalog marketplaces. */
  marketplacePath: AbsolutePathBuf | null;
  /** Marketplace name (PluginMarketplaceEntry.name) — used as
   *  remoteMarketplaceName when path is null. */
  marketplaceName: string;
}

export function PluginsPanel() {
  const [state, setState] = useState<LoadState<PluginRow>>({
    loading: true, error: null, items: [],
  });

  const fetchAll = useCallback(() => {
    let cancelled = false;
    setState({ loading: true, error: null, items: [] });
    return sendRequest<PluginListResponse>('plugin/list', {})
      .then((resp) => {
        if (cancelled) return;
        const rows: PluginRow[] = resp.marketplaces.flatMap((m) =>
          (m.plugins ?? []).map((p: PluginSummary) => ({
            id: p.id,
            name: p.name,
            installed: p.installed,
            marketplacePath: m.path,
            marketplaceName: m.name,
          })),
        );
        setState({ loading: false, error: null, items: rows });
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

  const install = async (row: PluginRow) => {
    const params: PluginInstallParams = row.marketplacePath
      ? { marketplacePath: row.marketplacePath, pluginName: row.name }
      : { remoteMarketplaceName: row.marketplaceName, pluginName: row.name };
    await sendRequest('plugin/install', params);
    fetchAll();
  };

  return (
    <SettingsListPicker
      state={state}
      getId={(r) => r.id}
      renderLabel={(r) => r.name}
      renderSub={(r) => r.installed ? 'installed' : 'available'}
      renderAction={(r) => r.installed
        ? <button className="settings-action-button" onClick={() => uninstall(r.id)}>uninstall</button>
        : <button className="settings-action-button" onClick={() => install(r)}>install</button>
      }
      emptyText="(no plugins in marketplace)"
    />
  );
}
