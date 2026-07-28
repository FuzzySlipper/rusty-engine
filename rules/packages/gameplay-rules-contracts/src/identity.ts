import { Buffer } from 'node:buffer';

import {
  RULE_LIMITS,
  type RuleDomainId,
  type RuleFingerprint,
  type RulePackageId,
  type RuleSourceId,
  type RuleSubjectId,
} from './generated.js';
import { RuleContractError } from './error.js';

export function parseRuleDomainId(
  value: string,
  logicalPath = 'domain',
): RuleDomainId {
  return parseIdentity(value, logicalPath) as RuleDomainId;
}

export function parseRulePackageId(
  value: string,
  logicalPath = 'package',
): RulePackageId {
  return parseIdentity(value, logicalPath) as RulePackageId;
}

export function parseRuleSourceId(
  value: string,
  logicalPath = 'source',
): RuleSourceId {
  return parseIdentity(value, logicalPath) as RuleSourceId;
}

export function parseRuleSubjectId(
  value: string,
  logicalPath = 'subject',
): RuleSubjectId {
  return parseIdentity(value, logicalPath) as RuleSubjectId;
}

export function parseRuleFingerprint(
  value: string,
  logicalPath = 'fingerprint',
): RuleFingerprint {
  if (!/^[0-9a-f]{64}$/.test(value)) {
    throw new RuleContractError(
      'invalid-fingerprint',
      logicalPath,
      'fingerprint must be 64 lowercase hexadecimal characters',
      { value },
    );
  }
  return value as RuleFingerprint;
}

export function parseRuleVersion(
  value: number,
  logicalPath = 'version',
): number {
  if (
    !Number.isSafeInteger(value) ||
    value <= 0 ||
    value > RULE_LIMITS.maxSafeJsonInteger
  ) {
    throw new RuleContractError(
      'invalid-version',
      logicalPath,
      'version must be a positive JavaScript-safe integer',
      { value: String(value) },
    );
  }
  return value;
}

export function parseSourceLocation(
  value: number,
  logicalPath: string,
): number {
  if (
    !Number.isSafeInteger(value) ||
    value <= 0 ||
    value > RULE_LIMITS.maxSafeJsonInteger
  ) {
    throw new RuleContractError(
      'invalid-source-location',
      logicalPath,
      'source location must be a positive JavaScript-safe integer',
      { value: String(value) },
    );
  }
  return value;
}

export function parseSourcePath(
  value: string,
  logicalPath = 'source.path',
): string {
  const bytes = utf8Length(value);
  if (value.length === 0) {
    throw new RuleContractError(
      'invalid-source-path',
      logicalPath,
      'source path is empty',
    );
  }
  if (bytes > RULE_LIMITS.maxSourcePathBytes) {
    throw new RuleContractError(
      'quota-exceeded',
      logicalPath,
      'source path exceeds its UTF-8 byte limit',
      { actual: bytes, maximum: RULE_LIMITS.maxSourcePathBytes },
    );
  }
  if (value.trim() !== value) {
    throw new RuleContractError(
      'invalid-source-path',
      logicalPath,
      'source path has leading or trailing whitespace',
    );
  }
  assertUnicodeScalars(value, logicalPath);
  if (hasControlCharacter(value)) {
    throw new RuleContractError(
      'invalid-source-path',
      logicalPath,
      'source path contains a control character',
    );
  }
  return value;
}

export function assertUnicodeScalars(
  value: string,
  logicalPath: string,
): void {
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (code >= 0xd800 && code <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (next < 0xdc00 || next > 0xdfff) {
        throw new RuleContractError(
          'noncanonical-value',
          logicalPath,
          'string contains an unpaired high surrogate',
        );
      }
      index += 1;
    } else if (code >= 0xdc00 && code <= 0xdfff) {
      throw new RuleContractError(
        'noncanonical-value',
        logicalPath,
        'string contains an unpaired low surrogate',
      );
    }
  }
}

export function utf8Length(value: string): number {
  return Buffer.byteLength(value, 'utf8');
}

function parseIdentity(value: string, logicalPath: string): string {
  const bytes = utf8Length(value);
  if (value.length === 0) {
    throw new RuleContractError(
      'invalid-identity',
      logicalPath,
      'identity is empty',
      { value },
    );
  }
  if (bytes > RULE_LIMITS.maxRuleIdBytes) {
    throw new RuleContractError(
      'invalid-identity',
      logicalPath,
      'identity exceeds its byte limit',
      { value },
    );
  }
  if (value.trim() !== value) {
    throw new RuleContractError(
      'invalid-identity',
      logicalPath,
      'identity has leading or trailing whitespace',
      { value },
    );
  }
  if (!/^[\x20-\x7e]+$/.test(value)) {
    throw new RuleContractError(
      'invalid-identity',
      logicalPath,
      'identity must contain printable ASCII only',
      { value },
    );
  }
  return value;
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
