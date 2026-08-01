import { useCallback, useEffect, useState } from 'react';
import { sendRequest } from '../../transport';
import { SettingsListPicker, type LoadState } from '../SettingsListPicker';
import type { SkillsListResponse } from '../../types/v2/SkillsListResponse';
import type { SkillsConfigWriteParams } from '../../types/v2/SkillsConfigWriteParams';
import type { SkillMetadata } from '../../types/v2/SkillMetadata';

/** Skills panel (`skills/list`), flattened across cwds. Each row = one skill;
 *  sub-line shows the description (or cwd). The trailing action is an
 *  enable/disable toggle that fires `skills/config/write` with the skill's
 *  name + cwd and refetches the list on success.
 *
 *  Note: a skill is identified by (cwd, name). The skills/config/write RPC
 *  accepts either a path or a name; we send `{ name, enabled }` and let the
 *  server resolve. If two skills with the same name exist across cwds the
 *  toggle is ambiguous server-side, but that's a rare config error and the
 *  refetch will surface whatever the server picked. */
interface SkillRow {
  name: string;
  description: string;
  cwd: string;
  enabled: boolean;
}

export function SkillsPanel() {
  const [state, setState] = useState<LoadState<SkillRow>>({
    loading: true, error: null, items: [],
  });

  const fetchAll = useCallback(() => {
    let cancelled = false;
    setState({ loading: true, error: null, items: [] });
    return sendRequest<SkillsListResponse>('skills/list', {})
      .then((resp) => {
        if (cancelled) return;
        const rows: SkillRow[] = resp.data.flatMap((entry) =>
          (entry.skills ?? []).map((s: SkillMetadata) => ({
            name: s.name,
            description: s.description,
            cwd: entry.cwd,
            enabled: s.enabled,
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

  const toggle = async (row: SkillRow, enabled: boolean) => {
    const params: SkillsConfigWriteParams = { name: row.name, enabled };
    await sendRequest('skills/config/write', params);
    fetchAll();
  };

  return (
    <SettingsListPicker
      state={state}
      getId={(r) => `${r.cwd}::${r.name}`}
      renderLabel={(r) => r.name}
      renderSub={(r) => r.description || r.cwd}
      renderAction={(r) => (
        <button
          className="settings-action-button"
          onClick={() => toggle(r, !r.enabled)}
          aria-pressed={r.enabled}
        >
          {r.enabled ? 'disable' : 'enable'}
        </button>
      )}
      emptyText="(no skills discovered)"
    />
  );
}
