import assert from 'node:assert/strict';
import test from 'node:test';
import type { RuntimeInputWireEvent } from '@rusty-engine/application-host';
import { createProductBrowserCadence } from './realtime-cadence.js';

test('realtime owner controls advancement without dropping typed cadence input', async () => {
  const input: RuntimeInputWireEvent = {
    runtime: { instanceId: '1', generation: '1', controlRevision: '1' },
    sequence: '1',
    context: 'gameplay.default',
    fact: { kind: 'key', code: 'key-w', edge: 'pressed' },
  };

  const run = async (realtimeAdvanceOwner: 'browser' | 'rust-host') => {
    const inputBatches: Array<readonly RuntimeInputWireEvent[]> = [];
    const observedTimes: string[] = [];
    const failures: unknown[] = [];
    const cadence = createProductBrowserCadence({
      lifecycleMode: 'realtime',
      realtimeAdvanceOwner,
      isReady: () => true,
      enqueueOperation: (operation) => operation(),
      sampleInput: () => [input],
      sendInput: async (batch) => {
        inputBatches.push(batch);
      },
      advanceRealtime: async (observedTimeNs) => {
        observedTimes.push(observedTimeNs);
      },
      admitDemandStep: async () => undefined,
      onFailure: (cause) => {
        failures.push(cause);
      },
    });
    cadence.enqueue(16.5);
    await cadence.settle();
    cadence.dispose();
    return { inputBatches, observedTimes, failures };
  };

  const browser = await run('browser');
  assert.deepEqual(browser.inputBatches, [[input]]);
  assert.deepEqual(browser.observedTimes, ['16500000']);
  assert.deepEqual(browser.failures, []);

  const rustHost = await run('rust-host');
  assert.deepEqual(rustHost.inputBatches, [[input]]);
  assert.deepEqual(rustHost.observedTimes, []);
  assert.deepEqual(rustHost.failures, []);
});

test('input availability wakes static realtime and demand admission without a second loop', async () => {
  const input: RuntimeInputWireEvent = {
    runtime: { instanceId: '1', generation: '1', controlRevision: '1' },
    sequence: '1',
    context: 'gameplay.default',
    intent: 'fixture.regenerate',
    value: {
      kind: 'product-payload',
      contract: 'fixture.regenerate.v1',
      data: { seed: 7, preset: 'spread' },
    },
  };
  const run = async (
    lifecycleMode: 'realtime' | 'demand' | 'external',
    realtimeAdvanceOwner: 'browser' | 'rust-host' = 'browser',
  ) => {
    const batches: Array<readonly RuntimeInputWireEvent[]> = [];
    const advances: string[] = [];
    let demandSteps = 0;
    const cadence = createProductBrowserCadence({
      lifecycleMode,
      realtimeAdvanceOwner,
      isReady: () => true,
      enqueueOperation: (operation) => operation(),
      sampleInput: () => [input],
      sendInput: async (batch) => { batches.push(batch); },
      advanceRealtime: async (time) => { advances.push(time); },
      admitDemandStep: async () => { demandSteps += 1; },
      onFailure: (cause) => { assert.fail(String(cause)); },
    });
    cadence.pulseInput(25);
    await cadence.settle();
    cadence.dispose();
    return { batches, advances, demandSteps };
  };

  assert.deepEqual(await run('realtime'), { batches: [[input]], advances: ['25000000'], demandSteps: 0 });
  assert.deepEqual(await run('realtime', 'rust-host'), { batches: [[input]], advances: [], demandSteps: 0 });
  assert.deepEqual(await run('demand'), { batches: [[input]], advances: [], demandSteps: 1 });
  assert.deepEqual(await run('external'), { batches: [[input]], advances: [], demandSteps: 0 });
});

