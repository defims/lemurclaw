// Transport: JS ↔ Rust over the wry IPC bridge.
//
// The Rust side (`lemurclaw-gui/src/lib.rs`) wires two halves of the bridge:
//   - JS → Rust: wry's `ipc_handler` receives whatever we postMessage here,
//     deserializes it into a `codex_app_server_protocol::ClientRequest`, and
//     forwards it to the in-process AppServerClient.
//   - Rust → JS: the backend's next_event loop serializes each
//     `ServerNotification` / `ServerRequest` to JSON and calls
//     `window.__lemurclaw.onEvent(json)` via `evaluate_script`.
//
// This file owns the JS half of both directions plus the global typing for the
// injected bridges. Callers stay untyped (`unknown`) at the boundary so this
// skeleton never lies about event shapes — later subprojects will tighten the
// onEvent callback to the real ServerNotification union once the React side
// starts rendering specific cells.

// Bridge surfaces injected by Rust. `ipc` comes from wry's `with_ipc_handler`
// plumbing; `__lemurclaw.onEvent` is installed by the initialization script in
// `run_gui` (and reassigned here via `onEvent`). `onResponse` is installed by
// `registerResponseHandler` to receive JSON-RPC response envelopes pushed back
// from the backend after each ClientRequest (matched by id in
// `pendingRequests`).
declare global {
  interface Window {
    ipc?: { postMessage: (s: string) => void };
    __lemurclaw?: {
      onEvent: (json: string) => void;
      onResponse?: (json: string) => void;
    };
  }
}

/**
 * Send a ClientRequest to the Rust backend.
 *
 * The caller is responsible for the shape matching
 * `codex_app_server_protocol::ClientRequest`; we keep the parameter untyped
 * here so the skeleton compiles before the typed helpers arrive.
 */
export function send(msg: unknown): void {
  const ipc = window.ipc;
  if (!ipc) {
    console.error('transport.send: window.ipc not injected by Rust');
    return;
  }
  ipc.postMessage(JSON.stringify(msg));
}

/**
 * Register a callback for events pushed from the Rust backend.
 *
 * Rust calls `window.__lemurclaw.onEvent(json)`; we parse here so app code
 * always receives a parsed object. Parse failures are logged and dropped
 * rather than thrown — a single malformed event must not break the loop.
 */
export function onEvent(cb: (ev: unknown) => void): void {
  // Mutate, don't replace: a sibling `onResponse` may already be installed by
  // `registerResponseHandler`. Wholesale `window.__lemurclaw = {...}` would
  // silently drop it (see transport.test.ts "preserves onResponse when ...").
  const existing = window.__lemurclaw ?? { onEvent: () => {} };
  window.__lemurclaw = {
    ...existing,
    onEvent: (json: string) => {
      try {
        cb(JSON.parse(json));
      } catch (e) {
        console.error('transport.onEvent: parse failed', e, json);
      }
    },
  };
}

/**
 * True when Rust has injected the IPC bridge. Used by the UI to tell the
 * difference between "running inside wry" and "opened as a plain web page"
 * (e.g. during `npm run dev`).
 */
export function hasBridge(): boolean {
  return typeof window.ipc !== 'undefined';
}

/**
 * Resolve a pending ServerRequest by id. Used by ApprovalCard's [accept] /
 * [approve] buttons. Sends the `__resolve` envelope consumed by
 * `backend.rs::handle_ipc`.
 *
 * `result` is the JSON-RPC result payload — its shape depends on the
 * ServerRequest method (see the `*ApprovalResponse` types in
 * `assets/src/types/v2/`). For exec/file approvals, a `{ decision: "accept" }`
 * object; for mcp elicitation, a `{ value: ... }` object.
 */
export function resolveServerRequest(requestId: string | number, result: unknown): void {
  send({ __resolve: requestId, result });
}

/**
 * Reject a pending ServerRequest by id. Used by ApprovalCard's [decline] /
 * [cancel] buttons. `code` defaults to -32000 (JSON-RPC server error);
 * `message` is required.
 */
