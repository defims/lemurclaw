import { Modal } from './Modal';
import { DiffText } from './DiffText';

interface Props {
  /** The unified diff text to render. If empty, modal shows an empty-state. */
  diff: string;
  onClose: () => void;
}

/** Full-screen diff viewer modal. Uses <Modal>'s `*ClassName` passthrough
 *  (same pattern as TranscriptPager) to override the default 480px card with
 *  near-fullscreen sizing (90vw × 90vh, z-index 1000) while keeping the
 *  shared Esc/backdrop/✕ close logic.
 *
 *  Renders <DiffText> when `diff` is non-empty; shows an empty-state message
 *  otherwise (e.g. /diff pressed before any turn produced a diff). */
export function DiffViewerModal({ diff, onClose }: Props) {
  return (
    <Modal
      title="diff"
      onClose={onClose}
      testId="diff-viewer-modal"
      overlayClassName="diff-viewer-overlay"
      contentClassName="diff-viewer-content"
      bodyClassName="diff-viewer-body"
    >
      {diff.trim() ? (
        <DiffText diff={diff} />
      ) : (
        <div className="modal-empty" data-testid="diff-viewer-empty">
          (no diff in this turn)
        </div>
      )}
    </Modal>
  );
}
