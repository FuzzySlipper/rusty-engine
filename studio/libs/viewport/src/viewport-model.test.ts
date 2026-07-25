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
  assert.equal(selected.frame.ops.at(-1)?.op, 'update');

  const preview = presentStudioSelection(canonical, 42, 42, [5, 6, 7]);
  assert.equal(preview.previewApplied, true);
  const previewUpdate = preview.frame.ops.at(-1);
  assert.equal(previewUpdate?.op, 'update');
  if (previewUpdate?.op === 'update') {
    assert.deepEqual(previewUpdate.transform, {
      translation: [5, 6, 7],
      rotation: [0, 0, 0, 1],
      scale: [2, 2, 2],
    });
  }

  const cancelled = presentStudioSelection(canonical, 42, null, null);
  assert.deepEqual(cancelled.frame, selected.frame);
  assert.deepEqual(canonical.ops[0], canonical.ops[0]);
});
