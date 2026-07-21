import { useEffect, type ReactNode } from 'react';

interface Props {
  /** Title shown in the header bar. */
  title: ReactNode;
  /** Close handler. Fired on Esc, backdrop click, and ✕ button. */
  onClose: () => void;
  /** Modal body. */
  children: ReactNode;
  /** Optional `data-testid` for the overlay root (existing pickers set one). */
  testId?: string;
  /** When true, adds `modal-content-wide` class for settings-style width.
   *  Pickers stay narrow (default). */
  wide?: boolean;
  /** Extra class on the overlay div (e.g. `transcript-pager-overlay` so a
   *  full-screen surface can keep its bespoke sizing + z-index while sharing
   *  the Esc/backdrop/✕ logic). */
  overlayClassName?: string;
  /** Extra class on the content div (e.g. `transcript-pager-content`). */
  contentClassName?: string;
  /** Extra class on the header (e.g. `transcript-pager-header`). */
  headerClassName?: string;
  /** Extra class on the title span (e.g. `transcript-pager-title`). */
  titleClassName?: string;
  /** Extra class on the close button (e.g. `transcript-pager-close`). */
  closeClassName?: string;
  /** Extra class on the body div (e.g. `transcript-pager-body`). */
  bodyClassName?: string;
}

/** Shared modal shell: fixed overlay, centered content card, header with title
 *  + ✕ close button, scrollable body. Owns the three close vectors (Esc window
 *  listener, backdrop click, ✕ button) that ModelPicker/ThemePicker/
 *  TranscriptPager previously duplicated.
 *
 *  The Esc handler is a window listener (not on the content node) so it fires
 *  regardless of focus — matches the pre-refactor behavior the existing tests
 *  assert on (`fireEvent.keyDown(window, { key: 'Escape' })`).
 *
 *  Surface-specific sizing/classes are kept via the `*ClassName` props —
 *  TranscriptPager uses them to stay full-screen (its existing test queries
 *  `.transcript-pager-overlay` and expects 90vw × 90vh + z-index 1000);
 *  pickers leave them off and get the default 480px card (`wide` → 720px). */
export function Modal({
  title,
  onClose,
  children,
  testId,
  wide,
  overlayClassName,
  contentClassName,
  headerClassName,
  titleClassName,
  closeClassName,
  bodyClassName,
}: Props) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  const overlayCls = ['modal-overlay', overlayClassName].filter(Boolean).join(' ');
  const contentCls = ['modal-content', wide ? 'modal-content-wide' : null, contentClassName].filter(Boolean).join(' ');
  const headerCls = ['modal-header', headerClassName].filter(Boolean).join(' ');
  const titleCls = ['modal-title', titleClassName].filter(Boolean).join(' ');
  const closeCls = ['modal-close', closeClassName].filter(Boolean).join(' ');
  const bodyCls = ['modal-body', bodyClassName].filter(Boolean).join(' ');

  return (
    <div className={overlayCls} data-testid={testId} onClick={onClose}>
      <div className={contentCls} onClick={(e) => e.stopPropagation()}>
        <header className={headerCls}>
          <span className={titleCls}>{title}</span>
          <button className={closeCls} onClick={onClose} aria-label="close">✕</button>
        </header>
        <div className={bodyCls}>{children}</div>
      </div>
    </div>
  );
}
