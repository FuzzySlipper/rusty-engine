import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  audioHandle,
  ContractDecodeError,
  decodePresentationFrameDiff,
  decodeRenderFrameDiff,
  renderHandle,
} from './index.js';

const repoRoot = resolve(import.meta.dirname, '../../../..');

function fixture(name: string): unknown {
  return JSON.parse(readFileSync(resolve(repoRoot, 'fixtures/render', name), 'utf8')) as unknown;
}

function mutableFixture(name: string): Record<string, unknown> {
  return structuredClone(fixture(name)) as Record<string, unknown>;
}

void test('strict TypeScript decoders accept the committed Rust render fixtures', () => {
  const renderFrame = decodeRenderFrameDiff(fixture('retained-frame-v1.json'));
  assert.deepEqual(renderFrame.ops.map((operation) => operation.op), [
    'defineTexture',
    'setSkyBackground',
    'defineMaterial',
    'defineSpriteAtlas',
    'defineStaticMesh',
    'defineAnimatedMesh',
    'create',
    'update',
    'replaceMeshPayload',
    'createLight',
    'updateLight',
    'createStaticMeshInstance',
    'setMaterialInstanceParameters',
    'createAnimatedMeshInstance',
    'setAnimatedMeshPlayback',
    'createSprite',
    'updateSprite',
    'destroy',
  ]);
  assert.equal(decodePresentationFrameDiff(fixture('presentation-frame-v1.json')).ops.length, 5);
  const indicator = decodePresentationFrameDiff(fixture('world-indicator-frame-v1.json'));
  const billboard = indicator.ops[0];
  assert.equal(billboard?.domain, 'billboard');
  if (billboard?.domain !== 'billboard' || billboard.op.op !== 'create') {
    assert.fail('world indicator fixture must contain a billboard create');
  }
  assert.equal(billboard.op.descriptor.content.kind, 'structured');
});

void test('held animated samples accept inclusive boundaries and reject malformed normalized times', () => {
  const sample = (normalizedTime: unknown) => ({
    schemaVersion: 1,
    ops: [{
      op: 'setAnimatedMeshPlayback',
      handle: 4,
      playback: { kind: 'sample', clip: 'idle', normalizedTime },
    }],
  });
  assert.equal(
    (decodeRenderFrameDiff(sample(0)).ops[0] as { playback: { normalizedTime: number } }).playback.normalizedTime,
    0,
  );
  assert.equal(
    (decodeRenderFrameDiff(sample(1)).ops[0] as { playback: { normalizedTime: number } }).playback.normalizedTime,
    1,
  );
  for (const normalizedTime of [Number.NaN, Number.POSITIVE_INFINITY, -0.01, 1.01]) {
    assert.throws(() => decodeRenderFrameDiff(sample(normalizedTime)), ContractDecodeError);
  }
  assert.throws(
    () => decodeRenderFrameDiff({
      schemaVersion: 1,
      ops: [{ op: 'setAnimatedMeshPlayback', handle: 4, playback: { kind: 'sample', clip: 'idle' } }],
    }),
    ContractDecodeError,
  );
});

void test('clip-pack joint identities use decoded Three binding names only', () => {
  const frame = mutableFixture('retained-frame-v1.json');
  const operations = frame['ops'] as { op?: string; asset?: Record<string, unknown> }[];
  const animated = operations.find((operation) => operation.op === 'defineAnimatedMesh');
  assert.ok(animated?.asset);
  animated.asset!['clipPacks'] = [{
    asset: 'animation-clip-pack/test', runtimeFormat: 'glb', contentHash: `sha256:${'a'.repeat(64)}`,
    rig: {
      joints: [{ id: 'mixamorig:Hips', parent: null }], bindRestHash: `sha256:${'a'.repeat(64)}`,
      bindRestConvention: 'localMatrixV1', rootConvention: 'inPlace', rootJointId: 'mixamorig:Hips',
    },
    clips: [{ id: 'wave', name: 'wave', durationSeconds: 1 }],
    provenance: { producer: 'fixture', sourceHash: `sha256:${'a'.repeat(64)}`, targetHash: `sha256:${'a'.repeat(64)}`, license: 'CC0-1.0' },
  }];
  assert.throws(() => decodeRenderFrameDiff(frame), ContractDecodeError);
});

