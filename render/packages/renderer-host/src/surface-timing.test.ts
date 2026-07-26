import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  RUSTY_RENDERER_SURFACE_MAX_TIMING_DURATION_MS,
  RendererSurfaceTimingTracker,
} from './surface-timing.js';

void test('surface timing distinguishes cadence from synchronous backend submission', () => {
  const timing = new RendererSurfaceTimingTracker();
  const mounted = timing.record({
    source: 'mount',
    sourceTimeMs: 0,
    backendSubmissionStartedMs: 10,
    backendSubmissionEndedMs: 12,
  });
  assert.equal(mounted.renderSequence, 1);
  assert.equal(mounted.frameIntervalMs, null);
  assert.equal(mounted.frameIntervalStatus, 'firstFrame');
  assert.equal(mounted.backendSubmissionDurationMs, 2);
  assert.equal(mounted.backendSubmissionDurationStatus, 'available');

  const explicit = timing.record({
    source: 'explicit',
    sourceTimeMs: 16.75,
    backendSubmissionStartedMs: 20,
    backendSubmissionEndedMs: 23.5,
  });
  assert.equal(explicit.renderSequence, 2);
  assert.equal(explicit.frameIntervalMs, 16.75);
  assert.equal(explicit.frameIntervalStatus, 'available');
  assert.equal(explicit.backendSubmissionDurationMs, 3.5);
  assert.equal(explicit.backendSubmissionDurationStatus, 'available');
  assert.equal(timing.read(), explicit);
  assert.equal(Object.isFrozen(explicit), true);
});

void test('surface timing rejects discontinuities without poisoning the next valid sample', () => {
  const timing = new RendererSurfaceTimingTracker();
  timing.record({
    source: 'mount',
    sourceTimeMs: 100,
    backendSubmissionStartedMs: 20,
    backendSubmissionEndedMs: 21,
  });
  const regressed = timing.record({
    source: 'cameraReset',
    sourceTimeMs: 0,
    backendSubmissionStartedMs: 30,
    backendSubmissionEndedMs: 29,
  });
  assert.equal(regressed.frameIntervalMs, null);
  assert.equal(regressed.frameIntervalStatus, 'sourceTimeRegressed');
  assert.equal(regressed.backendSubmissionDurationMs, null);
  assert.equal(regressed.backendSubmissionDurationStatus, 'clockRegressed');

  const excessive = timing.record({
    source: 'animationFrame',
    sourceTimeMs: RUSTY_RENDERER_SURFACE_MAX_TIMING_DURATION_MS + 1,
    backendSubmissionStartedMs: 40,
    backendSubmissionEndedMs: 40 + RUSTY_RENDERER_SURFACE_MAX_TIMING_DURATION_MS + 1,
  });
  assert.equal(excessive.frameIntervalMs, null);
  assert.equal(excessive.frameIntervalStatus, 'sourceTimeGapExceeded');
  assert.equal(excessive.backendSubmissionDurationMs, null);
  assert.equal(excessive.backendSubmissionDurationStatus, 'durationExceeded');

  const recovered = timing.record({
    source: 'animationFrame',
    sourceTimeMs: RUSTY_RENDERER_SURFACE_MAX_TIMING_DURATION_MS + 17,
    backendSubmissionStartedMs: 50,
    backendSubmissionEndedMs: 51,
  });
  assert.equal(recovered.frameIntervalMs, 16);
  assert.equal(recovered.frameIntervalStatus, 'available');
});

void test('invalid source time fails without mutating the latest sample', () => {
  const timing = new RendererSurfaceTimingTracker();
  const baseline = timing.record({
    source: 'mount',
    sourceTimeMs: 0,
    backendSubmissionStartedMs: 0,
    backendSubmissionEndedMs: 0,
  });
  assert.throws(() => timing.record({
    source: 'explicit',
    sourceTimeMs: Number.NaN,
    backendSubmissionStartedMs: 0,
    backendSubmissionEndedMs: 0,
  }), /source time/u);
  assert.equal(timing.read(), baseline);
});
