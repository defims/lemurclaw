import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ApprovalCard } from '../ApprovalCard';

vi.mock('../../transport', () => ({
  resolveServerRequest: vi.fn(),
  rejectServerRequest: vi.fn(),
}));

import { resolveServerRequest, rejectServerRequest } from '../../transport';
import type { PendingApproval } from '../../viewModel/types';

describe('ApprovalCard', () => {
  beforeEach(() => {
    vi.mocked(resolveServerRequest).mockClear();
    vi.mocked(rejectServerRequest).mockClear();
  });

  it('exec approval sends accept decision on [run once]', () => {
    const approval: PendingApproval = {
      requestId: 42,
      kind: 'commandExecution',
      raw: {
        method: 'item/commandExecution/requestApproval', id: 42,
        params: { threadId: 't1', turnId: 'tu1', itemId: 'i1', startedAtMs: 1, environmentId: null, command: 'ls', cwd: { path: '/x' }, commandActions: null },
      } as never,
    };
    render(<ApprovalCard approval={approval} />);
    fireEvent.click(screen.getByText('run once'));
    expect(resolveServerRequest).toHaveBeenCalledWith(42, { decision: 'accept' });
  });

  it('exec approval decline sends reject', () => {
    const approval: PendingApproval = {
      requestId: 99,
      kind: 'commandExecution',
      raw: { method: 'item/commandExecution/requestApproval', id: 99, params: { threadId: 't', turnId: 'tu', itemId: 'i', startedAtMs: 1, environmentId: null, command: 'rm', cwd: { path: '/' }, commandActions: null } } as never,
    };
    render(<ApprovalCard approval={approval} />);
    fireEvent.click(screen.getByText('decline'));
    expect(rejectServerRequest).toHaveBeenCalledWith(99, 'user declined');
  });

  it('patch approval renders reason/grantRoot and sends acceptForSession', () => {
    // NOTE: FileChangeRequestApprovalParams carries only threadId/turnId/itemId/
    // startedAtMs/reason/grantRoot — NO changes list. Those arrive via
    // `item/fileChange/patchUpdated` (Task 3.4 reducer captures them in the
    // fileChange CellModel). This test verifies what the approval envelope
    // actually exposes today.
    const approval: PendingApproval = {
      requestId: 7,
      kind: 'fileChange',
      raw: {
        method: 'item/fileChange/requestApproval', id: 7,
        params: { threadId: 't', turnId: 'tu', itemId: 'i', startedAtMs: 1, reason: 'needs write access', grantRoot: '/proj' },
      } as never,
    };
    render(<ApprovalCard approval={approval} />);
    expect(screen.getByText('needs write access')).toBeInTheDocument();
    expect(screen.getByText('/proj')).toBeInTheDocument();
    fireEvent.click(screen.getByText('always this session'));
    expect(resolveServerRequest).toHaveBeenCalledWith(7, { decision: 'acceptForSession' });
  });

  it('elicitation submits the typed value', () => {
    const approval: PendingApproval = {
      requestId: 'abc',
      kind: 'mcpElicitation',
      raw: { method: 'mcpServer/elicitation/request', id: 'abc', params: { threadId: 't', turnId: 'tu', itemId: 'i', requestedSchema: {} } } as never,
    };
    render(<ApprovalCard approval={approval} />);
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'my response' } });
    fireEvent.click(screen.getByText('submit'));
    expect(resolveServerRequest).toHaveBeenCalledWith('abc', { value: 'my response' });
  });
});
