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

void test('accelerated measured deadline starts with enclosed work rather than double-counting submit return', () => {
  const driver = new FakeTimerDriver();
  const duty = new RendererGpuSubmissionDuty(driver, {
    maximumPendingMeasurements: 8,
    rendererClass: 'accelerated',
  });

  driver.nowMs = 100;
  duty.begin();
  // The timer query already encloses GPU work while the synchronous render
  // call is active. Its eight-millisecond duration plus equal headroom targets
  // the next 60 Hz source interval from that start, not from the later return.
  driver.nowMs = 104;
  duty.submitted();
  driver.result = { durationMs: 8, status: 'complete' };
  driver.nowMs = 116;

  assert.equal(duty.ready(), true);
  assert.equal(duty.sample().completionAgeMs, 12);
  assert.equal(duty.sample().effectiveDurationMs, 8);
  assert.equal(duty.sample().admittedAtMs, 116);

  driver.nowMs = 116;
  duty.begin();
  driver.nowMs = 120;
  duty.submitted();
  driver.resultBySequence.set(2, { status: 'pending' });
  driver.nowMs = 131.9;
  assert.equal(duty.ready(), false);
  driver.nowMs = 132;
  assert.equal(duty.ready(), true);
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

void test('accelerated measured work ignores delayed polling while the default fence remains strict', () => {
  const timerDriver = new FakeTimerDriver();
  const fenceDriver = new FakeFenceDriver();
  const duty = new RendererGpuSubmissionDuty(timerDriver, {
    rendererClass: 'accelerated',
  });
  const fence = new RendererGpuSubmissionFence(fenceDriver);

  duty.begin();
  duty.submitted();
  fence.submitted();
  timerDriver.result = { durationMs: 4, status: 'complete' };
  timerDriver.nowMs = 80;

  // Both completion owners advance independently. The measured timer can
  // establish its accelerated deadline, while the exact completion fence still
  // prevents a second automatic submission.
  const fenceReady = fence.ready();
  const dutyReady = duty.ready();
  assert.equal(fenceReady && dutyReady, false);
  assert.equal(dutyReady, true);

  fenceDriver.status = 'signaled';
  assert.equal(fence.ready() && duty.ready(), true);
  assert.equal(fenceDriver.deleted, 1);
  assert.equal(timerDriver.deleted, 1);
  assert.deepEqual(duty.sample(), {
    schemaVersion: 1,
    mode: 'timerQuery',
    state: 'ready',
    rendererClass: 'accelerated',
    timerDurationMs: 4,
    completionAgeMs: 80,
    completionAllowanceMs: 17,
    effectiveDurationMs: 4,
    targetDutyFraction: 0.5,
    admittedAtMs: 8,
    admissionObservedAtMs: 80,
    observedAtMs: 80,
    maximumPendingMeasurements: 1,
    pendingMeasurementCount: 0,
  });
});

void test('accelerated measurements pipeline display-rate work behind a strict fixed cap', () => {
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

  for (let sequence = 1; sequence <= capacity; sequence += 1) {
    assert.equal(fence.ready() && duty.ready(), true);
    duty.begin();
    duty.submitted();
    fence.submitted();
    timerDriver.resultBySequence.set(sequence, { status: 'pending' });
    fenceDriver.statusBySequence.set(sequence, 'pending');
    timerDriver.nowMs += 8;
  }
  assert.equal(fence.ready() && duty.ready(), false);
  assert.equal(duty.sample().maximumPendingMeasurements, capacity);
  assert.equal(duty.sample().pendingMeasurementCount, capacity);
  assert.deepEqual(fence.sample(), {
    schemaVersion: 1,
    mode: 'active',
    maximumPendingSubmissions: capacity,
    pendingSubmissionCount: capacity,
  });

  timerDriver.resultBySequence.set(1, {
    durationMs: 4,
    status: 'complete',
  });
  fenceDriver.statusBySequence.set(1, 'signaled');
  assert.equal(fence.ready() && duty.ready(), true);
  assert.equal(duty.sample().effectiveDurationMs, 4);
  assert.equal(duty.sample().completionAgeMs, 64);
  assert.equal(duty.sample().admittedAtMs, 56);

  duty.begin();
  duty.submitted();
  fence.submitted();
  assert.equal(timerDriver.created, capacity + 1);
  assert.equal(fenceDriver.created, capacity + 1);

  duty.dispose();
  fence.dispose();
  assert.equal(timerDriver.deleted, capacity + 1);
  assert.equal(fenceDriver.deleted, capacity + 1);
});

void test('the timer-query ring stays bounded when accelerated WebGL has no sync fences', () => {
  const timerDriver = new FakeTimerDriver();
  const capacity = 8;
  const duty = new RendererGpuSubmissionDuty(timerDriver, {
    maximumPendingMeasurements: capacity,
    rendererClass: 'accelerated',
  });
  const unsupportedFence = new RendererGpuSubmissionFence(null, {
    maximumPendingSubmissions: capacity,
  });

  for (let sequence = 1; sequence <= capacity; sequence += 1) {
    assert.equal(unsupportedFence.ready(capacity) && duty.ready(), true);
    duty.begin();
    duty.submitted();
    timerDriver.resultBySequence.set(sequence, { status: 'pending' });
    timerDriver.nowMs += 8;
  }
  assert.equal(unsupportedFence.ready(capacity) && duty.ready(), false);
  assert.equal(duty.sample().pendingMeasurementCount, capacity);
  assert.deepEqual(unsupportedFence.sample(), {
    schemaVersion: 1,
    mode: 'unsupported',
    maximumPendingSubmissions: capacity,
    pendingSubmissionCount: 0,
  });

  timerDriver.resultBySequence.set(1, {
    durationMs: 4,
    status: 'complete',
  });
  assert.equal(unsupportedFence.ready(capacity) && duty.ready(), true);
  assert.equal(duty.sample().pendingMeasurementCount, capacity - 1);

  duty.dispose();
  unsupportedFence.dispose();
  assert.equal(timerDriver.deleted, capacity);
});

void test('a completed accelerated measurement paces later ring submissions prospectively', () => {
  const driver = new FakeTimerDriver();
  const duty = new RendererGpuSubmissionDuty(driver, {
    maximumPendingMeasurements: 8,
    rendererClass: 'accelerated',
  });

  duty.begin();
  duty.submitted();
  driver.resultBySequence.set(1, {
    durationMs: 12,
    status: 'complete',
  });
  driver.nowMs = 50;
  assert.equal(duty.ready(), true);
  assert.equal(duty.sample().targetDutyFraction, 1 / 3);

  duty.begin();
  duty.submitted();
  driver.nowMs = 85.9;
  assert.equal(duty.ready(), false);
  driver.nowMs = 86;
  assert.equal(duty.ready(), true);
});

void test('invalid measurement-ring bounds fail before timer mutation', () => {
  const driver = new FakeTimerDriver();
  assert.throws(
    () => new RendererGpuSubmissionDuty(driver, {
      maximumPendingMeasurements: Number.NaN,
      rendererClass: 'accelerated',
    }),
    /maximum pending GPU measurements must be a positive safe integer/,
  );
  assert.equal(driver.created, 0);
});

void test('late completion wall time corrects an under-reported timer duration for an unknown renderer', () => {
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
    rendererClass: 'unknown',
    timerDurationMs: 4,
    completionAgeMs: 33,
    completionAllowanceMs: 17,
    effectiveDurationMs: 16,
    targetDutyFraction: 0.25,
    admittedAtMs: 64,
    admissionObservedAtMs: null,
    observedAtMs: 33,
    maximumPendingMeasurements: 1,
    pendingMeasurementCount: 0,
  });
  assert.equal(Object.isFrozen(duty.sample()), true);
  driver.nowMs = 63.9;
  assert.equal(duty.ready(), false);
  driver.nowMs = 64;
  assert.equal(duty.ready(), true);
});

