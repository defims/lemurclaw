import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { UserMessageCell } from '../UserMessageCell';

describe('UserMessageCell', () => {
  it('renders the message text', () => {
    render(<UserMessageCell model={{ kind: 'userMessage', itemId: 'u1', text: 'hello world' }} />);
    expect(screen.getByTestId('user-message')).toHaveTextContent('hello world');
  });
});
