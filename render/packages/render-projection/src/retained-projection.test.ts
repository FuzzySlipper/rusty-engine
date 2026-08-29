import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  decodeRenderFrameDiff,
  renderHandle,
  type AnimatedMeshAsset,
  type MeshPayloadDescriptor,
  type RenderDiff,
  type RenderNode,
  type SpriteInstanceDescriptor,
  type StaticMeshAsset,
  type VoxelObjectRenderAsset,
} from '@rusty-engine/render-contracts';
import {
  MAX_VIEWMODEL_ASSET_EXTENT,
  MAX_VIEWMODEL_DISTINCT_ASSETS,
  MAX_VIEWMODEL_NODES,
  MAX_VIEWMODEL_TRANSLATION_COMPONENT,
  MAX_RETAINED_LIGHTS,
  RenderProjection,
  RenderProjectionError,
} from './index.js';

const repoRoot = resolve(import.meta.dirname, '../../../..');

void test('neutral projection retains validated lights and removes them with their parent', () => {
  const projection = new RenderProjection();
  projection.applyFrame({ schemaVersion: 1, ops: [
    { op: 'create', handle: renderHandle(1), parent: null, node: cubeNode() },
    {
      op: 'createLight', handle: renderHandle(2), parent: renderHandle(1),
      light: {
        kind: 'spot', color: [1, 0.8, 0.6], intensity: 3, enabled: true,
        position: [0, 5, 0], direction: [0, -1, 0], range: 12, decay: 2,
        outerAngleRadians: 0.7, penumbra: 0.2, shadowIntent: 'requested',
      },
    },
  ] });
  assert.equal(projection.snapshot().lights[0]?.parent, renderHandle(1));
  projection.applyDiff({
    op: 'updateLight', handle: renderHandle(2),
    light: {
      kind: 'spot', color: [0.2, 0.4, 1], intensity: 1, enabled: false,
      position: [1, 4, 2], direction: [0, -1, 0], range: 8, decay: 1,
      outerAngleRadians: 0.5, penumbra: 0.5, shadowIntent: 'disabled',
    },
  });
  assert.equal(projection.light(renderHandle(2))?.light.enabled, false);
  projection.applyDiff({ op: 'destroy', handle: renderHandle(1) });
  assert.deepEqual(projection.snapshot().lights, []);
  assert.equal(projection.has(renderHandle(2)), false);
});

void test('neutral projection rejects malformed lights and kind-changing updates', () => {
  const projection = new RenderProjection();
  assert.throws(() => projection.applyDiff({
    op: 'createLight', handle: renderHandle(1), parent: null,
    light: {
      kind: 'directional', color: [1, 1, 1], intensity: 1, enabled: true,
      direction: [0, 0, 0], shadowIntent: 'disabled',
    },
  }), RenderProjectionError);
  projection.applyDiff({
    op: 'createLight', handle: renderHandle(1), parent: null,
    light: {
      kind: 'ambient', color: [1, 1, 1], intensity: 1, enabled: true,
      shadowIntent: 'disabled',
    },
  });
  assert.throws(() => projection.applyDiff({
    op: 'updateLight', handle: renderHandle(1),
    light: {
      kind: 'point', color: [1, 1, 1], intensity: 1, enabled: true,
      position: [0, 0, 0], range: null, decay: 2, shadowIntent: 'disabled',
    },
  }), /cannot change kind/);
});

void test('retained light quota admits the exact limit and rejects one over atomically', () => {
  const projection = new RenderProjection();
  projection.applyFrame({
    schemaVersion: 1,
    ops: Array.from({ length: MAX_RETAINED_LIGHTS }, (_, index) => ({
      op: 'createLight' as const,
      handle: renderHandle(index + 1),
      parent: null,
      light: {
        kind: 'ambient' as const,
        color: [1, 1, 1] as const,
        intensity: 1,
        enabled: true,
        shadowIntent: 'disabled' as const,
      },
    })),
  });
  assert.equal(projection.snapshot().lights.length, MAX_RETAINED_LIGHTS);
  assert.throws(() => projection.applyDiff({
    op: 'createLight', handle: renderHandle(MAX_RETAINED_LIGHTS + 1), parent: null,
    light: {
      kind: 'ambient', color: [1, 1, 1], intensity: 1, enabled: true,
      shadowIntent: 'disabled',
    },
  }), /retained light quota/u);
  assert.equal(projection.snapshot().lights.length, MAX_RETAINED_LIGHTS);
});

function cubeNode(label = 'cube'): RenderNode {
  return {
    geometry: { kind: 'cube' },
    material: { color: [1, 1, 1, 1], wireframe: false },
    transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
    visible: true,
    layer: 'scene',
    metadata: { sourceEntity: 1, sourceSceneNode: null, tags: [], label },
  };
}

function createPrimitive(handle: number, label = `node-${handle}`, parent: number | null = null): RenderDiff {
  return {
    op: 'create',
    handle: renderHandle(handle),
    parent: parent === null ? null : renderHandle(parent),
    node: cubeNode(label),
  };
}

