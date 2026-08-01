import type { ConversationState } from '../../viewModel/types';

interface Props {
  state: ConversationState;
}

/** Sidebar "Agent" section. Shows main agent status + one row per sub-agent
 *  observed in collabAgentToolCall items (via state.subAgents). Read-only —
 *  no spawn/control UI (deferred to a later subproject). */
export function AgentPanel({ state }: Props) {
  const mainStatus = state.status;
  const mainLabel =
    mainStatus === null
      ? '(not started)'
      : mainStatus.type === 'active'
        ? `active · ${(mainStatus.activeFlags ?? []).join(', ') || 'working'}`
        : mainStatus.type;

  return (
    <div className="agent-panel" data-testid="agent-panel">
      <div className="agent-row agent-row-main">
        <span className="agent-name">main</span>
        <span className="agent-status">{mainLabel}</span>
      </div>
      {state.subAgents.length === 0 ? (
        <div className="agent-empty-hint">no sub-agents active</div>
      ) : (
        <ul className="agent-sub-list" data-testid="agent-sub-list">
          {state.subAgents.map((s) => (
            <li key={s.threadId} className={`agent-row agent-row-sub agent-row-sub-${s.status}`}>
              <span className="agent-name">{s.threadId}</span>
              <span className="agent-status">{s.status}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
