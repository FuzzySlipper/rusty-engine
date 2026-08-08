import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

const outputDirectory = fileURLToPath(
  new URL('../rust/crates/renderer-webview-host/artifacts', import.meta.url),
);

export default defineConfig({
  build: {
    emptyOutDir: true,
    lib: {
      entry: fileURLToPath(new URL('./private/webview/main.ts', import.meta.url)),
      formats: ['iife'],
      name: 'RustyEnginePrivateRendererArtifact',
      fileName: () => 'renderer-webview.js',
    },
    minify: 'oxc',
    outDir: outputDirectory,
    sourcemap: false,
    target: 'es2022',
  },
});
