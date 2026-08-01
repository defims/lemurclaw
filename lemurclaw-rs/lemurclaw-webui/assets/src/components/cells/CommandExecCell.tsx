import { useState } from 'react';
import type { CellModel } from '../../viewModel/types';

type Model = Extract<CellModel, { kind: 'commandExecution' }>;

interface Props {
  model: Model;
}

/** Command-execution cell. Shows command + cwd + status; output collapsible. */
export function CommandExecCell({ model }: Props) {
  const [expanded, setExpanded] = useState(false);
  const statusLabel = labelForStatus(model.status, model.exitCode);
  return (
    <div className={`cell cell-exec cell-exec-${model.status}`} data-testid="exec">
      <div className="cell-exec-header">
        <code className="cell-exec-command">$ {model.command}</code>
        <span className="cell-exec-status">{statusLabel}</span>
        <button onClick={() => setExpanded((e) => !e)} aria-expanded={expanded}>
          {expanded ? 'hide output' : 'show output'}
        </button>
      </div>
      <div className="cell-exec-cwd">{model.cwd}</div>
      {expanded && model.aggregatedOutput && (
        <pre className="cell-exec-output" data-testid="exec-output">{model.aggregatedOutput}</pre>
      )}
    </div>
  );
}

function labelForStatus(status: Model['status'], exitCode: number | null): string {
  switch (status) {
    case 'inProgress': return 'running';
    case 'completed': return exitCode === 0 ? '✓ exit 0' : `✓ exit ${exitCode}`;
    case 'failed': return `✗ exit ${exitCode}`;
    case 'declined': return 'declined';
  }
}