test('slow cadence coalesces an input wake while ingress preserves ordered edges', async () => {
  const pressed: RuntimeInputWireEvent = {
    runtime: { instanceId: '1', generation: '1', controlRevision: '1' },
    sequence: '1',
    context: 'gameplay.default',
    fact: { kind: 'key', code: 'key-w', edge: 'pressed' },
  };
  const released: RuntimeInputWireEvent = {
    ...pressed,
    sequence: '2',
    fact: { kind: 'key', code: 'key-w', edge: 'released' },
  };
  const queued: RuntimeInputWireEvent[] = [];
  const batches: Array<readonly RuntimeInputWireEvent[]> = [];
  const advances: string[] = [];
  let releaseFirstAdvance: () => void = () => undefined;
  const firstAdvance = new Promise<void>((resolve) => { releaseFirstAdvance = resolve; });
  const cadence = createProductBrowserCadence({
    lifecycleMode: 'realtime',
    realtimeAdvanceOwner: 'browser',
    isReady: () => true,
    enqueueOperation: (operation) => operation(),
    sampleInput: () => queued.splice(0),
    sendInput: async (batch) => { batches.push(batch); },
    advanceRealtime: async (time) => {
      advances.push(time);
      if (advances.length === 1) await firstAdvance;
    },
    admitDemandStep: async () => undefined,
    onFailure: (cause) => { assert.fail(String(cause)); },
  });

  cadence.enqueue(10);
  queued.push(pressed);
  cadence.pulseInput(20);
  for (let time = 61; time <= 100; time += 1) cadence.enqueue(time);
  queued.push(released);
  cadence.pulseInput(120);
  releaseFirstAdvance();
  await cadence.settle();
  cadence.dispose();

  assert.deepEqual(batches, [[pressed, released]]);
  assert.deepEqual(advances, ['10000000', '20000000', '100000000']);
});

test('a renderer cadence before a pending input wake does not drain later input early', async () => {
  const pressed: RuntimeInputWireEvent = {
    runtime: { instanceId: '1', generation: '1', controlRevision: '1' },
    sequence: '1',
    context: 'gameplay.default',
    fact: { kind: 'key', code: 'key-w', edge: 'pressed' },
  };
  const queued: RuntimeInputWireEvent[] = [];
  const batches: Array<readonly RuntimeInputWireEvent[]> = [];
  const advances: string[] = [];
  let samples = 0;
  let releaseFirstAdvance: () => void = () => undefined;
  const firstAdvance = new Promise<void>((resolve) => { releaseFirstAdvance = resolve; });
  const cadence = createProductBrowserCadence({
    lifecycleMode: 'realtime',
    realtimeAdvanceOwner: 'browser',
    isReady: () => true,
    enqueueOperation: (operation) => operation(),
    sampleInput: () => {
      samples += 1;
      return queued.splice(0);
    },
    sendInput: async (batch) => { batches.push(batch); },
    advanceRealtime: async (time) => {
      advances.push(time);
      if (advances.length === 1) await firstAdvance;
    },
    admitDemandStep: async () => undefined,
    onFailure: (cause) => { assert.fail(String(cause)); },
  });

  cadence.enqueue(10);
  cadence.enqueue(100);
  queued.push(pressed);
  cadence.pulseInput(120);
  releaseFirstAdvance();
  await cadence.settle();
  cadence.dispose();

  assert.deepEqual(batches, [[pressed]]);
  assert.deepEqual(advances, ['10000000', '100000000', '120000000']);
  assert.equal(samples, 2);
});

test('a cadence deferred by the serialized lane does not drain a later input wake', async () => {
  const pressed: RuntimeInputWireEvent = {
    runtime: { instanceId: '1', generation: '1', controlRevision: '1' },
    sequence: '1',
    context: 'gameplay.default',
    fact: { kind: 'key', code: 'key-w', edge: 'pressed' },
  };
  const queued: RuntimeInputWireEvent[] = [];
  const batches: Array<readonly RuntimeInputWireEvent[]> = [];
  const advances: string[] = [];
  let samples = 0;
  let releasePriorOperation: () => void = () => undefined;
  const priorOperation = new Promise<void>((resolve) => { releasePriorOperation = resolve; });
  let operationTail: Promise<void> = priorOperation;
  const cadence = createProductBrowserCadence({
    lifecycleMode: 'realtime',
    realtimeAdvanceOwner: 'browser',
    isReady: () => true,
    enqueueOperation: async <T>(operation: () => Promise<T>): Promise<T> => {
      const result = operationTail.then(operation);
      operationTail = result.then(
        () => undefined,
        () => undefined,
      );
      return result;
    },
    sampleInput: () => {
      samples += 1;
      return queued.splice(0);
    },
    sendInput: async (batch) => { batches.push(batch); },
    advanceRealtime: async (time) => { advances.push(time); },
    admitDemandStep: async () => undefined,
    onFailure: (cause) => { assert.fail(String(cause)); },
  });

  cadence.enqueue(10);
  queued.push(pressed);
  cadence.pulseInput(20);
  releasePriorOperation();
  await cadence.settle();
  cadence.dispose();

  assert.deepEqual(batches, [[pressed]]);
  assert.deepEqual(advances, ['10000000', '20000000']);
  assert.equal(samples, 1);
});

