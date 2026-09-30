import assert from 'node:assert/strict';
import test from 'node:test';
import type { ProductDevRuntimeOutput, RuntimeInputWireEvent } from './generated/contracts.js';
import {
  bufferProductBrowserPreMountOutput,
  isDroppedClockRegression,
  mountProductBrowserHostWithApplication,
  syncProductBrowserHealthDatasets,
  type ProductBrowserRuntimeAdapter,
  type ProductBrowserRuntimeOutputBatchListener,
  type ProductBrowserRuntimeTerminalFailureListener,
} from './product-browser-host.js';
import { ProductBrowserLocalTransportError } from './local-transport.js';

const ACCEPTED_FAULT = { code: 'DEV_HOST_ACCEPTED', disposition: 'accepted' } as const;
const RUNNING = { instanceId: '7', generation: '1', controlRevision: '2' } as const;

const adapter: ProductBrowserRuntimeAdapter = {
  lifecycle: async (operation) => ({
    accepted: true,
    ...ACCEPTED_FAULT,
    operation: operation.kind,
  }),
  input: async (batch: readonly RuntimeInputWireEvent[]) => ({
    accepted: true,
    ...ACCEPTED_FAULT,
    count: batch.length,
    acceptedCount: batch.length,
    droppedCount: 0,
  }),
  advanceRealtime: async () => ({ accepted: true, ...ACCEPTED_FAULT, operation: 'advance-realtime' as const }),
  admitDemandStep: async () => ({ accepted: true, ...ACCEPTED_FAULT, operation: 'admit-demand-step' as const }),
  subscribeOutputs: () => () => undefined,
  dispose: () => undefined,
};

/** A root the host can mount on without a DOM. */
async function withFakeRoot<T>(run: (root: HTMLElement) => Promise<T>): Promise<T> {
  const previousHTMLElement = globalThis.HTMLElement;
  class FakeElement {
    readonly childNodes: unknown[] = [];
    readonly dataset: Record<string, string> = {};
    readonly appended: { textContent: string; removed: boolean }[] = [];
    readonly ownerDocument = {
      body: this,
      defaultView: { addEventListener: () => undefined, removeEventListener: () => undefined },
      createElement: () => {
        const element = {
          textContent: '', removed: false, style: {},
          setAttribute: () => undefined,
          remove: () => { element.removed = true; },
        };
        return element;
      },
    };
    append(child: { textContent: string; removed: boolean }): void { this.appended.push(child); }
  }
  Object.defineProperty(globalThis, 'HTMLElement', { configurable: true, value: FakeElement });
  try {
    return await run(new FakeElement() as unknown as HTMLElement);
  } finally {
    Object.defineProperty(globalThis, 'HTMLElement', { configurable: true, value: previousHTMLElement });
  }
}

function fakeApplication(input: Record<string, unknown>, projections: unknown[] = []) {
  return {
    ui: {},
    input: { sampleController: () => 0, drain: () => [], ...input },
    uiProjection: {
      ingest: (envelope: unknown) => { projections.push(envelope); },
      bindRuntime: () => undefined,
    },
    readout: () => ({ state: 'ready' }),
    dispose: async () => undefined,
  };
}

test('pre-mount buffering keeps the newest readout and the newest projection per stream', () => {
  const pending: ProductDevRuntimeOutput[] = [];
  const readout = {
    artifact: 'rusty.product.runtime-readout' as const,
    runtime: { instanceId: '1', generation: '1', controlRevision: '1' },
    mode: 'realtime' as const,
    state: 'running' as const,
    admittedSimulationSteps: '1',
    admittedPresentations: '1',
    droppedRealtimeSteps: '0',
    clockRegressions: '0',
    scaledRemainder: 0,
    lastObservedTimeNs: '1',
    fault: null,
  };
  const projection = (stream: string, value: number): ProductDevRuntimeOutput => ({
    kind: 'ui-projection',
    envelope: { runtime: readout.runtime, sequence: String(value), stream, contract: 'hud.v1', value } as never,
  });
  assert.equal(bufferProductBrowserPreMountOutput(pending, { kind: 'runtime-readout', readout }, 2), true);
  assert.equal(bufferProductBrowserPreMountOutput(
    pending,
    { kind: 'runtime-readout', readout: { ...readout, admittedSimulationSteps: '2' } },
    2,
  ), true);
  assert.equal(bufferProductBrowserPreMountOutput(pending, projection('hud', 1), 2), true);
  assert.equal(bufferProductBrowserPreMountOutput(pending, projection('hud', 2), 2), true);
  assert.equal(pending.length, 2);
  assert.equal(pending[0]?.kind === 'runtime-readout' ? pending[0].readout.admittedSimulationSteps : null, '2');
  assert.equal(pending[1]?.kind === 'ui-projection' ? (pending[1].envelope as { value: number }).value : null, 2);
  assert.equal(bufferProductBrowserPreMountOutput(pending, projection('menu', 1), 2), false);
});

