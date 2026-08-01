import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoriesPanel } from '../MemoriesPanel';

vi.mock('../../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../../transport';

describe('MemoriesPanel', () => {
  it('renders a SettingsForm labeled "Memories"', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ config: {}, origins: {}, layers: null });
    render(<MemoriesPanel />);
    await waitFor(() => expect(screen.getByLabelText('Memories')).toBeInTheDocument());
  });
});
