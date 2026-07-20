import { useEffect, useState } from 'react';
import { sendRequest } from '../../transport';
import { SettingsListPicker, type LoadState } from '../SettingsListPicker';
import type { HooksListResponse } from '../../types/v2/HooksListResponse';
import type { HooksListEntry } from '../../types/v2/HooksListEntry';

/** Read-only list of configured hooks grouped by cwd (`hooks/list`). Each row
 *  is one cwd's entry; the sub-line summarizes hook count and any warnings.
 *  Read-only in this batch — adding/removing hooks is out of scope. */
export function HooksPanel() {
  const [state, setState] = useState<LoadState<HooksListEntry>>({
    loading: true, error: null, items: [],
  });

  useEffect(() => {
    let cancelled = false;
    setState({ loading: true, error: null, items: [] });
    sendRequest<HooksListResponse>('hooks/list', {})
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
      getId={(e) => e.cwd}
      renderLabel={(e) => e.cwd}
      renderSub={(e) => {
        const parts: string[] = [`${e.hooks.length} hook${e.hooks.length === 1 ? '' : 's'}`];
        if (e.warnings.length > 0) parts.push(`${e.warnings.length} warning${e.warnings.length === 1 ? '' : 's'}`);
        if (e.errors.length > 0) parts.push(`${e.errors.length} error${e.errors.length === 1 ? '' : 's'}`);
        return parts.join(' · ');
      }}
      emptyText="(no hooks configured)"
    />
  );
}
