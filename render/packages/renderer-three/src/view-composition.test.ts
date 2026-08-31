import assert from 'node:assert/strict';
import { test } from 'node:test';

import * as THREE from 'three';
import type { RendererViewComposition } from '@rusty-engine/render-contracts';

import type { ThreeRenderer } from './three-renderer.js';
import { RendererViewCompositionBackend } from './view-composition.js';

function composition(revision = 1, width = 64): RendererViewComposition {
  return {
    schemaVersion: 1,
    cameras: [{
      id: 'camera.overview',
      pose: { position: [0, 12, 0], pitchDegrees: -90, yawDegrees: 0 },
      projection: { kind: 'orthographic', verticalSize: 20, near: 0.1, far: 50 },
    }],
    targets: [{
      id: 'target.overview',
      revision,
      width,
      height: 64,
      color: 'rgba8_srgb',
      depth: 'depth24',
      sampling: 'nearest',
    }],
    views: [{
      id: 'view.overview',
      cameraId: 'camera.overview',
      target: { kind: 'offscreen', targetId: 'target.overview', targetRevision: revision },
      viewport: { x: 0, y: 0, width: 1, height: 1 },
      order: 1,
    }],
    presentations: [{
      id: 'presentation.overview',
      sourceTargetId: 'target.overview',
      sourceTargetRevision: revision,
      destination: {
        kind: 'primary',
        viewport: { x: 0.7, y: 0.7, width: 0.25, height: 0.25 },
      },
      order: 2,
    }],
  };
}

function backend(
  initRenderTarget: (target: THREE.WebGLRenderTarget) => void = () => undefined,
): RendererViewCompositionBackend {
  return new RendererViewCompositionBackend(
    { initRenderTarget } as unknown as THREE.WebGLRenderer,
    {} as ThreeRenderer,
  );
}

function primaryComposition(
  position: readonly [number, number, number],
  yawDegrees: number,
): RendererViewComposition {
  return {
    schemaVersion: 1,
    cameras: [{
      id: 'camera.primary',
      pose: { position, pitchDegrees: 0, yawDegrees },
      projection: { kind: 'perspective', fovYDegrees: 55, near: 0.1, far: 100 },
    }],
    targets: [],
    views: [{
      id: 'view.primary',
      cameraId: 'camera.primary',
      target: { kind: 'primary' },
      viewport: { x: 0, y: 0, width: 1, height: 1 },
      order: 1,
    }],
    presentations: [],
  };
}

void test('composition publication is immutable and target revisions cannot be resurrected', () => {
  const manager = backend();
  const submitted = composition();
  assert.deepEqual(manager.configure(submitted), {
    applied: true,
    diagnostics: [],
    revision: 1,
  });
  (submitted.cameras[0] as { id: string }).id = 'camera.mutated-after-submit';
  const first = manager.readout();
  assert.equal(first.cameras[0]?.id, 'camera.overview');
  assert.equal(Object.isFrozen(first), true);
  assert.equal(Object.isFrozen(first.cameras), true);
  assert.equal(Object.isFrozen(first.cameras[0]), true);
  assert.equal(first.targets[0]?.status, 'never_rendered');
  assert.deepEqual(first.resources, { presentationCount: 1, targetCount: 1 });

  assert.equal(manager.configure({
    schemaVersion: 1,
    cameras: [],
    targets: [],
    views: [],
    presentations: [],
  }).applied, true);
  const stale = manager.configure(composition());
  assert.equal(stale.applied, false);
  assert.equal(stale.diagnostics[0]?.code, 'stale_target_revision');
  assert.deepEqual(manager.readout().targets, []);
});

void test('composition visibility preserves configured view and camera identity', () => {
  const visibility = {
    schemaVersion: 1 as const,
    basis: 'cpuFrustum' as const,
    occlusion: 'notMeasured' as const,
    handles: [],
  };
  const manager = new RendererViewCompositionBackend(
    { initRenderTarget: () => undefined } as unknown as THREE.WebGLRenderer,
    {
      scene: {},
      visibilityReadout: () => visibility,
    } as unknown as ThreeRenderer,
  );
  assert.equal(manager.configure(composition()).applied, true);
  assert.deepEqual(manager.visibilityReadout(), {
    schemaVersion: 1,
    views: [{
      viewId: 'view.overview',
      cameraId: 'camera.overview',
      target: 'offscreen',
      visibility,
    }],
  });
});

