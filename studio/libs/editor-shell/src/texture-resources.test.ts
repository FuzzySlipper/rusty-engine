import { test } from 'node:test';
import assert from 'node:assert/strict';

import { resolveStudioTextureResource } from './texture-resources.js';

void test('texture resolver binds exact Rust readout identity before host access', async () => {
  const resources = [{
    resource: 'texture-resource/abc',
    contentHash: 'sha256:abc',
    byteLength: 4,
    sourcePath: '.rusty-engine/textures/abc.png',
  }];
  const calls: string[] = [];
  const bytes = await resolveStudioTextureResource(
    '/project',
    resources,
    { resource: 'texture-resource/abc', contentHash: 'sha256:abc', byteLength: 4 },
    async (root, path, hash) => {
      calls.push(`${root}|${path}|${hash}`);
      return new Uint8Array([1, 2, 3, 4]).buffer;
    },
  );
  assert.equal(bytes.byteLength, 4);
  assert.deepEqual(calls, ['/project|.rusty-engine/textures/abc.png|sha256:abc']);
  await assert.rejects(
    resolveStudioTextureResource(
      '/project',
      resources,
      { resource: 'texture-resource/abc', contentHash: 'sha256:stale', byteLength: 4 },
      async () => new ArrayBuffer(0),
    ),
    /not in the current Rust readout/u,
  );
});
