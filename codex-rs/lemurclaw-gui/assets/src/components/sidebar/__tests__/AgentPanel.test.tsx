import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { AgentPanel } from '../AgentPanel';
import { initialState } from '../../../viewModel/types';
import type { ConversationState } from '../../../viewModel/types';

describe('AgentPanel', () => {
  it('shows not-started when status is null', () => {
    render(<AgentPanel state={initialState} />);
    expect(screen.getByText('(not started)')).toBeInTheDocument();
  });

  it('shows idle status', () => {
    const state: ConversationState = { ...initialState, status: { type: 'idle' } };
    render(<AgentPanel state={state} />);
    expect(screen.getByText('idle')).toBeInTheDocument();
  });

  it('shows active status with flags', () => {
    // ThreadActiveFlag is "waitingOnApproval" | "waitingOnUserInput" per
    // types/v2/ThreadActiveFlag.ts — use a real value the reducer can emit.
    const state: ConversationState = {
      ...initialState,
      status: { type: 'active', activeFlags: ['waitingOnApproval'] } as never,
    };
    render(<AgentPanel state={state} />);
    // Anchored to "^active ·" so it doesn't also match the "no sub-agents
    // active" empty-hint (Task 5.3) in the same render.
    expect(screen.getByText(/^active ·/)).toBeInTheDocument();
    expect(screen.getByText(/waitingOnApproval/)).toBeInTheDocument();
  });

  it('shows empty hint when no sub-agents', () => {
    render(<AgentPanel state={initialState} />);
    expect(screen.getByText(/no sub-agents active/)).toBeInTheDocument();
  });

  it('renders sub-agent rows from state.subAgents', () => {
    const state: ConversationState = {
      ...initialState,
      status: { type: 'idle' },
      subAgents: [
        { threadId: 'sub1', status: 'running', message: null },
        { threadId: 'sub2', status: 'completed', message: 'done' },
      ],
    };
    render(<AgentPanel state={state} />);
    expect(screen.getByText('sub1')).toBeInTheDocument();
    expect(screen.getByText('sub2')).toBeInTheDocument();
    expect(screen.getByText('running')).toBeInTheDocument();
    expect(screen.getByText('completed')).toBeInTheDocument();
  });
});
