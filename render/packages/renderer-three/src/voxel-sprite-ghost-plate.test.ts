import assert from 'node:assert/strict';
import test from 'node:test';

import * as THREE from 'three';

import {
  GhostPlatePresentation,
  evaluateGhostPlateShell,
  warpGhostCameraPoint,
} from './voxel-sprite-ghost-plate.js';

for (const fov of [35, 75, 110]) {
  for (const aspect of [0.75, 1]) {
    void test(`perspective ghost compression preserves source ndc at fov ${fov} aspect ${aspect}`, () => {
      const projection = new THREE.PerspectiveCamera(fov, aspect, 0.1, 100).projectionMatrix;
      for (const retention of [0.02, 0.3, 1]) {
        const source: readonly [number, number, number] = [0.7, -0.35, -4.25];
        const warped = warpGhostCameraPoint(source, projection, 'perspective', 3, retention);
        const warpedClip = new THREE.Vector4(...warped.position, 1).applyMatrix4(projection);
        assert.ok(Math.abs(warpedClip.x / warpedClip.w - warped.sourceNdc[0]) < 2e-6);
        assert.ok(Math.abs(warpedClip.y / warpedClip.w - warped.sourceNdc[1]) < 2e-6);
      }
    });
  }
}

void test('asymmetric perspective projection remains invariant', () => {
  const projection = new THREE.Matrix4().makePerspective(-0.7, 1.1, 0.9, -0.8, 0.1, 50);
  const source: readonly [number, number, number] = [-0.4, 0.2, -5];
  const warped = warpGhostCameraPoint(source, projection, 'perspective', 2.5, 0.12);
  const clip = new THREE.Vector4(...warped.position, 1).applyMatrix4(projection);
  assert.ok(Math.abs(clip.x / clip.w - warped.sourceNdc[0]) < 2e-6);
  assert.ok(Math.abs(clip.y / clip.w - warped.sourceNdc[1]) < 2e-6);
});

void test('source shell admission includes unorm8 precision and bounded one-texel repair', () => {
  const center = { depth: 0.5, coverage: 1 };
  assert.deepEqual(
    evaluateGhostPlateShell(5.019, center, [], 0, 10, 'strict-source', 0),
    { accepted: true, repaired: false },
  );
  assert.deepEqual(
    evaluateGhostPlateShell(6, center, [], 0, 10, 'strict-source', 0.1),
    { accepted: false, repaired: false },
  );
  assert.deepEqual(
    evaluateGhostPlateShell(6, center, [{ depth: 0.6, coverage: 1 }], 0, 10, 'repaired-source', 0.1),
    { accepted: true, repaired: true },
  );
  assert.deepEqual(
    evaluateGhostPlateShell(6, center, [{ depth: 0.6, coverage: 0 }], 0, 10, 'repaired-source', 0.1),
    { accepted: false, repaired: false },
  );
  assert.deepEqual(
    evaluateGhostPlateShell(100, { depth: 0, coverage: 0 }, [], 0, 10, 'whole-mesh', 0),
    { accepted: true, repaired: false },
  );
});

void test('orthographic compression changes only camera-space depth', () => {
  const camera = new THREE.OrthographicCamera(-2, 2, 3, -3, 0.1, 50);
  const warped = warpGhostCameraPoint([0.8, -1.2, -7], camera.projectionMatrix, 'orthographic', 3, 0.2);
  assert.equal(warped.position[0], 0.8);
  assert.equal(warped.position[1], -1.2);
  assert.equal(warped.position[2], -3.8);
});

void test('cpu warp fails closed for invalid depth and retention', () => {
  const projection = new THREE.PerspectiveCamera().projectionMatrix;
  assert.throws(
    () => warpGhostCameraPoint([0, 0, 1], projection, 'perspective', 2, 0.2),
    /in front/,
  );
  assert.throws(
    () => warpGhostCameraPoint([0, 0, -2], projection, 'perspective', 2, 0.01),
    /0.02/,
  );
});

void test('presentation owns ghost materials but borrows geometry and capture textures', () => {
  const geometry = new THREE.BoxGeometry(1, 2, 1);
  const sourceMaterial = new THREE.MeshStandardMaterial({ color: 0x88aaff });
  let sourceMaterialDisposeCount = 0;
  sourceMaterial.addEventListener('dispose', () => { sourceMaterialDisposeCount += 1; });
  const appearance = new THREE.Group();
  const mesh = new THREE.Mesh(geometry, sourceMaterial);
  appearance.add(mesh);
  appearance.updateMatrixWorld(true);
  const color = new THREE.Texture();
  const coverage = new THREE.Texture();
  const depth = new THREE.Texture();
  let colorDisposeCount = 0;
  let coverageDisposeCount = 0;
  color.addEventListener('dispose', () => { colorDisposeCount += 1; });
  coverage.addEventListener('dispose', () => { coverageDisposeCount += 1; });
  const camera = new THREE.PerspectiveCamera(35, 1, 0.1, 20);
  camera.position.set(0, 0, 5);
  camera.updateMatrixWorld(true);
  const presentation = new GhostPlatePresentation({
    appearanceRoot: appearance,
    colorTexture: color,
    coverageTexture: coverage,
    depthTexture: depth,
    textureWidth: 128,
    textureHeight: 128,
    captureNear: 0.1,
    captureFar: 20,
    projectionKind: 'perspective',
    ghostCameraWorld: camera.matrixWorld.clone(),
    ghostProjection: camera.projectionMatrix.clone(),
    bounds: new THREE.Box3().setFromObject(appearance, true),
    transform: { position: [1, 2, 3], width: 2, height: 3 },
    config: {
      depthRetention: 0.12,
      anchorPolicy: 'bounds-center',
      anchorValue: 0.5,
      plateMapping: 'plate-locked',
      shellMode: 'whole-mesh',
      shellDepthEpsilon: 0.12,
    },
  });
  assert.notEqual(mesh.material, sourceMaterial);
  assert.equal(presentation.readout().meshCount, 1);
  assert.equal(presentation.readout().matchedPose, true);
  presentation.configure({
    depthRetention: 1,
    anchorPolicy: 'bounds-normalized',
    anchorValue: 0,
    plateMapping: 'projective-surface',
    shellMode: 'repaired-source',
    shellDepthEpsilon: 0.2,
  });
  assert.equal(presentation.readout().depthRetention, 1);
  assert.equal(presentation.readout().shellMode, 'repaired-source');
  assert.equal(presentation.readout().borrowedTextureCount, 3);
  assert.equal(presentation.readout().rejectedFragmentRatio.status, 'unavailable');
  presentation.dispose();
  assert.equal(sourceMaterialDisposeCount, 0);
  assert.equal(colorDisposeCount, 0);
  assert.equal(coverageDisposeCount, 0);
  assert.equal(geometry.attributes['position']?.count, 24);
  sourceMaterial.dispose();
  geometry.dispose();
  color.dispose();
  coverage.dispose();
  depth.dispose();
});
