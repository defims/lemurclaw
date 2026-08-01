import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { McpPanel } from '../McpPanel';

vi.mock('../../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../../transport';

describe('McpPanel', () => {
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('lists MCP servers with tool count', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      data: [
        { name: 'fs', serverInfo: { name: 'fs', version: '1.0' }, tools: { a: {}, b: {} }, resources: [], resourceTemplates: [], authStatus: 'ok' as never },
      ],
    });
    render(<McpPanel />);
    await waitFor(() => expect(screen.getByText('fs')).toBeInTheDocument());
    expect(screen.getByText(/2 tools/)).toBeInTheDocument();
  });

  it('shows empty state', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ data: [] });
    render(<McpPanel />);
    await waitFor(() => expect(screen.getByText(/no mcp servers/i)).toBeInTheDocument());
  });
});