function viewmodelRoot(handle = 1): RenderDiff {
  return {
    op: 'create',
    handle: renderHandle(handle),
    parent: null,
    node: {
      ...cubeNode('camera-relative-root'),
      geometry: { kind: 'group' },
      layer: 'viewmodel',
    },
  };
}

function quadPayload(): MeshPayloadDescriptor {
  return {
    layout: {
      vertexCount: 4,
      indexCount: 6,
      indexWidth: 'u32',
      attributes: [
        { name: 'position', components: 3, kind: 'f32' },
        { name: 'normal', components: 3, kind: 'f32' },
      ],
    },
    groups: [{ materialSlot: 1, start: 0, count: 6 }],
    bounds: { min: [0, 0, 0], max: [1, 1, 0] },
    source: {
      kind: 'inline',
      positions: [0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0],
      normals: [0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1],
      indices: [0, 1, 2, 0, 2, 3],
    },
    provenance: 'staticAsset',
  };
}

function meshAsset(asset = 'mesh/crate'): StaticMeshAsset {
  return {
    asset,
    payload: quadPayload(),
    materialSlots: [{ slot: 1, material: 'material/wood' }],
    collision: { kind: 'aabbFallback' },
  };
}

function largeMeshAsset(asset = 'mesh/large-unrelated'): StaticMeshAsset {
  const quadCount = 1_024;
  const positions: number[] = [];
  const normals: number[] = [];
  const indices: number[] = [];
  for (let quad = 0; quad < quadCount; quad += 1) {
    const x = quad % 32;
    const y = Math.floor(quad / 32);
    positions.push(
      x, y, 0,
      x + 1, y, 0,
      x + 1, y + 1, 0,
      x, y + 1, 0,
    );
    normals.push(0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1);
    const vertex = quad * 4;
    indices.push(vertex, vertex + 1, vertex + 2, vertex, vertex + 2, vertex + 3);
  }
  return {
    asset,
    payload: {
      layout: {
        vertexCount: quadCount * 4,
        indexCount: quadCount * 6,
        indexWidth: 'u32',
        attributes: [
          { name: 'position', components: 3, kind: 'f32' },
          { name: 'normal', components: 3, kind: 'f32' },
        ],
      },
      groups: [{ materialSlot: 1, start: 0, count: quadCount * 6 }],
      bounds: { min: [0, 0, 0], max: [32, 32, 0] },
      source: { kind: 'inline', positions, normals, indices },
      provenance: 'staticAsset',
    },
    materialSlots: [{ slot: 1, material: 'material/large' }],
    collision: { kind: 'visualOnly' },
  };
}

function animatedMeshAsset(asset = 'mesh-animation/kenney-retro-character-medium'): AnimatedMeshAsset {
  return {
    asset,
    runtimeFormat: 'glb',
    contentHash: 'sha256-fixture-pending',
    clips: [
      { id: 'idle', name: 'idle', durationSeconds: 1.2 },
      { id: 'run', name: 'run', durationSeconds: 0.8 },
      { id: 'jump', name: 'jump', durationSeconds: 0.6 },
    ],
    defaultClip: 'idle',
    materialSlots: [{ slot: 0, material: 'material/kenney-human-male-a' }],
    bounds: { min: [-0.5, 0, -0.5], max: [0.5, 1.8, 0.5] },
  };
}

function animatedMeshAssetWithClipPackOnlyClip(): AnimatedMeshAsset {
  return {
    ...animatedMeshAsset(),
    clipPacks: [{
      asset: 'animation-clip-pack/character-gestures',
      runtimeFormat: 'glb',
      contentHash: `sha256:${'a'.repeat(64)}`,
      rig: {
        joints: [{ id: 'mixamorig:Hips', parent: null }],
        bindRestHash: `sha256:${'a'.repeat(64)}`,
        bindRestConvention: 'localMatrixV1',
        rootConvention: 'inPlace',
        rootJointId: 'mixamorig:Hips',
        structuralRootIds: ['mixamorig:Hips'],
        designatedMotionRootIds: [],
        authoredPoseTranslationJointIds: [],
      },
      clips: [{ id: 'gesture', name: 'gesture', durationSeconds: 0.75 }],
      provenance: {
        producer: 'fixture',
        sourceHash: `sha256:${'a'.repeat(64)}`,
        targetHash: `sha256:${'a'.repeat(64)}`,
        license: 'CC0-1.0',
      },
    }],
  };
}

function voxelObjectAsset(): VoxelObjectRenderAsset {
  return {
    asset: 'voxel-object/runner',
    contentHash: 'sha256:runner',
    meshes: [
      { payload: { ...quadPayload(), provenance: 'voxelObject' } },
      { payload: { ...quadPayload(), provenance: 'voxelObject', bounds: { min: [0, 0, 0], max: [2, 1, 0] } } },
    ],
    frames: [{ id: 'default', mesh: 0 }, { id: 'walk/0', mesh: 1 }],
    materialSlots: [{ slot: 1, material: 'material/wood' }],
  };
}

