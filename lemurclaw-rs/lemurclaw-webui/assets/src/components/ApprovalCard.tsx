import type { PendingApproval } from '../viewModel/types';
import { resolveServerRequest, rejectServerRequest } from '../transport';

interface Props {
  approval: PendingApproval;
  /** For fileChange approvals: the diff text to show when the user clicks
   *  "view full diff". Null when no matching fileChange cell is in state yet
   *  (the patch hasn't arrived via item/fileChange/patchUpdated). Absent
   *  for non-fileChange approvals. */
  diffForApproval?: string | null;
  /** Fired when the user clicks "view full diff" on a fileChange approval.
   *  App opens <DiffViewerModal> with the provided diff. */
  onViewDiff?: (diff: string) => void;
}

/** ApprovalCard: renders a pending ServerRequest as a card with decision
 *  buttons. Dispatch shape depends on `approval.kind`:
 *  - commandExecution: [run once] [always this session] [decline]
 *  - fileChange:       [apply once] [always this session] [decline]
 *                      + optional "view full diff" button (when diffForApproval
 *                        is non-null)
 *  - mcpElicitation:   text input + [submit] [cancel]
 *  - permissions:      [allow] [deny]
 *  - toolUserInput:    text input + [submit] [cancel]
 *  - generic:          [resolve] [cancel]
 *
 *  Decision → transport.resolveServerRequest / rejectServerRequest. */
export function ApprovalCard({ approval, diffForApproval, onViewDiff }: Props) {
  switch (approval.kind) {
    case 'commandExecution':
      return <ExecApproval approval={approval} />;
    case 'fileChange':
      return <FileChangeApproval approval={approval} diffForApproval={diffForApproval ?? null} onViewDiff={onViewDiff} />;
    case 'mcpElicitation':
    case 'toolUserInput':
      return <ElicitationApproval approval={approval} />;
    case 'permissions':
      return <PermissionsApproval approval={approval} />;
    case 'generic':
    default:
      return <GenericApproval approval={approval} />;
  }
}

function ExecApproval({ approval }: { approval: PendingApproval }) {
  const params = approval.raw.params as {
    command?: string | null;
    cwd?: { path?: string } | string | null;
    commandActions?: Array<{ type: string; name?: string }> | null;
    reason?: string | null;
  };
  const cwdStr = typeof params.cwd === 'string' ? params.cwd : params.cwd?.path ?? '';
  const actions = (params.commandActions ?? []).map((a) => a.name ?? a.type).join(' | ');
  return (
    <div className="approval approval-exec" data-testid="approval-exec">
      <div className="approval-title">🛡 command approval</div>
      <div className="approval-detail">
        <code className="approval-command">$ {params.command ?? '(no command)'}</code>
        {cwdStr && <div className="approval-cwd">{cwdStr}</div>}
        {actions && <div className="approval-actions-summary">{actions}</div>}
        {params.reason && <div className="approval-reason">{params.reason}</div>}
      </div>
      <div className="approval-buttons">
        <button onClick={() => resolveServerRequest(approval.requestId, { decision: 'accept' })}>run once</button>
        <button onClick={() => resolveServerRequest(approval.requestId, { decision: 'acceptForSession' })}>always this session</button>
        <button onClick={() => rejectServerRequest(approval.requestId, 'user declined')}>decline</button>
      </div>
    </div>
  );
}

function FileChangeApproval({
  approval,
  diffForApproval,
  onViewDiff,
}: {
  approval: PendingApproval;
  diffForApproval: string | null;
  onViewDiff?: (diff: string) => void;
}) {
  // NOTE: FileChangeRequestApprovalParams only carries threadId/turnId/itemId/
  // startedAtMs/reason/grantRoot — it does NOT include the actual file change
  // list. Those arrive separately via `item/fileChange/patchUpdated`
  // notifications and land in the fileChange CellModel (reducer). App cross-
  // references the matching cell by itemId and hands the diff text in via
  // `diffForApproval` (null until the patch notification arrives).
  const params = approval.raw.params as {
    reason?: string | null;
    grantRoot?: string | null;
  };
  return (
    <div className="approval approval-patch" data-testid="approval-patch">
      <div className="approval-title">📝 file change approval</div>
      {params.reason && <div className="approval-reason">{params.reason}</div>}
      {params.grantRoot && (
        <div className="approval-grant-root">
          grants writes under: <code>{params.grantRoot}</code>
        </div>
      )}
      <div className="approval-buttons">
        <button onClick={() => resolveServerRequest(approval.requestId, { decision: 'accept' })}>apply once</button>
        <button onClick={() => resolveServerRequest(approval.requestId, { decision: 'acceptForSession' })}>always this session</button>
        <button onClick={() => rejectServerRequest(approval.requestId, 'user declined')}>decline</button>
        {diffForApproval && onViewDiff && (
          <button
            className="approval-view-diff"
            data-testid="approval-view-diff"
            onClick={() => onViewDiff(diffForApproval)}
          >
            view full diff
          </button>
        )}
      </div>
    </div>
  );
}

function ElicitationApproval({ approval }: { approval: PendingApproval }) {
  // MCP elicitation + tool/user-input both want a free-form value. Subproject
  // 3 keeps it as a plain text field; structured JSON-Schema elicitation UI
  // (multi-field forms) is deferred.
  const submit = (value: string) =>
    resolveServerRequest(approval.requestId, { value });
  const cancel = () =>
    rejectServerRequest(approval.requestId, 'user cancelled');
  return (
    <InlineInputApproval
      testId="approval-elicitation"
      title={approval.kind === 'mcpElicitation' ? '🔌 mcp elicitation' : '❓ tool input requested'}
      submitLabel="submit"
      onSubmit={submit}
      onCancel={cancel}
    />
  );
}

function PermissionsApproval({ approval }: { approval: PendingApproval }) {
  return (
    <div className="approval approval-permissions" data-testid="approval-permissions">
      <div className="approval-title">🔒 permission request</div>
      <pre className="approval-raw">{JSON.stringify(approval.raw.params, null, 2)}</pre>
      <div className="approval-buttons">
        <button onClick={() => resolveServerRequest(approval.requestId, { decision: 'accept' })}>allow</button>
        <button onClick={() => rejectServerRequest(approval.requestId, 'user denied')}>deny</button>
      </div>
    </div>
  );
}

function GenericApproval({ approval }: { approval: PendingApproval }) {
  return (
    <div className="approval approval-generic" data-testid="approval-generic">
      <div className="approval-title">server request: {approval.raw.method}</div>
      <pre className="approval-raw">{JSON.stringify(approval.raw.params, null, 2)}</pre>
      <div className="approval-buttons">
        <button onClick={() => resolveServerRequest(approval.requestId, {})}>resolve</button>
        <button onClick={() => rejectServerRequest(approval.requestId, 'cancelled')}>cancel</button>
      </div>
    </div>
  );
}

function InlineInputApproval(props: {
  testId: string;
  title: string;
  submitLabel: string;
  onSubmit: (value: string) => void;
  onCancel: () => void;
}) {
  const submit = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const data = new FormData(e.currentTarget);
    props.onSubmit(String(data.get('value') ?? ''));
  };
  return (
    <form className="approval approval-elicitation" data-testid={props.testId} onSubmit={submit}>
      <div className="approval-title">{props.title}</div>
      <input name="value" className="approval-input" />
      <div className="approval-buttons">
        <button type="submit">{props.submitLabel}</button>
        <button type="button" onClick={props.onCancel}>cancel</button>
      </div>
    </form>
  );
}
