import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ThemePicker } from '../ThemePicker';

describe('ThemePicker', () => {
  it('lists all themes and highlights current', () => {
    render(<ThemePicker current="dark" onPick={vi.fn()} onClose={vi.fn()} />);
    expect(screen.getByText('Light')).toBeInTheDocument();
    expect(screen.getByText('Dark')).toBeInTheDocument();
    expect(screen.getByText('High contrast')).toBeInTheDocument();
    expect(screen.getByText('Dark').closest('.theme-item')).toHaveClass('theme-item-active');
  });

  it('picking a theme calls onPick', () => {
    const onPick = vi.fn();
    render(<ThemePicker current="light" onPick={onPick} onClose={vi.fn()} />);
    fireEvent.click(screen.getByText('Dark'));
    expect(onPick).toHaveBeenCalledWith('dark');
  });

  it('Esc closes', () => {
    const onClose = vi.fn();
    render(<ThemePicker current="light" onPick={vi.fn()} onClose={onClose} />);
    // Esc is a window listener (matches ModelPicker/TranscriptPager), so
    // dispatch on window rather than the modal-content node.
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });

  it('backdrop click closes', () => {
    const onClose = vi.fn();
    const { container } = render(<ThemePicker current="light" onPick={vi.fn()} onClose={onClose} />);
    fireEvent.click(container.querySelector('.modal-overlay')!);
    expect(onClose).toHaveBeenCalled();
  });
});
