import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { test } from 'node:test';
import { loadRendererTextureResourceSource } from '@rusty-engine/renderer-host';

import {
  RustyApplicationContentError,
  prepareRustyApplicationContent,
  rustyApplicationAudioResourceResolver,
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

void test('application content borrows prepared bytes and renderer admission owns its snapshot', async () => {
  const expected = new Uint8Array([137, 80, 78, 71]);
  const source = expected.slice();
  const prepared = prepareRustyApplicationContent(textureContent(source));
  source.fill(0);
  assert.deepEqual(new Uint8Array(prepared.resources[0]!.bytes), expected);
  const options = rustyApplicationSurfaceResourceOptions(prepared);
  const manifest = options.textureResourceManifest;
  const resolve = options.resolveTextureResource;
  assert.ok(manifest !== undefined && resolve !== undefined);
  const descriptor = manifest.resources[0]!;
  const resolved = await resolve(descriptor);
  assert.equal(resolved, prepared.resources[0]!.bytes);
  assert.equal(await resolve(descriptor), resolved);
  const admitted = await loadRendererTextureResourceSource(manifest, resolve);
  // Once admitted, later changes by the prepared-content owner cannot change
  // the renderer's resource under its already-verified identity.
  new Uint8Array(prepared.resources[0]!.bytes).fill(0);
  const acquired = admitted.acquireResource(
    descriptor.resource, descriptor.contentHash, descriptor.byteLength,
  );
  assert.deepEqual(acquired.bytes, expected);
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

void test('application content admits closed resource families beyond retired texture and audio count caps', () => {
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

  const manyResources = prepareRustyApplicationContent({
    frame: { schemaVersion: 1, ops: [] },
    resources: [
      ...Array.from({ length: 257 }, (_, index) => {
        const digest = index.toString(16).padStart(64, '0');
        return {
          identity: `texture-resource/${digest}`,
          contentHash: `sha256:${digest}`,
          mediaType: 'image/png',
          bytes: new Uint8Array([index & 0xff]),
        };
      }),
      ...Array.from({ length: 65 }, (_, index) => {
        const digest = (index + 512).toString(16).padStart(64, '0');
        return {
          identity: `audio-resource/${digest}`,
          contentHash: `sha256:${digest}`,
          mediaType: 'audio/wav',
          bytes: new Uint8Array(44),
        };
      }),
    ],
  });
  assert.equal(manyResources.resources.length, 322);
});

void test('application content admits bounded WAV resources and resolves immutable audio bytes', async () => {
  const source = new Uint8Array(44);
  source.set([82, 73, 70, 70], 0);
  const digest = createHash('sha256').update(source).digest('hex');
  const prepared = prepareRustyApplicationContent({
    frame: { schemaVersion: 1, ops: [] },
    resources: [{
      identity: `audio-resource/${digest}`,
      contentHash: `sha256:${digest}`,
      mediaType: 'audio/wav',
      bytes: source,
    }],
  });
  source.fill(0);
  const resolver = rustyApplicationAudioResourceResolver(prepared);
  assert.ok(resolver !== null);
  const resolved = await resolver({ asset: 'audio/test-swing', contentHash: `sha256:${digest}` });
  assert.deepEqual(new Uint8Array(resolved.bytes).slice(0, 4), new Uint8Array([82, 73, 70, 70]));
  new Uint8Array(resolved.bytes).fill(0);
  const resolvedAgain = await resolver({
    asset: 'audio/test-swing',
    contentHash: `sha256:${digest}`,
  });
  assert.deepEqual(
    new Uint8Array(resolvedAgain.bytes).slice(0, 4),
    new Uint8Array([82, 73, 70, 70]),
  );
});

void test('audio resolver remains available for bus-only products without admitted clips', async () => {
  const resolver = rustyApplicationAudioResourceResolver(prepareRustyApplicationContent({
    frame: { schemaVersion: 1, ops: [] },
    resources: [],
  }));
  await assert.rejects(
    resolver({ asset: 'audio/missing', contentHash: `sha256:${'0'.repeat(64)}` }),
    /audio resource audio\/missing .* is unavailable/u,
  );
});

void test('application content rejects unsupported and undersized audio resources', () => {
  const bytes = new Uint8Array(44);
  const digest = createHash('sha256').update(bytes).digest('hex');
  const resource = {
    identity: `audio-resource/${digest}`,
    contentHash: `sha256:${digest}`,
    mediaType: 'audio/mpeg',
    bytes,
  };
  assert.throws(
    () => prepareRustyApplicationContent({
      frame: { schemaVersion: 1, ops: [] },
      resources: [resource],
    }),
    (error: unknown) => error instanceof RustyApplicationContentError
      && error.code === 'resource_media_type_unsupported',
  );
  assert.throws(
    () => {
      const shortBytes = new Uint8Array(43);
      const shortDigest = createHash('sha256').update(shortBytes).digest('hex');
      return prepareRustyApplicationContent({
        frame: { schemaVersion: 1, ops: [] },
        resources: [{
          identity: `audio-resource/${shortDigest}`,
          contentHash: `sha256:${shortDigest}`,
          mediaType: 'audio/wav',
          bytes: shortBytes,
        }],
      });
    },
    (error: unknown) => error instanceof RustyApplicationContentError
      && error.code === 'resource_limit_exceeded',
  );
});

void test('application content composes animated GLB, packed mesh, and texture resources', async () => {
  const animatedBytes = new Uint8Array(20).fill(7);
  animatedBytes.set([0x67, 0x6c, 0x54, 0x46]);
  const animatedDigest = createHash('sha256').update(animatedBytes).digest('hex');
  const meshBytes = new Uint8Array(16).fill(3);
  const meshDigest = createHash('sha256').update(meshBytes).digest('hex');
  const prepared = prepareRustyApplicationContent({
    frame: {
      schemaVersion: 1,
      ops: [{
        op: 'defineAnimatedMesh',
        asset: {
          asset: 'mesh-animation/test-actor',
          runtimeFormat: 'glb',
          contentHash: `sha256:${animatedDigest}`,
          clips: [{ id: 'idle', name: 'idle', durationSeconds: 1 }],
          clipPacks: [{
            asset: 'animation-clip-pack/test-actor-idle', runtimeFormat: 'glb', contentHash: `sha256:${animatedDigest}`,
            rig: {
              joints: [{ id: 'Root', parent: null }], bindRestHash: `sha256:${animatedDigest}`,
              bindRestConvention: 'localMatrixV1', rootConvention: 'inPlace', rootJointId: 'Root',
              structuralRootIds: ['Root'], designatedMotionRootIds: [], authoredPoseTranslationJointIds: [],
            },
            clips: [{ id: 'pack-idle', name: 'idle', durationSeconds: 1 }],
            provenance: { producer: 'fixture', sourceHash: `sha256:${animatedDigest}`, targetHash: `sha256:${animatedDigest}`, license: 'CC0-1.0' },
          }],
          defaultClip: 'idle',
          embeddedMaterialSlots: [{ slot: 0, sourceMaterialSlot: 3 }],
          materialSlots: [],
          bounds: { min: [0, 0, 0], max: [1, 1, 1] },
        },
      }],
    },
    resources: [
      {
        identity: `animated-mesh-resource/${animatedDigest}`,
        contentHash: `sha256:${animatedDigest}`,
        mediaType: 'model/gltf-binary',
        bytes: animatedBytes,
      },
      {
        identity: `clip-pack-resource/${animatedDigest}`,
        contentHash: `sha256:${animatedDigest}`,
        mediaType: 'model/gltf-binary',
        bytes: animatedBytes,
      },
      {
        identity: `mesh-resource/${meshDigest}`,
        contentHash: `sha256:${meshDigest}`,
        mediaType: 'application/octet-stream',
        bytes: meshBytes,
      },
      textureContent().resources![0]!,
    ],
  });
  const options = rustyApplicationSurfaceResourceOptions(prepared);
  assert.equal(options.animatedMeshManifest?.resources.length, 1);
  assert.equal(options.animatedMeshManifest?.clipPacks?.length, 1);
  assert.equal(options.meshResourceManifest?.resources.length, 1);
  assert.equal(options.textureResourceManifest?.resources.length, 1);
  const descriptor = options.animatedMeshManifest?.resources[0];
  assert.deepEqual(descriptor, {
    asset: 'mesh-animation/test-actor',
    contentHash: `sha256:${animatedDigest}`,
    clipIds: ['idle'],
    clipSourceNames: ['idle'],
    embeddedMaterialSlots: [{ slot: 0, sourceMaterialSlot: 3 }],
  });
  assert.deepEqual(
    new Uint8Array(await options.resolveAnimatedMeshResource!(descriptor!)),
    animatedBytes,
  );
  assert.deepEqual(
    new Uint8Array(await options.resolveAnimatedMeshResource!(options.animatedMeshManifest!.clipPacks![0]!)),
    animatedBytes,
  );
});
