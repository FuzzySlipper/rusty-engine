import assert from 'node:assert/strict';
import test from 'node:test';

import type { Transform } from '@rusty-engine/render-contracts';
import {
  beginStudioTransformManipulatorDrag,
  cancelStudioTransformManipulatorDrag,
  projectStudioTransformManipulator,
  studioTransformHandleFromId,
  studioTransformHandleId,
  updateStudioTransformManipulatorDrag,
  type StudioTransformHandle,
  type StudioTransformManipulatorCamera,
  type StudioTransformSnapping,
} from './transform-manipulator.js';

const SOURCE: Transform = {
  translation: [5, 2, -3],
  rotation: [0, 0, 0, 1],
  scale: [1, 1, 1],
};

const CAMERA: StudioTransformManipulatorCamera = {
  position: [5, 2, 7],
  basis: {
    forward: [0, 0, -1],
    right: [1, 0, 0],
    up: [0, 1, 0],
  },
  fovYDegrees: 60,
  viewport: { width: 800, height: 800 },
};

const SNAPPING: StudioTransformSnapping = {
  enabled: true,
  rotationDegrees: 15,
  scale: [0.1, 0.1, 0.1],
  translation: [0.25, 0.25, 0.25],
};

function drag(
  handle: StudioTransformHandle,
  pointer: readonly [number, number],
  source: Transform = SOURCE,
) {
  return beginStudioTransformManipulatorDrag({
    camera: CAMERA,
    handle,
    orientation: 'world',
    pointer,
    revision: 'scene-revision:7',
    snapping: SNAPPING,
    source,
  });
}

test('manipulator projects stable overlay handles at the selected world transform', () => {
  const frame = projectStudioTransformManipulator({
    active: null,
    hovered: { kind: 'axis', tool: 'translate', axis: 0 },
    tool: 'translate',
    orientation: 'world',
    transform: SOURCE,
    visible: true,
  });

  assert.equal(frame.ops.length, 6);
  for (const operation of frame.ops) {
    assert.equal(operation.op, 'create');
    if (operation.op !== 'create') continue;
    assert.equal(operation.node.layer, 'debug');
    assert.equal(operation.node.metadata.sourceEntity, null);
    assert.ok(operation.node.metadata.tags.includes('studio-transform-manipulator'));
    assert.match(operation.node.metadata.label ?? '', /^studio-transform-manipulator:translate:/);
    assert.ok(operation.node.transform.translation[0] > 4);
    assert.ok(operation.node.transform.translation[1] > 1);
    assert.ok(operation.node.transform.translation[2] > -4);
  }
  const x = { kind: 'axis', tool: 'translate', axis: 0 } as const;
  assert.deepEqual(studioTransformHandleFromId(studioTransformHandleId(x)), x);
});

test('axis and plane translation follow camera rays with snap and fine adjustment', () => {
  const axisDrag = drag({ kind: 'axis', tool: 'translate', axis: 0 }, [400, 400]);
  const axisCandidate = updateStudioTransformManipulatorDrag(axisDrag, CAMERA, [480, 400]);
  assert.ok(axisCandidate.transform.translation[0] > SOURCE.translation[0] + 1);
  assert.equal(axisCandidate.transform.translation[0] % 0.25, 0);
  assert.deepEqual(axisCandidate.transform.translation.slice(1), SOURCE.translation.slice(1));

  const offGridSource: Transform = {
    ...SOURCE,
    translation: [5, 2.13, -3.07],
  };
  const offGridDrag = drag(
    { kind: 'axis', tool: 'translate', axis: 0 },
    [400, 400],
    offGridSource,
  );
  const offGridCandidate = updateStudioTransformManipulatorDrag(offGridDrag, CAMERA, [480, 400]);
  assert.deepEqual(offGridCandidate.transform.translation.slice(1), [2.13, -3.07]);

  const fineCandidate = updateStudioTransformManipulatorDrag(
    axisDrag,
    CAMERA,
    [480, 400],
    { fine: true },
  );
  assert.ok(
    fineCandidate.transform.translation[0] - SOURCE.translation[0]
      < axisCandidate.transform.translation[0] - SOURCE.translation[0],
  );

  const planeDrag = drag({ kind: 'plane', tool: 'translate', plane: 'xy' }, [400, 400]);
  const planeCandidate = updateStudioTransformManipulatorDrag(
    planeDrag,
    CAMERA,
    [480, 320],
    { snapping: false },
  );
  assert.ok(planeCandidate.transform.translation[0] > SOURCE.translation[0]);
  assert.ok(planeCandidate.transform.translation[1] > SOURCE.translation[1]);
  assert.equal(planeCandidate.transform.translation[2], SOURCE.translation[2]);
});

