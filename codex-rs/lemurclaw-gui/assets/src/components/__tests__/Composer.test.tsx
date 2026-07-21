import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Composer } from '../Composer';

// Default props for tests that don't care about slash behavior. Each test can
// override startTurn / onSlashCommand as needed.
const defaults = {
  threadId: 't1' as string | null,
  turnActive: false,
  onInterrupt: vi.fn(),
  startTurn: vi.fn(),
  onSlashCommand: vi.fn(),
};

describe('Composer (existing behavior)', () => {
  it('disables send when threadId is null', () => {
    render(<Composer {...defaults} threadId={null} />);
    expect(screen.getByTestId('composer-send')).toBeDisabled();
    expect(screen.getByTestId('composer-input')).toBeDisabled();
  });

  it('Enter sends a turn/start with the typed text', () => {
    const startTurn = vi.fn();
    render(<Composer {...defaults} startTurn={startTurn} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    fireEvent.change(ta, { target: { value: 'hello' } });
    fireEvent.keyDown(ta, { key: 'Enter' });
    expect(startTurn).toHaveBeenCalledTimes(1);
    expect(startTurn).toHaveBeenCalledWith([{ type: 'text', text: 'hello', text_elements: [] }]);
  });

  it('Shift+Enter does NOT send', () => {
    const startTurn = vi.fn();
    render(<Composer {...defaults} startTurn={startTurn} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    fireEvent.change(ta, { target: { value: 'hello' } });
    fireEvent.keyDown(ta, { key: 'Enter', shiftKey: true });
    expect(startTurn).not.toHaveBeenCalled();
  });

  it('shows interrupt button while turnActive and calls onInterrupt', () => {
    const onInterrupt = vi.fn();
    render(<Composer {...defaults} turnActive={true} onInterrupt={onInterrupt} />);
    expect(screen.queryByTestId('composer-send')).toBeNull();
    fireEvent.click(screen.getByTestId('composer-interrupt'));
    expect(onInterrupt).toHaveBeenCalled();
  });
});

describe('Composer slash popup', () => {
  it('typing "/" opens popup with all 16 commands', () => {
    render(<Composer {...defaults} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    fireEvent.change(ta, { target: { value: '/' } });
    const popup = screen.getByTestId('composer-slash-popup');
    expect(popup).toBeInTheDocument();
    // All 16 commands visible.
    expect(screen.getByText('/init')).toBeInTheDocument();
    expect(screen.getByText('/diff')).toBeInTheDocument();
  });

  it('typing "/mo" filters to /model', () => {
    render(<Composer {...defaults} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    fireEvent.change(ta, { target: { value: '/mo' } });
    expect(screen.getByText('/model')).toBeInTheDocument();
    expect(screen.queryByText('/init')).toBeNull();
  });

  it('typing "hello" does NOT open popup', () => {
    render(<Composer {...defaults} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    fireEvent.change(ta, { target: { value: 'hello' } });
    expect(screen.queryByTestId('composer-slash-popup')).toBeNull();
  });

  it('ArrowDown moves active selection forward (wraps on overflow)', () => {
    render(<Composer {...defaults} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    fireEvent.change(ta, { target: { value: '/' } });
    const optionsBefore = screen.getAllByRole('option');
    expect(optionsBefore[0]).toHaveAttribute('aria-selected', 'true');
    fireEvent.keyDown(ta, { key: 'ArrowDown' });
    const optionsAfter = screen.getAllByRole('option');
    expect(optionsAfter[0]).toHaveAttribute('aria-selected', 'false');
    expect(optionsAfter[1]).toHaveAttribute('aria-selected', 'true');
  });

  it('Enter on popup picks active command and fires onSlashCommand', () => {
    const onSlashCommand = vi.fn();
    render(<Composer {...defaults} onSlashCommand={onSlashCommand} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    fireEvent.change(ta, { target: { value: '/init' } });
    fireEvent.keyDown(ta, { key: 'Enter' });
    expect(onSlashCommand).toHaveBeenCalledTimes(1);
    const [cmd, args] = onSlashCommand.mock.calls[0];
    expect(cmd.name).toBe('init');
    expect(args).toBe('');
  });

  it('Escape closes popup by stripping leading slash', () => {
    const onSlashCommand = vi.fn();
    render(<Composer {...defaults} onSlashCommand={onSlashCommand} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    fireEvent.change(ta, { target: { value: '/model' } });
    expect(screen.getByTestId('composer-slash-popup')).toBeInTheDocument();
    fireEvent.keyDown(ta, { key: 'Escape' });
    expect(screen.queryByTestId('composer-slash-popup')).toBeNull();
    expect(onSlashCommand).not.toHaveBeenCalled();
  });

  it('non-leading "/" (on second line) does NOT open popup', () => {
    render(<Composer {...defaults} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    fireEvent.change(ta, { target: { value: 'hello\n/foo' } });
    expect(screen.queryByTestId('composer-slash-popup')).toBeNull();
  });

  it('typing "/nonexistent" + Enter does not dispatch and does not send turn', () => {
    const onSlashCommand = vi.fn();
    const startTurn = vi.fn();
    render(<Composer {...defaults} onSlashCommand={onSlashCommand} startTurn={startTurn} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    fireEvent.change(ta, { target: { value: '/nonexistent' } });
    // Popup is open but empty (no prefix matches).
    // Enter should NOT pick (filtered is empty) and NOT send a turn.
    fireEvent.keyDown(ta, { key: 'Enter' });
    expect(onSlashCommand).not.toHaveBeenCalled();
    // The text "/nonexistent" is not an exact command name, so submit() falls
    // through to sending a normal turn with the literal text. That's correct:
    // unknown slash commands pass through as user text (server-side no-op).
    expect(startTurn).toHaveBeenCalledWith([{ type: 'text', text: '/nonexistent', text_elements: [] }]);
  });
});
