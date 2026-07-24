import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  renderHandle,
  type AnimatedMeshAsset,
  type MeshPayloadDescriptor,
  type RenderDiff,
  type RenderNode,
  type SpriteInstanceDescriptor,
  type StaticMeshAsset,
} from '@rusty-engine/render-contracts';
import {
  RenderProjection,
  RenderProjectionError,
} from './index.js';

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

function animatedMeshAsset(asset = 'mesh-animation/kenney-retro-character-medium'): AnimatedMeshAsset {
  return {
    asset,
    runtimeFormat: 'glb',
    contentHash: 'sha256-fixture-pending',
    clips: [
      { id: 'idle', name: 'Idle', durationSeconds: 1.2 },
      { id: 'run', name: 'Run', durationSeconds: 0.8 },
      { id: 'jump', name: 'Jump', durationSeconds: 0.6 },
    ],
    defaultClip: 'idle',
    materialSlots: [{ slot: 0, material: 'material/kenney-human-male-a' }],
    bounds: { min: [-0.5, 0, -0.5], max: [0.5, 1.8, 0.5] },
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
  assert.doesNotThrow(() => projection.applyDiff({ op: 'defineStaticMesh', asset: meshAsset() }));
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
          frames: [{ frame: 3, uvMin: [0.25, 0.5], uvMax: [0.5, 0.75] }],
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
});
