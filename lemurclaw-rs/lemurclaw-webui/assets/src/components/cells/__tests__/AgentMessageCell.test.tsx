import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { AgentMessageCell } from '../AgentMessageCell';

describe('AgentMessageCell', () => {
  it('renders streamed text with thinking marker when phase is null', () => {
    render(<AgentMessageCell model={{ kind: 'agentMessage', itemId: 'a1', text: 'let me think', phase: null }} />);
    expect(screen.getByTestId('agent-message')).toHaveTextContent('let me think');
    expect(screen.getByTestId('agent-message')).toHaveTextContent('thinking');
  });

  it('marks final_answer without the thinking suffix', () => {
    render(<AgentMessageCell model={{ kind: 'agentMessage', itemId: 'a1', text: 'done', phase: 'final_answer' }} />);
    const cell = screen.getByTestId('agent-message');
    expect(cell).toHaveTextContent('done');
    expect(cell).not.toHaveTextContent('thinking');
    expect(cell.className).toContain('cell-agent-final');
  });
});
