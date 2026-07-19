import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ReasoningCell } from '../ReasoningCell';

describe('ReasoningCell', () => {
  it('shows summary by default, hides content until expanded', () => {
    render(<ReasoningCell model={{ kind: 'reasoning', itemId: 'r1', summary: ['short'], content: ['long detail'] }} />);
    expect(screen.getByText('short')).toBeInTheDocument();
    expect(screen.queryByText('long detail')).toBeNull();
    fireEvent.click(screen.getByRole('button'));
    expect(screen.getByText('long detail')).toBeInTheDocument();
  });

  it('renders empty marker when no content', () => {
    render(<ReasoningCell model={{ kind: 'reasoning', itemId: 'r1', summary: [], content: [] }} />);
    expect(screen.getByTestId('reasoning')).toHaveTextContent('(empty)');
  });
});
