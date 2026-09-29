import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

const renderRoot = new URL('./', import.meta.url);

/**
 * Build the Engine-owned browser shell closure, application host included,
 * as one ordinary ES module the runtime pack serves beside its shell page.
 */
export default defineConfig({
  build: {
    emptyOutDir: true,
    lib: {
      entry: fileURLToPath(new URL('packages/product-browser-host/src/index.ts', renderRoot)),
      formats: ['es'],
      fileName: () => 'product-browser-host.js',
    },
    minify: 'oxc',
    outDir: fileURLToPath(new URL('artifacts/product-browser-host', renderRoot)),
    sourcemap: false,
    target: 'es2022',
  },
});
