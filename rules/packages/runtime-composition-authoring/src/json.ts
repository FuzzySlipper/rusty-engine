import { RuntimeCompositionAuthoringError } from './error.js';
import { PRODUCT_MODEL_LIMITS, PRODUCT_MODEL_NUMBER_ENCODING } from './generated.js';
import type { JsonValue } from './types.js';

export const MAX_OPAQUE_JSON_DEPTH = PRODUCT_MODEL_LIMITS.maximumOpaqueJsonDepth;
export const MAX_OPAQUE_JSON_NODES = PRODUCT_MODEL_LIMITS.maximumOpaqueJsonNodes;
export const MAX_OPAQUE_JSON_STRING_BYTES = PRODUCT_MODEL_LIMITS.maximumOpaqueJsonStringBytes;
export const MAX_OPAQUE_JSON_ARRAY_ENTRIES = PRODUCT_MODEL_LIMITS.maximumOpaqueJsonArrayEntries;
export const MAX_OPAQUE_JSON_OBJECT_ENTRIES = PRODUCT_MODEL_LIMITS.maximumOpaqueJsonObjectEntries;
export const MAX_SAFE_JSON_INTEGER = PRODUCT_MODEL_LIMITS.maximumSafeJsonInteger;

interface JsonBudget {
  nodes: number;
  readonly active: WeakSet<object>;
}

export function normalizeOpaqueJson(value: unknown, logicalPath: string): JsonValue {
  return normalize(value, logicalPath, 1, { nodes: 0, active: new WeakSet<object>() });
}

function normalize(
  value: unknown,
  logicalPath: string,
  depth: number,
  budget: JsonBudget,
): JsonValue {
  if (depth > MAX_OPAQUE_JSON_DEPTH) {
    throw new RuntimeCompositionAuthoringError(
      'json-depth-exceeded', logicalPath,
      `opaque JSON depth exceeds ${String(MAX_OPAQUE_JSON_DEPTH)}`,
    );
  }
  budget.nodes += 1;
  if (budget.nodes > MAX_OPAQUE_JSON_NODES) {
    throw new RuntimeCompositionAuthoringError(
      'json-node-quota-exceeded', logicalPath,
      `opaque JSON exceeds ${String(MAX_OPAQUE_JSON_NODES)} nodes`,
    );
  }
  if (value === null || typeof value === 'boolean') return value;
  if (typeof value === 'string') {
    assertString(value, logicalPath);
    return value;
  }
  if (typeof value === 'number') {
    if (!Number.isFinite(value) || (Number.isInteger(value) && Math.abs(value) > MAX_SAFE_JSON_INTEGER)) {
      throw new RuntimeCompositionAuthoringError(
        'invalid-json-value', logicalPath,
        'opaque JSON numbers must be finite and integer values must be IEEE-754 safe',
      );
    }
    return Object.is(value, -0) ? 0 : value;
  }
  if (typeof value !== 'object') {
    throw new RuntimeCompositionAuthoringError(
      'invalid-json-value', logicalPath,
      `values of type ${typeof value} cannot be materialized as JSON`,
    );
  }
  if (budget.active.has(value)) {
    throw new RuntimeCompositionAuthoringError(
      'invalid-json-value', logicalPath, 'cyclic object graphs cannot be materialized as JSON',
    );
  }
  budget.active.add(value);
  try {
    if (Array.isArray(value)) {
      assertArrayData(value, logicalPath);
      if (value.length > MAX_OPAQUE_JSON_ARRAY_ENTRIES) {
        throw new RuntimeCompositionAuthoringError(
          'quota-exceeded', logicalPath,
          `opaque arrays are limited to ${String(MAX_OPAQUE_JSON_ARRAY_ENTRIES)} entries`,
        );
      }
      const output: JsonValue[] = [];
      for (let index = 0; index < value.length; index += 1) {
        output.push(normalize(value[index], `${logicalPath}[${String(index)}]`, depth + 1, budget));
      }
      return Object.freeze(output);
    }
    const record = plainRecord(value, logicalPath);
    const keys = Object.keys(record).sort(compareUtf8);
    if (keys.length > MAX_OPAQUE_JSON_OBJECT_ENTRIES) {
      throw new RuntimeCompositionAuthoringError(
        'quota-exceeded', logicalPath,
        `opaque objects are limited to ${String(MAX_OPAQUE_JSON_OBJECT_ENTRIES)} entries`,
      );
    }
    const output: Record<string, JsonValue> = Object.create(null) as Record<string, JsonValue>;
    for (const key of keys) {
      assertString(key, `${logicalPath}.<key>`);
      Object.defineProperty(output, key, {
        value: normalize(record[key], `${logicalPath}.${key}`, depth + 1, budget),
        enumerable: true, configurable: false, writable: false,
      });
    }
    return Object.freeze(output);
  } finally {
    budget.active.delete(value);
  }
}

