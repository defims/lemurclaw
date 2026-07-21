import { useState, useCallback, useMemo } from 'react';
import { ComposerPopup } from './ComposerPopup';
import { SLASH_COMMANDS } from './composer/slashCommands';
import type { SlashCommand } from './composer/slashCommandTypes';

interface Props {
  /** Active thread id, or null before thread/started. When null, the composer
   *  is disabled. */
  threadId: string | null;
  /** True while a turn is in progress; disables send + shows interrupt button. */
  turnActive: boolean;
  /** Send `turn/interrupt` for the active turn. No-op if turnActive is false. */
  onInterrupt: () => void;
  /** Send `turn/start` with a single text UserInput and fold the response's
   *  cwd/model back into ConversationState via responseMeta. */
  startTurn: (input: unknown[], modelOverride?: string) => Promise<void>;
  /** Fired when the user picks a slash command from the popup OR submits a
   *  line whose first token exactly matches a command name. App routes the
   *  command via dispatchSlashCommand. */
  onSlashCommand: (cmd: SlashCommand, args: string) => void;
}

/** Returns the slash command name being typed at the start of `text`, or null
 *  if text doesn't start with "/".
 *  "/mod"    → "mod"
 *  "/model foo" → "model"
 *  "/"       → "" (popup trigger, shows all)
 *  "hello"   → null
 *  "\n/foo"  → null (only leading "/" on the first line triggers) */
function slashToken(text: string): string | null {
  if (!text.startsWith('/')) return null;
  const m = text.match(/^\/(\S*)/);
  return m ? m[1] : '';
}

/** Composer: textarea + send button + slash command popup. Enter sends
 *  (shift+enter newline). Typing a leading "/" opens <ComposerPopup> with
 *  prefix-filtered commands; arrow keys / Tab / Enter select; Esc closes.
 *
 *  Slash dispatch is delegated to the parent via onSlashCommand — Composer
 *  doesn't know about modals or RPCs. */
export function Composer({ threadId, turnActive, onInterrupt, startTurn, onSlashCommand }: Props) {
  const [text, setText] = useState('');
  const [activeIndex, setActiveIndex] = useState(0);

  const token = slashToken(text);
  const popupOpen = token !== null;
  const filtered = useMemo(() => {
    if (token === null) return [];
    return SLASH_COMMANDS.filter((c) => token === '' || c.name.startsWith(token));
  }, [token]);

  // Reset active index whenever the filter set changes (different length or
  // different entries). Cheap identity check on the array reference via the
  // token dependency — filtered is recomputed when token changes.
  // (Using a separate effect would over-render; inline comparison is fine
  // because filtered is small — max 16 entries.)
  const clampActive = (len: number, idx: number) => (len === 0 ? 0 : Math.min(idx, len - 1));

  const choose = (cmd: SlashCommand) => {
    // Strip the leading "/<name>" + any whitespace after it to get args.
    const re = new RegExp(`^/${cmd.name}\\s*`);
    const args = text.replace(re, '');
    onSlashCommand(cmd, args);
    setText('');
  };

  const submit = useCallback(() => {
    // If the user typed an exact "/cmd" (no continuation), dispatch as slash.
    // This handles Enter on a complete command without using popup nav.
    const exact = SLASH_COMMANDS.find((c) => c.name === token);
    if (exact) {
      choose(exact);
      return;
    }
    const trimmed = text.trim();
    if (!trimmed || !threadId) return;
    startTurn([{ type: 'text', text: trimmed, text_elements: [] }]);
    setText('');
  }, [text, threadId, startTurn, token]);

  const onKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (popupOpen && filtered.length > 0) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setActiveIndex((i) => (i + 1) % filtered.length);
        return;
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        setActiveIndex((i) => (i - 1 + filtered.length) % filtered.length);
        return;
      }
      if (e.key === 'Tab' || (e.key === 'Enter' && !e.shiftKey)) {
        e.preventDefault();
        const idx = clampActive(filtered.length, activeIndex);
        choose(filtered[idx]);
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        // Clear the leading "/" so popup closes; keep the rest of the text.
        setText((t) => t.replace(/^\//, ''));
        return;
      }
    }
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      submit();
    }
  };

  const disabled = !threadId || turnActive;
  const safeActive = clampActive(filtered.length, activeIndex);

  return (
    <div className="composer" data-testid="composer">
      <ComposerPopup<SlashCommand>
        filteredItems={filtered}
        activeIndex={safeActive}
        onChoose={choose}
        open={popupOpen && filtered.length > 0}
        testId="composer-slash-popup"
        renderItem={(cmd) => (
          <>
            <span className="composer-popup-item-name">/{cmd.name}</span>
            <span className="composer-popup-item-desc">{cmd.description}</span>
          </>
        )}
      />
      <textarea
        className="composer-input"
        data-testid="composer-input"
        placeholder={threadId ? 'type a message…  (Enter to send, Shift+Enter for newline, / for commands)' : 'starting up…'}
        value={text}
        onChange={(e) => {
          setText(e.target.value);
          // Reset to top when the filter could have changed.
          setActiveIndex(0);
        }}
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
