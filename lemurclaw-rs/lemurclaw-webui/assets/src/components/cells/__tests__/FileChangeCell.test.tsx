import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { FileChangeCell } from '../FileChangeCell';

describe('FileChangeCell', () => {
  it('lists files with kind markers and hides diff until clicked', () => {
    render(<FileChangeCell model={{
      kind: 'fileChange', itemId: 'p1', status: 'completed',
      changes: [
        { path: 'src/a.rs', kind: { type: 'add' }, diff: '+new' },
        { path: 'src/b.rs', kind: { type: 'update', move_path: null }, diff: '-old\n+new' },
      ],
    }} />);
    expect(screen.getByText('src/a.rs')).toBeInTheDocument();
    expect(screen.getByText('src/b.rs')).toBeInTheDocument();
    expect(screen.queryByText('+new')).toBeNull();
    fireEvent.click(screen.getByText('src/a.rs'));
    expect(screen.getByText('+new')).toBeInTheDocument();
    expect(screen.getByText('applied')).toBeInTheDocument();
  });

  it('does not render "view full diff" button when onViewDiff is absent', () => {
    render(<FileChangeCell model={{
      kind: 'fileChange', itemId: 'p1', status: 'completed',
      changes: [{ path: 'a.rs', kind: { type: 'add' }, diff: '+x' }],
    }} />);
    expect(screen.queryByTestId('patch-view-diff')).toBeNull();
    expect(screen.queryByLabelText('view full diff')).toBeNull();
  });

  it('renders "view full diff" button and fires onViewDiff on click', () => {
    const onViewDiff = vi.fn();
    render(<FileChangeCell model={{
      kind: 'fileChange', itemId: 'p1', status: 'completed',
      changes: [{ path: 'a.rs', kind: { type: 'add' }, diff: '+x' }],
    }} onViewDiff={onViewDiff} />);
    fireEvent.click(screen.getByTestId('patch-view-diff'));
    expect(onViewDiff).toHaveBeenCalledTimes(1);
  });
});
