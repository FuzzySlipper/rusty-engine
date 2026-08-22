import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const mode = process.argv[2];
if (mode !== '--check' && mode !== '--write') throw new Error('usage: node scripts/generate-standard-fixtures.mjs --check|--write');
const root = fileURLToPath(new URL('../../', import.meta.url));
const { standardFixtureArtifacts } = await import(new URL('../packages/gameplay-standard-authoring/dist/fixtures.js', import.meta.url));
const fixtures = standardFixtureArtifacts();
const paths = Object.freeze({
  exact: 'fixtures/gameplay-standard/exact-schema-1.canonical.json',
  continuous: 'fixtures/gameplay-standard/continuous-schema-2.canonical.json',
  extensionSchema1: 'fixtures/gameplay-standard/extension-schema-1.canonical.json',
  extensionSchema2: 'fixtures/gameplay-standard/extension-schema-2.canonical.json',
  composedExact: 'fixtures/gameplay-standard/composed-exact-schema-1.canonical.json',
});
for (const [name, path] of Object.entries(paths)) {
  const artifact = fixtures[name];
  if (!artifact) throw new Error(`missing standard fixture artifact ${name}`);
  const target = `${root}${path}`;
  if (mode === '--write') writeFileSync(target, artifact.canonicalJson);
  else if (readFileSync(target, 'utf8') !== artifact.canonicalJson) throw new Error(`gameplay-standard fixture drifted: ${path}; run pnpm --dir rules run generate-standard-fixtures`);
}
