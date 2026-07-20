import { useEffect, useState } from 'react';
import { sendRequest } from '../../transport';
import { SettingsListPicker, type LoadState } from '../SettingsListPicker';
import type { SkillsListResponse } from '../../types/v2/SkillsListResponse';
import type { SkillMetadata } from '../../types/v2/SkillMetadata';

/** Read-only list of discovered skills (`skills/list`), flattened across cwds.
 *  Each row = one skill; sub-line shows the description (if any) and the cwd
 *  it was discovered under. Editing skills (enable/disable, extra roots) is
 *  out of scope for this batch. */
interface SkillRow { name: string; description: string; cwd: string }

export function SkillsPanel() {
  const [state, setState] = useState<LoadState<SkillRow>>({
    loading: true, error: null, items: [],
  });

  useEffect(() => {
    let cancelled = false;
    setState({ loading: true, error: null, items: [] });
    sendRequest<SkillsListResponse>('skills/list', {})
      .then((resp) => {
        if (cancelled) return;
        const rows: SkillRow[] = resp.data.flatMap((entry) =>
          (entry.skills ?? []).map((s: SkillMetadata) => ({
            name: s.name,
            description: s.description,
            cwd: entry.cwd,
          })),
        );
        setState({ loading: false, error: null, items: rows });
      })
      .catch((e) => {
        if (cancelled) return;
        setState({ loading: false, error: e instanceof Error ? e.message : String(e), items: [] });
      });
    return () => { cancelled = true; };
  }, []);

  return (
    <SettingsListPicker
      state={state}
      getId={(r) => `${r.cwd}::${r.name}`}
      renderLabel={(r) => r.name}
      renderSub={(r) => r.description || r.cwd}
      emptyText="(no skills discovered)"
    />
  );
}
