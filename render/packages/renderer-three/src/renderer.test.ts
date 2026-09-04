// Runtime tests for the Three.js renderer shell, run with `node --test`.
// The scene graph is built without a GL context (no rendering), so these assert
// registry/scene-graph state directly.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { bytesToHex } from '@noble/hashes/utils.js';
import { sha256 } from '@noble/hashes/sha2.js';
import { zlibSync } from 'fflate';

import { renderHandle, type AnimatedMeshAsset, type RenderDiff, type RenderNode } from '@rusty-engine/render-contracts';
import {
  MapAnimatedMeshAssetSource,
  AnimatedMeshRegistry,
  animationRigFingerprint,
  RenderApplyError,
  RendererTerminalError,
  RenderResourceError,
  RUSTY_RENDERER_TEXTURE_MAX_DECODED_BYTES,
  RUSTY_RENDERER_TEXTURE_MAX_ENCODED_BYTES,
  RUSTY_RENDERER_TEXTURE_MAX_RETAINED,
  ThreeRenderer,
  admitRendererTextureResourceBudget,
  loadAnimatedMeshGlbResource,
  type MeshBufferView,
  type MeshBufferSource,
  type MeshResourceSource,
  type TextureResourceSource,
} from './backend.js';

function cubeNode(label = 'cube'): RenderNode {
  return {
    geometry: { kind: 'cube' },
    material: { color: [1, 1, 1, 1], wireframe: false },
    transform: { translation: [2, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
    visible: true,
    layer: 'scene',
    metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label },
  };
}

function createDiff(handle: number, node: RenderNode): RenderDiff {
  return { op: 'create', handle: renderHandle(handle), parent: null, node };
}

void test('create places a node in the scene layer with its transform', () => {
  const r = new ThreeRenderer();
  r.applyDiff(createDiff(1, cubeNode()));

  assert.equal(r.handleCount, 1);
  assert.ok(r.has(renderHandle(1)));
  const obj = r.objectFor(renderHandle(1))!;
  assert.equal(obj.position.x, 2);
  assert.equal(obj.parent?.name, 'scene');
  assert.equal(obj.name, 'cube');
});

void test('update mutates transform and visibility', () => {
  const r = new ThreeRenderer();
  r.applyDiff(createDiff(1, cubeNode()));
  r.applyDiff({
    op: 'update',
    handle: renderHandle(1),
    transform: { translation: [5, 1, 0], rotation: [0, 0, 0, 1], scale: [2, 2, 2] },
    material: null,
    visible: false,
    metadata: null,
  });

  const obj = r.objectFor(renderHandle(1))!;
  assert.equal(obj.position.x, 5);
  assert.equal(obj.scale.x, 2);
  assert.equal(obj.visible, false);
});

void test('recovered publication frontiers continue through the Three renderer', () => {
  const renderer = new ThreeRenderer({
    publicationFrontiers: [{ stream: 'voxel:active', revision: 4 }],
  });
  const handle = renderHandle(77);
  renderer.applyFrame({
    schemaVersion: 1,
    ops: [createDiff(handle, cubeNode('recovered-voxel'))],
  });

  renderer.applyFrame({
    schemaVersion: 1,
    publication: { stream: 'voxel:active', baseRevision: 4, revision: 5, operationCount: 1 },
    ops: [{ op: 'replaceMeshPayload', handle, payload: quadPayload() }],
  });

  assert.equal(renderer.objectFor(handle)?.name, 'recovered-voxel');
});

void test('destroy removes the node and frees the handle', () => {
  const r = new ThreeRenderer();
  r.applyDiff(createDiff(1, cubeNode()));
  r.applyDiff({ op: 'destroy', handle: renderHandle(1) });

  assert.equal(r.handleCount, 0);
  assert.ok(!r.has(renderHandle(1)));
});

void test('retained resource statistics stay exact across create update destroy and disposal', () => {
  const renderer = new ThreeRenderer();
  assert.deepEqual(renderer.resourceStatistics(), {
    renderHandleCount: 0,
    geometryResourceCount: 0,
    materialResourceCount: 0,
    textureResourceCount: 0,
    animatedInstanceCount: 0,
  });

  renderer.applyDiff(createDiff(1, cubeNode()));
  const created = renderer.resourceStatistics();
  assert.deepEqual(created, {
    renderHandleCount: 1,
    geometryResourceCount: 1,
    materialResourceCount: 1,
    textureResourceCount: 0,
    animatedInstanceCount: 0,
  });
  assert.equal(Object.isFrozen(created), true);

  renderer.applyDiff({
    op: 'update',
    handle: renderHandle(1),
    transform: null,
    material: { color: [0.2, 0.4, 0.6, 1], wireframe: true },
    visible: null,
    metadata: null,
  });
  assert.deepEqual(renderer.resourceStatistics(), created, 'replacement owns one new material');

  renderer.applyDiff({ op: 'destroy', handle: renderHandle(1) });
  assert.deepEqual(renderer.resourceStatistics(), {
    renderHandleCount: 0,
    geometryResourceCount: 0,
    materialResourceCount: 0,
    textureResourceCount: 0,
    animatedInstanceCount: 0,
  });
  renderer.dispose();
  assert.deepEqual(renderer.resourceStatistics(), {
    renderHandleCount: 0,
    geometryResourceCount: 0,
    materialResourceCount: 0,
    textureResourceCount: 0,
    animatedInstanceCount: 0,
  });
  assert.throws(() => renderer.applyDiff(createDiff(2, cubeNode())), /renderer is disposed/u);
});

void test('renderer-neutral lights retain parent, update, disable, degrade shadows, and destroy', () => {
  const renderer = new ThreeRenderer();
  renderer.applyDiff(createDiff(1, cubeNode('light-parent')));
  renderer.applyDiff({
    op: 'createLight',
    handle: renderHandle(2),
    parent: renderHandle(1),
    light: {
      kind: 'directional',
      color: [1, 0.8, 0.6],
      intensity: 2,
      enabled: true,
      direction: [-1, -2, -1],
      shadowIntent: 'requested',
    },
  });

  const directional = renderer.objectFor(renderHandle(2));
  assert.ok(directional instanceof THREE.DirectionalLight);
  assert.equal(directional.parent, renderer.objectFor(renderHandle(1)));
  assert.equal(directional.visible, true);
  assert.deepEqual(renderer.lightReadout(), [{
    descriptor: {
      kind: 'directional',
      color: [1, 0.8, 0.6],
      intensity: 2,
      enabled: true,
      direction: [-1, -2, -1],
      shadowIntent: 'requested',
    },
    handle: renderHandle(2),
    parent: renderHandle(1),
    shadowStatus: 'requested_unsupported',
  }]);

  renderer.applyDiff({
    op: 'updateLight',
    handle: renderHandle(2),
    light: {
      kind: 'directional',
      color: [0.2, 0.4, 1],
      intensity: 0.5,
      enabled: false,
      direction: [0, -1, 0],
      shadowIntent: 'disabled',
    },
  });
  assert.equal(directional.visible, false);
  assert.equal(directional.intensity, 0.5);
  assert.equal(renderer.lightReadout()[0]?.shadowStatus, 'disabled');
  renderer.applyDiff({ op: 'destroy', handle: renderHandle(1) });
  assert.equal(renderer.lightReadout().length, 0);
  assert.equal(renderer.handleCount, 0);
});

void test('point and spot adapters preserve range, decay, cone, and direction', () => {
  const renderer = new ThreeRenderer({ shadowsEnabled: true });
  renderer.applyFrame({ schemaVersion: 1, ops: [
    {
      op: 'createLight', handle: renderHandle(20), parent: null,
      light: {
        kind: 'point', color: [1, 0.2, 0.1], intensity: 5, enabled: true,
        position: [2, 3, 4], range: 9, decay: 2, shadowIntent: 'requested',
      },
    },
    {
      op: 'createLight', handle: renderHandle(21), parent: null,
      light: {
        kind: 'spot', color: [0.1, 0.2, 1], intensity: 7, enabled: true,
        position: [0, 8, 0], direction: [0, -2, 0], range: 15, decay: 1,
        outerAngleRadians: 0.6, penumbra: 0.35, shadowIntent: 'requested',
      },
    },
  ] });
  const point = renderer.objectFor(renderHandle(20));
  const spot = renderer.objectFor(renderHandle(21));
  assert.ok(point instanceof THREE.PointLight);
  assert.equal(point.distance, 9);
  assert.equal(point.decay, 2);
  assert.equal(point.castShadow, true);
  assert.ok(spot instanceof THREE.SpotLight);
  assert.equal(spot.distance, 15);
  assert.equal(spot.angle, 0.6);
  assert.equal(spot.penumbra, 0.35);
  assert.equal(spot.target.position.y, -1);
  assert.deepEqual(renderer.lightReadout().map((light) => light.shadowStatus), ['active', 'active']);
});

void test('shadow admission is bounded and rejected frames are atomic', () => {
  const renderer = new ThreeRenderer({ shadowsEnabled: true, maximumActiveShadowLights: 1 });
  const requested = (handle: number): RenderDiff => ({
    op: 'createLight',
    handle: renderHandle(handle),
    parent: null,
    light: {
      kind: 'point', color: [1, 0.5, 0.25], intensity: 4, enabled: true,
      position: [handle, 2, 0], range: 8, decay: 2, shadowIntent: 'requested',
    },
  });
  assert.throws(
    () => renderer.applyFrame({ schemaVersion: 1, ops: [requested(40), requested(41)] }),
    (error: unknown) => error instanceof Error
      && error.name === 'RendererLightingPolicyError'
      && 'code' in error
      && error.code === 'shadow_budget_exceeded',
  );
  assert.equal(renderer.lightReadout().length, 0);
  renderer.applyFrame({ schemaVersion: 1, ops: [requested(40)] });
  assert.deepEqual(renderer.lightReadout().map((light) => light.shadowStatus), ['active']);

  const disabledRequest = requested(41) as Extract<RenderDiff, { op: 'createLight' }>;
  renderer.applyFrame({ schemaVersion: 1, ops: [{
    ...disabledRequest,
    light: { ...disabledRequest.light, enabled: false },
  }] });
  assert.deepEqual(
    renderer.lightReadout().map((light) => light.shadowStatus),
    ['active', 'disabled'],
  );

  const unsupported = new ThreeRenderer({ shadowsEnabled: false, maximumActiveShadowLights: 0 });
  unsupported.applyFrame({ schemaVersion: 1, ops: [requested(50), requested(51)] });
  assert.deepEqual(
    unsupported.lightReadout().map((light) => light.shadowStatus),
    ['requested_unsupported', 'requested_unsupported'],
  );
});

void test('lighting configuration and descriptor intensity have hard bounds', () => {
  assert.throws(
    () => new ThreeRenderer({ shadowsEnabled: true, maximumActiveShadowLights: 9 }),
    /maximumActiveShadowLights/u,
  );
  const renderer = new ThreeRenderer();
  assert.throws(() => renderer.applyFrame({ schemaVersion: 1, ops: [{
    op: 'createLight', handle: renderHandle(60), parent: null,
    light: {
      kind: 'ambient', color: [1, 1, 1], intensity: 10_001, enabled: true,
      shadowIntent: 'disabled',
    },
  }] }), /intensity/u);
  assert.equal(renderer.lightReadout().length, 0);
});

void test('malformed and kind-changing lights fail closed', () => {
  const renderer = new ThreeRenderer();
  assert.throws(() => renderer.applyDiff({
    op: 'createLight', handle: renderHandle(30), parent: null,
    light: {
      kind: 'spot', color: [1, 1, 1], intensity: 1, enabled: true,
      position: [0, 0, 0], direction: [0, 0, 0], range: null, decay: 2,
      outerAngleRadians: 0.5, penumbra: 0, shadowIntent: 'disabled',
    },
  }), /direction must be non-zero/);
  renderer.applyDiff({
    op: 'createLight', handle: renderHandle(30), parent: null,
    light: {
      kind: 'ambient', color: [1, 1, 1], intensity: 1, enabled: true,
      shadowIntent: 'disabled',
    },
  });
  assert.throws(() => renderer.applyDiff({
    op: 'updateLight', handle: renderHandle(30),
    light: {
      kind: 'point', color: [1, 1, 1], intensity: 1, enabled: true,
      position: [0, 0, 0], range: null, decay: 2, shadowIntent: 'disabled',
    },
  }), /cannot change kind/);
});

void test('renderer disposal releases nested retained resources and all handles', () => {
  const renderer = new ThreeRenderer();
  renderer.applyDiff(createDiff(1, cubeNode('parent')));
  renderer.applyDiff({
    op: 'create',
    handle: renderHandle(2),
    parent: renderHandle(1),
    node: cubeNode('child'),
  });
  const parentGeometry = (renderer.objectFor(renderHandle(1)) as import('three').Mesh).geometry;
  const childGeometry = (renderer.objectFor(renderHandle(2)) as import('three').Mesh).geometry;
  let parentDisposed = false;
  let childDisposed = false;
  parentGeometry.addEventListener('dispose', () => { parentDisposed = true; });
  childGeometry.addEventListener('dispose', () => { childDisposed = true; });

  renderer.dispose();
  renderer.dispose();

  assert.equal(renderer.handleCount, 0);
  assert.equal(parentDisposed, true);
  assert.equal(childDisposed, true);
  assert.equal(renderer.scene.children.length, 0);
});

void test('duplicate create and stale/unknown handles throw', () => {
  const r = new ThreeRenderer();
  r.applyDiff(createDiff(1, cubeNode()));

  assert.throws(() => r.applyDiff(createDiff(1, cubeNode())), RenderApplyError);
  assert.throws(
    () =>
      r.applyDiff({
        op: 'update',
        handle: renderHandle(99),
        transform: null,
        material: null,
        visible: null,
        metadata: null,
      }),
    RenderApplyError,
  );
  assert.throws(
    () => r.applyDiff({ op: 'destroy', handle: renderHandle(42) }),
    RenderApplyError,
  );
});

void test('a rejected later frame operation leaves handles and retained resources unchanged', () => {
  const renderer = new ThreeRenderer();
  renderer.applyFrame({ schemaVersion: 1, ops: [
    {
      op: 'defineTexture',
      texture: {
        id: 'texture/stable', width: 1, height: 1, filter: 'nearest', wrap: 'clamp',
        contentHash: 'stable', version: 1,
      },
    },
    createDiff(1, cubeNode('stable')),
  ] });
  const snapshotBefore = renderer.snapshot();
  const textureBefore = renderer.textureDescriptor('texture/stable');

  assert.throws(() => renderer.applyFrame({ schemaVersion: 1, ops: [
    {
      op: 'defineTexture',
      texture: {
        id: 'texture/stable', width: 2, height: 2, filter: 'linear', wrap: 'repeat',
        contentHash: 'candidate', version: 2,
      },
    },
    createDiff(2, cubeNode('must-not-commit')),
    {
      op: 'update', handle: renderHandle(999), transform: null, material: null,
      visible: false, metadata: null,
    },
  ] }), /unknown handle 999/);

  assert.equal(renderer.snapshot(), snapshotBefore);
  assert.deepEqual(renderer.textureDescriptor('texture/stable'), textureBefore);
  assert.equal(renderer.handleCount, 1);
  assert.equal(renderer.has(renderHandle(1)), true);
  assert.equal(renderer.has(renderHandle(2)), false);
});

void test('a rejected later backend resource leaves earlier frame operations unapplied', () => {
  const renderer = new ThreeRenderer();
  renderer.applyDiff(createDiff(1, cubeNode('stable')));
  const snapshotBefore = renderer.snapshot();

  assert.throws(() => renderer.applyFrame({ schemaVersion: 1, ops: [
    {
      op: 'defineTexture',
      texture: {
        id: 'texture/candidate', width: 1, height: 1, filter: 'nearest', wrap: 'clamp',
        contentHash: null, version: 1,
      },
    },
    createDiff(2, cubeNode('must-not-commit')),
    { op: 'defineAnimatedMesh', asset: animatedMeshAsset() },
  ] }), /missing animated mesh resource/);

  assert.equal(renderer.snapshot(), snapshotBefore);
  assert.equal(renderer.textureDescriptor('texture/candidate'), undefined);
  assert.equal(renderer.handleCount, 1);
  assert.equal(renderer.has(renderHandle(2)), false);
});

void test('debug-layer nodes land in the debug group', () => {
  const r = new ThreeRenderer();
  const node: RenderNode = {
    ...cubeNode('#1'),
    geometry: { kind: 'point' },
    layer: 'debug',
  };
  r.applyDiff(createDiff(1, node));
  assert.equal(r.objectFor(renderHandle(1))?.parent?.name, 'debug');
});

void test('nested group snapshots retain their root scene layer', () => {
  const r = new ThreeRenderer();
  r.applyDiff(createDiff(1, {
    ...cubeNode('ui-root'),
    geometry: { kind: 'group' },
    layer: 'ui',
    visible: false,
  }));
  r.applyDiff({
    op: 'create',
    handle: renderHandle(2),
    parent: renderHandle(1),
    node: cubeNode('nested'),
  });

  assert.equal(r.objectFor(renderHandle(1))?.visible, false);
  assert.match(r.snapshot(), /handle 1  layer ui  shape group/);
  assert.match(r.snapshot(), /handle 2  layer ui  shape cube/);
});

void test('visibilityReadout classifies retained handles by effective visibility and frustum', () => {
  const renderer = new ThreeRenderer();
  const positionedCube = (
    translation: readonly [number, number, number],
    visible = true,
  ): RenderNode => ({
    ...cubeNode(),
    transform: { translation, rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
    visible,
  });
  renderer.applyFrame({
    schemaVersion: 1,
    ops: [
      createDiff(1, positionedCube([0, 0, -5])),
      createDiff(2, positionedCube([100, 0, -5])),
      createDiff(3, positionedCube([0, 0, -5], false)),
      {
        op: 'create',
        handle: renderHandle(4),
        parent: null,
        node: { ...positionedCube([0, 0, -5], false), geometry: { kind: 'group' } },
      },
      {
        op: 'create',
        handle: renderHandle(5),
        parent: renderHandle(4),
        node: positionedCube([0, 0, 0]),
      },
    ],
  });
  const camera = new THREE.PerspectiveCamera(90, 1, 0.1, 20);
  camera.lookAt(0, 0, -1);
  camera.updateProjectionMatrix();

  const readout = renderer.visibilityReadout(camera);
  const repeated = renderer.visibilityReadout(camera);
  assert.deepEqual(readout, repeated, 'visibility readout is deterministic');
  assert.equal(readout.schemaVersion, 1);
  assert.equal(readout.basis, 'cpuFrustum');
  assert.equal(readout.occlusion, 'notMeasured');
  assert.deepEqual(readout.handles.map(({ handle }) => handle), [1, 2, 3, 4, 5]);
  assert.deepEqual(
    readout.handles.map(({ handle, state, inFrustum, effectivelyVisible, occlusion }) => ({
      handle,
      state,
      inFrustum,
      effectivelyVisible,
      occlusion,
    })),
    [
      { handle: 1, state: 'frustumVisible', inFrustum: true, effectivelyVisible: true, occlusion: 'notMeasured' },
      { handle: 2, state: 'outsideFrustum', inFrustum: false, effectivelyVisible: true, occlusion: 'notMeasured' },
      { handle: 3, state: 'hidden', inFrustum: true, effectivelyVisible: false, occlusion: 'notMeasured' },
      { handle: 4, state: 'notDrawable', inFrustum: false, effectivelyVisible: false, occlusion: 'notMeasured' },
      { handle: 5, state: 'hidden', inFrustum: true, effectivelyVisible: false, occlusion: 'notMeasured' },
    ],
  );
  assert.equal(Object.isFrozen(readout), true);
  assert.equal(Object.isFrozen(readout.handles), true);
  assert.deepEqual(renderer.visibilityReadout(camera, renderer.viewmodelScene), {
    schemaVersion: 1,
    basis: 'cpuFrustum',
    occlusion: 'notMeasured',
    handles: [],
  });

  renderer.applyDiff({ op: 'destroy', handle: renderHandle(2) });
  assert.equal(
    renderer.visibilityReadout(camera).handles.some(({ handle }) => handle === renderHandle(2)),
    false,
    'destroyed handles disappear from a fresh readout',
  );

  renderer.dispose();
  assert.throws(() => renderer.visibilityReadout(camera), /renderer is disposed/u);
});

void test('camera-relative descendants live only in the dedicated viewmodel scene', () => {
  const renderer = new ThreeRenderer();
  renderer.applyDiff(createDiff(1, {
    ...cubeNode('viewmodel-root'),
    geometry: { kind: 'group' },
    layer: 'viewmodel',
  }));
  renderer.applyDiff({
    op: 'create',
    handle: renderHandle(2),
    parent: renderHandle(1),
    node: {
      ...cubeNode('viewmodel-child'),
      transform: {
        translation: [0.4, -0.35, -1.2],
        rotation: [0, 0, 0, 1],
        scale: [0.5, 0.5, 0.5],
      },
    },
  });

  const root = renderer.objectFor(renderHandle(1))!;
  const child = renderer.objectFor(renderHandle(2))!;
  assert.equal(root.parent?.name, 'viewmodel');
  assert.equal(root.parent?.parent, renderer.viewmodelScene);
  assert.equal(renderer.scene.getObjectByName('viewmodel-root'), undefined);
  assert.equal(renderer.projectionIdentityForObject(child)?.layer, 'viewmodel');
  assert.match(renderer.snapshot(), /handle 1  layer viewmodel  shape group/);
  assert.match(renderer.snapshot(), /handle 2  layer viewmodel  shape cube/);

  renderer.dispose();
  assert.equal(renderer.viewmodelScene.children.length, 0);
});

void test('applyEncodedFrame strictly decodes and sequences create→update→destroy', () => {
  const fixture: unknown = JSON.parse(
    readFileSync(
      resolve(import.meta.dirname, '../../../../fixtures/render/sample-frame-v1.json'),
      'utf8',
    ),
  );
  const r = new ThreeRenderer();
  r.applyEncodedFrame(fixture);
  // The fixture creates handle 1, updates it, then destroys it.
  assert.equal(r.handleCount, 0);
});

void test('applies the Rust-compatible render fixture sequence end-to-end', () => {
  // Versioned Rust-compatible fixture → strict TypeScript decode → renderer apply.
  // Frame 1 creates handles 1 & 2; frame 2 creates 3, updates 1, destroys 2.
  const frames = JSON.parse(
    readFileSync(
      resolve(import.meta.dirname, '../../../../fixtures/render/renderer-sequence-v1.json'),
      'utf8',
    ),
  ) as unknown[];

  const r = new ThreeRenderer();
  for (const frame of frames) {
    r.applyEncodedFrame(frame);
  }

  assert.equal(r.handleCount, 2);
  assert.ok(r.has(renderHandle(1)));
  assert.ok(r.has(renderHandle(3)));
  assert.ok(!r.has(renderHandle(2)));
  // The update carried the new tag onto handle 1's scene object metadata.
  assert.deepEqual(
    r.projectionIdentityForObject(r.objectFor(renderHandle(1))!)?.metadata.tags,
    ['updated'],
  );
});

// ── Mesh payload upload ───────────────────────────────────────────────────────

import * as THREE from 'three';
import type { MeshPayloadDescriptor } from '@rusty-engine/render-contracts';

void test('realizes every operation in the comprehensive retained fixture', () => {
  const fixture = JSON.parse(readFileSync(
    resolve(import.meta.dirname, '../../../../fixtures/render/retained-frame-v1.json'),
    'utf8',
  )) as unknown;
  const animatedScene = new THREE.Group();
  animatedScene.add(new THREE.Mesh(
    new THREE.BoxGeometry(1, 1, 1),
    new THREE.MeshStandardMaterial({ color: 0xffffff }),
  ));
  const renderer = new ThreeRenderer({
    animatedMeshSource: new MapAnimatedMeshAssetSource([{
      asset: 'mesh-animation/character',
      contentHash: 'f00d',
      scene: animatedScene,
      clips: [new THREE.AnimationClip('idle', 1, [])],
    }]),
  });

  renderer.applyEncodedFrame(fixture);

  assert.equal(renderer.handleCount, 4);
  assert.equal(renderer.has(renderHandle(5)), false);
  assert.equal(renderer.objectFor(renderHandle(1))?.visible, false);
  assert.equal(renderer.lightReadout()[0]?.descriptor.kind, 'directional');
  assert.equal(renderer.instanceCountFor('mesh/triangle'), 1);
  assert.equal(renderer.animatedMeshPlayback(renderHandle(4))?.currentClip, 'idle');
});

function meshNode(): RenderNode {
  return {
    geometry: { kind: 'cube' },
    material: { color: [1, 1, 1, 1], wireframe: false },
    transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
    visible: true,
    layer: 'scene',
    metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label: 'chunk' },
  };
}

// A quad (4 verts, 6 indices) split into two material-slot groups.
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
    groups: [
      { materialSlot: 1, start: 0, count: 3 },
      { materialSlot: 2, start: 3, count: 3 },
    ],
    bounds: { min: [0, 0, 0], max: [1, 1, 0] },
    source: {
      kind: 'inline',
      positions: [0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0],
      normals: [0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1],
      indices: [0, 1, 2, 0, 2, 3],
    },
    provenance: 'voxelChunk',
  };
}

function texturedQuadPayload(): MeshPayloadDescriptor {
  const payload = quadPayload();
  if (payload.source.kind !== 'inline') throw new Error('quad fixture must remain inline');
  return {
    ...payload,
    layout: {
      ...payload.layout,
      attributes: [...payload.layout.attributes, { name: 'uv', components: 2, kind: 'f32' }],
    },
    source: {
      kind: 'inline',
      positions: payload.source.positions,
      normals: payload.source.normals,
      uvs: [0, 0, 1, 0, 1, 1, 0, 1],
      indices: payload.source.indices,
    },
  };
}

void test('replaceMeshPayload uploads a BufferGeometry with groups and material slots', () => {
  const r = new ThreeRenderer();
  const h = renderHandle(1);
  r.applyDiff({ op: 'create', handle: h, parent: null, node: meshNode() });
  r.applyDiff({ op: 'replaceMeshPayload', handle: h, payload: quadPayload() });

  const mesh = r.objectFor(h) as THREE.Mesh;
  const geo = mesh.geometry;
  assert.equal(geo.getAttribute('position').count, 4);
  assert.equal(geo.getAttribute('normal').count, 4);
  assert.equal(geo.getIndex()!.count, 6);
  assert.equal(geo.groups.length, 2);
  assert.deepEqual(
    geo.groups.map((g) => [g.start, g.count, g.materialIndex]),
    [[0, 3, 0], [3, 3, 1]],
  );
  // Two materials, one per group.
  assert.ok(Array.isArray(mesh.material));
  assert.equal((mesh.material as THREE.Material[]).length, 2);
});

