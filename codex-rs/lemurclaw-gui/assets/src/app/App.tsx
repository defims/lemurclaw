import { useEffect, useState } from 'react';
import { useConversation } from './useConversation';
import { useTheme } from '../hooks/useTheme';
import { Scrollback } from '../components/Scrollback';
import { Composer } from '../components/Composer';
import { ApprovalCard } from '../components/ApprovalCard';
import { Sidebar } from '../components/Sidebar';
import { SessionPicker } from '../components/sidebar/SessionPicker';
import { AgentPanel } from '../components/sidebar/AgentPanel';
import { TopBar } from '../components/TopBar';
import { Onboarding } from '../components/Onboarding';
import { TranscriptPager } from '../components/TranscriptPager';
import { ModelPicker } from '../components/ModelPicker';
import { ThemePicker } from '../components/ThemePicker';

type ModalKind = 'none' | 'model' | 'theme' | 'transcript';

/** Top-level GUI application. Spec §4.3 layout: top bar (cwd + model + menu)
 *  over a main column (scrollback + approvals + composer) sitting beside a
 *  right sidebar (sessions + agent). Onboarding gates the whole thing until
 *  auth is resolved. Modal host overlays (TranscriptPager / ModelPicker /
 *  ThemePicker) render above the main layout.
 *
 *  Remaining limitation: Ctrl+T uses ctrl OR meta — safe in the wry webview
 *  (no browser chrome), but needs revisiting for webui mode (subproject 6)
 *  where Cmd+T is the browser new-tab shortcut. */
export function App() {
  const { state, threadId, interrupt, startTurn, resumeThread } = useConversation();
  const { theme, setTheme } = useTheme();
  const [modal, setModal] = useState<ModalKind>('none');
  const turnActive = state.activeTurnId !== null;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // Esc closes any open modal. The modals also handle Esc themselves via
      // their own window listeners; this is a safety net for focus edge cases.
      if (e.key === 'Escape' && modal !== 'none') {
        setModal('none');
      }
      // Ctrl+T (or Cmd+T on mac) opens the transcript pager. wry has no
      // browser chrome so Cmd+T won't open a new tab; revisit for webui.
      if ((e.ctrlKey || e.metaKey) && e.key === 't') {
        e.preventDefault();
        setModal('transcript');
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [modal]);

  return (
    <Onboarding>
      <div className="app-root">
        <TopBar
          cwd={state.cwd}
          model={state.currentModel}
          onOpenModelPicker={() => setModal('model')}
          onOpenThemePicker={() => setModal('theme')}
          onOpenTranscript={() => setModal('transcript')}
        />
        <div className="app-body">
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
            <Composer threadId={threadId} turnActive={turnActive} onInterrupt={interrupt} startTurn={startTurn} />
          </main>
          <Sidebar
            sections={[
              { key: 'sessions', title: 'Sessions', body: <SessionPicker activeThreadId={threadId} onResume={resumeThread} /> },
              { key: 'agents', title: 'Agent', body: <AgentPanel state={state} /> },
            ]}
          />
        </div>
      </div>

      {modal === 'transcript' && threadId && (
        <TranscriptPager threadId={threadId} onClose={() => setModal('none')} />
      )}
      {modal === 'model' && (
        <ModelPicker threadId={threadId} onClose={() => setModal('none')} startTurn={startTurn} />
      )}
      {modal === 'theme' && (
        <ThemePicker current={theme} onPick={(t) => setTheme(t)} onClose={() => setModal('none')} />
      )}
    </Onboarding>
  );
}
