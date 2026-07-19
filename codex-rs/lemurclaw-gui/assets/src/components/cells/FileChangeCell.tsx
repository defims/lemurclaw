import { useState } from 'react';
import type { CellModel } from '../../viewModel/types';

type Model = Extract<CellModel, { kind: 'fileChange' }>;

interface Props {
  model: Model;
}

/** File-change (patch) cell. Lists changed files with +/- markers; each
 *  file's diff is collapsible. Apply status shown as a badge. */
export function FileChangeCell({ model }: Props) {
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const statusBadge = labelForPatchStatus(model.status);
  return (
    <div className={`cell cell-patch cell-patch-${model.status}`} data-testid="patch">
      <div className="cell-patch-header">
        <span className="cell-patch-title">📝 patch</span>
        <span className="cell-patch-status">{statusBadge}</span>
        <span className="cell-patch-count">{model.changes.length} file(s)</span>
      </div>
      <ul className="cell-patch-files">
        {model.changes.map((c, i) => {
          const key = `${c.path}:${i}`;
          const isOpen = open[key] ?? false;
          return (
            <li key={key} className={`cell-patch-file cell-patch-file-${c.kind.type}`}>
              <button
                className="cell-patch-file-toggle"
                aria-expanded={isOpen}
                onClick={() => setOpen((o) => ({ ...o, [key]: !o[key] }))}
              >
                <span className="cell-patch-kind">{kindMarker(c.kind.type)}</span>
                <code>{c.path}</code>
              </button>
              {isOpen && c.diff && <pre className="cell-patch-diff">{c.diff}</pre>}
            </li>
          );
        })}
      </ul>
    </div>
  );
}

function labelForPatchStatus(status: Model['status']): string {
  switch (status) {
    case 'inProgress': return 'applying';
    case 'completed': return 'applied';
    case 'failed': return 'failed';
    case 'declined': return 'declined';
  }
}

function kindMarker(t: 'add' | 'delete' | 'update'): string {
  switch (t) {
    case 'add': return '+';
    case 'delete': return '-';
    case 'update': return '~';
  }
}
