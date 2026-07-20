import { useState, useCallback } from 'react';

interface Props {
  /** Active thread id, or null before thread/started. When null, the composer
   *  is disabled. */
  threadId: string | null;
  /** True while a turn is in progress; disables send + shows interrupt button. */
  turnActive: boolean;
  /** Send `turn/interrupt` for the active turn. No-op if turnActive is false. */
  onInterrupt: () => void;
  /** Send a turn/start ClientRequest with a single text UserInput and feed the
   *  response's cwd/model back into ConversationState via responseMeta. */
  startTurn: (input: unknown[], modelOverride?: string) => Promise<void>;
}

/** Composer: textarea + send button. Enter sends (shift+enter newline).
 *  Calls the parent's startTurn (from useConversation), which sends a
 *  `turn/start` ClientRequest via sendRequest and folds the response's
 *  authoritative cwd/model into state.
 *  Limitation: no slash popup, no @-mention popup, no file upload — those
 *  land in subproject 5 (SettingsModal + popups). */
export function Composer({ threadId, turnActive, onInterrupt, startTurn }: Props) {
  const [text, setText] = useState('');

  const submit = useCallback(() => {
    const trimmed = text.trim();
    if (!trimmed || !threadId) return;
    startTurn([{ type: 'text', text: trimmed, text_elements: [] }]);
    setText('');
  }, [text, threadId, startTurn]);

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
