import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SettingsModal } from '../SettingsModal';

describe('SettingsModal', () => {
  it('renders surface list in left nav', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    expect(screen.getByText('Permissions')).toBeInTheDocument();
    expect(screen.getByText('Keymap')).toBeInTheDocument();
    expect(screen.getByText('Skills')).toBeInTheDocument();
    expect(screen.getByText('Plugins')).toBeInTheDocument();
    expect(screen.getByText('Experimental')).toBeInTheDocument();
  });

  it('defaults to first surface selected with its placeholder', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    // First surface is "Permissions"
    expect(screen.getByText('Permissions').closest('.settings-nav-item')).toHaveClass('settings-nav-item-active');
    expect(screen.getByText(/permissions panel/i)).toBeInTheDocument();
  });

  it('clicking a surface swaps the right pane placeholder', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    fireEvent.click(screen.getByText('Plugins'));
    expect(screen.getByText('Plugins').closest('.settings-nav-item')).toHaveClass('settings-nav-item-active');
    expect(screen.getByText(/plugins panel/i)).toBeInTheDocument();
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
