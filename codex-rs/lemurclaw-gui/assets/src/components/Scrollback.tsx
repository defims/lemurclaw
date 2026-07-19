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
}

/** Scrollback: the main conversation region. Renders every turn's items in
 *  order, auto-scrolls to the bottom when new content arrives (unless the
 *  user has scrolled up to read history — detected via scroll position). */
export function Scrollback({ state }: Props) {
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
        turn.items.map((cell) => <CellRenderer key={cellKey(cell)} cell={cell} />),
      )}
      <div ref={bottomRef} />
    </div>
  );
}

function CellRenderer({ cell }: { cell: CellModel }) {
  switch (cell.kind) {
    case 'userMessage': return <UserMessageCell model={cell} />;
    case 'agentMessage': return <AgentMessageCell model={cell} />;
    case 'reasoning': return <ReasoningCell model={cell} />;
    case 'commandExecution': return <CommandExecCell model={cell} />;
    case 'fileChange': return <FileChangeCell model={cell} />;
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

function cellKey(cell: CellModel): string {
  if (cell.kind === 'hook') return `hook:${cell.run.id}`;
  return `${cell.kind}:${cell.itemId}`;
}
