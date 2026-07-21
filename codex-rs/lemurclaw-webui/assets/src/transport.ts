// Transport: JS ↔ Rust bridge.
//
// Two runtime modes, selected at module import time by detecting the host:
//   - wry (gui mode): Rust injects `window.ipc.postMessage`. JS calls it for
//     each outbound message; Rust calls back into `window.__lemurclaw.onEvent`
//     for server-pushed events and `window.__lemurclaw.onResponse` for the
//     JSON-RPC envelope of each ClientRequest.
//   - browser (webui mode): no `window.ipc`. We open a WebSocket to
//     `ws://<host>:<port>/ws` (served by lemurclaw-webui::server), send each
//     outbound message as a WS text frame, and route inbound frames through
//     the same onEvent + JSON-RPC-response plumbing.
//
// Both modes share `pendingRequests`, `sendRequest`, `registerResponseHandler`,
// and the response router — only the raw send/recv plumbing differs. This
// file owns all of it so the 26 callers across the React app keep importing
// from one place and never need to know which mode they're running in.

// Bridge surfaces injected by Rust (wry mode only). `ipc` comes from wry's
// `with_ipc_handler` plumbing; `__lemurclaw.onEvent` is installed by the
// initialization script in `run_gui` (and reassigned here via `onEvent`).
// `onResponse` is installed by `registerResponseHandler` to receive JSON-RPC
// response envelopes pushed back from the backend after each ClientRequest
// (matched by id in `pendingRequests`). In browser mode these stay undefined
// and a WebSocket is used instead.
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
 * True when Rust has injected the wry IPC bridge (gui mode). Used by the UI to
 * tell the difference between "running inside wry" and "opened as a plain web
 * page" (browser/webui mode, or `npm run dev`).
 */
export function hasBridge(): boolean {
  return typeof window.ipc !== 'undefined';
}

// ---------------------------------------------------------------------------
// Outbound: send a message to the Rust backend (wry) or the WS bridge (browser).
//
// In browser mode the WebSocket may still be connecting on the very first
// `send` (React mounts and fires a sendRequest before WS.OPEN). We queue
// outbound messages until OPEN, then flush.

let ws: WebSocket | null = null;
let wsReady: boolean = false;
const wsOutboundQueue: string[] = [];

function wsUrl(): string {
  // Derive `ws://host:port/ws` from the current page URL. `window.location`
  // is `http://127.0.0.1:PORT/` in webui mode (served by lemurclaw-webui).
  // Replace the scheme (http<->ws, https<->wss) and append `/ws`.
  const loc = window.location;
  const scheme = loc.protocol === 'https:' ? 'wss:' : 'ws:';
  // loc.pathname is `/` (or whatever the browser loaded); we want `/ws` as a
  // sibling route. Trim trailing slash and append.
  const path = loc.pathname.replace(/\/$/, '');
  return `${scheme}//${loc.host}${path}/ws${loc.search}`;
}

function initWebSocket(): void {
  if (ws !== null) return; // idempotent
  try {
    ws = new WebSocket(wsUrl());
  } catch (e) {
    console.error('transport: failed to open WebSocket', e);
    return;
  }
  ws.onopen = () => {
    wsReady = true;
    // Flush anything queued before OPEN.
    while (wsOutboundQueue.length > 0) {
      const msg = wsOutboundQueue.shift()!;
      try {
        ws?.send(msg);
      } catch (e) {
        console.error('transport: ws send (flush) failed', e);
      }
    }
  };
  ws.onclose = () => {
    wsReady = false;
  };
  ws.onerror = (e) => {
    console.error('transport: ws error', e);
  };
  ws.onmessage = (ev) => {
    // One WS text frame = one JSON message from the bridge. Route it through
    // the same single inbound path the wry onResponse/onEvent handlers use.
    handleInbound(typeof ev.data === 'string' ? ev.data : '');
  };
}

/**
 * Kick off the WebSocket eagerly when running in a browser (webui mode).
 * Called once at module-import time so the socket is connecting before any
 * React component mounts and fires a sendRequest. No-op in wry mode and in
 * non-browser test envs that lack `WebSocket`.
 */
function maybeInitWebSocketForBrowser(): void {
  if (typeof window === 'undefined') return; // SSR / node test without jsdom
  if (typeof window.WebSocket === 'undefined') return; // env without WS support
  if (hasBridge()) return; // wry mode — no socket needed
  initWebSocket();
}

/**
 * Send a ClientRequest envelope to the Rust backend. The caller is responsible
 * for the shape matching `codex_app_server_protocol::ClientRequest`; we keep
 * the parameter untyped here so the skeleton compiles before the typed helpers
 * arrive.
 *
 * In wry mode this calls `window.ipc.postMessage`. In browser mode it sends a
 * WS text frame (queuing if the socket is still connecting).
 */
export function send(msg: unknown): void {
  const json = JSON.stringify(msg);
  if (hasBridge()) {
    const ipc = window.ipc;
    if (!ipc) {
      console.error('transport.send: window.ipc disappeared mid-call');
      return;
    }
    ipc.postMessage(json);
    return;
  }
  // Browser mode: the WS was opened eagerly at module import time; just send
  // (or queue if still connecting).
  if (wsReady && ws && ws.readyState === WebSocket.OPEN) {
    try {
      ws.send(json);
    } catch (e) {
      console.error('transport.send: ws.send failed', e);
    }
  } else {
    wsOutboundQueue.push(json);
  }
}