export function rejectServerRequest(
  requestId: string | number,
  message: string,
  code: number = -32000,
): void {
  send({ __reject: requestId, error: { code, message } });
}

// ---------------------------------------------------------------------------
// Typed request channel: send a ClientRequest and await its JSON-RPC response.
//
// The Rust backend (backend.rs::handle_ipc) wraps every ClientRequest's
// RequestResult in a `{jsonrpc:"2.0", id, result|error}` envelope and pushes
// it back via window.__lemurclaw.onResponse. We match by id to settle the
// pending promise.

const pendingRequests = new Map<
  string | number,
  {
    resolve: (v: unknown) => void;
    reject: (e: Error) => void;
    timer: ReturnType<typeof setTimeout>;
  }
>();
// Locally-issued request ids start here so they never collide with any
// backend/server-assigned ids in flight at the same time. The number itself
// is arbitrary; it just needs to be high enough to stay clear.
const MIN_CLIENT_REQUEST_ID = 1000;
let nextRequestId = MIN_CLIENT_REQUEST_ID;

/**
 * Send a ClientRequest and return a Promise that resolves with the
 * JSON-RPC `result`, or rejects with the error message.
 *
 * The id is assigned locally and the response is matched by id via
 * `onResponse`. Auto-rejects after 30s to avoid leaked promises on dropped
 * responses (e.g. the backend goes away mid-request).
 */
export function sendRequest<T = unknown>(method: string, params: unknown): Promise<T> {
  const id = nextRequestId++;
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => {
      if (pendingRequests.has(id)) {
        pendingRequests.delete(id);
        reject(new Error(`request ${method} (id=${id}) timed out after 30s`));
      }
    }, 30_000);
    pendingRequests.set(id, { resolve: (v) => resolve(v as T), reject, timer });
    send({ method, id, params });
  });
}

/**
 * Install the `onResponse` handler.
 *
 * Each JSON-RPC response envelope from the backend is parsed, matched against
 * `pendingRequests` by id, and the pending promise is settled (resolved with
 * `result` or rejected with `error.message`). Envelopes with an unknown id,
 * non-string/non-number id, or unparseable JSON are silently dropped — they
 * are usually late responses to already-timed-out requests.
 *
 * This is **also installed automatically at module import time** (see the
 * `installResponseRouter()` call at the bottom of this module) so the handler
 * is live before any React component mounts. React runs child effects before
 * parent effects, so relying on a parent's `useEffect` to call this before a
 * child's `useEffect` fires `sendRequest` would be fragile — the module-level
 * install sidesteps that entirely. The public function is retained for
 * explicit re-install in tests and any future caller that wants it.
 */
export function registerResponseHandler(): void {
  installResponseRouter();
}

function installResponseRouter(): void {
  if (!window.__lemurclaw) window.__lemurclaw = { onEvent: () => {} };
  window.__lemurclaw.onResponse = (json: string) => {
    let envelope: {
      id?: unknown;
      result?: unknown;
      error?: { code?: number; message?: string };
    };
    try {
      envelope = JSON.parse(json);
    } catch (e) {
      console.error('transport.onResponse: parse failed', e);
      return;
    }
    const id = envelope.id;
    if (id === undefined || (typeof id !== 'string' && typeof id !== 'number')) return;
    const pending = pendingRequests.get(id);
    if (!pending) return;
    clearTimeout(pending.timer);
    pendingRequests.delete(id);
    if (envelope.error) {
      pending.reject(
        new Error(envelope.error.message ?? `request failed (code ${envelope.error.code})`),
      );
    } else {
      pending.resolve(envelope.result);
    }
  };
}

// Install the response router at module import time so it's live before any
// React component mounts and fires a sendRequest. Idempotent — re-assigns
// window.__lemurclaw.onResponse. Guarded for non-browser environments (tests
// without jsdom, SSR) where `window` may be undefined at import time.
if (typeof window !== 'undefined') {
  installResponseRouter();
}
