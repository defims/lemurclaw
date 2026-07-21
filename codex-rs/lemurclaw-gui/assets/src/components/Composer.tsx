import { useState, useCallback, useMemo, useRef, useEffect } from 'react';
import { sendRequest } from '../transport';
import { ComposerPopup } from './ComposerPopup';
import { SLASH_COMMANDS } from './composer/slashCommands';
import type { SlashCommand } from './composer/slashCommandTypes';
import type { FuzzyFileSearchResult } from '../types/FuzzyFileSearchResult';

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
  /** Working directory (from ConversationState.cwd). Used as the root for
   *  fuzzy file search sessions when the user types "@". */
  cwd: string | null;
  /** Latest fuzzy-search results mirrored from the server (reducer-cached).
   *  Null/empty when no @-mention popup is active. */
  fuzzyFiles: FuzzyFileSearchResult[];
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

/** Returns the @-mention query being typed at the cursor position, or null
 *  if the cursor isn't right after an @-mention token.
 *
 *  An @-mention starts when "@" appears at the start of the text OR right
 *  after a whitespace character. The query is the contiguous non-whitespace
 *  text after "@" up to the cursor.
 *
 *  "@co"        → "co"
 *  "hello @co"  → "co"
 *  "@"          → "" (popup trigger)
 *  "hello@"     → null (no preceding whitespace, not a mention)
 *  "hello"      → null
 */
function mentionQuery(text: string, cursor: number): string | null {
  // Walk back from cursor to find "@" preceded by start-of-text or whitespace.
  const upto = text.slice(0, cursor);
  // Match an @ that's at the start or after whitespace, then capture the
  // query (non-whitespace) up to the cursor.
  const m = upto.match(/(?:^|\s)@(\S*)$/);
  return m ? m[1] : null;
}

/** Composer: textarea + send button + slash command popup + @mention popup.
 *  Enter sends (shift+enter newline). Typing a leading "/" opens the slash
 *  popup; typing "@" (at start or after whitespace) opens the file-mention
 *  popup. Arrow keys / Tab / Enter select; Esc closes.
 *
 *  Slash dispatch is delegated to the parent via onSlashCommand. The @mention
 *  popup is fully Composer-owned: it fires fuzzyFileSearch/session* RPCs
 *  directly via transport and consumes the reducer-mirrored results passed in
 *  via `fuzzyFiles`. */
