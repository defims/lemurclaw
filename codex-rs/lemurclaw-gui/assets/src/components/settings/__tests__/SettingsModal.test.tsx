import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SettingsModal } from '../SettingsModal';

describe('SettingsModal', () => {
  it('renders surface list in left nav', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    expect(screen.getByText('Permissions')).toBeInTheDocument();
    expect(screen.getByText('Memories')).toBeInTheDocument();
    expect(screen.getByText('Model')).toBeInTheDocument();
    expect(screen.getByText('Skills')).toBeInTheDocument();
    expect(screen.getByText('Plugins')).toBeInTheDocument();
    expect(screen.getByText('Experimental')).toBeInTheDocument();
  });

  it('defaults to first surface selected with its panel rendered', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    // First surface is "Permissions" — its panel renders (no placeholder).
    expect(screen.getByText('Permissions').closest('.settings-nav-item')).toHaveClass('settings-nav-item-active');
    expect(screen.getByTestId('settings-pane-permissions')).toBeInTheDocument();
  });

  it('clicking a different surface swaps the right pane', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    fireEvent.click(screen.getByText('Model'));
    expect(screen.getByText('Model').closest('.settings-nav-item')).toHaveClass('settings-nav-item-active');
    expect(screen.getByTestId('settings-pane-model')).toBeInTheDocument();
  });

  it('Esc closes (via shared <Modal>)', () => {
    const onClose = vi.fn();
    render(<SettingsModal onClose={onClose} />);
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });

  it('close button closes', () => {
    const onClose = vi.fn();
    render(<SettingsModal onClose={onClose} />);
    fireEvent.click(screen.getByLabelText('close'));
    expect(onClose).toHaveBeenCalled();
  });

  it('has data-testid settings-modal on overlay', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    expect(screen.getByTestId('settings-modal')).toBeInTheDocument();
  });
});
