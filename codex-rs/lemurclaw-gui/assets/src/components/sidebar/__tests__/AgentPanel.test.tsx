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
    expect(screen.getByText(/active/)).toBeInTheDocument();
    expect(screen.getByText(/waitingOnApproval/)).toBeInTheDocument();
  });

  it('shows deferral hint for sub-agents', () => {
    render(<AgentPanel state={initialState} />);
    expect(screen.getByText(/sub-agent control deferred/)).toBeInTheDocument();
  });
});
