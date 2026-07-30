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
  driver.nowMs = 36.9;

  assert.equal(duty.ready(), false);
  assert.equal(driver.deleted, 1);
  driver.nowMs = 37;
  assert.equal(duty.ready(), true);
});

void test('completion-derived headroom is bounded for exceptionally slow work', () => {
  const driver = new FakeTimerDriver();
  const duty = new RendererGpuSubmissionDuty(driver);

  duty.begin();
  duty.submitted();
  driver.result = { durationMs: 500, status: 'complete' };
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
