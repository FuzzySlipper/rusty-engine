import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

const renderRoot = new URL('./', import.meta.url);

/**
 * Build the optional video options panel as one import-closed ES module that
 * the runtime pack serves at `engine/video-options/index.js`.
 */
export default defineConfig({
  build: {
    emptyOutDir: true,
    lib: {
      entry: fileURLToPath(new URL('packages/video-options/src/browser-mount.ts', renderRoot)),
      formats: ['es'],
      fileName: () => 'index.js',
    },
    minify: 'oxc',
    outDir: fileURLToPath(new URL('artifacts/video-options', renderRoot)),
    sourcemap: false,
    target: 'es2022',
  },
});
