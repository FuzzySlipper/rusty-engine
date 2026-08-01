import { test } from 'node:test';
import assert from 'node:assert/strict';

import type {
  RenderMaterialDescriptor,
  TextureDescriptor,
} from '@rusty-engine/render-contracts';
import {
  resolveVoxelSurfaceMaterial,
  sampleVoxelSurfaceUv,
  VoxelSurfaceMaterialError,
} from './voxel-surface-material.js';

function texture(
  version = 2,
  contentHash = 'texture-hash',
): TextureDescriptor {
  return {
    id: 'texture/atlas',
    width: 16,
    height: 8,
    filter: 'linear',
    wrap: 'clamp',
    contentHash,
    version,
    payload: {
      encoding: 'pngRgba8',
      colorSpace: 'srgb',
      contentHash: 'png-hash',
      byteLength: 1,
      source: { kind: 'inline', encodedBytes: [0] },
    },
  };
}

function material(): RenderMaterialDescriptor {
  return {
    schemaVersion: 3,
    id: 'material/atlas',
    color: [1, 1, 1, 1],
    texture: 'texture/atlas',
    roughness: 1,
    textureTint: [1, 1, 1, 1],
    emissionColor: [0, 0, 0],
    emissionIntensity: 0,
    uvStrategy: 'atlas',
    voxelSurface: {
      schemaVersion: 1,
      filter: 'linear',
      wrap: 'clamp',
      alphaMode: { kind: 'opaque' },
      mapping: {
        kind: 'atlas',
        atlas: 'sprite-sheet/atlas',
        atlasVersion: 3,
        atlasContentHash: 'atlas-hash',
        texture: 'texture/atlas',
        textureVersion: 2,
        textureContentHash: 'texture-hash',
        region: {
          id: 'stone',
          contentMin: [2, 2],
          contentExtent: [4, 4],
          padding: { left: 1, right: 1, bottom: 1, top: 1 },
          inset: 'halfTexel',
        },
        tileScaleCells: [2, 4],
        tileOriginCells: [-8, 12],
      },
    },
  };
}

void test('atlas reference sampling is Euclidean and continuous across independent chunk phases', () => {
  const readout = resolveVoxelSurfaceMaterial(material(), texture());
  assert.deepEqual(readout.sampleUvMin, [0.15625, 0.3125]);
  assert.deepEqual(readout.sampleUvMax, [0.34375, 0.6875]);
  assert.deepEqual(sampleVoxelSurfaceUv(readout, [-8, 12]), readout.sampleUvMin);
  assert.deepEqual(sampleVoxelSurfaceUv(readout, [-7, 14]), [0.25, 0.5]);
  assert.deepEqual(
    sampleVoxelSurfaceUv(readout, [-7 - 32, 14 + 64]),
    sampleVoxelSurfaceUv(readout, [-7, 14]),
  );
  assert.deepEqual(
    sampleVoxelSurfaceUv(readout, [25, -50]),
    sampleVoxelSurfaceUv(readout, [25 + 16, -50 - 32]),
  );
});

void test('stale, mismatched, and out-of-bounds retained texture facts fail closed', () => {
  assert.throws(
    () => resolveVoxelSurfaceMaterial(material(), texture(3)),
    (error: unknown) => error instanceof VoxelSurfaceMaterialError
      && /version 2/u.test(error.message),
  );
  assert.throws(
    () => resolveVoxelSurfaceMaterial(material(), texture(2, 'different')),
    /hash texture-hash/u,
  );
  const original = material();
  if (original.voxelSurface?.mapping.kind !== 'atlas') throw new Error('fixture');
  const outside: RenderMaterialDescriptor = {
    ...original,
    voxelSurface: {
      ...original.voxelSurface,
      mapping: {
        ...original.voxelSurface.mapping,
        region: { ...original.voxelSurface.mapping.region, contentMin: [15, 7] },
      },
    },
  };
  assert.throws(
    () => resolveVoxelSurfaceMaterial(outside, texture()),
    /exceeds texture\/atlas/u,
  );
});
