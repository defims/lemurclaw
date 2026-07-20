import { useEffect, useState } from 'react';
import { sendRequest } from '../../transport';
import type { ExperimentalFeatureListResponse } from '../../types/v2/ExperimentalFeatureListResponse';
import type { ExperimentalFeature } from '../../types/v2/ExperimentalFeature';

interface LoadState { loading: boolean; error: string | null; features: ExperimentalFeature[] }

/** Experimental feature toggle panel. Loads `experimentalFeature/list` once;
 *  each row has a checkbox bound to its `enabled` flag. Toggling fires
 *  `experimentalFeature/enablement/set` with the single delta and updates local
 *  state optimistically; reverts + surfaces the error on failure (the response
 *  is ack-only, so no refetch). */
export function ExperimentalPanel() {
  const [state, setState] = useState<LoadState>({ loading: true, error: null, features: [] });

  useEffect(() => {
    let cancelled = false;
    setState({ loading: true, error: null, features: [] });
    sendRequest<ExperimentalFeatureListResponse>('experimentalFeature/list', {})
      .then((resp) => {
        if (cancelled) return;
        setState({ loading: false, error: null, features: resp.data });
      })
      .catch((e) => {
        if (cancelled) return;
        setState({ loading: false, error: e instanceof Error ? e.message : String(e), features: [] });
      });
    return () => { cancelled = true; };
  }, []);

  const toggle = (feat: ExperimentalFeature, next: boolean) => {
    // Optimistic local update; the panel state is the source of truth for the
    // checkbox between server round-trips.
    setState((s) => ({
      ...s,
      features: s.features.map((f) => f.name === feat.name ? { ...f, enabled: next } : f),
    }));
    sendRequest('experimentalFeature/enablement/set', { enablement: { [feat.name]: next } }).catch((e) => {
      // Revert on failure + surface the error in the panel.
      setState((s) => ({
        ...s,
        error: e instanceof Error ? e.message : String(e),
        features: s.features.map((f) => f.name === feat.name ? { ...f, enabled: !next } : f),
      }));
    });
  };

  if (state.loading) return <div className="modal-loading">loading…</div>;
  if (state.error) return <div className="modal-error">failed: {state.error}</div>;
  if (state.features.length === 0) return <div className="modal-empty">(no experimental features)</div>;

  return (
    <ul className="settings-list">
      {state.features.map((f) => (
        <li key={f.name} className="settings-list-item">
          <label className="experimental-feature-row">
            <input
              type="checkbox"
              data-testid={`toggle-${f.name}`}
              checked={f.enabled}
              onChange={(e) => toggle(f, e.target.checked)}
            />
            <span className="settings-list-item-label">
              {f.displayName ?? f.name}
              {f.stage !== 'beta' && <span className="experimental-feature-stage"> · {f.stage}</span>}
            </span>
            {f.description && <span className="settings-list-item-sub">{f.description}</span>}
          </label>
        </li>
      ))}
    </ul>
  );
}
