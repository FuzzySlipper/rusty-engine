import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { RendererCameraSnapshot, RendererCameraTransitionReadout } from './host-types.js';
import { sampleCameraTransition } from './camera-transition.js';

function snapshot(position: readonly [number, number, number], yawDegrees: number): RendererCameraSnapshot {
  return {
    pose: { position, yawDegrees, pitchDegrees: -20 },
    basis: { forward: [0, 0, -1], right: [1, 0, 0], up: [0, 1, 0] },
    projection: { fovYDegrees: 60, near: 0.1, far: 500 },
    viewport: { width: 1280, height: 720 },
  };
}

void test('camera transition sampling is disposable and preserves exact caller endpoints', () => {
  const from = snapshot([0, 2, 4], 350);
  const to = snapshot([10, 4, -6], 10);
  const transition: RendererCameraTransitionReadout = {
    from,
    to,
    durationMilliseconds: 400,
    easing: 'smoothStep',
  };

  assert.equal(sampleCameraTransition(transition, -1), from);
  assert.equal(sampleCameraTransition(transition, 400), to);
  const midpoint = sampleCameraTransition(transition, 200);
  assert.deepEqual(midpoint.pose.position, [5, 3, -1]);
  assert.equal(midpoint.pose.yawDegrees, 360);
  assert.notEqual(midpoint, to);
  assert.deepEqual(transition.to, to, 'renderer sampling must not mutate accepted input');
});

void test('camera transition sampling rejects malformed renderer timing', () => {
  const value: RendererCameraTransitionReadout = {
    from: snapshot([0, 0, 0], 0),
    to: snapshot([1, 1, 1], 10),
    durationMilliseconds: 0,
    easing: 'linear',
  };
  assert.throws(() => sampleCameraTransition(value, 1), RangeError);
  assert.throws(() => sampleCameraTransition({ ...value, durationMilliseconds: 1 }, Number.NaN), TypeError);
});
