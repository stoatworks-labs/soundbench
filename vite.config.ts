import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

import { readFileSync } from 'node:fs';

const pkg = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8'));

// Desktop app, not a website: the output is loaded from the Tauri bundle over
// the custom protocol, so `base` is relative and there is no dev-server proxy.
//
// `clearScreen: false` keeps the Rust compiler's output visible when Vite and
// cargo share a terminal under `tauri dev` — without it, a Rust error scrolls
// away behind Vite's banner and the app just silently fails to open.
export default defineConfig({
  // The About dialog shows the version the build actually produced.
  define: { __APP_VERSION__: JSON.stringify(`v${pkg.version}`) },
  plugins: [react()],
  base: './',
  clearScreen: false,
  server: {
    port: 5174,
    strictPort: true,
  },
  build: {
    outDir: 'dist',
    sourcemap: true,
    // Tauri ships its own webview; there is no old-browser tail to support.
    target: 'es2023',
  },
  test: { environment: 'node' },
});
