import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

const renderRoot = new URL('./', import.meta.url);

/**
 * Build the Engine-owned browser-host closure as one ordinary ES module.
 * application-host is already a bundled public artifact; aliasing that exact
 * artifact keeps the result free of bare package imports at runtime.
 */
export default defineConfig({
  resolve: {
    alias: [
      {
        find: '@rusty-engine/application-host',
        replacement: fileURLToPath(new URL('artifacts/application-host/index.js', renderRoot)),
      },
    ],
  },
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
