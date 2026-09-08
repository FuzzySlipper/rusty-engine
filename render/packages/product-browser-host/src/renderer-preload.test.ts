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

function glbResource(index: number, byteLength: number): Record<string, unknown> {
  const digest = index.toString(16).padStart(64, '0');
  return {
    identity: `animated-mesh-resource/${digest}`,
    contentHash: `sha256:${digest}`,
    mediaType: 'model/gltf-binary',
    path: `content/animated-mesh-${String(index)}.glb`,
    byteLength,
  };
}

function textureResource(index: number, byteLength: number): Record<string, unknown> {
  const digest = index.toString(16).padStart(64, '0');
  return {
    identity: `texture-resource/${digest}`,
    contentHash: `sha256:${digest}`,
    mediaType: 'image/png',
    path: `content/texture-${String(index)}.png`,
    byteLength,
  };
}

function audioResource(index: number, byteLength: number): Record<string, unknown> {
  const digest = index.toString(16).padStart(64, '0');
  return {
    identity: `audio-resource/${digest}`,
    contentHash: `sha256:${digest}`,
    mediaType: 'audio/wav',
    path: `content/audio-${String(index)}.wav`,
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

test('renderer preload admits mesh and GLB descriptors beyond retired byte, aggregate, and count caps', async () => {
  const requests = { count: 0 };
  const resources = [
    glbResource(0, 64 * MIB + 1),
    ...Array.from({ length: 1_025 }, (_, index) => meshResource(index + 1, 64 * MIB + 1)),
  ];
  const fetcher = fetchDescriptorThenFailResource(
    descriptorResponse(resources),
    requests,
  );

  await assert.rejects(
    loadProductBrowserRendererInitialContent(MODULE_URL, fetcher),
    /resource fetch reached/u,
  );
  assert.equal(requests.count, resources.length + 1, 'all admitted descriptors reach resource loading');
});

test('renderer preload admits texture and audio descriptors beyond retired byte, aggregate, and count caps', async () => {
  const requests = { count: 0 };
  const resources = [
    ...Array.from({ length: 257 }, (_, index) => textureResource(index, 16 * MIB + 1)),
    ...Array.from({ length: 65 }, (_, index) => audioResource(index + 512, 8 * MIB + 1)),
  ];
  const fetcher = fetchDescriptorThenFailResource(descriptorResponse(resources), requests);

  await assert.rejects(
    loadProductBrowserRendererInitialContent(MODULE_URL, fetcher),
    /resource fetch reached/u,
  );
  assert.equal(requests.count, resources.length + 1, 'all admitted descriptors reach resource loading');
});
