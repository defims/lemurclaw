import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ComposerPopup } from '../ComposerPopup';

interface Item { name: string }

describe('ComposerPopup', () => {
  it('returns null when open=false', () => {
    const { container } = render(
      <ComposerPopup<Item> filteredItems={[]} renderItem={() => null} activeIndex={-1} onChoose={vi.fn()} open={false} />,
    );
    expect(container.querySelector('.composer-popup')).toBeNull();
  });

  it('renders all filteredItems when open', () => {
    render(
      <ComposerPopup<Item>
        filteredItems={[{ name: 'a' }, { name: 'b' }]}
        renderItem={(i) => <span>{i.name}</span>}
        activeIndex={-1}
        onChoose={vi.fn()}
        open
      />,
    );
    expect(screen.getByText('a')).toBeInTheDocument();
    expect(screen.getByText('b')).toBeInTheDocument();
  });

  it('marks the activeIndex row with composer-popup-item-active', () => {
    render(
      <ComposerPopup<Item>
        filteredItems={[{ name: 'a' }, { name: 'b' }]}
        renderItem={(i) => <span>{i.name}</span>}
        activeIndex={1}
        onChoose={vi.fn()}
        open
      />,
    );
    const items = screen.getAllByRole('option');
    expect(items[1]).toHaveClass('composer-popup-item-active');
    expect(items[0]).not.toHaveClass('composer-popup-item-active');
  });

  it('clicking a row fires onChoose with that item', () => {
    const onChoose = vi.fn();
    render(
      <ComposerPopup<Item>
        filteredItems={[{ name: 'a' }, { name: 'b' }]}
        renderItem={(i) => <span>{i.name}</span>}
        activeIndex={0}
        onChoose={onChoose}
        open
      />,
    );
    fireEvent.click(screen.getByText('b'));
    expect(onChoose).toHaveBeenCalledWith({ name: 'b' });
  });

  it('shows emptyText when filteredItems is empty', () => {
    render(
      <ComposerPopup<Item>
        filteredItems={[]}
        renderItem={() => null}
        activeIndex={-1}
        onChoose={vi.fn()}
        open
        emptyText="no commands"
      />,
    );
    expect(screen.getByText('no commands')).toBeInTheDocument();
  });

  it('passes the correct isActive flag to renderItem', () => {
    const renderItem = vi.fn(() => <span />);
    render(
      <ComposerPopup<Item>
        filteredItems={[{ name: 'a' }, { name: 'b' }]}
        renderItem={renderItem}
        activeIndex={0}
        onChoose={vi.fn()}
        open
      />,
    );
    expect(renderItem).toHaveBeenCalledWith({ name: 'a' }, true);
    expect(renderItem).toHaveBeenCalledWith({ name: 'b' }, false);
  });

  it('uses listbox + option aria roles', () => {
    render(
      <ComposerPopup<Item>
        filteredItems={[{ name: 'a' }]}
        renderItem={(i) => <span>{i.name}</span>}
        activeIndex={0}
        onChoose={vi.fn()}
        open
      />,
    );
    expect(screen.getByRole('listbox')).toBeInTheDocument();
    expect(screen.getByRole('option')).toBeInTheDocument();
  });
});
