import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import test from 'node:test';
import { ProductBrowserDynamicRendererResources } from './dynamic-renderer-resources.js';

function identity(kind: 'audio-resource' | 'font', body: Uint8Array): string {
  const hash = createHash('sha256').update(body).digest('hex');
  return `${kind}/${hash}`;
}

test('dynamic resources fence cache entries by runtime generation and share an in-flight body fetch', async () => {
  const body = new Uint8Array([82, 73, 70, 70, 0, 0, 0, 0, 87, 65, 86, 69, 0]);
  const resource = identity('audio-resource', body);
  const requests: string[] = [];
  let release: (() => void) | undefined;
  const response = new Promise<void>((resolve) => { release = resolve; });
  const cache = new ProductBrowserDynamicRendererResources(async (input) => {
    requests.push(String(input));
    await response;
    return new Response(body);
  });

  const first = cache.ensure([resource], '7');
  const second = cache.ensure([resource], '7');
  assert.equal(requests.length, 1, 'same immutable resource shares its pending fetch');
  release?.();
  assert.equal((await first)[0]?.identity, resource);
  assert.equal((await second)[0]?.identity, resource);

  await cache.ensure([resource], '8');
  assert.equal(requests.length, 2, 'a new runtime generation never reuses prior owner bytes');
  assert.match(requests[1]!, /generation=8/u);
  assert.match(requests[1]!, new RegExp(`identity=${encodeURIComponent(resource)}`));
});

test('dynamic resources trust delivered bodies and prune only inactive cached entries', async () => {
  const good = new Uint8Array([119, 79, 70, 50, 1, 2, 3]);
  const font = identity('font', good);
  const other = identity('font', new Uint8Array([119, 79, 70, 50, 4, 5, 6]));
  const cache = new ProductBrowserDynamicRendererResources(async (input) => {
    const requested = new URL(String(input), 'https://product.test').searchParams.get('identity');
    if (requested === font) return new Response(good);
    if (requested === other) return new Response(new Uint8Array([119, 79, 70, 50, 9]));
    return new Response(null, { status: 404 });
  });

  await cache.ensure([font], '9');
  cache.retainOnly(new Set(), '9');
  assert.throws(() => cache.resources([font], '9'), /was not prefetched/u);
  const [loaded] = await cache.ensure([other], '9');
  assert.deepEqual(loaded?.bytes, new Uint8Array([119, 79, 70, 50, 9]));
  assert.equal(loaded?.identity, other);
});

test('a late response cannot populate a replacement runtime cache', async () => {
  const resource = identity('font', new Uint8Array([1]));
  let release!: (response: Response) => void;
  const cache = new ProductBrowserDynamicRendererResources(async (_input) =>
    new Promise<Response>((resolve) => { release = resolve; }));
  const pending = cache.ensure([resource], '1');
  const rejected = assert.rejects(pending, /stale runtime generation/u);
  await cache.ensure([], '2');
  release(new Response(new Uint8Array([1])));
  await rejected;
  assert.throws(() => cache.resources([resource], '2'), /was not prefetched/u);
});
