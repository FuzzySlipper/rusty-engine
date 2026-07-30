import assert from 'node:assert/strict';
import test from 'node:test';

import {
  RendererGpuSubmissionFence,
  type RendererGpuSubmissionFenceDriver,
  type RendererGpuSubmissionFencePoll,
} from './gpu-submission-fence.js';

void test('automatic GPU submission stays blocked until the exact fence signals', () => {
  const driver = new FakeFenceDriver();
  const fence = new RendererGpuSubmissionFence(driver);

  assert.equal(fence.ready(), true);
  fence.submitted();
  assert.equal(driver.created, 1);
  assert.equal(driver.flushed, 1);
  assert.equal(fence.ready(), false);
  assert.equal(fence.ready(), false);
  assert.deepEqual(fence.sample(), {
    schemaVersion: 1,
    mode: 'active',
    maximumPendingSubmissions: 1,
    pendingSubmissionCount: 1,
  });
  driver.status = 'signaled';
  assert.equal(fence.ready(), true);
  assert.equal(driver.deleted, 1);
  assert.equal(fence.ready(), true);
});

void test('a later explicit submission replaces the fence and covers prior work', () => {
  const driver = new FakeFenceDriver();
  const fence = new RendererGpuSubmissionFence(driver);

  fence.submitted();
  fence.submitted();
  assert.equal(driver.created, 2);
  assert.equal(driver.deleted, 1);
  assert.equal(fence.ready(), false);
  fence.dispose();
  assert.equal(driver.deleted, 2);
  assert.equal(fence.ready(), true);
});

void test('accelerated automatic work uses a fixed completion ring without becoming unbounded', () => {
  const driver = new FakeFenceDriver();
  const fence = new RendererGpuSubmissionFence(driver, {
    maximumPendingSubmissions: 3,
  });

  for (let sequence = 1; sequence <= 3; sequence += 1) {
    assert.equal(fence.ready(), true);
    fence.submitted();
    driver.statusBySequence.set(sequence, 'pending');
  }
  assert.equal(fence.ready(), false);
  assert.equal(driver.created, 3);

  driver.statusBySequence.set(1, 'signaled');
  assert.equal(fence.ready(), true);
  fence.submitted();
  assert.equal(driver.created, 4);
  assert.equal(fence.ready(), false);

  fence.dispose();
  assert.equal(driver.deleted, 4);
});

void test('a runtime timer fallback can restore the strict one-fence admission limit', () => {
  const driver = new FakeFenceDriver();
  const fence = new RendererGpuSubmissionFence(driver, {
    maximumPendingSubmissions: 3,
  });

  fence.submitted();
  fence.submitted();
  assert.equal(fence.ready(), true);
  assert.equal(fence.ready(1), false);

  driver.statusBySequence.set(1, 'signaled');
  assert.equal(fence.ready(1), false);
  driver.statusBySequence.set(2, 'signaled');
  assert.equal(fence.ready(1), true);
});

void test('invalid completion-ring bounds fail before any driver mutation', () => {
  const driver = new FakeFenceDriver();
  assert.throws(
    () => new RendererGpuSubmissionFence(driver, {
      maximumPendingSubmissions: 0,
    }),
    /maximum pending GPU submissions must be a positive safe integer/,
  );
  assert.equal(driver.created, 0);
});

void test('unsupported and failed fences degrade without deadlocking rendering', () => {
  const unsupported = new RendererGpuSubmissionFence(null);
  unsupported.submitted();
  assert.equal(unsupported.ready(), true);
  assert.deepEqual(unsupported.sample(), {
    schemaVersion: 1,
    mode: 'unsupported',
    maximumPendingSubmissions: 1,
    pendingSubmissionCount: 0,
  });

  const driver = new FakeFenceDriver();
  driver.status = 'failed';
  const failed = new RendererGpuSubmissionFence(driver);
  failed.submitted();
  assert.equal(failed.ready(), true);
  assert.equal(driver.deleted, 1);
  assert.equal(failed.sample().mode, 'disabled');
});

void test('driver exceptions disable optional pacing without escaping or leaking', () => {
  const pollFailure = new FakeFenceDriver();
  const pollFence = new RendererGpuSubmissionFence(pollFailure);
  pollFence.submitted();
  pollFailure.throwOnPoll = true;
  assert.equal(pollFence.ready(), true);
  assert.equal(pollFailure.deleted, 1);
  pollFence.submitted();
  assert.equal(pollFailure.created, 1);

  const flushFailure = new FakeFenceDriver();
  flushFailure.throwOnFlush = true;
  const flushFence = new RendererGpuSubmissionFence(flushFailure);
  flushFence.submitted();
  assert.equal(flushFailure.created, 1);
  assert.equal(flushFailure.deleted, 1);
  assert.equal(flushFence.ready(), true);
});

class FakeFenceDriver implements RendererGpuSubmissionFenceDriver {
  status: RendererGpuSubmissionFencePoll = 'pending';
  readonly statusBySequence = new Map<number, RendererGpuSubmissionFencePoll>();
  created = 0;
  deleted = 0;
  flushed = 0;
  throwOnFlush = false;
  throwOnPoll = false;

  create(): object {
    this.created += 1;
    return { sequence: this.created };
  }

  delete(_fence: object): void {
    this.deleted += 1;
  }

  flush(): void {
    if (this.throwOnFlush) {
      throw new Error('driver flush failed');
    }
    this.flushed += 1;
  }

  poll(fence: object): RendererGpuSubmissionFencePoll {
    if (this.throwOnPoll) {
      throw new Error('driver poll failed');
    }
    const sequence = (fence as { readonly sequence: number }).sequence;
    return this.statusBySequence.get(sequence) ?? this.status;
  }
}
