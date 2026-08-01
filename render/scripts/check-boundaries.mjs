import { readFileSync, readdirSync } from 'node:fs';

const root = new URL('../', import.meta.url);
const packages = new Map([
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
  if (manifest.scripts?.prepare !== 'pnpm run build') {
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
