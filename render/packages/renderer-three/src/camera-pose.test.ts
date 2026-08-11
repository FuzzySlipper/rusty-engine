import assert from 'node:assert/strict';
import { test } from 'node:test';

import * as THREE from 'three';

import { applyRendererThreeCameraPose } from './camera-pose.js';

const EPSILON = 0.000_001;

function assertVectorClose(
  actual: THREE.Vector3,
  expected: readonly [number, number, number],
): void {
  assert.ok(Math.abs(actual.x - expected[0]) <= EPSILON, `${actual.x} != ${expected[0]}`);
  assert.ok(Math.abs(actual.y - expected[1]) <= EPSILON, `${actual.y} != ${expected[1]}`);
  assert.ok(Math.abs(actual.z - expected[2]) <= EPSILON, `${actual.z} != ${expected[2]}`);
}

function canonicalBasis(yawDegrees: number, pitchDegrees: number): {
  readonly forward: readonly [number, number, number];
  readonly right: readonly [number, number, number];
  readonly up: readonly [number, number, number];
} {
  const yaw = THREE.MathUtils.degToRad(yawDegrees);
  const pitch = THREE.MathUtils.degToRad(pitchDegrees);
  const cosPitch = Math.cos(pitch);
  return {
    forward: [Math.sin(yaw) * cosPitch, Math.sin(pitch), -Math.cos(yaw) * cosPitch],
    right: [Math.cos(yaw), 0, Math.sin(yaw)],
    up: [
      -Math.sin(yaw) * Math.sin(pitch),
      Math.cos(pitch),
      Math.cos(yaw) * Math.sin(pitch),
    ],
  };
}

for (const [yawDegrees, pitchDegrees] of [
  [0, 0],
  [90, 0],
  [-90, 0],
  [30, 20],
  [-35, -25],
] as const) {
  void test(`Three camera realizes canonical Engine pose ${yawDegrees}/${pitchDegrees}`, () => {
    const camera = new THREE.PerspectiveCamera();
    applyRendererThreeCameraPose(camera, {
      position: [4, 5, 6],
      yawDegrees,
      pitchDegrees,
    });
    camera.updateMatrixWorld(true);

    const expected = canonicalBasis(yawDegrees, pitchDegrees);
    assertVectorClose(camera.getWorldDirection(new THREE.Vector3()), expected.forward);
    assertVectorClose(new THREE.Vector3(1, 0, 0).applyQuaternion(camera.quaternion), expected.right);
    assertVectorClose(new THREE.Vector3(0, 1, 0).applyQuaternion(camera.quaternion), expected.up);
    assertVectorClose(camera.position, [4, 5, 6]);
  });
}
