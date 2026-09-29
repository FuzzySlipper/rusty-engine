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
  inspectionTime: ['action-driven', 60],
} as const;

type TestJson =
  | null
  | boolean
  | number
  | string
  | readonly TestJson[]
  | { readonly [key: string]: TestJson };

class FakeEventSource implements ProductBrowserLocalEventSource {
  static readonly instances: FakeEventSource[] = [];
  readonly namedListeners = new Map<string, (event: { readonly data: string; readonly lastEventId: string }) => void>();
  readonly url: string;
  onopen: ((event: unknown) => void) | null = null;
  onmessage: ((event: { readonly data: string; readonly lastEventId: string }) => void) | null = null;
  onerror: ((event: unknown) => void) | null = null;
  closed = false;
  readyState = 0;
  nextEventId = 1;
  messageDeliveryCallbacks = 0;

  constructor(url: string) {
    this.url = url;
    FakeEventSource.instances.push(this);
  }

  close(): void {
    this.closed = true;
  }

  addEventListener(type: 'rusty-output-baseline', listener: (event: { readonly data: string; readonly lastEventId: string }) => void): void {
    this.namedListeners.set(type, listener);
  }

  removeEventListener(type: 'rusty-output-baseline', listener: (event: { readonly data: string; readonly lastEventId: string }) => void): void {
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

  /** The server closed this stream (a subscriber that fell behind, or a runtime that stopped). */
  drop(): void {
    this.readyState = 2;
    this.onerror?.({});
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

function assertNestedArrayDepth(value: unknown, expectedDepth: number): void {
  let nested = value;
  for (let depth = 0; depth < expectedDepth; depth += 1) {
    if (!Array.isArray(nested)) assert.fail(`expected array at depth ${String(depth)}`);
    assert.equal(nested.length, 1);
    nested = nested[0];
  }
  assert.equal(nested, null);
}

test('a fresh stream refused before its baseline is reopened until a runtime answers', async () => {
  FakeEventSource.instances.length = 0;
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => response({}),
    eventSource: FakeEventSource,
  });
  const outputs: unknown[] = [];
  const unsubscribe = adapter.subscribeOutputs((output) => outputs.push(output));
  const refused = FakeEventSource.instances[0]!;
  // A 503 while the supervisor replaces the runtime closes EventSource for good.
  refused.readyState = 2;
  refused.onerror?.({});
  assert.equal(FakeEventSource.instances.length, 1);
  await new Promise((resolve) => setTimeout(resolve, 300));
  assert.equal(refused.closed, true);
  assert.equal(FakeEventSource.instances.length, 2);
  const reopened = FakeEventSource.instances[1]!;
  assert.match(reopened.url, /\/outputs\/fresh$/u);
  completeConnectionBaseline(reopened);
  assert.deepEqual(outputs.at(-1), { kind: 'binding', runtime: RUNTIME, nextInputSequence: '1' });
  // The refused stream's late events are ignored.
  refused.emit({ kind: 'binding', runtime: RUNTIME, nextInputSequence: '9' }, '');
  assert.deepEqual(outputs.at(-1), { kind: 'binding', runtime: RUNTIME, nextInputSequence: '1' });
  unsubscribe();
  adapter.dispose();
});

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
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}video-feedback`:
          assert.deepEqual(body, {
            runtime: RUNTIME,
            replaceOwner: true,
            evictedFactCount: '2',
            facts: [{ kind: 'completed', factId: '9', handle: '3' }],
          });
          return response({ accepted: true, ...ACCEPTED_FAULT, runtime: RUNTIME, acceptedThroughFactId: '9' });
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
  assert.deepEqual(await adapter.reportVideoFeedback?.({
    runtime: RUNTIME,
    replaceOwner: true,
    evictedFactCount: '2',
    facts: [{ kind: 'completed', factId: '9', handle: '3' }],
  }), { accepted: true, ...ACCEPTED_FAULT, runtime: RUNTIME, acceptedThroughFactId: '9' });
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
    'POST /__rusty/product/runtime/video-feedback',
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

test('local transport posts large deep timeline data as a detached immutable snapshot', async () => {
  let posted: Record<string, unknown> | null = null;
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async (input, init) => {
      assert.equal(
        new URL(String(input), 'http://product.local/').pathname,
        `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}timeline-completion`,
      );
      posted = JSON.parse(String(init?.body)) as Record<string, unknown>;
      return response({ accepted: true, ...ACCEPTED_FAULT, ticket: '1', binding: RUNTIME, readout: READOUT });
    },
    eventSource: FakeEventSource,
  });
  let deep: TestJson = null;
  for (let depth = 0; depth < 128; depth += 1) deep = [deep];
  const data = {
    status: { value: 'before' },
    deep,
    magnitude: 1e20,
    entries: Array.from({ length: 129 }, (_, index) => index),
    text: 'x'.repeat(4 * 1024 + 1),
  } satisfies { readonly [key: string]: TestJson };

  const completion = adapter.completeTimeline!({
    ticket: '1',
    runtime: RUNTIME,
    correlation: 'request-1',
    outcome: { kind: 'success', data },
    provenance: { correlation: 'request-1', detail: data },
  });
  data.status.value = 'after';
  await completion;

  assert.ok(posted !== null);
  const outcome = (posted as Record<string, unknown>)['outcome'] as { readonly data: Record<string, unknown> };
  const provenance = (posted as Record<string, unknown>)['provenance'] as { readonly detail: Record<string, unknown> };
  assert.equal((outcome.data['status'] as { readonly value: string }).value, 'before');
  assert.equal((provenance.detail['status'] as { readonly value: string }).value, 'before');
  assert.equal(outcome.data['magnitude'], 1e20);
  assert.equal((outcome.data['entries'] as readonly unknown[]).length, 129);
  assert.equal((outcome.data['text'] as string).length, 4 * 1024 + 1);
  assertNestedArrayDepth(outcome.data['deep'], 128);
  adapter.dispose();
});

test('rejected runtime results remain decoded result facts rather than transport failures', async () => {
  const rejected = {
    accepted: false,
    code: 'CSHARP_NEW_SOURCE_REJECTION',
    disposition: 'rejected-recoverable',
    diagnostic: 'runtime rejected before admission',
  } as const;
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async (input) => {
      const pathname = new URL(String(input), 'http://product.local/').pathname;
      switch (pathname) {
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}advance-realtime`:
          return response({ ...rejected, operation: 'advance-realtime' }, 200, { 'x-rusty-commit-disposition': 'not-applied' });
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}input`:
          return response({ ...rejected, count: 0, acceptedCount: 0, droppedCount: 0 }, 200, { 'x-rusty-commit-disposition': 'not-applied' });
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}audio-feedback`:
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}animation-feedback`:
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}ghost-plate-feedback`:
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}renderer-diagnostics`:
          return response({ ...rejected, runtime: RUNTIME }, 200, { 'x-rusty-commit-disposition': 'not-applied' });
        case `${PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH}timeline-completion`:
          return response({ ...rejected, ticket: '1' }, 200, { 'x-rusty-commit-disposition': 'not-applied' });
        default:
          throw new Error(`unexpected route ${pathname}`);
      }
    },
    eventSource: FakeEventSource,
  });

  assert.equal((await adapter.advanceRealtime('1')).disposition, 'rejected-recoverable');
  assert.equal((await adapter.input([])).disposition, 'rejected-recoverable');
  assert.equal((await adapter.reportAudioFeedback({
    runtime: RUNTIME, replaceOwner: false, evictedFactCount: '0', facts: [],
  })).disposition, 'rejected-recoverable');
  assert.equal((await adapter.reportAnimationFeedback({
    runtime: RUNTIME, replaceOwner: false, evictedFactCount: '0', facts: [],
  })).disposition, 'rejected-recoverable');
  assert.equal((await adapter.reportGhostPlateFeedback({
    runtime: RUNTIME, replaceOwner: false, facts: [],
  })).disposition, 'rejected-recoverable');
  assert.equal((await adapter.reportRendererDiagnostics?.({
    runtime: RUNTIME, snapshot: { schemaVersion: 1 },
  } as never))?.disposition, 'rejected-recoverable');
  assert.equal((await adapter.completeTimeline?.({
    ticket: '1', runtime: RUNTIME, correlation: 'request-1', outcome: { kind: 'success' },
    provenance: { correlation: 'request-1' },
  }))?.disposition, 'rejected-recoverable');
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

test('local transport preserves the UTF-8 response byte limit and committed receipt', async () => {
  const adapter = createProductBrowserLocalHttpAdapter({
    maximumResponseBytes: 1024,
    fetch: async () => response({ ...result('start'), diagnostic: '界'.repeat(400) }),
    eventSource: FakeEventSource,
  });
  await assert.rejects(
    adapter.lifecycle({ kind: 'start' }),
    (error: unknown) => error instanceof ProductBrowserLocalTransportError
      && error.code === 'response_decode_failed'
      && error.message.includes('exceeds 1024 bytes')
      && error.mutation.certainty === 'committed',
  );
  adapter.dispose();
});

test('local transport rejects a declared oversized local response before buffering', async () => {
  let consumed = false;
  const adapter = createProductBrowserLocalHttpAdapter({
    maximumResponseBytes: 1024,
    fetch: async () => {
      const receipt = response(result('start'), 200, { 'content-length': '2048' });
      receipt.arrayBuffer = async () => { consumed = true; return new ArrayBuffer(2048); };
      return receipt;
    },
    eventSource: FakeEventSource,
  });
  await assert.rejects(adapter.lifecycle({ kind: 'start' }), /exceeds 1024 bytes/u);
  assert.equal(consumed, false);
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

for (const cursor of ['', '5', '50000']) {
  test(`disconnect at cursor ${cursor || 'none'} replaces the baseline across host incarnations`, async () => {
    FakeEventSource.instances.length = 0;
    let mutations = 0;
    const adapter = createProductBrowserLocalHttpAdapter({
      fetch: async () => { mutations += 1; return response(result('advance-realtime')); },
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

    if (cursor !== '') stream.emit({ kind: 'runtime-readout', readout: READOUT }, cursor);
    const previousCount = outputs.length;
    stream.onerror?.(new Error('host disconnected'));
    assert.equal(stream.closed, true);
    assert.equal(FakeEventSource.instances.length, 2);
    const replacement = FakeEventSource.instances[1]!;
    // A down host can fail repeatedly before returning; retain the one new
    // EventSource, whose request has no retired process cursor.
    replacement.onerror?.(new Error('host still offline'));
    assert.equal(FakeEventSource.instances.length, 2);
    const nextRuntime = { ...RUNTIME, instanceId: '8' };
    replacement.emit({
      kind: 'binding', runtime: nextRuntime, nextInputSequence: '1',
      publicationFrontiers: [{ stream: 'voxel:active', revision: 2 }],
    }, '');
    replacement.emit({ kind: 'runtime-readout', readout: READOUT }, '');
    replacement.emitBaseline({ ...result('connect'), binding: nextRuntime }, '');
    assert.equal(outputs.length, previousCount + 2);
    assert.deepEqual(batches.map((batch) => batch.metadata), [
      { epoch: 1, baseline: true, recovery: 'none' },
      ...(cursor === '' ? [] : [{ epoch: 1, baseline: false, recovery: 'none' }]),
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
    }, '20');
    assert.equal(outputs.length, previousCount + 3);
    assert.equal(mutations, 0, 'output recovery must not replay any mutation');
    unsubscribeBatches?.();
    unsubscribe();
    adapter.dispose();
  });

}

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
      { kind: 'renderer-resources' },
    ],
  }, '1');
  assert.equal(received.length, 2);
  assert.deepEqual(received[1]?.map((output) => (output as { kind: string }).kind), [
    'runtime-readout',
    'renderer-resources',
  ]);
  unsubscribe?.();
  adapter.dispose();
});

test('runtime output UI projections pass empty strings and large deep data through unchanged', () => {
  FakeEventSource.instances.length = 0;
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => response({}),
    eventSource: FakeEventSource,
  });
  const received: unknown[] = [];
  const unsubscribe = adapter.subscribeOutputs((output) => received.push(output));
  const stream = FakeEventSource.instances[0]!;
  completeConnectionBaseline(stream);
  received.length = 0;

  let deep: TestJson = null;
  for (let depth = 0; depth < 128; depth += 1) deep = [deep];
  stream.emit({
    kind: 'runtime-output-batch',
    outputs: [{
      kind: 'ui-projection',
      envelope: {
        artifact: 'rusty.product.ui-projection',
        runtime: RUNTIME,
        sequence: '1',
        stream: 'product.ui',
        contract: 'product.ui.v1',
        value: {
          nested: { state: 'before' },
          empty: '',
          emptyArray: ['', { text: '' }],
          magnitude: 1e20,
          deep,
          entries: Array.from({ length: 1_025 }, (_, index) => index),
          text: 'x'.repeat(64 * 1024 + 1),
        },
      },
    }],
  }, '1');

  const output = received.at(-1) as {
    readonly kind: string;
    readonly envelope: { readonly value: Record<string, unknown> };
  };
  assert.equal(output.kind, 'ui-projection');
  const value = output.envelope.value;
  assert.equal(value['magnitude'], 1e20);
  assert.equal(value['empty'], '');
  assert.equal((value['emptyArray'] as readonly unknown[])[0], '');
  assert.equal(((value['emptyArray'] as readonly unknown[])[1] as { text: string }).text, '');
  assert.equal((value['nested'] as { readonly state: string }).state, 'before');
  assert.equal((value['entries'] as readonly unknown[]).length, 1_025);
  assert.equal((value['text'] as string).length, 64 * 1024 + 1);
  assertNestedArrayDepth(value['deep'], 128);
  unsubscribe();
  adapter.dispose();
});

test('sixty hertz receipt stream parses once and preserves four-output order per receipt', () => {
  const TICKS = 60;
  const OUTPUTS_PER_RECEIPT = 4;
  const expectedKinds = [
    'frame',
    'view-composition',
    'ui-projection',
    'runtime-readout',
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
    240,
    'the old one-callback-per-output stream would deliver about 240 typed outputs',
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
  first.drop();
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

test('one output batch far above the former 256 KiB event bound arrives as one event', () => {
  FakeEventSource.instances.length = 0;
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async () => response({}),
    eventSource: FakeEventSource,
  });
  const batches: unknown[][] = [];
  adapter.subscribeOutputBatches?.((outputs) => batches.push([...outputs]));
  const stream = FakeEventSource.instances[0]!;
  completeConnectionBaseline(stream);
  stream.emit({
    kind: 'runtime-output-batch',
    outputs: [{ kind: 'frame', frame: { payload: 'x'.repeat(4 * 1024 * 1024) } }],
  });
  assert.equal(batches.length, 2);
  assert.equal(((batches[1]?.[0] as { frame: { payload: string } }).frame.payload).length, 4 * 1024 * 1024);
  adapter.dispose();
});

test('a dropped output stream asks for one fresh baseline without closing the runtime transport', async () => {
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
  stream.drop();
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
  const attachment = (requestBodies[0] as { attachment: { id: string; baseline?: unknown } }).attachment;
  assert.match(attachment.id, /^browser-[0-9a-f]{32}$/u);
  assert.equal(attachment.baseline, undefined);
  assert.deepEqual(requestBodies, [{ ...terminalReport, attachment }]);

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

test('local transport carries immutable product payload intents without local quotas', async () => {
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
  for (let index = 0; index < 1_024; index += 1) deep = [deep];
  const manyNodes = Object.fromEntries(Array.from(
    { length: 1_024 },
    (_unused, index) => [`entry${String(index)}`, [index, index, index]],
  ));
  const largeText = Array.from({ length: 1_024 }, () => 'x'.repeat(64));

  for (const rejected of [
    accessor,
    inherited,
    Number.NaN,
    Number.POSITIVE_INFINITY,
    9_007_199_254_740_992,
  ]) {
    assert.throws(() => adapter.input(envelope(rejected)), (error: unknown) =>
      error instanceof TypeError || error instanceof RangeError);
  }
  await Promise.all([deep, manyNodes, largeText].map((data) => adapter.input(envelope(data))));
  assert.equal(requestBodies.length, 4);
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

test('local transport preserves analog button values and rejects pressure outside its range', async () => {
  const bodies: unknown[] = [];
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async (_input, init) => {
      bodies.push(JSON.parse(String(init?.body)));
      return response({ accepted: true, ...ACCEPTED_FAULT, count: 2 });
    }, eventSource: FakeEventSource,
  });
  const batch = [0.25, 0].map((value, index) => ({
    runtime: RUNTIME, sequence: String(index + 4), context: 'gameplay.default',
    fact: { kind: 'controller-button-value' as const, button: 'button-7' as const, value },
  }));
  await adapter.input(batch);
  assert.deepEqual(bodies, [{ batch }]);
  for (const value of [-0.1, 1.1, NaN]) {
    assert.throws(() => adapter.input([{ ...batch[0]!, fact: { ...batch[0]!.fact, value } }]));
  }
  assert.equal(bodies.length, 1);
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
  const attachment = (requestBodies[0] as { attachment: { id: string; baseline?: unknown } }).attachment;
  assert.match(attachment.id, /^browser-[0-9a-f]{32}$/u);
  assert.equal(attachment.baseline, undefined);
  assert.deepEqual(requestBodies, [{
    attachment,
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

test('attachment health reports only renderer-confirmed baselines and correlates each request', async () => {
  FakeEventSource.instances.length = 0;
  const reports: Array<{ attachment: { id: string; replaces?: string; baseline?: unknown } }> = [];
  const headers: Array<string | null> = [];
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async (_input, init) => {
      reports.push(JSON.parse(String(init?.body)) as typeof reports[number]);
      headers.push(new Headers(init?.headers).get('x-rusty-browser-attachment'));
      return response({ accepted: true, reported: 1 });
    }, eventSource: FakeEventSource,
  });
  adapter.subscribeOutputs(() => undefined);
  completeConnectionBaseline(FakeEventSource.instances[0]!);
  const report = { hostState: 'ready' as const, runtimeProgress: '1',
    transportState: 'open' as const, outputState: 'open' as const, pageEvents: [] };
  await adapter.reportBrowserDiagnostics?.(report);
  assert.equal(reports[0]!.attachment.baseline, undefined, 'delivery is not realization');
  adapter.confirmOutputBaseline?.(1);
  await adapter.reportBrowserDiagnostics?.(report);
  assert.deepEqual(reports[1]!.attachment.baseline, {
    runtime: RUNTIME, nextInputSequence: '1', publicationFrontiers: [],
  });
  FakeEventSource.instances[0]!.drop();
  completeConnectionBaseline(FakeEventSource.instances[1]!);
  await adapter.reportBrowserDiagnostics?.(report);
  assert.equal(reports[2]!.attachment.replaces, reports[1]!.attachment.id);
  assert.equal(reports[2]!.attachment.baseline, undefined);
  adapter.confirmOutputBaseline?.(1);
  await adapter.reportBrowserDiagnostics?.(report);
  assert.equal(reports[3]!.attachment.baseline, undefined, 'old confirmation cannot settle new attachment');
  adapter.confirmOutputBaseline?.(2);
  await adapter.reportBrowserDiagnostics?.(report);
  assert.ok(reports[4]!.attachment.baseline);
  assert.deepEqual(headers, reports.map((entry) => entry.attachment.id));
  adapter.dispose();
});

for (const certainty of ['not-applied', 'unknown'] as const) {
  test(`runtime ${certainty} disposition survives a truncated rejection body`, async () => {
    let requests = 0;
    const adapter = createProductBrowserLocalHttpAdapter({
      fetch: async () => {
        requests += 1;
        return new Response(new ReadableStream<Uint8Array>({
          start(controller) { controller.error(new TypeError('truncated rejection')); },
        }), { headers: {
          'content-type': 'application/json',
          'x-rusty-commit-disposition': certainty,
        } });
      },
      eventSource: FakeEventSource,
    });
    await assert.rejects(adapter.lifecycle({ kind: 'start' }), (error: unknown) =>
      error instanceof ProductBrowserLocalTransportError
      && error.mutation.certainty === (certainty === 'unknown' ? 'outcome-unknown' : certainty)
      && error.mutation.outputRecovery === 'none');
    assert.equal(requests, 1, 'the callback request must not be replayed');
    adapter.dispose();
  });
}

test('runtime output batches admit video resources and reject duplicate identities', () => {
  for (const duplicate of [false, true]) {
    FakeEventSource.instances.length = 0;
    const errors: unknown[] = [];
    const outputs: unknown[] = [];
    const adapter = createProductBrowserLocalHttpAdapter({
      fetch: async () => response({}), eventSource: FakeEventSource,
      onTransportError: (error) => errors.push(error),
    });
    adapter.subscribeOutputs((output) => outputs.push(output));
    const stream = FakeEventSource.instances[0]!;
    completeConnectionBaseline(stream);
    outputs.length = 0;
    const identity = `video-resource/${'a'.repeat(64)}`;
    stream.emit({ kind: 'runtime-output-batch', outputs: [{
      kind: 'renderer-resources', rendererResources: duplicate ? [identity, identity] : [identity],
    }] });
    assert.equal(errors.length, duplicate ? 1 : 0);
    assert.equal(outputs.length, duplicate ? 0 : 1);
    adapter.dispose();
  }
});


test('held connection baseline satisfies response output fences without another simulation tick', async () => {
  FakeEventSource.instances.length = 0;
  const adapter = createProductBrowserLocalHttpAdapter({
    eventSource: FakeEventSource,
    fetch: async () => response(result('connect')),
  });
  const unsubscribe = adapter.subscribeOutputs(() => {});
  const stream = FakeEventSource.instances[0]!;
  stream.open();
  stream.emit({ kind: 'binding', runtime: RUNTIME, nextInputSequence: '1' }, '');
  stream.emitBaseline({ ...result('connect'), outputThrough: '42' }, '');
  await adapter.connect!();
  await adapter.waitUntilOutputSequence!('42');
  unsubscribe();
  adapter.dispose();
});

test('local transport admits all four arrow key controls', async () => {
  const batch: RustyApplicationRuntimeInputEnvelope[] = (['arrow-up', 'arrow-down', 'arrow-left', 'arrow-right'] as const).map((code, index) => ({
    runtime: RUNTIME,
    sequence: String(index + 1),
    context: 'gameplay.default',
    fact: { kind: 'key', code, edge: 'pressed' },
  }));
  const adapter = createProductBrowserLocalHttpAdapter({
    fetch: async (_input, init) => {
      assert.deepEqual(JSON.parse(String(init?.body)).batch, batch);
      return response({ accepted: true, ...ACCEPTED_FAULT, count: batch.length, binding: RUNTIME, readout: READOUT });
    },
    eventSource: FakeEventSource,
  });
  assert.equal((await adapter.input(batch)).count, 4);
});
