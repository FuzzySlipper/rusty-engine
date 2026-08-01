import assert from 'node:assert/strict';
import test from 'node:test';

import {
  STUDIO_EDITOR_GRID,
  canvasPoint,
  movedPastPickThreshold,
  presentStudioLighting,
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

test('work light replaces authored lights only in the disposable presentation', () => {
  const canonical: RenderFrameDiff = {
    schemaVersion: 1,
    ops: [
      {
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
          metadata: { sourceEntity: 42, sourceSceneNode: 9, tags: [], label: 'lit object' },
        },
      },
      {
        op: 'createLight',
        handle: renderHandle(8),
        parent: null,
        light: {
          kind: 'directional',
          color: [0.2, 0.3, 0.4],
          intensity: 4,
          enabled: true,
          direction: [1, -1, 0],
          shadowIntent: 'requested',
        },
      },
    ],
  };
  const before = JSON.stringify(canonical);

  const work = presentStudioLighting(canonical, 'work_light');

  assert.equal(work.workLightActive, true);
  assert.equal(work.authoredLightCount, 1);
  assert.equal(work.activeLightCount, 2);
  assert.equal(work.frame.ops.some((operation) => 'handle' in operation && operation.handle === 8), false);
  const workLights = work.frame.ops.filter((operation) => operation.op === 'createLight');
  assert.deepEqual(workLights.map((operation) => operation.light.kind), ['ambient', 'directional']);
  assert.equal(workLights.every((operation) => operation.light.shadowIntent === 'disabled'), true);
  assert.equal(JSON.stringify(canonical), before);

  const authored = presentStudioLighting(canonical, 'authored_lights');
  assert.equal(authored.workLightActive, false);
  assert.equal(authored.activeLightCount, 1);
  assert.equal(authored.frame, canonical);
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

test('grounding inspection adds only disposable triad bounds and contact-plane lines', () => {
  const canonical: RenderFrameDiff = { schemaVersion: 1, ops: [] };
  const inspection = {
    origin: [2, 3, 4] as const,
    bounds: { min: [1, 0.5, 3] as const, max: [3, 5, 6] as const },
    contactPlaneY: 0,
    clearance: 0.5,
  };
  const presentation = presentStudioSelection(
    canonical,
    null,
    null,
    null,
    null,
    null,
    inspection,
  );
  const diagnostics = presentation.frame.ops.filter((operation) =>
    operation.op === 'create'
    && operation.node.metadata.tags.includes('grounding-inspection'));
  assert.equal(diagnostics.length, 19);
  assert.equal(presentation.previewApplied, false);
  assert.equal(canonical.ops.length, 0);
  assert.equal(diagnostics.every((operation) =>
    operation.op === 'create'
    && operation.node.geometry.kind === 'line'
    && operation.node.layer === 'debug'), true);

  const malformed = presentStudioSelection(canonical, null, null, null, null, null, {
    ...inspection,
    clearance: Number.NaN,
  });
  assert.equal(malformed.frame.ops.length, 0);

  const withBrush = presentStudioSelection(canonical, null, null, null, {
    kind: 'brush',
    transform: {
      translation: [0, 0, 0],
      rotation: [0, 0, 0, 1],
      scale: [1, 1, 1],
    },
    radius: 1,
    mode: 'paint',
  }, null, inspection);
  const createdHandles = withBrush.frame.ops.flatMap((operation) =>
    operation.op === 'create' ? [operation.handle] : []);
  assert.equal(new Set(createdHandles).size, createdHandles.length);
  assert.equal(withBrush.previewApplied, true);
  assert.equal(withBrush.voxelPreviewKind, 'brush');
});

test('voxel-object placement adds one disposable instance while reusing the canonical definition', () => {
  const canonical: RenderFrameDiff = {
    schemaVersion: 1,
    ops: [
      {
        op: 'defineMaterial',
        material: {
          schemaVersion: 1,
          id: 'material/base',
          color: [0.4, 0.5, 0.6, 1],
          texture: null,
          roughness: 1,
          textureTint: [1, 1, 1, 1],
          emissionColor: [0, 0, 0],
          emissionIntensity: 0,
          uvStrategy: 'flat',
        },
      },
      {
        op: 'defineMaterial',
        material: {
          schemaVersion: 1,
          id: 'material/accent',
          color: [0.9, 0.4, 0.2, 1],
          texture: null,
          roughness: 1,
          textureTint: [1, 1, 1, 1],
          emissionColor: [0, 0, 0],
          emissionIntensity: 0,
          uvStrategy: 'flat',
        },
      },
      {
        op: 'defineVoxelObject',
        asset: {
          asset: 'voxel-object/wall',
          contentHash: 'sha256:wall',
          meshes: [{
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
              groups: [{ materialSlot: 7, start: 0, count: 3 }],
              bounds: { min: [0, 0, 0], max: [1, 1, 0] },
              source: {
                kind: 'inline',
                positions: [0, 0, 0, 1, 0, 0, 0, 1, 0],
                normals: [0, 0, 1, 0, 0, 1, 0, 0, 1],
                indices: [0, 1, 2],
              },
              provenance: 'voxelObject',
            },
          }],
          frames: [{ id: 'default', mesh: 0 }, { id: 'clip/idle/0', mesh: 0 }],
          materialSlots: [{ slot: 7, material: 'material/base' }],
        },
      },
    ],
  };
  const presentation = presentStudioSelection(canonical, null, null, null, {
    kind: 'objectPlacement',
    assetId: 'voxel-object/wall',
    assetContentHash: 'sha256:wall',
    frameId: 'clip/idle/0',
    transform: {
      translation: [4, 1, 8],
      rotation: [0, 0, 0, 1],
      scale: [0.5, 0.5, 0.5],
    },
    materialOverrides: [{ slot: 7, material: 'material/accent' }],
    label: 'Place wall-a',
  });

  assert.equal(presentation.previewApplied, true);
  assert.equal(presentation.voxelPreviewKind, 'objectPlacement');
  assert.equal(
    presentation.frame.ops.filter((operation) => operation.op === 'defineVoxelObject').length,
    1,
  );
  const ghost = presentation.frame.ops.at(-1);
  assert.equal(ghost?.op, 'createVoxelObjectInstance');
  if (ghost?.op === 'createVoxelObjectInstance') {
    assert.equal(ghost.instance.asset, 'voxel-object/wall');
    assert.equal(ghost.instance.frame, 1);
    assert.deepEqual(ghost.instance.materialOverrides, [{ slot: 7, material: 'material/accent' }]);
    assert.deepEqual(ghost.instance.metadata.tags, ['studio-preview', 'voxel-object-placement-ghost']);
    const parent = presentation.frame.ops.at(-2);
    assert.equal(parent?.op, 'create');
    if (parent?.op === 'create') assert.equal(parent.node.layer, 'debug');
  }

  const stale = presentStudioSelection(canonical, null, null, null, {
    kind: 'objectPlacement',
    assetId: 'voxel-object/wall',
    assetContentHash: 'sha256:stale',
    frameId: 'default',
    transform: {
      translation: [0, 0, 0],
      rotation: [0, 0, 0, 1],
      scale: [1, 1, 1],
    },
    materialOverrides: [],
    label: 'stale',
  });
  assert.equal(stale.previewApplied, false);
  assert.equal(stale.frame.ops.length, canonical.ops.length);

  const canonicalWithoutObject: RenderFrameDiff = {
    schemaVersion: 1,
    ops: canonical.ops.filter((operation) => operation.op !== 'defineVoxelObject'),
  };
  const objectDefinition = canonical.ops.find(
    (operation) => operation.op === 'defineVoxelObject',
  );
  assert.ok(objectDefinition !== undefined);
  const resourceOnly = presentStudioSelection(
    canonicalWithoutObject,
    null,
    null,
    null,
    {
      kind: 'objectPlacement',
      assetId: 'voxel-object/wall',
      assetContentHash: 'sha256:wall',
      frameId: 'default',
      transform: {
        translation: [2, 0, 3],
        rotation: [0, 0, 0, 1],
        scale: [1, 1, 1],
      },
      materialOverrides: [],
      label: 'Place unused wall',
    },
    { schemaVersion: 1, ops: [objectDefinition] },
  );
  assert.equal(resourceOnly.previewApplied, true);
  assert.equal(
    resourceOnly.frame.ops.filter((operation) => operation.op === 'defineVoxelObject').length,
    1,
  );
  assert.equal(resourceOnly.frame.ops.at(-1)?.op, 'createVoxelObjectInstance');

  const baseMaterial = canonical.ops[0];
  if (baseMaterial?.op !== 'defineMaterial') throw new Error('missing material fixture');
  const conflictingResource = presentStudioSelection(
    canonicalWithoutObject,
    null,
    null,
    null,
    {
      kind: 'objectPlacement',
      assetId: 'voxel-object/wall',
      assetContentHash: 'sha256:wall',
      frameId: 'default',
      transform: {
        translation: [0, 0, 0],
        rotation: [0, 0, 0, 1],
        scale: [1, 1, 1],
      },
      materialOverrides: [],
      label: 'conflict',
    },
    {
      schemaVersion: 1,
      ops: [
        {
          op: 'defineMaterial',
          material: {
            ...baseMaterial.material,
            color: [1, 0, 1, 1],
          },
        },
        objectDefinition,
      ],
    },
  );
  assert.equal(conflictingResource.previewApplied, false);
  assert.equal(conflictingResource.frame.ops.length, canonicalWithoutObject.ops.length);
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

test('voxel-object selection supports transform preview without replacing object materials', () => {
  const canonical: RenderFrameDiff = {
    schemaVersion: 1,
    ops: [{
      op: 'createVoxelObjectInstance',
      handle: renderHandle(33),
      parent: null,
      instance: {
        asset: 'voxel-object/wall',
        frame: 0,
        transform: {
          translation: [1, 2, 3],
          rotation: [0, 0, 0, 1],
          scale: [1, 1, 1],
        },
        visible: true,
        materialOverrides: [],
        metadata: {
          sourceEntity: 91,
          sourceSceneNode: 10,
          tags: ['voxel-object'],
          label: 'wall',
        },
      },
    }],
  };
  const transform = {
    translation: [5, 6, 7] as const,
    rotation: [0, 0, 0, 1] as const,
    scale: [2, 2, 2] as const,
  };

  const presentation = presentStudioSelection(canonical, 91, 91, transform);
  assert.equal(presentation.selectedHandle, 33);
  const update = presentation.frame.ops.at(-1);
  assert.equal(update?.op, 'update');
  if (update?.op === 'update') {
    assert.deepEqual(update.transform, transform);
    assert.equal(update.material, null);
  }
});
