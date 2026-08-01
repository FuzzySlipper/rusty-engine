// Runtime tests for the Three.js renderer shell, run with `node --test`.
// The scene graph is built without a GL context (no rendering), so these assert
// registry/scene-graph state directly.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { renderHandle, type AnimatedMeshAsset, type RenderDiff, type RenderNode } from '@rusty-engine/render-contracts';
import {
  MapAnimatedMeshAssetSource,
  RenderApplyError,
  RenderResourceError,
  ThreeRenderer,
  loadAnimatedMeshGlbResource,
  type MeshBufferView,
  type MeshBufferSource,
  type MeshResourceSource,
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

void test('realizes every operation in the comprehensive Rust-authored retained fixture', () => {
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

void test('defineMaterial maps a static-mesh slot to its defined colour, not a placeholder', () => {
  const r = new ThreeRenderer();
  r.applyDiff({ op: 'defineMaterial', material: woodMaterial() });
  assert.deepEqual(r.materialDescriptor('material/wood')?.color, [0.6, 0.4, 0.2, 1]);

  // Define a single-slot mesh bound to material/wood, then instance it.
  r.applyDiff({
    op: 'defineStaticMesh',
    asset: {
      asset: 'mesh/plank',
      payload: { ...quadPayload(), provenance: 'staticAsset' },
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
      payload: { ...quadPayload(), provenance: 'staticAsset' },
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
      { frame: 3, uvMin: [0.5, 0], uvMax: [1, 1] },
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

void test('a sprite frame maps to its atlas UV sub-rectangle deterministically', () => {
  const r = new ThreeRenderer();
  r.applyDiff({ op: 'defineTexture', texture: sparkTexture() });
  r.applyDiff({ op: 'defineSpriteAtlas', atlas: sparkAtlas() });
  assert.equal(r.textureDescriptor('texture/spark')?.width, 64);
  assert.equal(r.spriteAtlas('sprite/spark-sheet')?.frames.length, 2);

  r.applyDiff({ op: 'createSprite', handle: renderHandle(1), parent: null, sprite: atlasSprite(0) });
  assert.deepEqual(spriteUv(r, 1), [0, 0, 0.5, 1], 'frame 0 → left half');

  // Advancing the frame re-resolves the UV rect deterministically.
  r.applyDiff({ op: 'updateSprite', handle: renderHandle(1), frame: 3, tint: null, renderOrder: null, visible: null });
  assert.deepEqual(spriteUv(r, 1), [0.5, 0, 1, 1], 'frame 3 → right half');
  assert.equal(r.spriteFallbackCount, 0, 'known frames are not fallbacks');
});

void test('a sprite frame with no atlas frame falls back to full UVs and is counted', () => {
  const r = new ThreeRenderer();
  r.applyDiff({ op: 'defineTexture', texture: sparkTexture() });
  r.applyDiff({ op: 'defineSpriteAtlas', atlas: sparkAtlas() });
  r.applyDiff({ op: 'createSprite', handle: renderHandle(1), parent: null, sprite: atlasSprite(9) });
  assert.deepEqual(spriteUv(r, 1), [0, 0, 1, 1], 'unknown frame → full UVs');
  assert.equal(r.spriteFallbackCount, 1);
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
      { id: 'idle', name: 'Idle', durationSeconds: 1.04166662693024 },
      { id: 'run', name: 'Run', durationSeconds: 0.666666686534882 },
    ],
    defaultClip: 'idle',
    materialSlots: [],
    bounds: { min: [-0.5, 0, -0.5], max: [0.5, 1.8, 0.5] },
    ...over,
  };
}

function testAnimatedMeshSource(asset = animatedMeshAsset()): MapAnimatedMeshAssetSource {
  const scene = new THREE.Group();
  scene.name = 'animated-fixture-root';
  const clips = asset.clips.map((clip) => {
    const duration = clip.durationSeconds ?? 1;
    const tracks =
      clip.id === 'run' ? [new THREE.VectorKeyframeTrack('.position', [0, duration], [0, 0, 0, 1, 0, 0])] : [];
    return new THREE.AnimationClip(clip.id, duration, tracks);
  });
  return new MapAnimatedMeshAssetSource([{ asset: asset.asset, contentHash: asset.contentHash, scene, clips }]);
}

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
    const resource = await loadAnimatedMeshGlbResource('mesh-animation/kenney-retro-character-medium', data);
    assert.deepEqual(
      resource.clips.map((clip) => clip.name).sort(),
      ['idle', 'jump', 'run'],
    );
    const asset = animatedMeshAsset();
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
    assert.notEqual(idleSample.sampledWorldBounds, null);
    assert.deepEqual(idleSample.diagnostics, []);
    assert.deepEqual(runSample.diagnostics, []);
    assert.equal(runBefore?.currentClip, 'run');
    assert.equal(renderer.animatedMeshPlayback(renderHandle(4098))?.status, 'paused');
    assert.equal(renderer.animatedMeshPlayback(renderHandle(4099))?.status, 'paused');
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
    playback: { kind: 'play', clip: 'run', loop: 'repeat', speed: 1, weight: 1, restart: true, fadeSeconds: null },
  });
  const selected = r.animatedMeshPlayback(handle);
  assert.equal(selected?.currentClip, 'run');
  assert.equal(selected?.commandSelected, true);
  assert.equal(selected?.loop, 'repeat');
  assert.equal(selected?.status, 'playing');
  assert.deepEqual(selected?.diagnostics, []);

  r.advanceAnimation(0.25);
  const advanced = r.animatedMeshPlayback(handle);
  assert.equal(advanced?.currentClip, 'run');
  assert.equal(advanced?.running, true);
  assert.ok((advanced?.mixerTimeSeconds ?? 0) > 0);
  assert.ok((advanced?.actionTimeSeconds ?? 0) > 0);
  assert.notDeepEqual(advanced?.poseSample.rootTranslation, selected?.poseSample.rootTranslation);
  assert.ok((advanced?.poseSample.rootTranslation[0] ?? 0) > (selected?.poseSample.rootTranslation[0] ?? 0));

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
      { asset: asset.asset, scene: new THREE.Group(), clips: [new THREE.AnimationClip('idle', 1, [])] },
    ]),
  });
  assert.throws(
    () => wrongClips.applyDiff({ op: 'defineAnimatedMesh', asset }),
    /does not contain clip run/,
  );

  const wrongHash = new ThreeRenderer({ animatedMeshSource: testAnimatedMeshSource(asset) });
  assert.throws(
    () => wrongHash.applyDiff({ op: 'defineAnimatedMesh', asset: animatedMeshAsset({ contentHash: 'sha256:wrong' }) }),
    /content hash mismatch/,
  );
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
