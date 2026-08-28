import assert from 'node:assert/strict';
import test from 'node:test';

import type { RendererViewComposition } from '@rusty-engine/render-contracts';

import { resolveRendererAudioListenerPose } from './renderer-listener-pose.js';

const FALLBACK = {
  pose: { position: [9, 8, 7] as const, pitchDegrees: 0, yawDegrees: 0 },
};

function composition(overrides: Partial<RendererViewComposition> = {}): RendererViewComposition {
  return {
    schemaVersion: 1,
    cameras: [],
    targets: [],
    views: [],
    presentations: [],
    ...overrides,
  };
}

void test('listener selects the earliest primary view by renderer order then id', () => {
  const pose = resolveRendererAudioListenerPose(composition({
    cameras: [
      camera('late', [1, 1, 1]),
      camera('id-wins', [2, 2, 2]),
      camera('early', [3, 3, 3]),
    ],
    views: [
      view('offscreen', 'late', 0, 'offscreen'),
      view('z-primary', 'late', 4, 'primary'),
      view('b-primary', 'early', 2, 'primary'),
      view('a-primary', 'id-wins', 2, 'primary'),
    ],
  }), FALLBACK);

  assert.deepEqual(pose.position, [2, 2, 2]);
});

void test('listener immediately falls back for offscreen-only and cleared primary compositions', () => {
  const offscreenOnly = composition({
    cameras: [camera('offscreen', [1, 2, 3])],
    views: [view('offscreen', 'offscreen', 0, 'offscreen')],
  });
  assert.deepEqual(resolveRendererAudioListenerPose(offscreenOnly, FALLBACK), {
    position: [9, 8, 7], forward: [0, 0, -1], up: [-0, 1, 0],
  });
  assert.deepEqual(resolveRendererAudioListenerPose(composition(), FALLBACK), {
    position: [9, 8, 7], forward: [0, 0, -1], up: [-0, 1, 0],
  });
});

void test('listener normalizes explicit basis and otherwise derives canonical yaw/pitch basis', () => {
  const explicit = resolveRendererAudioListenerPose(composition({
    cameras: [{
      ...camera('explicit', [1, 2, 3]),
      basis: { forward: [0, 0, -4], right: [1, 0, 0], up: [0, 9, 0] },
    }],
    views: [view('primary', 'explicit', 0, 'primary')],
  }), FALLBACK);
  assert.deepEqual(explicit, { position: [1, 2, 3], forward: [0, 0, -1], up: [0, 1, 0] });

  const canonical = resolveRendererAudioListenerPose(composition({
    cameras: [camera('canonical', [4, 5, 6], 30, 90)],
    views: [view('primary', 'canonical', 0, 'primary')],
  }), FALLBACK);
  assert.deepEqual(canonical.position, [4, 5, 6]);
  assert.ok(Math.abs(canonical.forward[0] - Math.cos(Math.PI / 6)) < 1e-12);
  assert.ok(Math.abs(canonical.forward[1] - 0.5) < 1e-12);
  assert.ok(Math.abs(canonical.forward[2]) < 1e-12);
  assert.ok(Math.abs(canonical.up[0] + 0.5) < 1e-12);
  assert.ok(Math.abs(canonical.up[1] - Math.cos(Math.PI / 6)) < 1e-12);
  assert.ok(Math.abs(canonical.up[2]) < 1e-12);
});

function camera(
  id: string,
  position: readonly [number, number, number],
  pitchDegrees = 0,
  yawDegrees = 0,
): RendererViewComposition['cameras'][number] {
  return {
    id,
    pose: { position, pitchDegrees, yawDegrees },
    projection: { kind: 'perspective', fovYDegrees: 60, near: 0.1, far: 100 },
  };
}

function view(
  id: string,
  cameraId: string,
  order: number,
  target: 'primary' | 'offscreen',
): RendererViewComposition['views'][number] {
  return {
    id,
    cameraId,
    order,
    target: target === 'primary' ? { kind: 'primary' } : {
      kind: 'offscreen', targetId: 'aux', targetRevision: 1,
    },
    viewport: { x: 0, y: 0, width: 1, height: 1 },
  };
}
