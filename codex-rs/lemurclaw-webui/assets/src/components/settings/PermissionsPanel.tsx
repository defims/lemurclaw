import { useEffect, useState } from 'react';
import { sendRequest } from '../../transport';
import { SettingsListPicker, type LoadState } from '../SettingsListPicker';
import type { PermissionProfileListResponse } from '../../types/v2/PermissionProfileListResponse';
import type { PermissionProfileSummary } from '../../types/v2/PermissionProfileSummary';

/** Read-only list of permission profiles from `permissionProfile/list`.
 *  Profiles with `allowed: false` are shown disabled (the effective
 *  requirements forbid selecting them). No activation — switching the active
 *  profile is a separate RPC not in this batch. */
export function PermissionsPanel() {
  const [state, setState] = useState<LoadState<PermissionProfileSummary>>({
    loading: true, error: null, items: [],
  });

  useEffect(() => {
    let cancelled = false;
    setState({ loading: true, error: null, items: [] });
    sendRequest<PermissionProfileListResponse>('permissionProfile/list', {})
      .then((resp) => {
        if (!cancelled) setState({ loading: false, error: null, items: resp.data });
      })
      .catch((e) => {
        if (!cancelled) setState({ loading: false, error: e instanceof Error ? e.message : String(e), items: [] });
      });
    return () => { cancelled = true; };
  }, []);

  return (
    <SettingsListPicker
      state={state}
      getId={(p) => p.id}
      renderLabel={(p) => p.id}
      renderSub={(p) => p.description}
      isDisabled={(p) => !p.allowed}
      emptyText="(no permission profiles configured)"
    />
  );
}
