import type { ReactNode } from 'react';

interface Props<T> {
  /** Currently-visible (already-filtered) items. Popup does no filtering. */
  filteredItems: T[];
  /** Render one row. isActive = keyboard-focused index match. */
  renderItem: (item: T, isActive: boolean) => ReactNode;
  /** Currently keyboard-focused index (-1 = none). */
  activeIndex: number;
  /** Fired when a row is clicked. Keyboard Enter/Tab handling lives in the
   *  parent (Composer), which calls onChoose with the active row. */
  onChoose: (item: T) => void;
  /** Whether the popup is visible. */
  open: boolean;
  /** Optional data-testid for the popup root. */
  testId?: string;
  /** Empty-state text when filteredItems is []. */
  emptyText?: string;
}

/** Generic text-anchored popover for the Composer. Absolute-positioned
 *  above the textarea. Stage 1: slash commands only; Stage 3 will reuse for
 *  mention + file-search popups.
 *
 *  Keyboard handling lives in the parent (Composer's textarea onKeyDown) —
 *  the popup is pure presentation + click handling. The parent owns
 *  activeIndex and calls onChoose on Enter/Tab. */
export function ComposerPopup<T>({
  filteredItems,
  renderItem,
  activeIndex,
  onChoose,
  open,
  testId,
  emptyText = '(no matches)',
}: Props<T>) {
  if (!open) return null;
  return (
    <div className="composer-popup" data-testid={testId} role="listbox">
      {filteredItems.length === 0 ? (
        <div className="composer-popup-empty">{emptyText}</div>
      ) : (
        filteredItems.map((item, i) => (
          <div
            key={i}
            className={`composer-popup-item${i === activeIndex ? ' composer-popup-item-active' : ''}`}
            role="option"
            aria-selected={i === activeIndex}
            onClick={() => onChoose(item)}
          >
            {renderItem(item, i === activeIndex)}
          </div>
        ))
      )}
    </div>
  );
}