void test('pickMesh traces an uploaded mesh handle back to its authority provenance', () => {
  const r = new ThreeRenderer();
  const h = renderHandle(1);
  r.applyDiff({ op: 'create', handle: h, parent: null, node: meshNode() });
  // No uploaded mesh yet → no source trace (missing metadata fails closed).
  assert.equal(r.pickMesh(h), undefined);

  r.applyDiff({ op: 'replaceMeshPayload', handle: h, payload: quadPayload() });
  // Now it maps back to the authority source (the voxel chunk that produced it).
  assert.deepEqual(r.pickMesh(h), {
    handle: h,
    provenance: 'voxelChunk',
    sourceEntity: null,
    sourceSceneNode: null,
  });
});

void test('pickMesh fails closed on a stale/missing handle (no invented source)', () => {
  const r = new ThreeRenderer();
  const h = renderHandle(1);
  // Unknown handle → undefined.
  assert.equal(r.pickMesh(renderHandle(99)), undefined);
  // Destroyed handle → undefined (stale): the renderer never invents a source.
  r.applyDiff({ op: 'create', handle: h, parent: null, node: meshNode() });
  r.applyDiff({ op: 'replaceMeshPayload', handle: h, payload: quadPayload() });
  r.applyDiff({ op: 'destroy', handle: h });
  assert.equal(r.pickMesh(h), undefined);
});

void test('registered slot colour maps to the group material; unregistered uses a fallback', () => {
  const r = new ThreeRenderer();
  r.registerSlotColor(1, 1, 0, 0); // slot 1 → red
  const h = renderHandle(1);
  r.applyDiff({ op: 'create', handle: h, parent: null, node: meshNode() });
  r.applyDiff({ op: 'replaceMeshPayload', handle: h, payload: quadPayload() });

  const mats = (r.objectFor(h) as THREE.Mesh).material as THREE.MeshBasicMaterial[];
  assert.deepEqual([mats[0]!.color.r, mats[0]!.color.g, mats[0]!.color.b], [1, 0, 0]);
  // Slot 2 was never registered → a deterministic non-red fallback colour.
  assert.notDeepEqual([mats[1]!.color.r, mats[1]!.color.g, mats[1]!.color.b], [1, 0, 0]);
});

void test('voxel material descriptors style uploaded groups and redefine them live', () => {
  const renderer = new ThreeRenderer();
  const handle = renderHandle(1);
  renderer.applyDiff({ op: 'create', handle, parent: null, node: meshNode() });
  renderer.applyDiff({
    op: 'defineMaterial',
    material: { ...woodMaterial(), id: 'voxel-material/1' },
  });
  renderer.applyDiff({ op: 'replaceMeshPayload', handle, payload: quadPayload() });

  const mesh = renderer.objectFor(handle) as THREE.Mesh;
  const before = (mesh.material as THREE.MeshStandardMaterial[])[0]!;
  assert.ok(Math.abs(before.color.r - 0.6) < 1e-6);
  assert.ok(Math.abs(before.color.b - 0.2) < 1e-6);
  let disposed = false;
  before.addEventListener('dispose', () => {
    disposed = true;
  });

  renderer.applyDiff({
    op: 'defineMaterial',
    material: {
      ...woodMaterial(),
      id: 'voxel-material/1',
      color: [0.1, 0.8, 0.2, 1],
    },
  });

  const after = (mesh.material as THREE.MeshStandardMaterial[])[0]!;
  assert.ok(Math.abs(after.color.g - 0.8) < 1e-6, 'voxel material redefine reached the live mesh');
  assert.ok(disposed, 'the prior uploaded-mesh material was disposed');
});

void test('replaceMeshPayload disposes the previous geometry and material', () => {
  const r = new ThreeRenderer();
  const h = renderHandle(1);
  r.applyDiff({ op: 'create', handle: h, parent: null, node: meshNode() });
  const mesh = r.objectFor(h) as THREE.Mesh;
  const oldGeo = mesh.geometry;
  let disposed = false;
  oldGeo.addEventListener('dispose', () => { disposed = true; });

  r.applyDiff({ op: 'replaceMeshPayload', handle: h, payload: quadPayload() });
  assert.ok(disposed, 'old geometry should be disposed on replace');
  assert.notEqual(mesh.geometry, oldGeo);

  // A second replace disposes the first uploaded geometry too.
  const firstUpload = mesh.geometry;
  let secondDisposed = false;
  firstUpload.addEventListener('dispose', () => { secondDisposed = true; });
  r.applyDiff({ op: 'replaceMeshPayload', handle: h, payload: quadPayload() });
  assert.ok(secondDisposed);
});

void test('published chunk frame replaces only changed geometry and rejects stale replay', () => {
  const renderer = new ThreeRenderer();
  const changed = renderHandle(91);
  const unchanged = renderHandle(92);
  renderer.applyFrame({
    schemaVersion: 1,
    publication: { stream: 'voxel:terrain', baseRevision: 0, revision: 1, operationCount: 4 },
    ops: [
      createDiff(91, cubeNode('changed-chunk')),
      { op: 'replaceMeshPayload', handle: changed, payload: quadPayload() },
      createDiff(92, cubeNode('unchanged-chunk')),
      { op: 'replaceMeshPayload', handle: unchanged, payload: quadPayload() },
    ],
  });
  const changedMesh = renderer.objectFor(changed) as THREE.Mesh;
  const unchangedMesh = renderer.objectFor(unchanged) as THREE.Mesh;
  const changedBefore = changedMesh.geometry;
  const unchangedBefore = unchangedMesh.geometry;
  let changedDisposed = 0;
  changedBefore.addEventListener('dispose', () => { changedDisposed += 1; });

  renderer.applyFrame({
    schemaVersion: 1,
    publication: { stream: 'voxel:terrain', baseRevision: 1, revision: 2, operationCount: 1 },
    ops: [{ op: 'replaceMeshPayload', handle: changed, payload: quadPayload() }],
  });
  assert.notEqual(changedMesh.geometry, changedBefore);
  assert.equal(changedDisposed, 1);
  assert.equal(unchangedMesh.geometry, unchangedBefore);

  const changedAfter = changedMesh.geometry;
  assert.throws(() => renderer.applyFrame({
    schemaVersion: 1,
    publication: { stream: 'voxel:terrain', baseRevision: 0, revision: 1, operationCount: 1 },
    ops: [{ op: 'replaceMeshPayload', handle: changed, payload: quadPayload() }],
  }), /stale publication/u);
  assert.equal(changedMesh.geometry, changedAfter);
  assert.equal(unchangedMesh.geometry, unchangedBefore);
});

void test('uploaded voxel meshes stay lit and preserve wireframe/opacity across updates and remeshes', () => {
  const renderer = new ThreeRenderer();
  renderer.registerSlotColor(1, 1, 0.5, 0.25);
  const handle = renderHandle(8);
  const node: RenderNode = {
    ...meshNode(),
    material: { color: [0.5, 1, 0.8, 0.6], wireframe: true },
  };
  renderer.applyDiff({ op: 'create', handle, parent: null, node });
  renderer.applyDiff({ op: 'replaceMeshPayload', handle, payload: quadPayload() });

  const mesh = renderer.objectFor(handle) as THREE.Mesh;
  let materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
  assert.ok(materials.every((material) => material instanceof THREE.MeshStandardMaterial));
  assert.equal((materials[0] as THREE.MeshStandardMaterial).wireframe, true);
  assert.equal((materials[0] as THREE.MeshStandardMaterial).opacity, 0.6);
  assert.equal((materials[0] as THREE.MeshStandardMaterial).transparent, true);
  assert.equal((materials[0] as THREE.MeshStandardMaterial).color.r, 0.5);

  renderer.applyDiff({
    op: 'update', handle, transform: null,
    material: { color: [1, 1, 1, 1], wireframe: false },
    visible: null, metadata: null,
  });
  renderer.applyDiff({ op: 'replaceMeshPayload', handle, payload: quadPayload() });
  materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
  assert.ok(materials.every((material) => material instanceof THREE.MeshStandardMaterial));
  assert.ok(materials.every((material) => !(material as THREE.MeshStandardMaterial).wireframe));
  assert.deepEqual(renderer.meshPresentationReadout(), [{
    handle,
    lit: true,
    materialSlots: [1, 2],
    opacity: 1,
    wireframe: false,
  }]);
});

void test('replaceMeshPayload on an unknown handle throws', () => {
  const r = new ThreeRenderer();
  assert.throws(
    () => r.applyDiff({ op: 'replaceMeshPayload', handle: renderHandle(9), payload: quadPayload() }),
    RenderApplyError,
  );
});

// ── Shared-buffer mesh payloads ───────────────────────────────────────────────

/** Pack the quad's inline streams into one `[positions|normals|indices]` blob. */
function quadHandleBytes(): Uint8Array {
  const positions = [0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0];
  const normals = [0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1];
  const indices = [0, 1, 2, 0, 2, 3];
  const bytes = new Uint8Array((positions.length + normals.length + indices.length) * 4);
  const dv = new DataView(bytes.buffer);
  let offset = 0;
  for (const v of positions) {
    dv.setFloat32(offset, v, true);
    offset += 4;
  }
  for (const v of normals) {
    dv.setFloat32(offset, v, true);
    offset += 4;
  }
  for (const v of indices) {
    dv.setUint32(offset, v, true);
    offset += 4;
  }
  return bytes;
}

/** The quad payload addressed by a shared-buffer id instead of inline arrays. */
function quadHandlePayload(buffer: number): MeshPayloadDescriptor {
  return {
    ...quadPayload(),
    source: {
      kind: 'sharedBuffer',
      buffer,
      positionsByteOffset: 0,
      normalsByteOffset: 48,
      indicesByteOffset: 96,
    },
  };
}

/** A minimal in-memory mesh buffer source mirroring the runtime bridge contract,
 *  recording borrow/release calls so tests can assert the lifetime semantics. */
class MapBufferSource implements MeshBufferSource {
  readonly #buffers = new Map<number, Uint8Array>();
  #expired = new Set<number>();
  #failRelease = new Set<number>();
  /** Handles passed to getBuffer / releaseBuffer, in call order. */
  readonly borrowed: number[] = [];
  readonly released: number[] = [];

  set(handle: number, bytes: Uint8Array): void {
    this.#buffers.set(handle, bytes);
  }

  expire(handle: number): void {
    this.#expired.add(handle);
  }

  failReleaseOf(handle: number): void {
    this.#failRelease.add(handle);
  }

  /** Borrows minus releases — must return to zero after every upload. */
  get outstanding(): number {
    return this.borrowed.length - this.released.length;
  }

  acquireBuffer(raw: number): MeshBufferView {
    if (this.#expired.has(raw)) {
      throw new RenderResourceError('expired', raw, `buffer ${raw} expired`);
    }
    const bytes = this.#buffers.get(raw);
    if (bytes === undefined) {
      throw new RenderResourceError('missing', raw, `no buffer for handle ${raw}`);
    }
    this.borrowed.push(raw);
    return { bytes };
  }

  releaseBuffer(raw: number): void {
    this.released.push(raw);
    if (this.#failRelease.has(raw)) {
      throw new RenderResourceError('missing', raw, `release: no buffer for handle ${raw}`);
    }
  }
}

void test('inline and shared-buffer sources produce equivalent geometry', () => {
  const inlineRenderer = new ThreeRenderer();
  const hi = renderHandle(1);
  inlineRenderer.applyDiff({ op: 'create', handle: hi, parent: null, node: meshNode() });
  inlineRenderer.applyDiff({ op: 'replaceMeshPayload', handle: hi, payload: quadPayload() });
  const inlineGeo = (inlineRenderer.objectFor(hi) as THREE.Mesh).geometry;

  const source = new MapBufferSource();
  source.set(7, quadHandleBytes());
  const handleRenderer = new ThreeRenderer({ meshBufferSource: source });
  const hh = renderHandle(1);
  handleRenderer.applyDiff({ op: 'create', handle: hh, parent: null, node: meshNode() });
  handleRenderer.applyDiff({ op: 'replaceMeshPayload', handle: hh, payload: quadHandlePayload(7) });
  const handleGeo = (handleRenderer.objectFor(hh) as THREE.Mesh).geometry;

  assert.deepEqual(
    Array.from(handleGeo.getAttribute('position').array),
    Array.from(inlineGeo.getAttribute('position').array),
  );
  assert.deepEqual(
    Array.from(handleGeo.getAttribute('normal').array),
    Array.from(inlineGeo.getAttribute('normal').array),
  );
  assert.deepEqual(Array.from(handleGeo.getIndex()!.array), Array.from(inlineGeo.getIndex()!.array));
  assert.deepEqual(
    handleGeo.groups.map((g) => [g.start, g.count, g.materialIndex]),
    inlineGeo.groups.map((g) => [g.start, g.count, g.materialIndex]),
  );
});

void test('shared-buffer source with no provider fails closed', () => {
  const r = new ThreeRenderer();
  const h = renderHandle(1);
  r.applyDiff({ op: 'create', handle: h, parent: null, node: meshNode() });
  assert.throws(
    () => r.applyDiff({ op: 'replaceMeshPayload', handle: h, payload: quadHandlePayload(7) }),
    RenderApplyError,
  );
});

void test('unknown and stale shared-buffer ids produce a classified error, not an empty mesh', () => {
  const source = new MapBufferSource();
  const r = new ThreeRenderer({ meshBufferSource: source });
  const h = renderHandle(1);
  r.applyDiff({ op: 'create', handle: h, parent: null, node: meshNode() });

  // Unknown handle (provider has no buffer 7).
  assert.throws(
    () => r.applyDiff({ op: 'replaceMeshPayload', handle: h, payload: quadHandlePayload(7) }),
    /unavailable \[missing\]/,
  );

  // Stale handle (provider reports the buffer expired).
  source.set(7, quadHandleBytes());
  source.expire(7);
  assert.throws(
    () => r.applyDiff({ op: 'replaceMeshPayload', handle: h, payload: quadHandlePayload(7) }),
    /unavailable \[expired\]/,
  );
});

void test('a buffer too small for the declared layout fails closed', () => {
  const source = new MapBufferSource();
  source.set(7, quadHandleBytes().slice(0, 64)); // truncated: not enough for normals+indices
  const r = new ThreeRenderer({ meshBufferSource: source });
  const h = renderHandle(1);
  r.applyDiff({ op: 'create', handle: h, parent: null, node: meshNode() });
  assert.throws(
    () => r.applyDiff({ op: 'replaceMeshPayload', handle: h, payload: quadHandlePayload(7) }),
    /exceeds buffer/,
  );
});

void test('replaceMeshPayload releases the borrow on success (borrow → copy → release)', () => {
  const source = new MapBufferSource();
  source.set(7, quadHandleBytes());
  const r = new ThreeRenderer({ meshBufferSource: source });
  const h = renderHandle(1);
  r.applyDiff({ op: 'create', handle: h, parent: null, node: meshNode() });
  r.applyDiff({ op: 'replaceMeshPayload', handle: h, payload: quadHandlePayload(7) });
  assert.deepEqual(source.borrowed, [7]);
  assert.deepEqual(source.released, [7]);
  assert.equal(source.outstanding, 0, 'no borrow is retained past the upload');
});

// ── Content-addressed mesh resources ────────────────────────────────────────

const RESOURCE_DIGEST = '1'.repeat(64);
const RESOURCE_ID = `mesh-resource/${RESOURCE_DIGEST}`;
const RESOURCE_HASH = `sha256:${RESOURCE_DIGEST}`;

function quadResourceBytes(): Uint8Array {
  const streams = quadHandleBytes();
  const bytes = new Uint8Array(16 + streams.byteLength);
  bytes.set([0x52, 0x4d, 0x53, 0x48, 0x4c, 0x45, 0x30, 0x31]);
  const header = new DataView(bytes.buffer);
  header.setUint32(8, bytes.byteLength, true);
  header.setUint32(12, 1, true);
  bytes.set(streams, 16);
  return bytes;
}

function quadResourcePayload(): MeshPayloadDescriptor {
  return {
    ...quadPayload(),
    source: {
      kind: 'resource',
      resource: RESOURCE_ID,
      contentHash: RESOURCE_HASH,
      byteLength: 136,
      encoding: 'packedStreamsLeV1',
      positionsByteOffset: 16,
      normalsByteOffset: 64,
      indicesByteOffset: 112,
    },
  };
}

function texturedQuadResourceBytes(): Uint8Array {
  const inline = texturedQuadPayload().source;
  assert.equal(inline.kind, 'inline');
  const positions = inline.positions;
  const normals = inline.normals;
  const uvs = inline.uvs!;
  const indices = inline.indices;
  const bytes = new Uint8Array(16 + (positions.length + normals.length + uvs.length + indices.length) * 4);
  bytes.set([0x52, 0x4d, 0x53, 0x48, 0x4c, 0x45, 0x30, 0x32]);
  const view = new DataView(bytes.buffer);
  view.setUint32(8, bytes.byteLength, true);
  view.setUint32(12, 1, true);
  let offset = 16;
  for (const value of [...positions, ...normals, ...uvs]) {
    view.setFloat32(offset, value, true);
    offset += 4;
  }
  for (const value of indices) {
    view.setUint32(offset, value, true);
    offset += 4;
  }
  return bytes;
}

function texturedQuadResourcePayload(): MeshPayloadDescriptor {
  const payload = texturedQuadPayload();
  return {
    ...payload,
    source: {
      kind: 'resource',
      resource: RESOURCE_ID,
      contentHash: RESOURCE_HASH,
      byteLength: 168,
      encoding: 'packedStreamsLeV2',
      positionsByteOffset: 16,
      normalsByteOffset: 64,
      uvsByteOffset: 112,
      indicesByteOffset: 144,
    },
  };
}

class MapResourceSource implements MeshResourceSource {
  readonly resources = new Map<string, Uint8Array>();
  readonly acquired: string[] = [];
  readonly released: string[] = [];

  acquireResource(resource: string, contentHash: string, byteLength: number): MeshBufferView {
    const bytes = this.resources.get(resource);
    if (bytes === undefined) throw new RenderResourceError('missing', resource, 'missing resource');
    if (contentHash !== RESOURCE_HASH || byteLength !== bytes.byteLength) {
      throw new RenderResourceError('invalid', resource, 'descriptor mismatch');
    }
    this.acquired.push(resource);
    return { bytes };
  }

  releaseResource(resource: string): void {
    this.released.push(resource);
  }
}

void test('resource mesh payloads produce equivalent geometry and release their borrow', () => {
  const source = new MapResourceSource();
  source.resources.set(RESOURCE_ID, quadResourceBytes());
  const renderer = new ThreeRenderer({ meshResourceSource: source });
  const handle = renderHandle(1);
  renderer.applyDiff({ op: 'create', handle, parent: null, node: meshNode() });
  renderer.applyDiff({ op: 'replaceMeshPayload', handle, payload: quadResourcePayload() });

  const geometry = (renderer.objectFor(handle) as THREE.Mesh).geometry;
  assert.deepEqual(Array.from(geometry.getIndex()!.array), [0, 1, 2, 0, 2, 3]);
  assert.deepEqual(source.acquired, [RESOURCE_ID]);
  assert.deepEqual(source.released, [RESOURCE_ID]);
});

void test('inline and packed-v2 voxel meshes converge on one tile-coordinate attribute', () => {
  const inlineRenderer = new ThreeRenderer();
  const handle = renderHandle(1);
  inlineRenderer.applyDiff({ op: 'create', handle, parent: null, node: meshNode() });
  inlineRenderer.applyDiff({ op: 'replaceMeshPayload', handle, payload: texturedQuadPayload() });

  const source = new MapResourceSource();
  source.resources.set(RESOURCE_ID, texturedQuadResourceBytes());
  const packedRenderer = new ThreeRenderer({ meshResourceSource: source });
  packedRenderer.applyDiff({ op: 'create', handle, parent: null, node: meshNode() });
  packedRenderer.applyDiff({
    op: 'replaceMeshPayload',
    handle,
    payload: texturedQuadResourcePayload(),
  });

  const inlineUvs = (inlineRenderer.objectFor(handle) as THREE.Mesh).geometry.getAttribute('uv');
  const packedUvs = (packedRenderer.objectFor(handle) as THREE.Mesh).geometry.getAttribute('uv');
  assert.deepEqual(Array.from(inlineUvs.array), [0, 0, 1, 0, 1, 1, 0, 1]);
  assert.deepEqual(Array.from(packedUvs.array), Array.from(inlineUvs.array));
  assert.deepEqual(source.released, [RESOURCE_ID]);
});

void test('packed-v2 voxel UV admission is finite, bounded, fail-atomic, and releases', () => {
  const source = new MapResourceSource();
  const invalid = texturedQuadResourceBytes();
  new DataView(invalid.buffer).setFloat32(112, Number.NaN, true);
  source.resources.set(RESOURCE_ID, invalid);
  const renderer = new ThreeRenderer({ meshResourceSource: source });
  const handle = renderHandle(1);
  renderer.applyDiff({ op: 'create', handle, parent: null, node: meshNode() });
  renderer.applyDiff({ op: 'replaceMeshPayload', handle, payload: texturedQuadPayload() });
  const original = (renderer.objectFor(handle) as THREE.Mesh).geometry;

  assert.throws(
    () => renderer.applyDiff({
      op: 'replaceMeshPayload',
      handle,
      payload: texturedQuadResourcePayload(),
    }),
    /invalid voxel tile coordinate NaN at uvs\[0\]/u,
  );
  assert.equal((renderer.objectFor(handle) as THREE.Mesh).geometry, original);
  assert.deepEqual(source.released, [RESOURCE_ID]);
});

void test('resource mesh payloads fail closed on missing providers and invalid headers', () => {
  const handle = renderHandle(1);
  const without = new ThreeRenderer();
  without.applyDiff({ op: 'create', handle, parent: null, node: meshNode() });
  assert.throws(
    () => without.applyDiff({ op: 'replaceMeshPayload', handle, payload: quadResourcePayload() }),
    /needs a mesh resource provider/u,
  );

  const source = new MapResourceSource();
  const invalid = quadResourceBytes();
  invalid[0] = 0;
  source.resources.set(RESOURCE_ID, invalid);
  const renderer = new ThreeRenderer({ meshResourceSource: source });
  renderer.applyDiff({ op: 'create', handle, parent: null, node: meshNode() });
  assert.throws(
    () => renderer.applyDiff({ op: 'replaceMeshPayload', handle, payload: quadResourcePayload() }),
    /invalid v1 header/u,
  );
  assert.deepEqual(source.released, [RESOURCE_ID]);
});

// ── Shared-buffer static mesh assets ──────────────────────────────────────────

/** A `mesh/crate` static mesh asset whose payload is addressed by a shared-buffer id. */
function handleCrateAsset(buffer: number): StaticMeshAsset {
  return { ...crateAsset(), payload: { ...quadHandlePayload(buffer), provenance: 'staticAsset' } };
}

void test('defineStaticMesh consumes a shared-buffer payload and releases the borrow', () => {
  const source = new MapBufferSource();
  source.set(7, quadHandleBytes());
  const r = new ThreeRenderer({ meshBufferSource: source });

  r.applyDiff({ op: 'defineStaticMesh', asset: handleCrateAsset(7) });
  r.applyDiff({
    op: 'createStaticMeshInstance',
    handle: renderHandle(1),
    parent: null,
    instance: crateInstance(),
  });

  // Borrow was released; nothing retained.
  assert.deepEqual(source.released, [7]);
  assert.equal(source.outstanding, 0);

  // The shared-buffer asset produced the same geometry as the inline path.
  const inline = new ThreeRenderer();
  inline.applyDiff({ op: 'defineStaticMesh', asset: crateAsset() });
  inline.applyDiff({
    op: 'createStaticMeshInstance',
    handle: renderHandle(1),
    parent: null,
    instance: crateInstance(),
  });
  const handleGeo = (r.objectFor(renderHandle(1)) as THREE.Mesh).geometry;
  const inlineGeo = (inline.objectFor(renderHandle(1)) as THREE.Mesh).geometry;
  assert.deepEqual(
    Array.from(handleGeo.getAttribute('position').array),
    Array.from(inlineGeo.getAttribute('position').array),
  );
  assert.deepEqual(Array.from(handleGeo.getIndex()!.array), Array.from(inlineGeo.getIndex()!.array));
});

void test('defineStaticMesh with a handle payload but no provider fails closed', () => {
  const r = new ThreeRenderer(); // no buffer source
  assert.throws(
    () => r.applyDiff({ op: 'defineStaticMesh', asset: handleCrateAsset(7) }),
    /defineStaticMesh: shared-buffer payload needs a mesh buffer provider/,
  );
  // The asset was not defined (no empty geometry left behind).
  assert.throws(
    () =>
      r.applyDiff({
        op: 'createStaticMeshInstance',
        handle: renderHandle(1),
        parent: null,
        instance: crateInstance(),
      }),
    /undefined static mesh asset/,
  );
});

void test('defineStaticMesh with an unknown handle fails closed without leaking a borrow', () => {
  const source = new MapBufferSource(); // buffer 7 never set
  const r = new ThreeRenderer({ meshBufferSource: source });
  assert.throws(
    () => r.applyDiff({ op: 'defineStaticMesh', asset: handleCrateAsset(7) }),
    /defineStaticMesh: buffer 7 unavailable \[missing\]/,
  );
  assert.equal(source.outstanding, 0, 'getBuffer threw, so no borrow to release');
  assert.deepEqual(source.released, []);
});

void test('defineStaticMesh releases the borrow even when the copy fails (too small)', () => {
  const source = new MapBufferSource();
  source.set(7, quadHandleBytes().slice(0, 64)); // truncated
  const r = new ThreeRenderer({ meshBufferSource: source });
  assert.throws(
    () => r.applyDiff({ op: 'defineStaticMesh', asset: handleCrateAsset(7) }),
    /defineStaticMesh: .* exceeds buffer/,
  );
  // Borrow acquired then released on the failure path — no leak.
  assert.deepEqual(source.borrowed, [7]);
  assert.deepEqual(source.released, [7]);
  assert.equal(source.outstanding, 0);
});

void test('a release failure on the success path is classified, not swallowed', () => {
  const source = new MapBufferSource();
  source.set(7, quadHandleBytes());
  source.failReleaseOf(7);
  const r = new ThreeRenderer({ meshBufferSource: source });
  assert.throws(
    () => r.applyDiff({ op: 'defineStaticMesh', asset: handleCrateAsset(7) }),
    /defineStaticMesh: buffer 7 release failed \[missing\]/,
  );
});

// ── Static mesh assets and instances ──────────────────────────────────────────

import type {
  StaticMeshAsset,
  StaticMeshInstanceDescriptor,
  SpriteInstanceDescriptor,
  VoxelObjectInstanceDescriptor,
  VoxelObjectRenderAsset,
} from '@rusty-engine/render-contracts';

function crateAsset(): StaticMeshAsset {
  return {
    asset: 'mesh/crate',
    payload: { ...quadPayload(), provenance: 'staticAsset' },
    materialSlots: [{ slot: 1, material: 'material/wood' }, { slot: 2, material: 'material/iron' }],
    collision: { kind: 'aabbFallback' },
  };
}