function sprite(asset = 'sprite/ui', frame = 0): SpriteInstanceDescriptor {
  return {
    asset,
    frame,
    pivot: [0.5, 0.5],
    size: [2, 1],
    sizeMode: 'world',
    billboard: 'none',
    tint: [1, 1, 1, 1],
    renderOrder: 4,
    depth: 'default',
    shading: 'unlit',
    visible: true,
    transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
    attachment: { sourceEntity: 7, sourceSceneNode: null, attachmentPoint: 'head' },
    metadata: { sourceEntity: 7, sourceSceneNode: null, tags: [], label: 'sprite' },
  };
}

void test('applies frame ops in order and exposes neutral instructions', () => {
  const projection = new RenderProjection();
  const instructions = projection.applyFrame({
    schemaVersion: 1,
    ops: [
      createPrimitive(1),
      {
        op: 'update',
        handle: renderHandle(1),
        transform: { translation: [5, 0, 0], rotation: [0, 0, 0, 1], scale: [2, 2, 2] },
        material: null,
        visible: false,
        metadata: null,
      },
      { op: 'destroy', handle: renderHandle(1) },
    ],
  });

  assert.deepEqual(instructions.map((instruction) => instruction.op), [
    'upsertNode',
    'upsertNode',
    'removeNode',
  ]);
  assert.equal(projection.handleCount, 0);
});

void test('a rejected later operation rolls back the entire frame', () => {
  const projection = new RenderProjection();
  projection.applyDiff(createPrimitive(10, 'preexisting'));
  const before = projection.snapshot();

  assert.throws(
    () => projection.applyFrame({
      schemaVersion: 1,
      ops: [
        createPrimitive(11, 'must-not-commit'),
        {
          op: 'update',
          handle: renderHandle(999),
          transform: null,
          material: null,
          visible: false,
          metadata: null,
        },
      ],
    }),
    /unknown handle 999/,
  );

  assert.deepEqual(projection.snapshot(), before);
  assert.equal(projection.has(renderHandle(11)), false);
});

void test('published frames reject clipping and stale revision without retained mutation', () => {
  const projection = new RenderProjection();
  projection.applyFrame({
    schemaVersion: 1,
    publication: { stream: 'voxel:terrain', baseRevision: 0, revision: 1, operationCount: 1 },
    ops: [createPrimitive(21, 'terrain-chunk')],
  });
  const before = projection.snapshot();

  assert.throws(
    () => projection.applyFrame({
      schemaVersion: 1,
      publication: { stream: 'voxel:terrain', baseRevision: 1, revision: 2, operationCount: 2 },
      ops: [{
        op: 'update', handle: renderHandle(21), transform: null, material: null,
        visible: false, metadata: null,
      }],
    }),
    /operationCount/u,
  );
  assert.deepEqual(projection.snapshot(), before);
  assert.throws(
    () => projection.applyFrame({
      schemaVersion: 1,
      publication: { stream: 'voxel:terrain', baseRevision: 0, revision: 1, operationCount: 1 },
      ops: [{
        op: 'update', handle: renderHandle(21), transform: null, material: null,
        visible: false, metadata: null,
      }],
    }),
    /stale publication/u,
  );
  assert.deepEqual(projection.snapshot(), before);
  assert.throws(
    () => projection.applyFrame({
      schemaVersion: 1,
      publication: { stream: 'voxel:terrain', baseRevision: 2, revision: 3, operationCount: 0 },
      ops: [],
    }),
    /publication gap/u,
  );
  assert.deepEqual(projection.snapshot(), before);
  assert.throws(
    () => projection.applyFrame({
      schemaVersion: 1,
      publication: { stream: 'voxel:terrain', baseRevision: 1, revision: 3, operationCount: 1 },
      ops: [{
        op: 'update', handle: renderHandle(21), transform: null, material: null,
        visible: false, metadata: null,
      }],
    }),
    /publication gap/u,
  );
  assert.deepEqual(projection.snapshot(), before);

  projection.applyFrame({
    schemaVersion: 1,
    publication: { stream: 'voxel:terrain', baseRevision: 1, revision: 2, operationCount: 1 },
    ops: [{
      op: 'update', handle: renderHandle(21), transform: null, material: null,
      visible: false, metadata: null,
    }],
  });
  assert.equal(projection.snapshot().nodes[0]?.visible, false);
});

