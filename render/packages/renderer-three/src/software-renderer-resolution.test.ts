import assert from 'node:assert/strict';
import test from 'node:test';

import { resolveRendererPixelRatio } from './software-renderer-resolution.js';

void test('software rendering bounds backing-buffer work without changing smaller requests', () => {
  assert.equal(resolveRendererPixelRatio(2, 'software'), 0.25);
  assert.equal(resolveRendererPixelRatio(1, 'software'), 0.25);
  assert.equal(resolveRendererPixelRatio(0.125, 'software'), 0.125);
});

void test('accelerated and unknown renderers preserve the requested ratio', () => {
  assert.equal(resolveRendererPixelRatio(2, 'accelerated'), 2);
  assert.equal(resolveRendererPixelRatio(1.5, 'unknown'), 1.5);
});

void test('invalid renderer ratios fail before surface mutation', () => {
  for (const ratio of [0, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
    assert.throws(
      () => resolveRendererPixelRatio(ratio, 'software'),
      /pixel ratio must be finite and greater than zero/u,
    );
  }
});
