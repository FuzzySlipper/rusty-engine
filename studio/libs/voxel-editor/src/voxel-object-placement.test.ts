import assert from 'node:assert/strict';
import test from 'node:test';

import type {
  VoxelObjectAssetAuthoringReadout,
  VoxelObjectInstanceReadout,
} from '@rusty-engine/studio-adapter-client';

import {
  MAX_VOXEL_OBJECT_PLACEMENT_ID_BYTES,
  MAX_VOXEL_OBJECT_PLACEMENT_MATERIAL_OVERRIDES,
  MAX_VOXEL_OBJECT_PLACEMENT_PALETTE_ASSETS,
  boundedVoxelObjectPlacementPalette,
  buildVoxelObjectPlacementCandidate,
  duplicateVoxelObjectInstance,
  nextVoxelObjectInstanceId,
} from './voxel-object-placement.js';

void test('placement candidates keep the closed attach value and renderer ghost selection aligned', () => {
  const candidate = buildVoxelObjectPlacementCandidate({
    sceneId: 'scene/main',
    asset: objectAsset(),
    instanceId: 'wall-a',
    clipId: 'clip/idle',
    frameIndex: 1,
    translation: [4, 1, 8],
    rotation: [0, 0, 0, 1],
    scale: [0.5, 0.5, 0.5],
    materialOverrides: [{ materialSlot: 7, materialAssetId: 'material/accent' }],
    canonicalMaterialIds: new Set(['material/base', 'material/accent']),
  });

  assert.equal(candidate.instance.voxelObjectAssetId, 'voxel-object/wall');
  assert.deepEqual(candidate.instance.frame, { kind: 'clip', clipId: 'clip/idle', frameIndex: 1 });
  assert.equal(candidate.presentation.frameId, 'clip/idle/1');
  assert.deepEqual(candidate.presentation.transform.translation, candidate.instance.translation);
  assert.deepEqual(candidate.presentation.materialOverrides, [{ slot: 7, material: 'material/accent' }]);
});

void test('placement rejects invalid transforms, stale clips, unknown materials, and duplicate override slots', () => {
  const base = {
    sceneId: 'scene/main',
    asset: objectAsset(),
    instanceId: 'wall-a',
    clipId: '',
    frameIndex: 0,
    translation: [0, 0, 0],
    rotation: [0, 0, 0, 1],
    scale: [1, 1, 1],
    materialOverrides: [],
    canonicalMaterialIds: new Set(['material/base']),
  } as const;
  assert.throws(
    () => buildVoxelObjectPlacementCandidate({ ...base, scale: [1, 0, 1] }),
    /scale axes must be greater/u,
  );
  assert.throws(
    () => buildVoxelObjectPlacementCandidate({ ...base, clipId: 'clip/missing' }),
    /Unknown voxel-object clip/u,
  );
  assert.throws(
    () => buildVoxelObjectPlacementCandidate({
      ...base,
      materialOverrides: [{ materialSlot: 7, materialAssetId: 'material/missing' }],
    }),
    /not a canonical project material/u,
  );
  assert.throws(
    () => buildVoxelObjectPlacementCandidate({
      ...base,
      materialOverrides: [
        { materialSlot: 7, materialAssetId: 'material/base' },
        { materialSlot: 7, materialAssetId: 'material/base' },
      ],
    }),
    /slot 7 is duplicated/u,
  );
  assert.throws(
    () => buildVoxelObjectPlacementCandidate({
      ...base,
      materialOverrides: Array.from(
        { length: MAX_VOXEL_OBJECT_PLACEMENT_MATERIAL_OVERRIDES + 1 },
        (_, index) => ({ materialSlot: index, materialAssetId: 'material/base' }),
      ),
    }),
    /at most 32 material overrides/u,
  );
});

void test('palette, identities, and duplicate candidates stay bounded and reuse object content', () => {
  const assets = Array.from(
    { length: MAX_VOXEL_OBJECT_PLACEMENT_PALETTE_ASSETS + 4 },
    (_, index) => objectAsset(`voxel-object/wall-${String(index)}`),
  );
  assert.equal(
    boundedVoxelObjectPlacementPalette(assets).length,
    MAX_VOXEL_OBJECT_PLACEMENT_PALETTE_ASSETS,
  );
  assert.equal(nextVoxelObjectInstanceId('wall', new Set(['wall', 'wall-2'])), 'wall-3');
  assert.equal(
    nextVoxelObjectInstanceId('wall-2', new Set(['wall', 'wall-2'])),
    'wall-3',
  );
  assert.throws(
    () => nextVoxelObjectInstanceId('x'.repeat(MAX_VOXEL_OBJECT_PLACEMENT_ID_BYTES + 1), new Set()),
    /exceeds 128/u,
  );

  const source: VoxelObjectInstanceReadout = {
    sceneId: 'scene/main',
    ownerEntityId: 9,
    instance: {
      instanceId: 'wall',
      voxelObjectAssetId: 'voxel-object/wall',
      frame: { kind: 'default' },
      translation: [1, 2, 3],
      rotation: [0, 0, 0, 1],
      scale: [1, 1, 1],
      materialOverrides: [{ materialSlot: 7, materialAssetId: 'material/accent' }],
    },
  };
  const duplicate = duplicateVoxelObjectInstance(source, new Set(['wall-copy']), 0.25);
  assert.equal(duplicate.instanceId, 'wall-copy-2');
  assert.equal(duplicate.voxelObjectAssetId, source.instance.voxelObjectAssetId);
  assert.deepEqual(duplicate.translation, [1.25, 2, 3]);
  assert.deepEqual(duplicate.materialOverrides, source.instance.materialOverrides);
  assert.notStrictEqual(duplicate.materialOverrides, source.instance.materialOverrides);
});

function objectAsset(assetId = 'voxel-object/wall'): VoxelObjectAssetAuthoringReadout {
  const frame = {
    bounds: { min: [0, 0, 0] as const, max: [2, 2, 2] as const },
    voxelDataHash: 'sha256:frame',
    voxelCount: 8,
    sparseRunCount: 2,
    durationMicroseconds: null,
  };
  return {
    assetId,
    contentHash: 'sha256:wall',
    grid: {
      coordinateSystem: 'rightHandedYUp',
      cellSize: 0.25,
      chunkSize: 16,
      pivot: [0, 0, 0],
    },
    bounds: frame.bounds,
    defaultFrame: frame,
    clips: [{
      clipId: 'clip/idle',
      name: 'Idle',
      framesPerSecond: 4,
      frames: [frame, { ...frame, voxelDataHash: 'sha256:frame-2' }],
    }],
    defaultClip: 'clip/idle',
    materialPalette: [{
      materialSlot: 7,
      materialAssetId: 'material/base',
      displayName: 'Base',
    }],
    materialMap: [{ sourceMaterialSlot: 0, voxelMaterialSlot: 7 }],
    provenance: {
      kind: 'convertedStaticMesh',
      sourcePath: 'content/wall.glb',
      sourceSha256: 'sha256:source',
      sourceByteCount: 128,
      converter: 'test',
      settingsSha256: 'sha256:settings',
      licensePath: null,
      sourceClips: [],
    },
  };
}
