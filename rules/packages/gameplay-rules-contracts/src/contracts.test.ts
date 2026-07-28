import { strict as assert } from 'node:assert';
import { Buffer } from 'node:buffer';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';

import {
  RULE_LIMITS,
  RULE_PACKAGE_ARTIFACT_KIND,
  RULE_PACKAGE_SCHEMA_VERSION,
  RuleContractError,
  admitRuleDiagnostics,
  admitRulePackageValue,
  decodeRulePackage,
  type JsonValue,
  type RuleContractErrorCode,
} from './index.js';

const fixtureUrl = new URL(
  '../../../../fixtures/gameplay-rules/package-v1.canonical.json',
  import.meta.url,
);

test('the Rust-owned golden fixture decodes through the generated contract', async () => {
  const fixture = await readFile(fixtureUrl);
  const packageValue = decodeRulePackage(fixture);

  assert.equal(packageValue.domain, 'fixture');
  assert.equal(packageValue.package, 'core');
  assert.equal(packageValue.sources[0]?.path, 'rules/core.ts');
  assert.equal(packageValue.provenance[0]?.subject, 'machine.alpha');
  assert.deepEqual(packageValue.payload, {
    machines: [{ id: 'alpha', output: 10 }],
    name: 'Fixture',
  });
  assert.ok(Object.isFrozen(packageValue));
  assert.ok(Object.isFrozen(packageValue.payload));
});

test('strict decoding rejects ambiguous, malformed, and nonportable JSON', () => {
  expectRuleError(
    () =>
      decodeRulePackage(
        bytes('{"kind":"rusty.gameplay-rules.package","kind":"again"}'),
      ),
    'duplicate-json-key',
  );
  expectRuleError(
    () => decodeRulePackage(Uint8Array.from([0xef, 0xbb, 0xbf, 0x7b, 0x7d])),
    'malformed-utf8',
  );
  expectRuleError(
    () => decodeRulePackage(Uint8Array.from([0x7b, 0x22, 0xff, 0x22, 0x7d])),
    'malformed-utf8',
  );
  expectRuleError(
    () => decodeRulePackage(bytes('{"payload":"\\ud800"}')),
    'malformed-json',
  );
  expectRuleError(
    () =>
      decodeRulePackage(
        bytes(
          JSON.stringify({
            ...validPackage(),
            payload: 1.5,
          }),
        ),
      ),
    'json-integer-out-of-range',
  );
  expectRuleError(
    () =>
      decodeRulePackage(
        bytes(
          '{"kind":"rusty.gameplay-rules.package","schemaVersion":1,"domain":"fixture","package":"core","version":1,"dependencies":[],"sources":[],"provenance":[],"payload":9007199254740992}',
        ),
      ),
    'json-integer-out-of-range',
  );
});

test('identities, source metadata, versions, and schema are exact and bounded', () => {
  const exactIdentity = 'i'.repeat(RULE_LIMITS.maxRuleIdBytes);
  const exactPath = 'p'.repeat(RULE_LIMITS.maxSourcePathBytes);
  const admitted = admitRulePackageValue({
    ...validPackage(),
    domain: exactIdentity,
    sources: [{ id: 'rules', path: exactPath }],
  });
  assert.equal(admitted.domain.length, RULE_LIMITS.maxRuleIdBytes);
  assert.equal(admitted.sources[0]?.path.length, RULE_LIMITS.maxSourcePathBytes);

  expectRuleError(
    () =>
      admitRulePackageValue({
        ...validPackage(),
        domain: `${exactIdentity}x`,
      }),
    'invalid-identity',
    '$/domain',
  );
  expectRuleError(
    () =>
      admitRulePackageValue({
        ...validPackage(),
        sources: [{ id: 'rules', path: `${exactPath}x` }],
      }),
    'quota-exceeded',
    '$/sources/0/path',
  );
  expectRuleError(
    () => admitRulePackageValue({ ...validPackage(), version: 0 }),
    'invalid-version',
    '$/version',
  );
  expectRuleError(
    () => admitRulePackageValue({ ...validPackage(), schemaVersion: 2 }),
    'unsupported-schema-version',
    '$/schemaVersion',
  );
  expectRuleError(
    () => admitRulePackageValue({ ...validPackage(), extra: true }),
    'unknown-field',
    '$/extra',
  );
});