void test('small atomic frames structurally share unrelated retained definitions', () => {
  const projection = new RenderProjection();
  projection.applyFrame({
    schemaVersion: 1,
    ops: [
      { op: 'defineStaticMesh', asset: largeMeshAsset() },
      createPrimitive(1, 'moving-node'),
    ],
  });
  const retainedAsset = projection.staticMesh('mesh/large-unrelated');
  assert.ok(retainedAsset);

  for (let tick = 1; tick <= 8; tick += 1) {
    projection.applyFrame({
      schemaVersion: 1,
      ops: [{
        op: 'update',
        handle: renderHandle(1),
        transform: {
          translation: [tick, 0, 0],
          rotation: [0, 0, 0, 1],
          scale: [1, 1, 1],
        },
        material: null,
        visible: null,
        metadata: null,
      }],
    });
    assert.deepEqual(projection.lastFrameStagingStatistics(), {
      copiedNodeRecords: 1,
      copiedLightRecords: 0,
      copiedResourceRecords: 0,
      sharedDefinitionRecords: 1,
    });
  }

  assert.deepEqual(projection.staticMesh('mesh/large-unrelated'), retainedAsset);
  const beforeRejectedFrame = projection.snapshot();
  assert.throws(
    () => projection.applyFrame({
      schemaVersion: 1,
      ops: [
        {
          op: 'update',
          handle: renderHandle(1),
          transform: {
            translation: [99, 0, 0],
            rotation: [0, 0, 0, 1],
            scale: [1, 1, 1],
          },
          material: null,
          visible: null,
          metadata: null,
        },
        {
          op: 'update',
          handle: renderHandle(999),
          transform: null,
          material: null,
          visible: false,
          metadata: null,
        },
      ],
    }),
    /unknown handle 999/u,
  );
  assert.deepEqual(projection.snapshot(), beforeRejectedFrame);
  assert.deepEqual(projection.staticMesh('mesh/large-unrelated'), retainedAsset);

  assert.throws(
    () => projection.applyFrame({
      schemaVersion: 1,
      ops: [
        {
          op: 'createStaticMeshInstance',
          handle: renderHandle(2),
          parent: null,
          instance: {
            asset: 'mesh/large-unrelated',
            transform: {
              translation: [0, 0, 0],
              rotation: [0, 0, 0, 1],
              scale: [1, 1, 1],
            },
            visible: true,
            materialOverrides: [],
            metadata: {
              sourceEntity: 2,
              sourceSceneNode: null,
              tags: [],
              label: 'rejected-large-instance',
            },
          },
        },
        {
          op: 'update',
          handle: renderHandle(999),
          transform: null,
          material: null,
          visible: false,
          metadata: null,
        },
      ],
    }),
    /unknown handle 999/u,
  );
  assert.equal(projection.has(renderHandle(2)), false);
  assert.equal(projection.staticMeshRefCount('mesh/large-unrelated'), 0);
  assert.deepEqual(projection.snapshot(), beforeRejectedFrame);
});

void test('viewmodel descendants retain one bounded camera-relative channel', () => {
  const projection = new RenderProjection();
  projection.applyFrame({
    schemaVersion: 1,
    ops: [
      { op: 'defineStaticMesh', asset: meshAsset('mesh/viewmodel') },
      viewmodelRoot(),
      {
        op: 'createStaticMeshInstance',
        handle: renderHandle(2),
        parent: renderHandle(1),
        instance: {
          asset: 'mesh/viewmodel',
          transform: {
            translation: [0.4, -0.35, -1.2],
            rotation: [0, 0, 0, 1],
            scale: [0.5, 0.5, 0.5],
          },
          visible: true,
          materialOverrides: [],
          metadata: {
            sourceEntity: null,
            sourceSceneNode: null,
            tags: [],
            label: 'viewmodel-mesh',
          },
        },
      },
    ],
  });

  assert.equal(projection.node(renderHandle(1))?.layer, 'viewmodel');
  assert.equal(projection.node(renderHandle(2))?.layer, 'viewmodel');
  const before = projection.snapshot();
  assert.throws(() => projection.applyDiff({
    op: 'update',
    handle: renderHandle(2),
    transform: {
      translation: [MAX_VIEWMODEL_TRANSLATION_COMPONENT + 1, 0, 0],
      rotation: [0, 0, 0, 1],
      scale: [1, 1, 1],
    },
    material: null,
    visible: null,
    metadata: null,
  }), /viewmodel translation/);
  assert.throws(() => projection.applyDiff({
    op: 'update',
    handle: renderHandle(2),
    transform: {
      translation: [0, 0, 0],
      rotation: [0, 0, 0, 2],
      scale: [1, 1, 1],
    },
    material: null,
    visible: null,
    metadata: null,
  }), /viewmodel rotation/);
  assert.throws(() => projection.applyDiff({
    op: 'createLight',
    handle: renderHandle(3),
    parent: renderHandle(1),
    light: {
      kind: 'ambient',
      color: [1, 1, 1],
      intensity: 1,
      enabled: true,
      shadowIntent: 'disabled',
    },
  }), /backend-owned neutral light rig/);
  assert.throws(() => projection.applyDiff({
    op: 'create',
    handle: renderHandle(4),
    parent: renderHandle(1),
    node: {
      ...cubeNode('oversized-line'),
      geometry: {
        kind: 'line',
        a: [0, 0, 0],
        b: [MAX_VIEWMODEL_ASSET_EXTENT + 1, 0, 0],
      },
    },
  }), /viewmodel asset coordinates/);
  assert.deepEqual(projection.snapshot(), before);
});

