import { test } from 'node:test';
import assert from 'node:assert/strict';

import * as THREE from 'three';
import { renderHandle, type RenderFrameDiff } from '@rusty-engine/render-contracts';
import { pickProjectedObject, projectWorldPointWithPerspectiveCamera } from './browser-surface.js';
import { ThreeRenderer } from './three-renderer.js';

const PICK_HANDLE = renderHandle(7701);
const PICK_SOURCE = 91;
const PICK_TAG = 'pickable';

function pickFrame(): RenderFrameDiff {
  return {
    schemaVersion: 1,
    ops: [{
      op: 'create',
      handle: PICK_HANDLE,
      parent: null,
      node: {
        geometry: { kind: 'cube' },
        layer: 'scene',
        material: { color: [0.2, 0.4, 0.6, 1], wireframe: false },
        metadata: {
          label: 'projection-object',
          sourceEntity: PICK_SOURCE,
          sourceSceneNode: null,
          tags: [PICK_TAG],
        },
        transform: {
          rotation: [0, 0, 0, 1],
          scale: [1, 1, 1],
          translation: [0, 0, -5],
        },
        visible: true,
      },
    }],
  };
}

function pickAtCenter(renderer: ThreeRenderer) {
  const camera = new THREE.PerspectiveCamera(70, 1, 0.05, 100);
  camera.position.set(0, 0, 0);
  camera.lookAt(0, 0, -1);
  camera.updateMatrixWorld(true);
  return pickProjectedObject(
    renderer,
    camera,
    new THREE.Raycaster(),
    new THREE.Vector2(),
    {
      filter: { handles: [PICK_HANDLE], labels: ['projection-object'], layers: ['scene'], tags: [PICK_TAG] },
      maxDistance: 10,
      ray: { kind: 'viewport', point: [0, 0] },
    },
  );
}

void test('projection pick returns typed source evidence without changing projection or authority-shaped state', () => {
  const renderer = new ThreeRenderer();
  renderer.applyFrame(pickFrame());
  const object = renderer.objectFor(PICK_HANDLE) as THREE.Mesh;
  const material = object.material as THREE.MeshBasicMaterial;
  const beforeSnapshot = renderer.snapshot();
  const beforeVisible = object.visible;
  const beforeColor = material.color.getHex();
  const authorityReadout = Object.freeze({
    health: 40,
    hitCount: 0,
    sessionHash: 'fnv1a64:authority-does-not-enter-render-picking',
    shotCount: 0,
  });

  const receipt = pickAtCenter(renderer);

  assert.equal(receipt.kind, 'rusty_renderer_browser_surface_pick.v1');
  assert.deepEqual(receipt.diagnostics, []);
  assert.equal(receipt.hit?.channel, 'render_projection');
  assert.equal(receipt.hit?.handle, PICK_HANDLE);
  assert.equal(receipt.hit?.layer, 'scene');
  assert.equal(receipt.hit?.label, 'projection-object');
  assert.deepEqual(receipt.hit?.sourceTrace, { entity: PICK_SOURCE, kind: 'render_metadata_entity' });
  assert.deepEqual(receipt.hit?.tags, [PICK_TAG]);
  assert.equal(receipt.hit?.distance, 4.5);
  assert.deepEqual(authorityReadout, {
    health: 40,
    hitCount: 0,
    sessionHash: 'fnv1a64:authority-does-not-enter-render-picking',
    shotCount: 0,
  });
  assert.equal(renderer.snapshot(), beforeSnapshot);
  assert.equal(object.visible, beforeVisible);
  assert.equal(material.color.getHex(), beforeColor);
});

void test('projection pick filters are descriptive only and invalid rays fail closed', () => {
  const renderer = new ThreeRenderer();
  renderer.applyFrame(pickFrame());
  const beforeSnapshot = renderer.snapshot();

  const filtered = pickProjectedObject(
    renderer,
    new THREE.PerspectiveCamera(70, 1, 0.05, 100),
    new THREE.Raycaster(),
    new THREE.Vector2(),
    {
      filter: { labels: ['some-other-projection-object'] },
      ray: { direction: [0, 0, -1], kind: 'world_ray', origin: [0, 0, 0] },
    },
  );
  assert.equal(filtered.hit, null);
  assert.deepEqual(filtered.diagnostics, []);

  const rejected = pickProjectedObject(
    renderer,
    new THREE.PerspectiveCamera(70, 1, 0.05, 100),
    new THREE.Raycaster(),
    new THREE.Vector2(),
    { ray: { kind: 'viewport', point: [1.01, 0] } },
  );
  assert.equal(rejected.hit, null);
  assert.equal(rejected.diagnostics[0]?.code, 'invalid_viewport_point');

  const oversizedFilter = pickProjectedObject(
    renderer,
    new THREE.PerspectiveCamera(70, 1, 0.05, 100),
    new THREE.Raycaster(),
    new THREE.Vector2(),
    {
      filter: { labels: Array.from({ length: 129 }, (_, index) => `projection-${index}`) },
      ray: { direction: [0, 0, -1], kind: 'world_ray', origin: [0, 0, 0] },
    },
  );
  assert.equal(oversizedFilter.hit, null);
  assert.equal(oversizedFilter.diagnostics[0]?.code, 'filter_limit_exceeded');
  assert.equal(renderer.snapshot(), beforeSnapshot);
});

void test('world projection uses the configured Three perspective camera', () => {
  const viewport = { width: 1_000, height: 500 };
  const narrow = new THREE.PerspectiveCamera(58, 2, 0.1, 100);
  const wide = new THREE.PerspectiveCamera(90, 2, 0.1, 100);
  for (const camera of [narrow, wide]) {
    camera.position.set(0, 0, 0);
    camera.lookAt(0, 0, -1);
    camera.updateMatrixWorld(true);
    camera.updateProjectionMatrix();
  }

  const narrowProjection = projectWorldPointWithPerspectiveCamera(
    narrow,
    viewport,
    [2, 0, -5],
  );
  const wideProjection = projectWorldPointWithPerspectiveCamera(
    wide,
    viewport,
    [2, 0, -5],
  );

  assert.equal(wide.fov, 90);
  assert.ok(wideProjection.xPixels < narrowProjection.xPixels);
  assert.equal(wideProjection.insideViewport, true);
});
