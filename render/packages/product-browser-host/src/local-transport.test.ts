import assert from 'node:assert/strict';
import test from 'node:test';
import {
  PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH,
  ProductBrowserLocalTransportError,
  createProductBrowserLocalHttpAdapter,
  type ProductBrowserLocalEventSource,
} from './local-transport.js';
import type { RustyApplicationRuntimeInputEnvelope } from '@rusty-engine/application-host';

const RUNTIME = { instanceId: '7', generation: '1', controlRevision: '2' } as const;
const ACCEPTED_FAULT = { code: 'DEV_HOST_ACCEPTED', disposition: 'accepted' } as const;
const READOUT = {
  artifact: 'rusty.product.runtime-readout',
  runtime: RUNTIME,
  mode: 'realtime',
  state: 'running',
  admittedSimulationSteps: '1',
  admittedPresentations: '0',
  droppedRealtimeSteps: '0',
  clockRegressions: '0',
  scaledRemainder: 0,
  lastObservedTimeNs: '100',
  fault: null,
} as const;

class FakeEventSource implements ProductBrowserLocalEventSource {
  static readonly instances: FakeEventSource[] = [];
  readonly namedListeners = new Map<string, (event: { readonly data: string; readonly lastEventId: string }) => void>();
  readonly url: string;
  onopen: ((event: unknown) => void) | null = null;
  onmessage: ((event: { readonly data: string; readonly lastEventId: string }) => void) | null = null;
  onerror: ((event: unknown) => void) | null = null;
  closed = false;
  nextEventId = 1;
  messageDeliveryCallbacks = 0;

  constructor(url: string) {
    this.url = url;
    FakeEventSource.instances.push(this);
  }

  close(): void {
    this.closed = true;
  }

  addEventListener(type: 'rusty-output-lag' | 'rusty-output-fragment' | 'rusty-output-baseline', listener: (event: { readonly data: string; readonly lastEventId: string }) => void): void {
    this.namedListeners.set(type, listener);
  }

  removeEventListener(type: 'rusty-output-lag' | 'rusty-output-fragment' | 'rusty-output-baseline', listener: (event: { readonly data: string; readonly lastEventId: string }) => void): void {
    if (this.namedListeners.get(type) === listener) this.namedListeners.delete(type);
  }

  emit(output: unknown, lastEventId = String(this.nextEventId++)): void {
    const listener = this.onmessage;
    if (listener === null) return;
    this.messageDeliveryCallbacks += 1;
    listener({ data: JSON.stringify(output), lastEventId });
  }

  open(): void {
    this.onopen?.({});
  }

  emitLag(value: unknown = { code: 'DEV_HOST_OUTPUT_LAG' }): void {
    this.namedListeners.get('rusty-output-lag')?.({
      data: JSON.stringify(value),
      lastEventId: String(this.nextEventId++),
    });
  }

  emitFragment(value: unknown, lastEventId = String(this.nextEventId++)): void {
    this.namedListeners.get('rusty-output-fragment')?.({ data: JSON.stringify(value), lastEventId });
  }

  emitBaseline(value: unknown, lastEventId = String(this.nextEventId++)): void {
    this.namedListeners.get('rusty-output-baseline')?.({ data: JSON.stringify(value), lastEventId });
  }
}

function response(body: unknown, status = 200, headers: Record<string, string> = {}): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: {
      'content-type': 'application/json',
      'x-rusty-commit-disposition': 'committed',
      ...headers,
    },
  });
}

function result(operation: string): Record<string, unknown> {
  return {
    accepted: true,
    ...ACCEPTED_FAULT,
    operation,
    binding: RUNTIME,
    nextInputSequence: '1',
    readout: READOUT,
  };
}

function completeConnectionBaseline(stream: FakeEventSource): void {
  stream.emit({ kind: 'binding', runtime: RUNTIME, nextInputSequence: '1' }, '');
  stream.emitBaseline(result('connect'), '');
  stream.nextEventId = 1;
}

