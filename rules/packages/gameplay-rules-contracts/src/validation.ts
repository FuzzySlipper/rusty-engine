import { Buffer } from 'node:buffer';

import {
  RULE_LIMITS,
  RULE_PACKAGE_ARTIFACT_KIND,
  RULE_PACKAGE_SCHEMA_VERSION,
  type JsonValue,
  type RulePackage,
  type RulePackageDependency,
  type RuleProvenance,
  type RuleSource,
} from './generated.js';
import { RuleContractError } from './error.js';
import {
  assertUnicodeScalars,
  parseRuleDomainId,
  parseRuleFingerprint,
  parseRulePackageId,
  parseRuleSourceId,
  parseRuleSubjectId,
  parseRuleVersion,
  parseSourceLocation,
  parseSourcePath,
  utf8Length,
} from './identity.js';
import { pointerIndex, pointerKey } from './json.js';

export function admitRulePackageValue<Payload extends JsonValue = JsonValue>(
  value: unknown,
): RulePackage<Payload> {
  const root = requireRecord(value, '$');
  ensureKnownFields(
    root,
    [
      'kind',
      'schemaVersion',
      'domain',
      'package',
      'version',
      'dependencies',
      'sources',
      'provenance',
      'payload',
    ],
    '$',
  );

  const kind = requireString(required(root, 'kind', '$'), '$/kind');
  if (kind !== RULE_PACKAGE_ARTIFACT_KIND) {
    throw new RuleContractError(
      'wrong-artifact-kind',
      '$/kind',
      `expected artifact kind ${RULE_PACKAGE_ARTIFACT_KIND}`,
      { actual: kind },
    );
  }
  const schemaVersion = requireNumber(
    required(root, 'schemaVersion', '$'),
    '$/schemaVersion',
  );
  if (schemaVersion !== RULE_PACKAGE_SCHEMA_VERSION) {
    throw new RuleContractError(
      'unsupported-schema-version',
      '$/schemaVersion',
      `unsupported gameplay-rules schema version ${String(schemaVersion)}`,
      { actual: String(schemaVersion) },
    );
  }

  const domain = parseRuleDomainId(
    requireString(required(root, 'domain', '$'), '$/domain'),
    '$/domain',
  );
  const packageId = parseRulePackageId(
    requireString(required(root, 'package', '$'), '$/package'),
    '$/package',
  );
  const version = parseRuleVersion(
    requireNumber(required(root, 'version', '$'), '$/version'),
    '$/version',
  );

  const rawDependencies = requireArray(
    required(root, 'dependencies', '$'),
    '$/dependencies',
    RULE_LIMITS.maxDependenciesPerRulePackage,
  );
  enforceCollectionQuota(
    '$/dependencies',
    rawDependencies.length,
    RULE_LIMITS.maxDependenciesPerRulePackage,
  );
  const dependencies = rawDependencies.map((dependency, index) =>
    admitDependency(dependency, pointerIndex('$/dependencies', index)),
  );
  dependencies.sort(compareDependencies);
  for (let index = 1; index < dependencies.length; index += 1) {
    const left = dependencies[index - 1] as RulePackageDependency;
    const right = dependencies[index] as RulePackageDependency;
    if (
      left.domain === right.domain &&
      left.package === right.package &&
      left.version === right.version
    ) {
      throw new RuleContractError(
        'duplicate-dependency',
        `$/dependencies/${String(index)}`,
        'package dependencies must be unique by exact identity',
      );
    }
  }
  const self = dependencies.find(
    (dependency) =>
      dependency.domain === domain && dependency.package === packageId,
  );
  if (self !== undefined) {
    throw new RuleContractError(
      'self-dependency',
      '$/dependencies',
      'a package cannot depend on its own logical identity',
    );
  }

  const rawSources = requireArray(
    required(root, 'sources', '$'),
    '$/sources',
    RULE_LIMITS.maxSourcesPerRulePackage,
  );
  enforceCollectionQuota(
    '$/sources',
    rawSources.length,
    RULE_LIMITS.maxSourcesPerRulePackage,
  );
  const sources = rawSources.map((source, index) =>
    admitSource(source, pointerIndex('$/sources', index)),
  );
  sources.sort((left, right) => compareText(left.id, right.id));
  for (let index = 1; index < sources.length; index += 1) {
    if (sources[index - 1]?.id === sources[index]?.id) {
      throw new RuleContractError(
        'duplicate-source',
        `$/sources/${String(index)}`,
        'source identities must be unique',
      );
    }
  }

  const rawProvenance = requireArray(
    required(root, 'provenance', '$'),
    '$/provenance',
    RULE_LIMITS.maxProvenancePerRulePackage,
  );
  enforceCollectionQuota(
    '$/provenance',
    rawProvenance.length,
    RULE_LIMITS.maxProvenancePerRulePackage,
  );
  const provenance = rawProvenance.map((entry, index) =>
    admitProvenance(entry, pointerIndex('$/provenance', index)),
  );
  provenance.sort((left, right) => compareText(left.subject, right.subject));
  for (let index = 1; index < provenance.length; index += 1) {
    if (provenance[index - 1]?.subject === provenance[index]?.subject) {
      throw new RuleContractError(
        'duplicate-provenance',
        `$/provenance/${String(index)}`,
        'provenance subjects must be unique',
      );
    }
  }
  const sourceIds = new Set(sources.map((source) => source.id));
  for (let index = 0; index < provenance.length; index += 1) {
    const entry = provenance[index] as RuleProvenance;
    if (!sourceIds.has(entry.source)) {
      throw new RuleContractError(
        'unknown-provenance-source',
        `$/provenance/${String(index)}/source`,
        'provenance references an unknown source identity',
        { source: entry.source },
      );
    }
  }

  const budget = new JsonBudget();
  addEnvelopeNodes(budget, dependencies, sources, provenance);
  const payload = normalizeJsonValue(
    required(root, 'payload', '$'),
    2,
    '$/payload',
    budget,
    new WeakSet<object>(),
  ) as Payload;

  return Object.freeze({
    kind: RULE_PACKAGE_ARTIFACT_KIND,
    schemaVersion: RULE_PACKAGE_SCHEMA_VERSION,
    domain,
    package: packageId,
    version,
    dependencies: Object.freeze(dependencies),
    sources: Object.freeze(sources),
    provenance: Object.freeze(provenance),
    payload,
  });
}

