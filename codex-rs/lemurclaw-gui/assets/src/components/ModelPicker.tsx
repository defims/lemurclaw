import { useEffect, useState } from 'react';
import { sendRequest, send } from '../transport';
import type { Model } from '../types/v2';
import type { ModelListResponse } from '../types/v2/ModelListResponse';

interface Props {
  /** Active thread id. Sent as thread/start's threadId when switching. */
  threadId: string | null;
  /** Currently selected model id (for highlight). Optional. */
  currentModel?: string | null;
  onClose: () => void;
}

interface LoadState {
  loading: boolean;
  error: string | null;
  models: Model[];
}

/** Modal model picker. On open, calls `model/list` to enumerate available
 *  models; selecting one fires a `turn/start` with `model` override on the
 *  active thread (codex doesn't have a dedicated "switch model mid-thread"
 *  method — the override takes effect on the next turn).
 *
 *  Close: Esc, backdrop click, or ✕ button. */
export function ModelPicker({ threadId, currentModel, onClose }: Props) {
  const [state, setState] = useState<LoadState>({ loading: true, error: null, models: [] });

  useEffect(() => {
    let cancelled = false;
    setState({ loading: true, error: null, models: [] });
    sendRequest<ModelListResponse>('model/list', {})
      .then((resp) => {
        if (!cancelled) setState({ loading: false, error: null, models: resp.data });
      })
      .catch((e) => {
        if (!cancelled) setState({ loading: false, error: e instanceof Error ? e.message : String(e), models: [] });
      });
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') { e.preventDefault(); onClose(); }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  const handlePick = (model: Model) => {
    if (!threadId) return;
    // Switch model via next-turn override. Empty input means "no new user
    // message, just switch model" — codex treats empty input as a no-op turn,
    // so we add a minimal steering placeholder if needed. For now, send an
    // empty text; user can type in Composer after.
    send({
      method: 'turn/start',
      id: Date.now(),
      params: {
        threadId,
        input: [{ type: 'text', text: '', text_elements: [] }],
        model: model.id,
      },
    });
    onClose();
  };

  return (
    <div className="modal-overlay" data-testid="model-picker" onClick={onClose}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()}>
        <header className="modal-header">
          <span className="modal-title">select model</span>
          <button className="modal-close" onClick={onClose} aria-label="close">✕</button>
        </header>
        <div className="modal-body">
          {state.loading && <div className="modal-loading">loading…</div>}
          {state.error && <div className="modal-error">failed: {state.error}</div>}
          {!state.loading && !state.error && state.models.length === 0 && (
            <div className="modal-empty">no models configured</div>
          )}
          {!state.loading && !state.error && state.models.length > 0 && (
            <ul className="model-list">
              {state.models.map((m) => (
                <li
                  key={m.id}
                  className={`model-item${m.id === currentModel ? ' model-item-active' : ''}`}
                >
                  <button onClick={() => handlePick(m)} className="model-item-button" disabled={!threadId}>
                    <span className="model-item-name">{m.displayName || m.id}</span>
                    <span className="model-item-id">{m.id}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
}