function coloredQuadAsset(): StaticMeshAsset {
  const base = quadPayload();
  assert.equal(base.source.kind, 'inline');
  return {
    asset: 'mesh/colored-quad',
    payload: {
      ...base,
      layout: {
        ...base.layout,
        attributes: [
          ...base.layout.attributes,
          { name: 'color', components: 4, kind: 'f32' },
        ],
      },
      source: {
        ...base.source,
        colors: [
          1, 0, 0, 1,
          0, 1, 0, 1,
          0, 0, 1, 0,
          1, 1, 1, 1,
        ],
      },
      provenance: 'staticAsset',
    },
    materialSlots: [
      { slot: 1, material: 'material/colored-mask' },
      { slot: 2, material: 'material/colored-mask' },
    ],
    collision: { kind: 'visualOnly' },
  };
}

void test('static meshes retain normalized vertex alpha with generic mask and double-sided material policy', () => {
  const renderer = new ThreeRenderer();
  renderer.applyFrame({ schemaVersion: 1, ops: [
    {
      op: 'defineMaterial',
      material: {
        schemaVersion: 3,
        id: 'material/colored-mask',
        color: [1, 1, 1, 1],
        texture: null,
        roughness: 1,
        textureTint: [1, 1, 1, 1],
        emissionColor: [0, 0, 0],
        emissionIntensity: 0,
        uvStrategy: 'flat',
        alphaMode: { kind: 'mask', cutoff: 0.5 },
        doubleSided: true,
      },
    },
    { op: 'defineStaticMesh', asset: coloredQuadAsset() },
    {
      op: 'createStaticMeshInstance',
      handle: renderHandle(700),
      parent: null,
      instance: crateInstance('mesh/colored-quad'),
    },
  ] });

  const mesh = renderer.objectFor(renderHandle(700)) as THREE.Mesh;
  const color = mesh.geometry.getAttribute('color');
  const materials = (Array.isArray(mesh.material) ? mesh.material : [mesh.material]) as THREE.MeshStandardMaterial[];
  assert.equal(color.itemSize, 4);
  assert.equal(color.count, 4);
  assert.equal(materials.length, 2);
  for (const material of materials) {
    assert.equal(material.vertexColors, true);
    assert.equal(material.alphaTest, 0.5);
    assert.equal(material.transparent, false);
    assert.equal(material.side, THREE.DoubleSide);
  }
});

void test('defineStaticMesh resolves draw groups through shuffled material slots', () => {
  const asset: StaticMeshAsset = {
    ...crateAsset(),
    payload: {
      ...quadPayload(),
      provenance: 'staticAsset',
      groups: [
        { materialSlot: 122, start: 0, count: 3 },
        { materialSlot: 68, start: 3, count: 3 },
      ],
    },
    materialSlots: [
      { slot: 68, material: 'material/floor' },
      { slot: 122, material: 'material/mural' },
    ],
  };
  const renderer = new ThreeRenderer();
  renderer.applyDiff({ op: 'defineStaticMesh', asset });
  renderer.applyDiff({
    op: 'createStaticMeshInstance',
    handle: renderHandle(1),
    parent: null,
    instance: crateInstance(asset.asset),
  });

  const geometry = (renderer.objectFor(renderHandle(1)) as THREE.Mesh).geometry;
  assert.deepEqual(
    geometry.groups.map((group) => [group.start, group.count, group.materialIndex]),
    [[0, 3, 1], [3, 3, 0]],
  );
});

void test('defineStaticMesh rejects wholly unbound draw groups before publication', () => {
  const renderer = new ThreeRenderer();
  const asset: StaticMeshAsset = {
    ...crateAsset(),
    payload: {
      ...quadPayload(),
      provenance: 'staticAsset',
      groups: [
        { materialSlot: 999, start: 0, count: 3 },
        { materialSlot: 1000, start: 3, count: 3 },
      ],
    },
  };

  assert.throws(
    () => renderer.applyDiff({ op: 'defineStaticMesh', asset }),
    /defineStaticMesh: unbound material slot 999/u,
  );
  assert.equal(renderer.snapshot(), '(empty scene)\n');
  assert.equal(renderer.instanceCountFor(asset.asset), 0);
});

function crateInstance(
  asset = 'mesh/crate',
  overrides: StaticMeshInstanceDescriptor['materialOverrides'] = [],
): StaticMeshInstanceDescriptor {
  return {
    asset,
    transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
    visible: true,
    materialOverrides: overrides,
    metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label: asset },
  };
}

function voxelObjectAsset(over: Partial<VoxelObjectRenderAsset> = {}): VoxelObjectRenderAsset {
  const second = quadPayload();
  return {
    asset: 'voxel-object/runner',
    contentHash: 'sha256:runner-v1',
    meshes: [
      { payload: { ...quadPayload(), provenance: 'voxelObject' } },
      {
        payload: {
          ...second,
          bounds: { min: [0, 0, 0], max: [2, 1, 0] },
          groups: [{ materialSlot: 2, start: 0, count: 6 }],
          source: second.source.kind === 'inline'
            ? { ...second.source, positions: second.source.positions.map((value, index) => index % 3 === 0 ? value * 2 : value) }
            : second.source,
          provenance: 'voxelObject',
        },
      },
    ],
    frames: [{ id: 'default', mesh: 0 }, { id: 'walk/0', mesh: 1 }],
    materialSlots: [{ slot: 1, material: 'material/wood' }, { slot: 2, material: 'material/iron' }],
    ...over,
  };
}

function voxelObjectInstance(frame = 0): VoxelObjectInstanceDescriptor {
  return {
    asset: 'voxel-object/runner',
    frame,
    transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
    visible: true,
    materialOverrides: [],
    metadata: { sourceEntity: 9, sourceSceneNode: null, tags: ['voxel-object'], label: 'Runner' },
  };
}

void test('voxel-object instances share frame meshes and swap frames without handle churn', () => {
  const renderer = new ThreeRenderer();
  const first = renderHandle(81);
  const second = renderHandle(82);
  renderer.applyDiff({ op: 'defineVoxelObject', asset: voxelObjectAsset() });
  renderer.applyDiff({ op: 'createVoxelObjectInstance', handle: first, parent: null, instance: voxelObjectInstance() });
  renderer.applyDiff({ op: 'createVoxelObjectInstance', handle: second, parent: null, instance: voxelObjectInstance() });
  const firstMesh = renderer.objectFor(first) as THREE.Mesh;
  const secondMesh = renderer.objectFor(second) as THREE.Mesh;
  assert.equal(firstMesh.geometry, secondMesh.geometry);
  const initialBatch = renderer.scene.children
    .flatMap((object) => {
      const found: THREE.InstancedMesh[] = [];
      object.traverse((child) => {
        if (child instanceof THREE.InstancedMesh) found.push(child);
      });
      return found;
    })[0];
  assert.ok(initialBatch instanceof THREE.InstancedMesh);
  assert.equal(initialBatch.count, 2);
  assert.equal(renderer.projectionIdentityForObject(initialBatch, 1)?.handle, second);
  const original = firstMesh.geometry;

  renderer.applyDiff({ op: 'setVoxelObjectFrame', handle: first, frame: 1 });
  assert.equal(renderer.objectFor(first), firstMesh, 'frame swap keeps the retained object and handle');
  assert.notEqual(firstMesh.geometry, original);
  assert.equal(secondMesh.geometry, original);
  assert.equal(firstMesh.geometry.groups[0]?.materialIndex, 1, 'frame groups resolve palette slots');
  assert.deepEqual(renderer.voxelObjectFrame(first), {
    handle: first, asset: 'voxel-object/runner', frame: 1, frameId: 'walk/0', mesh: 1,
  });
  assert.equal(renderer.pickMesh(first)?.provenance, 'voxelObject');
  assert.equal(
    renderer.scene.children.some((object) => {
      let found = false;
      object.traverse((child) => { found ||= child instanceof THREE.InstancedMesh; });
      return found;
    }),
    false,
    'different voxel frames keep their incompatible geometries as ordinary meshes',
  );
  renderer.applyDiff({ op: 'setVoxelObjectFrame', handle: second, frame: 1 });
  const replacementBatches: THREE.InstancedMesh[] = [];
  renderer.scene.traverse((object) => {
    if (object instanceof THREE.InstancedMesh) replacementBatches.push(object);
  });
  assert.equal(replacementBatches.length, 1);
  assert.equal(replacementBatches[0]?.count, 2);
});

void test('voxel-scene material definitions do not replace voxel-object palette materials', () => {
  const renderer = new ThreeRenderer();
  const handle = renderHandle(83);
  const iron = { ...woodMaterial(), id: 'material/iron', color: [0.4, 0.4, 0.45, 1] } as const;
  renderer.applyFrame({
    schemaVersion: 1,
    ops: [
      { op: 'defineMaterial', material: woodMaterial() },
      { op: 'defineMaterial', material: iron },
      { op: 'defineVoxelObject', asset: voxelObjectAsset() },
      {
        op: 'createVoxelObjectInstance',
        handle,
        parent: null,
        instance: voxelObjectInstance(),
      },
    ],
  });
  renderer.applyFrame({
    schemaVersion: 1,
    ops: [
      {
        op: 'defineMaterial',
        material: { ...woodMaterial(), id: 'voxel-material/1', color: [0.1, 0.8, 0.2, 1] },
      },
      {
        op: 'defineMaterial',
        material: { ...woodMaterial(), id: 'voxel-material/2', color: [0.8, 0.1, 0.2, 1] },
      },
    ],
  });

  const materials = (renderer.objectFor(handle) as THREE.Mesh).material as THREE.MeshStandardMaterial[];
  assert.equal(materials.length, 2, 'asset palette cardinality remains intact');
  assert.ok(Math.abs(materials[0]!.color.r - 0.6) < 1e-6, 'slot 1 retains material/wood');
  assert.ok(Math.abs(materials[0]!.color.b - 0.2) < 1e-6, 'slot 1 is not a debug hue');
  assert.ok(Math.abs(materials[1]!.color.r - 0.4) < 1e-6, 'slot 2 retains material/iron');
  assert.ok(Math.abs(materials[1]!.color.b - 0.45) < 1e-6, 'slot 2 is not a debug hue');
  assert.equal(renderer.fallbackMaterialCount, 0);
});

void test('voxel-object definitions consume the content-addressed mesh resource path', () => {
  const source = new MapResourceSource();
  source.resources.set(RESOURCE_ID, quadResourceBytes());
  const renderer = new ThreeRenderer({ meshResourceSource: source });
  const asset = voxelObjectAsset({
    meshes: [{ payload: { ...quadResourcePayload(), provenance: 'voxelObject' } }],
    frames: [{ id: 'default', mesh: 0 }],
  });
  renderer.applyDiff({ op: 'defineVoxelObject', asset });
  renderer.applyDiff({
    op: 'createVoxelObjectInstance',
    handle: renderHandle(83),
    parent: null,
    instance: voxelObjectInstance(),
  });
  assert.deepEqual(source.acquired, [RESOURCE_ID]);
  assert.deepEqual(source.released, [RESOURCE_ID]);
});

void test('voxel-object frame failure is atomic and explicit release bounds GPU lifetime', () => {
  const renderer = new ThreeRenderer();
  const handle = renderHandle(83);
  renderer.applyDiff({ op: 'defineVoxelObject', asset: voxelObjectAsset() });
  renderer.applyDiff({ op: 'createVoxelObjectInstance', handle, parent: null, instance: voxelObjectInstance() });
  const mesh = renderer.objectFor(handle) as THREE.Mesh;
  const before = mesh.geometry;
  assert.throws(
    () => renderer.applyDiff({ op: 'setVoxelObjectFrame', handle, frame: 99 }),
    /outside voxel object|unavailable/,
  );
  assert.equal(mesh.geometry, before);
  assert.throws(
    () => renderer.applyDiff({ op: 'releaseVoxelObject', asset: 'voxel-object/runner' }),
    /in use by 1 instance/,
  );

  let disposed = 0;
  before.addEventListener('dispose', () => { disposed += 1; });
  renderer.applyDiff({ op: 'destroy', handle });
  renderer.applyDiff({ op: 'releaseVoxelObject', asset: 'voxel-object/runner' });
  assert.equal(disposed, 1);
});

void test('voxel surface redefinition, release, and reopen keep retained resources bounded', () => {
  const renderer = new ThreeRenderer();
  const handle = renderHandle(84);
  const first = voxelObjectAsset({
    meshes: [{ payload: { ...quadPayload(), provenance: 'voxelObject' } }],
    frames: [{ id: 'default', mesh: 0 }],
  });
  renderer.applyDiff({ op: 'defineVoxelObject', asset: first });
  renderer.applyDiff({
    op: 'createVoxelObjectInstance', handle, parent: null, instance: voxelObjectInstance(),
  });
  const mesh = renderer.objectFor(handle) as THREE.Mesh;
  const greedyGeometry = mesh.geometry;
  const before = renderer.resourceStatistics();
  let greedyDisposed = false;
  greedyGeometry.addEventListener('dispose', () => { greedyDisposed = true; });

  const reconstructedPayload = quadPayload();
  if (reconstructedPayload.source.kind !== 'inline') throw new Error('surface fixture must remain inline');
  const reconstructed = voxelObjectAsset({
    contentHash: 'sha256:same-canonical-voxels-dual-contouring',
    meshes: [{
      payload: {
        ...reconstructedPayload,
        bounds: { min: [-0.1, 0, 0], max: [1.1, 1, 0] },
        source: {
          ...reconstructedPayload.source,
          positions: [-0.1, 0, 0, 1.1, 0, 0, 1, 1, 0, 0, 1, 0],
        },
        provenance: 'voxelObject',
      },
    }],
    frames: [{ id: 'default', mesh: 0 }],
  });
  renderer.applyDiff({ op: 'defineVoxelObject', asset: reconstructed });
  assert.equal(renderer.objectFor(handle), mesh, 'surface replacement keeps the retained handle/object');
  assert.notEqual(mesh.geometry, greedyGeometry);
  assert.equal(greedyDisposed, true, 'superseded surface geometry is disposed');
  assert.deepEqual(renderer.resourceStatistics(), before, 'surface replacement has no resource growth');

  renderer.applyDiff({ op: 'destroy', handle });
  renderer.applyDiff({ op: 'releaseVoxelObject', asset: reconstructed.asset });
  assert.equal(renderer.resourceStatistics().geometryResourceCount, 0);
  assert.equal(renderer.resourceStatistics().materialResourceCount, 0);

  renderer.applyDiff({ op: 'defineVoxelObject', asset: first });
  renderer.applyDiff({
    op: 'createVoxelObjectInstance', handle, parent: null, instance: voxelObjectInstance(),
  });
  assert.deepEqual(renderer.resourceStatistics(), before, 'reopen restores exactly one retained set');
  renderer.dispose();
  assert.deepEqual(renderer.resourceStatistics(), {
    renderHandleCount: 0,
    geometryResourceCount: 0,
    materialResourceCount: 0,
    textureResourceCount: 0,
    animatedInstanceCount: 0,
  });
});

void test('two instances share one BufferGeometry and the asset is reference-counted', () => {
  const r = new ThreeRenderer();
  r.applyDiff({ op: 'defineStaticMesh', asset: crateAsset() });
  r.applyDiff({ op: 'createStaticMeshInstance', handle: renderHandle(1), parent: null, instance: crateInstance() });
  r.applyDiff({ op: 'createStaticMeshInstance', handle: renderHandle(2), parent: null, instance: crateInstance() });

  const a = r.objectFor(renderHandle(1)) as THREE.Mesh;
  const b = r.objectFor(renderHandle(2)) as THREE.Mesh;
  assert.equal(a.geometry, b.geometry, 'instances must share one geometry');
  assert.equal(r.instanceCountFor('mesh/crate'), 2);
});

void test('compatible repeated static instances batch without losing handle identity or lifecycle', () => {
  const renderer = new ThreeRenderer();
  const instanceCount = 300;
  renderer.applyFrame({
    schemaVersion: 1,
    ops: [
      { op: 'defineStaticMesh', asset: crateAsset() },
      ...Array.from({ length: instanceCount }, (_, index): RenderDiff => ({
        op: 'createStaticMeshInstance',
        handle: renderHandle(1_000 + index),
        parent: null,
        instance: {
          ...crateInstance(),
          transform: {
            translation: [(index % 30) / 4, 0, Math.floor(index / 30) / 2],
            rotation: [0, 0, 0, 1],
            scale: [1, 1, 1],
          },
          metadata: {
            sourceEntity: 10_000 + index,
            sourceSceneNode: null,
            tags: ['repeated'],
            label: `crate-${String(index)}`,
          },
        },
      })),
    ],
  });

  const batches: THREE.InstancedMesh[] = [];
  renderer.scene.traverse((object) => {
    if (object instanceof THREE.InstancedMesh) batches.push(object);
  });
  assert.equal(batches.length, 1);
  const batch = batches[0]!;
  assert.equal(batch.count, instanceCount);
  assert.equal(batch.frustumCulled, true);
  assert.ok(batch.boundingBox instanceof THREE.Box3);
  assert.ok(batch.boundingSphere instanceof THREE.Sphere);
  assert.equal(renderer.handleCount, instanceCount);
  assert.equal(renderer.objectFor(renderHandle(1_127)) instanceof THREE.Mesh, true);
  assert.equal(renderer.objectFor(renderHandle(1_127)) instanceof THREE.InstancedMesh, false);
  assert.deepEqual(renderer.projectionIdentityForObject(batch, 127), {
    handle: renderHandle(1_127),
    layer: 'scene',
    metadata: {
      sourceEntity: 10_127,
      sourceSceneNode: null,
      tags: ['repeated'],
      label: 'crate-127',
    },
  });

  renderer.applyFrame({
    schemaVersion: 1,
    ops: [{
      op: 'update',
      handle: renderHandle(1_127),
      transform: {
        translation: [6, 2, 3],
        rotation: [0, 0, 0, 1],
        scale: [1, 1, 1],
      },
      material: null,
      visible: null,
      metadata: null,
    }],
  });
  const currentBatches: THREE.InstancedMesh[] = [];
  renderer.scene.traverse((object) => {
    if (object instanceof THREE.InstancedMesh) currentBatches.push(object);
  });
  assert.equal(currentBatches[0], batch, 'transform-only updates reuse the batch allocation');
  const matrix = new THREE.Matrix4();
  batch.getMatrixAt(127, matrix);
  assert.deepEqual(new THREE.Vector3().setFromMatrixPosition(matrix).toArray(), [6, 2, 3]);

  let disposed = false;
  batch.addEventListener('dispose', () => { disposed = true; });
  renderer.applyFrame({
    schemaVersion: 1,
    ops: Array.from({ length: instanceCount - 1 }, (_, index): RenderDiff => ({
      op: 'destroy',
      handle: renderHandle(1_000 + index),
    })),
  });
  assert.equal(disposed, true, 'a no-longer-useful batch releases its instance buffer');
  assert.equal(renderer.has(renderHandle(1_299)), true);
  assert.equal(renderer.objectFor(renderHandle(1_299))?.layers.test(new THREE.Layers()), true);
  assert.equal(renderer.instanceCountFor('mesh/crate'), 1);
  renderer.dispose();
  assert.equal(renderer.handleCount, 0);
  assert.equal(renderer.resourceStatistics().geometryResourceCount, 0);
});

void test('a static batch realization failure makes the mutated renderer explicitly terminal', () => {
  const renderer = new ThreeRenderer();
  const originalSetMatrixAt = THREE.InstancedMesh.prototype.setMatrixAt;
  THREE.InstancedMesh.prototype.setMatrixAt = function failStaticBatchRealization() {
    throw new Error('injected instance allocator failure');
  };
  try {
    assert.throws(
      () => renderer.applyFrame({
        schemaVersion: 1,
        ops: [
          { op: 'defineStaticMesh', asset: crateAsset() },
          { op: 'createStaticMeshInstance', handle: renderHandle(1), parent: null, instance: crateInstance() },
          { op: 'createStaticMeshInstance', handle: renderHandle(2), parent: null, instance: crateInstance() },
        ],
      }),
      (cause: unknown) => cause instanceof RendererTerminalError
        && cause.phase === 'static_instance_batch'
        && cause.message.includes('terminal state'),
    );
    assert.throws(
      () => renderer.applyDiff(createDiff(3, cubeNode())),
      (cause: unknown) => cause instanceof RendererTerminalError
        && cause.message.includes('injected instance allocator failure'),
    );
  } finally {
    THREE.InstancedMesh.prototype.setMatrixAt = originalSetMatrixAt;
    renderer.dispose();
  }
});

void test('dense level-scale repeated instances compact visible members into bounded definition batches', () => {
  const renderer = new ThreeRenderer();
  const definitionCount = 9;
  const instanceCount = 367;
  renderer.applyFrame({
    schemaVersion: 1,
    ops: [
      ...Array.from({ length: definitionCount }, (_, index): RenderDiff => ({
        op: 'defineStaticMesh',
        asset: {
          ...crateAsset(),
          asset: `mesh/dense-${String(index)}`,
        },
      })),
      ...Array.from({ length: instanceCount }, (_, index): RenderDiff => {
        const asset = `mesh/dense-${String(index % definitionCount)}`;
        return {
          op: 'createStaticMeshInstance',
          handle: renderHandle(3_000 + index),
          parent: null,
          instance: {
            ...crateInstance(asset),
            transform: {
              // A compact 48-by-32-unit level-like distribution matching the
              // acceptance consumer's repeated authored placement density.
              translation: [index % 48, 0, Math.floor(index / 48) * 4],
              rotation: [0, 0, 0, 1],
              scale: [1, 1, 1],
            },
            metadata: {
              sourceEntity: 30_000 + index,
              sourceSceneNode: null,
              tags: ['dense-spatial-batch'],
              label: `dense-spatial-crate-${String(index)}`,
            },
          },
        };
      }),
    ],
  });

  const batches: THREE.InstancedMesh[] = [];
  renderer.scene.traverse((object) => {
    if (object instanceof THREE.InstancedMesh) batches.push(object);
  });
  assert.equal(
    batches.length,
    definitionCount,
    'draw groups stay bounded by compatible definitions, not spatial placement cells',
  );
  assert.equal(
    batches.reduce((total, batch) => total + batch.count, 0),
    instanceCount,
  );

  const camera = new THREE.OrthographicCamera(-10, 10, 10, -10, 0.1, 100);
  camera.up.set(0, 0, -1);
  camera.position.set(8, 40, 8);
  camera.lookAt(8, 0, 8);
  camera.updateProjectionMatrix();
  renderer.prepareStaticInstanceBatches(camera);

  assert.equal(
    batches.length,
    definitionCount,
    'moving-camera compaction must not multiply definition draw groups',
  );
  const submittedCount = batches.reduce((total, batch) => total + batch.count, 0);
  assert.ok(submittedCount >= definitionCount * 2);
  assert.ok(
    submittedCount < instanceCount,
    'only camera-visible members are submitted while retained identities remain resident',
  );
  for (const batch of batches) {
    assert.equal(batch.frustumCulled, true);
    assert.ok(batch.boundingBox instanceof THREE.Box3);
    assert.ok(batch.boundingSphere instanceof THREE.Sphere);
    for (let index = 0; index < batch.count; index += 1) {
      assert.ok(renderer.projectionIdentityForObject(batch, index)?.handle !== undefined);
    }
  }
  assert.equal(renderer.handleCount, instanceCount);
  renderer.dispose();
  assert.equal(renderer.handleCount, 0);
});

void test('compatible static batches repack exact visible handles as the camera moves', () => {
  const renderer = new ThreeRenderer();
  renderer.applyFrame({
    schemaVersion: 1,
    ops: [
      { op: 'defineStaticMesh', asset: crateAsset() },
      ...[0, 1, 2, 128, 129, 130].map((x, index): RenderDiff => ({
        op: 'createStaticMeshInstance',
        handle: renderHandle(2_000 + index),
        parent: null,
        instance: {
          ...crateInstance(),
          transform: {
            translation: [x, 0, 0],
            rotation: [0, 0, 0, 1],
            scale: [1, 1, 1],
          },
          metadata: {
            sourceEntity: 20_000 + index,
            sourceSceneNode: null,
            tags: ['spatial-batch'],
            label: `spatial-crate-${String(index)}`,
          },
        },
      })),
    ],
  });

  const batches: THREE.InstancedMesh[] = [];
  renderer.scene.traverse((object) => {
    if (object instanceof THREE.InstancedMesh) batches.push(object);
  });
  assert.equal(batches.length, 1);
  const batch = batches[0]!;

  const camera = new THREE.PerspectiveCamera(55, 1, 0.1, 100);
  const lookAt = (x: number): void => {
    camera.position.set(x, 0, 8);
    camera.lookAt(x, 0, 0);
    renderer.prepareStaticInstanceBatches(camera);
  };

  lookAt(0);
  assert.equal(batch.count, 3);
  assert.deepEqual(
    Array.from({ length: batch.count }, (_, index) =>
      renderer.projectionIdentityForObject(batch, index)?.handle),
    [renderHandle(2_000), renderHandle(2_001), renderHandle(2_002)],
  );
  assert.deepEqual(
    renderer.visibilityReadout(camera).handles
      .filter(({ state }) => state === 'frustumVisible')
      .map(({ handle }) => handle),
    [renderHandle(2_000), renderHandle(2_001), renderHandle(2_002)],
    'visibility query matches the prepared static batch for the same camera',
  );
  lookAt(128);
  assert.equal(batch.count, 3);
  assert.deepEqual(
    Array.from({ length: batch.count }, (_, index) =>
      renderer.projectionIdentityForObject(batch, index)?.handle),
    [renderHandle(2_003), renderHandle(2_004), renderHandle(2_005)],
  );
  renderer.prepareStaticInstanceBatchesForPicking();
  assert.equal(batch.count, 6, 'arbitrary world-ray picking restores every retained candidate');
  assert.deepEqual(
    Array.from({ length: batch.count }, (_, index) =>
      renderer.projectionIdentityForObject(batch, index)?.handle),
    [
      renderHandle(2_000),
      renderHandle(2_001),
      renderHandle(2_002),
      renderHandle(2_003),
      renderHandle(2_004),
      renderHandle(2_005),
    ],
  );
  assert.deepEqual(
    renderer.visibilityReadout(camera).handles
      .filter(({ state }) => state === 'frustumVisible')
      .map(({ handle }) => handle),
    [renderHandle(2_003), renderHandle(2_004), renderHandle(2_005)],
    'picking does not broaden a fresh visibility query',
  );

  renderer.applyFrame({
    schemaVersion: 1,
    ops: [{
      op: 'update',
      handle: renderHandle(2_001),
      transform: {
        translation: [131, 0, 0],
        rotation: [0, 0, 0, 1],
        scale: [1, 1, 1],
      },
      material: null,
      visible: null,
      metadata: null,
    }],
  });
  const regrouped: THREE.InstancedMesh[] = [];
  renderer.scene.traverse((object) => {
    if (object instanceof THREE.InstancedMesh) regrouped.push(object);
  });
  assert.deepEqual(regrouped, [batch], 'transform changes retain the definition batch');
  lookAt(128);
  assert.equal(batch.count, 4);
  const movedIndex = Array.from({ length: batch.count }, (_, index) =>
    renderer.projectionIdentityForObject(batch, index)?.handle)
    .indexOf(renderHandle(2_001));
  assert.notEqual(movedIndex, -1);
  const movedMatrix = new THREE.Matrix4();
  batch.getMatrixAt(movedIndex, movedMatrix);
  assert.deepEqual(
    new THREE.Vector3().setFromMatrixPosition(movedMatrix).toArray(),
    [131, 0, 0],
  );
  assert.ok(batch.boundingSphere instanceof THREE.Sphere);
  renderer.dispose();
  assert.equal(renderer.handleCount, 0);
});

