import assert from 'node:assert/strict';
import test from 'node:test';
import { loadProductBrowserRendererInitialContent } from './renderer-preload.js';

const MODULE_URL = 'https://product.local/engine/product-browser-host.js';
const MIB = 1024 * 1024;

function meshResource(index: number, byteLength: number): Record<string, unknown> {
  const digest = index.toString(16).padStart(64, '0');
  return {
    identity: `mesh-resource/${digest}`,
    contentHash: `sha256:${digest}`,
    mediaType: 'application/octet-stream',
    path: `content/mesh-${String(index)}.rmesh`,
    byteLength,
  };
}

function descriptorResponse(resources: readonly Record<string, unknown>[]): Response {
  return new Response(JSON.stringify({
    artifact: 'rusty.product.renderer-preload.v1',
    resources,
  }), { headers: { 'content-type': 'application/json' } });
}

function fetchDescriptorThenFailResource(
  descriptor: Response,
  requests: { count: number },
): typeof globalThis.fetch {
  return async (input) => {
    requests.count += 1;
    if (new URL(String(input)).pathname.endsWith('/renderer-preload.json')) return descriptor;
    throw new Error('resource fetch reached');
  };
}

test('renderer preload admits a 64 MiB mesh resource before fetching its bytes', async () => {
  const requests = { count: 0 };
  const fetcher = fetchDescriptorThenFailResource(
    descriptorResponse([meshResource(0, 64 * MIB)]),
    requests,
  );

  await assert.rejects(
    loadProductBrowserRendererInitialContent(MODULE_URL, fetcher),
    /resource fetch reached/u,
  );
  assert.equal(requests.count, 2, 'descriptor admission must reach resource loading');
});

test('renderer preload admits a 256 MiB aggregate mesh set before fetching its bytes', async () => {
  const requests = { count: 0 };
  const fetcher = fetchDescriptorThenFailResource(
    descriptorResponse(Array.from({ length: 4 }, (_, index) => meshResource(index, 64 * MIB))),
    requests,
  );

  await assert.rejects(
    loadProductBrowserRendererInitialContent(MODULE_URL, fetcher),
    /resource fetch reached/u,
  );
  assert.equal(requests.count, 5, 'aggregate admission must reach resource loading');
});

test('renderer preload rejects mesh descriptors above the 64 MiB resource bound before fetching', async () => {
  const requests = { count: 0 };
  const fetcher = fetchDescriptorThenFailResource(
    descriptorResponse([meshResource(0, 64 * MIB + 1)]),
    requests,
  );

  await assert.rejects(
    loadProductBrowserRendererInitialContent(MODULE_URL, fetcher),
    /exceeds application-host bounds/u,
  );
  assert.equal(requests.count, 1, 'an over-limit descriptor must not fetch resource bytes');
});

test('renderer preload rejects mesh descriptors above the 256 MiB aggregate bound before fetching', async () => {
  const requests = { count: 0 };
  const fetcher = fetchDescriptorThenFailResource(
    descriptorResponse(Array.from({ length: 5 }, (_, index) => meshResource(index, 64 * MIB))),
    requests,
  );

  await assert.rejects(
    loadProductBrowserRendererInitialContent(MODULE_URL, fetcher),
    /exceeds application-host bounds/u,
  );
  assert.equal(requests.count, 1, 'an over-limit aggregate must not fetch resource bytes');
});
