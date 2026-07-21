import { describe, it, expect, beforeAll } from 'vitest';
import { render, screen } from '@testing-library/react';
import { Scrollback } from '../Scrollback';
import { initialState } from '../../viewModel/types';
import type { ConversationState } from '../../viewModel/types';

// jsdom does not implement Element.prototype.scrollIntoView (the component is
// correct in a real browser). Polyfill a no-op so the auto-scroll useEffect
// doesn't throw during these tests.
beforeAll(() => {
  if (!window.Element.prototype.scrollIntoView) {
    window.Element.prototype.scrollIntoView = () => {};
  }
});

describe('Scrollback', () => {
  it('shows placeholder when empty', () => {
    render(<Scrollback state={initialState} />);
    expect(screen.getByText('send a message to start')).toBeInTheDocument();
  });

  it('renders a user message cell for a userMessage item', () => {
    const state: ConversationState = {
      ...initialState,
      turns: [{
        id: 'tu1', status: 'inProgress', startedAt: 1, completedAt: null,
        items: [{ kind: 'userMessage', itemId: 'u1', text: 'hi' }],
      }],
    };
    render(<Scrollback state={state} />);
    expect(screen.getByTestId('user-message')).toHaveTextContent('hi');
  });

  it('renders mixed cells in order', () => {
    const state: ConversationState = {
      ...initialState,
      turns: [{
        id: 'tu1', status: 'inProgress', startedAt: 1, completedAt: null,
        items: [
          { kind: 'userMessage', itemId: 'u1', text: 'hi' },
          { kind: 'agentMessage', itemId: 'a1', text: 'hello', phase: null },
        ],
      }],
    };
    const { container } = render(<Scrollback state={state} />);
    expect(screen.getByTestId('user-message')).toBeInTheDocument();
    expect(screen.getByTestId('agent-message')).toBeInTheDocument();
    // Verify DOM order: user-message must come before agent-message (catches
    // a regression where turns[].items is reversed or flattened wrong).
    const cells = container.querySelectorAll('[data-testid="user-message"], [data-testid="agent-message"]');
    expect(cells[0]).toHaveAttribute('data-testid', 'user-message');
    expect(cells[1]).toHaveAttribute('data-testid', 'agent-message');
  });
});
