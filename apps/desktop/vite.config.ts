import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const repoRoot = fileURLToPath(new URL('../..', import.meta.url));

// The browser version is deployed from main between releases, so the version
// names the commit: v0.2.0 on a tag, v0.2.0-43-g060248a 43 commits after it.
function describe(): string {
  try {
    // no shell, so the pattern reaches git unquoted on Windows too
    return execFileSync('git', ['describe', '--tags', '--match', 'v*', '--always', '--dirty'], { cwd: repoRoot, encoding: 'utf8' }).trim();
  } catch {
    return 'unknown';
  }
}

// The desktop app is built on the platform it runs on (no cross builds).
const BUILD_PLATFORMS: Record<string, string> = { win32: 'Windows', darwin: 'macOS', linux: 'Linux' };

// Tauri expects a fixed dev port and serves the built files from ../dist.
// `--mode web` is the static build for GitHub Pages: it serves the wasm
// engine from web/ (`npm run build:wasm`).
export default defineConfig(({ mode }) => ({
  plugins: [react()],
  publicDir: mode === 'web' ? 'web' : 'public',
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // sample antennas are imported from <repo>/testdata
    fs: { allow: [repoRoot] },
    watch: { ignored: ['**/src-tauri/**'] },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  define: {
    __MMANA_VERSION__: JSON.stringify(describe()),
    __MMANA_BUILT_AT__: JSON.stringify(new Date().toISOString()),
    __MMANA_BUILD_PLATFORM__: JSON.stringify(BUILD_PLATFORMS[process.platform] ?? process.platform),
    // a fork's CI builds point their reports at the fork
    __MMANA_REPOSITORY__: JSON.stringify(process.env.GITHUB_REPOSITORY || 'Ebycow/MMANA'),
  },
  build: {
    target: 'es2022',
    chunkSizeWarningLimit: 1500,
  },
  worker: { format: 'es' },
}));
