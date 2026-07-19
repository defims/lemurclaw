import { useState, useCallback, useEffect } from 'react';
import { sendRequest } from '../transport';
import type { Thread } from '../types/v2';
import type { ThreadListResponse } from '../types/v2/ThreadListResponse';

interface ThreadListState {
  threads: Thread[];
  loading: boolean;
  error: string | null;
  nextCursor: string | null;
}

/** Paginated thread list hook. Auto-loads first page on mount; `loadMore`
 *  appends the next page using the returned cursor. Used by SessionPicker. */
export function useThreadList(limit: number = 20) {
  const [state, setState] = useState<ThreadListState>({
    threads: [], loading: false, error: null, nextCursor: null,
  });

  const fetchPage = useCallback(async (cursor: string | null, replace: boolean) => {
    setState((s) => ({ ...s, loading: true, error: null }));
    try {
      const resp = await sendRequest<ThreadListResponse>('thread/list', { limit, cursor });
      setState((s) => ({
        threads: replace ? resp.data : [...s.threads, ...resp.data],
        loading: false, error: null, nextCursor: resp.nextCursor,
      }));
    } catch (e) {
      setState((s) => ({ ...s, loading: false, error: e instanceof Error ? e.message : String(e) }));
    }
  }, [limit]);

  useEffect(() => { fetchPage(null, true); }, [fetchPage]);

  const loadMore = useCallback(() => {
    if (state.nextCursor && !state.loading) fetchPage(state.nextCursor, false);
  }, [state.nextCursor, state.loading, fetchPage]);

  const refresh = useCallback(() => { fetchPage(null, true); }, [fetchPage]);

  return { ...state, loadMore, refresh };
}