void test('viewmodel node and distinct-asset capacities reject without partial mutation', () => {
  const projection = new RenderProjection();
  projection.applyDiff(viewmodelRoot());
  for (let index = 0; index < MAX_VIEWMODEL_DISTINCT_ASSETS; index += 1) {
    projection.applyDiff({
      op: 'createSprite',
      handle: renderHandle(index + 2),
      parent: renderHandle(1),
      sprite: sprite(`viewmodel/sprite-${String(index)}`),
    });
  }
  const assetBound = projection.snapshot();
  assert.throws(() => projection.applyDiff({
    op: 'createSprite',
    handle: renderHandle(100),
    parent: renderHandle(1),
    sprite: sprite('viewmodel/one-too-many'),
  }), /viewmodel asset capacity/);
  assert.deepEqual(projection.snapshot(), assetBound);

  for (
    let handle = MAX_VIEWMODEL_DISTINCT_ASSETS + 2;
    handle <= MAX_VIEWMODEL_NODES;
    handle += 1
  ) {
    projection.applyDiff(createPrimitive(handle, `viewmodel-node-${String(handle)}`, 1));
  }
  const nodeBound = projection.snapshot();
  assert.equal(nodeBound.nodes.filter((node) => node.layer === 'viewmodel').length, MAX_VIEWMODEL_NODES);
  assert.throws(() => projection.applyDiff(
    createPrimitive(MAX_VIEWMODEL_NODES + 1, 'one-too-many', 1),
  ), /viewmodel node capacity/);
  assert.deepEqual(projection.snapshot(), nodeBound);
});

void test('applies every operation in the committed retained fixture', () => {
  const input = JSON.parse(readFileSync(
    resolve(repoRoot, 'fixtures/render/retained-frame-v1.json'),
    'utf8',
  )) as unknown;
  const frame = decodeRenderFrameDiff(input);
  const projection = new RenderProjection();

  const instructions = projection.applyFrame(frame);

  assert.equal(instructions.length, frame.ops.length);
  assert.deepEqual(
    projection.snapshot().nodes.map((node) => [node.handle, node.kind]),
    [
      [renderHandle(1), 'primitive'],
      [renderHandle(3), 'staticMesh'],
      [renderHandle(4), 'animatedMesh'],
    ],
  );
  assert.equal(projection.light(renderHandle(2))?.light.kind, 'directional');
  assert.equal(projection.has(renderHandle(5)), false);
  assert.equal(projection.textureDescriptor('texture/checker')?.version, 1);
  assert.deepEqual(projection.snapshot().skyBackground, { texture: 'texture/checker' });
  assert.equal(projection.staticMeshRefCount('mesh/triangle'), 1);
  assert.equal(projection.animatedMeshRefCount('mesh-animation/character'), 1);
});

void test('sky background replacement and clear are fail-atomic retained presentation', () => {
  const projection = new RenderProjection();
  assert.throws(
    () => projection.applyDiff({
      op: 'setSkyBackground', background: { texture: 'texture/missing' },
    }),
    /not a retained payload/u,
  );
  assert.equal(projection.snapshot().skyBackground, null);
  const fixtureFrame = decodeRenderFrameDiff(JSON.parse(readFileSync(
    resolve(repoRoot, 'fixtures/render/retained-frame-v1.json'),
    'utf8',
  )) as unknown);
  const skyOps = fixtureFrame.ops.filter(
    (operation) => operation.op === 'defineTexture' || operation.op === 'setSkyBackground',
  );
  projection.applyFrame({ schemaVersion: 1, ops: skyOps });
  assert.deepEqual(projection.snapshot().skyBackground, { texture: 'texture/checker' });
  projection.applyDiff({ op: 'setSkyBackground', background: null });
  assert.equal(projection.snapshot().skyBackground, null);
});

void test('keeps stable parent/child ids and removes descendants before parents', () => {
  const projection = new RenderProjection();
  projection.applyFrame({
    schemaVersion: 1,
    ops: [createPrimitive(10, 'parent'), createPrimitive(11, 'child', 10)],
  });

  assert.deepEqual(projection.node(renderHandle(10))?.children, [renderHandle(11)]);
  assert.equal(projection.node(renderHandle(11))?.parent, renderHandle(10));

  const instructions = projection.applyDiff({ op: 'destroy', handle: renderHandle(10) });
  assert.deepEqual(instructions, [
    { op: 'removeNode', handle: renderHandle(11) },
    { op: 'removeNode', handle: renderHandle(10) },
  ]);
  assert.equal(projection.handleCount, 0);
});

