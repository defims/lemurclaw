import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Modal } from '../Modal';

describe('Modal', () => {
  it('renders title and children', () => {
    render(
      <Modal title="select model" onClose={vi.fn()}>
        <div>body content</div>
      </Modal>,
    );
    expect(screen.getByText('select model')).toBeInTheDocument();
    expect(screen.getByText('body content')).toBeInTheDocument();
  });

  it('Esc closes via window listener', () => {
    const onClose = vi.fn();
    render(<Modal title="t" onClose={onClose}><div /></Modal>);
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });

  it('backdrop click closes', () => {
    const onClose = vi.fn();
    const { container } = render(<Modal title="t" onClose={onClose}><div /></Modal>);
    fireEvent.click(container.querySelector('.modal-overlay')!);
    expect(onClose).toHaveBeenCalled();
  });

  it('click inside content does not close', () => {
    const onClose = vi.fn();
    render(
      <Modal title="t" onClose={onClose}>
        <button>inside</button>
      </Modal>,
    );
    fireEvent.click(screen.getByText('inside'));
    expect(onClose).not.toHaveBeenCalled();
  });

  it('close button closes', () => {
    const onClose = vi.fn();
    render(<Modal title="t" onClose={onClose}><div /></Modal>);
    fireEvent.click(screen.getByLabelText('close'));
    expect(onClose).toHaveBeenCalled();
  });

  it('applies data-testid when provided', () => {
    render(
      <Modal title="t" onClose={vi.fn()} testId="model-picker">
        <div />
      </Modal>,
    );
    expect(screen.getByTestId('model-picker')).toBeInTheDocument();
  });

  it('applies wide class when wide=true', () => {
    const { container } = render(
      <Modal title="t" onClose={vi.fn()} wide>
        <div />
      </Modal>,
    );
    expect(container.querySelector('.modal-content')).toHaveClass('modal-content-wide');
  });

  it('passes through surface-specific className props (for full-screen surfaces)', () => {
    const { container } = render(
      <Modal
        title="t"
        onClose={vi.fn()}
        overlayClassName="transcript-pager-overlay"
        contentClassName="transcript-pager-content"
        headerClassName="transcript-pager-header"
        titleClassName="transcript-pager-title"
        closeClassName="transcript-pager-close"
        bodyClassName="transcript-pager-body"
      >
        <div />
      </Modal>,
    );
    // Each element gets BOTH the base modal-* class and the surfaced class.
    expect(container.querySelector('.modal-overlay')).toHaveClass('transcript-pager-overlay');
    expect(container.querySelector('.modal-content')).toHaveClass('transcript-pager-content');
    expect(container.querySelector('.modal-header')).toHaveClass('transcript-pager-header');
    expect(container.querySelector('.modal-title')).toHaveClass('transcript-pager-title');
    expect(container.querySelector('.modal-close')).toHaveClass('transcript-pager-close');
    expect(container.querySelector('.modal-body')).toHaveClass('transcript-pager-body');
  });
});
