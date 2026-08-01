import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { McpToolCell } from '../McpToolCell';

describe('McpToolCell', () => {
  it('shows server.tool + status, reveals args on expand', () => {
    render(<McpToolCell model={{
      kind: 'mcpToolCall', itemId: 'm1', server: 'fs', tool: 'read',
      status: 'completed', arguments: { path: '/x' }, progress: ['reading'],
      result: { content: 'hi' }, error: null,
    }} />);
    expect(screen.getByText(/fs\.read/)).toBeInTheDocument();
    expect(screen.getByText('reading')).toBeInTheDocument();
    expect(screen.queryByText('"path"')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'more' }));
    expect(screen.getByText(/"path"/)).toBeInTheDocument();
  });
});
