import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';

const renderRoot = new URL('../', import.meta.url);
const declarations = new URL('packages/live-debug-panel/dist/', renderRoot);
const client = new URL('packages/live-debug-client/dist/index.d.ts', renderRoot);
const contracts = new URL('packages/live-debug-client/dist/generated/contracts.d.ts', renderRoot);
const artifact = new URL('artifacts/live-debug-panel/', renderRoot);

// The bundle inlines the client, so its declarations point at a sibling copy.
function copyDeclaration(source, destination = source) {
  const contents = readFileSync(new URL(source, declarations), 'utf8')
    .replaceAll("'@rusty-engine/live-debug-client'", "'./live-debug-client.js'");
  writeFileSync(new URL(destination, artifact), contents);
}

// The entry's declarations and every sibling they import, however deep.
const shipped = [];
const pending = ['browser-mount.d.ts'];
while (pending.length > 0) {
  const source = pending.shift();
  if (shipped.includes(source)) continue;
  shipped.push(source);
  const contents = readFileSync(new URL(source, declarations), 'utf8');
  for (const [, name] of contents.matchAll(/from '\.\/([^']+)\.js'/g)) pending.push(`${name}.d.ts`);
}
for (const source of shipped) copyDeclaration(source, source === 'browser-mount.d.ts' ? 'index.d.ts' : source);
copyFileSync(client, new URL('live-debug-client.d.ts', artifact));
// The client's declarations re-export its generated contracts.
mkdirSync(new URL('generated/', artifact), { recursive: true });
copyFileSync(contracts, new URL('generated/contracts.d.ts', artifact));
const files = ['index.js', 'live-debug-client.d.ts', 'generated/contracts.d.ts', ...shipped.map((source) => (source === 'browser-mount.d.ts' ? 'index.d.ts' : source))];
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
