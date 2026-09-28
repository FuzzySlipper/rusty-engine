import assert from 'node:assert/strict';
import test from 'node:test';

import {
  RendererGpuSubmissionDuty,
  type RendererGpuSubmissionTimerDriver,
  type RendererGpuSubmissionTimerPoll,
} from './gpu-submission-duty.js';
import {
  RendererGpuSubmissionFence,
  type RendererGpuSubmissionFenceDriver,
  type RendererGpuSubmissionFencePoll,
} from './gpu-submission-fence.js';

const FRAME_MS = 16;

void test('twelve-millisecond accelerated work submits every animation frame', () => {
  // The former duty governor admitted this workload only every 36 ms.
  const timerDriver = new FakeTimerDriver();
  const fenceDriver = new FakeFenceDriver();
  const capacity = 8;
  const duty = new RendererGpuSubmissionDuty(timerDriver, {
    maximumPendingMeasurements: capacity,
    rendererClass: 'accelerated',
  });
  const fence = new RendererGpuSubmissionFence(fenceDriver, {
    maximumPendingSubmissions: capacity,
  });
  timerDriver.result = { durationMs: 12, status: 'complete' };
  fenceDriver.status = 'signaled';

  let submitted = 0;
  for (let frame = 0; frame < 10; frame += 1) {
    timerDriver.nowMs = frame * FRAME_MS;
    duty.observe();
    if (fence.ready(capacity)) {
      duty.begin();
      duty.submitted();
      fence.submitted();
      submitted += 1;
    }
  }
  assert.equal(submitted, 10);
  assert.equal(duty.sample().timerDurationMs, 12);
  assert.equal(duty.sample().effectiveDurationMs, 12);
});

void test('without a timer the completion age is the measured duration', () => {
  const duty = new RendererGpuSubmissionDuty(null, {
    clock: { now: () => clock },
    rendererClass: 'software',
  });
  let clock = 100;
  duty.begin();
  clock = 101;
  duty.submitted();
  clock = 130;
  duty.observe();
  assert.equal(duty.sample().mode, 'completionOnly');
  assert.equal(duty.sample().timerDurationMs, null);
  assert.equal(duty.sample().completionAgeMs, 29);
  assert.equal(duty.sample().effectiveDurationMs, 29);
});

void test('the measurement ring stays bounded by discarding the oldest query', () => {
  const driver = new FakeTimerDriver();
  const duty = new RendererGpuSubmissionDuty(driver, {
    maximumPendingMeasurements: 2,
    rendererClass: 'accelerated',
  });
  for (let sequence = 0; sequence < 5; sequence += 1) {
    duty.begin();
    duty.submitted();
    duty.observe();
  }
  assert.equal(duty.sample().pendingMeasurementCount, 2);
  assert.equal(driver.created, 5);
  assert.equal(driver.deleted, 3);
  duty.dispose();
  assert.equal(driver.deleted, 5);
});

void test('invalid measurement-ring bounds fail before timer mutation', () => {
  const driver = new FakeTimerDriver();
  assert.throws(
    () => new RendererGpuSubmissionDuty(driver, { maximumPendingMeasurements: 0 }),
    /maximum pending GPU measurements must be a positive safe integer/u,
  );
  assert.equal(driver.created, 0);
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
  duty.dispose();
  assert.equal(duty.sample().state, 'disposed');
});

void test('unsupported, disjoint and throwing timers degrade to completion age', () => {
  const unsupported = new RendererGpuSubmissionDuty(null);
  unsupported.begin();
  unsupported.submitted();
  unsupported.observe();
  assert.equal(unsupported.sample().mode, 'completionOnly');

  const disjointDriver = new FakeTimerDriver();
  const disjoint = new RendererGpuSubmissionDuty(disjointDriver);
  disjoint.begin();
  disjoint.submitted();
  disjointDriver.result = { status: 'failed' };
  disjoint.observe();
  assert.equal(disjoint.sample().mode, 'timerFailed');
  assert.equal(disjointDriver.deleted, 1);

  const throwingDriver = new FakeTimerDriver();
  const throwing = new RendererGpuSubmissionDuty(throwingDriver);
  throwing.begin();
  throwing.submitted();
  throwingDriver.throwOnPoll = true;
  throwing.observe();
  assert.equal(throwing.sample().mode, 'timerFailed');
  assert.equal(throwingDriver.deleted, 1);
});

class FakeTimerDriver implements RendererGpuSubmissionTimerDriver {
  created = 0;
  deleted = 0;
  ended = 0;
  nowMs = 0;
  result: RendererGpuSubmissionTimerPoll = { status: 'pending' };
  readonly resultBySequence = new Map<number, RendererGpuSubmissionTimerPoll>();
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

  poll(query: object): RendererGpuSubmissionTimerPoll {
    if (this.throwOnPoll) {
      throw new Error('timer poll failed');
    }
    const sequence = (query as { readonly sequence: number }).sequence;
    return this.resultBySequence.get(sequence) ?? this.result;
  }
}

class FakeFenceDriver implements RendererGpuSubmissionFenceDriver {
  status: RendererGpuSubmissionFencePoll = 'pending';
  readonly statusBySequence = new Map<number, RendererGpuSubmissionFencePoll>();
  created = 0;
  deleted = 0;
  flushed = 0;

  create(): object {
    this.created += 1;
    return { sequence: this.created };
  }

  delete(_fence: object): void {
    this.deleted += 1;
  }

  flush(): void {
    this.flushed += 1;
  }

  poll(fence: object): RendererGpuSubmissionFencePoll {
    const sequence = (fence as { readonly sequence: number }).sequence;
    return this.statusBySequence.get(sequence) ?? this.status;
  }
}
