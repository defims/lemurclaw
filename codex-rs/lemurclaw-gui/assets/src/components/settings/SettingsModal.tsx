import { useState } from 'react';
import { Modal } from '../Modal';
import { PermissionsPanel } from './PermissionsPanel';
import { HooksPanel } from './HooksPanel';
import { McpPanel } from './McpPanel';
import { SkillsPanel } from './SkillsPanel';
import { AppsPanel } from './AppsPanel';
import { PluginsPanel } from './PluginsPanel';

/** Identifiers for the settings surfaces. Each one maps to a panel file
 *  populated in Tasks 4-7. Order here = order in the left nav.
 *
 *  Scope note: keymap/status-line/terminal-title are TUI-only config keys
 *  (under [tui]) that the app-server's config/read API does not expose, so
 *  they are intentionally absent from this list — they can't be edited from
 *  the GUI. See plan 2026-07-20-settings-modal.md Task 7 revision. */
export type SettingsSurface =
  | 'permissions'
  | 'memories'
  | 'model'
  | 'skills'
  | 'hooks'
  | 'mcp'
  | 'apps'
  | 'plugins'
  | 'experimental';

interface SurfaceDef {
  key: SettingsSurface;
  label: string;
}

const SURFACES: SurfaceDef[] = [
  { key: 'permissions', label: 'Permissions' },
  { key: 'memories', label: 'Memories' },
  { key: 'model', label: 'Model' },
  { key: 'skills', label: 'Skills' },
  { key: 'hooks', label: 'Hooks' },
  { key: 'mcp', label: 'MCP' },
  { key: 'apps', label: 'Apps' },
  { key: 'plugins', label: 'Plugins' },
  { key: 'experimental', label: 'Experimental' },
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
          {surface === 'permissions' && <PermissionsPanel />}
          {surface === 'hooks' && <HooksPanel />}
          {surface === 'mcp' && <McpPanel />}
          {surface === 'skills' && <SkillsPanel />}
          {surface === 'apps' && <AppsPanel />}
          {surface === 'plugins' && <PluginsPanel />}
          {(surface === 'memories' || surface === 'model' || surface === 'experimental') && (
            <Placeholder surface={surface} />
          )}
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
