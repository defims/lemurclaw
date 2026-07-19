import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import type { ConversationState } from '../../viewModel/types';
import { initialState } from '../../viewModel/types';

// Mock useConversation so App renders without a backend. We expose a setter
// so individual tests can drive the ConversationState (e.g. give it a
// threadId so TranscriptPager can open).
let conversationState: ConversationState = initialState;
const setConversationState = (s: ConversationState): void => {
  conversationState = s;
};
vi.mock('../useConversation', () => ({
  useConversation: () => ({
    state: conversationState,
    threadId: conversationState.status === null ? null : 't1',
    interrupt: vi.fn(),
  }),
}));

// Mock Onboarding to render children directly (auth flow is tested in its
// own suite; here we want to exercise the App layout underneath the gate).
vi.mock('../../components/Onboarding', () => ({
  Onboarding: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

// Mock transport: sendRequest resolves so ModelPicker/ThemePicker mounts
// don't hang; send is a spy so we can assert on outbound dispatches.
vi.mock('../../transport', () => ({
  sendRequest: vi.fn().mockResolvedValue({ data: [], nextCursor: null }),
  send: vi.fn(),
  registerResponseHandler: vi.fn(),
}));

import { App } from '../App';

describe('App (integration)', () => {
  afterEach(() => {
    vi.clearAllMocks();
    setConversationState(initialState);
  });

  beforeEach(() => {
    // Give App a threadId + idle status so TranscriptPager (which needs a
    // threadId) can open, and SessionPicker's useThreadList is mocked away.
    setConversationState({ ...initialState, status: { type: 'idle' } });
  });

  it('renders the TopBar + main + sidebar layout under the Onboarding gate', () => {
    render(<App />);
    expect(screen.getByTestId('topbar')).toBeInTheDocument();
    expect(screen.getByTestId('sidebar')).toBeInTheDocument();
    expect(screen.getByTestId('composer')).toBeInTheDocument();
    // Scrollback has no data-testid of its own; assert via its placeholder
    // text (shown when the conversation is empty).
    expect(screen.getByText('send a message to start')).toBeInTheDocument();
  });

  it('Ctrl+T opens the TranscriptPager', () => {
    render(<App />);
    expect(screen.queryByTestId('transcript-pager')).toBeNull();
    fireEvent.keyDown(window, { ctrlKey: true, key: 't' });
    expect(screen.getByTestId('transcript-pager')).toBeInTheDocument();
  });

  it('Cmd+T also opens the TranscriptPager (mac)', () => {
    render(<App />);
    fireEvent.keyDown(window, { metaKey: true, key: 't' });
    expect(screen.getByTestId('transcript-pager')).toBeInTheDocument();
  });

  it('plain T (no ctrl/meta) does NOT open the TranscriptPager', () => {
    render(<App />);
    fireEvent.keyDown(window, { key: 't' });
    expect(screen.queryByTestId('transcript-pager')).toBeNull();
  });

  it('model button opens ModelPicker; Esc closes it', async () => {
    render(<App />);
    expect(screen.queryByTestId('model-picker')).toBeNull();
    fireEvent.click(screen.getByText(/no model/));
    await waitFor(() => expect(screen.getByTestId('model-picker')).toBeInTheDocument());
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByTestId('model-picker')).toBeNull();
  });

  it('theme button (🎨) opens ThemePicker', () => {
    render(<App />);
    expect(screen.queryByTestId('theme-picker')).toBeNull();
    fireEvent.click(screen.getByLabelText('theme'));
    expect(screen.getByTestId('theme-picker')).toBeInTheDocument();
  });

  it('transcript button (📜) opens TranscriptPager', () => {
    render(<App />);
    fireEvent.click(screen.getByLabelText('transcript'));
    expect(screen.getByTestId('transcript-pager')).toBeInTheDocument();
  });

  it('only one modal is open at a time (opening theme closes model)', async () => {
    render(<App />);
    fireEvent.click(screen.getByText(/no model/));
    await waitFor(() => expect(screen.getByTestId('model-picker')).toBeInTheDocument());
    fireEvent.click(screen.getByLabelText('theme'));
    expect(screen.queryByTestId('model-picker')).toBeNull();
    expect(screen.getByTestId('theme-picker')).toBeInTheDocument();
  });
});