void test('tracks static mesh definitions and fails closed on in-use redefinition', () => {
  const projection = new RenderProjection();
  projection.applyDiff({ op: 'defineStaticMesh', asset: meshAsset() });
  projection.applyDiff({
    op: 'createStaticMeshInstance',
    handle: renderHandle(1),
    parent: null,
    instance: {
      asset: 'mesh/crate',
      transform: { translation: [1, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
      visible: true,
      materialOverrides: [],
      metadata: { sourceEntity: 1, sourceSceneNode: null, tags: [], label: 'crate' },
    },
  });

  assert.equal(projection.staticMeshRefCount('mesh/crate'), 1);
  assert.deepEqual(projection.pickMesh(renderHandle(1)), {
    handle: renderHandle(1),
    provenance: 'staticAsset',
    sourceEntity: 1,
    sourceSceneNode: null,
  });
  assert.throws(
    () => projection.applyDiff({ op: 'defineStaticMesh', asset: meshAsset() }),
    RenderProjectionError,
  );

  projection.applyDiff({ op: 'destroy', handle: renderHandle(1) });
  assert.equal(projection.staticMeshRefCount('mesh/crate'), 0);

  const beforeRejectedFrame = projection.snapshot();
  assert.throws(
    () => projection.applyFrame({
      schemaVersion: 1,
      ops: [
        {
          op: 'createStaticMeshInstance',
          handle: renderHandle(2),
          parent: null,
          instance: {
            asset: 'mesh/missing',
            transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
            visible: true,
            materialOverrides: [],
            metadata: { sourceEntity: 2, sourceSceneNode: null, tags: [], label: 'missing' },
          },
        },
      ],
    }),
    /undefined static mesh asset mesh\/missing/,
  );
  assert.deepEqual(projection.snapshot(), beforeRejectedFrame);

  projection.applyFrame({
    schemaVersion: 1,
    ops: [{
      op: 'createStaticMeshInstance',
      handle: renderHandle(2),
      parent: null,
      instance: {
        asset: 'mesh/crate',
        transform: { translation: [2, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
        visible: true,
        materialOverrides: [],
        metadata: { sourceEntity: 2, sourceSceneNode: null, tags: [], label: 'recreated crate' },
      },
    }],
  });
  assert.equal(projection.staticMeshRefCount('mesh/crate'), 1);
  assert.equal(projection.node(renderHandle(2))?.metadata.label, 'recreated crate');
  projection.applyFrame({
    schemaVersion: 1,
    ops: [
      { op: 'destroy', handle: renderHandle(2) },
      {
        op: 'createStaticMeshInstance',
        handle: renderHandle(3),
        parent: null,
        instance: {
          asset: 'mesh/crate',
          transform: { translation: [3, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
          visible: true,
          materialOverrides: [],
          metadata: { sourceEntity: 3, sourceSceneNode: null, tags: [], label: 'same-frame crate' },
        },
      },
    ],
  });
  assert.equal(projection.has(renderHandle(2)), false);
  assert.equal(projection.node(renderHandle(3))?.metadata.label, 'same-frame crate');
  assert.equal(projection.staticMeshRefCount('mesh/crate'), 1);

  projection.applyDiff({ op: 'destroy', handle: renderHandle(3) });
  assert.doesNotThrow(() => projection.applyDiff({ op: 'defineStaticMesh', asset: meshAsset() }));
});

void test('voxel objects retain stable instances, explicit frames, and bounded resource lifetime', () => {
  const projection = new RenderProjection();
  const handle = renderHandle(31);
  projection.applyDiff({ op: 'defineVoxelObject', asset: voxelObjectAsset() });
  projection.applyDiff({
    op: 'createVoxelObjectInstance',
    handle,
    parent: null,
    instance: {
      asset: 'voxel-object/runner', frame: 0,
      transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
      visible: true, materialOverrides: [],
      metadata: { sourceEntity: 31, sourceSceneNode: null, tags: ['voxel-object'], label: 'runner' },
    },
  });
  assert.equal(projection.voxelObjectRefCount('voxel-object/runner'), 1);
  assert.equal(projection.node(handle)?.kind, 'voxelObject');
  assert.equal(projection.pickMesh(handle)?.provenance, 'voxelObject');

  projection.applyDiff({ op: 'setVoxelObjectFrame', handle, frame: 1 });
  const node = projection.node(handle);
  assert.equal(node?.kind, 'voxelObject');
  if (node?.kind === 'voxelObject') assert.equal(node.frame, 1);
  assert.throws(
    () => projection.applyDiff({ op: 'setVoxelObjectFrame', handle, frame: 99 }),
    /outside voxel object/,
  );
  assert.throws(
    () => projection.applyDiff({ op: 'releaseVoxelObject', asset: 'voxel-object/runner' }),
    /in use by 1 instance/,
  );
  projection.applyDiff({ op: 'destroy', handle });
  projection.applyDiff({ op: 'releaseVoxelObject', asset: 'voxel-object/runner' });
  assert.equal(projection.voxelObject('voxel-object/runner'), undefined);
});

void test('retains and resets handle-targeted material feedback parameters', () => {
  const projection = new RenderProjection();
  projection.applyDiff({ op: 'defineStaticMesh', asset: meshAsset() });
  projection.applyDiff({
    op: 'createStaticMeshInstance',
    handle: renderHandle(1),
    parent: null,
    instance: {
      asset: 'mesh/crate',
      transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
      visible: true,
      materialOverrides: [],
      metadata: { sourceEntity: 1, sourceSceneNode: null, tags: [], label: 'warning-light' },
    },
  });
  const parameters = {
    textureTint: [1, 0.2, 0.2, 1] as const,
    emissionColor: [1, 0, 0] as const,
    emissionIntensity: 2,
  };
  projection.applyDiff({
    op: 'setMaterialInstanceParameters',
    handle: renderHandle(1),
    slot: 1,
    parameters,
  });
  const active = projection.node(renderHandle(1));
  assert.equal(active?.kind, 'staticMesh');
  if (active?.kind === 'staticMesh') {
    assert.deepEqual(active.materialParameters, [{ slot: 1, parameters }]);
  }

  projection.applyDiff({
    op: 'setMaterialInstanceParameters',
    handle: renderHandle(1),
    slot: 1,
    parameters: null,
  });
  const reset = projection.node(renderHandle(1));
  assert.equal(reset?.kind, 'staticMesh');
  if (reset?.kind === 'staticMesh') {
    assert.deepEqual(reset.materialParameters, []);
  }
  assert.throws(
    () => projection.applyDiff({
      op: 'setMaterialInstanceParameters',
      handle: renderHandle(1),
      slot: 9,
      parameters,
    }),
    /unbound slot 9/,
  );
});

void test('tracks animated mesh definitions and command-selected named clip playback', () => {
  const projection = new RenderProjection();
  projection.applyDiff({ op: 'defineAnimatedMesh', asset: animatedMeshAsset() });
  projection.applyDiff({
    op: 'createAnimatedMeshInstance',
    handle: renderHandle(12),
    parent: null,
    instance: {
      asset: 'mesh-animation/kenney-retro-character-medium',
      transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
      materialOverrides: [],
      playback: null,
      visible: true,
      metadata: { sourceEntity: 12, sourceSceneNode: null, tags: [], label: 'animated-proof' },
    },
  });

  assert.equal(projection.animatedMeshRefCount('mesh-animation/kenney-retro-character-medium'), 1);
  const instructions = projection.applyDiff({
    op: 'setAnimatedMeshPlayback',
    handle: renderHandle(12),
    playback: {
      kind: 'play',
      clip: 'run',
      loop: 'repeat',
      speed: 1,
      weight: 1,
      restart: true,
      fadeSeconds: 0.1,
    },
  });

  assert.equal(instructions[0]?.op, 'upsertNode');
  const node = projection.node(renderHandle(12));
  assert.equal(node?.kind, 'animatedMesh');
  if (node?.kind === 'animatedMesh') {
    assert.equal(node.playback?.kind, 'play');
    if (node.playback?.kind === 'play') {
      assert.equal(node.playback.clip, 'run');
    }
  }

  const sampled = projection.applyDiff({
    op: 'setAnimatedMeshPlayback',
    handle: renderHandle(12),
    playback: { kind: 'sample', clip: 'idle', normalizedTime: 0.5 },
  });
  assert.equal(sampled[0]?.op, 'upsertNode');
  const sampledNode = projection.node(renderHandle(12));
  assert.equal(sampledNode?.kind, 'animatedMesh');
  if (sampledNode?.kind === 'animatedMesh') {
    assert.deepEqual(sampledNode.playback, { kind: 'sample', clip: 'idle', normalizedTime: 0.5 });
  }
  const beforeRejectedSample = projection.snapshot();
  assert.throws(
    () => projection.applyDiff({
      op: 'setAnimatedMeshPlayback',
      handle: renderHandle(12),
      playback: { kind: 'sample', clip: 'missing', normalizedTime: 0.5 },
    }),
    RenderProjectionError,
  );
  assert.deepEqual(projection.snapshot(), beforeRejectedSample);

  assert.throws(
    () =>
      projection.applyDiff({
        op: 'setAnimatedMeshPlayback',
        handle: renderHandle(12),
        playback: {
          kind: 'play',
          clip: 'dance',
          loop: 'repeat',
          speed: 1,
          weight: 1,
          restart: true,
          fadeSeconds: null,
        },
      }),
    RenderProjectionError,
  );

  assert.throws(
    () => projection.applyDiff({ op: 'defineAnimatedMesh', asset: animatedMeshAsset() }),
    RenderProjectionError,
  );
  projection.applyDiff({ op: 'destroy', handle: renderHandle(12) });
  assert.equal(projection.animatedMeshRefCount('mesh-animation/kenney-retro-character-medium'), 0);
});

void test('effective clip-pack clips admit both play and held sample playback', () => {
  const projection = new RenderProjection();
  projection.applyDiff({ op: 'defineAnimatedMesh', asset: animatedMeshAssetWithClipPackOnlyClip() });
  projection.applyDiff({
    op: 'createAnimatedMeshInstance',
    handle: renderHandle(13),
    parent: null,
    instance: {
      asset: 'mesh-animation/kenney-retro-character-medium',
      transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
      materialOverrides: [],
      playback: {
        kind: 'play',
        clip: 'gesture',
        loop: 'once',
        speed: 1,
        weight: 1,
        restart: true,
        fadeSeconds: null,
      },
      visible: true,
      metadata: { sourceEntity: 13, sourceSceneNode: null, tags: [], label: 'clip-pack-proof' },
    },
  });

  projection.applyDiff({
    op: 'setAnimatedMeshPlayback',
    handle: renderHandle(13),
    playback: { kind: 'sample', clip: 'gesture', normalizedTime: 0.5 },
  });
  const node = projection.node(renderHandle(13));
  assert.equal(node?.kind, 'animatedMesh');
  if (node?.kind === 'animatedMesh') {
    assert.deepEqual(node.playback, { kind: 'sample', clip: 'gesture', normalizedTime: 0.5 });
  }
});

void test('public retained projection re-adds one shared animated-mesh instance without redefining its live asset', () => {
  const projection = new RenderProjection();
  const instance = (handle: number) => ({
    op: 'createAnimatedMeshInstance' as const,
    handle: renderHandle(handle),
    parent: null,
    instance: {
      asset: 'mesh-animation/kenney-retro-character-medium',
      transform: { translation: [handle, 0, 0] as const, rotation: [0, 0, 0, 1] as const, scale: [1, 1, 1] as const },
      materialOverrides: [],
      playback: null,
      visible: true,
      metadata: { sourceEntity: handle, sourceSceneNode: null, tags: [], label: `shared-${handle}` },
    },
  });
  projection.applyFrame({ schemaVersion: 1, ops: [
    { op: 'defineAnimatedMesh', asset: animatedMeshAsset() },
    instance(21),
    instance(22),
  ] });
  assert.equal(projection.animatedMeshRefCount('mesh-animation/kenney-retro-character-medium'), 2);

  projection.applyDiff({ op: 'destroy', handle: renderHandle(21) });
  assert.equal(projection.animatedMeshRefCount('mesh-animation/kenney-retro-character-medium'), 1);
  assert.throws(
    () => projection.applyDiff({ op: 'defineAnimatedMesh', asset: animatedMeshAsset() }),
    /is in use by 1 instance/,
  );
  projection.applyDiff(instance(21));
  assert.equal(projection.animatedMeshRefCount('mesh-animation/kenney-retro-character-medium'), 2);
  assert.equal(projection.node(renderHandle(21))?.kind, 'animatedMesh');
  assert.equal(projection.node(renderHandle(22))?.kind, 'animatedMesh');
});

void test('resolves sprite atlas frames and sprite pick hints without renderer types', () => {
  const projection = new RenderProjection();
  projection.applyFrame({
    schemaVersion: 1,
    ops: [
      {
        op: 'defineSpriteAtlas',
        atlas: {
          id: 'sprite/ui',
          texture: 'texture/ui',
          frames: [{ frame: 3, uvMin: [0.25, 0.5], uvMax: [0.5, 0.75], size: [2, 3] }],
        },
      },
      { op: 'createSprite', handle: renderHandle(2), parent: null, sprite: sprite('sprite/ui', 0) },
      {
        op: 'updateSprite',
        handle: renderHandle(2),
        frame: 3,
        tint: [1, 0, 0, 0.5],
        renderOrder: 8,
        visible: false,
      },
    ],
  });

  const node = projection.node(renderHandle(2));
  assert.equal(node?.kind, 'sprite');
  if (node?.kind === 'sprite') {
    assert.deepEqual(node.frameUv, [0.25, 0.5, 0.5, 0.75]);
    assert.deepEqual(node.frameSize, [2, 3]);
    assert.deepEqual(node.sprite.tint, [1, 0, 0, 0.5]);
    assert.equal(node.renderOrder, 8);
    assert.equal(node.visible, false);
  }
  assert.deepEqual(projection.pickSprite(renderHandle(2)), {
    handle: renderHandle(2),
    sourceEntity: 7,
    sourceSceneNode: null,
    asset: 'sprite/ui',
    attachmentPoint: 'head',
  });
});

void test('fails closed on unknown handles, unsupported ops, and malformed mesh payloads', () => {
  const projection = new RenderProjection();
  assert.throws(
    () =>
      projection.applyDiff({
        op: 'update',
        handle: renderHandle(99),
        transform: null,
        material: null,
        visible: null,
        metadata: null,
      }),
    RenderProjectionError,
  );
  assert.throws(
    () => projection.applyDiff({ op: 'teleport', handle: renderHandle(1) } as unknown as RenderDiff),
    RenderProjectionError,
  );

  projection.applyDiff(createPrimitive(1));
  const validPayload = quadPayload();
  const malformedPayload: MeshPayloadDescriptor = {
    ...validPayload,
    source: {
      kind: 'inline',
      positions: [0, 0, 0],
      normals: [0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1],
      indices: [0, 1, 2, 0, 2, 3],
    },
  };
  assert.throws(
    () =>
      projection.applyDiff({
        op: 'replaceMeshPayload',
        handle: renderHandle(1),
        payload: malformedPayload,
      }),
    RenderProjectionError,
  );
  const digest = '1'.repeat(64);
  const malformedResource: MeshPayloadDescriptor = {
    ...validPayload,
    source: {
      kind: 'resource',
      resource: `mesh-resource/${'2'.repeat(64)}`,
      contentHash: `sha256:${digest}`,
      byteLength: 136,
      encoding: 'packedStreamsLeV1',
      positionsByteOffset: 16,
      normalsByteOffset: 64,
      indicesByteOffset: 112,
    },
  };
  assert.throws(
    () => projection.applyDiff({
      op: 'replaceMeshPayload',
      handle: renderHandle(1),
      payload: malformedResource,
    }),
    /content-addressed identity/u,
  );
});
