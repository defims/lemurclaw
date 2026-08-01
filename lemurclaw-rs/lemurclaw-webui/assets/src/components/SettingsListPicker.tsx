import type { ReactNode } from 'react';

/** Shared load/error/empty/list render for settings panels backed by a list
 *  RPC. Each panel owns the fetch useEffect (so it can vary method/params) and
 *  hands the resulting state here. Generic over the item type so panels don't
 *  need adapter layers. */
export interface LoadState<T> {
  loading: boolean;
  error: string | null;
  items: T[];
}

interface Props<T> {
  state: LoadState<T>;
  /** Stable key for React list rendering. */
  getId: (item: T) => string;
  /** Primary label text for each row. */
  renderLabel: (item: T) => ReactNode;
  /** Optional secondary line (id, description, etc). */
  renderSub?: (item: T) => ReactNode;
  /** Optional trailing action (uninstall, install, …). */
  renderAction?: (item: T) => ReactNode;
  /** Whether the row is non-selectable. Defaults to false. */
  isDisabled?: (item: T) => boolean;
  /** Currently-active row id (highlight). */
  activeId?: string | null;
  /** Fired when an enabled row is clicked. Absent for read-only panels. */
  onActivate?: (item: T) => void;
  /** Override the default "(empty)" empty-state copy. */
  emptyText?: string;
}

export function SettingsListPicker<T>({
  state,
  getId,
  renderLabel,
  renderSub,
  renderAction,
  isDisabled,
  activeId,
  onActivate,
  emptyText = '(empty)',
}: Props<T>) {
  if (state.loading) return <div className="modal-loading">loading…</div>;
  if (state.error) return <div className="modal-error">failed: {state.error}</div>;
  if (state.items.length === 0) return <div className="modal-empty">{emptyText}</div>;

  return (
    <ul className="settings-list">
      {state.items.map((item) => {
        const id = getId(item);
        const disabled = isDisabled ? isDisabled(item) : false;
        const active = id === activeId;
        return (
          <li
            key={id}
            className={`settings-list-item${active ? ' settings-list-item-active' : ''}${disabled ? ' settings-list-item-disabled' : ''}`}
          >
            <button
              className="settings-list-item-button"
              onClick={() => onActivate?.(item)}
              disabled={disabled || !onActivate}
            >
              <span className="settings-list-item-label">{renderLabel(item)}</span>
              {renderSub && <span className="settings-list-item-sub">{renderSub(item)}</span>}
            </button>
            {renderAction && <span className="settings-list-item-action">{renderAction(item)}</span>}
          </li>
        );
      })}
    </ul>
  );
}
