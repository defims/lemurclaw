// Type guards for narrowing the `unknown` event payloads the transport layer
// hands us (see transport.ts:onEvent). The wire shape is the codex
// `ServerNotification` / `ServerRequest` discriminated union — each variant
// carries `method` (string tag) + `params`. ServerRequest additionally carries
// `id`.

import type { ServerNotification } from './ServerNotification';
import type { ServerRequest } from './ServerRequest';

// Re-export for caller convenience.
export type { ServerNotification, ServerRequest };

/** Common envelope shape shared by ServerNotification and ServerRequest. */
interface Envelope {
  method?: unknown;
  params?: unknown;
  id?: unknown;
}

/** True if x looks like a ServerNotification envelope (method + params, no id). */
export function isServerNotification(x: unknown): x is ServerNotification {
  if (typeof x !== 'object' || x === null) return false;
  const env = x as Envelope;
  return typeof env.method === 'string' && 'params' in env && !('id' in env);
}

/** True if x looks like a ServerRequest envelope (method + params + id). */
export function isServerRequest(x: unknown): x is ServerRequest {
  if (typeof x !== 'object' || x === null) return false;
  const env = x as Envelope;
  return (
    typeof env.method === 'string' &&
    'params' in env &&
    'id' in env &&
    (typeof env.id === 'string' || typeof env.id === 'number')
  );
}

/** Narrow a ServerNotification to a specific `method`. */
export function hasMethod<T extends ServerNotification['method']>(
  x: ServerNotification,
  method: T,
): x is Extract<ServerNotification, { method: T }> {
  return x.method === method;
}

/** Narrow a ServerRequest to a specific `method`. */
export function hasServerRequestMethod<T extends ServerRequest['method']>(
  x: ServerRequest,
  method: T,
): x is Extract<ServerRequest, { method: T }> {
  return x.method === method;
}
