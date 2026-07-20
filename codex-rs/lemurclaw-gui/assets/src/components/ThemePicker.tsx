import { THEMES, type ThemeName } from '../themes';
import { Modal } from './Modal';

interface Props {
  current: ThemeName;
  onPick: (t: ThemeName) => void;
  onClose: () => void;
}

/** Modal theme picker. Lists THEMES, highlights the current one, calls onPick
 *  when a theme is selected. The caller (App.tsx) wires onPick to useTheme's
 *  setTheme + closes the modal. */
export function ThemePicker({ current, onPick, onClose }: Props) {
  return (
    <Modal title="select theme" onClose={onClose} testId="theme-picker">
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
    </Modal>
  );
}