export function admitRulePackage<Payload extends JsonValue>(
  packageValue: RulePackage<Payload>,
): RulePackage<Payload> {
  return admitRulePackageValue<Payload>(packageValue);
}

export function normalizeJsonValue(
  value: unknown,
  depth = 1,
  logicalPath = '$',
  budget = new JsonBudget(),
  active = new WeakSet<object>(),
): JsonValue {
  if (depth > RULE_LIMITS.maxJsonNestingDepth) {
    throw new RuleContractError(
      'json-depth-exceeded',
      logicalPath,
      'JSON nesting depth exceeds the package limit',
      {
        actual: depth,
        maximum: RULE_LIMITS.maxJsonNestingDepth,
      },
    );
  }
  budget.add(logicalPath);

  if (value === null || typeof value === 'boolean') return value;
  if (typeof value === 'number') {
    if (
      !Number.isSafeInteger(value) ||
      Math.abs(value) > RULE_LIMITS.maxSafeJsonInteger
    ) {
      throw new RuleContractError(
        'json-integer-out-of-range',
        logicalPath,
        'JSON values permit only portable safe integers',
        { value: String(value) },
      );
    }
    return Object.is(value, -0) ? 0 : value;
  }
  if (typeof value === 'string') {
    assertUnicodeScalars(value, logicalPath);
    const bytes = utf8Length(value);
    if (bytes > RULE_LIMITS.maxJsonStringBytes) {
      throw new RuleContractError(
        'quota-exceeded',
        logicalPath,
        'JSON string exceeds its UTF-8 byte limit',
        { actual: bytes, maximum: RULE_LIMITS.maxJsonStringBytes },
      );
    }
    return value;
  }
  if (typeof value !== 'object') {
    throw new RuleContractError(
      'noncanonical-value',
      logicalPath,
      `value of type ${typeof value} cannot be persisted as JSON`,
    );
  }
  if (active.has(value)) {
    throw new RuleContractError(
      'noncanonical-value',
      logicalPath,
      'cyclic object graphs cannot be persisted as JSON',
    );
  }
  active.add(value);
  try {
    if (Array.isArray(value)) {
      budget.preflight(value.length, logicalPath);
      validateArrayShape(value, logicalPath);
      return Object.freeze(
        value.map((entry, index) =>
          normalizeJsonValue(
            entry,
            depth + 1,
            pointerIndex(logicalPath, index),
            budget,
            active,
          ),
        ),
      );
    }

    const prototype = Object.getPrototypeOf(value);
    if (prototype !== Object.prototype && prototype !== null) {
      throw new RuleContractError(
        'noncanonical-value',
        logicalPath,
        'JSON objects must be plain data objects',
      );
    }
    const descriptors = Object.getOwnPropertyDescriptors(value);
    const keys = Reflect.ownKeys(descriptors);
    if (keys.some((key) => typeof key === 'symbol')) {
      throw new RuleContractError(
        'noncanonical-value',
        logicalPath,
        'JSON objects cannot contain symbol keys',
      );
    }
    const stringKeys = (keys as string[]).sort(compareUtf8);
    const normalized: Record<string, JsonValue> = {};
    for (const key of stringKeys) {
      assertUnicodeScalars(key, `${logicalPath}/<key>`);
      const keyBytes = utf8Length(key);
      if (keyBytes > RULE_LIMITS.maxJsonStringBytes) {
        throw new RuleContractError(
          'quota-exceeded',
          `${logicalPath}/<key>`,
          'JSON object key exceeds its UTF-8 byte limit',
          { actual: keyBytes, maximum: RULE_LIMITS.maxJsonStringBytes },
        );
      }
      const descriptor = descriptors[key];
      if (
        descriptor === undefined ||
        !descriptor.enumerable ||
        !('value' in descriptor)
      ) {
        throw new RuleContractError(
          'noncanonical-value',
          pointerKey(logicalPath, key),
          'JSON objects cannot contain accessors or non-enumerable fields',
        );
      }
      Object.defineProperty(normalized, key, {
        value: normalizeJsonValue(
          descriptor.value,
          depth + 1,
          pointerKey(logicalPath, key),
          budget,
          active,
        ),
        enumerable: true,
        configurable: false,
        writable: false,
      });
    }
    return Object.freeze(normalized);
  } finally {
    active.delete(value);
  }
}

