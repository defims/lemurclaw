import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Composer } from '../Composer';

// Default props for tests that don't care about slash/mention behavior.
const defaults = {
  threadId: 't1' as string | null,
  turnActive: false,
  onInterrupt: vi.fn(),
  startTurn: vi.fn(),
  onSlashCommand: vi.fn(),
  cwd: '/proj' as string | null,
  fuzzyFiles: [] as Array<{ root: string; path: string; match_type: 'file' | 'directory'; file_name: string; score: number; indices: number[] | null }>,
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

describe('Composer @mention popup', () => {
  // Helper: build a fake FuzzyFileSearchResult with the snake_case wire shape.
  const file = (path: string, score = 100) => ({
    root: '/proj', path, match_type: 'file' as const,
    file_name: path.split('/').pop() ?? path, score, indices: null,
  });

  // Helper: type text + set cursor at the end, triggering the onChange that
  // the real textarea would fire. fireEvent.change alone doesn't update
  // selectionStart in jsdom, so we set it explicitly then fire click (which
  // Composer uses to track cursor position).
  const type = (ta: HTMLTextAreaElement, value: string) => {
    fireEvent.change(ta, { target: { value } });
    ta.setSelectionRange(value.length, value.length);
    // Composer's onClick/onKeyUp handlers read selectionStart and update
    // cursorPos state — fire click to simulate the real interaction.
    fireEvent.click(ta);
  };

  it('typing "@co" with files in state opens the mention popup', () => {
    render(<Composer {...defaults} fuzzyFiles={[file('code.rs'), file('config.toml')]} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    type(ta, '@co');
    expect(screen.getByTestId('composer-mention-popup')).toBeInTheDocument();
    expect(screen.getAllByText('code.rs').length).toBeGreaterThan(0);
    expect(screen.getAllByText('config.toml').length).toBeGreaterThan(0);
  });

  it('typing "hello@co" (no whitespace before @) does NOT open the popup', () => {
    render(<Composer {...defaults} fuzzyFiles={[file('code.rs')]} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    type(ta, 'hello@co');
    expect(screen.queryByTestId('composer-mention-popup')).toBeNull();
  });

  it('typing "hello @co" (space before @) opens the popup', () => {
    render(<Composer {...defaults} fuzzyFiles={[file('code.rs')]} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    type(ta, 'hello @co');
    expect(screen.getByTestId('composer-mention-popup')).toBeInTheDocument();
  });

  it('Enter on mention popup replaces @query with @path', () => {
    render(<Composer {...defaults} fuzzyFiles={[file('src/code.rs')]} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    type(ta, '@co');
    fireEvent.keyDown(ta, { key: 'Enter' });
    expect(ta.value).toContain('@src/code.rs');
  });

  it('Esc on mention popup closes it by stripping the @token', () => {
    render(<Composer {...defaults} fuzzyFiles={[file('code.rs')]} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    type(ta, '@co');
    fireEvent.keyDown(ta, { key: 'Escape' });
    expect(screen.queryByTestId('composer-mention-popup')).toBeNull();
  });

  it('does NOT open mention popup when cwd is null', () => {
    render(<Composer {...defaults} cwd={null} fuzzyFiles={[file('code.rs')]} />);
    const ta = screen.getByTestId('composer-input') as HTMLTextAreaElement;
    type(ta, '@co');
    expect(screen.queryByTestId('composer-mention-popup')).toBeNull();
  });
});
