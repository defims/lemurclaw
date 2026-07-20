import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Composer } from '../Composer';

describe('Composer', () => {
  it('disables send when threadId is null', () => {
    render(<Composer threadId={null} turnActive={false} onInterrupt={() => {}} startTurn={vi.fn()} />);
    expect(screen.getByTestId('composer-send')).toBeDisabled();
    expect(screen.getByTestId('composer-input')).toBeDisabled();
  });

  it('Enter sends a turn/start with the typed text', () => {
    const startTurn = vi.fn();
    render(<Composer threadId="t1" turnActive={false} onInterrupt={() => {}} startTurn={startTurn} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    fireEvent.change(ta, { target: { value: 'hello' } });
    fireEvent.keyDown(ta, { key: 'Enter' });
    expect(startTurn).toHaveBeenCalledTimes(1);
    // Full UserInput.text variant contract (catches future drift):
    expect(startTurn).toHaveBeenCalledWith([{ type: 'text', text: 'hello', text_elements: [] }]);
  });

  it('Shift+Enter does NOT send', () => {
    const startTurn = vi.fn();
    render(<Composer threadId="t1" turnActive={false} onInterrupt={() => {}} startTurn={startTurn} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    fireEvent.change(ta, { target: { value: 'hello' } });
    fireEvent.keyDown(ta, { key: 'Enter', shiftKey: true });
    expect(startTurn).not.toHaveBeenCalled();
  });

  it('shows interrupt button while turnActive and calls onInterrupt', () => {
    const onInterrupt = vi.fn();
    render(<Composer threadId="t1" turnActive={true} onInterrupt={onInterrupt} startTurn={vi.fn()} />);
    expect(screen.queryByTestId('composer-send')).toBeNull();
    fireEvent.click(screen.getByTestId('composer-interrupt'));
    expect(onInterrupt).toHaveBeenCalled();
  });
});