void test('sky backgrounds decode as a narrow nullable texture reference', () => {
  const frame = decodeRenderFrameDiff({
    schemaVersion: 1,
    ops: [
      { op: 'setSkyBackground', background: { texture: 'texture/sky/e1m1' } },
      { op: 'setSkyBackground', background: null },
    ],
  });
  assert.equal(frame.ops[0]?.op, 'setSkyBackground');
  assert.throws(
    () => decodeRenderFrameDiff({
      schemaVersion: 1,
      ops: [{ op: 'setSkyBackground', background: { texture: 'sprite/not-a-sky' } }],
    }),
    /texture\/ asset namespace/u,
  );
});

void test('particle decoding admits cubes, local collision, and the legacy sprite shape', () => {
  const cube = mutableFixture('presentation-frame-v1.json');
  const cubeOps = cube['ops'] as Array<Record<string, unknown>>;
  const cubeOperation = cubeOps.find((operation) => operation['domain'] === 'particle')!;
  const cubeOp = cubeOperation['op'] as Record<string, unknown>;
  const cubeDescriptor = cubeOp['descriptor'] as Record<string, unknown>;
  cubeDescriptor['visual'] = { kind: 'cube' };
  cubeDescriptor['flipbookFramesPerSecond'] = 0;
  cubeDescriptor['collision'] = {
    radius: 0.1,
    restitution: 0.45,
    friction: 0.2,
    maximumImpacts: 4,
    sleepSpeed: 0.1,
    limitBehavior: 'sleep',
    volumes: [
      { kind: 'plane', normal: [0, 1, 0], offset: -1 },
      { kind: 'aabb', minimum: [-1, -1, -1], maximum: [1, 1, 1] },
    ],
  };
  const decodedCube = decodePresentationFrameDiff(cube);
  assert.equal(decodedCube.ops[2]?.domain, 'particle');

  const legacy = mutableFixture('presentation-frame-v1.json');
  const legacyOperation = (legacy['ops'] as Array<Record<string, unknown>>)
    .find((operation) => operation['domain'] === 'particle')!;
  const legacyOp = legacyOperation['op'] as Record<string, unknown>;
  const legacyDescriptor = legacyOp['descriptor'] as Record<string, unknown>;
  const visual = legacyDescriptor['visual'] as Record<string, unknown>;
  legacyDescriptor['sprite'] = visual['sprite'];
  delete legacyDescriptor['visual'];
  assert.equal(decodePresentationFrameDiff(legacy).ops[2]?.domain, 'particle');

  legacyDescriptor['visual'] = { kind: 'cube' };
  assert.throws(() => decodePresentationFrameDiff(legacy), /exactly one of visual or legacy sprite/u);

  const collision = cubeDescriptor['collision'] as Record<string, unknown>;
  collision['volumes'] = [];
  assert.throws(() => decodePresentationFrameDiff(cube), /must contain 1\.\.=16 volumes/u);
  collision['volumes'] = [{ kind: 'plane', normal: [0, 2, 0], offset: 0 }];
  assert.throws(() => decodePresentationFrameDiff(cube), /must be normalized/u);
});