void test('batch admission excludes invisible, overridden, reflected, and non-world instances', () => {
  const renderer = new ThreeRenderer();
  renderer.applyFrame({
    schemaVersion: 1,
    ops: [
      { op: 'defineStaticMesh', asset: crateAsset() },
      {
        op: 'create',
        handle: renderHandle(1),
        parent: null,
        node: {
          ...cubeNode('viewmodel-parent'),
          geometry: { kind: 'group' },
          layer: 'viewmodel',
        },
      },
      {
        op: 'createStaticMeshInstance',
        handle: renderHandle(2),
        parent: null,
        instance: crateInstance(),
      },
      {
        op: 'createStaticMeshInstance',
        handle: renderHandle(3),
        parent: null,
        instance: {
          ...crateInstance(),
          transform: {
            translation: [2, 0, 0],
            rotation: [0, 0, 0, 1],
            scale: [1, 1, 1],
          },
        },
      },
      {
        op: 'createStaticMeshInstance',
        handle: renderHandle(4),
        parent: null,
        instance: { ...crateInstance(), visible: false },
      },
      {
        op: 'createStaticMeshInstance',
        handle: renderHandle(5),
        parent: null,
        instance: crateInstance('mesh/crate', [{ slot: 1, material: 'material/red' }]),
      },
      {
        op: 'createStaticMeshInstance',
        handle: renderHandle(6),
        parent: null,
        instance: {
          ...crateInstance(),
          transform: {
            translation: [4, 0, 0],
            rotation: [0, 0, 0, 1],
            scale: [-1, 1, 1],
          },
        },
      },
      {
        op: 'createStaticMeshInstance',
        handle: renderHandle(7),
        parent: renderHandle(1),
        instance: crateInstance(),
      },
    ],
  });

  const batches: THREE.InstancedMesh[] = [];
  renderer.scene.traverse((object) => {
    if (object instanceof THREE.InstancedMesh) batches.push(object);
  });
  assert.equal(batches.length, 1);
  assert.equal(batches[0]?.count, 2);
  assert.deepEqual(
    [0, 1].map((index) => renderer.projectionIdentityForObject(batches[0]!, index)?.handle),
    [renderHandle(2), renderHandle(3)],
  );
  for (const handle of [4, 5, 6, 7]) {
    const object = renderer.objectFor(renderHandle(handle));
    assert.equal(object?.layers.test(new THREE.Layers()), true);
  }
  assert.equal(
    renderer.projectionIdentityForObject(renderer.objectFor(renderHandle(7))!)?.layer,
    'viewmodel',
  );
});

void test('static mesh definitions survive zero instances and dispose only on redefine or renderer disposal', () => {
  const r = new ThreeRenderer();
  r.applyDiff({ op: 'defineStaticMesh', asset: crateAsset() });
  r.applyDiff({ op: 'createStaticMeshInstance', handle: renderHandle(1), parent: null, instance: crateInstance() });
  r.applyDiff({ op: 'createStaticMeshInstance', handle: renderHandle(2), parent: null, instance: crateInstance() });

  const shared = (r.objectFor(renderHandle(1)) as THREE.Mesh).geometry;
  let disposed = false;
  shared.addEventListener('dispose', () => { disposed = true; });

  r.applyDiff({ op: 'destroy', handle: renderHandle(1) });
  assert.equal(disposed, false, 'shared geometry must survive while an instance remains');
  assert.equal(r.instanceCountFor('mesh/crate'), 1);

  r.applyDiff({ op: 'destroy', handle: renderHandle(2) });
  assert.equal(disposed, false, 'retained definition survives when its last instance is gone');
  assert.equal(r.instanceCountFor('mesh/crate'), 0);
  assert.deepEqual(r.resourceStatistics(), {
    renderHandleCount: 0,
    geometryResourceCount: 1,
    materialResourceCount: 2,
    textureResourceCount: 0,
    animatedInstanceCount: 0,
  });

  r.applyDiff({
    op: 'createStaticMeshInstance',
    handle: renderHandle(3),
    parent: null,
    instance: crateInstance(),
  });
  assert.equal((r.objectFor(renderHandle(3)) as THREE.Mesh).geometry, shared);
  assert.equal(r.instanceCountFor('mesh/crate'), 1);

  r.applyDiff({ op: 'destroy', handle: renderHandle(3) });
  r.applyDiff({ op: 'defineStaticMesh', asset: crateAsset() });
  assert.equal(disposed, true, 'redefining an unused asset disposes the replaced definition');

  const replacement = (() => {
    r.applyDiff({
      op: 'createStaticMeshInstance',
      handle: renderHandle(4),
      parent: null,
      instance: crateInstance(),
    });
    return (r.objectFor(renderHandle(4)) as THREE.Mesh).geometry;
  })();
  let replacementDisposed = false;
  replacement.addEventListener('dispose', () => { replacementDisposed = true; });
  r.applyDiff({ op: 'destroy', handle: renderHandle(4) });
  assert.equal(replacementDisposed, false);
  r.dispose();
  assert.equal(replacementDisposed, true, 'renderer disposal releases retained definitions');
  assert.deepEqual(r.resourceStatistics(), {
    renderHandleCount: 0,
    geometryResourceCount: 0,
    materialResourceCount: 0,
    textureResourceCount: 0,
    animatedInstanceCount: 0,
  });
});

void test('a frame can destroy the last static mesh instance and recreate it without resource churn', () => {
  const r = new ThreeRenderer();
  r.applyFrame({
    schemaVersion: 1,
    ops: [
      { op: 'defineStaticMesh', asset: crateAsset() },
      {
        op: 'createStaticMeshInstance',
        handle: renderHandle(1),
        parent: null,
        instance: crateInstance(),
      },
    ],
  });
  const shared = (r.objectFor(renderHandle(1)) as THREE.Mesh).geometry;
  const before = r.resourceStatistics();

  r.applyFrame({
    schemaVersion: 1,
    ops: [
      { op: 'destroy', handle: renderHandle(1) },
      {
        op: 'createStaticMeshInstance',
        handle: renderHandle(2),
        parent: null,
        instance: {
          ...crateInstance(),
          metadata: {
            sourceEntity: 2,
            sourceSceneNode: null,
            tags: ['same-frame'],
            label: 'same-frame replacement',
          },
        },
      },
    ],
  });

  assert.equal(r.has(renderHandle(1)), false);
  assert.equal((r.objectFor(renderHandle(2)) as THREE.Mesh).geometry, shared);
  assert.equal(r.instanceCountFor('mesh/crate'), 1);
  assert.deepEqual(r.resourceStatistics(), before);
  assert.doesNotMatch(r.snapshot(), /handle 1 /u);
  assert.match(
    r.snapshot(),
    /handle 2 .*kind staticMesh .*asset mesh\/crate .*label "same-frame replacement"/u,
  );
});

void test('a rejected static mesh replacement frame leaves the live instance and resources unchanged', () => {
  const r = new ThreeRenderer();
  r.applyDiff({ op: 'defineStaticMesh', asset: crateAsset() });
  r.applyDiff({
    op: 'createStaticMeshInstance',
    handle: renderHandle(1),
    parent: null,
    instance: crateInstance(),
  });
  const beforeObject = r.objectFor(renderHandle(1));
  const beforeSnapshot = r.snapshot();
  const beforeResources = r.resourceStatistics();

  assert.throws(() => r.applyFrame({
    schemaVersion: 1,
    ops: [
      { op: 'destroy', handle: renderHandle(1) },
      {
        op: 'createStaticMeshInstance',
        handle: renderHandle(2),
        parent: null,
        instance: crateInstance('mesh/missing'),
      },
    ],
  }), /undefined static mesh asset mesh\/missing/);

  assert.equal(r.objectFor(renderHandle(1)), beforeObject);
  assert.equal(r.has(renderHandle(2)), false);
  assert.equal(r.snapshot(), beforeSnapshot);
  assert.deepEqual(r.resourceStatistics(), beforeResources);
});

void test('per-instance material overrides apply only to that instance', () => {
  const r = new ThreeRenderer();
  r.registerSlotColor(1, 0, 0, 1); // base slot 1 → blue
  r.applyDiff({ op: 'defineStaticMesh', asset: crateAsset() });
  r.applyDiff({ op: 'createStaticMeshInstance', handle: renderHandle(1), parent: null, instance: crateInstance() });
  r.applyDiff({
    op: 'createStaticMeshInstance',
    handle: renderHandle(2),
    parent: null,
    instance: crateInstance('mesh/crate', [{ slot: 1, material: 'material/wood-red' }]),
  });

  const base = (r.objectFor(renderHandle(1)) as THREE.Mesh).material as THREE.MeshBasicMaterial[];
  const overridden = (r.objectFor(renderHandle(2)) as THREE.Mesh).material as THREE.MeshBasicMaterial[];
  // Slot-2 material is shared (identical object); slot-1 override is a distinct material instance.
  assert.equal(base[1], overridden[1], 'non-overridden slot material is shared');
  assert.notEqual(base[0], overridden[0], 'overridden slot gets its own material');
});

// ── Retained material descriptors ──────────────────────────────────────────────

import type { RenderMaterialDescriptor } from '@rusty-engine/render-contracts';

function woodMaterial(): RenderMaterialDescriptor {
  return {
    schemaVersion: 2,
    id: 'material/wood',
    color: [0.6, 0.4, 0.2, 1],
    texture: null,
    roughness: 1,
    textureTint: [1, 1, 1, 1],
    emissionColor: [0, 0, 0],
    emissionIntensity: 0,
    uvStrategy: 'flat',
  };
}

function pngCrc32(bytes: Uint8Array): number {
  let crc = 0xffff_ffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) {
      crc = (crc & 1) === 0 ? crc >>> 1 : (crc >>> 1) ^ 0xedb8_8320;
    }
  }
  return (crc ^ 0xffff_ffff) >>> 0;
}

function pngChunk(type: string, data: Uint8Array): Uint8Array {
  const chunk = new Uint8Array(12 + data.byteLength);
  const view = new DataView(chunk.buffer);
  view.setUint32(0, data.byteLength, false);
  for (let index = 0; index < 4; index++) chunk[4 + index] = type.charCodeAt(index);
  chunk.set(data, 8);
  view.setUint32(8 + data.byteLength, pngCrc32(chunk.subarray(4, 8 + data.byteLength)), false);
  return chunk;
}

function rgbaPng(width: number, height: number, pixels: readonly number[]): Uint8Array {
  assert.equal(pixels.length, width * height * 4);
  const header = new Uint8Array(13);
  const headerView = new DataView(header.buffer);
  headerView.setUint32(0, width, false);
  headerView.setUint32(4, height, false);
  header.set([8, 6, 0, 0, 0], 8);
  const filtered = new Uint8Array(height * (width * 4 + 1));
  for (let row = 0; row < height; row++) {
    filtered[row * (width * 4 + 1)] = 0;
    filtered.set(pixels.slice(row * width * 4, (row + 1) * width * 4), row * (width * 4 + 1) + 1);
  }
  const chunks = [
    pngChunk('IHDR', header),
    pngChunk('IDAT', zlibSync(filtered)),
    pngChunk('IEND', new Uint8Array()),
  ];
  const bytes = new Uint8Array(8 + chunks.reduce((sum, chunk) => sum + chunk.byteLength, 0));
  bytes.set([137, 80, 78, 71, 13, 10, 26, 10]);
  let offset = 8;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return bytes;
}

function textureDescriptor(
  bytes: Uint8Array,
  version = 1,
  source: 'inline' | 'resource' = 'inline',
  id = 'texture/checker',
  colorSpace: 'srgb' | 'linear' = 'srgb',
): import('@rusty-engine/render-contracts').TextureDescriptor {
  const digest = bytesToHex(sha256(bytes));
  const contentHash = `sha256:${digest}`;
  return {
    id,
    width: 2,
    height: 1,
    filter: 'nearest',
    wrap: 'repeat',
    contentHash,
    version,
    payload: {
      encoding: 'pngRgba8',
      colorSpace,
      contentHash,
      byteLength: bytes.byteLength,
      source: source === 'inline'
        ? { kind: 'inline', encodedBytes: [...bytes] }
        : { kind: 'resource', resource: `texture-resource/${digest}` },
    },
  };
}

function texturedMaterial(): RenderMaterialDescriptor {
  return {
    ...woodMaterial(),
    schemaVersion: 3,
    texture: 'texture/checker',
    uvStrategy: 'atlas',
  };
}

function voxelTexturedMaterial(
  texture: import('@rusty-engine/render-contracts').TextureDescriptor,
  mapping: 'repeat' | 'atlas' = 'repeat',
): RenderMaterialDescriptor {
  const common = {
    texture: texture.id,
    textureVersion: texture.version,
    textureContentHash: texture.contentHash!,
    tileScaleCells: [1, 1] as const,
    tileOriginCells: [-4, 8] as const,
  };
  return {
    ...texturedMaterial(),
    id: `material/voxel-${mapping}`,
    texture: texture.id,
    emissionColor: [0.1, 0.2, 0.3],
    emissionIntensity: 0.5,
    voxelSurface: {
      schemaVersion: 1,
      filter: texture.filter,
      wrap: texture.wrap,
      alphaMode: mapping === 'atlas'
        ? { kind: 'mask', cutoff: 0.4 }
        : { kind: 'opaque' },
      mapping: mapping === 'repeat'
        ? { kind: 'repeat', ...common }
        : {
            kind: 'atlas',
            atlas: 'sprite-sheet/voxel-atlas',
            atlasVersion: 1,
            atlasContentHash: 'atlas-hash',
            region: {
              id: 'stone',
              contentMin: [1, 1],
              contentExtent: [2, 2],
              padding: { left: 1, right: 1, bottom: 1, top: 1 },
              inset: 'halfTexel',
            },
            ...common,
          },
    },
  };
}

function voxelTextureDescriptor(
  bytes: Uint8Array,
  width: number,
  height: number,
  version = 1,
  filter: 'nearest' | 'linear' = 'nearest',
  wrap: 'repeat' | 'clamp' = 'repeat',
): import('@rusty-engine/render-contracts').TextureDescriptor {
  const digest = bytesToHex(sha256(bytes));
  const contentHash = `sha256:${digest}`;
  return {
    id: 'texture/checker',
    width,
    height,
    filter,
    wrap,
    contentHash,
    version,
    payload: {
      encoding: 'pngRgba8',
      colorSpace: 'srgb',
      contentHash,
      byteLength: bytes.byteLength,
      source: { kind: 'inline', encodedBytes: [...bytes] },
    },
  };
}

function texturedPlankAsset(): StaticMeshAsset {
  return {
    asset: 'mesh/textured-plank',
    payload: withMaterialSlot({ ...texturedQuadPayload(), provenance: 'staticAsset' }, 0),
    materialSlots: [{ slot: 0, material: 'material/wood' }],
    collision: { kind: 'visualOnly' },
  };
}

function withMaterialSlot(payload: MeshPayloadDescriptor, materialSlot: number): MeshPayloadDescriptor {
  return {
    ...payload,
    groups: payload.groups.map((group) => ({ ...group, materialSlot })),
  };
}

class TestTextureResourceSource implements TextureResourceSource {
  readonly acquired: string[] = [];
  readonly released: string[] = [];

  constructor(readonly bytes: Uint8Array) {}

  acquireResource(resource: string, contentHash: string, byteLength: number): MeshBufferView {
    const expected = textureDescriptor(this.bytes, 1, 'resource').payload!;
    assert.equal(resource, expected.source.kind === 'resource' ? expected.source.resource : null);
    assert.equal(contentHash, expected.contentHash);
    assert.equal(byteLength, this.bytes.byteLength);
    this.acquired.push(resource);
    return { bytes: this.bytes };
  }

  releaseResource(resource: string): void {
    this.released.push(resource);
  }
}

void test('inline and resource PNG textures converge on one generic static-mesh material path', () => {
  const bytes = rgbaPng(2, 1, [255, 0, 0, 255, 0, 255, 0, 128]);
  const source = new TestTextureResourceSource(bytes);
  const inline = new ThreeRenderer();
  const resource = new ThreeRenderer({ textureResourceSource: source });
  for (const [renderer, kind] of [[inline, 'inline'], [resource, 'resource']] as const) {
    renderer.applyFrame({ schemaVersion: 1, ops: [
      { op: 'defineTexture', texture: textureDescriptor(bytes, 1, kind) },
      { op: 'defineMaterial', material: texturedMaterial() },
      { op: 'defineStaticMesh', asset: texturedPlankAsset() },
      {
        op: 'createStaticMeshInstance', handle: renderHandle(301), parent: null,
        instance: crateInstance('mesh/textured-plank'),
      },
      {
        op: 'createStaticMeshInstance', handle: renderHandle(302), parent: null,
        instance: crateInstance('mesh/textured-plank'),
      },
    ] });
    const first = (renderer.objectFor(renderHandle(301)) as THREE.Mesh).material as THREE.MeshStandardMaterial;
    const second = (renderer.objectFor(renderHandle(302)) as THREE.Mesh).material as THREE.MeshStandardMaterial;
    assert.ok(first.map instanceof THREE.DataTexture);
    assert.equal(first.map, second.map, 'instances share one retained texture');
    assert.deepEqual([...((first.map.image as { data: Uint8Array }).data)], [255, 0, 0, 255, 0, 255, 0, 128]);
    assert.equal(first.map.colorSpace, THREE.SRGBColorSpace);
    assert.equal(first.map.magFilter, THREE.NearestFilter);
    assert.equal(first.map.wrapS, THREE.RepeatWrapping);
    assert.equal(renderer.resourceStatistics().textureResourceCount, 1);
    assert.equal(renderer.textureResourceReadout()[0]?.decodedBytes, 8);
  }
  assert.equal(source.acquired.length, 1);
  assert.deepEqual(source.released, source.acquired);
});

void test('generic materials use baked mesh UVs regardless of deprecated strategy metadata', () => {
  const bytes = rgbaPng(2, 1, [255, 0, 0, 255, 0, 255, 0, 255]);
  const expectedUv = [
    [0, 0], [1, 0], [1, 1], [0, 1],
  ];
  for (const uvStrategy of ['flat', 'planar', 'atlas'] as const) {
    const materialId = `material/baked-${uvStrategy}`;
    const assetId = `mesh/baked-${uvStrategy}`;
    const renderer = new ThreeRenderer();
    renderer.applyFrame({ schemaVersion: 1, ops: [
      { op: 'defineTexture', texture: textureDescriptor(bytes) },
      { op: 'defineMaterial', material: { ...texturedMaterial(), id: materialId, uvStrategy } },
      {
        op: 'defineStaticMesh',
        asset: {
          ...texturedPlankAsset(), asset: assetId,
          materialSlots: [{ slot: 0, material: materialId }],
        },
      },
      {
        op: 'createStaticMeshInstance', handle: renderHandle(4299), parent: null,
        instance: crateInstance(assetId),
      },
    ] });
    const mesh = renderer.objectFor(renderHandle(4299)) as THREE.Mesh;
    const uv = mesh.geometry.getAttribute('uv') as THREE.BufferAttribute;
    assert.deepEqual(
      Array.from({ length: uv.count }, (_, index) => [uv.getX(index), uv.getY(index)]),
      expectedUv,
    );
    assert.ok((mesh.material as THREE.MeshStandardMaterial).map instanceof THREE.Texture);
    renderer.dispose();
  }
});

void test('texture redefine is stale-safe and disposes replaced and final GPU resources exactly once', () => {
  const beforeBytes = rgbaPng(2, 1, [255, 0, 0, 255, 0, 255, 0, 255]);
  const afterBytes = rgbaPng(2, 1, [0, 0, 255, 255, 255, 255, 0, 255]);
  const renderer = new ThreeRenderer();
  renderer.applyFrame({ schemaVersion: 1, ops: [
    { op: 'defineTexture', texture: textureDescriptor(beforeBytes) },
    { op: 'defineMaterial', material: texturedMaterial() },
    { op: 'defineStaticMesh', asset: texturedPlankAsset() },
    {
      op: 'createStaticMeshInstance', handle: renderHandle(303), parent: null,
      instance: crateInstance('mesh/textured-plank'),
    },
  ] });
  const mesh = renderer.objectFor(renderHandle(303)) as THREE.Mesh;
  const oldTexture = (mesh.material as THREE.MeshStandardMaterial).map!;
  let oldDisposals = 0;
  oldTexture.addEventListener('dispose', () => { oldDisposals += 1; });

  renderer.applyDiff({ op: 'defineTexture', texture: textureDescriptor(afterBytes, 2) });
  const newTexture = (mesh.material as THREE.MeshStandardMaterial).map!;
  assert.notEqual(newTexture, oldTexture);
  assert.equal(oldDisposals, 1);
  const descriptorBeforeStale = renderer.textureDescriptor('texture/checker');
  assert.throws(
    () => renderer.applyDiff({ op: 'defineTexture', texture: textureDescriptor(beforeBytes, 1) }),
    /stale or duplicate version/u,
  );
  assert.equal((mesh.material as THREE.MeshStandardMaterial).map, newTexture);
  assert.deepEqual(renderer.textureDescriptor('texture/checker'), descriptorBeforeStale);

  let finalDisposals = 0;
  newTexture.addEventListener('dispose', () => { finalDisposals += 1; });
  renderer.dispose();
  renderer.dispose();
  assert.equal(finalDisposals, 1);
  assert.equal(renderer.resourceStatistics().textureResourceCount, 0);
});

void test('sky background flips asymmetric equirectangular content without changing its retained source', () => {
  const beforePixels = [
    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
    16, 32, 48, 0, 64, 80, 96, 0, 112, 128, 144, 0, 160, 176, 192, 0,
  ];
  const afterPixels = [
    0, 0, 255, 255, 255, 255, 0, 255, 255, 0, 255, 255, 0, 255, 255, 255,
    192, 176, 160, 0, 144, 128, 112, 0, 96, 80, 64, 0, 48, 32, 16, 0,
  ];
  const before = rgbaPng(4, 2, beforePixels);
  const after = rgbaPng(4, 2, afterPixels);
  const source = new TestTextureResourceSource(before);
  const renderer = new ThreeRenderer({ textureResourceSource: source });
  const beforeTexture = {
    ...textureDescriptor(before, 1, 'resource'), width: 4, height: 2, wrap: 'clamp' as const,
  };
  const beforeResource = beforeTexture.payload?.source;
  assert.equal(beforeResource?.kind, 'resource');
  renderer.applyFrame({ schemaVersion: 1, ops: [
    { op: 'defineTexture', texture: beforeTexture },
    { op: 'setSkyBackground', background: { texture: beforeTexture.id } },
  ] });
  assert.deepEqual(renderer.skyBackgroundReadout(), {
    textureId: beforeTexture.id,
    contentHash: beforeTexture.contentHash,
    resource: beforeResource.resource,
  });
  assert.ok(Object.isFrozen(renderer.skyBackgroundReadout()));
  const retainedBefore = renderer.textureObjectFor(beforeTexture.id);
  const firstBackground = renderer.scene.background;
  assert.ok(retainedBefore instanceof THREE.DataTexture);
  assert.ok(firstBackground instanceof THREE.Texture);
  assert.notEqual(firstBackground, retainedBefore, 'the sky owns a clone of the retained source');
  assert.equal(firstBackground.mapping, THREE.EquirectangularReflectionMapping);
  assert.equal(retainedBefore.flipY, false, 'ordinary retained texture sampling keeps decoded PNG row order');
  assert.equal(firstBackground.flipY, true, 'equirectangular sampling maps the authored opaque upper row above its transparent edge');
  assert.deepEqual(
    [...((retainedBefore.image as { data: Uint8Array }).data)],
    beforePixels,
    'the retained source stays vertically unmodified',
  );
  assert.deepEqual(
    [...((firstBackground.image as { data: Uint8Array }).data)],
    beforePixels,
    'the cloned sky preserves distinct opaque upper and transparent lower rows',
  );
  assert.equal(renderer.resourceStatistics().textureResourceCount, 2);

  const afterTexture = {
    ...textureDescriptor(after, 2), width: 4, height: 2, wrap: 'clamp' as const,
  };
  let firstDisposals = 0;
  firstBackground.addEventListener('dispose', () => { firstDisposals += 1; });
  renderer.applyDiff({ op: 'defineTexture', texture: afterTexture });
  assert.ok(renderer.scene.background instanceof THREE.Texture);
  assert.notEqual(renderer.scene.background, firstBackground);
  assert.equal(renderer.textureObjectFor(afterTexture.id)?.flipY, false, 'replacement leaves its retained source unchanged');
  assert.equal(renderer.scene.background.flipY, true, 'replacement corrects equirectangular orientation too');
  assert.equal(firstDisposals, 1);

  const finalBackground = renderer.scene.background as THREE.Texture;
  let finalDisposals = 0;
  finalBackground.addEventListener('dispose', () => { finalDisposals += 1; });
  renderer.applyDiff({ op: 'setSkyBackground', background: null });
  assert.equal(renderer.scene.background, null);
  assert.deepEqual(renderer.skyBackgroundReadout(), {
    textureId: null,
    contentHash: null,
    resource: null,
  });
  assert.equal(finalDisposals, 1);
  assert.equal(renderer.resourceStatistics().textureResourceCount, 1);
  renderer.dispose();
  assert.equal(renderer.resourceStatistics().textureResourceCount, 0);
});

