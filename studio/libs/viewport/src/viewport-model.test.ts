import assert from 'node:assert/strict';
import test from 'node:test';

import {
  STUDIO_EDITOR_GRID,
  canvasPoint,
  movedPastPickThreshold,
  presentStudioSelection,
} from './viewport-model.js';
import { renderHandle, type RenderFrameDiff } from '@rusty-engine/render-contracts';

test('Studio grid is a disposable public Y-up renderer descriptor', () => {
  assert.equal(STUDIO_EDITOR_GRID.grid.coordinateSystem, 'rightHandedYUp');
  assert.equal(STUDIO_EDITOR_GRID.plane, 'xz');
  assert.deepEqual(STUDIO_EDITOR_GRID.grid.spacing, [0.5, 0.5, 0.5]);
});

test('canvas-relative picking distinguishes a click from camera orbit input', () => {
  assert.deepEqual(canvasPoint([151, 92], { left: 101, top: 42 }), [50, 50]);
  assert.equal(movedPastPickThreshold([10, 10], [13, 12]), false);
  assert.equal(movedPastPickThreshold([10, 10], [15, 10]), true);
});

test('selection and preview are disposable shared-renderer frame presentations', () => {
  const canonical: RenderFrameDiff = {
    schemaVersion: 1,
    ops: [{
      op: 'create',
      handle: renderHandle(7),
      parent: null,
      node: {
        geometry: { kind: 'cube' },
        material: { color: [0.2, 0.4, 0.6, 1], wireframe: false },
        transform: {
          translation: [1, 2, 3],
          rotation: [0, 0, 0, 1],
          scale: [2, 2, 2],
        },
        visible: true,
        layer: 'scene',
        metadata: { sourceEntity: 42, sourceSceneNode: 9, tags: [], label: 'selected' },
      },
    }],
  };

  const selected = presentStudioSelection(canonical, 42, null, null);
  assert.equal(selected.selectedHandle, 7);
  assert.equal(selected.previewApplied, false);
  assert.equal(selected.voxelPreviewKind, null);
  assert.equal(selected.frame.ops.at(-1)?.op, 'update');

  const preview = presentStudioSelection(canonical, 42, 42, {
    translation: [5, 6, 7],
    rotation: [0, Math.SQRT1_2, 0, Math.SQRT1_2],
    scale: [3, 4, 5],
  });
  assert.equal(preview.previewApplied, true);
  const previewUpdate = preview.frame.ops.at(-1);
  assert.equal(previewUpdate?.op, 'update');
  if (previewUpdate?.op === 'update') {
    assert.deepEqual(previewUpdate.transform, {
      translation: [5, 6, 7],
      rotation: [0, Math.SQRT1_2, 0, Math.SQRT1_2],
      scale: [3, 4, 5],
    });
  }

  const cancelled = presentStudioSelection(canonical, 42, null, null);
  assert.deepEqual(cancelled.frame, selected.frame);
  assert.deepEqual(canonical.ops[0], canonical.ops[0]);

  const brush = presentStudioSelection(canonical, 42, null, null, {
    kind: 'brush',
    transform: {
      translation: [1.5, 2.5, 3.5],
      rotation: [0, Math.SQRT1_2, 0, Math.SQRT1_2],
      scale: [0.25, 0.5, 0.75],
    },
    radius: 1,
    mode: 'erase',
  });
  assert.equal(brush.previewApplied, true);
  assert.equal(brush.voxelPreviewKind, 'brush');
  const brushCreate = brush.frame.ops.at(-1);
  assert.equal(brushCreate?.op, 'create');
  if (brushCreate?.op === 'create') {
    assert.equal(brushCreate.node.layer, 'debug');
    assert.deepEqual(brushCreate.node.transform.translation, [1.5, 2.5, 3.5]);
    assert.deepEqual(brushCreate.node.transform.rotation, [0, Math.SQRT1_2, 0, Math.SQRT1_2]);
    assert.deepEqual(brushCreate.node.transform.scale, [0.75, 1.5, 2.25]);
  }
  assert.deepEqual(
    presentStudioSelection(canonical, 42, null, null).frame,
    selected.frame,
  );

  const conversion = presentStudioSelection(canonical, null, null, null, {
    kind: 'conversion',
    cellSize: 2,
    samples: [
      { coordinate: [0, 0, 0], materialSlot: 7 },
      { coordinate: [2, 1, -1], materialSlot: 8 },
    ],
  });
  assert.equal(conversion.previewApplied, true);
  assert.equal(conversion.voxelPreviewKind, 'conversion');
  assert.equal(conversion.frame.ops.length, canonical.ops.length + 2);
  const firstSample = conversion.frame.ops[canonical.ops.length];
  assert.equal(firstSample?.op, 'create');
  if (firstSample?.op === 'create') {
    assert.deepEqual(firstSample.node.transform.translation, [1, 1, 1]);
  }
});

test('animated selection preserves transform preview without replacing imported materials', () => {
  const canonical: RenderFrameDiff = {
    schemaVersion: 1,
    ops: [{
      op: 'createAnimatedMeshInstance',
      handle: renderHandle(17),
      parent: null,
      instance: {
        asset: 'character',
        transform: {
          translation: [1, 2, 3],
          rotation: [0, 0, 0, 1],
          scale: [1, 1, 1],
        },
        visible: true,
        materialOverrides: [],
        playback: {
          kind: 'play',
          clip: 'run',
          loop: 'repeat',
          speed: 1,
          weight: 1,
          restart: false,
          fadeSeconds: null,
        },
        metadata: {
          sourceEntity: 71,
          sourceSceneNode: 9,
          tags: [],
          label: 'animated character',
        },
      },
    }],
  };
  const previewTransform = {
    translation: [4, 5, 6] as const,
    rotation: [0, 0, 0, 1] as const,
    scale: [2, 3, 4] as const,
  };

  const presentation = presentStudioSelection(canonical, 71, 71, previewTransform);

  const selectionUpdate = presentation.frame.ops.at(-1);
  assert.equal(selectionUpdate?.op, 'update');
  if (selectionUpdate?.op === 'update') {
    assert.deepEqual(selectionUpdate.transform, previewTransform);
    assert.equal(selectionUpdate.material, null);
  }
});
