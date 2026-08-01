import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { WebSearchCell } from '../WebSearchCell';

describe('WebSearchCell', () => {
  it('renders query + status', () => {
    render(<WebSearchCell model={{ kind: 'webSearch', itemId: 'w1', query: 'rust async', status: 'completed' }} />);
    expect(screen.getByTestId('websearch')).toHaveTextContent('rust async');
    expect(screen.getByTestId('websearch')).toHaveTextContent('completed');
  });
});