export function Composer({ threadId, turnActive, onInterrupt, startTurn, onSlashCommand, cwd, fuzzyFiles }: Props) {
  const [text, setText] = useState('');
  const [activeIndex, setActiveIndex] = useState(0);
  const [cursorPos, setCursorPos] = useState(0);
  /** sessionId for the active fuzzy search session. Null when no @-popup is
   *  open. Held in a ref so the session-end effect can read the latest value
   *  without re-running on every keystroke. */
  const sessionIdRef = useRef<string | null>(null);
  /** Track the textarea ref so we can read selectionStart on change. */
  const taRef = useRef<HTMLTextAreaElement>(null);

  const token = slashToken(text);
  const slashPopupOpen = token !== null;
  const filtered = useMemo(() => {
    if (token === null) return [];
    return SLASH_COMMANDS.filter((c) => token === '' || c.name.startsWith(token));
  }, [token]);

  const mentionQ = useMemo(() => mentionQuery(text, cursorPos), [text, cursorPos]);
  const mentionPopupOpen = !slashPopupOpen && mentionQ !== null && !!cwd;

  // Reset active index whenever the active popup's filter set changes.
  const clampActive = (len: number, idx: number) => (len === 0 ? 0 : Math.min(idx, len - 1));

  // ----- Fuzzy session lifecycle -----
  // Start a session the first time @-popup opens; update on query change;
  // stop when popup closes. We compare against the previous state via the ref
  // so we only fire RPCs on actual transitions.
  useEffect(() => {
    if (!mentionPopupOpen) {
      // Popup closed: stop the session if one was open.
      const sid = sessionIdRef.current;
      if (sid) {
        sendRequest('fuzzyFileSearch/sessionStop', { sessionId: sid }).catch(() => {
          // Best-effort — server may have already dropped the session.
        });
        sessionIdRef.current = null;
      }
      return;
    }
    // Popup open: ensure we have a session, then update the query.
    if (!sessionIdRef.current) {
      const sid = `fuzzy-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
      sessionIdRef.current = sid;
      const roots = cwd ? [cwd] : [];
      sendRequest('fuzzyFileSearch/sessionStart', { sessionId: sid, roots }).catch(() => {
        // If start fails, clear the ref so a retry can begin next keystroke.
        sessionIdRef.current = null;
      });
    }
    const sid = sessionIdRef.current;
    if (sid) {
      sendRequest('fuzzyFileSearch/sessionUpdate', { sessionId: sid, query: mentionQ ?? '' }).catch(() => {
        /* ignore — best-effort */
      });
    }
  }, [mentionPopupOpen, mentionQ, cwd]);

  // Stop the session on unmount too.
  useEffect(() => {
    return () => {
      const sid = sessionIdRef.current;
      if (sid) {
        sendRequest('fuzzyFileSearch/sessionStop', { sessionId: sid }).catch(() => {});
        sessionIdRef.current = null;
      }
    };
  }, []);

  // ----- Slash popup: pick a command -----
  const choose = (cmd: SlashCommand) => {
    // Strip the leading "/<name>" + any whitespace after it to get args.
    const re = new RegExp(`^/${cmd.name}\\s*`);
    const args = text.replace(re, '');
    onSlashCommand(cmd, args);
    setText('');
  };

  // ----- Mention popup: replace @query with @path -----
  const chooseMention = (file: FuzzyFileSearchResult) => {
    const before = text.slice(0, cursorPos);
    const after = text.slice(cursorPos);
    // Replace the trailing "@<query>" in `before` with "@<path>".
    const newBefore = before.replace(/@(?:^|\s)?(\S*)$/, `@${file.path} `);
    const next = newBefore + after;
    setText(next);
    // Move cursor to right after the inserted path + space.
    const newPos = newBefore.length;
    setCursorPos(newPos);
    // Focus + set selection on next tick (textarea may not have updated yet).
    requestAnimationFrame(() => {
      const ta = taRef.current;
      if (ta) {
        ta.focus();
        ta.setSelectionRange(newPos, newPos);
      }
    });
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
    // Mention popup takes precedence when open (slash popup is mutually exclusive).
    if (mentionPopupOpen && fuzzyFiles.length > 0) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setActiveIndex((i) => (i + 1) % fuzzyFiles.length);
        return;
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        setActiveIndex((i) => (i - 1 + fuzzyFiles.length) % fuzzyFiles.length);
        return;
      }
      if (e.key === 'Tab' || (e.key === 'Enter' && !e.shiftKey)) {
        e.preventDefault();
        const idx = clampActive(fuzzyFiles.length, activeIndex);
        chooseMention(fuzzyFiles[idx]);
        setActiveIndex(0);
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        // Strip the trailing "@" so popup closes.
        const before = text.slice(0, cursorPos);
        const after = text.slice(cursorPos);
        setText(before.replace(/@(?:^|\s)?(\S*)$/, '') + after);
        return;
      }
    }
    if (slashPopupOpen && filtered.length > 0) {
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
  const safeMentionActive = clampActive(fuzzyFiles.length, activeIndex);

  return (
    <div className="composer" data-testid="composer">
      <ComposerPopup<SlashCommand>
        filteredItems={filtered}
        activeIndex={safeActive}
        onChoose={choose}
        open={slashPopupOpen && filtered.length > 0}
        testId="composer-slash-popup"
        renderItem={(cmd) => (
          <>
            <span className="composer-popup-item-name">/{cmd.name}</span>
            <span className="composer-popup-item-desc">{cmd.description}</span>
          </>
        )}
      />
      <ComposerPopup<FuzzyFileSearchResult>
        filteredItems={fuzzyFiles}
        activeIndex={safeMentionActive}
        onChoose={(f) => { chooseMention(f); setActiveIndex(0); }}
        open={mentionPopupOpen && fuzzyFiles.length > 0}
        testId="composer-mention-popup"
        emptyText="no matching files"
        renderItem={(f) => (
          <>
            <span className="composer-popup-item-name">{f.file_name}</span>
            <span className="composer-popup-item-desc">{f.path}</span>
          </>
        )}
      />
      <textarea
        ref={taRef}
        className="composer-input"
        data-testid="composer-input"
        placeholder={threadId ? 'type a message…  (Enter to send, / for commands, @ for files)' : 'starting up…'}
        value={text}
        onChange={(e) => {
          setText(e.target.value);
          setCursorPos(e.target.selectionStart);
          // Reset to top when the filter could have changed.
          setActiveIndex(0);
        }}
        onKeyUp={(e) => setCursorPos((e.target as HTMLTextAreaElement).selectionStart)}
        onClick={(e) => setCursorPos((e.target as HTMLTextAreaElement).selectionStart)}
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
