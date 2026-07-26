import assert from 'node:assert/strict';
import test from 'node:test';

import { applyTransformToolDelta } from './transform-tools.js';

const identity = {
  translation: [0, 0, 0] as const,
  rotation: [0, 0, 0, 1] as const,
  scale: [1, 1, 1] as const,
};
const settings = {
  snappingEnabled: true,
  translationSnapAxes: [0.5, 1, 2] as const,
  rotationSnapDegrees: 15,
  scaleSnapAxes: [0.25, 0.5, 1] as const,
  fineMultiplier: 0.1,
};

test('world translation converts through a rotated and anisotropically scaled parent', () => {
  const parent = {
    translation: [10, 0, 0] as const,
    rotation: [0, Math.SQRT1_2, 0, Math.SQRT1_2] as const,
    scale: [2, 1, 4] as const,
  };
  const result = applyTransformToolDelta({
    local: { ...identity, translation: [1, 0, 0] },
    world: { ...identity, translation: [10, 0, -2] },
    parentWorld: parent,
    tool: 'translate',
    orientation: 'world',
    axis: 0,
    delta: 0.6,
    fine: false,
    toggleSnap: false,
    settings,
  });
  assert.ok(Math.abs(result.translation[0] - 1) < 1e-9);
  assert.ok(Math.abs(result.translation[2] - 0.125) < 1e-9);
});

test('local rotation, world scale, anisotropic snap, and fine modifier stay explicit', () => {
  const rotated = applyTransformToolDelta({
    local: identity,
    world: identity,
    parentWorld: null,
    tool: 'rotate',
    orientation: 'local',
    axis: 1,
    delta: 22,
    fine: false,
    toggleSnap: false,
    settings,
  });
  assert.ok(Math.abs(rotated.rotation[1] - Math.sin(Math.PI / 24)) < 1e-9);

  const scaled = applyTransformToolDelta({
    local: identity,
    world: identity,
    parentWorld: null,
    tool: 'scale',
    orientation: 'world',
    axis: 0,
    delta: 0.14,
    fine: true,
    toggleSnap: false,
    settings,
  });
  assert.ok(Math.abs(scaled.scale[0] - 1.025) < 1e-12);

  const unsnapped = applyTransformToolDelta({
    local: identity,
    world: identity,
    parentWorld: null,
    tool: 'translate',
    orientation: 'world',
    axis: 2,
    delta: 0.3,
    fine: false,
    toggleSnap: true,
    settings,
  });
  assert.equal(unsnapped.translation[2], 0.3);
});
