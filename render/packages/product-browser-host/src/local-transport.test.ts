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

  constructor(url: string) {
    this.url = url;
    FakeEventSource.instances.push(this);
  }

  close(): void {
    this.closed = true;
  }

  addEventListener(type: 'rusty-output-lag' | 'rusty-output-fragment', listener: (event: { readonly data: string; readonly lastEventId: string }) => void): void {
    this.namedListeners.set(type, listener);
  }

  removeEventListener(type: 'rusty-output-lag' | 'rusty-output-fragment', listener: (event: { readonly data: string; readonly lastEventId: string }) => void): void {
    if (this.namedListeners.get(type) === listener) this.namedListeners.delete(type);
  }

  emit(output: unknown, lastEventId = String(this.nextEventId++)): void {
    this.onmessage?.({ data: JSON.stringify(output), lastEventId });
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
}

function response(body: unknown, status = 200, headers: Record<string, string> = {}): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json', ...headers },
  });
}

function result(operation: string): Record<string, unknown> {
  return {
    accepted: true,
    operation,
    binding: RUNTIME,
    nextInputSequence: '1',
    readout: READOUT,
  };
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
          return response({ accepted: true, count: (body?.['batch'] as readonly unknown[]).length, binding: RUNTIME, readout: READOUT });
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
          return response({ accepted: true, ticket: '1', binding: RUNTIME, readout: READOUT });
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}audio-feedback`:
          assert.deepEqual(body, {
            runtime: RUNTIME,
            replaceOwner: true,
            evictedFactCount: '2',
            facts: [{
              kind: 'naturalCompletion', source: 'oneShot', factId: '7', sequence: 3, signalHandle: '11',
            }],
          });
          return response({ accepted: true, runtime: RUNTIME, acceptedThroughFactId: '7' });
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
          return response({ accepted: true, runtime: RUNTIME, acceptedThroughFactId: '8' });
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
  assert.equal(FakeEventSource.instances[0]?.url, `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}outputs`);
  let outputSubscriptionReady = false;
  const readiness = adapter.waitUntilOutputSubscriptionReady?.().then(() => {
    outputSubscriptionReady = true;
  });
  await Promise.resolve();
  assert.equal(outputSubscriptionReady, false);
  FakeEventSource.instances[0]!.open();
  await readiness;
  assert.equal(outputSubscriptionReady, true);
  FakeEventSource.instances[0]!.emit({ kind: 'binding', runtime: RUNTIME, nextInputSequence: '1' });
  FakeEventSource.instances[0]!.emit({ kind: 'runtime-readout', readout: READOUT });
  assert.equal(outputs.length, 2);
  assert.equal(isolatedOutputs.length, 2);
  FakeEventSource.instances[0]!.onerror?.(new Error('transient stream failure'));
  assert.equal(FakeEventSource.instances[0]!.closed, false);
  FakeEventSource.instances[0]!.emit({ kind: 'binding', runtime: RUNTIME, nextInputSequence: '1' });
  assert.equal(outputs.length, 3);
  assert.equal(isolatedOutputs.length, 3);
  assert.equal(transportErrors.length, 4);
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
  }), { accepted: true, runtime: RUNTIME, acceptedThroughFactId: '7' });
  assert.deepEqual(await adapter.reportAnimationFeedback({
    runtime: RUNTIME,
    replaceOwner: true,
    evictedFactCount: '0',
    facts: [{
      kind: 'diagnostic', factId: '8', objectId: null, generation: null,
      code: 'assetMissing', sequence: 4,
    }],
  }), { accepted: true, runtime: RUNTIME, acceptedThroughFactId: '8' });
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

test('operation response waits for its exact retained-output cursor', async () => {
  FakeEventSource.instances.length = 0;
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => response(result('start'), 200, { 'x-rusty-output-through': '2' }),
    eventSource: FakeEventSource,
  });
  const outputs: unknown[] = [];
  adapter.subscribeOutputs((output) => outputs.push(output));
  const operation = adapter.lifecycle({ kind: 'start' });
  let settled = false;
  void operation.then(() => { settled = true; });
  await Promise.resolve();
  const stream = FakeEventSource.instances[0]!;
  stream.emit({ kind: 'binding', runtime: RUNTIME, nextInputSequence: '1' }, '1');
  await Promise.resolve();
  assert.equal(settled, false);
  stream.emit({ kind: 'runtime-readout', readout: READOUT }, '2');
  assert.equal((await operation).operation, 'start');
  assert.equal(settled, true);
  assert.equal(outputs.length, 2);
  adapter.dispose();
});

test('large retained output fragments publish once after complete ordered reassembly', () => {
  FakeEventSource.instances.length = 0;
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => response({}),
    eventSource: FakeEventSource,
  });
  const outputs: unknown[] = [];
  adapter.subscribeOutputs((output) => outputs.push(output));
  const stream = FakeEventSource.instances[0]!;
  stream.emit({ kind: 'binding', runtime: RUNTIME, nextInputSequence: '1' });
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
  }));
  assert.equal(outputs.length, 2);
  assert.equal((outputs[1] as { kind: string }).kind, 'frame');
  assert.equal(((outputs[1] as { frame: { payload: string } }).frame.payload).length, 300_000);
  adapter.dispose();
});

test('output fragments fail closed on missing, duplicate, stale, oversized, and interrupted transfers', () => {
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
    const failures: unknown[] = [];
    adapter.subscribeTerminalFailures?.((failure) => failures.push(failure));
    adapter.subscribeOutputs((output) => outputs.push(output));
    const stream = FakeEventSource.instances[0]!;
    stream.emit({ kind: 'binding', runtime: RUNTIME, nextInputSequence: '1' });
    exercise(stream);
    assert.equal(outputs.length, 1);
    assert.equal(failures.length, 1);
    assert.equal((failures[0] as { kind: string }).kind, 'runtime-failure');
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
    fetch: async () => response({ accepted: true, operation: 'advance-realtime' }),
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

test('named output lag is a terminal typed failure and never reconnects', async () => {
  FakeEventSource.instances.length = 0;
  const terminalFailures: unknown[] = [];
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => response({ accepted: true, operation: 'advance-realtime' }),
    eventSource: FakeEventSource,
    onTransportError: (error) => terminalFailures.push(error),
  });
  const hostFailures: unknown[] = [];
  const unsubscribeFailure = adapter.subscribeTerminalFailures?.((failure) => hostFailures.push(failure));
  const unsubscribeOutput = adapter.subscribeOutputs(() => undefined);
  const stream = FakeEventSource.instances[0];
  assert.ok(stream);
  stream.emitLag();
  assert.equal(stream.closed, true);
  assert.deepEqual(hostFailures, [{
    kind: 'output-lag',
    diagnostic: 'Product Browser local runtime output stream lost retained output; a fresh snapshot is required',
  }]);
  assert.equal(terminalFailures.length, 1);
  assert.equal(FakeEventSource.instances.length, 1);
  await assert.rejects(
    adapter.advanceRealtime('1'),
    (error: unknown) => error instanceof ProductBrowserLocalTransportError && error.code === 'stream_failed',
  );
  unsubscribeFailure?.();
  unsubscribeOutput();
  adapter.dispose();
});

test('local transport hardens the JSON border before requests', async () => {
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async (_input, init) => {
      assert.equal(new Headers(init?.headers).get('content-type'), 'application/json');
      return response({ accepted: true, operation: 'advance-realtime' });
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
    fetch: async () => new Response('{}', { headers: { 'content-type': 'text/plain' } }),
    eventSource: FakeEventSource,
  });
  await assert.rejects(
    wrongContentType.advanceRealtime('1'),
    (error: unknown) => error instanceof ProductBrowserLocalTransportError && error.code === 'response_decode_failed',
  );
  wrongContentType.dispose();
});
