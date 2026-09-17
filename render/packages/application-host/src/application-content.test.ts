import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { test } from 'node:test';

import {
  RustyApplicationResourceCatalog,
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

void test('application content borrows immutable Engine storage and normalizes subviews', () => {
  const source = new Uint8Array([1, 2, 3, 4]);
  const prepared = prepareRustyApplicationContent(textureContent(source));
  assert.equal(prepared.resources[0]!.bytes, source.buffer);
  const subview = prepareRustyApplicationContent(textureContent(source.subarray(1, 3)));
  assert.deepEqual(new Uint8Array(subview.resources[0]!.bytes), new Uint8Array([2, 3]));
});

void test('application content trusts resource metadata and leaves decoding to the renderer', () => {
  const resource = textureContent(new Uint8Array()).resources![0]!;
  const prepared = prepareRustyApplicationContent({
    frame: { schemaVersion: 1, ops: [] },
    resources: [{ ...resource, contentHash: 'engine-key', mediaType: 'image/custom' }],
  });
  assert.equal(prepared.resources[0]!.contentHash, 'engine-key');
  assert.equal(prepared.resources[0]!.mediaType, 'image/custom');
});

void test('resource catalog prunes a retired dynamic resource and admits its immutable identity again', async () => {
  const catalog = new RustyApplicationResourceCatalog();
  const content = textureContent();
  await catalog.admit(content.resources!);
  assert.deepEqual(catalog.readout(), { resources: 1, animated: 0, clipPacks: 0 });
  assert.equal(catalog.snapshot()[0]?.identity, content.resources![0]!.identity);

  catalog.retainOnly(new Set());
  assert.deepEqual(catalog.readout(), { resources: 0, animated: 0, clipPacks: 0 });
  assert.deepEqual(catalog.snapshot(), []);

  await catalog.admit(content.resources!);
  assert.deepEqual(catalog.readout(), { resources: 1, animated: 0, clipPacks: 0 });
  assert.equal(catalog.snapshot()[0]?.identity, content.resources![0]!.identity);
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
  const resolver = rustyApplicationAudioResourceResolver(prepared);
  assert.ok(resolver !== null);
  const resolved = await resolver({ asset: 'audio/test-swing', contentHash: `sha256:${digest}` });
  assert.deepEqual(new Uint8Array(resolved.bytes).slice(0, 4), new Uint8Array([82, 73, 70, 70]));
  assert.equal(resolved.bytes, prepared.resources[0]!.bytes);
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