void test('sky background rejects missing, metadata-only, non-sRGB, repeated, and non-2:1 textures atomically', () => {
  const renderer = new ThreeRenderer();
  assert.throws(
    () => renderer.applyDiff({
      op: 'setSkyBackground', background: { texture: 'texture/missing' },
    }),
    /not a retained payload|is not retained/u,
  );
  assert.equal(renderer.scene.background, null);

  const bytes = rgbaPng(2, 1, [255, 0, 0, 255, 0, 255, 0, 255]);
  const metadataOnly = {
    id: 'texture/metadata-only-sky',
    width: 2,
    height: 1,
    filter: 'nearest' as const,
    wrap: 'clamp' as const,
    contentHash: null,
    version: 1,
  };
  assert.throws(
    () => renderer.applyFrame({ schemaVersion: 1, ops: [
      { op: 'defineTexture', texture: metadataOnly },
      { op: 'setSkyBackground', background: { texture: metadataOnly.id } },
    ] }),
    /not a retained payload|is not retained/u,
  );
  assert.equal(renderer.textureDescriptor(metadataOnly.id), undefined);
  const squareBytes = rgbaPng(1, 1, [255, 0, 0, 255]);
  for (const [texture, error] of [
    [{ ...textureDescriptor(bytes, 1, 'inline', 'texture/linear', 'linear'), wrap: 'clamp' as const }, /sRGB/u],
    [{ ...textureDescriptor(bytes, 1, 'inline', 'texture/repeat'), wrap: 'repeat' as const }, /clamp/u],
    [{ ...textureDescriptor(squareBytes, 1, 'inline', 'texture/square'), width: 1, height: 1 }, /2:1/u],
  ] as const) {
    assert.throws(
      () => renderer.applyFrame({ schemaVersion: 1, ops: [
        { op: 'defineTexture', texture },
        { op: 'setSkyBackground', background: { texture: texture.id } },
      ] }),
      error,
    );
    assert.equal(renderer.textureDescriptor(texture.id), undefined);
    assert.equal(renderer.scene.background, null);
  }
  const validSky = { ...textureDescriptor(bytes, 2, 'inline', 'texture/sky-valid'), wrap: 'clamp' as const };
  renderer.applyFrame({ schemaVersion: 1, ops: [
    { op: 'defineTexture', texture: validSky },
    { op: 'setSkyBackground', background: { texture: validSky.id } },
  ] });
  assert.notEqual(renderer.scene.background, null, 'a later valid sky remains admissible');
  renderer.dispose();
});

void test('voxel surface specializes one greedy quad for repeat and atlas-safe sampling', () => {
  const pixels = Array.from({ length: 4 * 4 }, (_, index) => [
    index * 11 % 255,
    index * 23 % 255,
    index * 37 % 255,
    255,
  ]).flat();
  const bytes = rgbaPng(4, 4, pixels);
  const texture = voxelTextureDescriptor(bytes, 4, 4, 1, 'linear', 'clamp');
  const material = voxelTexturedMaterial(texture, 'atlas');
  const renderer = new ThreeRenderer();
  renderer.applyFrame({ schemaVersion: 1, ops: [
    { op: 'defineTexture', texture },
    { op: 'defineMaterial', material },
    {
      op: 'defineStaticMesh',
      asset: {
        asset: 'mesh/textured-plank',
        payload: withMaterialSlot({ ...texturedQuadPayload(), provenance: 'voxelChunk' }, 0),
        materialSlots: [{ slot: 0, material: material.id }],
        collision: { kind: 'visualOnly' },
      },
    },
    {
      op: 'createStaticMeshInstance', handle: renderHandle(320), parent: null,
      instance: crateInstance('mesh/textured-plank'),
    },
  ] });

  const mesh = renderer.objectFor(renderHandle(320)) as THREE.Mesh;
  const geometry = mesh.geometry as THREE.BufferGeometry;
  const realized = mesh.material as THREE.MeshStandardMaterial;
  assert.equal(geometry.getAttribute('position').count, 4, 'greedy quad remains four vertices');
  assert.equal(geometry.index?.count, 6, 'greedy quad remains two triangles');
  assert.deepEqual(renderer.voxelSurfaceMaterialReadout(), [{
    material: 'material/voxel-atlas',
    texture: 'texture/checker',
    mapping: 'atlas',
    tileScaleCells: [1, 1],
    tileOriginCells: [-4, 8],
    sampleUvMin: [0.375, 0.375],
    sampleUvMax: [0.625, 0.625],
    alphaMode: 'mask',
    alphaCutoff: 0.4,
  }]);
  assert.equal(realized.alphaTest, 0.4);
  assert.equal(realized.transparent, false);
  const shader = {
    uniforms: {} as Record<string, { value: unknown }>,
    vertexShader: '#include <uv_pars_vertex>\nvoid main(){#include <uv_vertex>}',
    fragmentShader: '#include <map_pars_fragment>\nvoid main(){#include <map_fragment>}',
  };
  realized.onBeforeCompile(shader as never, {} as never);
  assert.match(shader.fragmentShader, /fract\(\(vMapUv/u);
  assert.match(shader.fragmentShader, /mix\(rustyVoxelUvMin, rustyVoxelUvMax/u);
  assert.deepEqual(
    (shader.uniforms['rustyVoxelUvMin']?.value as THREE.Vector2).toArray(),
    [0.375, 0.375],
  );
});

void test('uploaded voxel mesh realizes its retained voxel-surface texture', () => {
  const bytes = rgbaPng(4, 4, Array.from({ length: 4 * 4 }, () => [80, 160, 60, 255]).flat());
  const texture = voxelTextureDescriptor(bytes, 4, 4, 1, 'nearest', 'clamp');
  const material = { ...voxelTexturedMaterial(texture, 'atlas'), id: 'voxel-material/1' };
  const renderer = new ThreeRenderer();
  const handle = renderHandle(322);

  renderer.applyFrame({ schemaVersion: 1, ops: [
    { op: 'defineTexture', texture },
    { op: 'defineMaterial', material },
    { op: 'create', handle, parent: null, node: meshNode() },
    {
      op: 'replaceMeshPayload',
      handle,
      payload: withMaterialSlot({ ...texturedQuadPayload(), provenance: 'voxelChunk' }, 1),
    },
  ] });

  const realized = (renderer.objectFor(handle) as THREE.Mesh).material;
  const materials = Array.isArray(realized) ? realized : [realized];
  assert.equal(materials.length, 2);
  for (const entry of materials as THREE.MeshStandardMaterial[]) {
    assert.ok(entry.map instanceof THREE.Texture);
    assert.equal(entry.userData['rustyVoxelSurface']?.material, 'voxel-material/1');
    assert.equal(entry.userData['rustyVoxelSurface']?.mapping, 'atlas');
  }
});

void test('voxel texture and material redefine is final-frame atomic without remeshing', () => {
  const beforeBytes = rgbaPng(2, 1, [255, 0, 0, 255, 0, 255, 0, 255]);
  const afterBytes = rgbaPng(2, 1, [0, 0, 255, 255, 255, 255, 0, 255]);
  const beforeTexture = voxelTextureDescriptor(beforeBytes, 2, 1);
  const afterTexture = voxelTextureDescriptor(afterBytes, 2, 1, 2);
  const renderer = new ThreeRenderer();
  renderer.applyFrame({ schemaVersion: 1, ops: [
    { op: 'defineTexture', texture: beforeTexture },
    { op: 'defineMaterial', material: voxelTexturedMaterial(beforeTexture) },
    {
      op: 'defineStaticMesh',
      asset: {
        asset: 'mesh/textured-plank',
        payload: withMaterialSlot({ ...texturedQuadPayload(), provenance: 'voxelChunk' }, 0),
        materialSlots: [{ slot: 0, material: 'material/voxel-repeat' }],
        collision: { kind: 'visualOnly' },
      },
    },
    {
      op: 'createStaticMeshInstance', handle: renderHandle(321), parent: null,
      instance: crateInstance('mesh/textured-plank'),
    },
  ] });
  const mesh = renderer.objectFor(renderHandle(321)) as THREE.Mesh;
  const geometry = mesh.geometry;
  const oldMaterial = mesh.material;
  const oldTexture = (oldMaterial as THREE.MeshStandardMaterial).map;
  const beforeStats = renderer.resourceStatistics();
  const beforeReadout = renderer.voxelSurfaceMaterialReadout();

  assert.throws(
    () => renderer.applyDiff({ op: 'defineTexture', texture: afterTexture }),
    /needs texture texture\/checker version 1/u,
  );
  assert.equal(mesh.geometry, geometry);
  assert.equal(mesh.material, oldMaterial);
  assert.equal((mesh.material as THREE.MeshStandardMaterial).map, oldTexture);
  assert.deepEqual(renderer.resourceStatistics(), beforeStats);
  assert.deepEqual(renderer.voxelSurfaceMaterialReadout(), beforeReadout);

  renderer.applyFrame({ schemaVersion: 1, ops: [
    { op: 'defineTexture', texture: afterTexture },
    { op: 'defineMaterial', material: voxelTexturedMaterial(afterTexture) },
  ] });
  assert.equal(mesh.geometry, geometry, 'material replacement does not remesh');
  assert.notEqual(mesh.material, oldMaterial);
  assert.equal(geometry.getAttribute('position').count, 4);
  assert.equal(renderer.resourceStatistics().geometryResourceCount, beforeStats.geometryResourceCount);
});

void test('retained texture budget accepts every exact limit and rejects each one-over prospectively', () => {
  const exact = {
    count: RUSTY_RENDERER_TEXTURE_MAX_RETAINED - 1,
    encodedBytes: RUSTY_RENDERER_TEXTURE_MAX_ENCODED_BYTES - 1,
    decodedBytes: RUSTY_RENDERER_TEXTURE_MAX_DECODED_BYTES - 4,
  };
  assert.deepEqual(
    admitRendererTextureResourceBudget(exact, undefined, { encodedBytes: 1, decodedBytes: 4 }),
    {
      count: RUSTY_RENDERER_TEXTURE_MAX_RETAINED,
      encodedBytes: RUSTY_RENDERER_TEXTURE_MAX_ENCODED_BYTES,
      decodedBytes: RUSTY_RENDERER_TEXTURE_MAX_DECODED_BYTES,
    },
  );
  assert.throws(
    () => admitRendererTextureResourceBudget(
      { ...exact, count: RUSTY_RENDERER_TEXTURE_MAX_RETAINED },
      undefined,
      { encodedBytes: 1, decodedBytes: 4 },
    ),
    /retained texture quota exceeded/u,
  );
  assert.throws(
    () => admitRendererTextureResourceBudget(
      { ...exact, encodedBytes: RUSTY_RENDERER_TEXTURE_MAX_ENCODED_BYTES },
      undefined,
      { encodedBytes: 1, decodedBytes: 4 },
    ),
    /aggregate encoded texture byte quota exceeded/u,
  );
  assert.throws(
    () => admitRendererTextureResourceBudget(
      { ...exact, decodedBytes: RUSTY_RENDERER_TEXTURE_MAX_DECODED_BYTES - 3 },
      undefined,
      { encodedBytes: 1, decodedBytes: 4 },
    ),
    /aggregate decoded texture byte quota exceeded/u,
  );
});

void test('malformed texture bytes reject the complete frame and release the resource borrow', () => {
  const expected = rgbaPng(2, 1, [255, 0, 0, 255, 0, 255, 0, 255]);
  const corrupt = expected.slice();
  corrupt[corrupt.length - 1] = (corrupt[corrupt.length - 1] ?? 0) ^ 1;
  const released: string[] = [];
  const source: TextureResourceSource = {
    acquireResource: () => ({ bytes: corrupt }),
    releaseResource: (resource) => { released.push(resource); },
  };
  const renderer = new ThreeRenderer({ textureResourceSource: source });
  renderer.applyDiff(createDiff(1, cubeNode('stable')));
  const before = renderer.snapshot();
  const descriptor = textureDescriptor(expected, 1, 'resource');
  assert.throws(
    () => renderer.applyFrame({ schemaVersion: 1, ops: [
      createDiff(2, cubeNode('must-not-commit')),
      { op: 'defineTexture', texture: descriptor },
    ] }),
    /content hash mismatch/u,
  );
  assert.equal(renderer.snapshot(), before);
  assert.equal(renderer.has(renderHandle(2)), false);
  assert.equal(renderer.textureDescriptor(descriptor.id), undefined);
  assert.deepEqual(released, [
    descriptor.payload?.source.kind === 'resource' ? descriptor.payload.source.resource : '',
  ]);
});

void test('defineMaterial maps a static-mesh slot to its defined colour, not a placeholder', () => {
  const r = new ThreeRenderer();
  r.applyDiff({ op: 'defineMaterial', material: woodMaterial() });
  assert.deepEqual(r.materialDescriptor('material/wood')?.color, [0.6, 0.4, 0.2, 1]);

  // Define a single-slot mesh bound to material/wood, then instance it.
  r.applyDiff({
    op: 'defineStaticMesh',
    asset: {
      asset: 'mesh/plank',
      payload: withMaterialSlot({ ...quadPayload(), provenance: 'staticAsset' }, 0),
      materialSlots: [{ slot: 0, material: 'material/wood' }],
      collision: { kind: 'visualOnly' },
    },
  });
  r.applyDiff({
    op: 'createStaticMeshInstance',
    handle: renderHandle(1),
    parent: null,
    instance: crateInstance('mesh/plank'),
  });

  const mat = (r.objectFor(renderHandle(1)) as THREE.Mesh).material as THREE.MeshStandardMaterial;
  // The defined wood colour (0.6,0.4,0.2), not the deterministic per-slot hue.
  assert.ok(Math.abs(mat.color.r - 0.6) < 1e-6 && Math.abs(mat.color.b - 0.2) < 1e-6);
  assert.equal(r.fallbackMaterialCount, 0, 'a defined material is not a fallback');
});

void test('a slot with no material descriptor falls back deterministically and is counted', () => {
  const r = new ThreeRenderer();
  r.applyDiff({ op: 'defineStaticMesh', asset: crateAsset() }); // two slots, no descriptors
  r.applyDiff({
    op: 'createStaticMeshInstance',
    handle: renderHandle(1),
    parent: null,
    instance: crateInstance(),
  });
  assert.equal(r.fallbackMaterialCount, 2, 'both unresolved slots count as fallbacks');
});

void test('two voxel materials project to distinct retained render descriptors', () => {
  // The fixture is generated by render-bridge's project_voxel_materials from a
  // A voxel material table maps compact u16 ids to retained material assets.
  const fixture: unknown = JSON.parse(
    readFileSync(
      resolve(import.meta.dirname, '../../../../fixtures/render/voxel-materials-v1.json'),
      'utf8',
    ),
  );
  const r = new ThreeRenderer();
  r.applyEncodedFrame(fixture);
  const stone = r.materialDescriptor('voxel-material/1');
  const dirt = r.materialDescriptor('voxel-material/2');
  assert.ok(stone && dirt, 'both voxel materials register a descriptor');
  assert.notDeepEqual(stone!.color, dirt!.color, 'distinct retained styles');
  // Visual projection only — the descriptor has no collision field.
  assert.ok(!('structuralClass' in stone!) && !('collidable' in stone!));
});

// ── Material update lifecycle and fallback diagnostics ────────────────────────

void test('redefining a material live-replaces instance materials and disposes the old', () => {
  const r = new ThreeRenderer();
  r.applyDiff({ op: 'defineMaterial', material: woodMaterial() });
  r.applyDiff({
    op: 'defineStaticMesh',
    asset: {
      asset: 'mesh/plank',
      payload: withMaterialSlot({ ...quadPayload(), provenance: 'staticAsset' }, 0),
      materialSlots: [{ slot: 0, material: 'material/wood' }],
      collision: { kind: 'visualOnly' },
    },
  });
  r.applyDiff({
    op: 'createStaticMeshInstance',
    handle: renderHandle(1),
    parent: null,
    instance: crateInstance('mesh/plank'),
  });

  const before = (r.objectFor(renderHandle(1)) as THREE.Mesh).material as THREE.MeshStandardMaterial;
  assert.ok(Math.abs(before.color.r - 0.6) < 1e-6);
  let disposed = false;
  before.addEventListener('dispose', () => {
    disposed = true;
  });

  // A visual-only redefine (new colour) applies live, deterministically.
  r.applyDiff({
    op: 'defineMaterial',
    material: { ...woodMaterial(), color: [0.1, 0.8, 0.2, 1] },
  });
  const after = (r.objectFor(renderHandle(1)) as THREE.Mesh).material as THREE.MeshStandardMaterial;
  assert.ok(Math.abs(after.color.g - 0.8) < 1e-6, 'rendered colour updated live');
  assert.ok(disposed, 'the old material was disposed (leak-safe)');
});

void test('material feedback updates one instance without duplicating its asset or handle', () => {
  const r = new ThreeRenderer();
  r.applyDiff({ op: 'defineMaterial', material: woodMaterial() });
  r.applyDiff({ op: 'defineStaticMesh', asset: crateAsset() });
  r.applyDiff({ op: 'createStaticMeshInstance', handle: renderHandle(1), parent: null, instance: crateInstance() });
  r.applyDiff({ op: 'createStaticMeshInstance', handle: renderHandle(2), parent: null, instance: crateInstance() });

  const normal = r.objectFor(renderHandle(1)) as THREE.Mesh;
  const warning = r.objectFor(renderHandle(2)) as THREE.Mesh;
  const normalBefore = normal.material as THREE.MeshStandardMaterial[];
  const warningBefore = warning.material as THREE.MeshStandardMaterial[];
  assert.ok(normalBefore[0] instanceof THREE.MeshStandardMaterial);
  assert.equal(normalBefore[0], warningBefore[0], 'descriptor material begins shared');
  assert.equal(normal.geometry, warning.geometry, 'asset geometry begins shared');

  r.applyDiff({
    op: 'setMaterialInstanceParameters',
    handle: renderHandle(2),
    slot: 1,
    parameters: {
      textureTint: [0.2, 1, 0.2, 1],
      emissionColor: [1, 0.08, 0],
      emissionIntensity: 2.5,
    },
  });

  const warningActive = (warning.material as THREE.MeshStandardMaterial[])[0]!;
  assert.equal(r.objectFor(renderHandle(2)), warning, 'retained handle object is unchanged');
  assert.equal(normal.geometry, warning.geometry, 'feedback does not duplicate asset geometry');
  assert.notEqual((normal.material as THREE.Material[])[0], warningActive, 'only target slot is cloned');
  assert.ok(Math.abs(warningActive.color.r - 0.12) < 1e-6);
  assert.ok(Math.abs(warningActive.color.g - 0.4) < 1e-6);
  assert.deepEqual(warningActive.emissive.toArray(), [1, 0.08, 0]);
  assert.equal(warningActive.emissiveIntensity, 2.5);

  let disposed = false;
  warningActive.addEventListener('dispose', () => { disposed = true; });
  r.applyDiff({
    op: 'setMaterialInstanceParameters',
    handle: renderHandle(2),
    slot: 1,
    parameters: null,
  });
  assert.ok(disposed, 'reset disposes the instance-owned material');
  assert.equal(
    (warning.material as THREE.Material[])[0],
    (normal.material as THREE.Material[])[0],
    'reset returns to the shared descriptor material',
  );
});

void test('material feedback rejects stale handles, non-mesh handles, and unbound slots', () => {
  const r = new ThreeRenderer();
  const parameters = {
    textureTint: [1, 1, 1, 1] as const,
    emissionColor: [1, 0, 0] as const,
    emissionIntensity: 1,
  };
  assert.throws(
    () => r.applyDiff({ op: 'setMaterialInstanceParameters', handle: renderHandle(99), slot: 1, parameters }),
    /unknown handle 99/,
  );
  r.applyDiff({ op: 'defineStaticMesh', asset: crateAsset() });
  r.applyDiff({ op: 'createStaticMeshInstance', handle: renderHandle(1), parent: null, instance: crateInstance() });
  assert.throws(
    () => r.applyDiff({ op: 'setMaterialInstanceParameters', handle: renderHandle(1), slot: 9, parameters }),
    /unbound slot 9/,
  );
});

void test('fallback material use is visible in diagnostics with the material id', () => {
  const r = new ThreeRenderer();
  // No defineMaterial for the crate's slots → both fall back.
  r.applyDiff({ op: 'defineStaticMesh', asset: crateAsset() });
  r.applyDiff({
    op: 'createStaticMeshInstance',
    handle: renderHandle(1),
    parent: null,
    instance: crateInstance(),
  });
  assert.deepEqual(r.fallbackMaterials(), ['material/iron', 'material/wood']);
  assert.equal(r.fallbackMaterialCount, 2);
});

// ── Textures and sprite atlases ────────────────────────────────────────────────

import type { SpriteAtlasDescriptor, TextureDescriptor } from '@rusty-engine/render-contracts';

function sparkTexture(): TextureDescriptor {
  return {
    id: 'texture/spark',
    width: 64,
    height: 32,
    filter: 'nearest',
    wrap: 'clamp',
    contentHash: null,
    version: 1,
  };
}

function sparkAtlas(): SpriteAtlasDescriptor {
  return {
    id: 'sprite/spark-sheet',
    texture: 'texture/spark',
    frames: [
      { frame: 0, uvMin: [0, 0], uvMax: [0.5, 1] },
      { frame: 3, uvMin: [0.5, 0], uvMax: [1, 1], size: [2, 3] },
    ],
  };
}

function atlasSprite(frame = 0): SpriteInstanceDescriptor {
  return {
    asset: 'sprite/spark-sheet',
    frame,
    pivot: [0.5, 0.5],
    size: [1, 1],
    sizeMode: 'world',
    billboard: 'spherical',
    tint: [1, 1, 1, 1],
    renderOrder: 0,
    depth: 'default',
    shading: 'unlit',
    visible: true,
    transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
    attachment: { sourceEntity: null, sourceSceneNode: 10, attachmentPoint: null },
    metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label: 'spark' },
  };
}

function spriteUv(r: ThreeRenderer, handle: number): number[] {
  return (r.objectFor(renderHandle(handle))!.userData['uv'] as number[]).map((x) => Number(x.toFixed(4)));
}

function spriteGeometryUv(r: ThreeRenderer, handle: number): number[][] {
  const mesh = r.objectFor(renderHandle(handle)) as THREE.Mesh;
  const uv = mesh.geometry.getAttribute('uv') as THREE.BufferAttribute;
  return Array.from({ length: uv.count }, (_, index) => [uv.getX(index), uv.getY(index)]);
}

void test('a sprite frame maps to its atlas UV sub-rectangle deterministically', () => {
  const r = new ThreeRenderer();
  r.applyDiff({ op: 'defineTexture', texture: sparkTexture() });
  r.applyDiff({ op: 'defineSpriteAtlas', atlas: sparkAtlas() });
  assert.equal(r.textureDescriptor('texture/spark')?.width, 64);
  assert.equal(r.spriteAtlas('sprite/spark-sheet')?.frames.length, 2);

  r.applyDiff({ op: 'createSprite', handle: renderHandle(1), parent: null, sprite: atlasSprite(0) });
  assert.deepEqual(spriteUv(r, 1), [0, 0, 0.5, 1], 'frame 0 → left half');
  assert.deepEqual(spriteGeometryUv(r, 1), [
    [0, 0], [0.5, 0], [0, 1], [0.5, 1],
  ], 'decoded PNG top maps to the sprite top');

  // Advancing the frame re-resolves the UV rect deterministically.
  r.applyDiff({ op: 'updateSprite', handle: renderHandle(1), frame: 3, tint: null, renderOrder: null, visible: null });
  assert.deepEqual(spriteUv(r, 1), [0.5, 0, 1, 1], 'frame 3 → right half');
  const mesh = r.objectFor(renderHandle(1)) as THREE.Mesh;
  mesh.geometry.computeBoundingBox();
  assert.deepEqual(mesh.geometry.boundingBox?.getSize(new THREE.Vector3()).toArray(), [2, 3, 0]);
  assert.equal(r.resourceStatistics().geometryResourceCount, 1, 'replaced frame geometry is disposed');
  assert.equal(r.spriteFallbackCount, 0, 'known frames are not fallbacks');
});

void test('multi-row sprite atlases select top-left image-space rows without vertical reversal', () => {
  const r = new ThreeRenderer();
  r.applyDiff({ op: 'defineTexture', texture: sparkTexture() });
  r.applyDiff({
    op: 'defineSpriteAtlas',
    atlas: {
      id: 'sprite/row-sheet',
      texture: 'texture/spark',
      frames: [
        { frame: 0, uvMin: [0, 0], uvMax: [1, 0.5] },
        { frame: 1, uvMin: [0, 0.5], uvMax: [1, 1] },
      ],
    },
  });
  r.applyDiff({
    op: 'createSprite',
    handle: renderHandle(1),
    parent: null,
    sprite: { ...atlasSprite(0), asset: 'sprite/row-sheet' },
  });
  assert.deepEqual(spriteGeometryUv(r, 1), [
    [0, 0], [1, 0], [0, 0.5], [1, 0.5],
  ], 'frame zero selects the upright top image row');

  r.applyDiff({
    op: 'updateSprite',
    handle: renderHandle(1),
    frame: 1,
    tint: null,
    renderOrder: null,
    visible: null,
  });
  assert.deepEqual(spriteGeometryUv(r, 1), [
    [0, 0.5], [1, 0.5], [0, 1], [1, 1],
  ], 'frame one selects the upright bottom image row');
});

void test('a sprite frame with no atlas frame falls back to full UVs and is counted', () => {
  const r = new ThreeRenderer();
  r.applyDiff({ op: 'defineTexture', texture: sparkTexture() });
  r.applyDiff({ op: 'defineSpriteAtlas', atlas: sparkAtlas() });
  r.applyDiff({ op: 'createSprite', handle: renderHandle(1), parent: null, sprite: atlasSprite(9) });
  assert.deepEqual(spriteUv(r, 1), [0, 0, 1, 1], 'unknown frame → full UVs');
  assert.equal(r.spriteFallbackCount, 1);
});

void test('sprite atlases bind retained PNG textures and refresh live sprites on replacement', () => {
  const beforeBytes = rgbaPng(2, 1, [255, 0, 0, 255, 0, 255, 0, 128]);
  const afterBytes = rgbaPng(2, 1, [0, 0, 255, 255, 255, 255, 0, 255]);
  const atlas: SpriteAtlasDescriptor = {
    id: 'sprite/textured-spark-sheet',
    texture: 'texture/checker',
    frames: [{ frame: 0, uvMin: [0, 0], uvMax: [0.5, 1] }],
  };
  const sprite = atlasSprite(0);
  const renderer = new ThreeRenderer();
  renderer.applyFrame({ schemaVersion: 1, ops: [
    { op: 'defineTexture', texture: textureDescriptor(beforeBytes) },
    { op: 'defineSpriteAtlas', atlas },
    { op: 'createSprite', handle: renderHandle(2), parent: null, sprite: { ...sprite, asset: atlas.id } },
  ] });

  const mesh = renderer.objectFor(renderHandle(2)) as THREE.Mesh;
  const material = mesh.material as THREE.MeshBasicMaterial;
  assert.ok(material.map instanceof THREE.DataTexture);
  assert.equal(material.transparent, true, 'texture alpha must remain observable with an opaque tint');
  assert.equal(material.depthWrite, true, 'legacy textured sprites preserve default depth writes');
  assert.deepEqual(
    [...((material.map.image as { data: Uint8Array }).data)],
    [255, 0, 0, 255, 0, 255, 0, 128],
  );
  assert.deepEqual(spriteUv(renderer, 2), [0, 0, 0.5, 1]);

  const previousTexture = material.map;
  renderer.applyDiff({ op: 'defineTexture', texture: textureDescriptor(afterBytes, 2) });
  const replaced = (renderer.objectFor(renderHandle(2)) as THREE.Mesh).material as THREE.MeshBasicMaterial;
  assert.notEqual(replaced.map, previousTexture, 'live sprites must follow retained texture replacement');
  assert.deepEqual(
    [...((replaced.map as THREE.DataTexture).image as { data: Uint8Array }).data],
    [0, 0, 255, 255, 255, 255, 0, 255],
  );

  const lateAtlasRenderer = new ThreeRenderer();
  lateAtlasRenderer.applyFrame({ schemaVersion: 1, ops: [
    { op: 'defineTexture', texture: textureDescriptor(beforeBytes) },
    { op: 'createSprite', handle: renderHandle(3), parent: null, sprite: { ...sprite, asset: atlas.id } },
  ] });
  assert.equal(
    (lateAtlasRenderer.objectFor(renderHandle(3)) as THREE.Mesh).material instanceof THREE.MeshBasicMaterial,
    true,
  );
  assert.deepEqual(spriteUv(lateAtlasRenderer, 3), [0, 0, 1, 1]);
  lateAtlasRenderer.applyDiff({ op: 'defineSpriteAtlas', atlas });
  const lateAtlasMesh = lateAtlasRenderer.objectFor(renderHandle(3)) as THREE.Mesh;
  assert.ok((lateAtlasMesh.material as THREE.MeshBasicMaterial).map instanceof THREE.DataTexture);
  assert.deepEqual(spriteUv(lateAtlasRenderer, 3), [0, 0, 0.5, 1]);
});

void test('retained sprites realize bounded authored normals, alpha, shadows, and linear texture data', () => {
  const colorBytes = rgbaPng(2, 1, [255, 90, 40, 255, 60, 140, 255, 0]);
  const normalBytes = rgbaPng(2, 1, [128, 128, 255, 255, 180, 128, 220, 255]);
  const color = textureDescriptor(colorBytes, 1, 'inline', 'texture/sprite-color', 'srgb');
  const normal = textureDescriptor(normalBytes, 1, 'inline', 'texture/sprite-normal', 'linear');
  const atlas: SpriteAtlasDescriptor = {
    id: 'sprite/lit-test',
    texture: color.id,
    frames: [{ frame: 0, uvMin: [0, 0], uvMax: [1, 1] }],
  };
  const sprite: SpriteInstanceDescriptor = {
    ...atlasSprite(),
    asset: atlas.id,
    material: {
      lighting: 'authoredNormal',
      normalTexture: normal.id,
      depthTexture: null,
      normalStrength: 1.25,
      normalBias: 0,
      alpha: { kind: 'mask', cutoff: 0.4 },
      shadow: 'castAndReceive',
    },
  };
  const renderer = new ThreeRenderer({ shadowsEnabled: true });
  renderer.applyFrame({ schemaVersion: 1, ops: [
    { op: 'defineTexture', texture: color },
    { op: 'defineTexture', texture: normal },
    { op: 'defineSpriteAtlas', atlas },
    { op: 'createSprite', handle: renderHandle(31), parent: null, sprite },
  ] });

  const mesh = renderer.objectFor(renderHandle(31)) as THREE.Mesh;
  const material = mesh.material as THREE.MeshStandardMaterial;
  assert.ok(material instanceof THREE.MeshStandardMaterial);
  assert.equal(material.map?.colorSpace, THREE.SRGBColorSpace);
  assert.equal(material.normalMap?.colorSpace, THREE.NoColorSpace);
  assert.equal(material.alphaTest, 0.4);
  assert.equal(material.transparent, false);
  assert.equal(mesh.castShadow, true);
  assert.equal(mesh.receiveShadow, true);
});

void test('invalid sprite lighting texture rejects a complete frame before retained mutation', () => {
  const bytes = rgbaPng(2, 1, [128, 128, 255, 255, 128, 128, 255, 255]);
  const invalidNormal = textureDescriptor(bytes, 1, 'inline', 'texture/sprite-normal', 'srgb');
  const renderer = new ThreeRenderer();
  assert.throws(() => renderer.applyFrame({ schemaVersion: 1, ops: [
    { op: 'defineTexture', texture: invalidNormal },
    createDiff(44, cubeNode('must-not-commit')),
    {
      op: 'createSprite', handle: renderHandle(45), parent: null,
      sprite: sparkSprite({
        material: {
          lighting: 'authoredNormal', normalTexture: invalidNormal.id, depthTexture: null,
          normalStrength: 1, normalBias: 0, alpha: { kind: 'blend' }, shadow: 'none',
        },
      }),
    },
  ] }), /must use linear color space/u);
  assert.equal(renderer.handleCount, 0);
  assert.equal(renderer.textureDescriptor(invalidNormal.id), undefined);
  assert.equal(renderer.resourceStatistics().textureResourceCount, 0);
  renderer.applyFrame({ schemaVersion: 1, ops: [createDiff(46, cubeNode('valid-after-rejection'))] });
  assert.equal(renderer.handleCount, 1, 'a later valid sprite-adjacent frame remains admissible');
});

void test('instance of an undefined asset, and redefine while in use, are classified errors', () => {
  const r = new ThreeRenderer();
  assert.throws(
    () => r.applyDiff({ op: 'createStaticMeshInstance', handle: renderHandle(1), parent: null, instance: crateInstance() }),
    RenderApplyError,
  );
  r.applyDiff({ op: 'defineStaticMesh', asset: crateAsset() });
  r.applyDiff({ op: 'createStaticMeshInstance', handle: renderHandle(1), parent: null, instance: crateInstance() });
  assert.throws(() => r.applyDiff({ op: 'defineStaticMesh', asset: crateAsset() }), RenderApplyError);
});

function animatedMeshAsset(over: Partial<AnimatedMeshAsset> = {}): AnimatedMeshAsset {
  return {
    asset: 'mesh-animation/kenney-retro-character-medium',
    runtimeFormat: 'glb',
    contentHash: 'sha256:c71255a41c0373f0d2ef52593369d5fd9d2f6220ae548aff8cd6bf5edb403674',
    clips: [
      { id: 'idle', name: 'idle', durationSeconds: 1.04166662693024 },
      { id: 'run', name: 'run', durationSeconds: 0.666666686534882 },
    ],
    defaultClip: 'idle',
    materialSlots: [],
    bounds: { min: [-0.5, 0, -0.5], max: [0.5, 1.8, 0.5] },
    ...over,
  };
}

void test('one admitted compatible clip pack serves independent instances with origin-qualified clips', () => {
  const provenanceHash = `sha256:${'a'.repeat(64)}`;
  const target = rigScene(true);
  const packScene = rigScene(false);
  const hash = animationRigFingerprint(target);
  const rig = {
    joints: [{ id: 'Root', parent: null }], bindRestHash: hash,
    bindRestConvention: 'localMatrixV1' as const, rootConvention: 'inPlace' as const, rootJointId: 'Root',
    structuralRootIds: ['Root'], designatedMotionRootIds: ['Root'], authoredPoseTranslationJointIds: [],
  };
  const pack = {
    asset: 'animation-clip-pack/fixture', runtimeFormat: 'glb' as const, contentHash: hash, rig,
    clips: [{ id: 'wave', name: 'Wave', durationSeconds: 1 }],
    provenance: { producer: 'fixture', sourceHash: provenanceHash, targetHash: provenanceHash, license: 'CC0-1.0' },
  };
  const asset = animatedMeshAsset({ clips: [], defaultClip: null, clipPacks: [pack] });
  const source = new MapAnimatedMeshAssetSource(
    [{ asset: asset.asset, contentHash: asset.contentHash, scene: target, clips: [] }],
    [{ asset: pack.asset, contentHash: hash, scene: packScene, clips: [
      new THREE.AnimationClip('Wave', 1, [new THREE.VectorKeyframeTrack('Root.position', [0, 1], [0, 0, 0, 0, 1, 0])]),
    ] }],
  );
  const registry = new AnimatedMeshRegistry(source);
  registry.define(asset);
  const instance = { asset: asset.asset, transform: { translation: [0, 0, 0] as const, rotation: [0, 0, 0, 1] as const, scale: [1, 1, 1] as const }, visible: true, materialOverrides: [], playback: null, metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label: null } };
  registry.create(renderHandle(71), instance);
  registry.create(renderHandle(72), instance);
  registry.setPlayback(renderHandle(71), { kind: 'play', clip: 'wave', loop: 'repeat', speed: 1, weight: 1, restart: true, fadeSeconds: null });
  registry.advance(0.5);
  const canonicalBeforeCapture = registry.playback(renderHandle(71))!.poseSample;
  const capturedAppearance = registry.createCaptureAppearance(renderHandle(71), 'wave', 0.25);
  assert.equal(capturedAppearance.source.origin, 'pack');
  assert.equal(capturedAppearance.source.pack?.asset, pack.asset);
  assert.equal(capturedAppearance.source.normalizedTime, 0.25);
  assert.notEqual(capturedAppearance.object, source.getAnimatedMeshResource(asset)!.scene);
  capturedAppearance.dispose();
  assert.deepEqual(registry.playback(renderHandle(71))!.poseSample, canonicalBeforeCapture,
    'external-pack capture appearance must not mutate canonical playback pose');
  assert.equal(registry.playback(renderHandle(71))?.effectiveClips[0]?.origin, 'pack');
  assert.equal(registry.playback(renderHandle(71))?.currentClip, 'wave');
  assert.equal(registry.playback(renderHandle(72))?.currentClip, null);
  assert.equal(registry.sample(renderHandle(72), 'wave', 0.25).clip, 'wave');
  assert.throws(() => registry.create(renderHandle(73), { ...instance, playback: {
    kind: 'play', clip: 'missing', loop: 'repeat', speed: 1, weight: 1, restart: true, fadeSeconds: null,
  } }), /missing clip missing/);
  assert.equal(registry.instanceCount, 2, 'failed initial playback must not publish an instance');

  const inconsistentPackScene = rigScene(true);
  inconsistentPackScene.traverse((node) => {
    if (node instanceof THREE.SkinnedMesh) node.skeleton.boneInverses[0]!.elements[12] = 1;
  });
  const inconsistentSource = new MapAnimatedMeshAssetSource(
    [{ asset: asset.asset, contentHash: asset.contentHash, scene: rigScene(true), clips: [] }],
    [{ asset: pack.asset, contentHash: hash, scene: inconsistentPackScene, clips: [
      new THREE.AnimationClip('Wave', 1, [new THREE.VectorKeyframeTrack('Root.position', [0, 1], [0, 0, 0, 0, 1, 0])]),
    ] }],
  );
  assert.throws(() => new AnimatedMeshRegistry(inconsistentSource).define(asset), /inverse bind/);
});

void test('clip descriptors bind one exact decoded source name and duration', () => {
  const asset = animatedMeshAsset({
    clips: [{ id: 'idle', name: 'source-idle', durationSeconds: 1 }],
    defaultClip: 'idle',
  });
  const scene = new THREE.Group();
  const source = (clips: readonly THREE.AnimationClip[]) => new MapAnimatedMeshAssetSource([{
    asset: asset.asset, contentHash: asset.contentHash, scene, clips,
  }]);
  assert.doesNotThrow(() => new AnimatedMeshRegistry(source([new THREE.AnimationClip('source-idle', 1.000001, [])])).define(asset));
  assert.throws(
    () => new AnimatedMeshRegistry(source([new THREE.AnimationClip('source-idle', 1.1, [])])).define(asset),
    /duration does not match/,
  );
  assert.throws(
    () => new AnimatedMeshRegistry(source([
      new THREE.AnimationClip('source-idle', 1, []), new THREE.AnimationClip('source-idle', 1, []),
    ])).define(asset),
    /exactly one clip named source-idle/,
  );
  assert.throws(
    () => new AnimatedMeshRegistry(source([new THREE.AnimationClip('Source-Idle', 1, [])])).define(asset),
    /exactly one clip named source-idle/,
  );
});

void test('mixed-case clip and rig identities use code-unit canonical order', () => {
  const target = mixedCaseRigScene();
  const identity = [...new THREE.Matrix4().elements];
  const canonical = [
    ['B', null, ...identity, ...identity],
    ['a', 'B', ...identity, ...identity],
  ];
  const expectedFingerprint = `sha256:${bytesToHex(sha256(new TextEncoder().encode(JSON.stringify(canonical))))}`;
  assert.equal(animationRigFingerprint(target), expectedFingerprint);

  const asset = animatedMeshAsset({
    clips: [
      { id: 'a', name: 'a', durationSeconds: 1 },
      { id: 'B', name: 'B', durationSeconds: 1 },
    ],
    defaultClip: null,
  });
  const source = new MapAnimatedMeshAssetSource([{
    asset: asset.asset,
    contentHash: asset.contentHash,
    scene: new THREE.Group(),
    clips: [new THREE.AnimationClip('a', 1, []), new THREE.AnimationClip('B', 1, [])],
  }]);
  const registry = new AnimatedMeshRegistry(source);
  registry.define(asset);
  registry.create(renderHandle(74), {
    asset: asset.asset,
    transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
    visible: true,
    materialOverrides: [],
    playback: null,
    metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label: null },
  });
  assert.deepEqual(registry.playback(renderHandle(74))?.effectiveClips.map((clip) => clip.id), ['B', 'a']);
});

