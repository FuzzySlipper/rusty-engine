import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as THREE from 'three';
import type { RendererCompositionCamera } from '@rusty-engine/render-contracts';
import { CameraMotion } from './camera-motion.js';

function sample(id: number, time: number, x: number, mode: 'position' | 'pose' = 'position', cut = false): RendererCompositionCamera {
  return {
    id: 'player', pose: { position: [x, 0, 0], pitchDegrees: 0, yawDegrees: x * 9 },
    projection: { kind: 'perspective', fovYDegrees: 60, near: 0.1, far: 100 },
    motion: { sampleId: String(id), sampleTimeSeconds: time, delaySeconds: 0.1, interpolation: mode, cut },
  };
}
const close = (actual: number, expected: number) => assert.ok(Math.abs(actual - expected) < 1e-8, `${actual} != ${expected}`);

void test('presentation advances between fixed samples, preserves latest look, and holds on missing samples', () => {
  const motion = new CameraMotion();
  const camera = new THREE.Camera();
  motion.receive(sample(1, 0, 0), 10);
  motion.receive(sample(2, 0.1, 10), 10.1);
  for (let frame = 0; frame <= 6; frame++) {
    motion.apply(camera, 10.1 + frame / 60);
    close(camera.position.x, frame * 10 / 6);
    close(new THREE.Vector3(0, 0, -1).applyQuaternion(camera.quaternion).x, 1);
  }
  motion.apply(camera, 50);
  close(camera.position.x, 10);
  assert.equal(motion.needsFrame(), false);
});

void test('full pose retains roll through quaternion interpolation and cuts reset immediately', () => {
  const motion = new CameraMotion();
  const camera = new THREE.Camera();
  motion.receive(sample(1, 0, 0, 'pose'), 0);
  const rolled = { ...sample(2, 0.1, 10, 'pose'), basis: {
    forward: [0, 0, -1] as const, right: [0, 1, 0] as const, up: [-1, 0, 0] as const,
  } };
  motion.receive(rolled, 0.1);
  motion.apply(camera, 0.15);
  close(camera.position.x, 5);
  close(new THREE.Vector3(0, 1, 0).applyQuaternion(camera.quaternion).x, -Math.SQRT1_2);
  motion.receive(sample(3, 0.2, 100, 'pose', true), 0.2);
  motion.apply(camera, 0.2);
  close(camera.position.x, 100);
  motion.receive(sample(4, 0.3, 110, 'pose'), 0.3);
  motion.apply(camera, 0.35);
  close(camera.position.x, 105);
});

void test('jitter and snapshot republication do not rewind presentation; timeline regressions reset', () => {
  const motion = new CameraMotion();
  const camera = new THREE.Camera();
  motion.receive(sample(1, 1, 0), 10);
  motion.receive(sample(2, 1.1, 10), 10.12);
  motion.apply(camera, 10.15);
  close(camera.position.x, 5);
  motion.receive(sample(2, 1.1, 10), 10.16);
  motion.receive(sample(3, 1.2, 20), 10.23);
  motion.apply(camera, 10.25);
  close(camera.position.x, 15);
  motion.receive(sample(4, 0, -100), 10.3);
  motion.apply(camera, 10.3);
  close(camera.position.x, -100);
  const { motion: _ignored, ...immediate } = sample(5, 1, 200);
  motion.receive(immediate, 10.4);
  motion.apply(camera, 10.4);
  close(camera.position.x, 200);
  assert.equal(motion.needsFrame(), false);
});

void test('a paused source timeline reanchors so subsequent samples still interpolate', () => {
  const motion = new CameraMotion();
  const camera = new THREE.Camera();
  motion.receive(sample(1, 0, 0), 0);
  motion.receive(sample(2, 0.1, 10), 0.1);
  motion.apply(camera, 10);
  close(camera.position.x, 10);
  motion.receive(sample(3, 0.2, 20), 10);
  motion.apply(camera, 10);
  close(camera.position.x, 20);
  motion.receive(sample(4, 0.3, 30), 10.1);
  motion.apply(camera, 10.15);
  close(camera.position.x, 25);
});
