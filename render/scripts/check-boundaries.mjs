import { readFileSync, readdirSync } from 'node:fs';

const root = new URL('../', import.meta.url);
const packages = new Map([
  ['developer-command-client', { dependencies: [], peers: [], preparesGitConsumer: true }],
  ['application-host', { dependencies: [], peers: [], preparesGitConsumer: false }],
  ['render-contracts', { dependencies: [], peers: [], preparesGitConsumer: true }],
  ['render-projection', {
    dependencies: [],
    peers: ['@rusty-engine/render-contracts'],
    preparesGitConsumer: true,
  }],
  ['renderer-three', {
    dependencies: ['@noble/hashes', '@types/three', 'fflate', 'three'],
    peers: [
      '@rusty-engine/render-contracts',
      '@rusty-engine/render-projection',
    ],
    preparesGitConsumer: true,
  }],
  ['renderer-host', {
    dependencies: [],
    peers: [
      '@rusty-engine/render-contracts',
      '@rusty-engine/render-projection',
      '@rusty-engine/renderer-three',
    ],
    preparesGitConsumer: true,
  }],
]);

for (const [name, expected] of packages) {
  const manifest = JSON.parse(readFileSync(new URL(`packages/${name}/package.json`, root), 'utf8'));
  assertKeys(name, 'dependencies', manifest.dependencies, expected.dependencies);
  assertKeys(name, 'peerDependencies', manifest.peerDependencies, expected.peers);
  if (expected.preparesGitConsumer && manifest.scripts?.prepare !== 'pnpm run build') {
    throw new Error(`${name} must prepare its distributable output for exact-revision Git consumers`);
  }

  for (const peer of expected.peers) {
    if (manifest.peerDependencies[peer] !== '0.1.0') {
      throw new Error(`${name} peer ${peer} must match the shared 0.1.0 package family`);
    }
    if (manifest.devDependencies?.[peer] !== 'workspace:*') {
      throw new Error(`${name} peer ${peer} must use its workspace package for provider builds`);
    }
  }
}

const applicationArtifact = JSON.parse(
  readFileSync(new URL('artifacts/application-host/package.json', root), 'utf8'),
);
const developerCommandArtifact = JSON.parse(
  readFileSync(new URL('artifacts/developer-command-client/package.json', root), 'utf8'),
);
if (developerCommandArtifact.name !== '@rusty-engine/developer-command-client') {
  throw new Error('developer-command client artifact must keep its public package identity');
}
for (const file of developerCommandArtifact.files) {
  readFileSync(new URL(`artifacts/developer-command-client/${file}`, root), 'utf8');
}
for (const file of ['index.js', 'index.d.ts', 'developer-command-client.d.ts', 'developer-command-client.js', 'developer-command-shell.d.ts', 'generated-developer-command-contract.js', 'generated-standard-host-wire.js']) {
  const source = readFileSync(new URL(`artifacts/application-host/${file}`, root), 'utf8');
  if (source.includes('@rusty-engine/developer-command-client')) {
    throw new Error(`application-host artifact ${file} leaked external developer-command client dependency`);
  }
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
  throw new Error('application-host artifact must own the sole public downstream package name');
}
for (const declaration of [
  'index.d.ts',
  'application-host.d.ts',
  'application-content.d.ts',
  'input-ingress.d.ts',
  'ui-projection.d.ts',
]) {
  const source = readFileSync(new URL(`artifacts/application-host/${declaration}`, root), 'utf8');
  if (/@rusty-engine\/(?:render|renderer)|\bthree\b|studio/iu.test(source)) {
    throw new Error(
      `application-host artifact declaration ${declaration} leaked an internal package or backend`,
    );
  }
}

const forbidden = /@asha\/|runtime-bridge|runtime-session|RuntimeBridge|RuntimeSession|ReplayRecord|ReactionFrame|DecisionReceipt|ProposalEnvelope/g;
const violations = [];
for (const name of packages.keys()) {
  walk(new URL(`packages/${name}/src/`, root), (url) => {
    if (!url.pathname.endsWith('.ts')) return;
    const source = readFileSync(url, 'utf8');
    for (const match of source.matchAll(forbidden)) {
      violations.push(`${url.pathname}:${String(lineAt(source, match.index ?? 0))}:${match[0]}`);
    }
  });
}
if (violations.length > 0) {
  throw new Error(`old runtime spine crossed the render boundary:\n${violations.join('\n')}`);
}

console.log('render package boundaries passed');

function assertKeys(packageName, field, value, expected) {
  const actual = Object.keys(value ?? {}).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    throw new Error(`${packageName} ${field} ${JSON.stringify(actual)} do not match ${JSON.stringify(wanted)}`);
  }
}

function walk(url, visit) {
  for (const entry of readdirSync(url, { withFileTypes: true })) {
    const child = new URL(`${entry.name}${entry.isDirectory() ? '/' : ''}`, url);
    if (entry.isDirectory()) walk(child, visit);
    else visit(child);
  }
}

function lineAt(source, index) {
  return source.slice(0, index).split('\n').length;
}