test('browser health datasets skip stable attributes', () => {
  const values: Record<string, string> = {};
  let writes = 0;
  const dataset = new Proxy(values, {
    set(target, key, value) {
      writes += 1;
      return Reflect.set(target, key, value);
    },
    deleteProperty(target, key) {
      writes += 1;
      return Reflect.deleteProperty(target, key);
    },
  }) as DOMStringMap;
  const roots = [{ dataset }];
  const health = { state: 'ready' as const, mode: 'realtime' as const, progress: '1', failure: null };
  syncProductBrowserHealthDatasets(roots, health);
  assert.equal(writes, 3);
  syncProductBrowserHealthDatasets(roots, health);
  assert.equal(writes, 3);
  syncProductBrowserHealthDatasets(roots, { ...health, progress: '2' });
  assert.equal(writes, 4);
});

test('only the typed lifecycle clock regression is a dropped cadence observation', () => {
  const dropped = {
    accepted: false,
    code: 'CSHARP_LIFECYCLE_CLOCK_REGRESSION',
    disposition: 'rejected-recoverable' as const,
    operation: 'advance-realtime' as const,
    diagnostic: 'observed clock regressed',
  };
  assert.equal(isDroppedClockRegression(dropped), true);
  assert.equal(isDroppedClockRegression({ ...dropped, code: 'CSHARP_LIFECYCLE_COUNTER_EXHAUSTED' }), false);
  assert.equal(isDroppedClockRegression({ ...dropped, disposition: 'terminal' }), false);
  assert.equal(isDroppedClockRegression({ ...dropped, operation: 'admit-demand-step' }), false);
});

test('a rebinding output rebinds input and the host stays ready', async () => {
  await withFakeRoot(async (root) => {
    const paused = { ...RUNNING, controlRevision: '3' } as const;
    let emit: ProductBrowserRuntimeOutputBatchListener | null = null;
    const boundRuntimes: unknown[] = [];
    const host = await mountProductBrowserHostWithApplication({
      root,
      transport: {
        ...adapter,
        subscribeOutputBatches: (listener) => {
          emit = listener;
          return () => { emit = null; };
        },
      },
      lifecycleMode: 'demand',
      // The runtime-pack shell passes this for every product; demand ignores it.
      realtimeAdvanceOwner: 'rust-host',
      mountUi: async () => undefined,
      autoStart: false,
    }, async () => fakeApplication({ bindRuntime: (binding: unknown) => { boundRuntimes.push(binding); } }) as never);
    const publish = emit as unknown as ProductBrowserRuntimeOutputBatchListener;
    publish([{ kind: 'binding', runtime: RUNNING, nextInputSequence: '4' }], {
      epoch: 1, baseline: false, recovery: 'none',
    });
    publish([{ kind: 'binding', runtime: paused, nextInputSequence: '5' }], {
      epoch: 1, baseline: false, recovery: 'none',
    });
    assert.deepEqual(boundRuntimes.at(-1), {
      runtime: paused, context: 'gameplay.default', nextSequence: '5',
    });
    assert.equal(host.readout().state, 'ready');
    await host.dispose();
  });
});

