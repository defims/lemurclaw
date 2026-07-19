import { useConversation } from './useConversation';
import { Scrollback } from '../components/Scrollback';
import { Composer } from '../components/Composer';
import { ApprovalCard } from '../components/ApprovalCard';
import { Sidebar } from '../components/Sidebar';
import { SessionPicker } from '../components/sidebar/SessionPicker';
import { AgentPanel } from '../components/sidebar/AgentPanel';

/** Top-level GUI application. Wires the transport stream into the ViewModel
 *  reducer via `useConversation`, then lays out the main conversation region
 *  (Scrollback), the approval queue (overlay above composer), and the input
 *  (Composer).
 *
 *  Layout follows spec §4.3: vertical main column. The right rail (sessions /
 *  agent / plan sidebar) is reserved for subproject 4. */
export function App() {
  const { state, threadId, interrupt } = useConversation();
  const turnActive = state.activeTurnId !== null;

  return (
    <div className="app-root">
      <main className="app-main">
        <div className="app-scrollback">
          <Scrollback state={state} />
        </div>
        {state.pendingApprovals.length > 0 && (
          <div className="approvals-queue" data-testid="approvals-queue">
            {state.pendingApprovals.map((a) => (
              <ApprovalCard key={String(a.requestId)} approval={a} />
            ))}
          </div>
        )}
        <Composer threadId={threadId} turnActive={turnActive} onInterrupt={interrupt} />
      </main>
      <Sidebar
        sections={[
          { key: 'sessions', title: 'Sessions', body: <SessionPicker activeThreadId={threadId} /> },
          { key: 'agents', title: 'Agent', body: <AgentPanel state={state} /> },
        ]}
      />
    </div>
  );
}
