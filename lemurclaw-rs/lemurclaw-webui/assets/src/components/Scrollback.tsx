import { useEffect, useRef } from 'react';
import type { ConversationState, CellModel } from '../viewModel/types';
import { UserMessageCell } from './cells/UserMessageCell';
import { AgentMessageCell } from './cells/AgentMessageCell';
import { ReasoningCell } from './cells/ReasoningCell';
import { CommandExecCell } from './cells/CommandExecCell';
import { FileChangeCell } from './cells/FileChangeCell';
import { McpToolCell } from './cells/McpToolCell';
import { PlanCell } from './cells/PlanCell';
import { HookCell } from './cells/HookCell';
import { WebSearchCell } from './cells/WebSearchCell';

interface Props {
  state: ConversationState;
  /** Optional: called when the user clicks "view full diff" on a patch cell.
   *  Absent in read-only contexts (TranscriptPager) — patch cells then omit
   *  the button. Receives the cell so App can extract that cell's diff. */
  onViewDiff?: (cell: CellModel) => void;
}

/** Scrollback: the main conversation region. Renders every turn's items in
 *  order, auto-scrolls to the bottom when new content arrives (unless the
 *  user has scrolled up to read history — detected via scroll position). */
export function Scrollback({ state, onViewDiff }: Props) {
  const bottomRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const stickToBottomRef = useRef(true);

  const onScroll = () => {
    const el = containerRef.current;
    if (!el) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
    stickToBottomRef.current = atBottom;
  };

  useEffect(() => {
    if (stickToBottomRef.current) {
      bottomRef.current?.scrollIntoView({ block: 'end' });
    }
  });

  if (state.turns.length === 0) {
    return (
      <div className="scrollback scrollback-empty" ref={containerRef}>
        <div className="scrollback-placeholder">send a message to start</div>
        <div ref={bottomRef} />
      </div>
    );
  }

  return (
    <div className="scrollback" ref={containerRef} onScroll={onScroll}>
      {state.turns.flatMap((turn) =>
        turn.items.map((cell) => (
          <CellRenderer key={cellKey(cell)} cell={cell} onViewDiff={onViewDiff} />
        )),
      )}
      <div ref={bottomRef} />
    </div>
  );
}

/** Render a single CellModel via the shared cell-component switch.
 *
 *  Used by Scrollback (live conversation region) and TranscriptPager (read-only
 *  full-transcript overlay). Both render the same CellModel shape produced by
 *  `viewModel/reducer.ts::threadItemToCell`, so the visual output is identical
 *  between the live view and the historical review. */
export function CellRenderer({ cell, onViewDiff }: { cell: CellModel; onViewDiff?: (cell: CellModel) => void }) {
  switch (cell.kind) {
    case 'userMessage': return <UserMessageCell model={cell} />;
    case 'agentMessage': return <AgentMessageCell model={cell} />;
    case 'reasoning': return <ReasoningCell model={cell} />;
    case 'commandExecution': return <CommandExecCell model={cell} />;
    case 'fileChange':
      // Only forward onViewDiff when caller actually provided it (absent in
      // TranscriptPager's read-only rendering — keeps the button hidden there).
      return <FileChangeCell model={cell} onViewDiff={onViewDiff ? () => onViewDiff(cell) : undefined} />;
    case 'mcpToolCall': return <McpToolCell model={cell} />;
    case 'plan': return <PlanCell model={cell} />;
    case 'hook': return <HookCell model={cell} />;
    case 'webSearch': return <WebSearchCell model={cell} />;
    case 'dynamicToolCall':
    case 'imageGeneration':
    case 'sleep':
    case 'generic':
      // Subproject 3 renders these as a minimal placeholder; full coverage in
      // later subprojects.
      return (
        <div className="cell cell-generic" data-testid="generic-cell">
          <pre>{cell.kind === 'generic' ? cell.rawType : cell.kind}</pre>
        </div>
      );
  }
}

/** Stable React key for a CellModel. Prefers the cell's `itemId` (unique per
 *  thread); hook cells key off their run id. Shared so TranscriptPager produces
 *  the same keys as Scrollback for the same items. */
export function cellKey(cell: CellModel): string {
  if (cell.kind === 'hook') return `hook:${cell.run.id}`;
  return `${cell.kind}:${cell.itemId}`;
}
