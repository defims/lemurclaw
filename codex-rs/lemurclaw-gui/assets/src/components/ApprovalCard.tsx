import type { PendingApproval } from '../viewModel/types';
import { resolveServerRequest, rejectServerRequest } from '../transport';

interface Props {
  approval: PendingApproval;
}

/** ApprovalCard: renders a pending ServerRequest as a card with decision
 *  buttons. Dispatch shape depends on `approval.kind`:
 *  - commandExecution: [run once] [always this session] [decline]
 *  - fileChange:       [apply once] [always this session] [decline]
 *  - mcpElicitation:   text input + [submit] [cancel]
 *  - permissions:      [allow] [deny]
 *  - toolUserInput:    text input + [submit] [cancel]
 *  - generic:          [resolve] [cancel]
 *
 *  Decision → transport.resolveServerRequest / rejectServerRequest. */
export function ApprovalCard({ approval }: Props) {
  switch (approval.kind) {
    case 'commandExecution':
      return <ExecApproval approval={approval} />;
    case 'fileChange':
      return <FileChangeApproval approval={approval} />;
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

function FileChangeApproval({ approval }: { approval: PendingApproval }) {
  const params = approval.raw.params as {
    changes?: Array<{ path: string; kind: { type: string }; diff?: string }> | null;
  };
  const changes = params.changes ?? [];
  return (
    <div className="approval approval-patch" data-testid="approval-patch">
      <div className="approval-title">📝 file change approval</div>
      <ul className="approval-files">
        {changes.map((c, i) => (
          <li key={i} className={`approval-file approval-file-${c.kind.type}`}>
            <code>{c.path}</code>
          </li>
        ))}
      </ul>
      <div className="approval-buttons">
        <button onClick={() => resolveServerRequest(approval.requestId, { decision: 'accept' })}>apply once</button>
        <button onClick={() => resolveServerRequest(approval.requestId, { decision: 'acceptForSession' })}>always this session</button>
        <button onClick={() => rejectServerRequest(approval.requestId, 'user declined')}>decline</button>
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
