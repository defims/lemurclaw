import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { HookCell } from '../HookCell';

// HookRunSummary uses bigint in ts-rs types, but the wire shape is JSON
// number (codex serializes i64 → number). Test with a plain-number fixture
// cast through `never` to satisfy TS.
const run = {
  id: 'h1', eventName: 'PreToolUse', handlerType: 'command', executionMode: 'blocking',
  scope: 'session', sourcePath: { path: '/h/.codex/hook.sh' }, source: 'project',
  displayOrder: 0, status: 'completed', statusMessage: null,
  startedAt: 1, completedAt: 2, durationMs: 1, entries: [{ stream: 'stdout', line: 'ok' }],
} as never;

describe('HookCell', () => {
  it('shows event + status, entry count button', () => {
    render(<HookCell model={{ kind: 'hook', run }} />);
    expect(screen.getByText(/PreToolUse/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /entries/ })).toBeInTheDocument();
  });
});