void test('sprite material decoding keeps legacy omission and validates bounded lighting modes', () => {
  const frame = mutableFixture('retained-frame-v1.json');
  const operation = (frame['ops'] as Array<Record<string, unknown>>)
    .find((candidate) => candidate['op'] === 'createSprite')!;
  const sprite = operation['sprite'] as Record<string, unknown>;
  assert.doesNotThrow(() => decodeRenderFrameDiff(frame), 'legacy material omission remains valid');

  sprite['material'] = {
    lighting: 'authoredNormal',
    normalTexture: 'texture/sprite-normal',
    depthTexture: null,
    normalStrength: 1.5,
    normalBias: 0.1,
    alpha: { kind: 'mask', cutoff: 0.45 },
    shadow: 'castAndReceive',
  };
  assert.doesNotThrow(() => decodeRenderFrameDiff(frame));

  const material = sprite['material'] as Record<string, unknown>;
  material['depthTexture'] = 'texture/depth';
  assert.throws(() => decodeRenderFrameDiff(frame), /texture references must match/u);
  material['depthTexture'] = null;
  material['normalStrength'] = 5;
  assert.throws(() => decodeRenderFrameDiff(frame), /must be in 0\.\.=4/u);
});

void test('structured indicator meters reject finite but unbounded magnitudes', () => {
  const frame = mutableFixture('world-indicator-frame-v1.json');
  const ops = frame['ops'] as Array<Record<string, unknown>>;
  const operation = ops[0]!;
  const op = operation['op'] as Record<string, unknown>;
  const descriptor = op['descriptor'] as Record<string, unknown>;
  const content = descriptor['content'] as Record<string, unknown>;
  const indicator = content['indicator'] as Record<string, unknown>;
  const meters = indicator['meters'] as Array<Record<string, unknown>>;
  meters[0]!['current'] = 1_500_000_000_000;
  meters[0]!['max'] = 2_000_000_000_000;
  assert.throws(() => decodePresentationFrameDiff(frame), /magnitude bound/u);
});

void test('static mesh collision decoding admits the trimesh policy', () => {
  const frame = mutableFixture('retained-frame-v1.json');
  const operations = frame['ops'] as Array<Record<string, unknown>>;
  const definition = operations.find((operation) => operation['op'] === 'defineStaticMesh');
  assert.ok(definition);
  const asset = definition['asset'] as Record<string, unknown>;
  asset['collision'] = { kind: 'trimesh' };
  assert.equal(decodeRenderFrameDiff(frame).ops.length, operations.length);
});

void test('static mesh decoding admits normalized colors and generic alpha-sided material policy', () => {
  const frame = mutableFixture('retained-frame-v1.json');
  const operations = frame['ops'] as Array<Record<string, unknown>>;
  const definition = operations.find((operation) => operation['op'] === 'defineStaticMesh')!;
  const asset = definition['asset'] as Record<string, unknown>;
  const payload = asset['payload'] as Record<string, unknown>;
  const layout = payload['layout'] as Record<string, unknown>;
  const vertexCount = layout['vertexCount'] as number;
  (layout['attributes'] as unknown[]).push({ name: 'color', components: 4, kind: 'f32' });
  const source = payload['source'] as Record<string, unknown>;
  source['colors'] = Array.from({ length: vertexCount * 4 }, (_, index) => index % 4 === 3 ? 1 : 0.5);

  const materialDefinition = operations.find((operation) => operation['op'] === 'defineMaterial')!;
  const material = materialDefinition['material'] as Record<string, unknown>;
  material['alphaMode'] = { kind: 'mask', cutoff: 0.5 };
  material['doubleSided'] = true;
  assert.doesNotThrow(() => decodeRenderFrameDiff(frame));

  (source['colors'] as number[])[0] = 1.1;
  assert.throws(() => decodeRenderFrameDiff(frame), /normalized to 0\.\.1/u);
});

void test('render decoding rejects unsafe handles and unknown nested fields', () => {
  const unsafe = mutableFixture('retained-frame-v1.json');
  const unsafeOps = unsafe['ops'] as Array<Record<string, unknown>>;
  const unsafeCreate = unsafeOps.find((operation) => operation['op'] === 'create');
  assert.ok(unsafeCreate);
  unsafeCreate['handle'] = Number.MAX_SAFE_INTEGER + 1;
  assert.throws(() => decodeRenderFrameDiff(unsafe), ContractDecodeError);

  const unknown = mutableFixture('retained-frame-v1.json');
  const unknownOps = unknown['ops'] as Array<Record<string, unknown>>;
  const unknownCreate = unknownOps.find((operation) => operation['op'] === 'create');
  assert.ok(unknownCreate);
  const node = unknownCreate['node'] as Record<string, unknown>;
  const metadata = node['metadata'] as Record<string, unknown>;
  metadata['authority'] = 'must-not-cross-render-border';
  assert.throws(() => decodeRenderFrameDiff(unknown), /authority is unknown/);
});

