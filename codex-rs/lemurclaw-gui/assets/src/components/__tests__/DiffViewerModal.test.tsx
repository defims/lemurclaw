import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { DiffViewerModal } from '../DiffViewerModal';

const SAMPLE_DIFF =
  'diff --git a/x.txt b/x.txt\n' +
  '+++ b/x.txt\n' +
  '@@ -0,0 +1 @@\n' +
  '+hello\n';

describe('DiffViewerModal', () => {
  it('renders data-testid diff-viewer-modal', () => {
    render(<DiffViewerModal diff={SAMPLE_DIFF} onClose={vi.fn()} />);
    expect(screen.getByTestId('diff-viewer-modal')).toBeInTheDocument();
  });

  it('renders DiffText (data-testid diff-text) when diff is non-empty', () => {
    render(<DiffViewerModal diff={SAMPLE_DIFF} onClose={vi.fn()} />);
    expect(screen.getByTestId('diff-text')).toBeInTheDocument();
  });

  it('renders empty-state when diff is empty', () => {
    render(<DiffViewerModal diff="" onClose={vi.fn()} />);
    expect(screen.getByTestId('diff-viewer-empty')).toBeInTheDocument();
    expect(screen.getByText(/no diff in this turn/i)).toBeInTheDocument();
  });

  it('Esc closes (inherited from <Modal>)', () => {
    const onClose = vi.fn();
    render(<DiffViewerModal diff="" onClose={onClose} />);
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });

  it('close button (✕) closes', () => {
    const onClose = vi.fn();
    render(<DiffViewerModal diff="" onClose={onClose} />);
    fireEvent.click(screen.getByLabelText('close'));
    expect(onClose).toHaveBeenCalled();
  });
});
