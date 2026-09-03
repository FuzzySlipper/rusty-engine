import assert from 'node:assert/strict';
import test from 'node:test';

import {
  RUSTY_RENDERER_SURFACE_ADMISSION_HISTORY_LIMIT,
  RendererSurfaceAutomaticSubmissionAdmissionObservation,
  type RendererSurfaceAutomaticSubmissionAdmissionOutcome,
} from './surface-admission-observation.js';
import type {
  RendererSurfaceSubmissionDemandDecision,
} from './surface-submission-demand.js';

const DEMAND: RendererSurfaceSubmissionDemandDecision = Object.freeze({
  schemaVersion: 1,
  requested: false,
  viewportChanged: false,
  controls: true,
  presentation: false,
  retainedAnimation: false,
  shouldSubmit: true,
});

const BACKEND = Object.freeze({
  mode: 'timerQuery' as const,
  state: 'ready' as const,
  rendererClass: 'accelerated' as const,
  timerDurationMs: 4,
  effectiveDurationMs: 4,
  admittedAtMs: 8,
  admissionObservedAtMs: 16,
  observedAtMs: 16,
  automaticSubmissionLimit: 8,
  pendingMeasurementCount: 2,
  completionFenceMode: 'active' as const,
  maximumPendingSubmissions: 8,
  pendingSubmissionCount: 2,
});

const CALLBACK = Object.freeze({
  schemaVersion: 1 as const,
  callbackStartedAtMs: 16,
  successorQueuedAtMs: 16.1,
  demandObservedAtMs: 16.2,
  backendReadinessObservedAtMs: 16.3,
  controlsUpdatedAtMs: 16.4,
  cameraUpdatedAtMs: 16.5,
  presentationAdvancedAtMs: 16.6,
  backendSubmittedAtMs: 18.7,
  callbackEndedAtMs: 18.8,
});

void test('host admission observation separates demand backend blocks and admissions', () => {
  const observation = new RendererSurfaceAutomaticSubmissionAdmissionObservation();

  observation.record(16, 'noDemand', {
    ...DEMAND,
    controls: false,
    shouldSubmit: false,
  }, BACKEND, {
    ...CALLBACK,
    backendReadinessObservedAtMs: null,
    controlsUpdatedAtMs: null,
    cameraUpdatedAtMs: null,
    presentationAdvancedAtMs: null,
    backendSubmittedAtMs: null,
  });
  observation.record(32, 'backendBlocked', DEMAND, {
    ...BACKEND,
    state: 'waiting',
  }, {
    ...CALLBACK,
    controlsUpdatedAtMs: null,
    cameraUpdatedAtMs: null,
    presentationAdvancedAtMs: null,
    backendSubmittedAtMs: null,
  });
  observation.record(48, 'admitted', DEMAND, BACKEND, CALLBACK);

  const sample = observation.sample();
  assert.deepEqual(
    {
      attemptCount: sample.attemptCount,
      admittedCount: sample.admittedCount,
      backendBlockedCount: sample.backendBlockedCount,
      noDemandCount: sample.noDemandCount,
    },
    {
      attemptCount: 3,
      admittedCount: 1,
      backendBlockedCount: 1,
      noDemandCount: 1,
    },
  );
  assert.deepEqual(sample.demandCounts, {
    requested: 0,
    viewportChanged: 0,
    controls: 2,
    presentation: 0,
    retainedAnimation: 0,
  });
  assert.deepEqual(sample.recentCallbackIntervalsMs, [16, 16]);
  assert.deepEqual(sample.recentSubmissionIntervalsMs, []);
  assert.deepEqual(
    sample.recentAttempts.map((attempt) => [
      attempt.sequence,
      attempt.sourceTimeMs,
      attempt.outcome,
      attempt.backend.state,
    ]),
    [
      [1, 16, 'noDemand', 'ready'],
      [2, 32, 'backendBlocked', 'waiting'],
      [3, 48, 'admitted', 'ready'],
    ],
  );
  assert.equal(Object.isFrozen(sample), true);
  assert.equal(Object.isFrozen(sample.recentAttempts), true);
  assert.equal(Object.isFrozen(sample.recentAttempts[0]?.backend), true);
  assert.equal(Object.isFrozen(sample.recentAttempts[0]?.callback), true);
  assert.deepEqual(sample.recentAttempts.at(-1)?.callback, CALLBACK);
});

void test('host admission history is bounded while lifetime counters remain exact', () => {
  const observation = new RendererSurfaceAutomaticSubmissionAdmissionObservation();
  const outcomes: readonly RendererSurfaceAutomaticSubmissionAdmissionOutcome[] = [
    'admitted',
    'backendBlocked',
    'noDemand',
  ];
  const attemptCount = RUSTY_RENDERER_SURFACE_ADMISSION_HISTORY_LIMIT + 9;

  for (let index = 0; index < attemptCount; index += 1) {
    observation.record(
      index * 8,
      outcomes[index % outcomes.length] ?? 'admitted',
      DEMAND,
      BACKEND,
      CALLBACK,
    );
  }

  const sample = observation.sample();
  assert.equal(sample.attemptCount, attemptCount);
  assert.equal(sample.recentAttempts.length, RUSTY_RENDERER_SURFACE_ADMISSION_HISTORY_LIMIT);
  assert.equal(sample.recentAttempts[0]?.sequence, 10);
  assert.equal(sample.recentAttempts.at(-1)?.sequence, attemptCount);
  assert.equal(
    sample.admittedCount + sample.backendBlockedCount + sample.noDemandCount,
    attemptCount,
  );
});
