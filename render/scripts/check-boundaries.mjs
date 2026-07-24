import { readFileSync, readdirSync } from 'node:fs';

const root = new URL('../', import.meta.url);
const packages = new Map([
  ['render-contracts', []],
  ['render-projection', ['@rusty-engine/render-contracts']],
  ['renderer-three', [
    '@rusty-engine/render-contracts',
    '@rusty-engine/render-projection',
    'three',
  ]],
]);

for (const [name, expected] of packages) {
  const manifest = JSON.parse(readFileSync(new URL(`packages/${name}/package.json`, root), 'utf8'));
  const actual = Object.keys(manifest.dependencies ?? {}).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    throw new Error(`${name} dependencies ${JSON.stringify(actual)} do not match ${JSON.stringify(wanted)}`);
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
