import { describe, it, expect, vi, beforeEach } from 'vitest';
import { sendRequest, registerResponseHandler, onEvent } from './transport';

// Transport tests for the typed request channel.
//
// Strategy: instead of mocking `./transport.send` (which would also replace
// `sendRequest`/`registerResponseHandler`), we inject a fake `window.ipc` so
// the real `send` runs end-to-end and posts to our spy. We then read the
// last-posted message to recover the id, and feed a synthetic JSON-RPC
// response to the installed `onResponse` handler to settle the promise.
// This exercises the same plumbing the real backend uses.

function lastPosted(): { method: string; id: number; params: unknown } {
  const ipc = window.ipc as unknown as { postMessage: (s: string) => void };
  const calls = (ipc.postMessage as unknown as ReturnType<typeof vi.fn>).mock.calls;
  const raw = calls.at(-1)?.[0] as string;
  return JSON.parse(raw);
}

function responseHandler(): (s: string) => void {
  // `registerResponseHandler` installs `window.__lemurclaw.onResponse`.
  return (window.__lemurclaw as { onResponse: (s: string) => void }).onResponse;
}

describe('sendRequest', () => {
  beforeEach(() => {
    window.ipc = { postMessage: vi.fn() };
    registerResponseHandler();
  });

  it('resolves with result when matching response arrives', async () => {
    const p = sendRequest('thread/list', { limit: 5 });

    // Assert the outbound shape the backend will see.
    const sent = lastPosted();
    expect(sent.method).toBe('thread/list');
    expect(sent.params).toEqual({ limit: 5 });
    expect(typeof sent.id).toBe('number');

    // Backend replies with a JSON-RPC result envelope carrying the same id.
    responseHandler()(
      JSON.stringify({
        jsonrpc: '2.0',
        id: sent.id,
        result: { data: [], nextCursor: null, backwardsCursor: null },
      }),
    );

    await expect(p).resolves.toEqual({
      data: [],
      nextCursor: null,
      backwardsCursor: null,
    });
  });

  it('rejects when error response arrives', async () => {
    const p = sendRequest('model/list', {});

    const sent = lastPosted();
    responseHandler()(
      JSON.stringify({
        jsonrpc: '2.0',
        id: sent.id,
        error: { code: -32601, message: 'method not found' },
      }),
    );

    await expect(p).rejects.toThrow('method not found');
  });

  it('ignores responses with unknown id (no crash)', () => {
    // No pending request has id 99999 — handler must simply return.
    expect(() =>
      responseHandler()(JSON.stringify({ jsonrpc: '2.0', id: 99999, result: {} })),
    ).not.toThrow();
  });

  it('onEvent preserves a previously installed onResponse (no clobber)', () => {
    // Regression guard: an earlier version of onEvent reassigned
    // window.__lemurclaw wholesale, dropping the onResponse handler that
    // registerResponseHandler had just installed. App.tsx wires both, so
    // they must coexist regardless of install order.
    registerResponseHandler();
    onEvent(() => {});
    expect(typeof window.__lemurclaw?.onResponse).toBe('function');
    expect(typeof window.__lemurclaw?.onEvent).toBe('function');
  });

  it('registerResponseHandler preserves a previously installed onEvent', () => {
    // Symmetric to the above — install order should not matter.
    onEvent(() => {});
    registerResponseHandler();
    expect(typeof window.__lemurclaw?.onEvent).toBe('function');
    expect(typeof window.__lemurclaw?.onResponse).toBe('function');
  });
});
