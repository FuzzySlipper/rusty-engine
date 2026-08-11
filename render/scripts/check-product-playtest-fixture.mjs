import { readFileSync, readdirSync } from 'node:fs';

const renderRoot = new URL('../', import.meta.url);
const repositoryRoot = new URL('../../', import.meta.url);
const fixtureRoot = new URL('product-playtest/', renderRoot);
const manifest = JSON.parse(readFileSync(new URL('.den-playwright.json', repositoryRoot), 'utf8'));
const scenario = JSON.parse(readFileSync(new URL('scenario.json', fixtureRoot), 'utf8'));

assert(manifest.project === 'rusty-engine-product-playtest', 'manifest project identity drifted');
assert(manifest.serve?.healthUrl === '/product-playtest/', 'manifest health path drifted');
assert(manifest.playtest?.startPath === '/product-playtest/', 'manifest start path drifted');
assert(manifest.playtest?.viewport?.width === 1280, 'manifest viewport width drifted');
assert(manifest.playtest?.viewport?.height === 720, 'manifest viewport height drifted');
assert(manifest.playtest?.recordVideo === false, 'live video must remain opt-in');
assert(typeof scenario.mission === 'string' && scenario.mission.length > 0, 'scenario mission missing');
assert(Array.isArray(scenario.controls) && scenario.controls.length >= 3, 'scenario controls missing');
assert(scenario.artifacts?.screenshots === true, 'scenario screenshots missing');
assert(scenario.artifacts?.frameBurst?.count >= 2, 'scenario needs repeated visual evidence');

const typescript = readdirSync(fixtureRoot)
  .filter((name) => name.endsWith('.ts'))
  .map((name) => readFileSync(new URL(name, fixtureRoot), 'utf8'))
  .join('\n');
const forbidden = [
  /@rusty-engine\/(?!application-host)/u,
  /\bthree\b/iu,
  /playwright/iu,
  /den-playwright/iu,
  /window\.__/u,
  /declare\s+global/u,
  /getContext\s*\(/u,
  /querySelector[^\n]*canvas/iu,
  /test[-_ ]?hook/iu,
];
for (const pattern of forbidden) {
  assert(!pattern.test(typescript), `product playtest fixture crossed public boundary: ${String(pattern)}`);
}
assert(
  typescript.includes("from '@rusty-engine/application-host'"),
  'fixture must enter through the public application host',
);

console.log('product playtest fixture boundary passed');

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
