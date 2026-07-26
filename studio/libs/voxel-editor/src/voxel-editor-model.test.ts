import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildVoxelObjectClipControl,
  buildVoxelObjectClipControlForSource,
  deriveVoxelPickValidation,
} from './voxel-editor-model.js';

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

test('maps transient clip controls to stable closed Rust clip requests', () => {
  const available = [
    {
      sourceAnimationIndex: 3,
      name: 'Walk Cycle',
      durationMicroseconds: 1_000_000,
      channelCount: 2,
      targetNodeIndices: [0],
      properties: ['translation' as const],
    },
    {
      sourceAnimationIndex: 7,
      name: 'Idle',
      durationMicroseconds: 2_000_000,
      channelCount: 1,
      targetNodeIndices: [0],
      properties: ['rotation' as const],
    },
  ];
  const output = buildVoxelObjectClipControl(available, {
    selectedSourceClipNames: ['Idle', 'Walk Cycle'],
    sampleRateHz: 12,
    startSeconds: 0.25,
    endSeconds: '0.75',
    endPolicy: 'excludeLoopSeam',
    defaultSourceClipName: 'Idle',
  });

  assert.deepEqual(output.clips.map((clip) => clip.outputClipId), [
    'clip/walk-cycle-4', 'clip/idle-8',
  ]);
  assert.equal(output.clips[0]?.startMicroseconds, 250_000);
  assert.equal(output.clips[0]?.endMicroseconds, 750_000);
  assert.equal(output.defaultClip, 'clip/idle-8');
  assert.deepEqual(output.initialFrame, {
    kind: 'clip', clipId: 'clip/walk-cycle-4', frameIndex: 0,
  });
});

test('automatic clip identities are independent of locale-sensitive casing', () => {
  assert.equal('IDLE'.toLocaleLowerCase('tr-TR'), 'ıdle');
  const output = buildVoxelObjectClipControl([{
    sourceAnimationIndex: 0,
    name: 'IDLE',
    durationMicroseconds: 1_000_000,
    channelCount: 1,
    targetNodeIndices: [0],
    properties: ['rotation'],
  }], {
    selectedSourceClipNames: ['IDLE'],
    sampleRateHz: 12,
    startSeconds: 0,
    endSeconds: '',
    endPolicy: 'excludeLoopSeam',
    defaultSourceClipName: 'IDLE',
  });

  assert.equal(output.clips[0]?.outputClipId, 'clip/idle-1');
  assert.equal(output.defaultClip, 'clip/idle-1');
});

test('static object controls ignore hidden stale animation values', () => {
  const output = buildVoxelObjectClipControlForSource('static', [], {
    selectedSourceClipNames: ['stale-clip'],
    sampleRateHz: Number.NaN,
    startSeconds: -1,
    endSeconds: '-2',
    endPolicy: 'includeClipEnd',
    defaultSourceClipName: 'stale-clip',
  });

  assert.deepEqual(output, {
    clips: [],
    initialFrame: { kind: 'default' },
  });
});

test('rejects invalid transient clip ranges before invoking the adapter', () => {
  assert.throws(
    () => buildVoxelObjectClipControl([], {
      selectedSourceClipNames: [],
      sampleRateHz: 241,
      startSeconds: 1,
      endSeconds: '0',
      endPolicy: 'includeClipEnd',
      defaultSourceClipName: '',
    }),
    /end must be greater/,
  );
});
