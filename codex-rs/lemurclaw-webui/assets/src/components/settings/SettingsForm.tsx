import { useEffect, useState } from 'react';
import { sendRequest } from '../../transport';
import type { ConfigReadResponse } from '../../types/v2/ConfigReadResponse';

interface Props {
  /** camelCase wire field name on the Config object (e.g. "developerInstructions").
   *  Used to read the current value out of config/read's response. */
  configKey: string;
  /** snake_case TOML key path for config/value/write (e.g. "developer_instructions").
   *  Defaults to the same string as configKey when the wire name matches the TOML key. */
  writeKeyPath?: string;
  /** Visible label above the textarea. */
  label: string;
  /** Optional helper text below the label. */
  hint?: string;
}

/** Generic single-value config editor. Reads the wire `configKey` from
 *  `config/read`, shows the value as a string in a textarea, and writes back
 *  via `config/value/write` (with `writeKeyPath` as the TOML key path, defaulting
 *  to configKey). Revert restores the last-loaded value.
 *
 *  Values are treated as opaque strings — no schema validation here; callers
 *  that care can wrap with their own validator. */
function getField(obj: unknown, key: string): unknown {
  if (obj && typeof obj === 'object' && key in (obj as Record<string, unknown>)) {
    return (obj as Record<string, unknown>)[key];
  }
  return undefined;
}

export function SettingsForm({ configKey, writeKeyPath, label, hint }: Props) {
  const path = writeKeyPath ?? configKey;
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [loaded, setLoaded] = useState<string>('');
  const [draft, setDraft] = useState<string>('');

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    sendRequest<ConfigReadResponse>('config/read', {})
      .then((resp) => {
        if (cancelled) return;
        const v = getField(resp.config, configKey);
        const s = v === undefined || v === null ? '' : typeof v === 'string' ? v : JSON.stringify(v);
        setLoaded(s);
        setDraft(s);
        setLoading(false);
      })
      .catch((e) => {
        if (cancelled) return;
        setError(e instanceof Error ? e.message : String(e));
        setLoading(false);
      });
    return () => { cancelled = true; };
  }, [configKey]);

  const save = async () => {
    try {
      await sendRequest('config/value/write', { keyPath: path, value: draft, mergeStrategy: 'replace' });
      setLoaded(draft);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  if (loading) return <div className="modal-loading">loading…</div>;
  if (error) return <div className="modal-error">failed: {error}</div>;

  const dirty = draft !== loaded;
  return (
    <div className="settings-form">
      <label className="settings-form-label" htmlFor={`sf-${configKey}`}>{label}</label>
      {hint && <div className="settings-form-hint">{hint}</div>}
      <textarea
        id={`sf-${configKey}`}
        className="settings-form-textarea"
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        rows={8}
      />
      <div className="settings-form-actions">
        <button className="settings-action-button" onClick={save} disabled={!dirty}>save</button>
        <button className="settings-action-button" onClick={() => setDraft(loaded)} disabled={!dirty}>revert</button>
      </div>
    </div>
  );
}
