import { useEffect, useState } from 'react';
import { sendRequest } from '../transport';

interface Props {
  /** Children to render once authed (or once user dismisses the prompt). */
  children: React.ReactNode;
}

type Phase = 'checking' | 'authed' | 'unauthed' | 'dismissed';

/** Minimal onboarding gate. On mount, calls `getAuthStatus`. If the response
 *  indicates no auth method is configured AND OpenAI auth is required, shows a
 *  welcome screen directing the user to the CLI to run `codex login`. Otherwise
 *  renders children directly.
 *
 *  Subproject 4 deliberately does NOT implement in-GUI OAuth — that's codex-
 *  api's WebSocket auth flow and is far heavier. The CLI handles login; the
 *  GUI just detects the result. */
export function Onboarding({ children }: Props) {
  const [phase, setPhase] = useState<Phase>('checking');
  const [authMethod, setAuthMethod] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    // Shape verified against types/GetAuthStatusResponse.ts:
    //   { authMethod: AuthMode | null, authToken: string | null, requiresOpenaiAuth: boolean | null }
    sendRequest<{ authMethod: string | null; requiresOpenaiAuth: boolean | null }>('getAuthStatus', {})
      .then((resp) => {
        if (cancelled) return;
        setAuthMethod(resp.authMethod);
        // "authed" = some auth method is configured (apikey/chatgpt/...).
        // "unauthed" = no method AND the server says OpenAI auth is required.
        // Otherwise (no method, not required) pass through — local-only mode.
        const authed = resp.authMethod !== null || resp.requiresOpenaiAuth !== true;
        setPhase(authed ? 'authed' : 'unauthed');
      })
      .catch(() => {
        // If getAuthStatus fails entirely, assume authed — don't block the UI
        // on a protocol issue. The user can still use apikey-configured setups.
        if (!cancelled) setPhase('authed');
      });
    return () => { cancelled = true; };
  }, []);

  if (phase === 'checking') {
    return <div className="onboarding onboarding-checking" data-testid="onboarding-checking">checking auth…</div>;
  }
  if (phase === 'authed' || phase === 'dismissed') {
    return <>{children}</>;
  }
  // phase === 'unauthed'
  return (
    <div className="onboarding onboarding-unauthed" data-testid="onboarding-unauthed">
      <div className="onboarding-card">
        <h2>welcome to lemurclaw</h2>
        <p>
          you're not signed in. lemurclaw needs a configured model provider
          {authMethod ? ` (current mode: ${authMethod})` : ''}.
        </p>
        <p>
          open a terminal in this project and run:
        </p>
        <pre className="onboarding-cmd">codex login</pre>
        <p>
          or set <code>OPENAI_API_KEY</code> in your environment, then restart lemurclaw.
        </p>
        <p className="onboarding-dismiss-hint">
          already configured? <button onClick={() => setPhase('dismissed')} className="onboarding-dismiss">continue anyway</button>
        </p>
      </div>
    </div>
  );
}
