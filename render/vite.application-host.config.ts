import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

const outputDirectory = fileURLToPath(
  new URL('./artifacts/application-host', import.meta.url),
);

export default defineConfig({
  build: {
    emptyOutDir: true,
    lib: {
      entry: fileURLToPath(
        new URL('./packages/application-host/src/index.ts', import.meta.url),
      ),
      formats: ['es'],
      fileName: () => 'index.js',
    },
    minify: 'oxc',
    outDir: outputDirectory,
    sourcemap: false,
    target: 'es2022',
  },
});
