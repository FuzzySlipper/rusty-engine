import { copyFileSync, readFileSync, writeFileSync } from 'node:fs';

const renderRoot = new URL('../', import.meta.url);
const declarations = new URL('packages/live-debug-panel/dist/', renderRoot);
const client = new URL('packages/live-debug-client/dist/index.d.ts', renderRoot);
const artifact = new URL('artifacts/live-debug-panel/', renderRoot);

// The bundle inlines the client, so its declarations point at a sibling copy.
function copyDeclaration(source, destination = source) {
  const contents = readFileSync(new URL(source, declarations), 'utf8')
    .replaceAll("'@rusty-engine/live-debug-client'", "'./live-debug-client.js'");
  writeFileSync(new URL(destination, artifact), contents);
}

copyDeclaration('browser-mount.d.ts', 'index.d.ts');
copyDeclaration('live-debug-panel-model.d.ts');
copyDeclaration('renderer-metrics-widget.d.ts');
copyFileSync(client, new URL('live-debug-client.d.ts', artifact));
const files = ['index.js', 'index.d.ts', 'live-debug-client.d.ts', 'live-debug-panel-model.d.ts', 'renderer-metrics-widget.d.ts'];
writeFileSync(
  new URL('package.json', artifact),
  `${JSON.stringify({
    name: '@rusty-engine/live-debug-panel-browser',
    version: '0.1.0',
    type: 'module',
    main: './index.js',
    types: './index.d.ts',
    exports: { '.': { import: './index.js', types: './index.d.ts' } },
    files,
  }, null, 2)}\n`,
);
