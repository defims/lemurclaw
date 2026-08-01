import { useState, useCallback, useEffect } from 'react';
import { DEFAULT_THEME, type ThemeName } from '../themes';

const STORAGE_KEY = 'lemurclaw.theme';

function readStored(): ThemeName {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    if (v === 'light' || v === 'dark' || v === 'high-contrast') return v;
  } catch {
    /* localStorage unavailable (e.g. privacy mode) — fall back to default. */
  }
  return DEFAULT_THEME;
}

/** Theme hook: reads/writes localStorage + sets `data-theme` on the document
 *  root. The CSS uses `[data-theme="..."]` blocks (in styles.css) to swap CSS
 *  variables. Call this once at the App root; descendants pick up the
 *  variables automatically. */
export function useTheme() {
  const [theme, setThemeState] = useState<ThemeName>(() => readStored());

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    try {
      localStorage.setItem(STORAGE_KEY, theme);
    } catch {
      /* ignore write failure — theme still applies for this session. */
    }
  }, [theme]);

  const setTheme = useCallback((t: ThemeName) => setThemeState(t), []);
  return { theme, setTheme };
}
