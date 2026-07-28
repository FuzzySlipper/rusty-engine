import { Buffer } from 'node:buffer';

import {
  RULE_LIMITS,
  type RuleDiagnostic,
  type RuleDiagnosticCorrelation,
  type RuleDiagnosticSeverity,
  type RulePackageIdentity,
} from './generated.js';
import { RuleContractError } from './error.js';
import {
  assertUnicodeScalars,
  parseRuleDomainId,
  parseRulePackageId,
  parseRuleSourceId,
  parseRuleSubjectId,
  parseRuleVersion,
  parseSourceLocation,
} from './identity.js';

export interface RuleDiagnosticCorrelationInput {
  readonly subject: string;
  readonly source: string;
  readonly line?: number;
  readonly column?: number;
}

export interface RulePackageIdentityInput {
  readonly domain: string;
  readonly package: string;
  readonly version: number;
}

export interface RuleDiagnosticInput {
  readonly code: string;
  readonly severity: RuleDiagnosticSeverity;
  readonly logicalPath: string;
  readonly message: string;
  readonly package?: RulePackageIdentityInput;
  readonly correlation?: RuleDiagnosticCorrelationInput;
}

export function admitRuleDiagnostics(
  inputs: readonly RuleDiagnosticInput[],
): readonly RuleDiagnostic[] {
  validateInputArray(inputs, '$/diagnostics');
  if (inputs.length > RULE_LIMITS.maxRuleDiagnostics) {
    throw new RuleContractError(
      'quota-exceeded',
      '$/diagnostics',
      'diagnostic report exceeds its item limit',
      {
        actual: inputs.length,
        maximum: RULE_LIMITS.maxRuleDiagnostics,
      },
    );
  }
  const diagnostics = inputs.map((input, index) =>
    admitDiagnostic(input, `$/diagnostics/${String(index)}`),
  );
  diagnostics.sort(compareDiagnostics);
  return Object.freeze(diagnostics);
}

function admitDiagnostic(
  input: RuleDiagnosticInput,
  path: string,
): RuleDiagnostic {
  const record = requireDiagnosticRecord(
    input,
    path,
    ['code', 'severity', 'logicalPath', 'message', 'package', 'correlation'],
  );
  const code = requireText(record, 'code', path);
  const logicalPath = requireText(record, 'logicalPath', path);
  const message = requireText(record, 'message', path);
  validateBoundedText(
    code,
    `${path}/code`,
    RULE_LIMITS.maxDiagnosticCodeBytes,
    true,
  );
  const severity = record['severity'];
  if (severity !== 'error' && severity !== 'warning') {
    throw new RuleContractError(
      'diagnostic-invalid',
      `${path}/severity`,
      'diagnostic severity must be error or warning',
    );
  }
  validateBoundedText(
    logicalPath,
    `${path}/logicalPath`,
    RULE_LIMITS.maxDiagnosticLogicalPathBytes,
    false,
  );
  validateBoundedText(
    message,
    `${path}/message`,
    RULE_LIMITS.maxDiagnosticMessageBytes,
    'message',
  );

  const packageInput = record['package'];
  const packageIdentity =
    packageInput === undefined
      ? undefined
      : admitPackageIdentity(packageInput, `${path}/package`);
  const correlationInput = record['correlation'];
  const correlation =
    correlationInput === undefined
      ? undefined
      : admitCorrelation(correlationInput, `${path}/correlation`);
  return Object.freeze({
    code,
    severity,
    logicalPath,
    message,
    ...(packageIdentity === undefined ? {} : { package: packageIdentity }),
    ...(correlation === undefined ? {} : { correlation }),
  });
}

function admitPackageIdentity(
  input: unknown,
  path: string,
): RulePackageIdentity {
  const record = requireDiagnosticRecord(
    input,
    path,
    ['domain', 'package', 'version'],
  );
  return Object.freeze({
    domain: parseRuleDomainId(
      requireText(record, 'domain', path),
      `${path}/domain`,
    ),
    package: parseRulePackageId(
      requireText(record, 'package', path),
      `${path}/package`,
    ),
    version: parseRuleVersion(
      requireNumber(record, 'version', path),
      `${path}/version`,
    ),
  });
}

function admitCorrelation(
  input: unknown,
  path: string,
): RuleDiagnosticCorrelation {
  const record = requireDiagnosticRecord(
    input,
    path,
    ['subject', 'source', 'line', 'column'],
  );
  const line = optionalNumber(record, 'line', path);
  const column = optionalNumber(record, 'column', path);
  return Object.freeze({
    subject: parseRuleSubjectId(
      requireText(record, 'subject', path),
      `${path}/subject`,
    ),
    source: parseRuleSourceId(
      requireText(record, 'source', path),
      `${path}/source`,
    ),
    ...(line === undefined
      ? {}
      : {
          line: parseSourceLocation(line, `${path}/line`),
        }),
    ...(column === undefined
      ? {}
      : {
          column: parseSourceLocation(column, `${path}/column`),
        }),
  });
}

