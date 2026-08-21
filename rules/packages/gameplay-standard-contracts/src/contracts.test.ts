import assert from 'node:assert/strict';
import test from 'node:test';

import {
  StandardContractError,
  decodeStandardExtensionArtifact,
  decodeStandardPayload,
} from './index.js';

const exact = {
  family: 'exact',
  roles: [{ role: 'self', capabilities: ['read.stat'] }],
  semanticsVersion: 1,
  source: 'rules',
  subject: 'health_formula',
  tree: {
    op: 'add',
    left: { op: 'literal', value: 3 },
    right: { op: 'input', input: { kind: 'standardStat', role: 'self', stat: 'health' } },
  },
} as const;

test('generated exact shapes validate closed fields, roles, mechanics IDs, and scalar limits', () => {
  assert.deepEqual(decodeStandardPayload(exact), exact);
  expectCode(() => decodeStandardPayload({ ...exact, extra: true }), 'unknown-field');
  expectCode(() => decodeStandardPayload({ ...exact, roles: [] }), 'undeclared-input-role');
  expectCode(() => decodeStandardPayload({ ...exact, tree: { op: 'literal', value: 1_000_000_000_001 } }), 'invalid-literal');
  expectCode(() => decodeStandardPayload({ ...exact, tree: { op: 'literal', value: -0 } }), 'invalid-literal');
  expectCode(() => decodeStandardPayload({ ...exact, roles: [{ role: 'self', capabilities: ['z', 'a'] }] }), 'non-canonical-roles');
});

test('generated continuous grammar rejects negative zero and non-finite binary64 encodings', () => {
  const continuous = {
    family: 'continuous', roles: [], semanticsVersion: 1, source: 'rules', subject: 'speed_formula',
    tree: { op: 'literal', bits: '0000000000000001' },
  } as const;
  assert.deepEqual(decodeStandardPayload(continuous), continuous);
  expectCode(() => decodeStandardPayload({ ...continuous, tree: { op: 'literal', bits: '8000000000000000' } }), 'invalid-literal');
  expectCode(() => decodeStandardPayload({ ...continuous, tree: { op: 'literal', bits: '7ff0000000000000' } }), 'invalid-literal');
  expectCode(() => decodeStandardPayload({ ...continuous, tree: { op: 'literal', bits: 'fff0000000000000' } }), 'invalid-literal');
  expectCode(() => decodeStandardPayload({ ...continuous, tree: { op: 'literal', bits: 'fff8000000000000' } }), 'invalid-literal');
});

for (const family of ['exact', 'continuous'] as const) {
  test(`${family} recursive quotas use the generated Rust limits`, () => {
    const payload = definition(family);
    assert.deepEqual(decodeStandardPayload(payload), payload);
    expectCode(() => decodeStandardPayload({ ...payload, tree: chain(family, 33) }), 'depth-quota-exceeded');
    expectCode(() => decodeStandardPayload({ ...payload, tree: { op: 'min', values: Array.from({ length: 17 }, () => literal(family)) } }), 'arity-quota-exceeded');
    expectCode(() => decodeStandardPayload({ ...payload, tree: balanced(family, 129) }), 'node-quota-exceeded');
    expectCode(() => decodeStandardPayload({ ...payload, roles: [{ role: 'self', capabilities: [] }], tree: balancedInputs(family, 65) }), 'input-quota-exceeded');
  });
}

test('generated extension artifact grammar remains separate and bounded', () => {
  const artifact = { family: 'standardExtension', kind: 'combat.option', namespace: 'example.combat', payload: { option: 'guard' }, schemaVersion: 1, source: 'rules', subject: 'guard' } as const;
  assert.deepEqual(decodeStandardExtensionArtifact(artifact), artifact);
  expectCode(() => decodeStandardExtensionArtifact({ ...artifact, ignored: true }), 'unknown-field');
  expectCode(() => decodeStandardExtensionArtifact({ ...artifact, namespace: 'a'.repeat(97) }), 'invalid-identity');
  expectCode(() => decodeStandardExtensionArtifact({ ...artifact, schemaVersion: 4_294_967_296 }), 'extension-schema-mismatch');
});

test('extension payloads accept only stable plain JSON data', () => {
  const artifact = { family: 'standardExtension', kind: 'combat.option', namespace: 'example.combat', payload: { nested: [{ option: 'guard' }, { values: [1, true, null] }] }, schemaVersion: 1, source: 'rules', subject: 'guard' } as const;
  assert.deepEqual(decodeStandardExtensionArtifact(artifact), artifact);
  expectCode(() => decodeStandardExtensionArtifact({ ...artifact, payload: new Date() }), 'invalid-node');
  expectCode(() => decodeStandardExtensionArtifact({ ...artifact, payload: new Map([['option', 'guard']]) }), 'invalid-node');
  expectCode(() => decodeStandardExtensionArtifact({ ...artifact, payload: Object.create({ inherited: true }) }), 'invalid-node');
  expectCode(() => decodeStandardExtensionArtifact({ ...artifact, payload: { toJSON: () => ({ option: 'guard' }) } }), 'invalid-node');
  const accessor: Record<string, unknown> = {};
  Object.defineProperty(accessor, 'option', { enumerable: true, get: () => 'guard' });
  expectCode(() => decodeStandardExtensionArtifact({ ...artifact, payload: accessor }), 'invalid-node');
});

function expectCode(action: () => unknown, code: string): void {
  assert.throws(action, (error: unknown) => error instanceof StandardContractError && error.code === code);
}

function definition(family: 'exact' | 'continuous') {
  return family === 'exact'
    ? { family, roles: [], semanticsVersion: 1, source: 'rules', subject: 'formula', tree: literal(family) }
    : { family, roles: [], semanticsVersion: 1, source: 'rules', subject: 'formula', tree: literal(family) };
}
function literal(family: 'exact' | 'continuous') { return family === 'exact' ? { op: 'literal' as const, value: 1 } : { op: 'literal' as const, bits: '3ff0000000000000' }; }
function chain(family: 'exact' | 'continuous', depth: number): unknown {
  let tree: unknown = literal(family);
  for (let index = 1; index < depth; index += 1) tree = { op: 'add', left: tree, right: literal(family) };
  return tree;
}
function balanced(family: 'exact' | 'continuous', leaves: number): unknown {
  let values: unknown[] = Array.from({ length: leaves }, () => literal(family));
  while (values.length > 1) {
    const next: unknown[] = [];
    for (let index = 0; index < values.length; index += 2) next.push(index + 1 === values.length ? values[index] : { op: 'add', left: values[index], right: values[index + 1] });
    values = next;
  }
  return values[0];
}
function balancedInputs(family: 'exact' | 'continuous', count: number): unknown {
  let values: unknown[] = Array.from({ length: count }, (_, index) => ({ op: 'input', input: { kind: 'parameter', role: 'self', id: `input-${index}` } }));
  while (values.length > 1) {
    const next: unknown[] = [];
    for (let index = 0; index < values.length; index += 2) next.push(index + 1 === values.length ? values[index] : { op: 'add', left: values[index], right: values[index + 1] });
    values = next;
  }
  return values[0];
}
