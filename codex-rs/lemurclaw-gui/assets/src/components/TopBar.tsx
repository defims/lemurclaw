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
 *  Task 4.9 limitation: cwd/model are hardcoded null for now — surfacing them
 *  requires a reducer extension to capture thread.cwd / current model from
 *  thread/started. Tracked as a follow-up. */
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