void test('clip-pack channels reject malformed decoded keyframes before publication', () => {
  const target = rigScene(true);
  const hash = animationRigFingerprint(target);
  const rig = {
    joints: [{ id: 'Root', parent: null }], bindRestHash: hash,
    bindRestConvention: 'localMatrixV1' as const, rootConvention: 'inPlace' as const, rootJointId: 'Root',
    structuralRootIds: ['Root'], designatedMotionRootIds: [], authoredPoseTranslationJointIds: ['Root'],
  };
  const pack = {
    asset: 'animation-clip-pack/strict', runtimeFormat: 'glb' as const, contentHash: hash, rig,
    clips: [{ id: 'wave', name: 'Wave', durationSeconds: 1 }],
    provenance: { producer: 'fixture', sourceHash: `sha256:${'b'.repeat(64)}`, targetHash: `sha256:${'b'.repeat(64)}`, license: 'CC0-1.0' },
  };
  const asset = animatedMeshAsset({ clips: [], defaultClip: null, clipPacks: [pack] });
  const invalidTracks = [
    new THREE.VectorKeyframeTrack('Root.position', [0, Number.NaN], [0, 0, 0, 0, 0, 0]),
    new THREE.VectorKeyframeTrack('Root.position', [0, 0], [0, 0, 0, 0, 0, 0]),
    new THREE.VectorKeyframeTrack('Root.position', [0, 1], [0, 0, 0, 0]),
    new THREE.VectorKeyframeTrack('Root.position', [0, 1], [0, 0, 0, Number.NaN, 0, 0]),
    new THREE.VectorKeyframeTrack('Root.position', [0, 1], [0, 0, 0, 0, 0, 0]),
  ];
  invalidTracks.forEach((track, index) => {
    const tracks = index === invalidTracks.length - 1
      ? [track, new THREE.VectorKeyframeTrack('Root.position', [0, 1], [0, 0, 0, 0, 0, 0])]
      : [track];
    const source = new MapAnimatedMeshAssetSource(
      [{ asset: asset.asset, contentHash: asset.contentHash, scene: rigScene(true), clips: [] }],
      [{ asset: pack.asset, contentHash: hash, scene: rigScene(false), clips: [new THREE.AnimationClip('Wave', 1, tracks)] }],
    );
    assert.throws(() => new AnimatedMeshRegistry(source).define(asset), /malformed or unsupported channels/);
  });
});

void test('clip packs admit a joint forest when the designated root is structural', () => {
  const target = forestRigScene();
  const sourceScene = forestRigScene();
  const hash = animationRigFingerprint(target);
  assert.equal(hash, 'sha256:7d1cd48c239af954230c7eb699b1255577ef1ce709f9b9d22a74b6249a20592f');
  const pack = {
    asset: 'animation-clip-pack/forest', runtimeFormat: 'glb' as const, contentHash: hash,
    rig: {
      joints: [{ id: 'RootA', parent: null }, { id: 'RootB', parent: null }],
      bindRestHash: hash, bindRestConvention: 'localMatrixV1' as const,
      rootConvention: 'inPlace' as const, rootJointId: 'RootB',
      structuralRootIds: ['RootA', 'RootB'], designatedMotionRootIds: ['RootB'], authoredPoseTranslationJointIds: [],
    },
    clips: [{ id: 'wave', name: 'Wave', durationSeconds: 1 }],
    provenance: { producer: 'fixture', sourceHash: `sha256:${'c'.repeat(64)}`, targetHash: `sha256:${'c'.repeat(64)}`, license: 'CC0-1.0' },
  };
  const asset = animatedMeshAsset({ clips: [], defaultClip: null, clipPacks: [pack] });
  const registry = new AnimatedMeshRegistry(new MapAnimatedMeshAssetSource(
    [{ asset: asset.asset, contentHash: asset.contentHash, scene: target, clips: [] }],
    [{ asset: pack.asset, contentHash: hash, scene: sourceScene, clips: [
      new THREE.AnimationClip('Wave', 1, [new THREE.QuaternionKeyframeTrack('RootA.quaternion', [0, 1], [0, 0, 0, 1, 0, 0, 0, 1])]),
    ] }],
  ));
  assert.doesNotThrow(() => registry.define(asset));
});

void test('clip packs retain multiple root translations as authored pose when motion is unspecified', () => {
  const target = forestRigScene();
  const sourceScene = forestRigScene();
  const hash = animationRigFingerprint(target);
  const pack = {
    asset: 'animation-clip-pack/forest-pose', runtimeFormat: 'glb' as const, contentHash: hash,
    rig: {
      joints: [{ id: 'RootA', parent: null }, { id: 'RootB', parent: null }],
      bindRestHash: hash, bindRestConvention: 'localMatrixV1' as const,
      rootConvention: 'inPlace' as const, rootJointId: 'RootA',
      structuralRootIds: ['RootA', 'RootB'], designatedMotionRootIds: [],
      authoredPoseTranslationJointIds: ['RootA', 'RootB'],
    },
    clips: [{ id: 'pose', name: 'Pose', durationSeconds: 1 }],
    provenance: { producer: 'fixture', sourceHash: `sha256:${'d'.repeat(64)}`, targetHash: `sha256:${'d'.repeat(64)}`, license: 'CC0-1.0' },
  };
  const asset = animatedMeshAsset({ clips: [], defaultClip: null, clipPacks: [pack] });
  const source = new MapAnimatedMeshAssetSource(
    [{ asset: asset.asset, contentHash: asset.contentHash, scene: target, clips: [] }],
    [{ asset: pack.asset, contentHash: hash, scene: sourceScene, clips: [
      new THREE.AnimationClip('Pose', 1, [
        new THREE.VectorKeyframeTrack('RootA.position', [0, 1], [0, 0, 0, 1, 0, 0]),
        new THREE.VectorKeyframeTrack('RootB.position', [0, 1], [0, 0, 0, 0, 0, 1]),
      ]),
    ] }],
  );
  assert.doesNotThrow(() => new AnimatedMeshRegistry(source).define(asset));
});

function rigScene(withSkin = false): THREE.Group {
  const scene = new THREE.Group();
  const root = new THREE.Bone();
  root.name = 'Root';
  scene.add(root);
  if (withSkin) {
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.Float32BufferAttribute([0, 0, 0], 3));
    geometry.setAttribute('skinIndex', new THREE.Uint16BufferAttribute([0, 0, 0, 0], 4));
    geometry.setAttribute('skinWeight', new THREE.Float32BufferAttribute([1, 0, 0, 0], 4));
    const mesh = new THREE.SkinnedMesh(geometry, new THREE.MeshBasicMaterial());
    mesh.bind(new THREE.Skeleton([root], [new THREE.Matrix4()]));
    scene.add(mesh);
  }
  return scene;
}

function forestRigScene(): THREE.Group {
  const scene = new THREE.Group();
  const rootA = new THREE.Bone();
  rootA.name = 'RootA';
  rootA.position.set(Math.fround(0.12345674), Math.fround(-0.76543218), 0);
  const rootB = new THREE.Bone();
  rootB.name = 'RootB';
  scene.add(rootA, rootB);
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.Float32BufferAttribute([0, 0, 0], 3));
  geometry.setAttribute('skinIndex', new THREE.Uint16BufferAttribute([0, 1, 0, 0], 4));
  geometry.setAttribute('skinWeight', new THREE.Float32BufferAttribute([0.5, 0.5, 0, 0], 4));
  const mesh = new THREE.SkinnedMesh(geometry, new THREE.MeshBasicMaterial());
  mesh.bind(new THREE.Skeleton([rootA, rootB], [
    new THREE.Matrix4(),
    new THREE.Matrix4().fromArray([
      1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0,
      Math.fround(-1.23456776), Math.fround(0.33333334), 0, 1,
    ]),
  ]), new THREE.Matrix4());
  scene.add(mesh);
  return scene;
}

function mixedCaseRigScene(): THREE.Group {
  const scene = new THREE.Group();
  const root = new THREE.Bone();
  root.name = 'B';
  const child = new THREE.Bone();
  child.name = 'a';
  root.add(child);
  scene.add(root);
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.Float32BufferAttribute([0, 0, 0], 3));
  geometry.setAttribute('skinIndex', new THREE.Uint16BufferAttribute([0, 0, 0, 0], 4));
  geometry.setAttribute('skinWeight', new THREE.Float32BufferAttribute([1, 0, 0, 0], 4));
  const mesh = new THREE.SkinnedMesh(geometry, new THREE.MeshBasicMaterial());
  mesh.bind(new THREE.Skeleton([root, child], [new THREE.Matrix4(), new THREE.Matrix4()]));
  scene.add(mesh);
  return scene;
}

function testAnimatedMeshSource(asset = animatedMeshAsset()): MapAnimatedMeshAssetSource {
  const scene = new THREE.Group();
  scene.name = 'animated-fixture-root';
  const clips = asset.clips.map((clip) => {
    const duration = clip.durationSeconds ?? 1;
    const tracks =
      clip.id === 'run' ? [new THREE.VectorKeyframeTrack('.position', [0, duration], [0, 0, 0, 1, 0, 0])] : [];
    return new THREE.AnimationClip(clip.name ?? clip.id, duration, tracks);
  });
  return new MapAnimatedMeshAssetSource([{ asset: asset.asset, contentHash: asset.contentHash, scene, clips }]);
}

function diagnosticSkinnedMeshSource(
  asset: AnimatedMeshAsset,
  boneCount: number,
  weightRows: readonly (readonly [number, number, number, number])[],
): MapAnimatedMeshAssetSource {
  const scene = new THREE.Group();
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(
    weightRows.flatMap((_, index) => [index * 0.1, 0, 0]),
    3,
  ));
  geometry.setAttribute('skinIndex', new THREE.Uint16BufferAttribute(
    weightRows.flatMap(() => [0, 0, 0, 0]),
    4,
  ));
  geometry.setAttribute('skinWeight', new THREE.Float32BufferAttribute(
    weightRows.flatMap((row) => [...row]),
    4,
  ));
  const bones = Array.from({ length: boneCount }, (_, index) => {
    const bone = new THREE.Bone();
    bone.name = `joint-${String(index)}`;
    return bone;
  });
  for (let index = 1; index < bones.length; index += 1) {
    bones[index - 1]!.add(bones[index]!);
  }
  const mesh = new THREE.SkinnedMesh(geometry, new THREE.MeshBasicMaterial());
  if (bones[0] !== undefined) mesh.add(bones[0]);
  mesh.bind(new THREE.Skeleton(bones));
  scene.add(mesh);
  return new MapAnimatedMeshAssetSource([{
    asset: asset.asset,
    contentHash: asset.contentHash,
    scene,
    clips: asset.clips.map((clip) => new THREE.AnimationClip(
      clip.name ?? clip.id,
      clip.durationSeconds ?? 1,
      [],
    )),
  }]);
}

function rewriteGlbJson(
  source: Uint8Array,
  mutate: (root: unknown) => void,
): ArrayBuffer {
  const sourceView = new DataView(source.buffer, source.byteOffset, source.byteLength);
  assert.equal(sourceView.getUint32(0, true), 0x46546c67);
  const jsonLength = sourceView.getUint32(12, true);
  const oldJsonEnd = 20 + jsonLength;
  const root = JSON.parse(new TextDecoder().decode(source.subarray(20, oldJsonEnd))) as unknown;
  mutate(root);
  const encoded = new TextEncoder().encode(JSON.stringify(root));
  const paddedLength = Math.ceil(encoded.byteLength / 4) * 4;
  const remainder = source.subarray(oldJsonEnd);
  const totalLength = 20 + paddedLength + remainder.byteLength;
  const rewritten = new Uint8Array(totalLength);
  const view = new DataView(rewritten.buffer);
  view.setUint32(0, 0x46546c67, true);
  view.setUint32(4, 2, true);
  view.setUint32(8, totalLength, true);
  view.setUint32(12, paddedLength, true);
  view.setUint32(16, 0x4e4f534a, true);
  rewritten.fill(0x20, 20, 20 + paddedLength);
  rewritten.set(encoded, 20);
  rewritten.set(remainder, 20 + paddedLength);
  return rewritten.buffer;
}

void test('animated GLB loader realizes KHR_texture_transform from retained source bytes', async () => {
  const testGlobal = globalThis as unknown as {
    self: unknown;
    createImageBitmap?: (blob: Blob, options?: ImageBitmapOptions) => Promise<ImageBitmap>;
  };
  const priorSelf = testGlobal.self;
  const priorCreateImageBitmap = testGlobal.createImageBitmap;
  testGlobal.self = globalThis;
  testGlobal.createImageBitmap = async () => ({ width: 1, height: 1, close() {} }) as ImageBitmap;
  const priorWarn = console.warn;
  const priorError = console.error;
  console.warn = () => undefined;
  console.error = () => undefined;
  try {
    const source = readFileSync(
      resolve(import.meta.dirname, '../../../../fixtures/render/assets/kenney-retro-character/character-medium.glb'),
    );
    const data = rewriteGlbJson(source, (untypedRoot) => {
      const root = untypedRoot as {
        extensionsUsed: string[];
        materials: Array<{
          pbrMetallicRoughness: {
            baseColorTexture: { extensions?: Record<string, unknown> };
          };
        }>;
      };
      root.extensionsUsed.push('KHR_texture_transform');
      root.materials[0]!.pbrMetallicRoughness.baseColorTexture.extensions = {
        KHR_texture_transform: {
          offset: [-0.25, 0.5],
          rotation: 0.75,
          scale: [2, -3],
          texCoord: 0,
        },
      };
    });
    const resource = await loadAnimatedMeshGlbResource(
      'mesh-animation/transformed-character',
      data,
      undefined,
      [{ slot: 0, sourceMaterialSlot: 0 }],
    );
    const material = resource.embeddedMaterialSlots?.get(0)?.materials[0] as THREE.MeshBasicMaterial;
    assert.ok(material.map instanceof THREE.Texture);
    assert.deepEqual(material.map.offset.toArray(), [-0.25, 0.5]);
    assert.equal(material.map.rotation, 0.75);
    assert.deepEqual(material.map.repeat.toArray(), [2, -3]);
    assert.equal(material.map.channel, 0);
    assert.deepEqual(resource.clips.map((clip) => clip.name).sort(), ['idle', 'jump', 'run']);
  } finally {
    console.warn = priorWarn;
    console.error = priorError;
    testGlobal.self = priorSelf;
    if (priorCreateImageBitmap === undefined) delete testGlobal.createImageBitmap;
    else testGlobal.createImageBitmap = priorCreateImageBitmap;
  }
});