void test('composed primary views keep viewmodel transforms camera-relative across world poses', () => {
  const worldScene = new THREE.Scene();
  const viewmodelScene = new THREE.Scene();
  const viewmodelCamera = new THREE.PerspectiveCamera();
  const localPoint = new THREE.Vector3(0.25, -0.2, -1.5);
  const projected: THREE.Vector3[] = [];
  const worldCameras: THREE.Camera[] = [];
  const webgl = {
    initRenderTarget: () => undefined,
    getPixelRatio: () => 1,
    setRenderTarget: () => undefined,
    setScissorTest: () => undefined,
    setViewport: () => undefined,
    setScissor: () => undefined,
    clear: () => undefined,
    clearDepth: () => undefined,
    render: (scene: THREE.Scene, camera: THREE.Camera) => {
      if (scene === worldScene) worldCameras.push(camera);
      if (scene === viewmodelScene) projected.push(localPoint.clone().project(camera));
    },
  } as unknown as THREE.WebGLRenderer;
  const projection = {
    scene: worldScene,
    viewmodelScene,
    prepareSpritesForCamera: () => undefined,
    prepareStaticInstanceBatches: () => undefined,
  } as unknown as ThreeRenderer;
  const manager = new RendererViewCompositionBackend(webgl, projection, viewmodelCamera);

  assert.equal(manager.configure(primaryComposition([48, 6, -32], 67)).applied, true);
  manager.render(1, 800, 600);
  assert.equal(manager.configure(primaryComposition([-64, 9, 72], -121)).applied, true);
  manager.render(2, 800, 600);

  assert.equal(worldCameras.length, 2);
  assert.ok(worldCameras[0]!.position.length() > 16);
  assert.ok(worldCameras[0]!.quaternion.angleTo(new THREE.Quaternion()) > 0.1);
  assert.equal(projected.length, 2);
  assert.ok(projected[0]!.distanceTo(projected[1]!) < 1e-12);
  assert.deepEqual(viewmodelCamera.position.toArray(), [0, 0, 0]);
  assert.ok(viewmodelCamera.quaternion.angleTo(new THREE.Quaternion()) < 1e-12);
});

void test('changed target facts require a higher revision and publish exactly once', () => {
  const initialized: THREE.WebGLRenderTarget[] = [];
  let disposed = 0;
  const manager = backend((target) => initialized.push(target));
  assert.equal(manager.configure(composition()).applied, true);
  initialized[0]!.addEventListener('dispose', () => { disposed += 1; });

  const sameRevisionResize = manager.configure(composition(1, 128));
  assert.equal(sameRevisionResize.applied, false);
  assert.equal(sameRevisionResize.diagnostics[0]?.code, 'stale_target_revision');
  assert.equal(manager.readout().revision, 1);
  assert.equal(manager.readout().targets[0]?.width, 64);

  assert.equal(manager.configure(composition(2, 128)).applied, true);
  assert.equal(manager.readout().revision, 2);
  assert.equal(manager.readout().targets[0]?.width, 128);
  assert.equal(initialized.length, 2);
  assert.equal(disposed, 1);
  initialized[1]!.addEventListener('dispose', () => { disposed += 1; });
  manager.dispose();
  assert.equal(disposed, 2);
});

void test('target allocation failure leaves the prior composition unchanged', () => {
  let failAllocation = false;
  const manager = backend(() => {
    if (failAllocation) throw new Error('simulated allocation failure');
  });
  assert.equal(manager.configure(composition()).applied, true);
  const before = manager.readout();

  failAllocation = true;
  const rejected = manager.configure(composition(2, 128));
  assert.equal(rejected.applied, false);
  assert.equal(rejected.diagnostics[0]?.code, 'target_allocation_failed');
  assert.deepEqual(manager.readout(), before);
});
