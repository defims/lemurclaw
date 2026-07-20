interface Props {
  cwd: string | null;
  model: string | null;
  onOpenModelPicker: () => void;
  onOpenThemePicker: () => void;
  onOpenTranscript: () => void;
}

/** Top bar (spec §4.3 "顶栏 目录+模型+菜单"). Shows cwd + current model +
 *  buttons for model picker, theme picker, transcript pager.
 *
 *  cwd comes from ConversationState.cwd (reducer captures thread.cwd from
 *  thread/started + the turn/start / thread/resume JSON-RPC response).
 *  model comes from ConversationState.currentModel (reducer captures
 *  model/rerouted.toModel + the same JSON-RPC response). Both are null
 *  until the first turn/start or a model/rerouted event. */
export function TopBar({ cwd, model, onOpenModelPicker, onOpenThemePicker, onOpenTranscript }: Props) {
  return (
    <header className="app-topbar" data-testid="topbar">
      <span className="topbar-cwd">{cwd ?? '(no cwd)'}</span>
      <button className="topbar-button" onClick={onOpenModelPicker}>
        <span className="topbar-model">{model ?? '(no model)'}</span> ⏷
      </button>
      <div className="topbar-spacer" />
      <button className="topbar-icon-button" onClick={onOpenTranscript} aria-label="transcript" title="transcript (Ctrl+T)">
        📜
      </button>
      <button className="topbar-icon-button" onClick={onOpenThemePicker} aria-label="theme" title="theme">
        🎨
      </button>
    </header>
  );
}
