import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Vite config for lemurclaw-gui frontend.
// Output is a static SPA bundled into the wry webview at build time via
// lemurclaw-gui/build.rs (npm run build → assets/dist/).
export default defineConfig({
  plugins: [react()],
  // `base: './'` makes asset URLs relative to index.html. This is required
  // for `file://` loading (Task 2.3 dev path) and for a custom wry protocol
  // (production path), neither of which has an http origin to anchor `/`.
  base: './',
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
  },
});