export function compareUtf8(left: string, right: string): number {
  const leftBytes = utf8(left);
  const rightBytes = utf8(right);
  const length = Math.min(leftBytes.length, rightBytes.length);
  for (let index = 0; index < length; index += 1) {
    const difference = (leftBytes[index] as number) - (rightBytes[index] as number);
    if (difference !== 0) return difference;
  }
  return leftBytes.length - rightBytes.length;
}

export function utf8Length(value: string): number { return utf8(value).length; }

/** Emits recursive canonical JSON without relying on JavaScript object property order. */
export function writeCanonicalJson(value: JsonValue): string {
  if (value === null) return 'null';
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (typeof value === 'number') return writeCanonicalNumber(value);
  if (typeof value === 'string') return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map((entry) => writeCanonicalJson(entry)).join(',')}]`;
  const record = value as Readonly<Record<string, JsonValue>>;
  return `{${Object.keys(record).sort(compareUtf8).map((key) => `${JSON.stringify(key)}:${writeCanonicalJson(record[key] as JsonValue)}`).join(',')}}`;
}

/** Rust chooses ryu-js/ECMAScript Number::toString spelling; normalization turns -0 into 0. */
function writeCanonicalNumber(value: number): string {
  if (
    PRODUCT_MODEL_NUMBER_ENCODING.finiteBinary64 !== 'ecmascript-number-to-string'
    || PRODUCT_MODEL_NUMBER_ENCODING.negativeZero !== '0'
    || PRODUCT_MODEL_NUMBER_ENCODING.integer !== 'base10'
  ) {
    throw new RuntimeCompositionAuthoringError('invalid-json-value', '$', 'unsupported Rust-owned canonical number policy');
  }
  const encoded = JSON.stringify(value);
  if (encoded === undefined) {
    throw new RuntimeCompositionAuthoringError('invalid-json-value', '$', 'non-finite JSON number cannot be encoded');
  }
  return encoded;
}

function utf8(value: string): Uint8Array { return new TextEncoder().encode(value); }

function assertString(value: string, logicalPath: string): void {
  if (utf8Length(value) > MAX_OPAQUE_JSON_STRING_BYTES) {
    throw new RuntimeCompositionAuthoringError(
      'quota-exceeded', logicalPath,
      `opaque JSON strings are limited to ${String(MAX_OPAQUE_JSON_STRING_BYTES)} UTF-8 bytes`,
    );
  }
  for (const unit of value) {
    const code = unit.codePointAt(0) as number;
    if (code >= 0xd800 && code <= 0xdfff) {
      throw new RuntimeCompositionAuthoringError(
        'invalid-json-value', logicalPath, 'JSON strings must contain Unicode scalar values',
      );
    }
  }
}

function plainRecord(value: object, logicalPath: string): Readonly<Record<string, unknown>> {
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new RuntimeCompositionAuthoringError('invalid-json-value', logicalPath, 'JSON objects must be plain data objects');
  }
  const descriptors = Object.getOwnPropertyDescriptors(value);
  for (const key of Reflect.ownKeys(descriptors)) {
    if (typeof key === 'symbol') {
      throw new RuntimeCompositionAuthoringError('invalid-json-value', logicalPath, 'JSON objects cannot contain symbol keys');
    }
    const descriptor = descriptors[key];
    if (descriptor === undefined || !descriptor.enumerable || !('value' in descriptor)) {
      throw new RuntimeCompositionAuthoringError('invalid-json-value', `${logicalPath}.${key}`, 'JSON objects cannot contain accessors or non-enumerable fields');
    }
  }
  return value as Readonly<Record<string, unknown>>;
}

function assertArrayData(value: readonly unknown[], logicalPath: string): void {
  if (Object.getPrototypeOf(value) !== Array.prototype) {
    throw new RuntimeCompositionAuthoringError('invalid-json-value', logicalPath, 'JSON arrays must use the ordinary Array prototype');
  }
  for (let index = 0; index < value.length; index += 1) {
    if (!Object.hasOwn(value, index)) {
      throw new RuntimeCompositionAuthoringError('invalid-json-value', `${logicalPath}[${String(index)}]`, 'JSON arrays cannot contain holes or undefined entries');
    }
  }
  for (const key of Reflect.ownKeys(value)) {
    if (key === 'length') continue;
    if (typeof key === 'symbol' || !isArrayIndex(key)) {
      throw new RuntimeCompositionAuthoringError('invalid-json-value', logicalPath, 'JSON arrays cannot contain non-index properties');
    }
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (descriptor === undefined || !descriptor.enumerable || !('value' in descriptor)) {
      throw new RuntimeCompositionAuthoringError('invalid-json-value', `${logicalPath}[${key}]`, 'JSON arrays cannot contain accessors');
    }
  }
}

function isArrayIndex(key: string): boolean {
  if (key !== '0' && !/^[1-9][0-9]*$/.test(key)) return false;
  const number = Number(key);
  return Number.isSafeInteger(number) && number < 4_294_967_295 && String(number) === key;
}
