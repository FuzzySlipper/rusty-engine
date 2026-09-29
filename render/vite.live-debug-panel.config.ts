import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

const renderRoot = new URL('./', import.meta.url);

/**
 * Build the optional live-debug panel as one import-closed ES module that a
 * runtime pack or product can copy and load without a bare-import resolver.
 */
export default defineConfig({
  build: {
    emptyOutDir: true,
    lib: {
      entry: fileURLToPath(new URL('packages/live-debug-panel/src/browser-mount.ts', renderRoot)),
      formats: ['es'],
      fileName: () => 'index.js',
    },
    minify: 'oxc',
    outDir: fileURLToPath(new URL('artifacts/live-debug-panel', renderRoot)),
    sourcemap: false,
    target: 'es2022',
  },
});