function admitDependency(
  value: unknown,
  path: string,
): RulePackageDependency {
  const record = requireRecord(value, path);
  ensureKnownFields(record, ['domain', 'package', 'version', 'fingerprint'], path);
  const fingerprintValue = record['fingerprint'];
  const base = {
    domain: parseRuleDomainId(
      requireString(required(record, 'domain', path), `${path}/domain`),
      `${path}/domain`,
    ),
    package: parseRulePackageId(
      requireString(required(record, 'package', path), `${path}/package`),
      `${path}/package`,
    ),
    version: parseRuleVersion(
      requireNumber(required(record, 'version', path), `${path}/version`),
      `${path}/version`,
    ),
  };
  return Object.freeze(
    fingerprintValue === undefined
      ? base
      : {
          ...base,
          fingerprint: parseRuleFingerprint(
            requireString(fingerprintValue, `${path}/fingerprint`),
            `${path}/fingerprint`,
          ),
        },
  );
}

function admitSource(value: unknown, path: string): RuleSource {
  const record = requireRecord(value, path);
  ensureKnownFields(record, ['id', 'path'], path);
  return Object.freeze({
    id: parseRuleSourceId(
      requireString(required(record, 'id', path), `${path}/id`),
      `${path}/id`,
    ),
    path: parseSourcePath(
      requireString(required(record, 'path', path), `${path}/path`),
      `${path}/path`,
    ),
  });
}