void test('committed animated GLB instances share GPU resources while playback remains independent', async () => {
  const testGlobal = globalThis as unknown as { self: unknown };
  const priorSelf = testGlobal.self;
  testGlobal.self = globalThis;
  const priorWarn = console.warn;
  const priorError = console.error;
  console.warn = () => undefined;
  console.error = () => undefined;
  try {
    const bytes = readFileSync(
      resolve(import.meta.dirname, '../../../../fixtures/render/assets/kenney-retro-character/character-medium.glb'),
    );
    const data = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
    const embeddedMaterialSlots = [{ slot: 0, sourceMaterialSlot: 0 }] as const;
    const resource = await loadAnimatedMeshGlbResource(
      'mesh-animation/kenney-retro-character-medium',
      data,
      undefined,
      embeddedMaterialSlots,
    );
    assert.equal(resource.embeddedMaterialSlots?.get(0)?.sourceMaterialSlot, 0);
    assert.deepEqual(
      resource.clips.map((clip) => clip.name).sort(),
      ['idle', 'jump', 'run'],
    );
    const asset = animatedMeshAsset({ embeddedMaterialSlots });
    const registry = new AnimatedMeshRegistry(new MapAnimatedMeshAssetSource([resource]));
    registry.define(asset);
    const mappedInstance = registry.create(renderHandle(4097), {
      asset: asset.asset,
      transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
      visible: true,
      materialOverrides: [],
      playback: null,
      metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label: 'mapped-instance' },
    });
    const mappedMaterial = registry.embeddedMaterialSlots(renderHandle(4097))?.get(0);
    assert.equal(mappedMaterial?.sourceMaterialSlot, 0);
    assert.ok(mappedMaterial?.materials.includes(firstMesh(mappedInstance.object).material as THREE.Material));
    assert.throws(
      () => registry.validateDefinition({
        ...asset,
        embeddedMaterialSlots: [{ slot: 0, sourceMaterialSlot: 1 }],
      }),
      /embedded material slot mapping is unavailable/,
    );
    registry.release(renderHandle(4097));
    const renderer = new ThreeRenderer({
      animatedMeshSource: new MapAnimatedMeshAssetSource([resource]),
    });
    renderer.applyDiff({ op: 'defineAnimatedMesh', asset });
    for (const [handle, clip] of [
      [renderHandle(4098), 'idle'],
      [renderHandle(4099), 'run'],
    ] as const) {
      renderer.applyDiff({
        op: 'createAnimatedMeshInstance',
        handle,
        parent: null,
        instance: {
          asset: asset.asset,
          transform: { translation: [handle - 4098, 0, -2], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
          materialOverrides: [],
          playback: {
            kind: 'play',
            clip,
            loop: 'repeat',
            speed: 1,
            weight: 1,
            restart: true,
            fadeSeconds: null,
          },
          visible: true,
          metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label: `animated-${handle}` },
        },
      });
    }
    assert.deepEqual(renderer.resourceStatistics(), {
      renderHandleCount: 2,
      geometryResourceCount: 1,
      materialResourceCount: 1,
      textureResourceCount: 0,
      animatedInstanceCount: 2,
    });
    const idle = firstMesh(renderer.objectFor(renderHandle(4098))!) as THREE.SkinnedMesh;
    const run = firstMesh(renderer.objectFor(renderHandle(4099))!) as THREE.SkinnedMesh;
    assert.equal(idle.geometry, run.geometry);
    assert.equal(idle.material, run.material);
    assert.notEqual(idle.skeleton, run.skeleton);
    assert.notEqual(idle.skeleton.bones[0], run.skeleton.bones[0]);
    renderer.advanceAnimation(0.25);
    assert.equal(renderer.animatedMeshPlayback(renderHandle(4098))?.currentClip, 'idle');
    assert.equal(renderer.animatedMeshPlayback(renderHandle(4099))?.currentClip, 'run');
    const idleSample = renderer.sampleAnimatedMesh(renderHandle(4098), 'idle', 0.25);
    const runBefore = renderer.animatedMeshPlayback(renderHandle(4099));
    const runSample = renderer.sampleAnimatedMesh(renderHandle(4099), 'run', 0.75);
    assert.equal(idleSample.normalizedTime, 0.25);
    assert.equal(runSample.normalizedTime, 0.75);
    assert.equal(idleSample.contentHash, asset.contentHash);
    assert.ok(idleSample.sampledVertexCount > 0);
    assert.ok(idleSample.boneCount > 0);
    assert.ok(idleSample.skinningFacts.joints.length > 0);
    assert.ok(idleSample.skinningFacts.joints.every((joint) => joint.restLocalMatrix.length === 16));
    assert.equal(idleSample.skinningFacts.inverseBindMatricesFinite, true);
    assert.equal(idleSample.skinningFacts.weightsNormalized, true);
    assert.deepEqual(idleSample.skinningFacts.interpolationModes, ['linear']);
    assert.equal(idleSample.skinningFacts.instanceRootDistinctFromTemplate, true);
    assert.equal(idleSample.skinningFacts.skeletonsIndependentFromTemplate, true);
    assert.ok(idleSample.skinningFacts.sharedGeometryCount > 0);
    assert.ok(idleSample.skinningFacts.sharedMaterialCount > 0);
    assert.notEqual(idleSample.sampledWorldBounds, null);
    assert.deepEqual(idleSample.diagnostics, []);
    assert.deepEqual(runSample.diagnostics, []);
    assert.equal(runBefore?.currentClip, 'run');
    assert.equal(renderer.animatedMeshPlayback(renderHandle(4098))?.status, 'sampled');
    assert.equal(renderer.animatedMeshPlayback(renderHandle(4099))?.status, 'sampled');
    assert.notDeepEqual(idleSample.sampledWorldBounds, runSample.sampledWorldBounds);
    idle.skeleton.bones[0]!.scale.set(0, 1, 1);
    const invalidSample = renderer.sampleAnimatedMesh(renderHandle(4098), 'idle', 0.5);
    assert.ok(invalidSample.diagnostics.some((item) => item.code === 'node_scale_invalid'));
    assert.ok(invalidSample.diagnostics.some((item) => item.code === 'bone_matrix_singular'));
    renderer.dispose();
  } finally {
    console.warn = priorWarn;
    console.error = priorError;
    testGlobal.self = priorSelf;
  }
});

void test('animated mesh playback is command-selected and advances through renderer ticks only', () => {
  const asset = animatedMeshAsset();
  const r = new ThreeRenderer({ animatedMeshSource: testAnimatedMeshSource(asset) });
  const handle = renderHandle(4100);
  r.applyDiff({ op: 'defineAnimatedMesh', asset });
  r.applyDiff({
    op: 'createAnimatedMeshInstance',
    handle,
    parent: null,
    instance: {
      asset: asset.asset,
      transform: { translation: [0, 0, -2], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
      materialOverrides: [],
      playback: null,
      visible: true,
      metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label: 'animated character' },
    },
  });

  const initial = r.animatedMeshPlayback(handle);
  assert.equal(initial?.currentClip, null);
  assert.equal(initial?.commandSelected, false);
  assert.equal(initial?.status, 'not_started');
  assert.deepEqual(initial?.diagnostics, ['animation_not_started']);

  r.applyDiff({
    op: 'setAnimatedMeshPlayback',
    handle,
    playback: { kind: 'sample', clip: 'run', normalizedTime: 0 },
  });
  const heldStart = r.animatedMeshPlayback(handle);
  assert.equal(heldStart?.status, 'sampled');
  assert.deepEqual(heldStart?.heldSample, { clip: 'run', normalizedTime: 0 });
  assert.deepEqual(heldStart?.diagnostics, ['animation_sampled']);
  r.advanceAnimation(0.75);
  const heldAfterAdvance = r.animatedMeshPlayback(handle);
  assert.equal(heldAfterAdvance?.actionTimeSeconds, heldStart?.actionTimeSeconds);
  assert.deepEqual(heldAfterAdvance?.heldSample, heldStart?.heldSample);

  r.applyDiff({
    op: 'setAnimatedMeshPlayback',
    handle,
    playback: { kind: 'sample', clip: 'run', normalizedTime: 1 },
  });
  assert.deepEqual(r.animatedMeshPlayback(handle)?.heldSample, { clip: 'run', normalizedTime: 1 });
  const beforeRejectedSample = r.animatedMeshPlayback(handle);
  assert.throws(
    () => r.applyDiff({
      op: 'setAnimatedMeshPlayback',
      handle,
      playback: { kind: 'sample', clip: 'missing', normalizedTime: 0.5 },
    }),
    /clip missing is not defined/,
  );
  assert.deepEqual(r.animatedMeshPlayback(handle), beforeRejectedSample);

  r.applyDiff({
    op: 'setAnimatedMeshPlayback',
    handle,
    playback: { kind: 'play', clip: 'run', loop: 'repeat', speed: 1, weight: 1, restart: true, fadeSeconds: null },
  });
  const selected = r.animatedMeshPlayback(handle);
  assert.equal(selected?.currentClip, 'run');
  assert.equal(selected?.commandSelected, true);
  assert.equal(selected?.loop, 'repeat');
  assert.equal(selected?.status, 'playing');
  assert.equal(selected?.heldSample, null);
  assert.deepEqual(selected?.diagnostics, []);

  r.advanceAnimation(0.25);
  const advanced = r.animatedMeshPlayback(handle);
  assert.equal(advanced?.currentClip, 'run');
  assert.equal(advanced?.running, true);
  assert.ok((advanced?.mixerTimeSeconds ?? 0) > 0);
  assert.ok((advanced?.actionTimeSeconds ?? 0) > 0);
  assert.notDeepEqual(advanced?.poseSample.rootTranslation, selected?.poseSample.rootTranslation);

  r.applyDiff({ op: 'setAnimatedMeshPlayback', handle, playback: { kind: 'pause' } });
  assert.equal(r.animatedMeshPlayback(handle)?.status, 'paused');
  assert.deepEqual(r.animatedMeshPlayback(handle)?.diagnostics, ['animation_paused']);

  r.applyDiff({ op: 'setAnimatedMeshPlayback', handle, playback: { kind: 'stop', fadeSeconds: null } });
  assert.equal(r.animatedMeshPlayback(handle)?.status, 'stopped');
  assert.deepEqual(r.animatedMeshPlayback(handle)?.diagnostics, ['animation_stopped']);
  assert.throws(
    () => r.sampleAnimatedMesh(handle, 'run', 1.01),
    /normalizedTime must be finite and between 0 and 1/,
  );
  assert.throws(
    () => r.sampleAnimatedMesh(handle, 'missing', 0.5),
    /missing clip missing/,
  );
});

void test('LoopOnce natural completion is mixer-event driven and rejects invalidated epochs', () => {
  const asset = animatedMeshAsset();
  const registry = new AnimatedMeshRegistry(testAnimatedMeshSource(asset));
  const completions: { readonly objectId: number; readonly generation: number; readonly clip: string }[] = [];
  const unsubscribe = registry.subscribeNaturalCompletions((completion) => completions.push(completion));
  const handle = renderHandle(4810);
  const instance = {
    asset: asset.asset,
    transform: { translation: [0, 0, 0] as const, rotation: [0, 0, 0, 1] as const, scale: [1, 1, 1] as const },
    materialOverrides: [], playback: null, visible: true,
    metadata: { sourceEntity: 77, sourceSceneNode: null, tags: [], label: 'one shot' },
  };
  const playOnce = () => registry.setPlayback(handle, {
    kind: 'play', clip: 'run', loop: 'once', speed: 1, weight: 1, restart: true, fadeSeconds: null,
  });
  registry.define(asset);
  registry.create(handle, instance);

  // The actual Three mixer event completes exactly once; no time/status poll
  // can produce another observation after the token has been cleared.
  playOnce();
  registry.advance(1);
  assert.deepEqual(completions.splice(0), [{ objectId: 77, generation: 1, clip: 'run' }]);
  assert.equal(registry.playback(handle)?.status, 'stopped');
  registry.advance(1);
  assert.deepEqual(completions.splice(0), []);

  registry.setPlayback(handle, { kind: 'play', clip: 'run', loop: 'repeat', speed: 1, weight: 1, restart: true, fadeSeconds: null });
  registry.advance(2);
  assert.deepEqual(completions.splice(0), []);

  // Pause invalidates the token; resume arms a fresh token for the retained
  // LoopOnce action and only that resumed run can report completion.
  playOnce();
  registry.advance(0.2);
  registry.setPlayback(handle, { kind: 'pause' });
  registry.advance(2);
  assert.deepEqual(completions.splice(0), []);
  registry.setPlayback(handle, { kind: 'resume' });
  registry.advance(1);
  assert.deepEqual(completions.splice(0), [{ objectId: 77, generation: 1, clip: 'run' }]);

  playOnce();
  registry.setPlayback(handle, { kind: 'stop', fadeSeconds: 0.1 });
  registry.advance(2);
  assert.deepEqual(completions.splice(0), []);

  playOnce();
  registry.setControllerWeights(handle, [{ clip: 'idle', weight: 1, speed: 1 }]);
  registry.advance(2);
  assert.deepEqual(completions.splice(0), []);
  registry.clearControllerWeights(handle);
  assert.deepEqual(completions.splice(0), []);

  playOnce();
  registry.sample(handle, 'run', 1);
  registry.advance(2);
  assert.deepEqual(completions.splice(0), []);

  // Releasing removes the mixer listener before uncache, so the old action
  // cannot report after replacement; the new realization has generation two.
  playOnce();
  registry.release(handle);
  registry.create(handle, instance);
  playOnce();
  registry.advance(1);
  assert.deepEqual(completions.splice(0), [{ objectId: 77, generation: 2, clip: 'run' }]);
  playOnce();
  registry.release(handle);
  assert.deepEqual(completions.splice(0), []);
  unsubscribe();
  registry.dispose();
});

void test('animated skinning inspection rejects an over-budget hierarchy before playback mutation', () => {
  const asset = animatedMeshAsset();
  const renderer = new ThreeRenderer({
    animatedMeshSource: diagnosticSkinnedMeshSource(asset, 257, [[1, 0, 0, 0]]),
  });
  const handle = renderHandle(4198);
  renderer.applyDiff({ op: 'defineAnimatedMesh', asset });
  renderer.applyDiff({
    op: 'createAnimatedMeshInstance',
    handle,
    parent: null,
    instance: {
      asset: asset.asset,
      transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
      materialOverrides: [],
      playback: {
        kind: 'play', clip: 'run', loop: 'repeat', speed: 1, weight: 1,
        restart: true, fadeSeconds: null,
      },
      visible: true,
      metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label: 'joint-budget' },
    },
  });
  renderer.advanceAnimation(0.25);
  const before = renderer.animatedMeshPlayback(handle);
  assert.throws(
    () => renderer.sampleAnimatedMesh(handle, 'idle', 0.5),
    /joint count exceeds 256/,
  );
  assert.deepEqual(renderer.animatedMeshPlayback(handle), before);
  renderer.dispose();
});

void test('a rejected initial animated sample preserves live texture/material resources and releases preparation', () => {
  const bytes = rgbaPng(2, 1, [255, 0, 0, 255, 0, 255, 0, 255]);
  const source = new TestTextureResourceSource(bytes);
  const asset = animatedMeshAsset();
  const renderer = new ThreeRenderer({
    animatedMeshSource: diagnosticSkinnedMeshSource(asset, 257, [[1, 0, 0, 0]]),
    textureResourceSource: source,
  });
  const beforeTexture = textureDescriptor(bytes, 1, 'resource');
  renderer.applyFrame({ schemaVersion: 1, ops: [
    { op: 'defineTexture', texture: beforeTexture },
    { op: 'defineMaterial', material: texturedMaterial() },
    { op: 'defineStaticMesh', asset: texturedPlankAsset() },
    {
      op: 'createStaticMeshInstance', handle: renderHandle(4290), parent: null,
      instance: crateInstance('mesh/textured-plank'),
    },
    { op: 'defineAnimatedMesh', asset },
  ] });
  const mesh = renderer.objectFor(renderHandle(4290)) as THREE.Mesh;
  const oldMaterial = mesh.material;
  const oldTexture = (oldMaterial as THREE.MeshStandardMaterial).map;
  const beforeSnapshot = renderer.snapshot();
  const beforeDescriptor = renderer.textureDescriptor(beforeTexture.id);
  const beforeResources = renderer.resourceStatistics();
  const beforeReadout = renderer.textureResourceReadout();

  assert.throws(() => renderer.applyFrame({ schemaVersion: 1, ops: [
    { op: 'defineTexture', texture: textureDescriptor(bytes, 2, 'resource') },
    {
      op: 'defineMaterial',
      material: { ...texturedMaterial(), color: [0.2, 0.7, 0.4, 1] },
    },
    {
      op: 'createAnimatedMeshInstance', handle: renderHandle(4291), parent: null,
      instance: {
        asset: asset.asset,
        transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
        materialOverrides: [],
        playback: { kind: 'sample', clip: 'idle', normalizedTime: 0.5 },
        visible: true,
        metadata: { sourceEntity: 88, sourceSceneNode: null, tags: [], label: 'must-not-publish' },
      },
    },
  ] }), /joint count exceeds 256/u);

  assert.equal(mesh.material, oldMaterial);
  assert.equal((mesh.material as THREE.MeshStandardMaterial).map, oldTexture);
  assert.equal(renderer.animatedMeshPlayback(renderHandle(4291)), undefined);
  assert.equal(renderer.snapshot(), beforeSnapshot);
  assert.deepEqual(renderer.textureDescriptor(beforeTexture.id), beforeDescriptor);
  assert.deepEqual(renderer.resourceStatistics(), beforeResources);
  assert.deepEqual(renderer.textureResourceReadout(), beforeReadout);
  assert.deepEqual(source.released, source.acquired, 'prepared resource borrows are always released');
  renderer.dispose();
});

void test('a rejected non-sample animated creation preserves earlier frame resources', () => {
  const bytes = rgbaPng(2, 1, [255, 0, 0, 255, 0, 255, 0, 255]);
  const source = new TestTextureResourceSource(bytes);
  const asset = animatedMeshAsset();
  const renderer = new ThreeRenderer({
    animatedMeshSource: testAnimatedMeshSource(asset),
    textureResourceSource: source,
  });
  const beforeTexture = textureDescriptor(bytes, 1, 'resource');
  renderer.applyFrame({ schemaVersion: 1, ops: [
    { op: 'defineTexture', texture: beforeTexture },
    { op: 'defineMaterial', material: texturedMaterial() },
    { op: 'defineStaticMesh', asset: texturedPlankAsset() },
    {
      op: 'createStaticMeshInstance', handle: renderHandle(4294), parent: null,
      instance: crateInstance('mesh/textured-plank'),
    },
    { op: 'defineAnimatedMesh', asset },
  ] });
  const mesh = renderer.objectFor(renderHandle(4294)) as THREE.Mesh;
  const oldMaterial = mesh.material;
  const oldTexture = (oldMaterial as THREE.MeshStandardMaterial).map;
  const beforeSnapshot = renderer.snapshot();
  const beforeDescriptor = renderer.textureDescriptor(beforeTexture.id);
  const beforeResources = renderer.resourceStatistics();

  assert.throws(() => renderer.applyFrame({ schemaVersion: 1, ops: [
    { op: 'defineTexture', texture: textureDescriptor(bytes, 2, 'resource') },
    { op: 'defineMaterial', material: { ...texturedMaterial(), color: [0.4, 0.2, 0.8, 1] } },
    {
      op: 'createAnimatedMeshInstance', handle: renderHandle(4295), parent: null,
      instance: {
        asset: asset.asset,
        transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
        materialOverrides: [{ slot: 0, material: texturedMaterial().id }],
        playback: null,
        visible: true,
        metadata: { sourceEntity: 90, sourceSceneNode: null, tags: [], label: 'must-not-publish' },
      },
    },
  ] }), /override for unbound embedded material slot 0/u);

  assert.equal(mesh.material, oldMaterial);
  assert.equal((mesh.material as THREE.MeshStandardMaterial).map, oldTexture);
  assert.equal(renderer.animatedMeshPlayback(renderHandle(4295)), undefined);
  assert.equal(renderer.snapshot(), beforeSnapshot);
  assert.deepEqual(renderer.textureDescriptor(beforeTexture.id), beforeDescriptor);
  assert.deepEqual(renderer.resourceStatistics(), beforeResources);
  assert.deepEqual(source.released, source.acquired, 'prepared resource borrows are always released');
  renderer.dispose();
});

void test('a rejected animated sample update preserves live texture/material and handle state', () => {
  const bytes = rgbaPng(2, 1, [255, 0, 0, 255, 0, 255, 0, 255]);
  const source = new TestTextureResourceSource(bytes);
  const asset = animatedMeshAsset();
  const renderer = new ThreeRenderer({
    animatedMeshSource: diagnosticSkinnedMeshSource(asset, 257, [[1, 0, 0, 0]]),
    textureResourceSource: source,
  });
  const beforeTexture = textureDescriptor(bytes, 1, 'resource');
  renderer.applyFrame({ schemaVersion: 1, ops: [
    { op: 'defineTexture', texture: beforeTexture },
    { op: 'defineMaterial', material: texturedMaterial() },
    { op: 'defineStaticMesh', asset: texturedPlankAsset() },
    {
      op: 'createStaticMeshInstance', handle: renderHandle(4292), parent: null,
      instance: crateInstance('mesh/textured-plank'),
    },
    { op: 'defineAnimatedMesh', asset },
    {
      op: 'createAnimatedMeshInstance', handle: renderHandle(4293), parent: null,
      instance: {
        asset: asset.asset,
        transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
        materialOverrides: [], playback: null, visible: true,
        metadata: { sourceEntity: 89, sourceSceneNode: null, tags: [], label: 'live-animated' },
      },
    },
  ] });
  const mesh = renderer.objectFor(renderHandle(4292)) as THREE.Mesh;
  const oldMaterial = mesh.material;
  const oldTexture = (oldMaterial as THREE.MeshStandardMaterial).map;
  const beforeSnapshot = renderer.snapshot();
  const beforePlayback = renderer.animatedMeshPlayback(renderHandle(4293));
  const beforeDescriptor = renderer.textureDescriptor(beforeTexture.id);
  const beforeResources = renderer.resourceStatistics();

  assert.throws(() => renderer.applyFrame({ schemaVersion: 1, ops: [
    { op: 'defineTexture', texture: textureDescriptor(bytes, 2, 'resource') },
    { op: 'defineMaterial', material: { ...texturedMaterial(), color: [0.8, 0.3, 0.1, 1] } },
    {
      op: 'setAnimatedMeshPlayback', handle: renderHandle(4293),
      playback: { kind: 'sample', clip: 'idle', normalizedTime: 0.5 },
    },
  ] }), /joint count exceeds 256/u);

  assert.equal(mesh.material, oldMaterial);
  assert.equal((mesh.material as THREE.MeshStandardMaterial).map, oldTexture);
  assert.equal(renderer.snapshot(), beforeSnapshot);
  assert.deepEqual(renderer.animatedMeshPlayback(renderHandle(4293)), beforePlayback);
  assert.deepEqual(renderer.textureDescriptor(beforeTexture.id), beforeDescriptor);
  assert.deepEqual(renderer.resourceStatistics(), beforeResources);
  assert.deepEqual(source.released, source.acquired, 'prepared resource borrows are always released');
  renderer.dispose();
});

void test('animated skinning inspection rejects zero and non-finite weight sums as normalized', () => {
  const asset = animatedMeshAsset();
  const renderer = new ThreeRenderer({
    animatedMeshSource: diagnosticSkinnedMeshSource(asset, 1, [
      [1, 0, 0, 0],
      [0, 0, 0, 0],
      [Number.NaN, 0, 0, 0],
    ]),
  });
  const handle = renderHandle(4199);
  renderer.applyDiff({ op: 'defineAnimatedMesh', asset });
  renderer.applyDiff({
    op: 'createAnimatedMeshInstance',
    handle,
    parent: null,
    instance: {
      asset: asset.asset,
      transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
      materialOverrides: [],
      playback: null,
      visible: true,
      metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label: 'invalid-weights' },
    },
  });
  const sample = renderer.sampleAnimatedMesh(handle, 'idle', 0.5);
  assert.equal(sample.skinningFacts.weightedVertexCount, 3);
  assert.equal(sample.skinningFacts.invalidWeightVertexCount, 2);
  assert.equal(sample.skinningFacts.weightsNormalized, false);
  renderer.dispose();
});

