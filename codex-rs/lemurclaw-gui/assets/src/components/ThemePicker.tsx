import { THEMES, type ThemeName } from '../themes';

interface Props {
  current: ThemeName;
  onPick: (t: ThemeName) => void;
  onClose: () => void;
}

/** Modal theme picker. Lists THEMES, highlights the current one, calls onPick
 *  when a theme is selected. The caller (App.tsx) wires onPick to useTheme's
 *  setTheme + closes the modal.
 *
 *  Close: Esc (handled locally via the content div's onKeyDown), backdrop
 *  click, or ✕ button. */
export function ThemePicker({ current, onPick, onClose }: Props) {
  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  };

  return (
    <div className="modal-overlay" data-testid="theme-picker" onClick={onClose}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()} onKeyDown={onKey} tabIndex={-1}>
        <header className="modal-header">
          <span className="modal-title">select theme</span>
          <button className="modal-close" onClick={onClose} aria-label="close">✕</button>
        </header>
        <div className="modal-body">
          <ul className="theme-list">
            {THEMES.map((t) => (
              <li key={t.name} className={`theme-item${t.name === current ? ' theme-item-active' : ''}`}>
                <button onClick={() => onPick(t.name)} className="theme-item-button">
                  <span className="theme-item-name">{t.label}</span>
                  <span className="theme-item-desc">{t.description}</span>
                </button>
              </li>
            ))}
          </ul>
        </div>
      </div>
    </div>
  );
}
