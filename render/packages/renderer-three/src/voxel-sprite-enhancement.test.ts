import assert from 'node:assert/strict';
import test from 'node:test';
import * as THREE from 'three';

import { VoxelSpriteFrame } from './voxel-sprite-capture.js';
import {
  VOXEL_SPRITE_ENHANCEMENT_MODES,
  VoxelSpriteEnhancement,
  type VoxelSpriteEnhancementMode,
} from './voxel-sprite-enhancement.js';

type ShaderMesh = THREE.Mesh<THREE.BufferGeometry, THREE.ShaderMaterial>;

void test('all enhancement modes reuse one bounded base and splat resource pair', () => {
  const frame = testFrame(32, 24);
  const enhancement = new VoxelSpriteEnhancement(
    { frame, captureCpuSubmissionMilliseconds: 2.5 },
    { sampleColumns: 16, sampleRows: 12 },
  );
  const [base, splat] = enhancement.object.children as ShaderMesh[];
  const resourceIds = [base!.geometry.uuid, base!.material.uuid, splat!.geometry.uuid, splat!.material.uuid];

  const expectations: Record<VoxelSpriteEnhancementMode, readonly [number, number]> = {
    sprite: [1, 16 * 12],
    relit: [1, 16 * 12],
    'depth-parallax': [1, 16 * 12],
    'sprite-splat': [2, 16 * 12 * 2],
    'full-splat': [1, 16 * 12],
  };
  for (const mode of VOXEL_SPRITE_ENHANCEMENT_MODES) {
    const readout = enhancement.configure({ mode });
    assert.equal(readout.expectedDrawCalls, expectations[mode][0]);
    assert.equal(readout.geometrySampleCount, expectations[mode][1]);
    assert.deepEqual(
      [base!.geometry.uuid, base!.material.uuid, splat!.geometry.uuid, splat!.material.uuid],
      resourceIds,
    );
  }

  const readout = enhancement.readout();
  assert.equal(readout.captureCpuSubmissionMilliseconds, 2.5);
  assert.equal(readout.frameTextureBytes, 32 * 24 * 16);
  assert.equal(readout.geometryResourceCount, 2);
  assert.equal(readout.materialResourceCount, 2);
  assert.equal(readout.borrowedTextureCount, 4);
  assert.equal(readout.composition, 'depth-writing-splats');
  enhancement.dispose();
  frame.dispose();
});

void test('configuration updates are validated, fail-atomic, and avoid recapture', () => {
  const frame = testFrame(16, 16);
  const enhancement = new VoxelSpriteEnhancement({ frame });
  const configured = enhancement.configure({
    mode: 'sprite-splat',
    depthAmplitude: 0.8,
    depthScale: 'world',
    depthQuantizationSteps: 12,
    depthDilationTexels: 1.5,
    depthConfidenceThreshold: 0.7,
    splatFootprint: 1.4,
    splatOverlap: 0.4,
    normalInfluence: 0.8,
    normalOrientationBlend: 0.6,
    baseSpriteContribution: 0.45,
    viewAngleFalloff: 3,
    lightDirection: [1, 2, 3],
  });
  assert.equal(configured.revision, 2);
  assert.equal(configured.captureCpuSubmissionMilliseconds, null);
  assert.equal(configured.config.depthScale, 'world');
  assert.ok(Math.abs(length(configured.config.lightDirection) - 1) < 1e-6);

  assert.throws(() => enhancement.configure({ depthAmplitude: 4.01 }), /depthAmplitude/);
  assert.throws(() => enhancement.configure({ depthConfidenceThreshold: 1 }), /depthConfidence/);
  assert.throws(() => enhancement.configure({ sampleColumns: 64 }), /construction-time geometry/);
  assert.throws(
    () => enhancement.configure({ surprisingField: 1 } as Partial<never>),
    /unknown enhancement config fields/,
  );
  assert.equal(enhancement.readout().revision, 2);
  assert.equal(enhancement.readout().config.depthAmplitude, 0.8);

  enhancement.recordSteadyStateFrame(1.25);
  assert.equal(enhancement.readout().steadyStateCpuSubmissionMilliseconds, 1.25);
  assert.throws(() => enhancement.recordSteadyStateFrame(Number.NaN), /finite and nonnegative/);
  enhancement.dispose();
  frame.dispose();
});

void test('source replacement rebinds borrowed textures without disposing either frame', () => {
  const first = testFrame(16, 16);
  const second = testFrame(64, 32);
  const enhancement = new VoxelSpriteEnhancement({ frame: first });

  const replaced = enhancement.replaceSource({
    frame: second,
    captureCpuSubmissionMilliseconds: 4.75,
  });
  assert.equal(replaced.revision, 2);
  assert.equal(replaced.frameTextureBytes, 64 * 32 * 16);
  assert.equal(replaced.captureCpuSubmissionMilliseconds, 4.75);
  enhancement.dispose();
  assert.equal(first.disposed, false);
  assert.equal(second.disposed, false);
  first.dispose();
  second.dispose();
});

void test('camera facing respects a transformed parent and disposal releases owned render resources', () => {
  const frame = testFrame(16, 16);
  const enhancement = new VoxelSpriteEnhancement({ frame });
  const parent = new THREE.Group();
  parent.rotation.y = 0.4;
  parent.add(enhancement.object);
  parent.updateMatrixWorld(true);
  const camera = new THREE.PerspectiveCamera();
  camera.rotation.set(0.2, -0.7, 0.1);
  camera.updateMatrixWorld(true);
  enhancement.faceCamera(camera);
  parent.updateMatrixWorld(true);
  const objectWorld = enhancement.object.getWorldQuaternion(new THREE.Quaternion());
  const cameraWorld = camera.getWorldQuaternion(new THREE.Quaternion());
  assert.ok(1 - Math.abs(objectWorld.dot(cameraWorld)) < 1e-6);

  let disposeCount = 0;
  for (const child of enhancement.object.children as ShaderMesh[]) {
    child.geometry.addEventListener('dispose', () => { disposeCount += 1; });
    child.material.addEventListener('dispose', () => { disposeCount += 1; });
  }
  enhancement.dispose();
  enhancement.dispose();
  assert.equal(disposeCount, 4);
  assert.equal(enhancement.object.children.length, 0);
  assert.equal(enhancement.readout().expectedDrawCalls, 0);
  assert.equal(enhancement.readout().disposed, true);
  assert.throws(() => enhancement.configure({ mode: 'relit' }), /disposed/);
  assert.equal(frame.disposed, false);
  frame.dispose();
});

function testFrame(width: number, height: number): VoxelSpriteFrame {
  return VoxelSpriteFrame.borrowed({
    width,
    height,
    textures: {
      color: dataTexture(width, height),
      depth: dataTexture(width, height),
      normal: dataTexture(width, height),
      coverage: dataTexture(width, height),
    },
    depth: { encoding: 'linear-view-depth-unorm8', near: 0.1, far: 10 },
    normalSpace: 'view',
    capture: {
      projection: 'orthographic',
      basis: {
        position: [0, 0, 3],
        right: [1, 0, 0],
        up: [0, 1, 0],
        forward: [0, 0, -1],
      },
      bounds: { minimum: [-1, -1, -1], maximum: [1, 1, 1] },
    },
  });
}

function dataTexture(width: number, height: number): THREE.DataTexture {
  const texture = new THREE.DataTexture(new Uint8Array(width * height * 4), width, height);
  texture.needsUpdate = true;
  return texture;
}

function length(value: readonly [number, number, number]): number {
  return Math.hypot(...value);
}
