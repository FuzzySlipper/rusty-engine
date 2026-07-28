import { Buffer } from 'node:buffer';
import { TextDecoder } from 'node:util';

import {
  RULE_LIMITS,
  type JsonValue,
} from './generated.js';
import { RuleContractError } from './error.js';

const collectionLimits = new Map<string, number>([
  ['$/dependencies', RULE_LIMITS.maxDependenciesPerRulePackage],
  ['$/sources', RULE_LIMITS.maxSourcesPerRulePackage],
  ['$/provenance', RULE_LIMITS.maxProvenancePerRulePackage],
]);

export interface StrictJsonResult {
  readonly value: JsonValue;
  readonly nodes: number;
}

export function parseStrictJson(input: Uint8Array): StrictJsonResult {
  if (
    input.length >= 3 &&
    input[0] === 0xef &&
    input[1] === 0xbb &&
    input[2] === 0xbf
  ) {
    throw new RuleContractError(
      'malformed-utf8',
      '$',
      'UTF-8 byte-order marks are not accepted',
    );
  }
  let text: string;
  try {
    text = new TextDecoder('utf-8', { fatal: true }).decode(input);
  } catch {
    throw new RuleContractError(
      'malformed-utf8',
      '$',
      'artifact is not valid UTF-8',
    );
  }
  return new StrictJsonParser(text).parse();
}

class StrictJsonParser {
  private offset = 0;
  private nodes = 0;

  public constructor(private readonly input: string) {}

  public parse(): StrictJsonResult {
    this.skipWhitespace();
    const value = this.parseValue(1, '$');
    this.skipWhitespace();
    if (this.offset !== this.input.length) {
      this.fail('$', 'trailing data after the JSON value');
    }
    return Object.freeze({ value, nodes: this.nodes });
  }

  private parseValue(depth: number, path: string): JsonValue {
    if (depth > RULE_LIMITS.maxJsonNestingDepth) {
      throw new RuleContractError(
        'json-depth-exceeded',
        path,
        'JSON nesting depth exceeds the package limit',
        {
          actual: depth,
          maximum: RULE_LIMITS.maxJsonNestingDepth,
        },
      );
    }
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

    const character = this.peek();
    if (character === 'n') {
      this.expectLiteral('null', path);
      return null;
    }
    if (character === 't') {
      this.expectLiteral('true', path);
      return true;
    }
    if (character === 'f') {
      this.expectLiteral('false', path);
      return false;
    }
    if (character === '"') return this.parseString(path);
    if (character === '[') return this.parseArray(depth, path);
    if (character === '{') return this.parseObject(depth, path);
    if (character === '-' || isDigit(character)) {
      return this.parseNumber(path);
    }
    this.fail(path, character === undefined ? 'unexpected end of input' : 'expected a JSON value');
  }

  private parseArray(depth: number, path: string): readonly JsonValue[] {
    this.offset += 1;
    this.skipWhitespace();
    const values: JsonValue[] = [];
    if (this.consume(']')) return Object.freeze(values);
    for (;;) {
      const maximum = collectionLimits.get(path);
      if (maximum !== undefined && values.length >= maximum) {
        throw new RuleContractError(
          'quota-exceeded',
          path,
          'envelope collection exceeds its item limit',
          { actual: maximum + 1, maximum },
        );
      }
      values.push(this.parseValue(depth + 1, pointerIndex(path, values.length)));
      this.skipWhitespace();
      if (this.consume(']')) return Object.freeze(values);
      if (!this.consume(',')) {
        this.fail(path, "expected ',' or ']' in array");
      }
      this.skipWhitespace();
    }
  }

  private parseObject(
    depth: number,
    path: string,
  ): Readonly<Record<string, JsonValue>> {
    this.offset += 1;
    this.skipWhitespace();
    const values: Record<string, JsonValue> = Object.create(null);
    const seen = new Set<string>();
    if (this.consume('}')) return Object.freeze(values);
    for (;;) {
      if (this.peek() !== '"') {
        this.fail(path, 'expected a string object key');
      }
      const key = this.parseString(`${path}/<key>`);
      if (seen.has(key)) {
        throw new RuleContractError(
          'duplicate-json-key',
          path,
          `duplicate JSON object key ${JSON.stringify(key)}`,
          { key },
        );
      }
      seen.add(key);
      this.skipWhitespace();
      if (!this.consume(':')) {
        this.fail(path, "expected ':' after object key");
      }
      this.skipWhitespace();
      values[key] = this.parseValue(depth + 1, pointerKey(path, key));
      this.skipWhitespace();
      if (this.consume('}')) return Object.freeze(values);
      if (!this.consume(',')) {
        this.fail(path, "expected ',' or '}' in object");
      }
      this.skipWhitespace();
    }
  }

  private parseNumber(path: string): number {
    const start = this.offset;
    this.consume('-');
    const first = this.peek();
    if (first === '0') {
      this.offset += 1;
      if (isDigit(this.peek())) {
        this.fail(path, 'leading zero in JSON number');
      }
    } else if (isNonzeroDigit(first)) {
      this.offset += 1;
      while (isDigit(this.peek())) this.offset += 1;
    } else {
      this.fail(path, 'invalid JSON number');
    }

    if (this.peek() === '.' || this.peek() === 'e' || this.peek() === 'E') {
      while (isNumberTokenCharacter(this.peek())) this.offset += 1;
      throw new RuleContractError(
        'json-integer-out-of-range',
        path,
        'fractional and exponent JSON numbers are not supported',
        { value: boundedToken(this.input.slice(start, this.offset)) },
      );
    }
    const token = this.input.slice(start, this.offset);
    const magnitude = BigInt(token.startsWith('-') ? token.slice(1) : token);
    if (magnitude > BigInt(RULE_LIMITS.maxSafeJsonInteger)) {
      throw new RuleContractError(
        'json-integer-out-of-range',
        path,
        'JSON integer exceeds the portable safe range',
        { value: boundedToken(token) },
      );
    }
    const value = Number(token);
    return Object.is(value, -0) ? 0 : value;
  }

