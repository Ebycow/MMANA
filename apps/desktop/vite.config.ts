import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { fileURLToPath } from 'node:url';

const repoRoot = fileURLToPath(new URL('../..', import.meta.url));

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
  build: {
    target: 'es2022',
    chunkSizeWarningLimit: 1500,
  },
  worker: { format: 'es' },
}));
