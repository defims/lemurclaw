import { useState, useRef, useCallback } from 'react';
import { send } from '../transport';

interface Props {
  /** Active thread id, or null before thread/started. When null, the composer
   *  is disabled. */
  threadId: string | null;
  /** True while a turn is in progress; disables send + shows interrupt button. */
  turnActive: boolean;
  /** Send `turn/interrupt` for the active turn. No-op if turnActive is false. */
  onInterrupt: () => void;
}

/** Composer: textarea + send button. Enter sends (shift+enter newline).
 *  Sends a `turn/start` ClientRequest with a single text UserInput.
 *  Subproject 3 limitation: no slash popup, no @-mention popup, no file
 *  upload — those land in subproject 4. */
export function Composer({ threadId, turnActive, onInterrupt }: Props) {
  const [text, setText] = useState('');
  const seqRef = useRef(1);

  const submit = useCallback(() => {
    const trimmed = text.trim();
    if (!trimmed || !threadId) return;
    const clientUserMessageId = `c${seqRef.current++}`;
    send({
      method: 'turn/start',
      id: seqRef.current++,
      params: {
        threadId,
        clientUserMessageId,
        input: [{ type: 'text', text: trimmed, text_elements: [] }],
      },
    });
    setText('');
  }, [text, threadId]);

  const onKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      submit();
    }
  };

  const disabled = !threadId || turnActive;

  return (
    <div className="composer" data-testid="composer">
      <textarea
        className="composer-input"
        data-testid="composer-input"
        placeholder={threadId ? 'type a message…  (Enter to send, Shift+Enter for newline)' : 'starting up…'}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={onKeyDown}
        disabled={!threadId}
        rows={3}
      />
      <div className="composer-actions">
        {turnActive ? (
          <button className="composer-interrupt" onClick={onInterrupt} data-testid="composer-interrupt">
            interrupt
          </button>
        ) : (
          <button className="composer-send" onClick={submit} disabled={disabled} data-testid="composer-send">
            send
          </button>
        )}
      </div>
    </div>
  );
}