test('while a harness holds input the page sends none and shows the claim', async () => {
  await withFakeRoot(async (root) => {
    const claimed = { ...RUNNING, controlRevision: '3' } as const;
    const released = { ...RUNNING, controlRevision: '4' } as const;
    let emit: ProductBrowserRuntimeOutputBatchListener | null = null;
    const sent: (readonly RuntimeInputWireEvent[])[] = [];
    const key = (runtime: typeof RUNNING | typeof claimed | typeof released): RuntimeInputWireEvent => ({
      runtime, sequence: '1', context: 'gameplay.default', fact: { kind: 'key', code: 'key-w', edge: 'pressed' },
    });
    let pending: RuntimeInputWireEvent[] = [];
    const host = await mountProductBrowserHostWithApplication({
      root,
      transport: {
        ...adapter,
        input: async (batch: readonly RuntimeInputWireEvent[]) => {
          sent.push(batch);
          return adapter.input(batch);
        },
        subscribeOutputBatches: (listener) => {
          emit = listener;
          return () => { emit = null; };
        },
      },
      lifecycleMode: 'demand',
      mountUi: async () => undefined,
      autoStart: false,
    }, async () => fakeApplication({
      drain: () => { const drained = pending; pending = []; return drained; },
      bindRuntime: () => undefined,
    }) as never);
    const publish = emit as unknown as ProductBrowserRuntimeOutputBatchListener;
    const fake = root as unknown as { appended: { textContent: string; removed: boolean }[] };

    publish([{ kind: 'binding', runtime: claimed, nextInputSequence: '1', inputClaim: 'crew-agent-2' }], {
      epoch: 1, baseline: false, recovery: 'none',
    });
    assert.equal(root.dataset['rustyInputClaim'], 'crew-agent-2');
    assert.equal(fake.appended[0]?.textContent, 'Input held by crew-agent-2');
    pending = [key(claimed)];
    await host.admitDemandStep();
    assert.deepEqual(sent, [], 'no page input while claimed');
    // The harness's input results reach the page too; they keep the claim.
    publish([{
      kind: 'runtime-input-result',
      result: {
        accepted: true, ...ACCEPTED_FAULT, count: 1, acceptedCount: 1, droppedCount: 0,
        binding: claimed, nextInputSequence: '2',
      },
    }], { epoch: 1, baseline: false, recovery: 'none' });
    assert.equal(root.dataset['rustyInputClaim'], 'crew-agent-2');
    pending = [key(claimed)];
    await host.admitDemandStep();
    assert.deepEqual(sent, [], 'an input result does not end the claim');

    publish([{ kind: 'binding', runtime: released, nextInputSequence: '1' }], {
      epoch: 1, baseline: false, recovery: 'none',
    });
    assert.equal(root.dataset['rustyInputClaim'], undefined);
    assert.equal(fake.appended[0]?.removed, true);
    pending = [key(released)];
    await host.admitDemandStep();
    assert.equal(sent.length, 1, 'input resumes once the claim is released');
    await host.dispose();
  });
});


test('a claimed binding that completes input recovery keeps the page from sending', async () => {
  await withFakeRoot(async (root) => {
    const claimed = { ...RUNNING, controlRevision: '3' } as const;
    let emit: ProductBrowserRuntimeOutputBatchListener | null = null;
    let calls = 0;
    let binding: typeof RUNNING | typeof claimed = RUNNING;
    let pending = true;
    const unknown = () => new ProductBrowserLocalTransportError('request_failed', 'lost input response', {
      route: 'input', mutation: { certainty: 'outcome-unknown', outputRecovery: 'none', outputThrough: null },
    });
    const host = await mountProductBrowserHostWithApplication({
      root, lifecycleMode: 'demand', autoStart: false, mountUi: async () => undefined,
      transport: {
        ...adapter,
        input: async (batch) => { calls++; if (calls === 1) throw unknown(); return adapter.input(batch); },
        replaceControl: async () => { throw unknown(); },
        subscribeOutputBatches: listener => { emit = listener; return () => { emit = null; }; },
      },
    }, async () => fakeApplication({
      bindRuntime: () => undefined,
      rebaselineRuntime: (value: { runtime: typeof claimed }) => { binding = value.runtime; },
      drain: () => {
        if (!pending) return [];
        pending = false;
        return [{ runtime: binding, sequence: '1', context: 'gameplay.default',
          fact: { kind: 'key', code: 'key-w', edge: 'pressed' } }];
      },
    }) as never);
    const publish = emit as unknown as ProductBrowserRuntimeOutputBatchListener;
    publish([{ kind: 'binding', runtime: RUNNING, nextInputSequence: '1' }], {
      epoch: 1, baseline: false, recovery: 'none',
    });
    await assert.rejects(host.admitDemandStep());
    await new Promise<void>(resolve => setImmediate(resolve));
    assert.equal(host.readout().state, 'degraded');
    publish([{ kind: 'binding', runtime: claimed, nextInputSequence: '1', inputClaim: 'review-harness' }], {
      epoch: 1, baseline: false, recovery: 'none',
    });
    assert.equal(host.readout().state, 'ready');
    pending = true;
    await host.admitDemandStep();
    const badge = root.dataset['rustyInputClaim'];
    assert.deepEqual({ calls, badge }, { calls: 1, badge: 'review-harness' },
      'fresh harness binding must show its claim and keep the page from sending into it');
    // A later release hands input back to the page.
    const released = { ...RUNNING, controlRevision: '4' } as const;
    binding = released as never;
    publish([{ kind: 'binding', runtime: released, nextInputSequence: '1' }], {
      epoch: 1, baseline: false, recovery: 'none',
    });
    assert.equal(root.dataset['rustyInputClaim'], undefined);
    pending = true;
    await host.admitDemandStep();
    assert.equal(calls, 2, 'page input resumes after the release');
    await host.dispose();
  });
});

