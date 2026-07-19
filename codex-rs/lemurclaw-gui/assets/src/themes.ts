// Theme definitions for the GUI. Each theme is a map of CSS variable name →
// value, applied to the root element via `data-theme="<name>"`. The actual
// variable → CSS rule mapping lives in styles.css ([data-theme="..."] blocks);
// this file just enumerates the available theme names + metadata for the
// picker UI.

export type ThemeName = 'light' | 'dark' | 'high-contrast';

export interface ThemeMeta {
  name: ThemeName;
  label: string;
  description: string;
}

export const THEMES: ThemeMeta[] = [
  { name: 'light', label: 'Light', description: 'default bright theme' },
  { name: 'dark', label: 'Dark', description: 'low-light theme' },
  { name: 'high-contrast', label: 'High contrast', description: 'maximum text/background separation' },
];

export const DEFAULT_THEME: ThemeName = 'light';
