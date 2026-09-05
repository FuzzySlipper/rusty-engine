import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';

const renderRoot = new URL('../', import.meta.url);
const declarations = new URL('packages/application-host/dist/', renderRoot);
const artifact = new URL('artifacts/application-host/', renderRoot);

mkdirSync(artifact, { recursive: true });
const bundle = new URL('index.js', artifact);
writeFileSync(
  bundle,
  readFileSync(bundle, 'utf8').replace(
    /^\/\/#region .*?node_modules\/\.pnpm\//gm,
    '//#region node_modules/.pnpm/',
  ),
);
for (const file of [
  'index.d.ts',
  'application-host.d.ts',
  'application-content.d.ts',
  'input-ingress.d.ts',
  'presentation-frame.d.ts',
  'ui-projection.d.ts',
]) {
  copyFileSync(new URL(file, declarations), new URL(file, artifact));
}
writeFileSync(
  new URL('package.json', artifact),
  `${JSON.stringify({
    name: '@rusty-engine/application-host',
    private: true,
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
    files: ['application-content.d.ts', 'application-host.d.ts', 'index.d.ts', 'index.js', 'input-ingress.d.ts', 'presentation-frame.d.ts', 'ui-projection.d.ts'],
  }, null, 2)}\n`,
);