  private parseString(path: string): string {
    this.offset += 1;
    let output = '';
    let outputBytes = 0;
    for (;;) {
      const code = this.input.charCodeAt(this.offset);
      if (Number.isNaN(code)) {
        this.fail(path, 'unterminated JSON string');
      }
      if (code === 0x22) {
        this.offset += 1;
        return output;
      }

      let value: string;
      if (code === 0x5c) {
        this.offset += 1;
        value = this.parseEscape(path);
      } else {
        if (code <= 0x1f) {
          this.fail(path, 'unescaped control character in JSON string');
        }
        if (code >= 0xd800 && code <= 0xdbff) {
          const next = this.input.charCodeAt(this.offset + 1);
          if (next < 0xdc00 || next > 0xdfff) {
            this.fail(path, 'unpaired high surrogate');
          }
          value = this.input.slice(this.offset, this.offset + 2);
          this.offset += 2;
        } else if (code >= 0xdc00 && code <= 0xdfff) {
          this.fail(path, 'unpaired low surrogate');
        } else {
          value = this.input[this.offset] as string;
          this.offset += 1;
        }
      }

      outputBytes += Buffer.byteLength(value, 'utf8');
      if (outputBytes > RULE_LIMITS.maxJsonStringBytes) {
        throw new RuleContractError(
          'quota-exceeded',
          path,
          'JSON string exceeds its UTF-8 byte limit',
          {
            actual: outputBytes,
            maximum: RULE_LIMITS.maxJsonStringBytes,
          },
        );
      }
      output += value;
    }
  }

  private parseEscape(path: string): string {
    const escaped = this.peek();
    if (escaped === undefined) this.fail(path, 'incomplete JSON escape');
    this.offset += 1;
    switch (escaped) {
      case '"':
      case '\\':
      case '/':
        return escaped;
      case 'b':
        return '\b';
      case 'f':
        return '\f';
      case 'n':
        return '\n';
      case 'r':
        return '\r';
      case 't':
        return '\t';
      case 'u': {
        const first = this.parseHexQuad(path);
        if (first >= 0xd800 && first <= 0xdbff) {
          if (!this.consume('\\') || !this.consume('u')) {
            this.fail(path, 'unpaired high surrogate');
          }
          const second = this.parseHexQuad(path);
          if (second < 0xdc00 || second > 0xdfff) {
            this.fail(path, 'unpaired high surrogate');
          }
          return String.fromCodePoint(
            0x10000 + ((first - 0xd800) << 10) + (second - 0xdc00),
          );
        }
        if (first >= 0xdc00 && first <= 0xdfff) {
          this.fail(path, 'unpaired low surrogate');
        }
        return String.fromCodePoint(first);
      }
      default:
        this.fail(path, 'unsupported JSON escape');
    }
  }

  private parseHexQuad(path: string): number {
    let value = 0;
    for (let index = 0; index < 4; index += 1) {
      const character = this.peek();
      const digit = hexDigit(character);
      if (digit === undefined) {
        this.fail(path, 'invalid or incomplete Unicode escape');
      }
      value = value * 16 + digit;
      this.offset += 1;
    }
    return value;
  }

  private expectLiteral(literal: string, path: string): void {
    if (!this.input.startsWith(literal, this.offset)) {
      this.fail(path, 'invalid JSON literal');
    }
    this.offset += literal.length;
  }

  private skipWhitespace(): void {
    while (
      this.peek() === ' ' ||
      this.peek() === '\n' ||
      this.peek() === '\r' ||
      this.peek() === '\t'
    ) {
      this.offset += 1;
    }
  }

  private consume(expected: string): boolean {
    if (this.peek() !== expected) return false;
    this.offset += 1;
    return true;
  }

  private peek(): string | undefined {
    return this.input[this.offset];
  }

  private fail(path: string, reason: string): never {
    throw new RuleContractError(
      'malformed-json',
      path,
      reason,
      { offset: this.offset },
    );
  }
}

export function pointerKey(parent: string, key: string): string {
  return `${parent}/${key.replaceAll('~', '~0').replaceAll('/', '~1')}`;
}

export function pointerIndex(parent: string, index: number): string {
  return `${parent}/${String(index)}`;
}

function isDigit(value: string | undefined): boolean {
  return value !== undefined && value >= '0' && value <= '9';
}

function isNonzeroDigit(value: string | undefined): boolean {
  return value !== undefined && value >= '1' && value <= '9';
}

function isNumberTokenCharacter(value: string | undefined): boolean {
  return (
    isDigit(value) ||
    value === '.' ||
    value === 'e' ||
    value === 'E' ||
    value === '+' ||
    value === '-'
  );
}

function hexDigit(value: string | undefined): number | undefined {
  if (value === undefined) return undefined;
  if (value >= '0' && value <= '9') return value.charCodeAt(0) - 0x30;
  if (value >= 'a' && value <= 'f') return value.charCodeAt(0) - 0x61 + 10;
  if (value >= 'A' && value <= 'F') return value.charCodeAt(0) - 0x41 + 10;
  return undefined;
}

function boundedToken(value: string): string {
  return value.length <= 64 ? value : `${value.slice(0, 64)}...`;
}