test('same-origin local transport uses fixed typed operation routes and SSE outputs', async () => {
  FakeEventSource.instances.length = 0;
  const routes: string[] = [];
  const transportErrors: unknown[] = [];
  const batches: RustyApplicationRuntimeInputEnvelope[][] = [];
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async (input, init) => {
      const url = new URL(String(input), 'http://product.local/');
      routes.push(`${init?.method ?? 'GET'} ${url.pathname}`);
      const body = init?.body === undefined ? null : JSON.parse(String(init.body)) as Record<string, unknown>;
      switch (url.pathname) {
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}lifecycle/start`:
          return response(result('start'));
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}input`:
          batches.push([...(body?.['batch'] as readonly RustyApplicationRuntimeInputEnvelope[])]);
          return response({ accepted: true, ...ACCEPTED_FAULT, count: (body?.['batch'] as readonly unknown[]).length, binding: RUNTIME, readout: READOUT });
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}advance-realtime`:
          assert.equal(body?.['observedTimeNs'], '100');
          return response(result('advance-realtime'));
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}admit-demand-step`:
          return response(result('admit-demand-step'));
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}admit-external-step`:
          assert.equal(body?.['step'], '1');
          return response(result('admit-external-step'));
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}timeline-completion`:
          assert.equal(body?.['ticket'], '1');
          return response({ accepted: true, ...ACCEPTED_FAULT, ticket: '1', binding: RUNTIME, readout: READOUT });
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}audio-feedback`:
          assert.deepEqual(body, {
            runtime: RUNTIME,
            replaceOwner: true,
            evictedFactCount: '2',
            facts: [{
              kind: 'naturalCompletion', source: 'oneShot', factId: '7', sequence: 3, signalHandle: '11',
            }],
          });
          return response({ accepted: true, ...ACCEPTED_FAULT, runtime: RUNTIME, acceptedThroughFactId: '7' });
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}animation-feedback`:
          assert.deepEqual(body, {
            runtime: RUNTIME,
            replaceOwner: true,
            evictedFactCount: '0',
            facts: [{
              kind: 'diagnostic', factId: '8', objectId: null, generation: null,
              code: 'assetMissing', sequence: 4,
            }],
          });
          return response({ accepted: true, ...ACCEPTED_FAULT, runtime: RUNTIME, acceptedThroughFactId: '8' });
        default:
          return response({ error: 'missing route' }, 404);
      }
    },
    eventSource: FakeEventSource,
    onTransportError: (error) => transportErrors.push(error),
  });

  const outputs: unknown[] = [];
  const unsubscribe = adapter.subscribeOutputs((output) => outputs.push(output));
  const throwingUnsubscribe = adapter.subscribeOutputs(() => { throw new Error('listener probe'); });
  const isolatedOutputs: unknown[] = [];
  const isolatedUnsubscribe = adapter.subscribeOutputs((output) => isolatedOutputs.push(output));
  assert.equal(FakeEventSource.instances[0]?.url, `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}outputs/fresh`);
  let outputSubscriptionReady = false;
  const readiness = adapter.waitUntilOutputSubscriptionReady?.().then(() => {
    outputSubscriptionReady = true;
  });
  await Promise.resolve();
  assert.equal(outputSubscriptionReady, false);
  FakeEventSource.instances[0]!.open();
  await readiness;
  assert.equal(outputSubscriptionReady, true);
  FakeEventSource.instances[0]!.emit({ kind: 'binding', runtime: RUNTIME, nextInputSequence: '1' }, '');
  FakeEventSource.instances[0]!.emit({ kind: 'runtime-readout', readout: READOUT }, '');
  assert.equal(outputs.length, 0);
  assert.equal(isolatedOutputs.length, 0);
  FakeEventSource.instances[0]!.onerror?.(new Error('transient stream failure'));
  assert.equal(FakeEventSource.instances[0]!.closed, false);
  FakeEventSource.instances[0]!.emit({ kind: 'binding', runtime: RUNTIME, nextInputSequence: '1' }, '');
  FakeEventSource.instances[0]!.emit({ kind: 'runtime-readout', readout: READOUT }, '');
  assert.equal(outputs.length, 0);
  assert.equal(isolatedOutputs.length, 0);
  const connection = adapter.connect?.();
  FakeEventSource.instances[0]!.emitBaseline(result('connect'), '');
  assert.equal((await connection)?.operation, 'connect');
  assert.equal(outputs.length, 2);
  assert.equal(isolatedOutputs.length, 2);
  assert.equal(transportErrors.length, 3);
  const lifecycle = await adapter.lifecycle({ kind: 'start' });
  assert.deepEqual(lifecycle.binding, RUNTIME);
  assert.equal(lifecycle.nextInputSequence, '1');
  assert.equal((await adapter.input([])).count, 0);
  assert.equal((await adapter.advanceRealtime('100')).operation, 'advance-realtime');
  assert.equal((await adapter.admitDemandStep?.())?.operation, 'admit-demand-step');
  assert.equal((await adapter.admitExternalStep?.('1'))?.operation, 'admit-external-step');
  assert.equal((await adapter.completeTimeline?.({
    ticket: '1',
    runtime: RUNTIME,
    correlation: 'request-1',
    outcome: { kind: 'success' },
    provenance: { correlation: 'request-1' },
  }))?.ticket, '1');
  assert.deepEqual(await adapter.reportAudioFeedback({
    runtime: RUNTIME,
    replaceOwner: true,
    evictedFactCount: '2',
    facts: [{
      kind: 'naturalCompletion', source: 'oneShot', factId: '7', sequence: 3, signalHandle: '11',
    }],
  }), { accepted: true, ...ACCEPTED_FAULT, runtime: RUNTIME, acceptedThroughFactId: '7' });
  assert.deepEqual(await adapter.reportAnimationFeedback({
    runtime: RUNTIME,
    replaceOwner: true,
    evictedFactCount: '0',
    facts: [{
      kind: 'diagnostic', factId: '8', objectId: null, generation: null,
      code: 'assetMissing', sequence: 4,
    }],
  }), { accepted: true, ...ACCEPTED_FAULT, runtime: RUNTIME, acceptedThroughFactId: '8' });
  assert.throws(
    () => adapter.completeTimeline?.({
      ticket: '01',
      runtime: RUNTIME,
      correlation: 'request-1',
      outcome: { kind: 'success' },
      provenance: { correlation: 'request-1' },
    }),
    (error: unknown) => error instanceof ProductBrowserLocalTransportError
      && error.code === 'invalid_options',
  );
  assert.throws(
    () => adapter.completeTimeline?.({
      ticket: '1',
      runtime: RUNTIME,
      correlation: 'request-1',
      outcome: { kind: 'success', data: { entries: Array.from({ length: 129 }, () => 1) } },
      provenance: { correlation: 'request-1' },
    }),
    (error: unknown) => error instanceof TypeError,
  );
  assert.throws(
    () => adapter.completeTimeline?.({
      ticket: '1',
      runtime: RUNTIME,
      correlation: 'request-1',
      outcome: { kind: 'success' },
      provenance: { correlation: 'different' },
    }),
    (error: unknown) => error instanceof TypeError,
  );
  assert.deepEqual(routes, [
    'POST /__rusty/product/runtime/lifecycle/start',
    'POST /__rusty/product/runtime/input',
    'POST /__rusty/product/runtime/advance-realtime',
    'POST /__rusty/product/runtime/admit-demand-step',
    'POST /__rusty/product/runtime/admit-external-step',
    'POST /__rusty/product/runtime/timeline-completion',
    'POST /__rusty/product/runtime/audio-feedback',
    'POST /__rusty/product/runtime/animation-feedback',
  ]);
  assert.equal(batches.length, 1);
  unsubscribe();
  throwingUnsubscribe();
  isolatedUnsubscribe();
  assert.equal(FakeEventSource.instances[0]!.closed, true);
  adapter.dispose();
  await assert.rejects(
    adapter.advanceRealtime('101'),
    (error: unknown) => error instanceof ProductBrowserLocalTransportError && error.code === 'disposed',
  );
});

test('local transport distinguishes an unknown mutation outcome from an HTTP rejection', async () => {
  const unavailable = createProductBrowserLocalHttpAdapter({
    fetch: async () => { throw new TypeError('Failed to fetch'); },
    eventSource: FakeEventSource,
  });
  await assert.rejects(
    unavailable.lifecycle({ kind: 'start' }),
    (error: unknown) => error instanceof ProductBrowserLocalTransportError
      && error.code === 'request_failed'
      && error.mutation.certainty === 'outcome-unknown'
      && error.mutation.outputRecovery === 'none'
      && error.mutation.outputThrough === null
      && error.route === 'lifecycle/start',
  );

  const rejected = createProductBrowserLocalHttpAdapter({
    fetch: async () => response({ error: 'unavailable' }, 503),
    eventSource: FakeEventSource,
  });
  await assert.rejects(
    rejected.lifecycle({ kind: 'start' }),
    (error: unknown) => error instanceof ProductBrowserLocalTransportError
      && error.code === 'request_failed'
      && error.mutation.certainty === 'not-applied',
  );
});

