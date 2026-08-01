import { useState, useCallback } from 'react';
import { sendRequest } from '../transport';

interface RequestState<T> {
  data: T | null;
  loading: boolean;
  error: string | null;
}

/**
 * Lazy typed-request hook: returns a `run` function the caller invokes
 * explicitly (no auto-fire on mount). Most callers (SessionPicker refresh,
 * ModelPicker open) want to control when the request fires.
 */
export function useRequestLazy<T>() {
  const [state, setState] = useState<RequestState<T>>({ data: null, loading: false, error: null });

  const run = useCallback(async (method: string, params: unknown): Promise<T | null> => {
    setState((s) => ({ ...s, loading: true, error: null }));
    try {
      const data = await sendRequest<T>(method, params);
      setState({ data, loading: false, error: null });
      return data;
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setState({ data: null, loading: false, error: msg });
      return null;
    }
  }, []);

  return { ...state, run };
}