void test('typed handle constructors reject values that cannot cross JSON exactly', () => {
  assert.equal(renderHandle(Number.MAX_SAFE_INTEGER), Number.MAX_SAFE_INTEGER);
  assert.equal(audioHandle(0), 0);
  assert.throws(() => renderHandle(-1), RangeError);
  assert.throws(() => audioHandle(Number.MAX_SAFE_INTEGER + 1), RangeError);
});

void test('render decoding admits the closed camera-relative viewmodel layer', () => {
  const frame = decodeRenderFrameDiff({
    schemaVersion: 1,
    ops: [{
      op: 'create',
      handle: 1,
      parent: null,
      node: {
        geometry: { kind: 'group' },
        material: { color: [1, 1, 1, 1], wireframe: false },
        transform: {
          translation: [0, 0, 0],
          rotation: [0, 0, 0, 1],
          scale: [1, 1, 1],
        },
        visible: true,
        layer: 'viewmodel',
        metadata: {
          sourceEntity: null,
          sourceSceneNode: null,
          tags: [],
          label: 'camera-relative-root',
        },
      },
    }],
  });

  assert.equal(frame.ops[0]?.op, 'create');
  assert.equal(frame.ops[0]?.op === 'create' ? frame.ops[0].node.layer : null, 'viewmodel');
});

void test('texture payload decoding is strict, bounded, and content-addressed', () => {
  const digest = 'a'.repeat(64);
  const frame = {
    schemaVersion: 1,
    ops: [{
      op: 'defineTexture',
      texture: {
        id: 'texture/checker',
        width: 2,
        height: 1,
        filter: 'nearest',
        wrap: 'repeat',
        contentHash: `sha256:${digest}`,
        version: 1,
        payload: {
          encoding: 'pngRgba8',
          colorSpace: 'srgb',
          contentHash: `sha256:${digest}`,
          byteLength: 2,
          source: { kind: 'inline', encodedBytes: [137, 80] },
        },
      },
    }],
  };
  assert.equal(decodeRenderFrameDiff(frame).ops.length, 1);

  const drift = structuredClone(frame);
  drift.ops[0]!.texture.payload.contentHash = `sha256:${'b'.repeat(64)}`;
  assert.throws(() => decodeRenderFrameDiff(drift), /canonical texture content hash/u);

  const oversized = structuredClone(frame);
  oversized.ops[0]!.texture.width = 4_097;
  assert.throws(() => decodeRenderFrameDiff(oversized), /must be in 1\.\.=4096/u);

  const unknown = structuredClone(frame) as typeof frame & {
    ops: Array<{ texture: { payload: Record<string, unknown> } }>;
  };
  unknown.ops[0]!.texture.payload['runtimeUrl'] = 'https://must-not-cross.example';
  assert.throws(() => decodeRenderFrameDiff(unknown), /runtimeUrl is unknown/u);
});

