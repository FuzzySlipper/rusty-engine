import assert from 'node:assert/strict';
import test from 'node:test';

import {
  RendererGpuSubmissionDuty,
  type RendererGpuSubmissionTimerDriver,
  type RendererGpuSubmissionTimerPoll,
} from './gpu-submission-duty.js';

void test('fast completed work retains display-rate automatic submission', () => {
  const driver = new FakeTimerDriver();
  const duty = new RendererGpuSubmissionDuty(driver);

  duty.begin();
  driver.nowMs = 1;
  duty.submitted();
  driver.result = { durationMs: 4, status: 'complete' };
  driver.nowMs = 16;

  assert.equal(duty.ready(), true);
  assert.equal(driver.deleted, 1);
});

void test('slow completed work progressively lowers automatic GPU duty', () => {
  const driver = new FakeTimerDriver();
  const duty = new RendererGpuSubmissionDuty(driver);

  duty.begin();
  driver.nowMs = 1;
  duty.submitted();
  driver.result = { durationMs: 12, status: 'complete' };
  driver.nowMs = 10;

  assert.equal(duty.ready(), false);
  assert.equal(driver.deleted, 1);
  driver.nowMs = 36.9;
  assert.equal(duty.ready(), false);
  driver.nowMs = 37;
  assert.equal(duty.ready(), true);
});

void test('late completion wall time corrects an under-reported timer duration', () => {
  const driver = new FakeTimerDriver();
  const duty = new RendererGpuSubmissionDuty(driver);

  duty.begin();
  duty.submitted();
  driver.result = { durationMs: 4, status: 'complete' };
  driver.nowMs = 33;

  // Thirty-three milliseconds to observe completion leaves sixteen
  // milliseconds of pressure after one ordinary 60 Hz polling allowance.
  // That effective duration targets twenty-five-percent duty: 64 ms total.
  assert.equal(duty.ready(), false);
  assert.equal(driver.deleted, 1);
  assert.deepEqual(duty.sample(), {
    schemaVersion: 1,
    mode: 'timerQuery',
    state: 'waiting',
    timerDurationMs: 4,
    completionAgeMs: 33,
    effectiveDurationMs: 16,
    targetDutyFraction: 0.25,
    admittedAtMs: 64,
    observedAtMs: 33,
  });
  assert.equal(Object.isFrozen(duty.sample()), true);
  driver.nowMs = 63.9;
  assert.equal(duty.ready(), false);
  driver.nowMs = 64;
  assert.equal(duty.ready(), true);
});

void test('completion wall latency paces automatic work without timer-query support', () => {
  const clock = new FakeTimerDriver();
  const duty = new RendererGpuSubmissionDuty(null, clock);

  duty.begin();
  duty.submitted();
  clock.nowMs = 33;

  assert.equal(duty.ready(), false);
  assert.deepEqual(duty.sample(), {
    schemaVersion: 1,
    mode: 'completionOnly',
    state: 'waiting',
    timerDurationMs: null,
    completionAgeMs: 33,
    effectiveDurationMs: 16,
    targetDutyFraction: 0.25,
    admittedAtMs: 64,
    observedAtMs: 33,
  });
  clock.nowMs = 64;
  assert.equal(duty.ready(), true);
});

void test('failed timer query retains completion-wall pacing', () => {
  const driver = new FakeTimerDriver();
  const duty = new RendererGpuSubmissionDuty(driver);

  duty.begin();
  duty.submitted();
  driver.result = { status: 'failed' };
  driver.nowMs = 33;

  assert.equal(duty.ready(), false);
  assert.deepEqual(duty.sample(), {
    schemaVersion: 1,
    mode: 'timerFailed',
    state: 'waiting',
    timerDurationMs: null,
    completionAgeMs: 33,
    effectiveDurationMs: 16,
    targetDutyFraction: 0.25,
    admittedAtMs: 64,
    observedAtMs: 33,
  });
  assert.equal(driver.deleted, 1);
  driver.nowMs = 64;
  assert.equal(duty.ready(), true);
});

void test('completion-derived headroom is bounded for exceptionally slow work', () => {
  const driver = new FakeTimerDriver();
  const duty = new RendererGpuSubmissionDuty(driver);

  duty.begin();
  duty.submitted();
  driver.result = { durationMs: 500, status: 'complete' };
  driver.nowMs = 500;
  assert.equal(duty.ready(), false);
  driver.nowMs = 599.9;
  assert.equal(duty.ready(), false);
  driver.nowMs = 600;
  assert.equal(duty.ready(), true);
});

void test('a pending timer query keeps automatic work bounded', () => {
  const driver = new FakeTimerDriver();
  const duty = new RendererGpuSubmissionDuty(driver);

  duty.begin();
  duty.submitted();
  assert.equal(duty.ready(), false);
  assert.equal(duty.ready(), false);
  driver.result = { durationMs: 3, status: 'complete' };
  driver.nowMs = 16;
  assert.equal(duty.ready(), true);
});

void test('explicit replacement and abort release older query state', () => {
  const driver = new FakeTimerDriver();
  const duty = new RendererGpuSubmissionDuty(driver);

  duty.begin();
  duty.submitted();
  duty.begin();
  assert.equal(driver.deleted, 1);
  duty.aborted();
  assert.equal(driver.ended, 2);
  assert.equal(driver.deleted, 2);
  assert.equal(duty.ready(), true);
  duty.dispose();
  assert.equal(duty.sample().state, 'disposed');
});

void test('unsupported disjoint and exceptional timing degrade without deadlock', () => {
  const unsupported = new RendererGpuSubmissionDuty(null);
  unsupported.begin();
  unsupported.submitted();
  assert.equal(unsupported.ready(), true);

  const disjointDriver = new FakeTimerDriver();
  const disjoint = new RendererGpuSubmissionDuty(disjointDriver);
  disjoint.begin();
  disjoint.submitted();
  disjointDriver.result = { status: 'failed' };
  assert.equal(disjoint.ready(), true);
  assert.equal(disjointDriver.deleted, 1);

  const throwingDriver = new FakeTimerDriver();
  const throwing = new RendererGpuSubmissionDuty(throwingDriver);
  throwing.begin();
  throwing.submitted();
  throwingDriver.throwOnPoll = true;
  assert.equal(throwing.ready(), true);
  assert.equal(throwingDriver.deleted, 1);
});

class FakeTimerDriver implements RendererGpuSubmissionTimerDriver {
  created = 0;
  deleted = 0;
  ended = 0;
  nowMs = 0;
  result: RendererGpuSubmissionTimerPoll = { status: 'pending' };
  throwOnPoll = false;

  begin(): object {
    this.created += 1;
    return { sequence: this.created };
  }

  delete(_query: object): void {
    this.deleted += 1;
  }

  end(_query: object): void {
    this.ended += 1;
  }

  now(): number {
    return this.nowMs;
  }

  poll(_query: object): RendererGpuSubmissionTimerPoll {
    if (this.throwOnPoll) {
      throw new Error('timer poll failed');
    }
    return this.result;
  }
}
