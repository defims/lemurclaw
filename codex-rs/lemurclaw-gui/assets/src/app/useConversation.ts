import { useEffect, useReducer, useRef, useCallback } from 'react';
import { onEvent, send } from '../transport';
import { reducer } from '../viewModel/reducer';
import { initialState } from '../viewModel/types';

/** Wires the transport's onEvent stream into the ViewModel reducer.
 *  Returns the live ConversationState + the captured thread id (for the
 *  composer) + an `interrupt` callback for the active turn.
 *
 *  The thread id is captured opportunistically from any event that carries
 *  `params.threadId` (most ServerNotifications do) and held in a ref so the
 *  composer doesn't re-render on every event. */
export function useConversation() {
  const [state, dispatch] = useReducer(reducer, initialState);
  const threadIdRef = useRef<string | null>(null);

  useEffect(() => {
    onEvent((ev) => {
      const maybeThreadId = (ev as { params?: { threadId?: string } })?.params?.threadId;
      if (maybeThreadId && !threadIdRef.current) {
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