test('rejected runtime recovery facts remain decoded result facts rather than transport failures', async () => {
  const recovery = {
    mutation: 'not-applied',
    invalidatedScope: 'none',
    nextAction: 'continue',
  } as const;
  const rejected = {
    accepted: false,
    code: 'CSHARP_NEW_SOURCE_REJECTION',
    disposition: 'rejected-recoverable',
    recovery,
    diagnostic: 'runtime rejected before admission',
  } as const;
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async (input) => {
      const pathname = new URL(String(input), 'http://product.local/').pathname;
      switch (pathname) {
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}advance-realtime`:
          return response({ ...rejected, operation: 'advance-realtime' });
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}input`:
          return response({ ...rejected, count: 0, acceptedCount: 0, droppedCount: 0 });
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}audio-feedback`:
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}animation-feedback`:
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}ghost-plate-feedback`:
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}renderer-diagnostics`:
          return response({ ...rejected, runtime: RUNTIME });
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}timeline-completion`:
          return response({ ...rejected, ticket: '1' });
        default:
          throw new Error(`unexpected route ${pathname}`);
      }
    },
    eventSource: FakeEventSource,
  });

  assert.deepEqual((await adapter.advanceRealtime('1')).recovery, recovery);
  assert.deepEqual((await adapter.input([])).recovery, recovery);
  assert.deepEqual((await adapter.reportAudioFeedback({
    runtime: RUNTIME, replaceOwner: false, evictedFactCount: '0', facts: [],
  })).recovery, recovery);
  assert.deepEqual((await adapter.reportAnimationFeedback({
    runtime: RUNTIME, replaceOwner: false, evictedFactCount: '0', facts: [],
  })).recovery, recovery);
  assert.deepEqual((await adapter.reportGhostPlateFeedback({
    runtime: RUNTIME, replaceOwner: false, facts: [],
  })).recovery, recovery);
  assert.deepEqual((await adapter.reportRendererDiagnostics?.({
    runtime: RUNTIME, snapshot: { schemaVersion: 1 },
  } as never))?.recovery, recovery);
  assert.deepEqual((await adapter.completeTimeline?.({
    ticket: '1', runtime: RUNTIME, correlation: 'request-1', outcome: { kind: 'success' },
    provenance: { correlation: 'request-1' },
  }))?.recovery, recovery);
  adapter.dispose();
});

test('local transport exposes only the fixed control-replace recovery fence', async () => {
  const requests: Array<{ readonly url: string; readonly body: string | null }> = [];
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async (url, init) => {
      requests.push({ url: String(url), body: typeof init?.body === 'string' ? init.body : null });
      return response(result('replace-control'));
    },
    eventSource: FakeEventSource,
  });
  assert.deepEqual(await adapter.replaceControl?.(RUNTIME), result('replace-control'));
  assert.deepEqual(requests, [{
    url: `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}control/replace`,
    body: JSON.stringify({ runtime: RUNTIME }),
  }]);
  adapter.dispose();
});

test('local transport preserves committed output headers for #7761 when the response body truncates', async () => {
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => new Response(new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(new TextEncoder().encode('{"accepted":true'));
        controller.error(new TypeError('truncated response body'));
      },
    }), {
      headers: {
        'content-type': 'application/json',
        'x-rusty-commit-disposition': 'committed',
        'x-rusty-output-through': '7',
      },
    }),
    eventSource: FakeEventSource,
  });
  await assert.rejects(
    adapter.lifecycle({ kind: 'start' }),
    (error: unknown) => error instanceof ProductBrowserLocalTransportError
      && error.code === 'request_failed'
      && error.mutation.certainty === 'committed'
      && error.mutation.outputRecovery === 'none'
      && error.mutation.outputThrough === '7'
      && error.route === 'lifecycle/start',
  );
  adapter.dispose();
});

test('local transport marks a successful response with no commit boundary outcome-unknown', async () => {
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => new Response(JSON.stringify(result('start')), {
      headers: { 'content-type': 'application/json' },
    }),
    eventSource: FakeEventSource,
  });
  await assert.rejects(
    adapter.lifecycle({ kind: 'start' }),
    (error: unknown) => error instanceof ProductBrowserLocalTransportError
      && error.code === 'response_decode_failed'
      && error.mutation.certainty === 'outcome-unknown'
      && error.mutation.outputThrough === null,
  );
  adapter.dispose();
});

test('cursorless disconnect replaces the baseline in a new output epoch', async () => {
  FakeEventSource.instances.length = 0;
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => response(result('advance-realtime')),
    eventSource: FakeEventSource,
  });
  const outputs: unknown[] = [];
  const batches: { readonly outputs: readonly unknown[]; readonly metadata: unknown }[] = [];
  const unsubscribe = adapter.subscribeOutputs((output) => outputs.push(output));
  const unsubscribeBatches = adapter.subscribeOutputBatches?.((output, metadata) => {
    batches.push({ outputs: [...output], metadata });
  });
  const stream = FakeEventSource.instances[0]!;
  stream.open();
  const connection = adapter.connect?.();
  stream.emit({
    kind: 'binding', runtime: RUNTIME, nextInputSequence: '1',
    publicationFrontiers: [{ stream: 'voxel:active', revision: 1 }],
  }, '');
  stream.emit({ kind: 'runtime-readout', readout: READOUT }, '');
  stream.emitBaseline(result('connect'), '');
  await connection;
  assert.equal(outputs.length, 2);

  stream.onerror?.(new Error('cursorless reconnect'));
  assert.equal(stream.closed, true);
  assert.equal(FakeEventSource.instances.length, 2);
  const replacement = FakeEventSource.instances[1]!;
  replacement.emit({
    kind: 'binding', runtime: RUNTIME, nextInputSequence: '1',
    publicationFrontiers: [{ stream: 'voxel:active', revision: 2 }],
  }, '');
  replacement.emit({ kind: 'runtime-readout', readout: READOUT }, '');
  replacement.emitBaseline(result('connect'), '');
  assert.equal(outputs.length, 4);
  assert.deepEqual(batches.map((batch) => batch.metadata), [
    { epoch: 1, baseline: true, recovery: 'none' },
    { epoch: 1, baseline: false, recovery: 'fresh-baseline-required' },
    { epoch: 2, baseline: true, recovery: 'none' },
  ]);

  replacement.emit({
    kind: 'frame',
    frame: {
      schemaVersion: 1,
      publication: { stream: 'voxel:active', baseRevision: 2, revision: 3, operationCount: 0 },
      ops: [],
    },
  }, '1');
  assert.equal(outputs.length, 5);
  unsubscribeBatches?.();
  unsubscribe();
  adapter.dispose();
});

test('local transport decodes Rust-host realtime progress output', () => {
  FakeEventSource.instances.length = 0;
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => response({}),
    eventSource: FakeEventSource,
  });
  const outputs: unknown[] = [];
  const unsubscribe = adapter.subscribeOutputs((output) => outputs.push(output));
  const stream = FakeEventSource.instances[0]!;
  completeConnectionBaseline(stream);
  stream.emit({ kind: 'runtime-progress', owner: 'rust-host' }, '1');
  assert.deepEqual(outputs.at(-1), { kind: 'runtime-progress', owner: 'rust-host' });
  unsubscribe();
  adapter.dispose();
});

test('one runtime output batch is decoded and delivered through one batch callback', () => {
  FakeEventSource.instances.length = 0;
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => response({}),
    eventSource: FakeEventSource,
  });
  const received: unknown[][] = [];
  const unsubscribe = adapter.subscribeOutputBatches?.((outputs) => received.push([...outputs]));
  const stream = FakeEventSource.instances[0]!;
  completeConnectionBaseline(stream);
  stream.emit({
    kind: 'runtime-output-batch',
    outputs: [
      { kind: 'runtime-readout', readout: READOUT },
      { kind: 'runtime-progress', owner: 'rust-host' },
    ],
  }, '1');
  assert.equal(received.length, 2);
  assert.deepEqual(received[1]?.map((output) => (output as { kind: string }).kind), [
    'runtime-readout',
    'runtime-progress',
  ]);
  unsubscribe?.();
  adapter.dispose();
});

test('sixty hertz receipt stream parses once and preserves five-output order per receipt', () => {
  const TICKS = 60;
  const OUTPUTS_PER_RECEIPT = 5;
  const expectedKinds = [
    'frame',
    'view-composition',
    'ui-projection',
    'runtime-readout',
    'runtime-progress',
  ];
  FakeEventSource.instances.length = 0;
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => response({}),
    eventSource: FakeEventSource,
  });
  const batchKinds: string[][] = [];
  const outputKinds: string[] = [];
  const unsubscribeBatches = adapter.subscribeOutputBatches?.((outputs) => {
    batchKinds.push(outputs.map((output) => output.kind));
  });
  const unsubscribeOutputs = adapter.subscribeOutputs((output) => {
    outputKinds.push(output.kind);
  });
  const stream = FakeEventSource.instances[0]!;
  completeConnectionBaseline(stream);
  // Exclude the unnumbered binding baseline from the steady-state receipt
  // counters below. The actual stream begins at the first retained receipt.
  batchKinds.length = 0;
  outputKinds.length = 0;
  stream.messageDeliveryCallbacks = 0;

  const originalJsonParse = JSON.parse;
  let jsonParseCalls = 0;
  JSON.parse = ((...args: Parameters<typeof JSON.parse>) => {
    jsonParseCalls += 1;
    return originalJsonParse(...args);
  }) as typeof JSON.parse;
  try {
    for (let tick = 0; tick < TICKS; tick += 1) {
      stream.emit({
        kind: 'runtime-output-batch',
        outputs: [
          { kind: 'frame', frame: { tick } },
          {
            kind: 'view-composition',
            composition: {
              schemaVersion: 1,
              cameras: [],
              targets: [],
              views: [],
              presentations: [],
            },
          },
          {
            kind: 'ui-projection',
            envelope: {
              artifact: 'rusty.product.ui-projection',
              runtime: RUNTIME,
              sequence: String(tick),
              stream: 'product.ui',
              contract: 'runtime.tick.v1',
              value: { tick },
            },
          },
          {
            kind: 'runtime-readout',
            readout: {
              ...READOUT,
              admittedSimulationSteps: String(tick + 1),
              lastObservedTimeNs: String(tick + 1),
            },
          },
          { kind: 'runtime-progress', owner: 'rust-host' },
        ],
      });
    }
  } finally {
    JSON.parse = originalJsonParse;
  }

  assert.equal(stream.messageDeliveryCallbacks, TICKS);
  assert.equal(jsonParseCalls, TICKS);
  assert.equal(batchKinds.length, TICKS);
  assert.deepEqual(batchKinds, Array.from({ length: TICKS }, () => expectedKinds));
  assert.equal(outputKinds.length, TICKS * OUTPUTS_PER_RECEIPT);
  assert.deepEqual(
    outputKinds,
    Array.from({ length: TICKS }, () => expectedKinds).flat(),
  );
  assert.equal(
    TICKS * OUTPUTS_PER_RECEIPT,
    300,
    'the old one-callback-per-output stream would deliver about 300 typed outputs',
  );
  unsubscribeBatches?.();
  unsubscribeOutputs();
  adapter.dispose();
});

test('local transport decodes scheduled input receipts with authoritative progress and recovery cursors', () => {
  FakeEventSource.instances.length = 0;
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => response({}),
    eventSource: FakeEventSource,
  });
  const outputs: unknown[] = [];
  const unsubscribe = adapter.subscribeOutputs((output) => outputs.push(output));
  const stream = FakeEventSource.instances[0]!;
  completeConnectionBaseline(stream);

  stream.emit({
    kind: 'runtime-input-result',
    result: {
      accepted: true,
      code: 'DEV_HOST_ACCEPTED',
      disposition: 'accepted',
      count: 2,
      acceptedCount: 2,
      droppedCount: 0,
      acceptedThrough: '4',
      consumedThrough: '4',
      nextInputSequence: '5',
      binding: RUNTIME,
      readout: READOUT,
    },
  }, '1');
  assert.deepEqual(outputs.at(-1), {
    kind: 'runtime-input-result',
    result: {
      accepted: true,
      code: 'DEV_HOST_ACCEPTED',
      disposition: 'accepted',
      count: 2,
      acceptedCount: 2,
      droppedCount: 0,
      acceptedThrough: '4',
      consumedThrough: '4',
      nextInputSequence: '5',
      binding: RUNTIME,
      readout: READOUT,
    },
  });

  stream.emit({
    kind: 'runtime-input-result',
    result: {
      accepted: false,
      code: 'CSHARP_INPUT_STALE_DROPPED',
      disposition: 'rejected-recoverable',
      count: 2,
      acceptedCount: 1,
      droppedCount: 1,
      acceptedThrough: '6',
      consumedThrough: '7',
      nextInputSequence: '8',
      binding: RUNTIME,
      readout: READOUT,
      diagnostic: 'dropped one stale input event',
    },
  }, '2');
  assert.deepEqual((outputs.at(-1) as { readonly result: Record<string, unknown> }).result, {
    accepted: false,
    code: 'CSHARP_INPUT_STALE_DROPPED',
    disposition: 'rejected-recoverable',
    count: 2,
    acceptedCount: 1,
    droppedCount: 1,
    acceptedThrough: '6',
    consumedThrough: '7',
    nextInputSequence: '8',
    binding: RUNTIME,
    readout: READOUT,
    diagnostic: 'dropped one stale input event',
  });

  stream.emit({
    kind: 'runtime-input-result',
    result: {
      accepted: false,
      code: 'DEV_HOST_INPUT_MAILBOX_FULL',
      disposition: 'resync-required',
      count: 2,
      acceptedCount: 0,
      droppedCount: 2,
      nextInputSequence: '9',
      binding: RUNTIME,
      readout: READOUT,
      diagnostic: 'input mailbox requires a fresh binding',
    },
  }, '3');
  assert.deepEqual((outputs.at(-1) as { readonly result: Record<string, unknown> }).result, {
    accepted: false,
    code: 'DEV_HOST_INPUT_MAILBOX_FULL',
    disposition: 'resync-required',
    count: 2,
    acceptedCount: 0,
    droppedCount: 2,
    nextInputSequence: '9',
    binding: RUNTIME,
    readout: READOUT,
    diagnostic: 'input mailbox requires a fresh binding',
  });

  unsubscribe();
  adapter.dispose();
});

test('local transport accepts a host-bounded decode-resync receipt without widening outgoing input batches', async () => {
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => response({
      accepted: false,
      code: 'DEV_HOST_INPUT_DECODE',
      disposition: 'resync-required',
      count: 1_025,
      acceptedCount: 0,
      droppedCount: 1_025,
      diagnostic: 'input binding was resynchronized after strict decode rejection',
    }),
    eventSource: FakeEventSource,
  });

  assert.deepEqual(await adapter.input([]), {
    accepted: false,
    code: 'DEV_HOST_INPUT_DECODE',
    disposition: 'resync-required',
    count: 1_025,
    acceptedCount: 0,
    droppedCount: 1_025,
    diagnostic: 'input binding was resynchronized after strict decode rejection',
  });
  assert.throws(
    () => adapter.input(Array.from({ length: 1_025 }, () => ({})) as never),
    (error: unknown) => error instanceof ProductBrowserLocalTransportError
      && error.code === 'invalid_options',
  );
  adapter.dispose();
});

test('operation response waits for its exact retained-output cursor', async () => {
  FakeEventSource.instances.length = 0;
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => response(result('start'), 200, { 'x-rusty-output-through': '2' }),
    eventSource: FakeEventSource,
  });
  const outputs: unknown[] = [];
  adapter.subscribeOutputs((output) => outputs.push(output));
  const stream = FakeEventSource.instances[0]!;
  completeConnectionBaseline(stream);
  const operation = adapter.lifecycle({ kind: 'start' });
  let settled = false;
  void operation.then(() => { settled = true; });
  await Promise.resolve();
  assert.equal(settled, false);
  stream.emit({ kind: 'runtime-readout', readout: READOUT }, '2');
  assert.equal((await operation).operation, 'start');
  assert.equal(settled, true);
  assert.equal(outputs.length, 2);
  adapter.dispose();
});

test('a committed output boundary joins a fresh baseline when its old cursor is lost', async () => {
  FakeEventSource.instances.length = 0;
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => response(result('start'), 200, { 'x-rusty-output-through': '2' }),
    eventSource: FakeEventSource,
  });
  adapter.subscribeOutputs(() => undefined);
  const first = FakeEventSource.instances[0]!;
  completeConnectionBaseline(first);
  const operation = adapter.lifecycle({ kind: 'start' });
  let settled = false;
  void operation.then(() => { settled = true; });
  await Promise.resolve();
  first.emitLag();
  assert.equal(FakeEventSource.instances.length, 2);
  await new Promise<void>((resolve) => setImmediate(resolve));
  assert.equal(settled, false, 'the replacement stream alone does not settle the old committed boundary');
  completeConnectionBaseline(FakeEventSource.instances[1]!);
  assert.equal((await operation).operation, 'start');
  adapter.dispose();
});

test('resync-required commit reconnects the fresh output baseline without replaying the operation', async () => {
  FakeEventSource.instances.length = 0;
  let operations = 0;
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => {
      operations += 1;
      return response(result('start'), 200, {
        'x-rusty-commit-disposition': 'resync-required',
        'x-rusty-resync-outputs': 'fresh',
      });
    },
    eventSource: FakeEventSource,
  });
  const outputs: unknown[] = [];
  adapter.subscribeOutputs((output) => outputs.push(output));
  const first = FakeEventSource.instances[0]!;
  completeConnectionBaseline(first);

  const operation = adapter.lifecycle({ kind: 'start' });
  await new Promise<void>((resolve) => setImmediate(resolve));
  assert.equal(operations, 1);
  assert.equal(first.closed, true);
  assert.equal(FakeEventSource.instances.length, 2);
  const fresh = FakeEventSource.instances[1]!;
  assert.equal(fresh.url, `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}outputs/fresh`);
  completeConnectionBaseline(fresh);

  assert.equal((await operation).operation, 'start');
  assert.equal(operations, 1, 'fresh output resync never replays the operation request');
  assert.equal(outputs.length, 2);
  adapter.dispose();
});

test('a truncated committed resync response refreshes output before surfacing its known mutation failure', async () => {
  FakeEventSource.instances.length = 0;
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => new Response(new ReadableStream<string>({
      start(controller) {
        controller.error(new TypeError('truncated response body'));
      },
    }), {
      headers: {
        'content-type': 'application/json',
        'x-rusty-commit-disposition': 'resync-required',
        'x-rusty-resync-outputs': 'fresh',
      },
    }),
    eventSource: FakeEventSource,
  });
  const failures: unknown[] = [];
  adapter.subscribeTerminalFailures?.((failure) => failures.push(failure));
  adapter.subscribeOutputs(() => undefined);
  completeConnectionBaseline(FakeEventSource.instances[0]!);
  const operation = adapter.lifecycle({ kind: 'start' });
  await new Promise<void>((resolve) => setImmediate(resolve));
  assert.equal(FakeEventSource.instances.length, 2);
  completeConnectionBaseline(FakeEventSource.instances[1]!);
  await assert.rejects(
    operation,
    (error: unknown) => error instanceof ProductBrowserLocalTransportError
      && error.mutation.certainty === 'committed'
      && error.mutation.outputRecovery === 'fresh-baseline-required',
  );
  assert.deepEqual(failures, []);
  adapter.dispose();
});

test('concurrent resync-required receipts share one fresh output connection', async () => {
  FakeEventSource.instances.length = 0;
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async (input) => response(result(String(input).endsWith('admit-demand-step')
      ? 'admit-demand-step'
      : 'advance-realtime'), 200, {
      'x-rusty-commit-disposition': 'resync-required',
      'x-rusty-resync-outputs': 'fresh',
    }),
    eventSource: FakeEventSource,
  });
  adapter.subscribeOutputs(() => undefined);
  completeConnectionBaseline(FakeEventSource.instances[0]!);

  const first = adapter.advanceRealtime('1');
  const second = adapter.admitDemandStep?.();
  await new Promise<void>((resolve) => setImmediate(resolve));
  assert.equal(FakeEventSource.instances.length, 2);
  completeConnectionBaseline(FakeEventSource.instances[1]!);
  await Promise.all([first, second]);
  adapter.dispose();
});

test('incoherent commit headers remain outcome-unknown for recovery fencing', async () => {
  let first = true;
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => {
      if (first) {
        first = false;
        return response(result('start'), 200, {
          'x-rusty-commit-disposition': 'committed',
          'x-rusty-resync-outputs': 'fresh',
        });
      }
      return response(result('advance-realtime'));
    },
    eventSource: FakeEventSource,
  });
  await assert.rejects(
    adapter.lifecycle({ kind: 'start' }),
    (error: unknown) => error instanceof ProductBrowserLocalTransportError
      && error.mutation.certainty === 'outcome-unknown',
  );
  assert.equal((await adapter.advanceRealtime('1')).operation, 'advance-realtime');
  adapter.dispose();
});

test('output cursor mismatch replaces the projection and ignores late old-stream output', () => {
  for (const staleId of ['1', '0']) {
    FakeEventSource.instances.length = 0;
    const outputs: unknown[] = [];
    const batches: Array<{ readonly outputs: unknown[]; readonly recovery: string; readonly baseline: boolean }> = [];
    const adapter = createProductBrowserLocalHttpAdapter({
      fetch: async () => response({}),
      eventSource: FakeEventSource,
    });
    adapter.subscribeOutputBatches?.((batch, metadata) => batches.push({
      outputs: [...batch], recovery: metadata?.recovery ?? 'none', baseline: metadata?.baseline ?? false,
    }));
    adapter.subscribeOutputs((output) => outputs.push(output));
    const stream = FakeEventSource.instances[0]!;
    completeConnectionBaseline(stream);
    stream.emit({ kind: 'runtime-readout', readout: READOUT }, '1');
    stream.emit({ kind: 'runtime-readout', readout: READOUT }, staleId);
    assert.equal(outputs.length, 2);
    assert.equal(FakeEventSource.instances.length, 2);
    assert.equal(stream.closed, true);
    const fresh = FakeEventSource.instances[1]!;
    completeConnectionBaseline(fresh);
    // FakeEventSource deliberately still invokes a callback after close: the
    // browser-local epoch must fence that stale delivery.
    stream.emit({ kind: 'runtime-readout', readout: { ...READOUT, admittedSimulationSteps: '99' } }, '2');
    fresh.emit({ kind: 'runtime-readout', readout: READOUT }, '1');
    assert.equal(outputs.length, 4);
    assert.equal(batches.some((batch) => batch.recovery === 'fresh-baseline-required'), true);
    assert.equal(batches.some((batch) => batch.baseline), true);
    adapter.dispose();
  }
});

test('large retained output fragments publish once after complete ordered reassembly', () => {
  FakeEventSource.instances.length = 0;
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => response({}),
    eventSource: FakeEventSource,
  });
  const batches: unknown[][] = [];
  adapter.subscribeOutputBatches?.((outputs) => batches.push([...outputs]));
  const stream = FakeEventSource.instances[0]!;
  completeConnectionBaseline(stream);
  const encoded = JSON.stringify({
    kind: 'runtime-output-batch',
    outputs: [
      { kind: 'frame', frame: { payload: 'x'.repeat(300_000) } },
      { kind: 'runtime-progress', owner: 'rust-host' },
    ],
  });
  const chunks = encoded.match(/[\s\S]{1,98304}/gu)!;
  chunks.forEach((data, fragmentIndex) => stream.emitFragment({
    schemaVersion: 1,
    transferId: '1',
    runtime: RUNTIME,
    fragmentIndex,
    fragmentCount: chunks.length,
    aggregateBytes: new TextEncoder().encode(encoded).byteLength,
    data,
  }));
  assert.equal(batches.length, 2);
  assert.deepEqual(batches[1]?.map((output) => (output as { kind: string }).kind), ['frame', 'runtime-progress']);
  assert.equal(((batches[1]?.[0] as { frame: { payload: string } }).frame.payload).length, 300_000);
  adapter.dispose();
});

test('corrupt completed fragment replacement does not publish the corrupt batch', () => {
  FakeEventSource.instances.length = 0;
  const outputs: unknown[] = [];
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => response({}),
    eventSource: FakeEventSource,
  });
  adapter.subscribeOutputs((output) => outputs.push(output));
  const stream = FakeEventSource.instances[0]!;
  completeConnectionBaseline(stream);
  const encoded = JSON.stringify({ kind: 'frame', frame: { payload: 'x'.repeat(300_000) } });
  const chunks = encoded.match(/[\s\S]{1,98304}/gu)!;
  chunks.forEach((data, fragmentIndex) => stream.emitFragment({
    schemaVersion: 1,
    transferId: '1',
    runtime: RUNTIME,
    fragmentIndex,
    fragmentCount: chunks.length,
    aggregateBytes: new TextEncoder().encode(encoded).byteLength,
    data,
  }, fragmentIndex === chunks.length - 1 ? String(fragmentIndex) : String(fragmentIndex + 2)));
  assert.equal(outputs.length, 1);
  assert.equal(FakeEventSource.instances.length, 2);
  assert.equal(stream.closed, true);
  adapter.dispose();
});

test('output fragments recover through a fresh baseline when an active projection is corrupt', () => {
  const cases: readonly ((stream: FakeEventSource) => void)[] = [
    (stream) => stream.emitFragment(fragment({ fragmentIndex: 1 })),
    (stream) => {
      const first = fragment();
      stream.emitFragment(first);
      stream.emitFragment(first);
    },
    (stream) => stream.emitFragment(fragment({ runtime: { ...RUNTIME, generation: '2' } })),
    (stream) => stream.emitFragment(fragment({ aggregateBytes: 16 * 1024 * 1024 + 1 })),
    (stream) => {
      stream.emitFragment(fragment());
      stream.emit({ kind: 'runtime-readout', readout: READOUT });
    },
  ];
  for (const exercise of cases) {
    FakeEventSource.instances.length = 0;
    const adapter = createProductBrowserLocalHttpAdapter({
      fetch: async () => response({}),
      eventSource: FakeEventSource,
    });
    const outputs: unknown[] = [];
    adapter.subscribeOutputs((output) => outputs.push(output));
    const stream = FakeEventSource.instances[0]!;
    completeConnectionBaseline(stream);
    exercise(stream);
    assert.equal(outputs.length, 1);
    assert.equal(FakeEventSource.instances.length, 2);
    assert.equal(stream.closed, true);
    adapter.dispose();
  }
});

function fragment(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    schemaVersion: 1,
    transferId: '1',
    runtime: RUNTIME,
    fragmentIndex: 0,
    fragmentCount: 3,
    aggregateBytes: 300_000,
    data: 'x'.repeat(98_304),
    ...overrides,
  };
}

test('local transport rejects malformed typed output and bounded paths', () => {
  const errors: ProductBrowserLocalTransportError[] = [];
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => response({ accepted: true, ...ACCEPTED_FAULT, operation: 'advance-realtime' }),
    eventSource: FakeEventSource,
    onTransportError: (error) => errors.push(error),
  });
  adapter.subscribeOutputs(() => undefined);
  const stream = FakeEventSource.instances.at(-1);
  assert.ok(stream);
  stream.emit({ kind: 'runtime-readout', readout: { artifact: 'wrong' } });
  assert.equal(errors[0]?.code, 'output_decode_failed');
  adapter.dispose();
  assert.throws(
    () => createProductBrowserLocalHttpAdapter({
      fetch: async () => response({}),
      eventSource: FakeEventSource,
      basePath: '/../runtime/',
    }),
    (error: unknown) => error instanceof ProductBrowserLocalTransportError && error.code === 'invalid_options',
  );
  assert.throws(
    () => createProductBrowserLocalHttpAdapter({
      fetch: async () => response({}),
      eventSource: FakeEventSource,
      basePath: '//other-origin/runtime/',
    }),
    (error: unknown) => error instanceof ProductBrowserLocalTransportError && error.code === 'invalid_options',
  );
});

test('local transport rejects missing or incoherent host fault facts', async () => {
  const missingFault = createProductBrowserLocalHttpAdapter({
    fetch: async () => response({ accepted: true, operation: 'advance-realtime' }),
    eventSource: FakeEventSource,
  });
  await assert.rejects(missingFault.advanceRealtime('1'), /operation result code/u);

  const incoherentFault = createProductBrowserLocalHttpAdapter({
    fetch: async () => response({
      accepted: false,
      code: 'CSHARP_CONTROL_BINDING',
      disposition: 'accepted',
      operation: 'advance-realtime',
      diagnostic: 'stale binding',
    }),
    eventSource: FakeEventSource,
  });
  await assert.rejects(
    incoherentFault.advanceRealtime('1'),
    /accepted and disposition are incoherent/u,
  );
});

test('ordinary outputs honor the configured quota below the hard event bound', () => {
  FakeEventSource.instances.length = 0;
  const errors: ProductBrowserLocalTransportError[] = [];
  const outputs: unknown[] = [];
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => response({}),
    eventSource: FakeEventSource,
    maximumOutputBytes: 1,
    onTransportError: (error) => errors.push(error),
  });
  adapter.subscribeOutputs((output) => outputs.push(output));
  const stream = FakeEventSource.instances[0];
  assert.ok(stream);
  stream.emit({ kind: 'binding', runtime: RUNTIME, nextInputSequence: '1' });
  assert.equal(outputs.length, 0);
  assert.equal(errors.length, 1);
  assert.equal(errors[0]?.code, 'output_decode_failed');
  adapter.dispose();
});

test('named output lag asks for one fresh baseline without closing the runtime transport', async () => {
  FakeEventSource.instances.length = 0;
  const transportErrors: unknown[] = [];
  const requestBodies: unknown[] = [];
  const requestUrls: string[] = [];
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async (input, init) => {
      requestUrls.push(String(input));
      requestBodies.push(JSON.parse(String(init?.body)) as unknown);
      if (String(input).endsWith('advance-realtime')) return response(result('advance-realtime'));
      return response({ accepted: true, reported: 1 });
    },
    eventSource: FakeEventSource,
    onTransportError: (error) => transportErrors.push(error),
  });
  const hostFailures: unknown[] = [];
  const unsubscribeFailure = adapter.subscribeTerminalFailures?.((failure) => hostFailures.push(failure));
  const unsubscribeOutput = adapter.subscribeOutputs(() => undefined);
  const stream = FakeEventSource.instances[0];
  assert.ok(stream);
  completeConnectionBaseline(stream);
  stream.emitLag();
  assert.equal(stream.closed, true);
  assert.deepEqual(hostFailures, []);
  assert.equal(transportErrors.length, 1);
  assert.equal(FakeEventSource.instances.length, 2);
  completeConnectionBaseline(FakeEventSource.instances[1]!);

  // The bounded browser-health route stays usable during a projection swap.
  const terminalReport = {
    hostState: 'degraded' as const,
    runtimeProgress: '9',
    transportState: 'open' as const,
    outputState: 'open' as const,
    lastRendererSequence: '60',
    rendererObservationAgeMs: '100',
    firstTerminal: { code: 'BROWSER_HOST_TRANSPORT_FAILED', message: 'output stream lagged' },
    pageEvents: [],
  };
  await adapter.reportBrowserDiagnostics?.(terminalReport);
  assert.deepEqual(requestUrls, [`${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}browser-diagnostics`]);
  assert.deepEqual(requestBodies, [terminalReport]);

  await adapter.advanceRealtime('1');
  unsubscribeFailure?.();
  unsubscribeOutput();
  adapter.dispose();
});

test('local transport hardens the JSON border before requests', async () => {
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async (_input, init) => {
      assert.equal(new Headers(init?.headers).get('content-type'), 'application/json');
      return response({ accepted: true, ...ACCEPTED_FAULT, operation: 'advance-realtime' });
    },
    eventSource: FakeEventSource,
  });
  class ArraySubclass extends Array<unknown> {}
  assert.throws(
    () => adapter.input(new ArraySubclass() as readonly RustyApplicationRuntimeInputEnvelope[]),
    (error: unknown) => error instanceof TypeError,
  );
  const getterEnvelope = {} as Record<string, unknown>;
  Object.defineProperty(getterEnvelope, 'runtime', { get: () => RUNTIME, enumerable: true });
  assert.throws(
    () => adapter.input([getterEnvelope] as never),
    (error: unknown) => error instanceof TypeError,
  );
  assert.throws(
    () => adapter.admitExternalStep?.('01'),
    (error: unknown) => error instanceof ProductBrowserLocalTransportError && error.code === 'invalid_options',
  );
  adapter.dispose();

  const wrongContentType = createProductBrowserLocalHttpAdapter({
    fetch: async () => new Response('{}', {
      headers: { 'content-type': 'text/plain', 'x-rusty-commit-disposition': 'committed' },
    }),
    eventSource: FakeEventSource,
  });
  await assert.rejects(
    wrongContentType.advanceRealtime('1'),
    (error: unknown) => error instanceof ProductBrowserLocalTransportError && error.code === 'response_decode_failed',
  );
  wrongContentType.dispose();
});

test('local transport carries immutable bounded product payload intents', async () => {
  const requestBodies: unknown[] = [];
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async (_input, init) => {
      requestBodies.push(JSON.parse(String(init?.body)) as unknown);
      return response({ accepted: true, ...ACCEPTED_FAULT, count: 1, binding: RUNTIME, readout: READOUT });
    },
    eventSource: FakeEventSource,
  });
  const envelope = (data: unknown): readonly RustyApplicationRuntimeInputEnvelope[] => [{
    runtime: RUNTIME,
    sequence: '1',
    context: 'gameplay',
    intent: 'regenerate',
    value: { kind: 'product-payload', contract: 'example.regenerate.v1', data },
  } as never];

  const source = { nested: { seed: 7 }, values: [true, null, 'stable'] };
  const request = adapter.input(envelope(source));
  source.nested.seed = 99;
  source.values[2] = 'mutated';
  await request;
  assert.deepEqual(requestBodies, [{ batch: [{
    runtime: RUNTIME,
    sequence: '1',
    context: 'gameplay',
    intent: 'regenerate',
    value: {
      kind: 'product-payload',
      contract: 'example.regenerate.v1',
      data: { nested: { seed: 7 }, values: [true, null, 'stable'] },
    },
  }] }]);

  const accessor = {} as Record<string, unknown>;
  Object.defineProperty(accessor, 'value', { enumerable: true, get: () => 1 });
  const inherited = Object.create({ inherited: true }) as Record<string, unknown>;
  inherited['value'] = 1;
  let deep: unknown = null;
  for (let index = 0; index < 33; index += 1) deep = [deep];
  const nodeOverflow = Object.fromEntries(Array.from(
    { length: 1_024 },
    (_unused, index) => [`entry${String(index)}`, [index, index, index]],
  ));
  const byteOverflow = Array.from({ length: 1_024 }, () => 'x'.repeat(64));

  for (const rejected of [
    accessor,
    inherited,
    Number.NaN,
    Number.POSITIVE_INFINITY,
    9_007_199_254_740_992,
    deep,
    nodeOverflow,
    byteOverflow,
  ]) {
    assert.throws(() => adapter.input(envelope(rejected)), (error: unknown) =>
      error instanceof TypeError || error instanceof RangeError);
  }
  adapter.dispose();
});

test('local transport preserves primary and secondary pointer button edges', async () => {
  const requestBodies: unknown[] = [];
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async (_input, init) => {
      requestBodies.push(JSON.parse(String(init?.body)) as unknown);
      return response({ accepted: true, ...ACCEPTED_FAULT, count: 4 });
    },
    eventSource: FakeEventSource,
  });
  await adapter.input([
    { runtime: RUNTIME, sequence: '4', context: 'gameplay.default', fact: { kind: 'pointer-button', button: 'primary', edge: 'pressed' } },
    { runtime: RUNTIME, sequence: '5', context: 'gameplay.default', fact: { kind: 'pointer-button', button: 'primary', edge: 'released' } },
    { runtime: RUNTIME, sequence: '6', context: 'gameplay.default', fact: { kind: 'pointer-button', button: 'secondary', edge: 'pressed' } },
    { runtime: RUNTIME, sequence: '7', context: 'gameplay.default', fact: { kind: 'pointer-button', button: 'secondary', edge: 'released' } },
  ]);
  assert.deepEqual(requestBodies, [{ batch: [
    { runtime: RUNTIME, sequence: '4', context: 'gameplay.default', fact: { kind: 'pointer-button', button: 'primary', edge: 'pressed' } },
    { runtime: RUNTIME, sequence: '5', context: 'gameplay.default', fact: { kind: 'pointer-button', button: 'primary', edge: 'released' } },
    { runtime: RUNTIME, sequence: '6', context: 'gameplay.default', fact: { kind: 'pointer-button', button: 'secondary', edge: 'pressed' } },
    { runtime: RUNTIME, sequence: '7', context: 'gameplay.default', fact: { kind: 'pointer-button', button: 'secondary', edge: 'released' } },
  ] }]);
  adapter.dispose();
});

test('renderer diagnostics use their own 256 KiB snapshot budget', async () => {
  const requestBodies: unknown[] = [];
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async (_input, init) => {
      requestBodies.push(JSON.parse(String(init?.body)) as unknown);
      return response({ accepted: true, ...ACCEPTED_FAULT, runtime: RUNTIME });
    },
    eventSource: FakeEventSource,
  });

  const withinRendererBudget = {
    schemaVersion: 1,
    resources: Array.from({ length: 1_024 }, (_unused, index) => ({
      id: index,
      diagnostic: 'x'.repeat(64),
    })),
  };
  assert.ok(JSON.stringify(withinRendererBudget).length > 65_536);
  await adapter.reportRendererDiagnostics?.({
    runtime: RUNTIME,
    snapshot: withinRendererBudget,
  } as never);
  assert.deepEqual(requestBodies, [{ runtime: RUNTIME, snapshot: withinRendererBudget }]);

  const overRendererBudget = {
    schemaVersion: 1,
    resources: Array.from({ length: 1_024 }, (_unused, index) => ({
      id: index,
      diagnostic: 'x'.repeat(256),
    })),
  };
  assert.ok(JSON.stringify(overRendererBudget).length > 256 * 1024);
  assert.throws(
    () => adapter.reportRendererDiagnostics?.({
      runtime: RUNTIME,
      snapshot: overRendererBudget,
    } as never),
    /renderer diagnostics snapshot exceeds 262144 bytes/u,
  );
  assert.equal(requestBodies.length, 1);
  adapter.dispose();
});

test('terminal browser diagnostics remain postable after the output transport closes', async () => {
  const requestBodies: unknown[] = [];
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async (_input, init) => {
      requestBodies.push(JSON.parse(String(init?.body)) as unknown);
      return response({ accepted: true, reported: 2 });
    },
    eventSource: FakeEventSource,
  });
  adapter.dispose();
  await adapter.reportBrowserDiagnostics?.({
    hostState: 'failed', runtimeProgress: '9', transportState: 'closed', outputState: 'closed',
    lastRendererSequence: '60', rendererObservationAgeMs: '100',
    firstTerminal: { code: 'BROWSER_HOST_TRANSPORT_FAILED', message: 'transport closed' },
    recoverableEvent: { code: 'CSHARP_LIFECYCLE_CLOCK_REGRESSION', message: 'dropped clock observation' },
    pageEvents: [],
  });
  assert.deepEqual(requestBodies, [{
    hostState: 'failed', runtimeProgress: '9', transportState: 'closed', outputState: 'closed',
    lastRendererSequence: '60', rendererObservationAgeMs: '100',
    firstTerminal: { code: 'BROWSER_HOST_TRANSPORT_FAILED', message: 'transport closed' },
    recoverableEvent: { code: 'CSHARP_LIFECYCLE_CLOCK_REGRESSION', message: 'dropped clock observation' },
    pageEvents: [],
  }]);
});

test('browser diagnostics accepts the production committed response without recovery or duplicate report', async () => {
  let requests = 0;
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => {
      requests += 1;
      // Do not use response(): this is the production route's committed
      // mutation boundary, which deliberately has no output-through cursor.
      return new Response(JSON.stringify({ accepted: true, reported: 1 }), {
        status: 200,
        headers: {
          'content-type': 'application/json',
          'x-rusty-commit-disposition': 'committed',
        },
      });
    },
    eventSource: FakeEventSource,
  });
  const result = await adapter.reportBrowserDiagnostics?.({
    hostState: 'ready', runtimeProgress: '1', transportState: 'open', outputState: 'open',
    pageEvents: [],
  });
  assert.deepEqual(result, { accepted: true, reported: 1 });
  assert.equal(requests, 1, 'an accepted committed report is neither retried nor recovered');
  adapter.dispose();
});
