import assert from 'node:assert/strict';
import test from 'node:test';

import {
  STUDIO_EDITOR_GRID,
  canvasPoint,
  movedPastPickThreshold,
} from './viewport-model.js';

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