test('after an output gap only the fresh baseline is applied', async () => {
  await withFakeRoot(async (root) => {
    let emit: ProductBrowserRuntimeOutputBatchListener | null = null;
    const confirmed: number[] = [];
    const projections: unknown[] = [];
    const host = await mountProductBrowserHostWithApplication({
      root,
      transport: {
        ...adapter,
        subscribeOutputBatches: (listener) => {
          emit = listener;
          return () => { emit = null; };
        },
        confirmOutputBaseline: (epoch) => { confirmed.push(epoch); },
      },
      lifecycleMode: 'demand',
      mountUi: async () => undefined,
      uiProjection: { expectedContract: 'hud.v1' },
      autoStart: false,
    }, async () => fakeApplication({ bindRuntime: () => undefined }, projections) as never);
    const publish = emit as unknown as ProductBrowserRuntimeOutputBatchListener;
    const projection = (value: number): ProductDevRuntimeOutput => ({
      kind: 'ui-projection',
      envelope: { runtime: RUNNING, sequence: String(value), stream: 'hud', contract: 'hud.v1', value } as never,
    });
    publish([], { epoch: 1, baseline: false, recovery: 'fresh-baseline-required' });
    assert.equal(host.readout().state, 'degraded');
    publish([projection(1)], { epoch: 1, baseline: false, recovery: 'none' });
    assert.equal(projections.length, 0, 'an incremental after the gap is not applied');
    publish([{ kind: 'binding', runtime: RUNNING, nextInputSequence: '1' }, projection(2)], {
      epoch: 2, baseline: true, recovery: 'none',
    });
    assert.deepEqual(projections.map((envelope) => (envelope as { value: number }).value), [2]);
    assert.equal(confirmed.at(-1), 2);
    assert.equal(host.readout().state, 'ready');
    publish([projection(3)], { epoch: 1, baseline: false, recovery: 'none' });
    assert.equal(projections.length, 1, 'an output of the replaced attachment is stale');
    await host.dispose();
  });
});

test('host recovers an unknown input batch from a fresh binding after a lost control response', async () => {
  await withFakeRoot(async (root) => {
    const freshRuntime = { ...RUNNING, controlRevision: '3' } as const;
    const inputBatch: RuntimeInputWireEvent = {
      runtime: RUNNING,
      sequence: '4',
      context: 'gameplay.default',
      fact: { kind: 'key', code: 'key-w', edge: 'pressed' },
    };
    let emitOutputs: ProductBrowserRuntimeOutputBatchListener | null = null;
    const baselines: unknown[] = [];
    const boundRuntimes: unknown[] = [];
    let controlAttempts = 0;
    let timelineCalls = 0;
    let inputAvailable = true;
    const unknown = (): ProductBrowserLocalTransportError => new ProductBrowserLocalTransportError(
      'request_failed', 'no response', {
        route: 'input', mutation: { certainty: 'outcome-unknown', outputRecovery: 'none', outputThrough: null },
      },
    );
    const host = await mountProductBrowserHostWithApplication({
      root,
      transport: {
        ...adapter,
        replaceControl: async () => {
          controlAttempts += 1;
          throw unknown();
        },
        input: async () => { throw unknown(); },
        completeTimeline: async () => {
          timelineCalls += 1;
          return { accepted: true as const, ...ACCEPTED_FAULT, ticket: '1' };
        },
        subscribeOutputBatches: (listener) => {
          emitOutputs = listener;
          return () => { emitOutputs = null; };
        },
      },
      lifecycleMode: 'demand',
      mountUi: async () => undefined,
      autoStart: false,
    }, async () => fakeApplication({
      drain: () => {
        if (!inputAvailable) return [];
        inputAvailable = false;
        return [inputBatch];
      },
      bindRuntime: (binding: unknown) => { boundRuntimes.push(binding); },
      rebaselineRuntime: (binding: unknown) => { baselines.push(binding); },
    }) as never);

    const publishOutputs = emitOutputs as unknown as ProductBrowserRuntimeOutputBatchListener;
    publishOutputs([{ kind: 'binding', runtime: RUNNING, nextInputSequence: '4' }], {
      epoch: 1, baseline: false, recovery: 'none',
    });
    assert.deepEqual(boundRuntimes, [{
      runtime: RUNNING,
      context: 'gameplay.default',
      nextSequence: '4',
    }]);
    const demand = host.admitDemandStep();
    const queuedTimeline = host.completeTimeline({} as never);
    await assert.rejects(demand);
    await assert.rejects(queuedTimeline);
    await new Promise<void>((resolve) => setImmediate(resolve));
    assert.equal(host.readout().state, 'degraded');
    assert.equal(controlAttempts, 1, 'the lost control response remains in the one recovery episode');
    assert.equal(timelineCalls, 0, 'a queued mutation is cancelled at execution after recovery begins');
    publishOutputs([{ kind: 'binding', runtime: freshRuntime, nextInputSequence: '1' }], {
      epoch: 1, baseline: false, recovery: 'none',
    });
    assert.equal(host.readout().state, 'ready');
    assert.deepEqual(baselines, [{
      runtime: freshRuntime,
      context: 'gameplay.default',
      nextSequence: '1',
    }]);
    // A delayed admission receipt for the uncertain old epoch is ignored.
    publishOutputs([{
      kind: 'runtime-input-result',
      result: {
        accepted: true,
        ...ACCEPTED_FAULT,
        count: 1,
        acceptedCount: 1,
        droppedCount: 0,
        binding: RUNNING,
        nextInputSequence: '5',
      },
    }], { epoch: 1, baseline: false, recovery: 'none' });
    assert.equal(baselines.length, 1);
    assert.equal(boundRuntimes.length, 1, 'late old input result cannot rebind the fresh control revision');
    await host.dispose();
  });
});

