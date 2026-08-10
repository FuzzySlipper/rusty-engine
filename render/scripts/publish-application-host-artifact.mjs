import { copyFileSync, mkdirSync, writeFileSync } from 'node:fs';

const renderRoot = new URL('../', import.meta.url);
const declarations = new URL('packages/application-host/dist/', renderRoot);
const artifact = new URL('artifacts/application-host/', renderRoot);

mkdirSync(artifact, { recursive: true });
for (const file of ['index.d.ts', 'application-host.d.ts']) {
  copyFileSync(new URL(file, declarations), new URL(file, artifact));
}
writeFileSync(
  new URL('package.json', artifact),
  `${JSON.stringify({
    name: '@rusty-engine/application-host',
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
    files: ['application-host.d.ts', 'index.d.ts', 'index.js'],
  }, null, 2)}\n`,
);