function admitProvenance(value: unknown, path: string): RuleProvenance {
  const record = requireRecord(value, path);
  ensureKnownFields(record, ['subject', 'source', 'line', 'column'], path);
  const lineValue = record['line'];
  const columnValue = record['column'];
  const base = {
    subject: parseRuleSubjectId(
      requireString(required(record, 'subject', path), `${path}/subject`),
      `${path}/subject`,
    ),
    source: parseRuleSourceId(
      requireString(required(record, 'source', path), `${path}/source`),
      `${path}/source`,
    ),
  };
  return Object.freeze({
    ...base,
    ...(lineValue === undefined
      ? {}
      : {
          line: parseSourceLocation(
            requireNumber(lineValue, `${path}/line`),
            `${path}/line`,
          ),
        }),
    ...(columnValue === undefined
      ? {}
      : {
          column: parseSourceLocation(
            requireNumber(columnValue, `${path}/column`),
            `${path}/column`,
          ),
        }),
  });
}

function addEnvelopeNodes(
  budget: JsonBudget,
  dependencies: readonly RulePackageDependency[],
  sources: readonly RuleSource[],
  provenance: readonly RuleProvenance[],
): void {
  budget.add('$');
  for (const path of [
    '$/kind',
    '$/schemaVersion',
    '$/domain',
    '$/package',
    '$/version',
  ]) {
    budget.add(path);
  }
  budget.add('$/dependencies');
  dependencies.forEach((dependency, index) => {
    const path = `$/dependencies/${String(index)}`;
    budget.add(path);
    budget.add(`${path}/domain`);
    budget.add(`${path}/package`);
    budget.add(`${path}/version`);
    if (dependency.fingerprint !== undefined) budget.add(`${path}/fingerprint`);
  });
  budget.add('$/sources');
  sources.forEach((_, index) => {
    const path = `$/sources/${String(index)}`;
    budget.add(path);
    budget.add(`${path}/id`);
    budget.add(`${path}/path`);
  });
  budget.add('$/provenance');
  provenance.forEach((entry, index) => {
    const path = `$/provenance/${String(index)}`;
    budget.add(path);
    budget.add(`${path}/subject`);
    budget.add(`${path}/source`);
    if (entry.line !== undefined) budget.add(`${path}/line`);
    if (entry.column !== undefined) budget.add(`${path}/column`);
  });
}

class JsonBudget {
  private nodes = 0;

  public add(path: string): void {
    this.nodes += 1;
    if (this.nodes > RULE_LIMITS.maxJsonNodesPerRulePackage) {
      throw new RuleContractError(
        'json-node-quota-exceeded',
        path,
        'JSON node count exceeds the package limit',
        {
          actual: this.nodes,
          maximum: RULE_LIMITS.maxJsonNodesPerRulePackage,
        },
      );
    }
  }

  public preflight(additional: number, path: string): void {
    if (
      additional >
      RULE_LIMITS.maxJsonNodesPerRulePackage - this.nodes
    ) {
      throw new RuleContractError(
        'json-node-quota-exceeded',
        path,
        'JSON node count exceeds the package limit',
        {
          actual: this.nodes + additional,
          maximum: RULE_LIMITS.maxJsonNodesPerRulePackage,
        },
      );
    }
  }
}

