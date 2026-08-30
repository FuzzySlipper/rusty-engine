import assert from 'node:assert/strict';
import test from 'node:test';
import { createLiveDebugHttpTransport } from './index.js';

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
