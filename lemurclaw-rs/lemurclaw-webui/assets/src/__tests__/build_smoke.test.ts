import { describe, it, expect, beforeAll } from 'vitest';
import { execSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

// Build smoke test: runs `npm run build` once, then asserts the production
// bundle is well-formed. Guards against:
//   - TypeScript / JSX that compiles in test (esbuild) but breaks under
//     vite build (rollup) — e.g. bad import paths, circular deps.
//   - Missing assets that include_dir! / the wry custom protocol expects.
//   - Empty / zero-byte chunks that would ship a blank page.
//
// This runs as a vitest test (not a shell script) so `npm test` in CI
// exercises it automatically. The build is slow (~5s) so it's isolated in
// its own file and runs once via beforeAll.

const distDir = join(process.cwd(), 'dist');
const assetsDir = join(distDir, 'assets');

describe('production build (vite build smoke)', () => {
  beforeAll(() => {
    // Run the real vite build. execSync throws on non-zero exit, failing the
    // suite — that's what we want if the build breaks.
    execSync('npm run build', { stdio: 'pipe', cwd: process.cwd() });
  }, 60_000);

  it('dist/index.html exists and is non-empty', () => {
    const idx = join(distDir, 'index.html');
    expect(existsSync(idx)).toBe(true);
    expect(statSync(idx).size).toBeGreaterThan(0);
  });

  it('dist/index.html references at least one JS bundle', () => {
    const html = readFileSync(join(distDir, 'index.html'), 'utf8');
    // vite emits hashed chunks under ./assets/ and references them with a
    // relative `./assets/<name>-<hash>.js`. Match loosely so hash changes
    // don't break this assertion.
    expect(html).toMatch(/\.\/assets\/[^"]+\.js/);
  });

  it('dist/assets/ contains a non-empty JS bundle', () => {
    expect(existsSync(assetsDir)).toBe(true);
    const js = readdirSync(assetsDir).filter((f) => f.endsWith('.js'));
    expect(js.length).toBeGreaterThan(0);
    for (const f of js) {
      const size = statSync(join(assetsDir, f)).size;
      // The React + app bundle is hundreds of KB; sanity floor at 10 KB to
      // catch accidental empty chunks without being flaky.
      expect(size, `${f} should be non-trivial`).toBeGreaterThan(10_000);
    }
  });

  it('dist/assets/ contains a non-empty CSS bundle', () => {
    expect(existsSync(assetsDir)).toBe(true);
    const css = readdirSync(assetsDir).filter((f) => f.endsWith('.css'));
    expect(css.length).toBeGreaterThan(0);
    for (const f of css) {
      const size = statSync(join(assetsDir, f)).size;
      expect(size, `${f} should be non-trivial`).toBeGreaterThan(1_000);
    }
  });
});
