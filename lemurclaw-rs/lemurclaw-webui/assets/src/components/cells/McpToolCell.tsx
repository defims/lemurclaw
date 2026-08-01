import { useState } from 'react';
import type { CellModel } from '../../viewModel/types';

interface Props {
  model: Extract<CellModel, { kind: 'mcpToolCall' }>;
}

/** MCP tool-call cell. Shows server/tool + status; args + progress + result
 *  collapsible. */
export function McpToolCell({ model }: Props) {
  const [expanded, setExpanded] = useState(false);
  return (
    <div className={`cell cell-mcp cell-mcp-${model.status}`} data-testid="mcp">
      <div className="cell-mcp-header">
        <span className="cell-mcp-title">🔧 {model.server}.{model.tool}</span>
        <span className="cell-mcp-status">{model.status}</span>
        <button aria-expanded={expanded} onClick={() => setExpanded((e) => !e)}>
          {expanded ? 'less' : 'more'}
        </button>
      </div>
      {model.progress.length > 0 && (
        <ul className="cell-mcp-progress">
          {model.progress.map((p, i) => <li key={i}>{p}</li>)}
        </ul>
      )}
      {expanded && (
        <div className="cell-mcp-detail">
          <pre className="cell-mcp-args">{JSON.stringify(model.arguments, null, 2)}</pre>
          {model.error && <pre className="cell-mcp-error">{model.error}</pre>}
          {model.result != null && <pre className="cell-mcp-result">{JSON.stringify(model.result, null, 2)}</pre>}
        </div>
      )}
    </div>
  );
}
