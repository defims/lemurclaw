import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { SettingsForm } from '../SettingsForm';

vi.mock('../../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../../transport';

describe('SettingsForm', () => {
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('loads the value via config/read and shows it in the textarea', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      config: { developerInstructions: 'remember to use rust 2021 edition' },
      origins: {},
      layers: null,
    });
    render(<SettingsForm configKey="developerInstructions" label="Memories" />);
    await waitFor(() => expect(screen.getByLabelText('Memories')).toHaveValue('remember to use rust 2021 edition'));
  });

  it('save button fires config/value/write with the new value', async () => {
    vi.mocked(sendRequest)
      .mockResolvedValueOnce({ config: { developerInstructions: 'old' }, origins: {}, layers: null })
      .mockResolvedValueOnce({}); // write ack
    render(<SettingsForm configKey="developerInstructions" writeKeyPath="developer_instructions" label="Memories" />);
    await waitFor(() => expect(screen.getByLabelText('Memories')).toHaveValue('old'));
    fireEvent.change(screen.getByLabelText('Memories'), { target: { value: 'new' } });
    fireEvent.click(screen.getByRole('button', { name: /save/i }));
    await waitFor(() => {
      expect(sendRequest).toHaveBeenCalledWith('config/value/write', expect.objectContaining({
        keyPath: 'developer_instructions',
        value: 'new',
        mergeStrategy: 'replace',
      }));
    });
  });

  it('defaults writeKeyPath to configKey when not provided', async () => {
    vi.mocked(sendRequest)
      .mockResolvedValueOnce({ config: { model: 'gpt-4' }, origins: {}, layers: null })
      .mockResolvedValueOnce({});
    render(<SettingsForm configKey="model" label="Model id" />);
    await waitFor(() => expect(screen.getByLabelText('Model id')).toHaveValue('gpt-4'));
    fireEvent.change(screen.getByLabelText('Model id'), { target: { value: 'gpt-5' } });
    fireEvent.click(screen.getByRole('button', { name: /save/i }));
    await waitFor(() => {
      expect(sendRequest).toHaveBeenCalledWith('config/value/write', expect.objectContaining({
        keyPath: 'model',
      }));
    });
  });

  it('revert button restores the loaded value', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      config: { developerInstructions: 'loaded' }, origins: {}, layers: null,
    });
    render(<SettingsForm configKey="developerInstructions" label="Memories" />);
    await waitFor(() => expect(screen.getByLabelText('Memories')).toHaveValue('loaded'));
    fireEvent.change(screen.getByLabelText('Memories'), { target: { value: 'edited' } });
    fireEvent.click(screen.getByRole('button', { name: /revert/i }));
    expect(screen.getByLabelText('Memories')).toHaveValue('loaded');
  });

  it('shows error state on failed load', async () => {
    vi.mocked(sendRequest).mockRejectedValue(new Error('nope'));
    render(<SettingsForm configKey="developerInstructions" label="Memories" />);
    await waitFor(() => expect(screen.getByText(/nope/)).toBeInTheDocument());
  });

  it('disables save/revert when not dirty', async () => {
    vi.mocked(sendRequest).mockResolvedValue({
      config: { developerInstructions: 'loaded' }, origins: {}, layers: null,
    });
    render(<SettingsForm configKey="developerInstructions" label="Memories" />);
    await waitFor(() => expect(screen.getByLabelText('Memories')).toHaveValue('loaded'));
    expect(screen.getByRole('button', { name: /save/i })).toBeDisabled();
    expect(screen.getByRole('button', { name: /revert/i })).toBeDisabled();
  });
});
