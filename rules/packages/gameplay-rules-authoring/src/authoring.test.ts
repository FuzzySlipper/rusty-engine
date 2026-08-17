import { strict as assert } from 'node:assert';
import { Buffer } from 'node:buffer';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';

import {
  RULE_LIMITS,
  RULE_PACKAGE_BINARY64_SCHEMA_VERSION,
  RuleContractError,
  decodeRulePackage,
  type JsonValue,
  type RuleContractErrorCode,
} from '@rusty-engine/gameplay-rules-contracts';

import {
  authorRulePackage,
  authorBinary64RulePackage,
  canonicalRuleArtifactBytes,
} from './index.js';

const fixtureUrl = new URL(
  '../../../../fixtures/gameplay-rules/package-v1.canonical.json',
  import.meta.url,
);
const unicodeFixtureUrl = new URL(
  '../../../../fixtures/gameplay-rules/package-v1-unicode.canonical.json',
  import.meta.url,
);
const binary64FixtureUrl = new URL(
  '../../../../fixtures/gameplay-rules/package-v2-binary64.canonical.json',
  import.meta.url,
);
const fixtureFingerprint =
  '8ef484b4505310b757c59133985608c29d38b421e02488797cf7df9a999d57b2';

type MachinePayload = {
  readonly machines: readonly {
    readonly id: string;
    readonly output: number;
  }[];
  readonly name: string;
};

test('ordinary typed authoring emits the exact Rust-decoded golden artifact', async () => {
  const machines: { id: string; output: number }[] = [];
  for (const [id, output] of [['alpha', 10] as const]) {
    machines.push({ id, output });
  }
  const payload: MachinePayload = { name: 'Fixture', machines };
  const artifact = authorRulePackage<MachinePayload>({
    domain: 'fixture',
    package: 'core',
    version: 1,
    sources: [{ id: 'rules', path: 'rules/core.ts' }],
    provenance: [
      {
        subject: `machine.${machines[0]?.id ?? 'missing'}`,
        source: 'rules',
        line: 4,
        column: 3,
      },
    ],
    payload,
  });
  const fixture = await readFile(fixtureUrl);

  assert.equal(artifact.canonicalJson, fixture.toString('utf8'));
  assert.equal(artifact.fingerprint, fixtureFingerprint);
  assert.deepEqual(Buffer.from(canonicalRuleArtifactBytes(artifact)), fixture);
  const decoded = decodeRulePackage(canonicalRuleArtifactBytes(artifact));
  assert.deepEqual(decoded, artifact.package);
  assert.ok(Object.isFrozen(artifact.package));
  assert.ok(Object.isFrozen(artifact.package.payload));
});

test('binary64 authoring emits the exact Rust-owned float fixture', async () => {
  const artifact = authorBinary64RulePackage({
    domain: 'fixture',
    package: 'binary64',
    version: 1,
    payload: {
      values: [-0, 1.0, 1.5, 1e-6, 1e20, 1e21, 5e-324, Number.MAX_VALUE],
    },
  });
  const fixture = await readFile(binary64FixtureUrl);
  assert.equal(artifact.package.schemaVersion, RULE_PACKAGE_BINARY64_SCHEMA_VERSION);
  assert.equal(artifact.canonicalJson, fixture.toString('utf8'));
  assert.deepEqual(Buffer.from(canonicalRuleArtifactBytes(artifact)), fixture);
  assert.deepEqual(decodeRulePackage(fixture), artifact.package);
});