test('references and provenance reject duplicates and broken correlations', () => {
  const dependency = {
    domain: 'shared',
    package: 'core',
    version: 1,
  };
  expectRuleError(
    () =>
      admitRulePackageValue({
        ...validPackage(),
        dependencies: [dependency, dependency],
      }),
    'duplicate-dependency',
  );
  expectRuleError(
    () =>
      admitRulePackageValue({
        ...validPackage(),
        dependencies: [{ domain: 'fixture', package: 'core', version: 2 }],
      }),
    'self-dependency',
  );
  expectRuleError(
    () =>
      admitRulePackageValue({
        ...validPackage(),
        sources: [
          { id: 'rules', path: 'rules/a.ts' },
          { id: 'rules', path: 'rules/b.ts' },
        ],
      }),
    'duplicate-source',
  );
  expectRuleError(
    () =>
      admitRulePackageValue({
        ...validPackage(),
        provenance: [
          { subject: 'machine.alpha', source: 'rules' },
          { subject: 'machine.alpha', source: 'rules' },
        ],
      }),
    'duplicate-provenance',
  );
  expectRuleError(
    () =>
      admitRulePackageValue({
        ...validPackage(),
        provenance: [{ subject: 'machine.alpha', source: 'missing' }],
      }),
    'unknown-provenance-source',
  );
  expectRuleError(
    () =>
      admitRulePackageValue({
        ...validPackage(),
        dependencies: [
          { domain: ' bad', package: 'core', version: 1 },
        ],
      }),
    'invalid-identity',
    '$/dependencies/0/domain',
  );
  expectRuleError(
    () =>
      admitRulePackageValue({
        ...validPackage(),
        provenance: [
          { subject: 'machine.alpha', source: 'rules', line: 0 },
        ],
      }),
    'invalid-source-location',
    '$/provenance/0/line',
  );
});

test('collection quotas reject before parsing an over-limit item', () => {
  const dependency =
    '{"domain":"shared","package":"core","version":1}';
  const prefix =
    `{"kind":"${RULE_PACKAGE_ARTIFACT_KIND}",` +
    `"schemaVersion":${String(RULE_PACKAGE_SCHEMA_VERSION)},` +
    '"domain":"fixture","package":"core","version":1,"dependencies":[' +
    Array.from(
      { length: RULE_LIMITS.maxDependenciesPerRulePackage },
      () => dependency,
    ).join(',') +
    ',{"domain":';

  expectRuleError(
    () => decodeRulePackage(bytes(prefix)),
    'quota-exceeded',
    '$/dependencies',
  );
});

test('package collections and JSON depth and node budgets accept exact limits', () => {
  const dependencies = Array.from(
    { length: RULE_LIMITS.maxDependenciesPerRulePackage },
    (_, index) => ({
      domain: `dependency-${String(index).padStart(2, '0')}`,
      package: 'core',
      version: 1,
    }),
  );
  const sources = Array.from(
    { length: RULE_LIMITS.maxSourcesPerRulePackage },
    (_, index) => ({
      id: `source-${String(index).padStart(2, '0')}`,
      path: `rules/source-${String(index)}.ts`,
    }),
  );
  const provenance = Array.from(
    { length: RULE_LIMITS.maxProvenancePerRulePackage },
    (_, index) => ({
      subject: `subject-${String(index).padStart(4, '0')}`,
      source: 'source-00',
    }),
  );
  const exact = admitRulePackageValue({
    ...validPackage(),
    dependencies,
    sources,
    provenance,
  });
  assert.equal(
    exact.dependencies.length,
    RULE_LIMITS.maxDependenciesPerRulePackage,
  );
  assert.equal(exact.sources.length, RULE_LIMITS.maxSourcesPerRulePackage);
  assert.equal(
    exact.provenance.length,
    RULE_LIMITS.maxProvenancePerRulePackage,
  );

  expectRuleError(
    () =>
      admitRulePackageValue({
        ...validPackage(),
        sources: [...sources, { id: 'too-many', path: 'rules/extra.ts' }],
      }),
    'quota-exceeded',
    '$/sources',
  );
  expectRuleError(
    () =>
      admitRulePackageValue({
        ...validPackage(),
        sources: [sources[0]],
        provenance: [
          ...provenance,
          { subject: 'too-many', source: 'source-00' },
        ],
      }),
    'quota-exceeded',
    '$/provenance',
  );

  const exactDepth = nestedArray(62);
  assert.deepEqual(
    admitRulePackageValue({ ...validPackage(), payload: exactDepth }).payload,
    exactDepth,
  );
  expectRuleError(
    () =>
      admitRulePackageValue({
        ...validPackage(),
        payload: nestedArray(63),
      }),
    'json-depth-exceeded',
  );

  const envelopeNodes = 10;
  const exactNodePayload = Array.from(
    {
      length:
        RULE_LIMITS.maxJsonNodesPerRulePackage - envelopeNodes,
    },
    () => null,
  );
  assert.equal(
    (
      admitRulePackageValue({
        ...validPackage(),
        sources: [],
        provenance: [],
        payload: exactNodePayload,
      }).payload as readonly unknown[]
    ).length,
    exactNodePayload.length,
  );
  expectRuleError(
    () =>
      admitRulePackageValue({
        ...validPackage(),
        sources: [],
        provenance: [],
        payload: [...exactNodePayload, null],
      }),
    'json-node-quota-exceeded',
  );
});

