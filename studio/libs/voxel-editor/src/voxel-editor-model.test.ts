import assert from 'node:assert/strict';
import test from 'node:test';

import { deriveVoxelPickValidation } from './voxel-editor-model.js';

test('derives asset-local untrusted pick claims through transformed instances', () => {
  const result = deriveVoxelPickValidation(
    {
      instanceId: 'wall',
      cameraOrigin: [4.5, 0.5, 0],
      direction: [0, 0, 1],
      worldPoint: [4.5, 0.5, 6],
      worldNormal: [0, 0, -1],
      maxDistance: 20,
    },
    'scene/wall',
    {
      instanceId: 'wall',
      voxelAssetId: 'voxel-volume/wall',
      translation: [0, 0, 0],
      rotation: [0, 0, 0, 1],
      scale: [1, 1, 1],
    },
    {
      inspection: {
        assetId: 'voxel-volume/wall',
        schemaVersion: 1,
        cellSize: 1,
        chunkSize: 16,
        origin: [4, 0, 6],
        boundsMin: [0, 0, 0],
        boundsMax: [1, 1, 1],
        representedVoxelCount: 8,
        sparseRunCount: 4,
        materialCounts: [],
        voxelDataHash: 'data',
        contentHash: 'content',
        provenanceKind: 'convertedStaticMesh',
        provenanceSource: 'wall.glb',
        diagnostics: {},
      },
      palette: [],
      history: {
        persisted: false,
        entryCount: 0,
        cursor: 0,
        undoDepth: 0,
        redoDepth: 0,
        authorityHash: 'authority',
        historyHash: 'history',
      },
      annotations: [],
    },
  );

  assert.deepEqual(result?.claimedVoxel, [0, 0, 0]);
  assert.equal(result?.claimedFace, 'negativeZ');
});
