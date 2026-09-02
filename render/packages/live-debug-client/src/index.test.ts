import assert from 'node:assert/strict';
import test from 'node:test';
import { createLiveDebugHttpTransport, diagnosticRendererObservationAgeMilliseconds } from './index.js';

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

test('diagnostics retain independent cursor facts and age a stopped browser observation', async () => {
  const client = createLiveDebugHttpTransport({
    origin: 'http://127.0.0.1:8123',
    fetch: (async () => new Response(JSON.stringify({
      events: [{
        sequence: '8', monotonicNanoseconds: '2000000000', severity: 'warning', disposition: 'degraded',
        source: 'browser-host', code: 'BROWSER_HOST_STATUS', message: 'status',
        fields: [{ key: 'renderer-age-ms', value: '100' }],
      }],
      floorSequence: '8', throughSequence: '8', nextCursor: '8', readMonotonicNanoseconds: '2750000000',
      lagged: false, warningCount: '1', errorCount: '0', droppedCount: '0',
    }), { status: 200 })) as typeof fetch,
  });
  const batch = await client.diagnostics!('7');
  assert.equal(batch.nextCursor, '8');
  assert.equal(diagnosticRendererObservationAgeMilliseconds(batch, batch.events[0]!), 850);
});

test('diagnostics reject cursor values outside canonical u64 range before transport', async () => {
  let calls = 0;
  const client = createLiveDebugHttpTransport({
    origin: 'http://127.0.0.1:8123',
    fetch: (async () => {
      calls += 1;
      return new Response('{}', { status: 200 });
    }) as typeof fetch,
  });
  await assert.rejects(
    () => client.diagnostics!('18446744073709551616'),
    /cursor is invalid/u,
  );
  assert.equal(calls, 0);
});
