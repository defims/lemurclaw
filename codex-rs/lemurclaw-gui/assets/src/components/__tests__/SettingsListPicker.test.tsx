import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SettingsListPicker } from '../SettingsListPicker';

interface Item { id: string; label: string; sub?: string; disabled?: boolean }

describe('SettingsListPicker', () => {
  it('renders loading state', () => {
    render(
      <SettingsListPicker<Item>
        state={{ loading: true, error: null, items: [] }}
        getId={(i) => i.id}
        renderLabel={(i) => i.label}
        renderSub={(i) => i.sub}
        isDisabled={(i) => i.disabled ?? false}
      />,
    );
    expect(screen.getByText('loading…')).toBeInTheDocument();
  });

  it('renders error state', () => {
    render(
      <SettingsListPicker<Item>
        state={{ loading: false, error: 'oops', items: [] }}
        getId={(i) => i.id}
        renderLabel={(i) => i.label}
      />,
    );
    expect(screen.getByText(/failed: oops/)).toBeInTheDocument();
  });

  it('renders empty state', () => {
    render(
      <SettingsListPicker<Item>
        state={{ loading: false, error: null, items: [] }}
        getId={(i) => i.id}
        renderLabel={(i) => i.label}
        emptyText="nothing here"
      />,
    );
    expect(screen.getByText('nothing here')).toBeInTheDocument();
  });

  it('renders items and highlights active', () => {
    render(
      <SettingsListPicker<Item>
        state={{ loading: false, error: null, items: [
          { id: 'a', label: 'Alpha' },
          { id: 'b', label: 'Beta' },
        ] }}
        getId={(i) => i.id}
        renderLabel={(i) => i.label}
        activeId="b"
      />,
    );
    expect(screen.getByText('Alpha')).toBeInTheDocument();
    expect(screen.getByText('Beta')).toBeInTheDocument();
    expect(screen.getByText('Beta').closest('.settings-list-item')).toHaveClass('settings-list-item-active');
  });

  it('fires onActivate when an enabled item is clicked', () => {
    const onActivate = vi.fn();
    render(
      <SettingsListPicker<Item>
        state={{ loading: false, error: null, items: [{ id: 'a', label: 'Alpha' }] }}
        getId={(i) => i.id}
        renderLabel={(i) => i.label}
        onActivate={onActivate}
      />,
    );
    fireEvent.click(screen.getByText('Alpha'));
    expect(onActivate).toHaveBeenCalledWith({ id: 'a', label: 'Alpha' });
  });

  it('does not fire onActivate when disabled', () => {
    const onActivate = vi.fn();
    render(
      <SettingsListPicker<Item>
        state={{ loading: false, error: null, items: [{ id: 'a', label: 'Alpha', disabled: true }] }}
        getId={(i) => i.id}
        renderLabel={(i) => i.label}
        isDisabled={(i) => i.disabled ?? false}
        onActivate={onActivate}
      />,
    );
    fireEvent.click(screen.getByText('Alpha'));
    expect(onActivate).not.toHaveBeenCalled();
  });

  it('renders trailing action button when renderAction provided', () => {
    render(
      <SettingsListPicker<Item>
        state={{ loading: false, error: null, items: [{ id: 'a', label: 'Alpha' }] }}
        getId={(i) => i.id}
        renderLabel={(i) => i.label}
        renderAction={(i) => (
          <button data-testid={`act-${i.id}`} onClick={vi.fn()}>uninstall</button>
        )}
      />,
    );
    expect(screen.getByTestId('act-a')).toBeInTheDocument();
  });
});
