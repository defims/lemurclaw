import { useEffect, useState } from 'react';
import { sendRequest } from '../../transport';
import { SettingsListPicker, type LoadState } from '../SettingsListPicker';
import type { AppsListResponse } from '../../types/v2/AppsListResponse';
import type { AppInfo } from '../../types/v2/AppInfo';

/** Read-only list of registered apps (`app/list`). Each row = one app; the
 *  sub-line shows the description if available. Install/uninstall is out of
 *  scope for this batch.
 *
 *  Note: AppsListResponse / AppInfo are marked EXPERIMENTAL in the protocol,
 *  but the read shape is stable enough to surface here. */
export function AppsPanel() {
  const [state, setState] = useState<LoadState<AppInfo>>({
    loading: true, error: null, items: [],
  });

  useEffect(() => {
    let cancelled = false;
    setState({ loading: true, error: null, items: [] });
    sendRequest<AppsListResponse>('app/list', {})
      .then((resp) => {
        if (cancelled) return;
        setState({ loading: false, error: null, items: resp.data });
      })
      .catch((e) => {
        if (cancelled) setState({ loading: false, error: e instanceof Error ? e.message : String(e), items: [] });
      });
    return () => { cancelled = true; };
  }, []);

  return (
    <SettingsListPicker
      state={state}
      getId={(a) => a.id}
      renderLabel={(a) => a.name}
      renderSub={(a) => a.description ?? undefined}
      emptyText="(no apps registered)"
    />
  );
}
