import { useState } from 'react';
import { Modal } from '../Modal';

/** Identifiers for the settings surfaces. Each one maps to a panel file
 *  populated in Tasks 4-7. Order here = order in the left nav. */
export type SettingsSurface =
  | 'permissions'
  | 'keymap'
  | 'memories'
  | 'skills'
  | 'hooks'
  | 'mcp'
  | 'apps'
  | 'plugins'
  | 'experimental'
  | 'statusline';

interface SurfaceDef {
  key: SettingsSurface;
  label: string;
}

const SURFACES: SurfaceDef[] = [
  { key: 'permissions', label: 'Permissions' },
  { key: 'keymap', label: 'Keymap' },
  { key: 'memories', label: 'Memories' },
  { key: 'skills', label: 'Skills' },
  { key: 'hooks', label: 'Hooks' },
  { key: 'mcp', label: 'MCP' },
  { key: 'apps', label: 'Apps' },
  { key: 'plugins', label: 'Plugins' },
  { key: 'experimental', label: 'Experimental' },
  { key: 'statusline', label: 'Status line' },
];

interface Props {
  onClose: () => void;
}

/** Settings modal shell: left-nav of surfaces + a right pane that renders the
 *  active surface's panel. Surfaces are added incrementally — until a panel
 *  exists, the right pane shows a placeholder naming the surface. Esc, backdrop
 *  click, and ✕ close are inherited from <Modal>. */
export function SettingsModal({ onClose }: Props) {
  const [surface, setSurface] = useState<SettingsSurface>('permissions');

  return (
    <Modal title="settings" onClose={onClose} testId="settings-modal" wide>
      <div className="settings-modal-body">
        <nav className="settings-nav" aria-label="settings sections">
          {SURFACES.map((s) => (
            <button
              key={s.key}
              className={`settings-nav-item${s.key === surface ? ' settings-nav-item-active' : ''}`}
              onClick={() => setSurface(s.key)}
            >
              {s.label}
            </button>
          ))}
        </nav>
        <div className="settings-pane" data-testid={`settings-pane-${surface}`}>
          <Placeholder surface={surface} />
        </div>
      </div>
    </Modal>
  );
}

/** Placeholder shown until each surface's real panel lands (Tasks 4-7). Each
 *  real panel replaces the matching case. */
function Placeholder({ surface }: { surface: SettingsSurface }) {
  return <div className="modal-empty">{surface} panel — coming soon</div>;
}
