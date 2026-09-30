import assert from 'node:assert/strict';
import test from 'node:test';
import {
  createLiveDebugHttpTransport,
  diagnosticEventAgeMilliseconds,
} from './index.js';

test('accepts an unavailable generated catalog without inventing commands', async () => {
  const client = createLiveDebugHttpTransport({
    origin: 'http://127.0.0.1:8123',
    fetch: (async () => new Response(JSON.stringify({ available: false, commands: [] }), { status: 200 })) as typeof fetch,
  });
  assert.deepEqual(await client.catalog(), { available: false, commands: [] });
});

test('sends one raw command body and keeps semantic failure typed', async () => {
  let request: RequestInit | undefined;
  const client = createLiveDebugHttpTransport({
    origin: 'http://127.0.0.1:8123',
    fetch: (async (_input, init) => {
      request = init;
      return new Response('unknown command', { status: 422 });
    }) as typeof fetch,
  });
  assert.deepEqual(await client.execute('fixture.unknown'), { succeeded: false, message: 'unknown command' });
  assert.equal(request?.headers && new Headers(request.headers).get('content-type'), 'text/plain; charset=utf-8');
  assert.equal(request?.body, 'fixture.unknown');
});

test('diagnostics retain independent cursor facts and age each event', async () => {
  const client = createLiveDebugHttpTransport({
    origin: 'http://127.0.0.1:8123',
    fetch: (async () => new Response(JSON.stringify({
      events: [{
        sequence: '8', monotonicNanoseconds: '2000000000', severity: 'warning', disposition: 'degraded',
        source: 'browser-host', code: 'BROWSER_HOST_STATUS', message: 'status',
        fields: [{ key: 'renderer-observation-age-ms', value: '100' }],
      }],
      floorSequence: '8', throughSequence: '8', nextCursor: '8', readMonotonicNanoseconds: '2750000000',
      lagged: false, warningCount: '1', errorCount: '0', droppedCount: '0',
      telemetry: {
        inFlightOperation: 'advance-realtime', inFlightAgeMs: '4',
        lastProductAdmissionLatencyMs: '6', lastInputAdmissionLatencyMs: '2',
        queuedInputBatches: 3, queuedInputEvents: 4, inputBatchCapacity: 256,
        oldestInputAgeMs: '9', inputOverflowPending: false,
        runtimeProgressRateMillihertz: '60000', runtimeProgressAgeMs: '1',
        runtimeProgressUnavailableReason: null,
        connections: 1, subscribers: 1, outputQueueItems: 2, outputQueueCapacity: 256,
        outputBindingActive: true,
        updateAttribution: {
          sampleCount: '2048', callbackDurationUsP50: '80', callbackDurationUsP95: '120', callbackDurationUsMax: '140', rollingSlowestAgeMs: '20', slowestAgeMs: '2000',
          latest: { runtime: { instanceId: '42', generation: '7', controlRevision: '9' }, simulationStep: '12', admittedStepCount: '13', postCallbackDurationUs: '15', callbackDurationUs: '90', characterStepCalls: '1', characterStepDurationUs: '30', characterStepCastCount: '4', characterStepCandidateCount: '16', characterStepNarrowPhaseCount: '16', voxelResidencyCalls: '0', voxelResidencyDurationUs: '0', voxelScenePresentationCalls: '1', voxelScenePresentationDurationUs: '10' },
          rollingSlowest: { runtime: null, simulationStep: '10', admittedStepCount: '11', postCallbackDurationUs: '20', callbackDurationUs: '140', characterStepCalls: '1', characterStepDurationUs: '60', characterStepCastCount: '8', characterStepCandidateCount: '32', characterStepNarrowPhaseCount: '32', voxelResidencyCalls: '1', voxelResidencyDurationUs: '30', voxelScenePresentationCalls: '1', voxelScenePresentationDurationUs: '20' },
          slowest: { runtime: null, simulationStep: '10', admittedStepCount: '11', postCallbackDurationUs: '20', callbackDurationUs: '140', characterStepCalls: '1', characterStepDurationUs: '60', characterStepCastCount: '8', characterStepCandidateCount: '32', characterStepNarrowPhaseCount: '32', voxelResidencyCalls: '1', voxelResidencyDurationUs: '30', voxelScenePresentationCalls: '1', voxelScenePresentationDurationUs: '20' },
        },
      },
    }), { status: 200 })) as typeof fetch,
  });
  const batch = await client.diagnostics!('7');
  assert.equal(batch.nextCursor, '8');
  assert.equal(diagnosticEventAgeMilliseconds(batch, batch.events[0]!), 750);
  assert.equal(batch.telemetry?.queuedInputEvents, 4);
  assert.equal(batch.telemetry?.updateAttribution?.slowest.characterStepCastCount, '8');
  assert.equal(batch.telemetry?.updateAttribution?.slowest.characterStepNarrowPhaseCount, '32');
  assert.equal(batch.telemetry?.updateAttribution?.rollingSlowestAgeMs, '20');
  assert.equal(batch.telemetry?.updateAttribution?.latest.postCallbackDurationUs, '15');
});

test('a host error names its code and diagnostic', async () => {
  const client = createLiveDebugHttpTransport({
    origin: 'http://127.0.0.1:8123',
    fetch: (async () => new Response(JSON.stringify({
      accepted: false,
      error: { code: 'PRODUCT_HOST_ROUTE_NOT_FOUND', diagnostic: 'route is not admitted' },
    }), { status: 404 })) as typeof fetch,
  });
  await assert.rejects(() => client.diagnostics!(), /PRODUCT_HOST_ROUTE_NOT_FOUND: route is not admitted/u);
});
