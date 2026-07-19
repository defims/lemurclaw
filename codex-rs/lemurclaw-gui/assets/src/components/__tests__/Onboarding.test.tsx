import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { Onboarding } from '../Onboarding';

vi.mock('../../transport', () => ({ sendRequest: vi.fn() }));
import { sendRequest } from '../../transport';

describe('Onboarding', () => {
  // See useRequest.test.ts (Task 4.2) for why this is `afterEach` not `beforeEach`.
  // CRITICAL: do NOT switch to beforeEach — Vitest 2.1.9 false-positives.
  afterEach(() => vi.mocked(sendRequest).mockReset());

  it('renders children when authed (authMethod set)', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ authMethod: 'chatgpt', authToken: null, requiresOpenaiAuth: false } as never);
    render(<Onboarding><div data-testid="app-content">app</div></Onboarding>);
    await waitFor(() => expect(screen.getByTestId('app-content')).toBeInTheDocument());
    expect(screen.queryByTestId('onboarding-unauthed')).toBeNull();
  });

  it('renders children when no authMethod but OpenAI auth not required (local mode)', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ authMethod: null, authToken: null, requiresOpenaiAuth: false } as never);
    render(<Onboarding><div data-testid="app-content">app</div></Onboarding>);
    await waitFor(() => expect(screen.getByTestId('app-content')).toBeInTheDocument());
  });

  it('shows welcome screen when unauthed (no method + requiresOpenaiAuth)', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ authMethod: null, authToken: null, requiresOpenaiAuth: true } as never);
    render(<Onboarding><div data-testid="app-content">app</div></Onboarding>);
    await waitFor(() => expect(screen.getByTestId('onboarding-unauthed')).toBeInTheDocument());
    expect(screen.queryByTestId('app-content')).toBeNull();
    expect(screen.getByText(/codex login/)).toBeInTheDocument();
  });

  it('"continue anyway" dismisses and shows children', async () => {
    vi.mocked(sendRequest).mockResolvedValue({ authMethod: null, authToken: null, requiresOpenaiAuth: true } as never);
    render(<Onboarding><div data-testid="app-content">app</div></Onboarding>);
    await waitFor(() => expect(screen.getByTestId('onboarding-unauthed')).toBeInTheDocument());
    fireEvent.click(screen.getByText('continue anyway'));
    expect(screen.getByTestId('app-content')).toBeInTheDocument();
  });

  it('on getAuthStatus error, renders children (fail open)', async () => {
    vi.mocked(sendRequest).mockRejectedValue(new Error('boom'));
    render(<Onboarding><div data-testid="app-content">app</div></Onboarding>);
    await waitFor(() => expect(screen.getByTestId('app-content')).toBeInTheDocument());
  });
});