test('rotation, axis scale, uniform scale, and cancellation retain explicit preview semantics', () => {
  const rotationDrag = drag({ kind: 'axis', tool: 'rotate', axis: 2 }, [520, 400]);
  const rotationCandidate = updateStudioTransformManipulatorDrag(rotationDrag, CAMERA, [400, 280]);
  assert.equal(rotationCandidate.previewOnly, true);
  assert.equal(rotationCandidate.revision, 'scene-revision:7');
  assert.ok(Math.abs(rotationCandidate.transform.rotation[2]) > 0.1);
  assert.ok(Math.abs(Math.hypot(...rotationCandidate.transform.rotation) - 1) < 1e-9);

  const axisScaleDrag = drag({ kind: 'axis', tool: 'scale', axis: 0 }, [400, 400]);
  const axisScaleCandidate = updateStudioTransformManipulatorDrag(
    axisScaleDrag,
    CAMERA,
    [460, 400],
    { snapping: false },
  );
  assert.ok(axisScaleCandidate.transform.scale[0] > 1);
  assert.deepEqual(axisScaleCandidate.transform.scale.slice(1), [1, 1]);

  const offGridScaleSource: Transform = { ...SOURCE, scale: [1, 1.07, 1.03] };
  const offGridScaleDrag = drag(
    { kind: 'axis', tool: 'scale', axis: 0 },
    [400, 400],
    offGridScaleSource,
  );
  const offGridScale = updateStudioTransformManipulatorDrag(offGridScaleDrag, CAMERA, [460, 400]);
  assert.deepEqual(offGridScale.transform.scale.slice(1), [1.07, 1.03]);

  const uniformDrag = drag({ kind: 'uniform', tool: 'scale' }, [400, 400]);
  const uniformCandidate = updateStudioTransformManipulatorDrag(
    uniformDrag,
    CAMERA,
    [460, 340],
    { snapping: false },
  );
  assert.ok(uniformCandidate.transform.scale[0] > 1);
  assert.equal(uniformCandidate.transform.scale[0], uniformCandidate.transform.scale[1]);
  assert.equal(uniformCandidate.transform.scale[1], uniformCandidate.transform.scale[2]);

  const cancelled = cancelStudioTransformManipulatorDrag(uniformDrag);
  assert.deepEqual(cancelled.transform, SOURCE);
  assert.match(cancelled.diagnostics[0] ?? '', /cancelled/);
});

test('local axes rotate with the selected transform and degeneracies stay finite', () => {
  const half = Math.sin(Math.PI / 4);
  const rotatedSource: Transform = {
    ...SOURCE,
    rotation: [0, 0, half, half],
  };
  const localDrag = beginStudioTransformManipulatorDrag({
    camera: CAMERA,
    handle: { kind: 'axis', tool: 'translate', axis: 0 },
    orientation: 'local',
    pointer: [400, 400],
    revision: 'scene-revision:8',
    snapping: SNAPPING,
    source: rotatedSource,
  });
  const candidate = updateStudioTransformManipulatorDrag(
    localDrag,
    CAMERA,
    [400, 320],
    { snapping: false },
  );
  assert.ok(candidate.transform.translation[1] > rotatedSource.translation[1]);
  assert.ok(Math.abs(candidate.transform.translation[0] - rotatedSource.translation[0]) < 1e-6);
  assert.ok(candidate.transform.scale.every((value) => Number.isFinite(value) && value > 0));

  assert.throws(
    () => beginStudioTransformManipulatorDrag({
      camera: { ...CAMERA, viewport: { width: 0, height: 800 } },
      handle: { kind: 'uniform', tool: 'scale' },
      orientation: 'world',
      pointer: [0, 0],
      revision: 'bad-camera',
      snapping: SNAPPING,
      source: SOURCE,
    }),
    /viewport must be positive/,
  );
});
