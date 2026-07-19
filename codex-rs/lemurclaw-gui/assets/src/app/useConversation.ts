import { useEffect, useReducer, useRef, useCallback } from 'react';
import { onEvent, send, registerResponseHandler } from '../transport';
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
    // Install the onResponse handler so sendRequest promises can settle.
    // Must run before any component fires a sendRequest (Onboarding does on
    // its own mount, but useConversation mounts first as the App root hook).
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

  return { state, threadId: threadIdRef.current, interrupt };
}
