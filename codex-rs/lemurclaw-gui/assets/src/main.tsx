import { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { hasBridge, onEvent, send } from './transport';

// Minimal valid `initialize` ClientRequest (see
// app-server-protocol schema: ClientRequest.ts → InitializeParams).
// The id is a per-tab counter; any string/number is valid per RequestId.
let nextRequestId = 1;

function makeInitializeRequest() {
  return {
    method: 'initialize',
    id: nextRequestId++,
    params: {
      clientInfo: { name: 'lemurclaw-gui', title: 'lemurclaw', version: '0.0.0' },
      capabilities: {
        experimentalApi: true,
        requestAttestation: false,
      },
    },
  };
}

interface EventRow {
  // monotonic counter so React keys stay stable across re-renders
  seq: number;
  raw: string;
}

function App() {
  const [events, setEvents] = useState<EventRow[]>([]);
  // Stable per-tab counter for stable React keys + display numbering.
  const seqRef = useRef(0);

  useEffect(() => {
    onEvent((ev) => {
      seqRef.current += 1;
      const seq = seqRef.current;
      setEvents((prev) => {
        const row = { seq, raw: JSON.stringify(ev) };
        const trimmed = prev.length >= 100 ? prev.slice(prev.length - 99) : prev;
        return [...trimmed, row];
      });
    });
  }, []);

  return (
    <div style={{ fontFamily: 'monospace', padding: 12, color: '#111' }}>
      <h2 style={{ marginTop: 0 }}>lemurclaw GUI (skeleton)</h2>
      <p style={{ fontSize: 12, color: '#555' }}>
        bridge: {hasBridge() ? 'injected (wry)' : 'missing (plain web — npm run dev)'}
      </p>
      <button onClick={() => send(makeInitializeRequest())}>send initialize</button>
      <button onClick={() => setEvents([])} style={{ marginLeft: 8 }}>
        clear
      </button>
      <pre
        style={{
          maxHeight: 480,
          overflow: 'auto',
          background: '#f4f4f4',
          padding: 8,
          marginTop: 12,
          fontSize: 12,
        }}
      >
        {events.length === 0
          ? '(no events yet)'
          : events.map((e) => (
              <div key={e.seq}>
                <span style={{ color: '#999' }}>{String(e.seq).padStart(4, '0')}</span>{' '}
                {e.raw}
              </div>
            ))}
      </pre>
    </div>
  );
}

const rootEl = document.getElementById('root');
if (!rootEl) {
  throw new Error('lemurclaw-gui: #root element not found in index.html');
}
createRoot(rootEl).render(<App />);