// ---------------------------------------------------------------------------
// Inbound: route a JSON message from the backend to onEvent or pendingRequests.
//
// Both modes converge here. In wry mode Rust calls `window.__lemurclaw.onEvent`
// (for server-pushed events) and `.onResponse` (for JSON-RPC envelopes). In
// browser mode every WS frame comes through here directly. We distinguish by
// shape: a JSON-RPC envelope has `{jsonrpc, id, result|error}` and is routed
// to `pendingRequests`; everything else is a server-pushed event routed to
// the onEvent callback.

let onEventCb: ((ev: unknown) => void) | null = null;

/**
 * Register a callback for events pushed from the Rust backend.
 *
 * Rust calls `window.__lemurclaw.onEvent(json)` (wry) or the WS bridge sends
 * a non-JSON-RPC frame (browser); either way we parse here so app code always
 * receives a parsed object. Parse failures are logged and dropped rather than
 * thrown — a single malformed event must not break the loop.
 */
export function onEvent(cb: (ev: unknown) => void): void {
  onEventCb = cb;
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

function handleInbound(json: string): void {
  let obj: unknown;
  try {
    obj = JSON.parse(json);
  } catch (e) {
    console.error('transport: inbound parse failed', e, json);
    return;
  }
  // JSON-RPC response envelope? Match against pendingRequests by id. The
  // backend only emits `{jsonrpc:"2.0", id, result|error}` for the
  // ClientRequest responses it returns via handle_ipc — server-pushed events
  // never carry a top-level `jsonrpc` field.
  if (
    obj !== null &&
    typeof obj === 'object' &&
    (obj as { jsonrpc?: unknown }).jsonrpc === '2.0' &&
    'id' in (obj as object)
  ) {
    settlePending(obj as { id?: unknown; result?: unknown; error?: { code?: number; message?: string } });
    return;
  }
  // Server-pushed event. Route to onEvent if registered.
  if (onEventCb) {
    onEventCb(obj);
  }
}

// ---------------------------------------------------------------------------
// Typed request channel: send a ClientRequest and await its JSON-RPC response.
//
// In wry mode the Rust backend (backend.rs::handle_ipc) wraps every
// ClientRequest's RequestResult in a `{jsonrpc, id, result|error}` envelope
// and pushes it back via window.__lemurclaw.onResponse. In browser mode the
// WS bridge (lemurclaw-webui::server::handle_ipc_async) does the same and
// sends it as a WS text frame, which handleInbound routes to settlePending.

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
 * Send a ClientRequest and return a Promise that resolves with the JSON-RPC
 * `result`, or rejects with the error message.
 *
 * The id is assigned locally and the response is matched by id via
 * `onResponse` (wry) / WS frame (browser). Auto-rejects after 30s to avoid
 * leaked promises on dropped responses (e.g. the backend goes away mid-request).
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

function settlePending(envelope: {
  id?: unknown;
  result?: unknown;
  error?: { code?: number; message?: string };
}): void {
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
}

/**
 * Install the `onResponse` handler (wry mode). In browser mode every WS frame
 * is already routed through handleInbound, so this is a no-op for response
 * routing — but we still ensure `window.__lemurclaw` exists with an `onEvent`
 * stub so `onEvent` mutation above doesn't crash.
 *
 * Auto-installed at module import time (see `installResponseRouter()` call at
 * the bottom of this module) so the handler is live before any React component
 * mounts. React runs child effects before parent effects, so relying on a
 * parent's `useEffect` to call this before a child's `useEffect` fires
 * `sendRequest` would be fragile — the module-level install sidesteps that
 * entirely. The public function is retained for explicit re-install in tests
 * and any future caller that wants it.
 */
export function registerResponseHandler(): void {
  installResponseRouter();
}

function installResponseRouter(): void {
  if (!window.__lemurclaw) window.__lemurclaw = { onEvent: () => {} };
  // In wry mode the backend pushes JSON-RPC envelopes via this callback. We
  // re-parse + route through handleInbound so the wry path shares the exact
  // same settlement logic as the WS path.
  window.__lemurclaw.onResponse = (json: string) => handleInbound(json);
}

/**
 * Resolve a pending ServerRequest by id. Used by ApprovalCard's [accept] /
 * [approve] buttons. Sends the `__resolve` envelope consumed by
 * backend.rs::handle_ipc / server.rs::handle_ipc_async.
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

// Install the response router at module import time so it's live before any
// React component mounts and fires a sendRequest. Idempotent — re-assigns
// window.__lemurclaw.onResponse. Guarded for non-browser environments (tests
// without jsdom, SSR) where `window` may be undefined at import time.
if (typeof window !== 'undefined') {
  installResponseRouter();
  // Also eagerly open the WebSocket in browser/webui mode so the socket is
  // connecting by the time React mounts and fires a sendRequest. No-op in
  // wry mode (window.ipc present) and in non-browser test envs (no WS).
  maybeInitWebSocketForBrowser();
}