test('authoring normalizes all unordered inputs without reordering payload arrays', () => {
  const artifact = authorRulePackage({
    domain: 'fixture',
    package: 'ordered',
    version: 1,
    dependencies: [
      { domain: 'z', package: 'core', version: 1 },
      { domain: 'a', package: 'core', version: 1 },
    ],
    sources: [
      { id: 'z', path: 'rules/z.ts' },
      { id: 'a', path: 'rules/a.ts' },
    ],
    provenance: [
      { subject: 'z.subject', source: 'z' },
      { subject: 'a.subject', source: 'a' },
    ],
    payload: {
      z: 1,
      a: 2,
      sequence: ['z', 'a'],
    },
  });

  assert.deepEqual(
    artifact.package.dependencies.map((entry) => entry.domain),
    ['a', 'z'],
  );
  assert.deepEqual(
    artifact.package.sources.map((entry) => entry.id),
    ['a', 'z'],
  );
  assert.deepEqual(
    artifact.package.provenance.map((entry) => entry.subject),
    ['a.subject', 'z.subject'],
  );
  assert.match(
    artifact.canonicalJson,
    /"payload":\{"a":2,"sequence":\["z","a"\],"z":1\}\}\n$/,
  );
});

test('authoring persists plain data only', () => {
  expectPayloadError(() => () => 1, 'noncanonical-value');
  expectPayloadError(() => undefined, 'noncanonical-value');
  expectPayloadError(() => 1n, 'noncanonical-value');
  expectPayloadError(() => Number.NaN, 'json-number-out-of-range');
  expectPayloadError(() => 1.5, 'json-integer-out-of-range');

  const cyclic: Record<string, unknown> = {};
  cyclic['self'] = cyclic;
  expectPayloadError(() => cyclic, 'noncanonical-value');

  class SemanticClass {
    public readonly value = 1;
  }
  expectPayloadError(() => new SemanticClass(), 'noncanonical-value');

  const accessor = {};
  Object.defineProperty(accessor, 'value', {
    enumerable: true,
    get: () => 1,
  });
  expectPayloadError(() => accessor, 'noncanonical-value');
});

test('Unicode scalar ordering and JSON escaping match the Rust canonical fixture', async () => {
  const artifact = authorRulePackage({
    domain: 'fixture',
    package: 'unicode',
    version: 1,
    payload: {
      中: ['é', 'a'],
      é: 'snowman ☃',
      a: 'line\nquote"slash\\\u0001',
    },
  });
  const fixture = await readFile(unicodeFixtureUrl);

  assert.equal(artifact.canonicalJson, fixture.toString('utf8'));
  assert.deepEqual(Buffer.from(canonicalRuleArtifactBytes(artifact)), fixture);
  assert.deepEqual(decodeRulePackage(fixture), artifact.package);
});

test('canonical emission enforces the exact artifact byte boundary', () => {
  const stringLimit = RULE_LIMITS.maxJsonStringBytes;
  const fixed = 'x'.repeat(stringLimit);
  const base = authorLargeArtifact(fixed, fixed, fixed, '');
  const remaining =
    RULE_LIMITS.maxEncodedRulePackageBytes -
    Buffer.byteLength(base.canonicalJson, 'utf8');
  assert.ok(remaining > 0);
  assert.ok(remaining <= stringLimit);

  const exact = authorLargeArtifact(
    fixed,
    fixed,
    fixed,
    'x'.repeat(remaining),
  );
  assert.equal(
    Buffer.byteLength(exact.canonicalJson, 'utf8'),
    RULE_LIMITS.maxEncodedRulePackageBytes,
  );
  expectRuleError(
    () =>
      authorLargeArtifact(
        fixed,
        fixed,
        fixed,
        'x'.repeat(remaining + 1),
      ),
    'artifact-quota-exceeded',
  );
});

function authorLargeArtifact(
  a: string,
  b: string,
  c: string,
  d: string,
) {
  return authorRulePackage({
    domain: 'fixture',
    package: 'large',
    version: 1,
    payload: { a, b, c, d },
  });
}

function expectPayloadError(
  makePayload: () => unknown,
  code: RuleContractErrorCode,
): void {
  expectRuleError(
    () =>
      authorRulePackage({
        domain: 'fixture',
        package: 'invalid',
        version: 1,
        payload: makePayload() as JsonValue,
      }),
    code,
  );
}

function expectRuleError(
  action: () => unknown,
  code: RuleContractErrorCode,
): void {
  assert.throws(action, (error: unknown) => {
    assert.ok(error instanceof RuleContractError);
    assert.equal(error.code, code);
    return true;
  });
}
