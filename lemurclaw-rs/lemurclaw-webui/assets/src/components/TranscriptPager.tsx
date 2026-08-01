import { useEffect, useState } from 'react';
import { sendRequest } from '../transport';
import { Modal } from './Modal';
import type { Thread } from '../types/v2';
import type { CellModel } from '../viewModel/types';
import { threadItemToCell } from '../viewModel/reducer';
import { CellRenderer, cellKey } from './Scrollback';

interface Props {
  /** Thread to load. The pager fetches its turns on mount. */
  threadId: string;
  /** Close handler (Esc or backdrop click). */
  onClose: () => void;
}

interface ReadState {
  loading: boolean;
  error: string | null;
  cells: CellModel[];
  thread: Thread | null;
}

/** Full-screen transcript pager (codex TUI's Ctrl+T equivalent).
 *
 *  Loads the thread's full turn history via `thread/read { includeTurns: true }`
 *  and renders all items flat (no turn boundaries) using the same cell
 *  components as Scrollback (via the shared CellRenderer). Read-only — no
 *  input, no approvals.
 *
 *  Uses <Modal>'s `*ClassName` props to keep its bespoke full-screen sizing
 *  (90vw × 90vh, z-index 1000) while sharing the Esc/backdrop/✕ logic. */
export function TranscriptPager({ threadId, onClose }: Props) {
  const [state, setState] = useState<ReadState>({ loading: true, error: null, cells: [], thread: null });

  useEffect(() => {
    let cancelled = false;
    setState({ loading: true, error: null, cells: [], thread: null });
    sendRequest<{ thread: Thread }>('thread/read', { threadId, includeTurns: true })
      .then((resp) => {
        if (cancelled) return;
        const cells = resp.thread.turns.flatMap((t) => t.items.map(threadItemToCell));
        setState({ loading: false, error: null, cells, thread: resp.thread });
      })
      .catch((e) => {
        if (cancelled) return;
        setState({ loading: false, error: e instanceof Error ? e.message : String(e), cells: [], thread: null });
      });
    return () => { cancelled = true; };
  }, [threadId]);

  return (
    <Modal
      title={`transcript · ${state.thread?.name ?? state.thread?.preview ?? threadId}`}
      onClose={onClose}
      testId="transcript-pager"
      overlayClassName="transcript-pager-overlay"
      contentClassName="transcript-pager-content"
      headerClassName="transcript-pager-header"
      titleClassName="transcript-pager-title"
      closeClassName="transcript-pager-close"
      bodyClassName="transcript-pager-body"
    >
      {state.loading && <div className="transcript-pager-loading">loading…</div>}
      {state.error && <div className="transcript-pager-error">failed: {state.error}</div>}
      {!state.loading && !state.error && state.cells.length === 0 && (
        <div className="transcript-pager-empty">(no items in transcript)</div>
      )}
      {!state.loading && !state.error && state.cells.length > 0 && (
        <div className="transcript-pager-cells">
          {state.cells.map((c) => <CellRenderer key={cellKey(c)} cell={c} />)}
        </div>
      )}
    </Modal>
  );
}
