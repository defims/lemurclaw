import { useEffect, useState } from 'react';
import { sendRequest } from '../../transport';
import { SettingsListPicker, type LoadState } from '../SettingsListPicker';
import type { McpServerStatus } from '../../types/v2/McpServerStatus';

/** mcpServerStatus/list response shape. There is no generated
 *  McpServerStatusListResponse.ts in types/v2 (the app-server emits this
 *  inline), so we type it locally as `{ data: McpServerStatus[] }`. */
interface McpListResponse { data: McpServerStatus[] }

/** Read-only list of MCP servers from `mcpServerStatus/list`. Shows the server
 *  name + tool count. Read-only in this batch — server enable/disable is out
 *  of scope. */
export function McpPanel() {
  const [state, setState] = useState<LoadState<McpServerStatus>>({
    loading: true, error: null, items: [],
  });

  useEffect(() => {
    let cancelled = false;
    setState({ loading: true, error: null, items: [] });
    sendRequest<McpListResponse>('mcpServerStatus/list', {})
      .then((resp) => {
        if (!cancelled) setState({ loading: false, error: null, items: resp.data });
      })
      .catch((e) => {
        if (!cancelled) setState({ loading: false, error: e instanceof Error ? e.message : String(e), items: [] });
      });
    return () => { cancelled = true; };
  }, []);

  return (
    <SettingsListPicker
      state={state}
      getId={(s) => s.name}
      renderLabel={(s) => s.name}
      renderSub={(s) => `${Object.keys(s.tools).length} tools`}
      emptyText="(no MCP servers configured)"
    />
  );
}
