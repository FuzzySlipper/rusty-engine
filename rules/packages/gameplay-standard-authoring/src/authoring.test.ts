import test from 'node:test';
import assert from 'node:assert/strict';
import { authorBinary64StandardExtension, authorExactDefinition } from './author.js';
import { standardFixtureArtifacts } from './fixtures.js';
import { readFile } from 'node:fs/promises';

test('exact authoring uses the rules provenance envelope and Rust-owned payload', () => {
  const artifact = authorExactDefinition({ domain: 'game', package: 'standard', version: 1, sources: [{ id: 'rules', path: 'rules.json' }], provenance: [{ subject: 'health_formula', source: 'rules' }], definition: { family: 'exact', semanticsVersion: 1, subject: 'health_formula', source: 'rules', roles: [], tree: { op: 'add', left: { op: 'literal', value: 3 }, right: { op: 'literal', value: 4 } } } });
  assert.match(artifact.canonicalJson, /"family":"exact"/);
  assert.throws(() => authorExactDefinition({ domain: 'game', package: 'standard', version: 1, sources: [{ id: 'rules', path: 'rules.json' }], provenance: [{ subject: 'other_formula', source: 'rules' }], definition: { family: 'exact', semanticsVersion: 1, subject: 'health_formula', source: 'rules', roles: [], tree: { op: 'literal', value: 3 } } }));
  assert.throws(() => authorExactDefinition({ domain: 'game', package: 'standard', version: 1, sources: [{ id: 'rules', path: 'rules.json' }], provenance: [{ subject: 'health_formula', source: 'rules' }], definition: { family: 'exact', semanticsVersion: 1, subject: 'health_formula', source: 'rules', roles: [], tree: { op: 'literal', value: -0 } } }));
});

test('extension authoring selects binary64 envelope and requires exact provenance correlation', () => {
  const schema = { namespace: 'example.combat', version: 1 } as const;
  const artifact = authorBinary64StandardExtension({ domain: 'game', package: 'combat-extension', version: 1, sources: [{ id: 'rules', path: 'rules.json' }], provenance: [{ subject: 'guard', source: 'rules' }], schema, kind: 'combat.option', subject: 'guard', source: 'rules', payload: { weight: 1.5 } });
  assert.equal(artifact.package.schemaVersion, 2);
  assert.throws(() => authorBinary64StandardExtension({ domain: 'game', package: 'combat-extension', version: 1, sources: [{ id: 'rules', path: 'rules.json' }], provenance: [{ subject: 'other', source: 'rules' }], schema, kind: 'combat.option', subject: 'guard', source: 'rules', payload: { weight: 1.5 } }));
});

test('typed authoring converges exactly on the Rust-owned exact, continuous, and extension fixtures', async () => {
  const artifacts = standardFixtureArtifacts();
  const fixtures = {
    exact: '../../../../fixtures/gameplay-standard/exact-schema-1.canonical.json',
    continuous: '../../../../fixtures/gameplay-standard/continuous-schema-2.canonical.json',
    extensionSchema1: '../../../../fixtures/gameplay-standard/extension-schema-1.canonical.json',
    extensionSchema2: '../../../../fixtures/gameplay-standard/extension-schema-2.canonical.json',
    composedExact: '../../../../fixtures/gameplay-standard/composed-exact-schema-1.canonical.json',
  } as const;
  for (const [name, path] of Object.entries(fixtures)) {
    const artifact = artifacts[name];
    assert.ok(artifact);
    assert.equal(artifact.canonicalJson, await readFile(new URL(path, import.meta.url), 'utf8'));
  }
});