void test('completion wall latency paces automatic work without timer-query support', () => {
  const clock = new FakeTimerDriver();
  const duty = new RendererGpuSubmissionDuty(null, { clock });

  duty.begin();
  duty.submitted();
  clock.nowMs = 33;

  assert.equal(duty.ready(), false);
  assert.deepEqual(duty.sample(), {
    schemaVersion: 1,
    mode: 'completionOnly',
    state: 'waiting',
    rendererClass: 'unknown',
    timerDurationMs: null,
    completionAgeMs: 33,
    completionAllowanceMs: 17,
    effectiveDurationMs: 16,
    targetDutyFraction: 0.25,
    admittedAtMs: 64,
    admissionObservedAtMs: null,
    observedAtMs: 33,
    maximumPendingMeasurements: 1,
    pendingMeasurementCount: 0,
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
    rendererClass: 'unknown',
    timerDurationMs: null,
    completionAgeMs: 33,
    completionAllowanceMs: 17,
    effectiveDurationMs: 16,
    targetDutyFraction: 0.25,
    admittedAtMs: 64,
    admissionObservedAtMs: null,
    observedAtMs: 33,
    maximumPendingMeasurements: 1,
    pendingMeasurementCount: 0,
  });
  assert.equal(driver.deleted, 1);
  driver.nowMs = 64;
  assert.equal(duty.ready(), true);
});