test('slow admission keeps ingress overflow recovery bounded after more than 1024 input wakes', async () => {
  const overflowClear: RuntimeInputWireEvent = {
    runtime: { instanceId: '1', generation: '1', controlRevision: '1' },
    sequence: '0',
    context: 'gameplay.default',
    fact: { kind: 'clear', reason: 'ingress-overflow' },
  };
  const laterPressed: RuntimeInputWireEvent = {
    ...overflowClear,
    sequence: '1',
    fact: { kind: 'key', code: 'key-w', edge: 'pressed' },
  };
  const queued: RuntimeInputWireEvent[] = [];
  const batches: Array<readonly RuntimeInputWireEvent[]> = [];
  const advances: string[] = [];
  const failures: unknown[] = [];
  let samples = 0;
  let releaseFirstAdvance: () => void = () => undefined;
  const firstAdvance = new Promise<void>((resolve) => { releaseFirstAdvance = resolve; });
  const cadence = createProductBrowserCadence({
    lifecycleMode: 'realtime',
    realtimeAdvanceOwner: 'browser',
    isReady: () => true,
    enqueueOperation: (operation) => operation(),
    sampleInput: () => {
      samples += 1;
      return queued.splice(0);
    },
    sendInput: async (batch) => { batches.push(batch); },
    advanceRealtime: async (time) => {
      advances.push(time);
      if (advances.length === 1) await firstAdvance;
    },
    admitDemandStep: async () => undefined,
    onFailure: (cause) => { failures.push(cause); },
  });

  cadence.enqueue(10);
  for (let wake = 0; wake < 1_025; wake += 1) cadence.pulseInput(20 + wake);
  // This is the application ingress outcome: overflow replaces stale input
  // with its one clear, and a later physical edge follows it in queue order.
  queued.push(overflowClear, laterPressed);
  assert.equal(samples, 1);
  releaseFirstAdvance();
  await cadence.settle();
  cadence.dispose();

  assert.deepEqual(failures, []);
  assert.deepEqual(batches, [[overflowClear, laterPressed]]);
  assert.deepEqual(advances, ['10000000', '20000000']);
  assert.equal(samples, 2);
});

test('cadence keeps an older same-frame RAF timestamp monotonic after an input wakeup', async () => {
  const pressed: RuntimeInputWireEvent = {
    runtime: { instanceId: '1', generation: '1', controlRevision: '1' },
    sequence: '1',
    context: 'gameplay.default',
    fact: { kind: 'key', code: 'key-a', edge: 'pressed' },
  };
  const queued: RuntimeInputWireEvent[] = [];
  const advances: string[] = [];
  const batches: Array<readonly RuntimeInputWireEvent[]> = [];
  let releaseFirstAdvance: () => void = () => undefined;
  const firstAdvance = new Promise<void>((resolve) => { releaseFirstAdvance = resolve; });
  const cadence = createProductBrowserCadence({
    lifecycleMode: 'realtime',
    realtimeAdvanceOwner: 'browser',
    isReady: () => true,
    enqueueOperation: (operation) => operation(),
    sampleInput: () => queued.splice(0),
    sendInput: async (batch) => { batches.push(batch); },
    advanceRealtime: async (time) => {
      advances.push(time);
      if (advances.length === 1) await firstAdvance;
    },
    admitDemandStep: async () => undefined,
    onFailure: (cause) => { assert.fail(String(cause)); },
  });

  cadence.enqueue(6_720);
  queued.push(pressed);
  cadence.pulseInput(6_721.4);
  cadence.enqueue(6_721.1);
  releaseFirstAdvance();
  await cadence.settle();
  cadence.dispose();

  assert.deepEqual(batches, [[pressed]]);
  assert.deepEqual(advances, ['6720000000', '6721400000', '6721400000']);
});

