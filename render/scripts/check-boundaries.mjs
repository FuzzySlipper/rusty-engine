import { readFileSync } from 'node:fs';

const root = new URL('../', import.meta.url);
const packages = new Map([
  ['application-host', { dependencies: [], peers: [] }],
  ['product-browser-host', {
    dependencies: [],
    peers: ['@rusty-engine/application-host'],
  }],
  ['render-contracts', { dependencies: [], peers: [] }],
  ['render-projection', {
    dependencies: [],
    peers: ['@rusty-engine/render-contracts'],
  }],
  ['renderer-three', {
    dependencies: ['@noble/hashes', '@types/three', 'fflate', 'three'],
    peers: [
      '@rusty-engine/render-contracts',
      '@rusty-engine/render-projection',
    ],
  }],
  ['renderer-host', {
    dependencies: [],
    peers: [
      '@rusty-engine/render-contracts',
      '@rusty-engine/render-projection',
      '@rusty-engine/renderer-three',
    ],
  }],
]);

for (const [name, expected] of packages) {
  const manifest = JSON.parse(readFileSync(new URL(`packages/${name}/package.json`, root), 'utf8'));
  assertKeys(name, 'dependencies', manifest.dependencies, expected.dependencies);
  assertKeys(name, 'peerDependencies', manifest.peerDependencies, expected.peers);
}

const applicationArtifact = JSON.parse(
  readFileSync(new URL('artifacts/application-host/package.json', root), 'utf8'),
);
const productBrowserArtifact = JSON.parse(
  readFileSync(new URL('artifacts/product-browser-host/package.json', root), 'utf8'),
);
if (productBrowserArtifact.name !== '@rusty-engine/product-browser-host') {
  throw new Error('product browser host artifact must keep its runtime bundle identity');
}
for (const file of productBrowserArtifact.files) {
  readFileSync(new URL(`artifacts/product-browser-host/${file}`, root), 'utf8');
}
const productBrowserRuntime = readFileSync(
  new URL('artifacts/product-browser-host/product-browser-host.js', root),
  'utf8',
);
if (productBrowserRuntime.split(/\r?\n/u).some((line) => /^\s*(?:import|export)\b/u.test(line)
  && /['"]@rusty-engine\//u.test(line))) {
  throw new Error('product browser host artifact leaked a bare Engine package import');
}
assertKeys(
  'application-host artifact',
  'dependencies',
  applicationArtifact.dependencies,
  [],
);
assertKeys(
  'application-host artifact',
  'peerDependencies',
  applicationArtifact.peerDependencies,
  [],
);
if (applicationArtifact.name !== '@rusty-engine/application-host') {
  throw new Error('application-host artifact must keep its Engine bundle identity');
}
// These are Engine implementation artifacts. Ordinary products consume the C#
// SDK/runtime pack, not a public TypeScript declaration package. Keep package
// dependency direction above and verify that both browser bundles are closed.
const applicationRuntime = readFileSync(new URL('artifacts/application-host/index.js', root), 'utf8');
if (applicationRuntime.split(/\r?\n/u).some((line) => /^\s*(?:import|export)\b/u.test(line)
  && /['"]@rusty-engine\//u.test(line))) {
  throw new Error('application host artifact leaked a bare Engine package import');
}

console.log('render package boundaries passed');

function assertKeys(packageName, field, value, expected) {
  const actual = Object.keys(value ?? {}).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    throw new Error(`${packageName} ${field} ${JSON.stringify(actual)} do not match ${JSON.stringify(wanted)}`);
  }
}
