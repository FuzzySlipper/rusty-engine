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
});
