import { useEffect, useReducer, useRef, useCallback } from 'react';
import { onEvent, send, registerResponseHandler, sendRequest } from '../transport';
import { reducer } from '../viewModel/reducer';
import { initialState } from '../viewModel/types';

/** Wires the transport's onEvent stream into the ViewModel reducer.
 *  Returns the live ConversationState + the captured thread id (for the
 *  composer + SessionPicker active highlight) + an `interrupt` callback for
 *  the active turn.
 *
 *  The thread id is captured from any event that carries `params.threadId`
 *  (most ServerNotifications do) and held in a ref so the composer doesn't
 *  re-render on every event. The ref tracks the *latest* threadId — when the
 *  user switches sessions (thread/resume → thread/started for a different
 *  thread), the composer, the SessionPicker active highlight, and the
 *  interrupt callback all follow the switch. */
export function useConversation() {
  const [state, dispatch] = useReducer(reducer, initialState);
  const threadIdRef = useRef<string | null>(null);

  useEffect(() => {
    // registerResponseHandler is also auto-installed at transport.ts module
    // import time, so this call is a defensive re-install (idempotent) rather
    // than load-bearing. Kept so a future refactor that removes the
    // module-level install doesn't silently break response routing.
    registerResponseHandler();
    onEvent((ev) => {
      // Track the latest threadId. We deliberately do NOT guard with
      // `!threadIdRef.current` — that would pin the ref to the first thread
      // and break session switching (the highlight wouldn't move, the composer
      // would send to the old thread, and interrupt would hit the wrong turn).
      const maybeThreadId = (ev as { params?: { threadId?: string } })?.params?.threadId;
      if (maybeThreadId) {
        threadIdRef.current = maybeThreadId;
      }
      dispatch(ev);
    });
  }, []);

  const interrupt = useCallback(() => {
    const threadId = threadIdRef.current;
    const turnId = state.activeTurnId;
    if (!threadId || !turnId) return;
    send({
      method: 'turn/interrupt',
      id: Date.now(),
      params: { threadId, turnId },
    });
  }, [state.activeTurnId]);

  const startTurn = useCallback(async (input: unknown[], modelOverride?: string): Promise<void> => {
    const threadId = threadIdRef.current;
    if (!threadId) return;
    const params: Record<string, unknown> = { threadId, input };
    if (modelOverride) params.model = modelOverride;
    try {
      const resp = await sendRequest<{ model: string; cwd: string }>('turn/start', params);
      dispatch({ kind: 'responseMeta', model: resp.model, cwd: resp.cwd });
    } catch (e) {
      console.error('useConversation.startTurn failed', e);
    }
  }, []);

  const resumeThread = useCallback(async (threadId: string): Promise<void> => {
    try {
      const resp = await sendRequest<{ model: string; cwd: string }>('thread/resume', { threadId });
      // Assign threadIdRef AFTER the await succeeds — if sendRequest rejects,
      // the ref stays on the old thread (no inconsistency window). The
      // eventual thread/started ServerNotification for the new thread will
      // also flow through onEvent, but setting the ref here makes the
      // composer + sidebar highlight follow the switch without waiting for
      // that notification.
      threadIdRef.current = threadId;
      dispatch({ kind: 'responseMeta', model: resp.model, cwd: resp.cwd });
    } catch (e) {
      console.error('useConversation.resumeThread failed', e);
    }
  }, []);

  return { state, threadId: threadIdRef.current, interrupt, startTurn, resumeThread };
}
