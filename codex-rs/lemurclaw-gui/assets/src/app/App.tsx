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
import { SettingsModal, type SettingsSurface } from '../components/settings/SettingsModal';
import { dispatchSlashCommand } from '../components/composer/dispatch';
import type { SlashCommand, SlashCommandContext, LocalAction } from '../components/composer/slashCommandTypes';

type ModalKind = 'none' | 'model' | 'theme' | 'transcript' | 'settings';

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
  /** Which SettingsModal tab to open. Set by /skills, /mcp, etc. before
   *  flipping modal to 'settings'. */
  const [settingsSurface, setSettingsSurface] = useState<SettingsSurface>('permissions');
  /** Bumped by /clear to force-remount Scrollback (UI-only clear; server-side
   *  conversation state unchanged — NOT equivalent to TUI's ClearUi). */
  const [clearKey, setClearKey] = useState(0);
  const turnActive = state.activeTurnId !== null;

  const handleLocalAction = (action: LocalAction) => {
    switch (action) {
      case 'clear':
        setClearKey((k) => k + 1);
        break;
      case 'new':
        // Submit /new as a turn — server treats unknown slash commands as
        // user text (matches TUI fallback for commands without a dedicated
        // RPC).
        startTurn([{ type: 'text', text: '/new', text_elements: [] }]);
        break;
      case 'quit':
        // wry webview: window.close() may be a no-op (often blocked by host).
        // Documented limitation — a future toast could surface "use Cmd+Q".
        window.close();
        break;
    }
  };

  const handleSlashCommand = (cmd: SlashCommand, args: string) => {
    const ctx: SlashCommandContext = {
      threadId,
      openSettings: (surface) => {
        setSettingsSurface(surface);
        setModal('settings');
      },
      openModal: (m) => setModal(m),
      localAction: handleLocalAction,
    };
    const result = dispatchSlashCommand(cmd, args, ctx);
    // ctx callbacks already fired for openSettings/openModal/localAction.
    // Handle the two categories that need App-level follow-up:
    if (result.kind === 'sendTurn') {
      startTurn(result.input);
    } else if (result.kind === 'notImplemented') {
      // Simple stub — toast comes later. alert() is synchronous and works
      // in jsdom tests (stubbed) and in wry (native dialog).
      window.alert(result.message);
    }
  };

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
          onOpenSettings={() => setModal('settings')}
        />
        <div className="app-body">
          <main className="app-main">
            <div className="app-scrollback" key={clearKey}>
              <Scrollback state={state} />
            </div>
            {state.pendingApprovals.length > 0 && (
              <div className="approvals-queue" data-testid="approvals-queue">
                {state.pendingApprovals.map((a) => (
                  <ApprovalCard key={String(a.requestId)} approval={a} />
                ))}
              </div>
            )}
            <Composer threadId={threadId} turnActive={turnActive} onInterrupt={interrupt} startTurn={startTurn} onSlashCommand={handleSlashCommand} />
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
      {modal === 'settings' && (
        <SettingsModal onClose={() => setModal('none')} initialSurface={settingsSurface} />
      )}
    </Onboarding>
  );
}