function validateBoundedText(
  value: string,
  path: string,
  maximum: number,
  kind: boolean | 'message',
): void {
  if (value.length === 0) {
    throw new RuleContractError(
      'diagnostic-invalid',
      path,
      'diagnostic text is empty',
    );
  }
  assertUnicodeScalars(value, path);
  const bytes = Buffer.byteLength(value, 'utf8');
  if (bytes > maximum) {
    throw new RuleContractError(
      'quota-exceeded',
      path,
      'diagnostic text exceeds its UTF-8 byte limit',
      { actual: bytes, maximum },
    );
  }
  if (kind !== 'message' && value.trim() !== value) {
    throw new RuleContractError(
      'diagnostic-invalid',
      path,
      'diagnostic text has leading or trailing whitespace',
    );
  }
  if (kind === true && !/^[\x20-\x7e]+$/.test(value)) {
    throw new RuleContractError(
      'diagnostic-invalid',
      path,
      'diagnostic code must contain printable ASCII only',
    );
  }
  if (kind === false && hasControlCharacter(value)) {
    throw new RuleContractError(
      'diagnostic-invalid',
      path,
      'diagnostic path or message contains a control character',
    );
  }
}

function compareDiagnostics(
  left: RuleDiagnostic,
  right: RuleDiagnostic,
): number {
  return (
    compareOptionalPackage(left.package, right.package) ||
    compareUtf8(left.logicalPath, right.logicalPath) ||
    compareUtf8(left.code, right.code) ||
    compareUtf8(left.severity, right.severity) ||
    compareUtf8(left.message, right.message) ||
    compareCorrelation(left.correlation, right.correlation)
  );
}

function compareOptionalPackage(
  left: RulePackageIdentity | undefined,
  right: RulePackageIdentity | undefined,
): number {
  if (left === undefined) return right === undefined ? 0 : -1;
  if (right === undefined) return 1;
  return (
    compareUtf8(left.domain, right.domain) ||
    compareUtf8(left.package, right.package) ||
    left.version - right.version
  );
}

function compareCorrelation(
  left: RuleDiagnosticCorrelation | undefined,
  right: RuleDiagnosticCorrelation | undefined,
): number {
  if (left === undefined) return right === undefined ? 0 : -1;
  if (right === undefined) return 1;
  return (
    compareUtf8(left.subject, right.subject) ||
    compareUtf8(left.source, right.source) ||
    (left.line ?? 0) - (right.line ?? 0) ||
    (left.column ?? 0) - (right.column ?? 0)
  );
}

function compareUtf8(left: string, right: string): number {
  return Buffer.compare(Buffer.from(left, 'utf8'), Buffer.from(right, 'utf8'));
}

function hasControlCharacter(value: string): boolean {
  for (const character of value) {
    const code = character.codePointAt(0);
    if (
      code !== undefined &&
      (code <= 0x1f || (code >= 0x7f && code <= 0x9f))
    ) {
      return true;
    }
  }
  return false;
}

function validateInputArray(value: unknown, path: string): void {
  if (!Array.isArray(value)) {
    throw new RuleContractError(
      'diagnostic-invalid',
      path,
      'diagnostics must be an array',
    );
  }
  const descriptors = Object.getOwnPropertyDescriptors(value);
  const keys = Reflect.ownKeys(descriptors);
  const expected = new Set<string>(['length']);
  for (let index = 0; index < value.length; index += 1) {
    expected.add(String(index));
  }
  if (
    keys.length !== expected.size ||
    keys.some((key) => typeof key !== 'string' || !expected.has(key)) ||
    Array.from({ length: value.length }, (_, index) => {
      const descriptor = descriptors[String(index)];
      return (
        descriptor === undefined ||
        !descriptor.enumerable ||
        !('value' in descriptor)
      );
    }).some(Boolean)
  ) {
    throw new RuleContractError(
      'diagnostic-invalid',
      path,
      'diagnostics must be a dense array without custom properties',
    );
  }
}

function requireDiagnosticRecord(
  value: unknown,
  path: string,
  knownFields: readonly string[],
): Readonly<Record<string, unknown>> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new RuleContractError(
      'diagnostic-invalid',
      path,
      'diagnostic value must be a plain object',
    );
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new RuleContractError(
      'diagnostic-invalid',
      path,
      'diagnostic value must be a plain object',
    );
  }
  const allowed = new Set(knownFields);
  const descriptors = Object.getOwnPropertyDescriptors(value);
  for (const key of Reflect.ownKeys(descriptors)) {
    if (typeof key !== 'string' || !allowed.has(key)) {
      throw new RuleContractError(
        'diagnostic-invalid',
        path,
        'diagnostic value contains an unknown field',
      );
    }
    const descriptor = descriptors[key];
    if (
      descriptor === undefined ||
      !descriptor.enumerable ||
      !('value' in descriptor)
    ) {
      throw new RuleContractError(
        'diagnostic-invalid',
        `${path}/${key}`,
        'diagnostic fields must be enumerable plain data',
      );
    }
  }
  return value as Readonly<Record<string, unknown>>;
}

function requireText(
  record: Readonly<Record<string, unknown>>,
  field: string,
  path: string,
): string {
  const value = record[field];
  if (typeof value !== 'string') {
    throw new RuleContractError(
      'diagnostic-invalid',
      `${path}/${field}`,
      `diagnostic field ${field} must be a string`,
    );
  }
  return value;
}

function requireNumber(
  record: Readonly<Record<string, unknown>>,
  field: string,
  path: string,
): number {
  const value = record[field];
  if (typeof value !== 'number') {
    throw new RuleContractError(
      'diagnostic-invalid',
      `${path}/${field}`,
      `diagnostic field ${field} must be a number`,
    );
  }
  return value;
}

function optionalNumber(
  record: Readonly<Record<string, unknown>>,
  field: string,
  path: string,
): number | undefined {
  return record[field] === undefined
    ? undefined
    : requireNumber(record, field, path);
}
