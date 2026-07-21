import { useState } from 'react';
import type { CellModel } from '../../viewModel/types';

interface Props {
  model: Extract<CellModel, { kind: 'hook' }>;
}

/** Hook cell. Shows hook event + handler + status; entries collapsible.
 *
 *  NOTE: HookRunSummary fields marked `bigint` in ts-rs types are actually
 *  serialized by codex as JSON numbers (serde i64 → number), so JS sees plain
 *  numbers at runtime. This subproject's UI only renders the string-typed
 *  fields (eventName/handlerType/status/scope/statusMessage) and the entries
 *  list (via JSON.stringify); it does not yet surface the numeric fields
 *  (startedAt/durationMs/etc.), so no bigint coercion is needed yet. If/when
 *  those fields are rendered, wrap them in Number() defensively in case an
 *  engine ever preserves bigint. */
export function HookCell({ model }: Props) {
  const [expanded, setExpanded] = useState(false);
  const r = model.run;
  return (
    <div className={`cell cell-hook cell-hook-${r.status}`} data-testid="hook">
      <div className="cell-hook-header">
        <span className="cell-hook-title">🪝 {r.eventName} · {r.handlerType}</span>
        <span className="cell-hook-status">{r.status}</span>
        <span className="cell-hook-scope">{r.scope}</span>
        <button aria-expanded={expanded} onClick={() => setExpanded((e) => !e)}>
          {expanded ? 'hide' : `${r.entries.length} entries`}
        </button>
      </div>
      {r.statusMessage && <div className="cell-hook-message">{r.statusMessage}</div>}
      {expanded && r.entries.length > 0 && (
        <ul className="cell-hook-entries">
          {r.entries.map((e, i) => <li key={i}><pre>{JSON.stringify(e)}</pre></li>)}
        </ul>
      )}
    </div>
  );
}