void test('software rendering treats observed completion age as work pressure', () => {
  const driver = new FakeTimerDriver();
  const duty = new RendererGpuSubmissionDuty(driver, {
    rendererClass: 'software',
  });

  driver.nowMs = 100;
  duty.begin();
  driver.nowMs = 104;
  duty.submitted();
  driver.result = { durationMs: 3, status: 'complete' };
  driver.nowMs = 119;

  assert.equal(duty.ready(), false);
  assert.deepEqual(duty.sample(), {
    schemaVersion: 1,
    mode: 'timerQuery',
    state: 'waiting',
    rendererClass: 'software',
    timerDurationMs: 3,
    completionAgeMs: 15,
    completionAllowanceMs: 0,
    effectiveDurationMs: 15,
    targetDutyFraction: 4 / 15,
    // Software completion backpressure remains anchored to the completed
    // submission wall clock, rather than the accelerated timer-query start.
    admittedAtMs: 160.25,
    admissionObservedAtMs: null,
    observedAtMs: 119,
    maximumPendingMeasurements: 1,
    pendingMeasurementCount: 0,
  });
  driver.nowMs = 160.24;
  assert.equal(duty.ready(), false);
  driver.nowMs = 160.25;
  assert.equal(duty.ready(), true);
});

void test('exceptionally slow work retains equal headroom plus a bounded progressive gap', () => {
  const driver = new FakeTimerDriver();
  const duty = new RendererGpuSubmissionDuty(driver);

  duty.begin();
  duty.submitted();
  driver.result = { durationMs: 500, status: 'complete' };
  driver.nowMs = 500;
  assert.equal(duty.ready(), false);
  assert.equal(duty.sample().targetDutyFraction, 5 / 11);
  assert.equal(duty.sample().admittedAtMs, 1_100);
  driver.nowMs = 1_099.9;
  assert.equal(duty.ready(), false);
  driver.nowMs = 1_100;
  assert.equal(duty.ready(), true);
});

void test('multi-second software completion cannot collapse to a one-hundred-ms gap', () => {
  const driver = new FakeTimerDriver();
  const duty = new RendererGpuSubmissionDuty(driver, {
    rendererClass: 'software',
  });

  duty.begin();
  duty.submitted();
  driver.result = { durationMs: 1_236, status: 'complete' };
  driver.nowMs = 1_257;

  assert.equal(duty.ready(), false);
  assert.equal(duty.sample().effectiveDurationMs, 1_257);
  assert.equal(duty.sample().admittedAtMs, 2_614);
  assert.equal(duty.sample().targetDutyFraction, 1_257 / 2_614);
  driver.nowMs = 1_357;
  assert.equal(duty.ready(), false);
  driver.nowMs = 2_613.9;
  assert.equal(duty.ready(), false);
  driver.nowMs = 2_614;
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
