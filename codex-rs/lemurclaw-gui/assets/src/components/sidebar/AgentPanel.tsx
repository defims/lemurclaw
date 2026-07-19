import type { ConversationState } from '../../viewModel/types';

interface Props {
  state: ConversationState;
}

/** Sidebar "Agent" section. Shows the main agent's status (sub-agent activity
 *  is deferred — see the hint rendered below).
 *
 *  Subproject 4 minimal: this is read-only — no spawn/control UI. The multi-
 *  agent control surface (spawn, message, interrupt sub-agents) is reserved
 *  for a later subproject. */
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
      {/* Sub-agent rows deferred — lemurclaw-gui doesn't yet surface
          collab/subAgent items with structure (they go to CellModel 'generic'
          in Task 3.4). Add rows here once Task 3.4's reducer exports a typed
          sub-agent view. */}
      <div className="agent-empty-hint">sub-agent control deferred to a later subproject</div>
    </div>
  );
}
