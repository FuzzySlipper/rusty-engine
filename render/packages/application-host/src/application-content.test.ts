import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { test } from 'node:test';
import { RUSTY_RENDERER_TEXTURE_RESOURCE_MAX_COUNT } from '@rusty-engine/renderer-host';

import {
  RustyApplicationContentError,
  prepareRustyApplicationContent,
  rustyApplicationSurfaceResourceOptions,
  type RustyApplicationContent,
} from './application-content.js';

function textureContent(bytes = new Uint8Array([137, 80, 78, 71])): RustyApplicationContent {
  const digest = createHash('sha256').update(bytes).digest('hex');
  return {
    frame: { schemaVersion: 1, ops: [] },
    resources: [{
      identity: `texture-resource/${digest}`,
      contentHash: `sha256:${digest}`,
      mediaType: 'image/png',
      bytes,
    }],
  };
}

void test('application content snapshots caller bytes and derives private resource options', async () => {
  const source = new Uint8Array([137, 80, 78, 71]);
  const prepared = prepareRustyApplicationContent(textureContent(source));
  source.fill(0);
  assert.deepEqual(
    new Uint8Array(prepared.resources[0]!.bytes),
    new Uint8Array([137, 80, 78, 71]),
  );
  const options = rustyApplicationSurfaceResourceOptions(prepared);
  const descriptor = options.textureResourceManifest?.resources[0];
  assert.ok(descriptor !== undefined);
  const resolved = await options.resolveTextureResource?.(descriptor);
  assert.deepEqual(new Uint8Array(resolved!), new Uint8Array([137, 80, 78, 71]));
  new Uint8Array(resolved!).fill(0);
  assert.deepEqual(
    new Uint8Array(prepared.resources[0]!.bytes),
    new Uint8Array([137, 80, 78, 71]),
  );
});

void test('application content rejects duplicated identities without exposing renderer manifests', () => {
  const content = textureContent();
  assert.throws(
    () => prepareRustyApplicationContent({
      ...content,
      resources: [content.resources![0]!, content.resources![0]!],
    }),
    (error: unknown) => error instanceof RustyApplicationContentError
      && error.code === 'resource_duplicate',
  );
});

void test('application content rejects mismatched hashes, unsupported media, and empty resources', () => {
  const content = textureContent();
  const resource = content.resources![0]!;
  assert.throws(
    () => prepareRustyApplicationContent({
      ...content,
      resources: [{ ...resource, contentHash: `sha256:${'0'.repeat(64)}` }],
    }),
    (error: unknown) => error instanceof RustyApplicationContentError
      && error.code === 'resource_identity_invalid',
  );
  assert.throws(
    () => prepareRustyApplicationContent({
      ...content,
      resources: [{ ...resource, mediaType: 'application/octet-stream' }],
    }),
    (error: unknown) => error instanceof RustyApplicationContentError
      && error.code === 'resource_media_type_unsupported',
  );
  assert.throws(
    () => prepareRustyApplicationContent(textureContent(new Uint8Array())),
    (error: unknown) => error instanceof RustyApplicationContentError
      && error.code === 'resource_limit_exceeded',
  );
});

void test('application content admits both closed resource families and enforces count bounds', () => {
  const meshBytes = new Uint8Array(16);
  const meshDigest = createHash('sha256').update(meshBytes).digest('hex');
  const texture = textureContent().resources![0]!;
  const prepared = prepareRustyApplicationContent({
    frame: { schemaVersion: 1, ops: [] },
    resources: [
      {
        identity: `mesh-resource/${meshDigest}`,
        contentHash: `sha256:${meshDigest}`,
        mediaType: 'application/octet-stream',
        bytes: meshBytes,
      },
      texture,
    ],
  });
  const options = rustyApplicationSurfaceResourceOptions(prepared);
  assert.equal(options.meshResourceManifest?.resources.length, 1);
  assert.equal(options.textureResourceManifest?.resources.length, 1);

  assert.throws(
    () => prepareRustyApplicationContent({
      frame: { schemaVersion: 1, ops: [] },
      resources: Array.from(
        { length: RUSTY_RENDERER_TEXTURE_RESOURCE_MAX_COUNT + 1 },
        (_, index) => {
          const digest = index.toString(16).padStart(64, '0');
          return {
            identity: `texture-resource/${digest}`,
            contentHash: `sha256:${digest}`,
            mediaType: 'image/png',
            bytes: new Uint8Array([index & 0xff]),
          };
        },
      ),
    }),
    (error: unknown) => error instanceof RustyApplicationContentError
      && error.code === 'resource_limit_exceeded',
  );
});