void test('animated instances reuse asset-scoped geometry and materials with independent skeletons and lifecycle', () => {
  const asset = animatedMeshAsset({ clips: [], defaultClip: null });
  const sourceScene = new THREE.Group();
  const sourceGeometry = new THREE.BoxGeometry(1, 1, 1);
  const sourceMaterial = new THREE.MeshStandardMaterial({ color: 0xffffff });
  const sourceTexture = new THREE.DataTexture(new Uint8Array([255, 255, 255, 255]), 1, 1);
  let geometryCloneCount = 0;
  let materialCloneCount = 0;
  const cloneSourceGeometry = sourceGeometry.clone.bind(sourceGeometry);
  const cloneSourceMaterial = sourceMaterial.clone.bind(sourceMaterial);
  sourceGeometry.clone = () => {
    geometryCloneCount += 1;
    return cloneSourceGeometry();
  };
  sourceMaterial.clone = () => {
    materialCloneCount += 1;
    return cloneSourceMaterial();
  };
  sourceTexture.needsUpdate = true;
  sourceMaterial.map = sourceTexture;
  const sourceBone = new THREE.Bone();
  sourceBone.name = 'root-bone';
  const sourceMesh = new THREE.SkinnedMesh(sourceGeometry, sourceMaterial);
  sourceMesh.add(sourceBone);
  sourceMesh.bind(new THREE.Skeleton([sourceBone]));
  sourceScene.add(sourceMesh);
  const source = new MapAnimatedMeshAssetSource([{
    asset: asset.asset,
    contentHash: asset.contentHash,
    scene: sourceScene,
    clips: [],
  }]);
  const renderer = new ThreeRenderer({ animatedMeshSource: source });
  renderer.applyDiff({ op: 'defineAnimatedMesh', asset });
  const createInstance = (handle: ReturnType<typeof renderHandle>, translationX = 0): void => {
    renderer.applyDiff({
      op: 'createAnimatedMeshInstance',
      handle,
      parent: null,
      instance: {
        asset: asset.asset,
        transform: { translation: [translationX, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
        materialOverrides: [],
        playback: null,
        visible: true,
        metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label: `animated-${handle}` },
      },
    });
  };
  for (const handle of [renderHandle(4201), renderHandle(4202)]) {
    createInstance(handle);
  }
  assert.equal(geometryCloneCount, 1);
  assert.equal(materialCloneCount, 1);
  assert.deepEqual(renderer.resourceStatistics(), {
    renderHandleCount: 2,
    geometryResourceCount: 1,
    materialResourceCount: 1,
    textureResourceCount: 1,
    animatedInstanceCount: 2,
  });
  const firstMeshInstance = firstMesh(renderer.objectFor(renderHandle(4201))!) as THREE.SkinnedMesh;
  const secondMeshInstance = firstMesh(renderer.objectFor(renderHandle(4202))!) as THREE.SkinnedMesh;
  const firstGeometry = firstMeshInstance.geometry;
  const secondGeometry = secondMeshInstance.geometry;
  const firstMaterial = firstMeshInstance.material as THREE.Material;
  const secondMaterial = secondMeshInstance.material as THREE.Material;
  let sharedGeometryDisposed = false;
  let sharedMaterialDisposed = false;
  let sourceDisposed = false;
  firstGeometry.addEventListener('dispose', () => { sharedGeometryDisposed = true; });
  firstMaterial.addEventListener('dispose', () => { sharedMaterialDisposed = true; });
  sourceGeometry.addEventListener('dispose', () => { sourceDisposed = true; });

  assert.equal(firstGeometry, secondGeometry);
  assert.equal(firstMaterial, secondMaterial);
  assert.notEqual(firstGeometry, sourceGeometry);
  assert.notEqual(firstMaterial, sourceMaterial);
  assert.notEqual(firstMeshInstance.skeleton, secondMeshInstance.skeleton);
  assert.notEqual(firstMeshInstance.skeleton.bones[0], secondMeshInstance.skeleton.bones[0]);
  firstMeshInstance.skeleton.bones[0]!.position.x = 3;
  assert.equal(secondMeshInstance.skeleton.bones[0]!.position.x, 0);
  renderer.applyDiff({ op: 'destroy', handle: renderHandle(4201) });
  assert.equal(sharedGeometryDisposed, false);
  assert.equal(sharedMaterialDisposed, false);
  assert.equal(sourceDisposed, false);
  assert.deepEqual(renderer.resourceStatistics(), {
    renderHandleCount: 1,
    geometryResourceCount: 1,
    materialResourceCount: 1,
    textureResourceCount: 1,
    animatedInstanceCount: 1,
  });

  renderer.applyDiff({ op: 'destroy', handle: renderHandle(4202) });
  assert.equal(sharedGeometryDisposed, false);
  assert.equal(sharedMaterialDisposed, false);
  assert.deepEqual(renderer.resourceStatistics(), {
    renderHandleCount: 0,
    geometryResourceCount: 1,
    materialResourceCount: 1,
    textureResourceCount: 1,
    animatedInstanceCount: 0,
  });
  createInstance(renderHandle(4201), 1);
  createInstance(renderHandle(4202), 2);
  assert.ok(renderer.objectFor(renderHandle(4201)));
  assert.ok(renderer.objectFor(renderHandle(4202)));
  assert.equal(sourceDisposed, false);
  assert.deepEqual(renderer.resourceStatistics(), {
    renderHandleCount: 2,
    geometryResourceCount: 1,
    materialResourceCount: 1,
    textureResourceCount: 1,
    animatedInstanceCount: 2,
  });

  renderer.dispose();
  assert.equal(sharedGeometryDisposed, true);
  assert.equal(sharedMaterialDisposed, true);
  assert.equal(sourceDisposed, false);
  assert.deepEqual(renderer.resourceStatistics(), {
    renderHandleCount: 0,
    geometryResourceCount: 0,
    materialResourceCount: 0,
    textureResourceCount: 0,
    animatedInstanceCount: 0,
  });
});

void test('animated mesh adapter fails closed for missing resources and clips', () => {
  const asset = animatedMeshAsset();
  const missingResource = new ThreeRenderer();
  assert.throws(
    () => missingResource.applyDiff({ op: 'defineAnimatedMesh', asset }),
    /missing animated mesh resource/,
  );

  const wrongClips = new ThreeRenderer({
    animatedMeshSource: new MapAnimatedMeshAssetSource([
      { asset: asset.asset, scene: new THREE.Group(), clips: [new THREE.AnimationClip('idle', asset.clips[0]!.durationSeconds!, [])] },
    ]),
  });
  assert.throws(
    () => wrongClips.applyDiff({ op: 'defineAnimatedMesh', asset }),
    /exactly one clip named run/,
  );

  const wrongHash = new ThreeRenderer({ animatedMeshSource: testAnimatedMeshSource(asset) });
  assert.throws(
    () => wrongHash.applyDiff({ op: 'defineAnimatedMesh', asset: animatedMeshAsset({ contentHash: 'sha256:wrong' }) }),
    /content hash mismatch/,
  );
});

void test('frame rollback does not publish an animated definition or instance after invalid initial playback', () => {
  const asset = animatedMeshAsset();
  const renderer = new ThreeRenderer({ animatedMeshSource: testAnimatedMeshSource(asset) });
  const instance = {
    asset: asset.asset, transform: { translation: [0, 0, 0] as const, rotation: [0, 0, 0, 1] as const, scale: [1, 1, 1] as const },
    visible: true, materialOverrides: [],
    playback: { kind: 'play' as const, clip: 'not-admitted', loop: 'repeat' as const, speed: 1, weight: 1, restart: true, fadeSeconds: null },
    metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label: null },
  };
  assert.throws(() => renderer.applyFrame({ schemaVersion: 1, ops: [
    { op: 'defineAnimatedMesh', asset },
    { op: 'createAnimatedMeshInstance', handle: renderHandle(991), parent: null, instance },
  ] }), /not-admitted is not defined/);
  assert.equal(renderer.animatedMeshPlayback(renderHandle(991)), undefined);
  assert.equal(renderer.resourceStatistics().animatedInstanceCount, 0);
  assert.doesNotThrow(() => renderer.applyDiff({ op: 'defineAnimatedMesh', asset }));
});

void test('animated mesh overrides are instance-owned, live-redefined, and capture-safe after source release', () => {
  const base = new THREE.MeshStandardMaterial({ color: 0xffffff });
  const scene = new THREE.Group();
  scene.add(new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1), base));
  const asset = animatedMeshAsset({
    clips: [{ id: 'idle', name: 'idle', durationSeconds: 1 }],
    embeddedMaterialSlots: [{ slot: 0, sourceMaterialSlot: 0 }],
  });
  const resource = {
    asset: asset.asset,
    scene,
    clips: [new THREE.AnimationClip('idle', 1, [])],
    embeddedMaterialSlots: new Map([[0, {
      sourceMaterialSlot: 0,
      materials: [base],
    }]]),
  };
  const instance = {
    asset: asset.asset,
    transform: { translation: [0, 0, 0] as const, rotation: [0, 0, 0, 1] as const, scale: [1, 1, 1] as const },
    visible: true,
    materialOverrides: [{ slot: 0, material: 'material/wood' }],
    playback: null,
    metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label: 'override' },
  };
  const registry = new AnimatedMeshRegistry(new MapAnimatedMeshAssetSource([resource]));
  registry.define(asset);
  const record = registry.create(renderHandle(880), instance, () => new THREE.MeshStandardMaterial({ color: 0xff0000 }));
  const live = firstMesh(record.object).material as THREE.Material;
  assert.notEqual(live, base, 'the override never mutates the admitted GLB material');
  let liveDisposed = false;
  live.addEventListener('dispose', () => { liveDisposed = true; });
  const capture = registry.createCaptureAppearance(renderHandle(880), 'idle', 0.5);
  const captured = firstMesh(capture.object).material as THREE.Material;
  assert.notEqual(captured, live, 'capture owns a clone of the source instance override');
  let captureDisposed = false;
  captured.addEventListener('dispose', () => { captureDisposed = true; });
  registry.release(renderHandle(880));
  assert.ok(liveDisposed, 'releasing the source disposes only its owned override');
  assert.equal(captureDisposed, false, 'capture material remains valid after source release');
  capture.dispose();
  assert.ok(captureDisposed, 'capture disposal releases its override clone');

  const renderer = new ThreeRenderer({ animatedMeshSource: new MapAnimatedMeshAssetSource([resource]) });
  renderer.applyFrame({ schemaVersion: 1, ops: [
    { op: 'defineMaterial', material: woodMaterial() },
    { op: 'defineAnimatedMesh', asset },
    { op: 'createAnimatedMeshInstance', handle: renderHandle(881), parent: null, instance },
  ] });
  const before = firstMesh(renderer.objectFor(renderHandle(881))!).material as THREE.MeshStandardMaterial;
  assert.ok(Math.abs(before.color.r - 0.6) < 1e-6);
  let redefinedDisposed = false;
  before.addEventListener('dispose', () => { redefinedDisposed = true; });
  renderer.applyDiff({
    op: 'defineMaterial',
    material: { ...woodMaterial(), color: [0.1, 0.8, 0.2, 1] },
  });
  const after = firstMesh(renderer.objectFor(renderHandle(881))!).material as THREE.MeshStandardMaterial;
  assert.notEqual(after, before);
  assert.ok(Math.abs(after.color.g - 0.8) < 1e-6, 'redefinition reaches the owned animated override');
  assert.ok(redefinedDisposed, 'the prior owned animated override is disposed');
});

// ── Sprites, billboards, and picking ───────────────────────────────────────────

function sparkSprite(over: Partial<SpriteInstanceDescriptor> = {}): SpriteInstanceDescriptor {
  return {
    asset: 'sprite/spark',
    frame: 0,
    pivot: [0.5, 0.5],
    size: [1, 1],
    sizeMode: 'world',
    billboard: 'spherical',
    tint: [1, 1, 1, 1],
    renderOrder: 0,
    depth: 'default',
    shading: 'unlit',
    visible: true,
    transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
    attachment: { sourceEntity: null, sourceSceneNode: null, attachmentPoint: null },
    metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label: 'spark' },
    ...over,
  };
}

void test('createSprite builds a plane geometry (not THREE.Sprite) with render order + depth policy', () => {
  const r = new ThreeRenderer();
  r.applyDiff({ op: 'createSprite', handle: renderHandle(1), parent: null, sprite: sparkSprite({ renderOrder: 7, depth: 'depthTestOff' }) });
  const mesh = r.objectFor(renderHandle(1)) as THREE.Mesh;
  assert.ok(mesh instanceof THREE.Mesh, 'sprite uses a Mesh + PlaneGeometry, not THREE.Sprite');
  assert.ok(mesh.geometry instanceof THREE.PlaneGeometry);
  assert.equal(mesh.renderOrder, 7);
  assert.equal((mesh.material as THREE.MeshBasicMaterial).depthTest, false);
});

void test('textureless legacy and explicit blend sprites retain distinct depth policy', () => {
  const renderer = new ThreeRenderer();
  renderer.applyDiff({
    op: 'createSprite', handle: renderHandle(1), parent: null, sprite: sparkSprite(),
  });
  renderer.applyDiff({
    op: 'createSprite',
    handle: renderHandle(2),
    parent: null,
    sprite: sparkSprite({
      material: {
        lighting: 'unlit', normalTexture: null, depthTexture: null,
        normalStrength: 1, normalBias: 0, alpha: { kind: 'blend' }, shadow: 'none',
      },
    }),
  });

  const legacy = (renderer.objectFor(renderHandle(1)) as THREE.Mesh).material as THREE.MeshBasicMaterial;
  const blend = (renderer.objectFor(renderHandle(2)) as THREE.Mesh).material as THREE.MeshBasicMaterial;
  assert.equal(legacy.transparent, false);
  assert.equal(legacy.depthWrite, true);
  assert.equal(blend.transparent, true);
  assert.equal(blend.depthWrite, false);
});

void test('sprite billboards face the active camera while none preserves the authored rotation', () => {
  const renderer = new ThreeRenderer();
  const parentRotation = new THREE.Quaternion().setFromEuler(new THREE.Euler(0, 0.45, 0));
  renderer.applyDiff({
    op: 'create',
    handle: renderHandle(20),
    parent: null,
    node: {
      geometry: { kind: 'group' },
      material: { color: [1, 1, 1, 1], wireframe: false },
      transform: {
        translation: [0, 0, 0],
        rotation: [parentRotation.x, parentRotation.y, parentRotation.z, parentRotation.w],
        scale: [1, 1, 1],
      },
      visible: true,
      layer: 'scene',
      metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label: 'billboard-parent' },
    },
  });
  const authoredRotation = new THREE.Quaternion().setFromEuler(new THREE.Euler(0.2, 0.3, 0.4));
  renderer.applyFrame({ schemaVersion: 1, ops: [
    {
      op: 'createSprite',
      handle: renderHandle(21),
      parent: renderHandle(20),
      sprite: sparkSprite({
        billboard: 'none',
        transform: {
          translation: [0, 0, 0],
          rotation: [authoredRotation.x, authoredRotation.y, authoredRotation.z, authoredRotation.w],
          scale: [1, 1, 1],
        },
      }),
    },
    {
      op: 'createSprite',
      handle: renderHandle(22),
      parent: renderHandle(20),
      sprite: sparkSprite({ billboard: 'spherical' }),
    },
    {
      op: 'createSprite',
      handle: renderHandle(23),
      parent: renderHandle(20),
      sprite: sparkSprite({ billboard: 'cylindrical' }),
    },
  ] });

  const camera = new THREE.PerspectiveCamera(55, 1, 0.1, 100);
  camera.position.set(4, 3, 6);
  camera.lookAt(0, 0, 0);
  camera.updateMatrixWorld(true);
  renderer.prepareSpritesForCamera(camera);

  const none = renderer.objectFor(renderHandle(21)) as THREE.Mesh;
  const spherical = renderer.objectFor(renderHandle(22)) as THREE.Mesh;
  const cylindrical = renderer.objectFor(renderHandle(23)) as THREE.Mesh;
  const noneWorld = none.getWorldQuaternion(new THREE.Quaternion());
  const authoredWorld = parentRotation.clone().multiply(authoredRotation);
  assert.ok(noneWorld.angleTo(authoredWorld) < 1e-6, 'none keeps authored world orientation');
  const cameraWorld = camera.getWorldQuaternion(new THREE.Quaternion());
  assert.ok(
    spherical.getWorldQuaternion(new THREE.Quaternion()).angleTo(cameraWorld) < 1e-6,
    'spherical copies the camera world orientation',
  );

  const cylindricalWorld = cylindrical.getWorldQuaternion(new THREE.Quaternion());
  const normal = new THREE.Vector3(0, 0, 1).applyQuaternion(cylindricalWorld).normalize();
  const toCamera = new THREE.Vector3(4, 0, 6).normalize();
  assert.ok(normal.distanceTo(toCamera) < 1e-6, 'cylindrical faces the camera around world Y');
  const up = new THREE.Vector3(0, 1, 0).applyQuaternion(cylindricalWorld).normalize();
  assert.ok(up.distanceTo(new THREE.Vector3(0, 1, 0)) < 1e-6, 'cylindrical keeps world Y upright');
});

void test('cylindrical billboards keep a valid orientation when horizontal distance is zero', () => {
  const renderer = new ThreeRenderer();
  const authored = new THREE.Quaternion().setFromEuler(new THREE.Euler(0.1, 0.2, 0.3));
  renderer.applyDiff({
    op: 'createSprite',
    handle: renderHandle(1),
    parent: null,
    sprite: sparkSprite({
      billboard: 'cylindrical',
      transform: {
        translation: [2, 0, 3],
        rotation: [authored.x, authored.y, authored.z, authored.w],
        scale: [1, 1, 1],
      },
    }),
  });
  const camera = new THREE.PerspectiveCamera();
  camera.position.set(2, 4, 3);
  camera.lookAt(2, 0, 3);
  camera.updateMatrixWorld(true);
  renderer.prepareSpritesForCamera(camera);
  const mesh = renderer.objectFor(renderHandle(1)) as THREE.Mesh;
  assert.ok(mesh.quaternion.toArray().every(Number.isFinite));
  const expectedYaw = new THREE.Vector3(0, 0, 1).applyQuaternion(authored);
  expectedYaw.y = 0;
  expectedYaw.normalize();
  const realizedNormal = new THREE.Vector3(0, 0, 1)
    .applyQuaternion(mesh.getWorldQuaternion(new THREE.Quaternion()))
    .normalize();
  assert.ok(realizedNormal.distanceTo(expectedYaw) < 1e-6);
});

void test('billboards recompute for each camera, including orthographic projection', () => {
  const renderer = new ThreeRenderer();
  renderer.applyDiff({
    op: 'createSprite',
    handle: renderHandle(1),
    parent: null,
    sprite: sparkSprite({ billboard: 'cylindrical' }),
  });

  const perspective = new THREE.PerspectiveCamera(55, 1, 0.1, 100);
  perspective.position.set(4, 2, 6);
  perspective.lookAt(0, 0, 0);
  const alternate = new THREE.PerspectiveCamera(55, 1, 0.1, 100);
  alternate.position.set(-5, 3, 4);
  alternate.lookAt(0, 0, 0);
  renderer.prepareSpritesForCamera(perspective);
  const first = (renderer.objectFor(renderHandle(1)) as THREE.Mesh)
    .getWorldQuaternion(new THREE.Quaternion());
  renderer.prepareSpritesForCamera(alternate);
  const second = (renderer.objectFor(renderHandle(1)) as THREE.Mesh)
    .getWorldQuaternion(new THREE.Quaternion());
  assert.ok(first.angleTo(second) > 0.1, 'camera movement changes the realized heading');
  renderer.prepareSpritesForCamera(perspective);
  const restored = (renderer.objectFor(renderHandle(1)) as THREE.Mesh)
    .getWorldQuaternion(new THREE.Quaternion());
  assert.ok(first.angleTo(restored) < 1e-6, 'A → B → A restores the first realization');

  const orthographic = new THREE.OrthographicCamera(-2, 2, 2, -2, 0.1, 100);
  orthographic.position.set(4, 5, 6);
  orthographic.lookAt(0, 0, 0);
  renderer.prepareSpritesForCamera(orthographic);
  const realizedNormal = new THREE.Vector3(0, 0, 1)
    .applyQuaternion((renderer.objectFor(renderHandle(1)) as THREE.Mesh)
      .getWorldQuaternion(new THREE.Quaternion()))
    .normalize();
  const viewDirection = orthographic.getWorldDirection(new THREE.Vector3()).negate();
  viewDirection.y = 0;
  viewDirection.normalize();
  assert.ok(realizedNormal.distanceTo(viewDirection) < 1e-6, 'orthographic uses view direction');
});

void test('sprite frame/tint updates are deterministic and projection-driven', () => {
  const r = new ThreeRenderer();
  r.applyDiff({ op: 'createSprite', handle: renderHandle(1), parent: null, sprite: sparkSprite() });
  r.applyDiff({ op: 'updateSprite', handle: renderHandle(1), frame: 4, tint: [1, 0, 0, 1], renderOrder: 2, visible: false });

  const mesh = r.objectFor(renderHandle(1)) as THREE.Mesh;
  assert.equal(mesh.userData['frame'], 4);
  assert.equal(mesh.renderOrder, 2);
  assert.equal(mesh.visible, false);
  const c = (mesh.material as THREE.MeshBasicMaterial).color;
  assert.deepEqual([c.r, c.g, c.b], [1, 0, 0]);
});

void test('reserved lit/shadow shading is accepted (renderer does not force unlit-only)', () => {
  const r = new ThreeRenderer();
  assert.doesNotThrow(() =>
    r.applyDiff({ op: 'createSprite', handle: renderHandle(1), parent: null, sprite: sparkSprite({ shading: 'lit' }) }),
  );
});

void test('pickSprite traces to source entity / scene node / asset, never a render handle as authority', () => {
  const r = new ThreeRenderer();
  r.applyDiff({
    op: 'createSprite',
    handle: renderHandle(5),
    parent: null,
    sprite: sparkSprite({ attachment: { sourceEntity: 42, sourceSceneNode: 9, attachmentPoint: 'muzzle' } }),
  });
  const hit = r.pickSprite(renderHandle(5));
  assert.ok(hit);
  assert.equal(hit!.handle, renderHandle(5));
  assert.equal(hit!.sourceEntity, 42);
  assert.equal(hit!.sourceSceneNode, 9);
  assert.equal(hit!.asset, 'sprite/spark');
  assert.equal(hit!.attachmentPoint, 'muzzle');
  // A non-sprite handle yields no pick hit.
  assert.equal(r.pickSprite(renderHandle(99)), undefined);
});

// ── Large-payload lifecycle and resource cleanup ──────────────────────────────

/** Generate a non-trivially large triangle-strip mesh's inline streams. */
function bigMeshStreams(vertexCount: number): {
  positions: number[];
  normals: number[];
  indices: number[];
} {
  const positions: number[] = [];
  const normals: number[] = [];
  for (let i = 0; i < vertexCount; i++) {
    positions.push(i, i * 0.5, 0);
    normals.push(0, 0, 1);
  }
  const indices: number[] = [];
  for (let i = 0; i + 2 < vertexCount; i++) {
    indices.push(i, i + 1, i + 2);
  }
  return { positions, normals, indices };
}

/** Pack inline streams into one `[positions|normals|indices]` little-endian blob. */
function packStreams(streams: {
  positions: number[];
  normals: number[];
  indices: number[];
}): Uint8Array {
  const { positions, normals, indices } = streams;
  const bytes = new Uint8Array((positions.length + normals.length + indices.length) * 4);
  const dv = new DataView(bytes.buffer);
  let offset = 0;
  for (const v of positions) {
    dv.setFloat32(offset, v, true);
    offset += 4;
  }
  for (const v of normals) {
    dv.setFloat32(offset, v, true);
    offset += 4;
  }
  for (const v of indices) {
    dv.setUint32(offset, v, true);
    offset += 4;
  }
  return bytes;
}

/** A shared-buffer payload sized for arbitrary vertex/index counts. */
function bigMeshPayload(buffer: number, vertexCount: number, indexCount: number): MeshPayloadDescriptor {
  return {
    layout: {
      vertexCount,
      indexCount,
      indexWidth: 'u32',
      attributes: [
        { name: 'position', components: 3, kind: 'f32' },
        { name: 'normal', components: 3, kind: 'f32' },
      ],
    },
    groups: [{ materialSlot: 1, start: 0, count: indexCount }],
    bounds: { min: [0, 0, 0], max: [vertexCount, vertexCount, 0] },
    source: {
      kind: 'sharedBuffer',
      buffer,
      positionsByteOffset: 0,
      normalsByteOffset: vertexCount * 3 * 4,
      indicesByteOffset: vertexCount * 3 * 4 * 2,
    },
    provenance: 'voxelChunk',
  };
}

void test('large shared-buffer payload uploads with the declared counts', () => {
  const vertexCount = 4096;
  const streams = bigMeshStreams(vertexCount);
  const source = new MapBufferSource();
  source.set(10, packStreams(streams));
  const r = new ThreeRenderer({ meshBufferSource: source });
  const h = renderHandle(1);
  r.applyDiff({ op: 'create', handle: h, parent: null, node: meshNode() });
  r.applyDiff({
    op: 'replaceMeshPayload',
    handle: h,
    payload: bigMeshPayload(10, vertexCount, streams.indices.length),
  });
  const geo = (r.objectFor(h) as THREE.Mesh).geometry;
  assert.equal(geo.getAttribute('position').count, vertexCount);
  assert.equal(geo.getIndex()!.count, streams.indices.length);
});

void test('create/replace/destroy/invalidate cycle leaves no leaked geometry and stable diagnostics', () => {
  const source = new MapBufferSource();
  source.set(1, quadHandleBytes());
  source.set(2, quadHandleBytes());
  source.set(3, quadHandleBytes());
  const r = new ThreeRenderer({ meshBufferSource: source });

  // Upload three shared-buffer meshes.
  const handles = [renderHandle(1), renderHandle(2), renderHandle(3)];
  for (const h of handles) {
    r.applyDiff({ op: 'create', handle: h, parent: null, node: meshNode() });
    r.applyDiff({ op: 'replaceMeshPayload', handle: h, payload: quadHandlePayload((h as number)) });
  }
  for (const h of handles) {
    assert.ok(r.has(h));
  }

  // Replace one: its previous uploaded geometry must be disposed (no leak).
  const replaced = (r.objectFor(handles[0]!) as THREE.Mesh).geometry;
  let replacedDisposed = false;
  replaced.addEventListener('dispose', () => {
    replacedDisposed = true;
  });
  r.applyDiff({ op: 'replaceMeshPayload', handle: handles[0]!, payload: quadHandlePayload(1) });
  assert.ok(replacedDisposed, 'replaced geometry should be disposed');

  // Destroy one: handle freed and its geometry disposed.
  const destroyed = (r.objectFor(handles[1]!) as THREE.Mesh).geometry;
  let destroyedDisposed = false;
  destroyed.addEventListener('dispose', () => {
    destroyedDisposed = true;
  });
  r.applyDiff({ op: 'destroy', handle: handles[1]! });
  assert.ok(!r.has(handles[1]!));
  assert.ok(destroyedDisposed, 'destroyed geometry should be disposed');

  // Invalidate the third buffer in the provider: a re-upload referencing it fails
  // closed with a stable, source-linked diagnostic, and the node keeps its prior
  // geometry (no partial mutation).
  const survivor = (r.objectFor(handles[2]!) as THREE.Mesh).geometry;
  source.expire(3);
  assert.throws(
    () => r.applyDiff({ op: 'replaceMeshPayload', handle: handles[2]!, payload: quadHandlePayload(3) }),
    /buffer 3 unavailable \[expired\]/,
  );
  assert.equal((r.objectFor(handles[2]!) as THREE.Mesh).geometry, survivor, 'failed upload must not swap geometry');

  // Final state: handles 1 and 3 survive, handle 2 destroyed.
  assert.ok(r.has(handles[0]!));
  assert.ok(!r.has(handles[1]!));
  assert.ok(r.has(handles[2]!));
});

function firstMesh(root: THREE.Object3D): THREE.Mesh {
  let selected: THREE.Mesh | null = null;
  root.traverse((object) => {
    if (selected === null && object instanceof THREE.Mesh) {
      selected = object;
    }
  });
  assert.ok(selected);
  return selected;
}