for (const terminalFirst of [false, true]) {
  test(`terminal transport failure remains authoritative when input rejects ${terminalFirst ? 'after' : 'before'} closure`, async () => {
    await withFakeRoot(async (root) => {
      let emitTerminal: ProductBrowserRuntimeTerminalFailureListener = () => assert.fail('not subscribed');
      let rejectInput: (cause: unknown) => void = () => assert.fail('input not started');
      const inputResponse = new Promise<never>((_resolve, reject) => { rejectInput = reject; });
      let inputCalls = 0;
      let controlCalls = 0;
      let disposed = 0;
      const unknown = new ProductBrowserLocalTransportError('request_failed', 'earlier input NetworkError', {
        route: 'input', mutation: { certainty: 'outcome-unknown', outputRecovery: 'none', outputThrough: null },
      });
      let inputAvailable = true;
      const host = await mountProductBrowserHostWithApplication({
        root,
        transport: {
          ...adapter,
          input: () => { inputCalls += 1; return inputResponse; },
          replaceControl: async () => { controlCalls += 1; throw unknown; },
          subscribeTerminalFailures: (listener) => {
            emitTerminal = listener;
            return () => undefined;
          },
          dispose: () => { disposed += 1; },
        },
        lifecycleMode: 'demand',
        mountUi: async () => undefined,
        autoStart: false,
      }, async () => fakeApplication({
        drain: () => {
          if (!inputAvailable) return [];
          inputAvailable = false;
          return [{ runtime: RUNNING, sequence: '1', context: 'gameplay.default',
            fact: { kind: 'key', code: 'key-w', edge: 'pressed' } }];
        },
      }) as never);
      const pending = assert.rejects(host.admitDemandStep());
      await new Promise<void>((resolve) => setImmediate(resolve));
      assert.equal(inputCalls, 1);
      const terminate = () => emitTerminal({ kind: 'runtime-failure', diagnostic: 'fresh output stream failed' });
      if (terminalFirst) terminate();
      rejectInput(unknown);
      await pending;
      await new Promise<void>((resolve) => setImmediate(resolve));
      if (!terminalFirst) {
        assert.equal(host.readout().state, 'degraded');
        assert.equal(host.readout().lastFailure, 'earlier input NetworkError');
        terminate();
      }
      assert.equal(host.readout().state, 'failed');
      assert.equal(host.readout().lastFailure, 'fresh output stream failed');
      assert.equal(controlCalls, terminalFirst ? 0 : 1);
      assert.equal(inputCalls, 1, 'uncertain input is never replayed');
      assert.equal(disposed, 1);
      await host.dispose();
    });
  });
}
