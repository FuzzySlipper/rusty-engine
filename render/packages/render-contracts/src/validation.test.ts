import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  audioHandle,
  ContractDecodeError,
  decodePresentationFrameDiff,
  decodeRenderFrameDiff,
  renderHandle,
} from './index.js';

const repoRoot = resolve(import.meta.dirname, '../../../..');

function fixture(name: string): unknown {
  return JSON.parse(readFileSync(resolve(repoRoot, 'fixtures/render', name), 'utf8')) as unknown;
}

function mutableFixture(name: string): Record<string, unknown> {
  return structuredClone(fixture(name)) as Record<string, unknown>;
}

void test('strict TypeScript decoders accept the committed Rust render fixtures', () => {
  assert.equal(decodeRenderFrameDiff(fixture('retained-frame-v1.json')).ops.length, 2);
  assert.equal(decodePresentationFrameDiff(fixture('presentation-frame-v1.json')).ops.length, 5);
});

void test('render decoding rejects unsafe handles and unknown nested fields', () => {
  const unsafe = mutableFixture('retained-frame-v1.json');
  const unsafeOps = unsafe['ops'] as Array<Record<string, unknown>>;
  unsafeOps[0]!['handle'] = Number.MAX_SAFE_INTEGER + 1;
  assert.throws(() => decodeRenderFrameDiff(unsafe), ContractDecodeError);

  const unknown = mutableFixture('retained-frame-v1.json');
  const unknownOps = unknown['ops'] as Array<Record<string, unknown>>;
  const node = unknownOps[0]!['node'] as Record<string, unknown>;
  const metadata = node['metadata'] as Record<string, unknown>;
  metadata['authority'] = 'must-not-cross-render-border';
  assert.throws(() => decodeRenderFrameDiff(unknown), /authority is unknown/);
});

void test('typed handle constructors reject values that cannot cross JSON exactly', () => {
  assert.equal(renderHandle(Number.MAX_SAFE_INTEGER), Number.MAX_SAFE_INTEGER);
  assert.equal(audioHandle(0), 0);
  assert.throws(() => renderHandle(-1), RangeError);
  assert.throws(() => audioHandle(Number.MAX_SAFE_INTEGER + 1), RangeError);
});

void test('presentation decoding rejects unsafe identities, sequence gaps, and nested drift', () => {
  const unsafe = mutableFixture('presentation-frame-v1.json');
  const unsafeOps = unsafe['ops'] as Array<Record<string, unknown>>;
  const billboard = unsafeOps[1]!['op'] as Record<string, unknown>;
  billboard['handle'] = Number.MAX_SAFE_INTEGER + 1;
  assert.throws(() => decodePresentationFrameDiff(unsafe), /safe integer/);

  const gap = mutableFixture('presentation-frame-v1.json');
  const gapOps = gap['ops'] as Array<Record<string, unknown>>;
  const meta = gapOps[2]!['meta'] as Record<string, unknown>;
  meta['sequence'] = 7;
  assert.throws(() => decodePresentationFrameDiff(gap), /must equal ordered index 2/);

  const unknown = mutableFixture('presentation-frame-v1.json');
  const unknownOps = unknown['ops'] as Array<Record<string, unknown>>;
  const billboardOp = unknownOps[1]!['op'] as Record<string, unknown>;
  const descriptor = billboardOp['descriptor'] as Record<string, unknown>;
  const content = descriptor['content'] as Record<string, unknown>;
  content['sendMessage'] = 'no';
  assert.throws(() => decodePresentationFrameDiff(unknown), /sendMessage is unknown/);
});
