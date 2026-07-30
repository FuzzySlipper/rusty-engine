import assert from 'node:assert/strict';
import test from 'node:test';

import {
  RendererSurfaceReadinessPoll,
  type RendererSurfaceReadinessPollScheduler,
} from './surface-readiness-poll.js';

void test('accelerated readiness advances in one bounded non-rendering burst', () => {
  const scheduler = new FakeReadinessScheduler();
  let pollCount = 0;
  let readyCount = 0;
  const poll = new RendererSurfaceReadinessPoll({
    isAccelerated: () => true,
    isReady: () => {
      pollCount += 1;
      return pollCount === 3;
    },
    onReady: () => {
      readyCount += 1;
    },
    scheduler,
  });

  poll.request();
  poll.request();
  assert.deepEqual(scheduler.delays, [0]);

  scheduler.runNext();
  assert.deepEqual(scheduler.delays, [0, 1]);
  scheduler.runNext();
  assert.deepEqual(scheduler.delays, [0, 1, 2]);
  scheduler.runNext();

  assert.equal(pollCount, 3);
  assert.equal(readyCount, 1);
  assert.equal(scheduler.pendingCount(), 0);
});

void test('software and unknown readiness remain RAF-owned', () => {
  for (const _rendererClass of ['software', 'unknown'] as const) {
    const scheduler = new FakeReadinessScheduler();
    let polled = false;
    const poll = new RendererSurfaceReadinessPoll({
      isAccelerated: () => false,
      isReady: () => {
        polled = true;
        return true;
      },
      onReady: () => {
        throw new Error('non-accelerated readiness must not be notified');
      },
      scheduler,
    });

    poll.request();

    assert.equal(scheduler.pendingCount(), 0);
    assert.equal(polled, false);
  }
});

void test('accelerated readiness polling is bounded and cancellation removes stale work', () => {
  const scheduler = new FakeReadinessScheduler();
  let pollCount = 0;
  const poll = new RendererSurfaceReadinessPoll({
    isAccelerated: () => true,
    isReady: () => {
      pollCount += 1;
      return false;
    },
    onReady: () => {
      throw new Error('permanently pending readiness must not be notified');
    },
    scheduler,
  });

  poll.request();
  scheduler.runAll();

  assert.equal(pollCount, 6);
  assert.deepEqual(scheduler.delays, [0, 1, 2, 4, 4, 4]);
  assert.equal(scheduler.pendingCount(), 0);

  poll.request();
  assert.equal(scheduler.pendingCount(), 1);
  poll.cancel();
  assert.equal(scheduler.pendingCount(), 0);
  assert.equal(scheduler.cancelled, 1);
});

class FakeReadinessScheduler implements RendererSurfaceReadinessPollScheduler {
  readonly delays: number[] = [];
  cancelled = 0;
  #nextHandle = 1;
  readonly #pending = new Map<number, () => void>();

  request(callback: () => void, delayMs: number): () => void {
    const handle = this.#nextHandle;
    this.#nextHandle += 1;
    this.delays.push(delayMs);
    this.#pending.set(handle, callback);
    return () => {
      if (this.#pending.delete(handle)) {
        this.cancelled += 1;
      }
    };
  }

  pendingCount(): number {
    return this.#pending.size;
  }

  runAll(): void {
    while (this.#pending.size > 0) {
      this.runNext();
    }
  }

  runNext(): void {
    const next = this.#pending.entries().next().value as
      | readonly [number, () => void]
      | undefined;
    if (next === undefined) {
      throw new Error('no readiness callback is pending');
    }
    const [handle, callback] = next;
    this.#pending.delete(handle);
    callback();
  }
}
