import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { ModelPanel } from '../ModelPanel';

vi.mock('../../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../../transport';

describe('ModelPanel', () => {
  it('renders SettingsForms for model, provider, reasoning effort, verbosity', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ config: {}, origins: {}, layers: null });
    render(<ModelPanel />);
    await waitFor(() => expect(screen.getByLabelText('Model id')).toBeInTheDocument());
    expect(screen.getByLabelText('Model provider')).toBeInTheDocument();
    expect(screen.getByLabelText('Reasoning effort')).toBeInTheDocument();
    expect(screen.getByLabelText('Verbosity')).toBeInTheDocument();
  });
});
