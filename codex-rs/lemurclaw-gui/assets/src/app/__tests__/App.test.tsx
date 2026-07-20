import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import type { ConversationState } from '../../viewModel/types';
import { initialState } from '../../viewModel/types';
import { sendRequest } from '../../transport';

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
    // App.tsx passes these to Composer/ModelPicker/SessionPicker. Include
    // them in the mock so any future test that triggers those components
    // doesn't crash on `undefined is not a function`.
    startTurn: vi.fn().mockResolvedValue(undefined),
    resumeThread: vi.fn().mockResolvedValue(undefined),
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

  // ----- SettingsModal integration (subproject 5-A) -----
  // These exercise the full App → TopBar gear → SettingsModal → surface panel
  // → RPC chain in one mounted tree. The single-panel tests cover leaf
  // behavior; these guard against routing/wiring regressions across the
  // component boundary.

  it('gear button (⚙) opens SettingsModal and defaults to Permissions', () => {
    render(<App />);
    expect(screen.queryByTestId('settings-modal')).toBeNull();
    fireEvent.click(screen.getByLabelText('settings'));
    expect(screen.getByTestId('settings-modal')).toBeInTheDocument();
    // First surface is Permissions and its pane is rendered.
    expect(screen.getByText('Permissions').closest('.settings-nav-item')).toHaveClass('settings-nav-item-active');
    expect(screen.getByTestId('settings-pane-permissions')).toBeInTheDocument();
  });

  it('Esc closes the SettingsModal', () => {
    render(<App />);
    fireEvent.click(screen.getByLabelText('settings'));
    expect(screen.getByTestId('settings-modal')).toBeInTheDocument();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByTestId('settings-modal')).toBeNull();
  });

  it('switching to Plugins renders the plugin list with install actions', async () => {
    // Route responses by method name so the PermissionsPanel mount (default
    // surface) doesn't consume the plugin/list fixture via mockResolvedValueOnce.
    const pluginFixture = {
      marketplaces: [
        {
          name: 'local-market',
          path: { path: '/home/.codex/marketplaces/local.json' },
          interface: null,
          plugins: [
            {
              id: 'p1', remotePluginId: null, version: null, localVersion: null,
              name: 'Plugin One', shareContext: null, source: 'marketplace',
              installed: false, enabled: false, installPolicy: 'allow',
              installPolicySource: null, mustShowInstallationInterstitial: null,
              authPolicy: 'never', availability: 'available', interface: null, keywords: [],
            },
          ],
        },
      ],
      marketplaceLoadErrors: [],
      featuredPluginIds: [],
    };
    vi.mocked(sendRequest).mockImplementation(async (method: string) => {
      if (method === 'plugin/list') return pluginFixture as never;
      return { data: [], nextCursor: null } as never;
    });
    render(<App />);
    fireEvent.click(screen.getByLabelText('settings'));
    fireEvent.click(screen.getByText('Plugins'));
    // Plugins nav is active and the row rendered.
    expect(screen.getByText('Plugins').closest('.settings-nav-item')).toHaveClass('settings-nav-item-active');
    await waitFor(() => expect(screen.getByText('Plugin One')).toBeInTheDocument());
    // Uninstalled plugin offers install.
    expect(screen.getByRole('button', { name: (n) => n === 'install' })).toBeInTheDocument();
  });

  it('clicking install fires plugin/install with marketplace context', async () => {
    let installed = false;
    const buildFixture = () => ({
      marketplaces: [
        {
          name: 'local-market',
          path: { path: '/home/.codex/marketplaces/local.json' },
          interface: null,
          plugins: [
            {
              id: 'p1', remotePluginId: null, version: null, localVersion: null,
              name: 'Plugin One', shareContext: null, source: 'marketplace',
              installed, enabled: installed, installPolicy: 'allow',
              installPolicySource: null, mustShowInstallationInterstitial: null,
              authPolicy: 'never', availability: 'available', interface: null, keywords: [],
            },
          ],
        },
      ],
      marketplaceLoadErrors: [],
      featuredPluginIds: [],
    });
    vi.mocked(sendRequest).mockImplementation(async (method: string) => {
      if (method === 'plugin/list') return buildFixture() as never;
      if (method === 'plugin/install') { installed = true; return { authPolicy: 'never', appsNeedingAuth: [] } as never; }
      return { data: [], nextCursor: null } as never;
    });
    render(<App />);
    fireEvent.click(screen.getByLabelText('settings'));
    fireEvent.click(screen.getByText('Plugins'));
    await waitFor(() => expect(screen.getByText('Plugin One')).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: (n) => n === 'install' }));
    await waitFor(() => {
      expect(sendRequest).toHaveBeenCalledWith('plugin/install', {
        marketplacePath: { path: '/home/.codex/marketplaces/local.json' },
        pluginName: 'Plugin One',
      });
    });
  });
});