void test('voxel surface material decoding preserves strict resolved atlas provenance', () => {
  const frame = {
    schemaVersion: 1,
    ops: [{
      op: 'defineMaterial',
      material: {
        schemaVersion: 2,
        id: 'material/stone',
        color: [1, 1, 1, 1],
        texture: 'texture/voxel-surfaces',
        roughness: 1,
        textureTint: [1, 1, 1, 1],
        emissionColor: [0, 0, 0],
        emissionIntensity: 0,
        uvStrategy: 'atlas',
        voxelSurface: {
          schemaVersion: 1,
          filter: 'linear',
          wrap: 'clamp',
          alphaMode: { kind: 'mask', cutoff: 0.5 },
          mapping: {
            kind: 'atlas',
            atlas: 'sprite-sheet/voxel-surfaces',
            atlasVersion: 1,
            atlasContentHash: 'bb01',
            texture: 'texture/voxel-surfaces',
            textureVersion: 2,
            textureContentHash: 'aa02',
            region: {
              id: 'stone',
              contentMin: [2, 2],
              contentExtent: [28, 28],
              padding: { left: 1, right: 1, bottom: 1, top: 1 },
              inset: 'halfTexel',
            },
            tileScaleCells: [1, 2],
            tileOriginCells: [-4, 8],
          },
        },
      },
    }],
  };
  assert.equal(decodeRenderFrameDiff(frame).ops[0]?.op, 'defineMaterial');

  const insufficientPadding = structuredClone(frame);
  insufficientPadding.ops[0]!.material.voxelSurface.mapping.region.padding.left = 0;
  assert.throws(() => decodeRenderFrameDiff(insufficientPadding), /padding.left/);

  const mismatchedTexture = structuredClone(frame);
  mismatchedTexture.ops[0]!.material.voxelSurface.mapping.texture = 'texture/other';
  assert.throws(() => decodeRenderFrameDiff(mismatchedTexture), /must match material texture/);
});

void test('presentation decoding rejects unsafe identities, sequence gaps, and nested drift', () => {
  const unsafe = mutableFixture('presentation-frame-v1.json');
  const unsafeOps = unsafe['ops'] as Array<Record<string, unknown>>;
  const billboard = unsafeOps[1]!['op'] as Record<string, unknown>;
  billboard['handle'] = Number.MAX_SAFE_INTEGER + 1;
  assert.throws(() => decodePresentationFrameDiff(unsafe), /safe integer/);

  const gap = mutableFixture('presentation-frame-v1.json');
  const gapOps = gap['ops'] as Array<Record<string, unknown>>;
  const meta = gapOps[2]!['meta'] as Record<string, unknown>;
  meta['sequence'] = 7;
  assert.throws(() => decodePresentationFrameDiff(gap), /must equal ordered index 2/);

  const unknown = mutableFixture('presentation-frame-v1.json');
  const unknownOps = unknown['ops'] as Array<Record<string, unknown>>;
  const billboardOp = unknownOps[1]!['op'] as Record<string, unknown>;
  const descriptor = billboardOp['descriptor'] as Record<string, unknown>;
  const content = descriptor['content'] as Record<string, unknown>;
  content['sendMessage'] = 'no';
  assert.throws(() => decodePresentationFrameDiff(unknown), /sendMessage is unknown/);
});

void test('voxel-object resources decode strictly and reject invalid frame references', () => {
  const payload = {
    layout: {
      vertexCount: 3, indexCount: 3, indexWidth: 'u32',
      attributes: [
        { name: 'position', components: 3, kind: 'f32' },
        { name: 'normal', components: 3, kind: 'f32' },
      ],
    },
    groups: [{ materialSlot: 1, start: 0, count: 3 }],
    bounds: { min: [0, 0, 0], max: [1, 1, 0] },
    source: {
      kind: 'inline',
      positions: [0, 0, 0, 1, 0, 0, 0, 1, 0],
      normals: [0, 0, 1, 0, 0, 1, 0, 0, 1],
      indices: [0, 1, 2],
    },
    provenance: 'voxelObject',
  };
  const frame = {
    schemaVersion: 1,
    ops: [
      {
        op: 'defineVoxelObject',
        asset: {
          asset: 'voxel-object/runner', contentHash: 'sha256:runner',
          meshes: [{ payload }], frames: [{ id: 'default', mesh: 0 }],
          materialSlots: [{ slot: 1, material: 'material/runner' }],
        },
      },
      {
        op: 'createVoxelObjectInstance', handle: 7, parent: null,
        instance: {
          asset: 'voxel-object/runner', frame: 0,
          transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
          visible: true, materialOverrides: [],
          metadata: { sourceEntity: 7, sourceSceneNode: null, tags: [], label: 'runner' },
        },
      },
      { op: 'setVoxelObjectFrame', handle: 7, frame: 0 },
      { op: 'destroy', handle: 7 },
      { op: 'releaseVoxelObject', asset: 'voxel-object/runner' },
    ],
  };
  assert.equal(decodeRenderFrameDiff(frame).ops.length, 5);
  const invalid = structuredClone(frame);
  const definition = invalid.ops[0]!.asset as { frames: Array<{ id: string; mesh: number }> };
  definition.frames[0]!.mesh = 1;
  assert.throws(() => decodeRenderFrameDiff(invalid), /must be in 0\.\.=0/);
});

