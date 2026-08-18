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
    { sampleColumns: 16, sampleRows: 12, splatColumns: 24, splatRows: 20 },
  );
  const [base, splat] = enhancement.object.children as ShaderMesh[];
  const resourceIds = [base!.geometry.uuid, base!.material.uuid, splat!.geometry.uuid, splat!.material.uuid];

  const expectations: Record<VoxelSpriteEnhancementMode, readonly [number, number]> = {
    sprite: [1, 16 * 12],
    relit: [1, 16 * 12],
    'depth-parallax': [1, 16 * 12],
    'sprite-splat': [2, (16 * 12) + (24 * 20)],
    'full-splat': [1, 24 * 20],
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
  assert.equal((splat!.geometry as THREE.InstancedBufferGeometry).instanceCount, 24 * 20);
  enhancement.dispose();
  frame.dispose();
});

void test('configuration updates are validated, fail-atomic, and avoid recapture', () => {
  const frame = testFrame(16, 16);
  const enhancement = new VoxelSpriteEnhancement({ frame });
  const configured = enhancement.configure({
    mode: 'sprite-splat',
    depthAmplitude: 0.8,
    depthContrast: 6,
    depthClamp: 0.75,
    depthScale: 'world',
    depthQuantizationSteps: 12,
    parallaxOcclusionScale: 0.08,
    parallaxOcclusionSteps: 20,
    depthDilationTexels: 1.5,
    depthConfidenceThreshold: 0.7,
    splatFootprint: 1.4,
    splatOverlap: 0.4,
    splatOpacity: 0.55,
    splatBlendMode: 'alpha-blend',
    normalInfluence: 0.8,
    normalOrientationBlend: 0.6,
    baseSpriteContribution: 0.45,
    viewAngleFalloff: 3,
    lightingMode: 'normal',
    ambientLight: 0.6,
    diffuseLight: 1.4,
    outputGain: 1.5,
    ambientColor: [0.5, 0.6, 0.7],
    lightColor: [1, 0.9, 0.8],
    lightDirection: [1, 2, 3],
  });
  assert.equal(configured.revision, 2);
  assert.equal(configured.captureCpuSubmissionMilliseconds, null);
  assert.equal(configured.config.depthScale, 'world');
  assert.equal(configured.config.depthContrast, 6);
  assert.equal(configured.config.parallaxOcclusionSteps, 20);
  assert.equal(configured.config.lightingMode, 'normal');
  assert.equal(configured.config.outputGain, 1.5);
  assert.equal(configured.config.splatOpacity, 0.55);
  assert.equal(configured.config.orientationPolicy, 'camera-facing');
  assert.equal(configured.config.orientationBlend, 0.5);
  assert.equal(configured.composition, 'base-blend-then-alpha-blended-splats');
  const splatMaterial = (enhancement.object.children[1] as ShaderMesh).material;
  assert.equal(splatMaterial.depthWrite, false);
  assert.equal(splatMaterial.blending, THREE.NormalBlending);
  assert.ok(Math.abs(length(configured.config.lightDirection) - 1) < 1e-6);

  const additive = enhancement.configure({ splatBlendMode: 'additive' });
  assert.equal(additive.composition, 'base-blend-then-additive-splats');
  assert.equal(splatMaterial.depthWrite, false);
  assert.equal(splatMaterial.blending, THREE.AdditiveBlending);

  const depthWriting = enhancement.configure({ splatBlendMode: 'depth-write' });
  assert.equal(depthWriting.composition, 'base-blend-then-depth-writing-splats');
  assert.equal(splatMaterial.depthWrite, true);
  assert.equal(splatMaterial.blending, THREE.NormalBlending);

  const plainButRelit = enhancement.configure({ mode: 'sprite', lightingMode: 'normal' });
  assert.equal(plainButRelit.mode, 'sprite');
  assert.equal(plainButRelit.config.lightingMode, 'normal');
  const enhancedButCaptured = enhancement.configure({
    mode: 'depth-parallax',
    lightingMode: 'captured',
  });
  assert.equal(enhancedButCaptured.mode, 'depth-parallax');
  assert.equal(enhancedButCaptured.config.lightingMode, 'captured');

  assert.throws(() => enhancement.configure({ depthAmplitude: 4.01 }), /depthAmplitude/);
  assert.throws(() => enhancement.configure({ depthContrast: 16.01 }), /depthContrast/);
  assert.throws(() => enhancement.configure({ parallaxOcclusionScale: 0.251 }), /parallaxOcclusionScale/);
  assert.throws(() => enhancement.configure({ parallaxOcclusionSteps: 3 }), /parallaxOcclusionSteps/);
  assert.throws(() => enhancement.configure({ representationWeight: 1.01 }), /representationWeight/);
  assert.throws(
    () => enhancement.configure({ representationTransition: 'multiply' as never }),
    /representationTransition/,
  );
  assert.throws(
    () => enhancement.configure({ orientationAzimuthOffsetDegrees: 45.01 }),
    /orientationAzimuthOffsetDegrees/,
  );
  assert.throws(() => enhancement.configure({ depthClamp: 1.01 }), /depthClamp/);
  assert.throws(() => enhancement.configure({ depthConfidenceThreshold: 1 }), /depthConfidence/);
  assert.throws(() => enhancement.configure({ outputGain: 4.01 }), /outputGain/);
  assert.throws(() => enhancement.configure({ splatOpacity: 1.01 }), /splatOpacity/);
  assert.throws(() => enhancement.configure({ orientationBlend: 1.01 }), /orientationBlend/);
  assert.throws(
    () => enhancement.configure({ orientationPolicy: 'tracking' as never }),
    /orientationPolicy/,
  );
  assert.throws(
    () => enhancement.configure({ splatBlendMode: 'multiply' as never }),
    /splatBlendMode/,
  );
  assert.throws(() => enhancement.configure({ ambientColor: [1, -0.1, 1] }), /ambientColor/);
  assert.throws(() => enhancement.configure({ sampleColumns: 64 }), /construction-time geometry/);
  assert.throws(() => enhancement.configure({ splatColumns: 64 }), /construction-time geometry/);
  assert.throws(
    () => enhancement.configure({ surprisingField: 1 } as Partial<never>),
    /unknown enhancement config fields/,
  );
  assert.equal(enhancement.readout().revision, 6);
  assert.equal(enhancement.readout().config.depthAmplitude, 0.8);

  enhancement.recordSteadyStateFrame(1.25);
  assert.equal(enhancement.readout().steadyStateCpuSubmissionMilliseconds, 1.25);
  assert.throws(() => enhancement.recordSteadyStateFrame(Number.NaN), /finite and nonnegative/);
  enhancement.dispose();
  frame.dispose();
});

