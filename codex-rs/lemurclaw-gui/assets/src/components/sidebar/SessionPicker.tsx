import { useThreadList } from '../../hooks/useThreadList';
import { send } from '../../transport';
import type { Thread } from '../../types/v2';

interface Props {
  /** Currently active thread id (for highlight). Null pre-first-thread. */
  activeThreadId: string | null;
  /** Override the default thread/resume dispatch (used by tests). */
  onSelect?: (thread: Thread) => void;
}

/** Session picker: lists past threads (thread/list, paginated), highlights
 *  the active one, and switches on click (thread/resume).
 *
 *  Lives in the sidebar (spec §4.3 "会话"区). Subproject 4 doesn't implement
 *  fuzzy search or rename — those land in subproject 5 (SettingsModal). */
export function SessionPicker({ activeThreadId, onSelect }: Props) {
  const { threads, loading, error, loadMore, refresh, nextCursor } = useThreadList(20);

  const handleSelect = (thread: Thread) => {
    if (onSelect) {
      onSelect(thread);
    } else {
      // Fire-and-forget thread/resume. The thread/started ServerNotification
      // (consumed by the reducer) will confirm the switch.
      send({
        method: 'thread/resume',
        id: Date.now(),
        params: { threadId: thread.id },
      });
    }
  };

  if (loading && threads.length === 0) {
    return <div className="session-picker session-picker-loading" data-testid="session-picker">loading…</div>;
  }
  if (error) {
    return (
      <div className="session-picker session-picker-error" data-testid="session-picker">
        <div>failed to load: {error}</div>
        <button onClick={refresh}>retry</button>
      </div>
    );
  }
  if (threads.length === 0) {
    return <div className="session-picker session-picker-empty" data-testid="session-picker">no sessions yet</div>;
  }

  return (
    <div className="session-picker" data-testid="session-picker">
      <ul className="session-list">
        {threads.map((t) => (
          <li
            key={t.id}
            className={`session-item${t.id === activeThreadId ? ' session-item-active' : ''}`}
          >
            <button
              onClick={() => handleSelect(t)}
              className="session-item-button"
              aria-current={t.id === activeThreadId ? 'true' : undefined}
            >
              <span className="session-item-preview">{t.preview || t.name || '(untitled)'}</span>
              <span className="session-item-meta">
                {t.modelProvider} · {new Date((t.recencyAt ?? t.updatedAt) * 1000).toLocaleDateString()}
              </span>
            </button>
          </li>
        ))}
      </ul>
      {nextCursor && (
        <button onClick={loadMore} className="session-load-more" disabled={loading}>
          {loading ? 'loading…' : 'load more'}
        </button>
      )}
    </div>
  );
}
