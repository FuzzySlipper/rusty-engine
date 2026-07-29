import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  createRendererSurfaceStatisticsSample,
  createRendererSurfaceSubmissionSample,
} from './surface-statistics.js';
import { RendererSurfaceTimingTracker } from './surface-timing.js';

const AVAILABLE_INPUT = {
  drawCallCount: 7,
  renderHandleCount: 12,
  geometryResourceCount: 5,
  materialResourceCount: 4,
  textureResourceCount: 2,
  animatedInstanceCount: 3,
  triangleCount: 48,
} as const;

void test('surface submission sample freezes exact per-submission and live-resident counters', () => {
  const timing = new RendererSurfaceTimingTracker().record({
    source: 'explicit',
    sourceTimeMs: 16,
    backendSubmissionStartedMs: 2,
    backendSubmissionEndedMs: 3,
  });
  const sample = createRendererSurfaceSubmissionSample(timing, AVAILABLE_INPUT);

  assert.equal(sample.statistics.drawCallCount.scope, 'perSubmission');
  assert.deepEqual(sample.statistics.drawCallCount, {
    scope: 'perSubmission', status: 'available', value: 7,
  });
  assert.deepEqual(sample.statistics.renderHandleCount, {
    scope: 'liveResident', status: 'available', value: 12,
  });
  assert.deepEqual(sample.statistics.triangleCount, {
    scope: 'perSubmission', status: 'available', value: 48,
  });
  assert.equal(Object.isFrozen(sample), true);
  assert.equal(Object.isFrozen(sample.statistics), true);
  assert.equal(Object.isFrozen(sample.statistics.drawCallCount), true);
});

void test('surface statistics distinguish unavailable, unsupported, and invalid values from zero', () => {
  const sample = createRendererSurfaceStatisticsSample({
    ...AVAILABLE_INPUT,
    drawCallCount: 0,
    geometryResourceCount: null,
    materialResourceCount: undefined,
    textureResourceCount: Number.NaN,
    triangleCount: -1,
  });

  assert.deepEqual(sample.drawCallCount, {
    scope: 'perSubmission', status: 'available', value: 0,
  });
  assert.deepEqual(sample.geometryResourceCount, {
    scope: 'liveResident', status: 'unavailable', value: null,
  });
  assert.deepEqual(sample.materialResourceCount, {
    scope: 'liveResident', status: 'unsupported', value: null,
  });
  assert.deepEqual(sample.textureResourceCount, {
    scope: 'liveResident', status: 'unavailable', value: null,
  });
  assert.deepEqual(sample.triangleCount, {
    scope: 'perSubmission', status: 'unavailable', value: null,
  });
});
