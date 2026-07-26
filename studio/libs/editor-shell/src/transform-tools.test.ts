import assert from 'node:assert/strict';
import test from 'node:test';

import { composeTransform, localTransformFromWorld } from './transform-tools.js';

test('world presentation and local Rust ownership round-trip through a transformed parent', () => {
  const parent = {
    translation: [10, 2, -4] as const,
    rotation: [0, Math.SQRT1_2, 0, Math.SQRT1_2] as const,
    scale: [2, 1, 4] as const,
  };
  const local = {
    translation: [1, 3, -2] as const,
    rotation: [0, 0, Math.SQRT1_2, Math.SQRT1_2] as const,
    scale: [0.5, 2, 3] as const,
  };

  const world = composeTransform(parent, local);
  const roundTrip = localTransformFromWorld(parent, world);

  assert.ok(roundTrip.translation.every(
    (value, axis) => Math.abs(value - (local.translation[axis] as number)) < 1e-9,
  ));
  assert.ok(roundTrip.rotation.every(
    (value, axis) => Math.abs(value - (local.rotation[axis] as number)) < 1e-9,
  ));
  assert.deepEqual(roundTrip.scale, local.scale);
});

test('root world candidates clone directly into local owner values', () => {
  const world = {
    translation: [4, 5, 6] as const,
    rotation: [0, 0, 0, 1] as const,
    scale: [2, 3, 4] as const,
  };

  const local = localTransformFromWorld(null, world);

  assert.deepEqual(local, world);
  assert.notEqual(local.translation, world.translation);
});
