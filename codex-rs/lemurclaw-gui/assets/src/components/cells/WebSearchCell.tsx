import type { CellModel } from '../../viewModel/types';

interface Props {
  model: Extract<CellModel, { kind: 'webSearch' }>;
}

/** Web-search cell. Subproject 3 lightweight: shows query + status; full
 *  result rendering (citations, snippets) deferred. */
export function WebSearchCell({ model }: Props) {
  return (
    <div className="cell cell-websearch" data-testid="websearch">
      <span className="cell-websearch-icon">🌐</span>
      <span className="cell-websearch-query">{model.query || '(web search)'}</span>
      <span className="cell-websearch-status">{model.status}</span>
    </div>
  );
}