void test('depth relief is rebased to captured subject bounds instead of the camera clip range', () => {
  const frame = testFrame(16, 16);
  const enhancement = new VoxelSpriteEnhancement({ frame });
  const base = enhancement.object.children[0] as ShaderMesh;
  const uniforms = base.material.uniforms;

  assert.equal(uniforms['captureNear']!.value, 0.1);
  assert.equal(uniforms['captureDepthRange']!.value, 9.9);
  assert.equal(uniforms['reliefRearDepth']!.value, 4);
  assert.equal(uniforms['reliefDepthRange']!.value, 2);
  assert.match(base.material.vertexShader, /reliefRearDepth - sampledViewDepth/);
  assert.match(base.material.vertexShader, /subjectDepth - 0\.5/);
  assert.match(base.material.vertexShader, /centeredDepth \* depthAmplitude/);
  assert.match(base.material.vertexShader, /reliefDepthRange/);

  enhancement.dispose();
  frame.dispose();
});

void test('depth-parallax uses bounded connected-card POM with vertex fallback when disabled', () => {
  const frame = testFrame(16, 16);
  const enhancement = new VoxelSpriteEnhancement(
    { frame },
    { mode: 'depth-parallax', parallaxOcclusionScale: 0.1, parallaxOcclusionSteps: 24 },
  );
  const base = enhancement.object.children[0] as ShaderMesh;

  assert.equal(base.material.uniforms['parallaxOcclusionEnabled']!.value, 1);
  assert.equal(base.material.uniforms['baseDepthDisplacement']!.value, 0);
  assert.match(base.material.fragmentShader, /parallaxOcclusionUv/);
  assert.match(base.material.fragmentShader, /for \(int index = 0; index < 32/);
  assert.match(base.material.fragmentShader, /currentUv \+= uvDelta/);
  assert.match(base.material.fragmentShader, /previousUv = currentUv - uvDelta/);

  enhancement.configure({ parallaxOcclusionSteps: 0 });
  assert.equal(base.material.uniforms['parallaxOcclusionEnabled']!.value, 0);
  assert.equal(base.material.uniforms['baseDepthDisplacement']!.value, 1);

  enhancement.dispose();
  frame.dispose();
});

void test('neighboring representations support opaque, dithered, and alpha transition weights', () => {
  const frame = testFrame(16, 16);
  const enhancement = new VoxelSpriteEnhancement({ frame }, { mode: 'sprite' });
  const [base, splat] = enhancement.object.children as ShaderMesh[];

  enhancement.configure({
    representationTransition: 'dither',
    representationWeight: 0.4,
    representationDitherOffset: 0.35,
  });
  assert.equal(base!.material.uniforms['representationTransitionMode']!.value, 1);
  assert.equal(base!.material.uniforms['representationWeight']!.value, 0.4);
  assert.equal(base!.material.uniforms['representationDitherOffset']!.value, 0.35);
  assert.equal(base!.material.transparent, false);
  assert.equal(base!.material.depthWrite, true);
  assert.match(base!.material.fragmentShader, /floor\(gl_FragCoord\.xy\)/);
  assert.match(base!.material.fragmentShader, /threshold - representationDitherOffset/);

  enhancement.configure({ representationTransition: 'alpha', representationWeight: 0.6 });
  assert.equal(base!.material.uniforms['representationTransitionMode']!.value, 2);
  assert.equal(base!.material.transparent, true);
  assert.equal(base!.material.depthWrite, false);
  assert.equal(splat!.material.depthWrite, false);

  enhancement.dispose();
  frame.dispose();
});

void test('splat density scales independently to the bounded 512 square maximum', () => {
  const frame = testFrame(16, 16);
  const enhancement = new VoxelSpriteEnhancement(
    { frame },
    {
      mode: 'full-splat',
      sampleColumns: 8,
      sampleRows: 8,
      splatColumns: 512,
      splatRows: 512,
    },
  );
  const splat = enhancement.object.children[1] as ShaderMesh;

  assert.equal(enhancement.readout().geometrySampleCount, 512 * 512);
  assert.equal((splat.geometry as THREE.InstancedBufferGeometry).instanceCount, 512 * 512);

  enhancement.dispose();
  frame.dispose();
});

void test('source replacement rebinds borrowed textures without disposing either frame', () => {
  const first = testFrame(16, 16);
  const second = testFrame(64, 32, {
    position: [3, 2, 1],
    right: [0, 0, -1],
    up: [0, 1, 0],
    forward: [-1, 0, 0],
  });
  const enhancement = new VoxelSpriteEnhancement({ frame: first });

  const replaced = enhancement.replaceSource({
    frame: second,
    captureCpuSubmissionMilliseconds: 4.75,
  });
  assert.equal(replaced.revision, 2);
  assert.equal(replaced.frameTextureBytes, 64 * 32 * 16);
  assert.equal(replaced.captureCpuSubmissionMilliseconds, 4.75);
  assert.deepEqual(replaced.captureBasis.forward, [-1, 0, 0]);
  enhancement.dispose();
  assert.equal(first.disposed, false);
  assert.equal(second.disposed, false);
  first.dispose();
  second.dispose();
});

void test('orientation policies respect capture basis, blend endpoints, and transformed parents', () => {
  const frame = testFrame(16, 16);
  const enhancement = new VoxelSpriteEnhancement({ frame });
  const parent = new THREE.Group();
  parent.rotation.y = 0.4;
  parent.add(enhancement.object);
  parent.updateMatrixWorld(true);
  const camera = new THREE.PerspectiveCamera();
  camera.position.set(2, 1, 4);
  camera.rotation.set(0.2, -0.7, 0.1);
  camera.updateMatrixWorld(true);
  assert.equal(enhancement.readout().angularOffsetDegrees, null);
  enhancement.prepare(camera);
  parent.updateMatrixWorld(true);
  const objectWorld = enhancement.object.getWorldQuaternion(new THREE.Quaternion());
  const cameraWorld = camera.getWorldQuaternion(new THREE.Quaternion());
  assert.ok(1 - Math.abs(objectWorld.dot(cameraWorld)) < 1e-6);
  assert.ok(enhancement.readout().angularOffsetDegrees! > 0);

  enhancement.configure({ orientationPolicy: 'capture-held' });
  enhancement.prepare(camera);
  parent.updateMatrixWorld(true);
  assert.ok(enhancement.object.getWorldQuaternion(new THREE.Quaternion())
    .angleTo(new THREE.Quaternion()) < 1e-6);

  enhancement.configure({ orientationPolicy: 'capture-camera-blend', orientationBlend: 0 });
  enhancement.prepare(camera);
  parent.updateMatrixWorld(true);
  assert.ok(enhancement.object.getWorldQuaternion(new THREE.Quaternion())
    .angleTo(new THREE.Quaternion()) < 1e-6);

  enhancement.configure({ orientationBlend: 1 });
  enhancement.prepare(camera);
  parent.updateMatrixWorld(true);
  assert.ok(1 - Math.abs(enhancement.object.getWorldQuaternion(new THREE.Quaternion()).dot(cameraWorld)) < 1e-6);

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

void test('world-upright orientation removes capture elevation while preserving azimuth', () => {
  const tilt = new THREE.Quaternion().setFromEuler(new THREE.Euler(0.35, 0.6, 0));
  const frame = testFrame(16, 16, {
    position: [0, 2, 3],
    right: new THREE.Vector3(1, 0, 0).applyQuaternion(tilt).toArray(),
    up: new THREE.Vector3(0, 1, 0).applyQuaternion(tilt).toArray(),
    forward: new THREE.Vector3(0, 0, -1).applyQuaternion(tilt).toArray(),
  });
  const enhancement = new VoxelSpriteEnhancement(
    { frame },
    { orientationPolicy: 'capture-held', orientationElevationPolicy: 'world-upright' },
  );
  const camera = new THREE.PerspectiveCamera();
  camera.position.set(3, 2, 4);
  camera.updateMatrixWorld(true);
  enhancement.prepare(camera);
  const worldUp = new THREE.Vector3(0, 1, 0).applyQuaternion(
    enhancement.object.getWorldQuaternion(new THREE.Quaternion()),
  );
  assert.ok(worldUp.angleTo(new THREE.Vector3(0, 1, 0)) < 1e-6);
  enhancement.dispose();
  frame.dispose();
});

function testFrame(
  width: number,
  height: number,
  basis: {
    position: readonly [number, number, number];
    right: readonly [number, number, number];
    up: readonly [number, number, number];
    forward: readonly [number, number, number];
  } = {
    position: [0, 0, 3],
    right: [1, 0, 0],
    up: [0, 1, 0],
    forward: [0, 0, -1],
  },
): VoxelSpriteFrame {
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
      basis,
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