test('direct admission rejects executable or ambiguous object shapes', () => {
  const sparse = new Array(1);
  expectRuleError(
    () => admitRulePackageValue({ ...validPackage(), dependencies: sparse }),
    'noncanonical-value',
    '$/dependencies',
  );

  const executableArray: unknown[] = [];
  Object.defineProperty(executableArray, '0', {
    enumerable: true,
    get: () => {
      throw new Error('array getter must not execute');
    },
  });
  expectRuleError(
    () =>
      admitRulePackageValue({
        ...validPackage(),
        dependencies: executableArray,
      }),
    'noncanonical-value',
    '$/dependencies',
  );

  const accessor = { ...validPackage() };
  Object.defineProperty(accessor, 'payload', {
    enumerable: true,
    get: () => ({ unsafe: true }),
  });
  expectRuleError(
    () => admitRulePackageValue(accessor),
    'noncanonical-value',
    '$/payload',
  );

  expectRuleError(
    () =>
      admitRulePackageValue({
        ...validPackage(),
        payload: 'x'.repeat(RULE_LIMITS.maxJsonStringBytes + 1),
      }),
    'quota-exceeded',
    '$/payload',
  );
  const exact = admitRulePackageValue({
    ...validPackage(),
    payload: 'x'.repeat(RULE_LIMITS.maxJsonStringBytes),
  });
  assert.equal(
    (exact.payload as string).length,
    RULE_LIMITS.maxJsonStringBytes,
  );
});

test('diagnostics are bounded, source-correlated, and deterministically ordered', () => {
  const diagnostics = admitRuleDiagnostics([
    {
      code: 'machine-output',
      severity: 'warning',
      logicalPath: '$/machines/1/output',
      message: ' line one\nline two ',
      package: { domain: 'fixture', package: 'core', version: 1 },
      correlation: {
        subject: 'machine.beta',
        source: 'rules',
        line: 8,
        column: 5,
      },
    },
    {
      code: 'machine-id',
      severity: 'error',
      logicalPath: '$/machines/0/id',
      message: 'identity is invalid',
      package: { domain: 'fixture', package: 'core', version: 1 },
      correlation: {
        subject: 'machine.alpha',
        source: 'rules',
        line: 4,
        column: 3,
      },
    },
  ]);

  assert.equal(diagnostics[0]?.code, 'machine-id');
  assert.equal(diagnostics[1]?.correlation?.line, 8);
  assert.equal(diagnostics[1]?.message, ' line one\nline two ');
  assert.ok(Object.isFrozen(diagnostics));

  const exact = admitRuleDiagnostics([
    {
      code: 'c'.repeat(RULE_LIMITS.maxDiagnosticCodeBytes),
      severity: 'error',
      logicalPath: 'p'.repeat(RULE_LIMITS.maxDiagnosticLogicalPathBytes),
      message: 'm'.repeat(RULE_LIMITS.maxDiagnosticMessageBytes),
    },
  ]);
  assert.equal(exact.length, 1);
  expectRuleError(
    () =>
      admitRuleDiagnostics([
        {
          code: `${exact[0]?.code ?? ''}x`,
          severity: 'error',
          logicalPath: '$',
          message: 'bad',
        },
      ]),
    'quota-exceeded',
  );
  expectRuleError(
    () =>
      admitRuleDiagnostics(
        Array.from(
          { length: RULE_LIMITS.maxRuleDiagnostics + 1 },
          (_, index) => ({
            code: `code-${String(index)}`,
            severity: 'warning' as const,
            logicalPath: '$',
            message: 'bounded',
          }),
        ),
      ),
    'quota-exceeded',
    '$/diagnostics',
  );
});

function validPackage(): Record<string, unknown> {
  return {
    kind: RULE_PACKAGE_ARTIFACT_KIND,
    schemaVersion: RULE_PACKAGE_SCHEMA_VERSION,
    domain: 'fixture',
    package: 'core',
    version: 1,
    dependencies: [],
    sources: [{ id: 'rules', path: 'rules/core.ts' }],
    provenance: [
      {
        subject: 'machine.alpha',
        source: 'rules',
        line: 4,
        column: 3,
      },
    ],
    payload: { machines: [{ id: 'alpha', output: 10 }], name: 'Fixture' },
  };
}

function bytes(value: string): Uint8Array {
  return Uint8Array.from(Buffer.from(value, 'utf8'));
}

function nestedArray(levels: number): JsonValue {
  let value: JsonValue = null;
  for (let index = 0; index < levels; index += 1) {
    value = [value];
  }
  return value;
}

function expectRuleError(
  action: () => unknown,
  code: RuleContractErrorCode,
  logicalPath?: string,
): void {
  assert.throws(action, (error: unknown) => {
    assert.ok(error instanceof RuleContractError);
    assert.equal(error.code, code);
    if (logicalPath !== undefined) {
      assert.equal(error.logicalPath, logicalPath);
    }
    return true;
  });
}
