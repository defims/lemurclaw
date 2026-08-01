import { useState } from 'react';
import type { CellModel } from '../../viewModel/types';

interface Props {
  model: Extract<CellModel, { kind: 'reasoning' }>;
}

/** Reasoning cell. Collapsed by default (summary only); click to expand and
 *  show full content array. */
export function ReasoningCell({ model }: Props) {
  const [expanded, setExpanded] = useState(false);
  const hasContent = model.content.length > 0 || model.summary.length > 0;
  return (
    <div className="cell cell-reasoning" data-testid="reasoning">
      <button
        className="cell-reasoning-toggle"
        onClick={() => setExpanded((e) => !e)}
        aria-expanded={expanded}
      >
        reasoning {expanded ? '▾' : '▸'}
      </button>
      {hasContent ? (
        <div className="cell-body">
          {model.summary.map((s, i) => (
            <pre key={`s${i}`} className="cell-text cell-reasoning-summary">{s}</pre>
          ))}
          {expanded &&
            model.content.map((c, i) => (
              <pre key={`c${i}`} className="cell-text cell-reasoning-content">{c}</pre>
            ))}
        </div>
      ) : (
        <div className="cell-body cell-empty">(empty)</div>
      )}
    </div>
  );
}
