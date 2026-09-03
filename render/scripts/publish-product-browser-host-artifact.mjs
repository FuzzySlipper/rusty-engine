import { copyFileSync, mkdirSync, writeFileSync } from 'node:fs';

const renderRoot = new URL('../', import.meta.url);
const declarations = new URL('packages/product-browser-host/dist/', renderRoot);
const artifact = new URL('artifacts/product-browser-host/', renderRoot);

mkdirSync(artifact, { recursive: true });
copyFileSync(new URL('index.d.ts', declarations), new URL('index.d.ts', artifact));
for (const file of ['local-transport.d.ts', 'product-browser-host.d.ts', 'renderer-preload.d.ts']) {
  copyFileSync(new URL(file, declarations), new URL(file, artifact));
}
writeFileSync(new URL('index.js', artifact), "export * from './product-browser-host.js';\n");
writeFileSync(
  new URL('package.json', artifact),
  `${JSON.stringify({
    name: '@rusty-engine/product-browser-host',
    version: '0.1.0',
    type: 'module',
    main: './index.js',
    types: './index.d.ts',
    exports: {
      '.': {
        import: './index.js',
        types: './index.d.ts',
      },
    },
    files: [
      'index.js',
      'index.d.ts',
      'local-transport.d.ts',
      'product-browser-host.d.ts',
      'product-browser-host.js',
      'renderer-preload.d.ts',
      'package.json',
    ],
  }, null, 2)}\n`,
);