function requireRecord(
  value: unknown,
  path: string,
): Readonly<Record<string, unknown>> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new RuleContractError(
      'invalid-field-type',
      path,
      'expected an object',
    );
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new RuleContractError(
      'noncanonical-value',
      path,
      'JSON objects must be plain data objects',
    );
  }
  const descriptors = Object.getOwnPropertyDescriptors(value);
  for (const key of Reflect.ownKeys(descriptors)) {
    if (typeof key === 'symbol') {
      throw new RuleContractError(
        'noncanonical-value',
        path,
        'JSON objects cannot contain symbol keys',
      );
    }
    const descriptor = descriptors[key];
    if (
      descriptor === undefined ||
      !descriptor.enumerable ||
      !('value' in descriptor)
    ) {
      throw new RuleContractError(
        'noncanonical-value',
        pointerKey(path, key),
        'JSON objects cannot contain accessors or non-enumerable fields',
      );
    }
  }
  return value as Readonly<Record<string, unknown>>;
}

function requireArray(
  value: unknown,
  path: string,
  maximum: number,
): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw new RuleContractError(
      'invalid-field-type',
      path,
      'expected an array',
    );
  }
  enforceCollectionQuota(path, value.length, maximum);
  validateArrayShape(value, path);
  return value;
}

function requireString(value: unknown, path: string): string {
  if (typeof value !== 'string') {
    throw new RuleContractError(
      'invalid-field-type',
      path,
      'expected a string',
    );
  }
  return value;
}

function requireNumber(value: unknown, path: string): number {
  if (typeof value !== 'number') {
    throw new RuleContractError(
      'invalid-field-type',
      path,
      'expected a number',
    );
  }
  return value;
}

function required(
  record: Readonly<Record<string, unknown>>,
  field: string,
  path: string,
): unknown {
  if (!Object.hasOwn(record, field)) {
    throw new RuleContractError(
      'missing-field',
      `${path}/${field}`,
      `required field ${field} is missing`,
    );
  }
  return record[field];
}

function ensureKnownFields(
  record: Readonly<Record<string, unknown>>,
  expected: readonly string[],
  path: string,
): void {
  const allowed = new Set(expected);
  const unknown = Object.keys(record).sort(compareUtf8).find((key) => !allowed.has(key));
  if (unknown !== undefined) {
    throw new RuleContractError(
      'unknown-field',
      `${path}/${unknown}`,
      `unknown field ${unknown}`,
    );
  }
}

function enforceCollectionQuota(
  path: string,
  actual: number,
  maximum: number,
): void {
  if (actual > maximum) {
    throw new RuleContractError(
      'quota-exceeded',
      path,
      'collection exceeds its item limit',
      { actual, maximum },
    );
  }
}

function compareDependencies(
  left: RulePackageDependency,
  right: RulePackageDependency,
): number {
  return (
    compareText(left.domain, right.domain) ||
    compareText(left.package, right.package) ||
    left.version - right.version
  );
}

function compareText(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}

function compareUtf8(left: string, right: string): number {
  return Buffer.compare(Buffer.from(left, 'utf8'), Buffer.from(right, 'utf8'));
}

function validateArrayShape(value: readonly unknown[], path: string): void {
  const descriptors = Object.getOwnPropertyDescriptors(value);
  const keys = Reflect.ownKeys(descriptors);
  if (
    keys.length !== value.length + 1 ||
    keys.some((key) => {
      if (key === 'length') return false;
      if (!isCanonicalArrayIndex(key, value.length)) return true;
      const descriptor = descriptors[key];
      return (
        descriptor === undefined ||
        !descriptor.enumerable ||
        !('value' in descriptor)
      );
    })
  ) {
    throw new RuleContractError(
      'noncanonical-value',
      path,
      'JSON arrays must be dense and cannot contain custom properties',
    );
  }
}

function isCanonicalArrayIndex(
  key: string | symbol,
  length: number,
): key is string {
  if (typeof key !== 'string' || !/^(?:0|[1-9][0-9]*)$/.test(key)) {
    return false;
  }
  const index = Number(key);
  return Number.isSafeInteger(index) && index < length;
}
