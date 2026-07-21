import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// WebSocket-mode tests for transport.ts.
//
// The transport module picks its mode at module-import time by checking
// `window.ipc`. The sibling transport.test.ts installs `window.ipc` to
// exercise the wry path; here we deliberately do NOT install `window.ipc`
// so `hasBridge()` is false and the WebSocket branch runs.
//
// Strategy: stub `globalThis.WebSocket` with a fake that records sent frames
// and lets each test push inbound frames via `socket.dispatchEvent`. Because
// the transport module holds a module-level `ws` singleton, each test calls
// `vi.resetModules()` + dynamic `import()` to get a fresh transport that
// re-runs its lazy WS-open on first `send`.

interface FakeSocket {
  url: string;
  readyState: number;
  sent: string[];
  onopen: ((ev: Event) => void) | null;
  onmessage: ((ev: MessageEvent) => void) | null;
  onclose: ((ev: CloseEvent) => void) | null;
  onerror: ((ev: Event) => void) | null;
  send(data: string): void;
  close(): void;
  // Test helper: simulate an inbound WS frame.
  push(data: string): void;
  // Test helper: simulate the server completing the WS handshake.
  open(): void;
}

function installFakeWebSocket(): { instances: FakeSocket[]; ctor: typeof WebSocket } {
  const instances: FakeSocket[] = [];
  class FakeWebSocket {
    static CONNECTING = 0;
    static OPEN = 1;
    static CLOSING = 2;
    static CLOSED = 3;
    url: string;
    readyState = 0;
    sent: string[] = [];
    onopen: ((ev: Event) => void) | null = null;
    onmessage: ((ev: MessageEvent) => void) | null = null;
    onclose: ((ev: CloseEvent) => void) | null = null;
    onerror: ((ev: Event) => void) | null = null;
    constructor(url: string) {
      this.url = url;
      instances.push(this as unknown as FakeSocket);
    }
    send(data: string) {
      this.sent.push(data);
    }
    close() {
      this.readyState = 2;
    }
    open() {
      this.readyState = 1;
      this.onopen?.(new Event('open'));
    }
    push(data: string) {
      this.onmessage?.({ data } as MessageEvent);
    }
  }
  const ctor = FakeWebSocket as unknown as typeof WebSocket;
  vi.stubGlobal('WebSocket', ctor);
  return { instances, ctor };
}

describe('transport (WebSocket mode)', () => {
  let instances: FakeSocket[] = [];

  beforeEach(() => {
    // Browser/webui mode: NO window.ipc. Also clear any leftover bridge from
    // sibling tests.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    delete (window as any).ipc;
    window.__lemurclaw = undefined;
    // Reset the module registry so each test gets a fresh transport module
    // (with fresh module-level `ws` / `wsOutboundQueue` singletons). Without
    // this, vi would hand back the cached module from the previous test,
    // whose WS might already be open.
    vi.resetModules();
    const fake = installFakeWebSocket();
    instances = fake.instances;
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.resetModules();
    instances = [];
  });

  it('sendRequest opens a WebSocket and posts the envelope as a WS frame', async () => {
    const mod = await import('./transport');
    mod.registerResponseHandler();

    const p = mod.sendRequest('thread/list', { limit: 5 });
    // Allow the synchronous `send` -> initWebSocket -> new WebSocket() to run.
    await Promise.resolve();

    expect(instances.length).toBe(1);
    // jsdom's default location is http://localhost/ → ws://localhost/ws.
    // Just assert the scheme swap + path; the host comes from the test env.
    expect(instances[0].url).toMatch(/^ws:\/\/[^/]+\/ws$/);

    // Open the socket; queued frame should flush.
    instances[0].open();
    await Promise.resolve();

    expect(instances[0].sent.length).toBe(1);
    const posted = JSON.parse(instances[0].sent[0]);
    expect(posted.method).toBe('thread/list');
    expect(posted.params).toEqual({ limit: 5 });
    expect(typeof posted.id).toBe('number');

    // Backend replies with a JSON-RPC result envelope carrying the same id.
    instances[0].push(
      JSON.stringify({ jsonrpc: '2.0', id: posted.id, result: { data: [], nextCursor: null } }),
    );

    await expect(p).resolves.toEqual({ data: [], nextCursor: null });
  });

  it('sendRequest rejects when an error envelope arrives', async () => {
    const mod = await import('./transport');
    mod.registerResponseHandler();

    const p = mod.sendRequest('model/list', {});
    await Promise.resolve();
    instances[0].open();
    await Promise.resolve();

    const posted = JSON.parse(instances[0].sent[0]);
    instances[0].push(
      JSON.stringify({ jsonrpc: '2.0', id: posted.id, error: { code: -32601, message: 'nope' } }),
    );

    await expect(p).rejects.toThrow('nope');
  });

  it('non-JSON-RPC inbound frames route to onEvent, not pendingRequests', async () => {
    const mod = await import('./transport');
    mod.registerResponseHandler();

    const cb = vi.fn();
    mod.onEvent(cb);
    await Promise.resolve();
    instances[0].open();

    // A server-pushed notification: no `jsonrpc` field.
    instances[0].push(
      JSON.stringify({ method: 'thread/started', params: { threadId: 't-1' } }),
    );

    expect(cb).toHaveBeenCalledTimes(1);
    expect(cb).toHaveBeenCalledWith({ method: 'thread/started', params: { threadId: 't-1' } });
  });

  it('JSON-RPC envelopes do NOT invoke onEvent', async () => {
    const mod = await import('./transport');
    mod.registerResponseHandler();

    const cb = vi.fn();
    mod.onEvent(cb);
    await Promise.resolve();
    instances[0].open();

    // An unrelated JSON-RPC envelope (unknown id — should be silently dropped
    // by settlePending). onEvent must NOT see it.
    instances[0].push(JSON.stringify({ jsonrpc: '2.0', id: 999_999, result: {} }));

    expect(cb).not.toHaveBeenCalled();
  });

  it('outbound messages queue before OPEN and flush on open', async () => {
    const mod = await import('./transport');
    mod.registerResponseHandler();

    // Two sends before open — both should queue.
    mod.send({ method: 'a', id: 1, params: {} });
    mod.send({ method: 'b', id: 2, params: {} });
    await Promise.resolve();

    expect(instances[0].sent.length).toBe(0);
    expect(instances[0].readyState).toBe(0);

    instances[0].open();
    await Promise.resolve();

    expect(instances[0].sent.length).toBe(2);
    expect(JSON.parse(instances[0].sent[0]).method).toBe('a');
    expect(JSON.parse(instances[0].sent[1]).method).toBe('b');
  });

  it('resolveServerRequest / rejectServerRequest send the magic envelopes', async () => {
    const mod = await import('./transport');
    mod.registerResponseHandler();
    await Promise.resolve();
    instances[0].open();

    mod.resolveServerRequest('req-1', { decision: 'accept' });
    mod.rejectServerRequest('req-2', 'user declined');

    await Promise.resolve();
    expect(instances[0].sent.length).toBe(2);
    expect(JSON.parse(instances[0].sent[0])).toEqual({
      __resolve: 'req-1',
      result: { decision: 'accept' },
    });
    expect(JSON.parse(instances[0].sent[1])).toEqual({
      __reject: 'req-2',
      error: { code: -32000, message: 'user declined' },
    });
  });
});