void test('content-addressed mesh resources validate identity, layout, and bounds', () => {
  const digest = '1'.repeat(64);
  const frame = {
    schemaVersion: 1,
    ops: [{
      op: 'replaceMeshPayload',
      handle: 1,
      payload: {
        layout: {
          vertexCount: 3,
          indexCount: 3,
          indexWidth: 'u32',
          attributes: [
            { name: 'position', components: 3, kind: 'f32' },
            { name: 'normal', components: 3, kind: 'f32' },
          ],
        },
        groups: [{ materialSlot: 0, start: 0, count: 3 }],
        bounds: { min: [0, 0, 0], max: [1, 1, 0] },
        source: {
          kind: 'resource',
          resource: `mesh-resource/${digest}`,
          contentHash: `sha256:${digest}`,
          byteLength: 100,
          encoding: 'packedStreamsLeV1',
          positionsByteOffset: 16,
          normalsByteOffset: 52,
          indicesByteOffset: 88,
        },
        provenance: 'voxelObject',
      },
    }],
  };
  assert.equal(decodeRenderFrameDiff(frame).ops.length, 1);

  const wrongIdentity = structuredClone(frame);
  wrongIdentity.ops[0]!.payload.source.resource = `mesh-resource/${'2'.repeat(64)}`;
  assert.throws(() => decodeRenderFrameDiff(wrongIdentity), /content-addressed/u);

  const outside = structuredClone(frame);
  outside.ops[0]!.payload.source.byteLength = 99;
  assert.throws(() => decodeRenderFrameDiff(outside), /outside the resource/u);

  type MutableMeshFrame = {
    ops: Array<{
      payload: {
        layout: { attributes: Array<Record<string, unknown>> };
        source: Record<string, unknown>;
      };
    }>;
  };
  const textured = structuredClone(frame) as unknown as MutableMeshFrame;
  textured.ops[0]!.payload.layout.attributes.push({ name: 'uv', components: 2, kind: 'f32' });
  textured.ops[0]!.payload.source['encoding'] = 'packedStreamsLeV2';
  textured.ops[0]!.payload.source['uvsByteOffset'] = 88;
  textured.ops[0]!.payload.source['indicesByteOffset'] = 112;
  textured.ops[0]!.payload.source['byteLength'] = 124;
  assert.equal(decodeRenderFrameDiff(textured).ops.length, 1);

  const mismatched = structuredClone(textured);
  mismatched.ops[0]!.payload.source['encoding'] = 'packedStreamsLeV1';
  assert.throws(() => decodeRenderFrameDiff(mismatched), /encoding and optional streams/u);

  const malformedInline = structuredClone(frame) as unknown as MutableMeshFrame;
  malformedInline.ops[0]!.payload.layout.attributes.push({ name: 'uv', components: 2, kind: 'f32' });
  malformedInline.ops[0]!.payload.source = {
    kind: 'inline',
    positions: [0, 0, 0, 1, 0, 0, 0, 1, 0],
    normals: [0, 0, 1, 0, 0, 1, 0, 0, 1],
    uvs: [0, 0, 1, 0, 16_777_218, 1],
    indices: [0, 1, 2],
  };
  assert.throws(() => decodeRenderFrameDiff(malformedInline), /exact f32 integer range/u);
});
