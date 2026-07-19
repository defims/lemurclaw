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
// `run_gui` (and reassigned here via `onEvent`).
declare global {
  interface Window {
    ipc?: { postMessage: (s: string) => void };
    __lemurclaw?: { onEvent: (json: string) => void };
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
  window.__lemurclaw = {
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
