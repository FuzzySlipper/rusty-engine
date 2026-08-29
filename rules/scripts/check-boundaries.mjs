import { existsSync, readFileSync, readdirSync } from 'node:fs';

const root = new URL('../', import.meta.url);
const repositoryRoot = new URL('../../', import.meta.url);
const packages = new Map([
  [
    'runtime-composition-authoring',
    {
      dependencies: [],
      peers: [],
      devDependencies: ['@types/node'],
    },
  ],
]);

for (const [name, expected] of packages) {
  const manifest = JSON.parse(
    readFileSync(new URL(`packages/${name}/package.json`, root), 'utf8'),
  );
  assertKeys(name, 'dependencies', manifest.dependencies, expected.dependencies);
  assertKeys(name, 'peerDependencies', manifest.peerDependencies, expected.peers);
  assertKeys(
    name,
    'devDependencies',
    manifest.devDependencies,
    expected.devDependencies,
  );
  if (manifest.scripts?.prepare !== 'pnpm run build') {
    throw new Error(`${name} must prepare package-root distributable output`);
  }
  if (Object.keys(manifest.exports ?? {}).join(',') !== '.') {
    throw new Error(`${name} must expose only its package root`);
  }
  for (const peer of expected.peers) {
    if (manifest.peerDependencies[peer] !== '0.1.0') {
      throw new Error(`${name} peer ${peer} must match the 0.1.0 package family`);
    }
    if (manifest.devDependencies[peer] !== 'workspace:*') {
      throw new Error(`${name} peer ${peer} must use the workspace package in provider tests`);
    }
  }
}

const forbidden = /\b(?:D20|ArmorClass|SavingThrow|SpellDefinition|FeatDefinition|AttackRoll|RuntimeSession|RuntimeBridge|ReplayRecord|GameplayContext|ServiceLocator|RuleRegistry|RuleEvaluator|RuleScheduler)\b/g;
const browser = /\b(?:window|document|localStorage|sessionStorage|indexedDB|fetch)\b/g;
const violations = [];
for (const name of packages.keys()) {
  walk(new URL(`packages/${name}/src/`, root), (url) => {
    if (!url.pathname.endsWith('.ts')) return;
    const source = readFileSync(url, 'utf8');
    for (const match of source.matchAll(forbidden)) {
      violations.push(
        `${url.pathname}:${String(lineAt(source, match.index ?? 0))}:forbidden semantic/runtime vocabulary ${match[0]}`,
      );
    }
    for (const match of source.matchAll(browser)) {
      violations.push(
        `${url.pathname}:${String(lineAt(source, match.index ?? 0))}:browser API ${match[0]}`,
      );
    }
    for (const match of source.matchAll(
      /@rusty-engine\/[^'"]+\/[^'"]+/g,
    )) {
      violations.push(
        `${url.pathname}:${String(lineAt(source, match.index ?? 0))}:deep package import ${match[0]}`,
      );
    }
  });
}
if (violations.length > 0) {
  throw new Error(`runtime-composition authoring boundary violations:\n${violations.join('\n')}`);
}

const ordinaryVerify = readFileSync(
  new URL('scripts/verify.sh', repositoryRoot),
  'utf8',
);
if (ordinaryVerify.includes('verify-rules') || ordinaryVerify.includes('pnpm')) {
  throw new Error('ordinary provider verification must remain Node-free');
}
const rootWorkspaceUrl = new URL('pnpm-workspace.yaml', repositoryRoot);
if (
  existsSync(rootWorkspaceUrl) &&
  readFileSync(rootWorkspaceUrl, 'utf8').trim() !== ''
) {
  throw new Error('runtime-composition authoring must not join a repository-root pnpm workspace');
}
const productModelGenerated = readFileSync(
  new URL(
    'packages/runtime-composition-authoring/src/generated.ts',
    root,
  ),
  'utf8',
);
if (!productModelGenerated.startsWith('// Generated from Rust product-model contract descriptor.')) {
  throw new Error('generated Product Model contract lost its Rust ownership marker');
}

console.log('runtime-composition authoring package boundaries passed');

function assertKeys(packageName, field, value, expected) {
  const actual = Object.keys(value ?? {}).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    throw new Error(
      `${packageName} ${field} ${JSON.stringify(actual)} do not match ${JSON.stringify(wanted)}`,
    );
  }
}

function walk(url, visit) {
  for (const entry of readdirSync(url, { withFileTypes: true })) {
    const child = new URL(
      `${entry.name}${entry.isDirectory() ? '/' : ''}`,
      url,
    );
    if (entry.isDirectory()) walk(child, visit);
    else visit(child);
  }
}

function lineAt(source, index) {
  return source.slice(0, index).split('\n').length;
}
