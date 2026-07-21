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
    // The `include` glob above already restricts discovery to *.test.* /
    // *.spec.* files, so the 616 ts-rs generated files under src/types/
    // (none of which match that pattern) are never collected. We rely on
    // that rather than excluding src/types/** — a directory exclude would
    // also hide any future *.test.ts under src/types/, which we don't want.
    exclude: ['node_modules', 'dist'],
  },
});
