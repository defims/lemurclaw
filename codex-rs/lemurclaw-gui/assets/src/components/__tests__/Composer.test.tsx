import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Composer } from '../Composer';

// Mock transport.send so tests can assert outbound ClientRequest shape.
vi.mock('../../transport', () => ({
  send: vi.fn(),
}));

import { send } from '../../transport';

describe('Composer', () => {
  beforeEach(() => vi.mocked(send).mockClear());

  it('disables send when threadId is null', () => {
    render(<Composer threadId={null} turnActive={false} onInterrupt={() => {}} />);
    expect(screen.getByTestId('composer-send')).toBeDisabled();
    expect(screen.getByTestId('composer-input')).toBeDisabled();
  });

  it('Enter sends a turn/start with the typed text', () => {
    render(<Composer threadId="t1" turnActive={false} onInterrupt={() => {}} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    fireEvent.change(ta, { target: { value: 'hello' } });
    fireEvent.keyDown(ta, { key: 'Enter' });
    expect(send).toHaveBeenCalledTimes(1);
    const req = vi.mocked(send).mock.calls[0][0] as {
      method: string;
      id: unknown;
      params: {
        threadId: string;
        clientUserMessageId: string;
        input: Array<{ type: string; text: string; text_elements: unknown[] }>;
      };
    };
    expect(req.method).toBe('turn/start');
    expect(req.id).toEqual(expect.any(Number)); // RequestId = string | number
    expect(req.params.threadId).toBe('t1');
    expect(req.params.clientUserMessageId).toEqual(expect.any(String));
    // Full UserInput.text variant contract (catches future drift):
    expect(req.params.input).toHaveLength(1);
    expect(req.params.input[0].type).toBe('text');
    expect(req.params.input[0].text).toBe('hello');
    expect(req.params.input[0].text_elements).toEqual([]);
  });

  it('Shift+Enter does NOT send', () => {
    render(<Composer threadId="t1" turnActive={false} onInterrupt={() => {}} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    fireEvent.change(ta, { target: { value: 'hello' } });
    fireEvent.keyDown(ta, { key: 'Enter', shiftKey: true });
    expect(send).not.toHaveBeenCalled();
  });

  it('shows interrupt button while turnActive and calls onInterrupt', () => {
    const onInterrupt = vi.fn();
    render(<Composer threadId="t1" turnActive={true} onInterrupt={onInterrupt} />);
    expect(screen.queryByTestId('composer-send')).toBeNull();
    fireEvent.click(screen.getByTestId('composer-interrupt'));
    expect(onInterrupt).toHaveBeenCalled();
  });
});
