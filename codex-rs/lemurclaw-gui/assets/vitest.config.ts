import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

// Vitest config. Separate from vite.config.ts so production build (vite build)
// is not affected by test-only setup (jsdom globals, testing-library
// matchers). Reuses the same React plugin for JSX transform.
export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/__tests__/setup.ts'],
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
    exclude: ['node_modules', 'dist', 'src/types/**'],
  },
});
