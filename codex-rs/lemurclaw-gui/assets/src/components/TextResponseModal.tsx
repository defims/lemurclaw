import { Modal } from './Modal';

interface Props {
  /** Modal title (shown in header). */
  title: string;
  /** The response payload to display. JSON-stringified with 2-space indent. */
  response: unknown;
  /** Loading flag — when true, shows a spinner instead of the body. */
  loading?: boolean;
  /** Error message — when set, shows it instead of the response. */
  error?: string | null;
  onClose: () => void;
}

/** Generic modal that displays a server response as pretty-printed JSON.
 *  Used by slash commands like /status /usage /debug-config whose responses
 *  don't warrant bespoke UIs — the raw JSON is the most honest rendering.
 *
 *  For richer per-command UIs (e.g. /usage with a chart), a future stage can
 *  swap in a specialized component keyed off `title`. */
export function TextResponseModal({ title, response, loading, error, onClose }: Props) {
  return (
    <Modal title={title} onClose={onClose} testId="text-response-modal" wide>
      {loading && <div className="modal-loading">loading…</div>}
      {!loading && error && <div className="modal-error">failed: {error}</div>}
      {!loading && !error && (
        <pre className="text-response-body" data-testid="text-response-body">
          {JSON.stringify(response, null, 2)}
        </pre>
      )}
    </Modal>
  );
}
