import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Vite config for lemurclaw-gui frontend.
// Output is a static SPA bundled into the wry webview at build time via
// lemurclaw-gui/build.rs (npm run build → assets/dist/), then embedded into
// the Rust binary via `include_dir!` and served from a custom wry protocol
// (`lemurclaw://app/...`). See lemurclaw-gui/src/assets.rs.
export default defineConfig({
  plugins: [react()],
  // `base: './'` makes asset URLs relative to index.html. Required because
  // the bundle is served under a synthetic origin (`lemurclaw://app/`)
  // where an absolute `/assets/...` would resolve incorrectly.
  base: './',
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
  },
});
