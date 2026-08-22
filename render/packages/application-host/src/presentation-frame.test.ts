import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  resolvePresentationFrameGeometry,
  validatePresentationAspectBounds,
} from './presentation-frame.js';

void test('presentation bounds reject non-finite, non-positive, and reversed values', () => {
  for (const value of [
    { minimum: Number.NaN, maximum: 1 },
    { minimum: 1, maximum: Number.POSITIVE_INFINITY },
    { minimum: 0, maximum: 1 },
    { minimum: -1, maximum: 1 },
    { minimum: 2, maximum: 1 },
  ]) {
    assert.throws(() => validatePresentationAspectBounds(value), RangeError);
  }
});

void test('presentation bounds preserve omission and calculate exact, narrow, and wide frames', () => {
  assert.equal(validatePresentationAspectBounds(undefined), undefined);
  const bounds = validatePresentationAspectBounds({ minimum: 4 / 3, maximum: 16 / 9 })!;
  assert.deepEqual(resolvePresentationFrameGeometry(800, 600, bounds), { width: 800, height: 600 });
  assert.deepEqual(resolvePresentationFrameGeometry(600, 600, bounds), { width: 600, height: 450 });
  assert.deepEqual(resolvePresentationFrameGeometry(900, 400, bounds), {
    width: 400 * (16 / 9), height: 400,
  });
  assert.deepEqual(resolvePresentationFrameGeometry(0, 400, bounds), { width: 0, height: 0 });
});
