import type { CellModel } from '../../viewModel/types';

interface Props {
  model: Extract<CellModel, { kind: 'agentMessage' }>;
}

/** Assistant message cell. Renders streamed + final text identically (the
 *  reducer already keeps the latest snapshot). `phase: 'final_answer'` gets a
 *  subtle visual marker; null or 'commentary' renders without it. */
export function AgentMessageCell({ model }: Props) {
  const isFinal = model.phase === 'final_answer';
  return (
    <div className={`cell cell-agent${isFinal ? ' cell-agent-final' : ''}`} data-testid="agent-message">
      <div className="cell-role">assistant{isFinal ? '' : ' · thinking'}</div>
      <div className="cell-body">
        <pre className="cell-text">{model.text}</pre>
      </div>
    </div>
  );
}
