import assert from 'node:assert/strict';
import test from 'node:test';
import * as THREE from 'three';

import {
  VoxelSpriteFrame,
  VoxelSpriteRuntimeCapture,
  type VoxelSpriteFrameTextures,
} from './voxel-sprite-capture.js';

void test('borrowed prepared frames validate metadata without acquiring caller textures', () => {
  const textures = testTextures();
  let disposalCount = 0;
  for (const texture of Object.values(textures)) {
    texture.addEventListener('dispose', () => { disposalCount += 1; });
  }
  const frame = VoxelSpriteFrame.borrowed({
    width: 16,
    height: 16,
    textures,
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

  assert.deepEqual(frame.readout(), {
    schemaVersion: 1,
    width: 16,
    height: 16,
    provenance: 'prepared',
    estimatedTextureBytes: 16 * 16 * 16,
    disposed: false,
  });
  frame.dispose();
  frame.dispose();
  assert.equal(frame.disposed, true);
  assert.equal(disposalCount, 0);
});

void test('borrowed prepared frames reject non-finite depth metadata', () => {
  assert.throws(() => VoxelSpriteFrame.borrowed({
    width: 16,
    height: 16,
    textures: testTextures(),
    depth: { encoding: 'linear-view-depth-unorm8', near: 0.1, far: Number.POSITIVE_INFINITY },
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
  }), /finite and increasing/);
});

void test('capture rejects invalid requests without rendering or replacing the current frame', () => {
  const renderer = new FakeRenderer();
  const capture = new VoxelSpriteRuntimeCapture(renderer as unknown as THREE.WebGLRenderer);
  const scene = captureScene();
  const camera = captureCamera();

  const receipt = capture.capture({ scene, camera, width: 7, height: 16 });

  assert.equal(receipt.applied, false);
  assert.equal(receipt.diagnostics[0]?.code, 'invalid_capture_request');
  assert.equal(renderer.renderCalls, 0);
  assert.equal(capture.currentFrame(), null);
  assert.equal(receipt.readout.rejectedCaptureCount, 1);
});

void test('capture accepts the 4096 experiment ceiling without allocating CPU pixel arrays', () => {
  const renderer = new FakeRenderer();
  const capture = new VoxelSpriteRuntimeCapture(renderer as unknown as THREE.WebGLRenderer);
  const scene = captureScene();
  const camera = captureCamera();

  const receipt = capture.capture({ scene, camera, width: 4096, height: 4096 });

  assert.equal(receipt.applied, true);
  assert.equal(receipt.frame?.readout().estimatedTextureBytes, 4096 * 4096 * 16);
  assert.equal(renderer.renderCalls, 4);
  capture.dispose();

  const rejected = new VoxelSpriteRuntimeCapture(
    new FakeRenderer() as unknown as THREE.WebGLRenderer,
  ).capture({ scene, camera, width: 4097, height: 4096 });
  assert.equal(rejected.applied, false);
  assert.match(rejected.diagnostics[0]?.message ?? '', /4096/);
});

void test('successful recapture is atomic, restores caller state, and disposes replaced resources', () => {
  const renderer = new FakeRenderer();
  const originalTarget = new THREE.WebGLRenderTarget(2, 2);
  renderer.renderTarget = originalTarget;
  renderer.viewport.set(4, 5, 640, 480);
  renderer.scissor.set(7, 8, 320, 240);
  renderer.scissorTest = true;
  renderer.clearColor.set(0x123456);
  renderer.clearAlpha = 0.75;
  renderer.autoClear = false;
  renderer.xr.enabled = true;

  const scene = captureScene();
  const background = new THREE.Color(0x334455);
  const fog = new THREE.Fog(0x334455, 1, 10);
  const override = new THREE.MeshBasicMaterial({ color: 0xff00ff });
  scene.background = background;
  scene.fog = fog;
  scene.overrideMaterial = override;
  const camera = captureCamera();
  const capture = new VoxelSpriteRuntimeCapture(renderer as unknown as THREE.WebGLRenderer);

  const first = capture.capture({ scene, camera, width: 32, height: 24 });
  assert.equal(first.applied, true);
  assert.equal(first.frame?.descriptor.provenance, 'runtime-capture');
  assert.equal(first.frame?.descriptor.depth.encoding, 'linear-view-depth-unorm8');
  assert.equal(first.readout.currentFrame?.estimatedTextureBytes, 32 * 24 * 16);
  assert.equal(first.readout.captureCount, 1);
  assert.equal(renderer.renderCalls, 4);
  assert.equal(scene.background, background);
  assert.equal(scene.fog, fog);
  assert.equal(scene.overrideMaterial, override);
  assert.equal(renderer.renderTarget, originalTarget);
  assert.deepEqual(renderer.viewport.toArray(), [4, 5, 640, 480]);
  assert.deepEqual(renderer.scissor.toArray(), [7, 8, 320, 240]);
  assert.equal(renderer.scissorTest, true);
  assert.equal(renderer.clearColor.getHex(), 0x123456);
  assert.equal(renderer.clearAlpha, 0.75);
  assert.equal(renderer.autoClear, false);
  assert.equal(renderer.xr.enabled, true);

  const firstFrame = first.frame!;
  camera.position.x = 1;
  camera.lookAt(0, 0, 0);
  const second = capture.capture({ scene, camera, width: 32, height: 24 });
  assert.equal(second.applied, true);
  assert.equal(second.revision, 2);
  assert.equal(firstFrame.disposed, true);
  assert.equal(second.frame?.disposed, false);
  assert.notDeepEqual(
    second.frame?.descriptor.capture.basis.position,
    firstFrame.descriptor.capture.basis.position,
  );
  assert.equal(renderer.renderCalls, 8);

  capture.dispose();
  assert.equal(second.frame?.disposed, true);
  assert.equal(capture.readout().disposed, true);
  originalTarget.dispose();
  override.dispose();
});

void test('failed recapture retains the last successful frame and restores state', () => {
  const renderer = new FakeRenderer();
  const scene = captureScene();
  const camera = captureCamera();
  const capture = new VoxelSpriteRuntimeCapture(renderer as unknown as THREE.WebGLRenderer);
  const first = capture.capture({ scene, camera, width: 16, height: 16 });
  assert.equal(first.applied, true);
  const stable = first.frame!;

  renderer.throwOnRender = renderer.renderCalls + 2;
  const failed = capture.capture({ scene, camera, width: 32, height: 32 });

  assert.equal(failed.applied, false);
  assert.equal(failed.diagnostics[0]?.code, 'capture_failed');
  assert.equal(failed.frame, stable);
  assert.equal(capture.currentFrame(), stable);
  assert.equal(stable.disposed, false);
  assert.equal(failed.revision, 1);
  assert.equal(scene.overrideMaterial, null);
  assert.equal(renderer.renderTarget, null);

  capture.dispose();
  const disposed = capture.capture({ scene, camera, width: 16, height: 16 });
  assert.equal(disposed.applied, false);
  assert.equal(disposed.diagnostics[0]?.code, 'capture_disposed');
});

function captureScene(): THREE.Scene {
  const scene = new THREE.Scene();
  scene.add(new THREE.Mesh(
    new THREE.BoxGeometry(1, 1, 1),
    new THREE.MeshBasicMaterial({ color: 0x88aaff }),
  ));
  return scene;
}

function captureCamera(): THREE.PerspectiveCamera {
  const camera = new THREE.PerspectiveCamera(45, 1, 0.1, 20);
  camera.position.set(0, 0, 4);
  camera.lookAt(0, 0, 0);
  camera.updateMatrixWorld(true);
  return camera;
}

function testTextures(): VoxelSpriteFrameTextures {
  return {
    color: dataTexture(),
    depth: dataTexture(),
    normal: dataTexture(),
    coverage: dataTexture(),
  };
}

function dataTexture(): THREE.DataTexture {
  const texture = new THREE.DataTexture(new Uint8Array(16 * 16 * 4), 16, 16);
  texture.needsUpdate = true;
  return texture;
}

class FakeRenderer {
  autoClear = true;
  clearAlpha = 1;
  clearColor = new THREE.Color(0);
  renderCalls = 0;
  renderTarget: THREE.WebGLRenderTarget | null = null;
  scissor = new THREE.Vector4(0, 0, 1, 1);
  scissorTest = false;
  throwOnRender: number | null = null;
  viewport = new THREE.Vector4(0, 0, 1, 1);
  readonly xr = { enabled: false };

  clear(_color: boolean, _depth: boolean, _stencil: boolean): void {}

  getClearAlpha(): number {
    return this.clearAlpha;
  }

  getClearColor(target: THREE.Color): THREE.Color {
    return target.copy(this.clearColor);
  }

  getRenderTarget(): THREE.WebGLRenderTarget | null {
    return this.renderTarget;
  }

  getScissor(target: THREE.Vector4): THREE.Vector4 {
    return target.copy(this.scissor);
  }

  getScissorTest(): boolean {
    return this.scissorTest;
  }

  getViewport(target: THREE.Vector4): THREE.Vector4 {
    return target.copy(this.viewport);
  }

  render(_scene: THREE.Scene, _camera: THREE.Camera): void {
    this.renderCalls += 1;
    if (this.throwOnRender === this.renderCalls) throw new Error('synthetic render failure');
  }

  setClearColor(color: THREE.ColorRepresentation | THREE.Color, alpha = 1): void {
    if (color instanceof THREE.Color) this.clearColor.copy(color);
    else this.clearColor.set(color);
    this.clearAlpha = alpha;
  }

  setRenderTarget(target: THREE.WebGLRenderTarget | null): void {
    this.renderTarget = target;
  }

  setScissor(value: THREE.Vector4 | number, y?: number, width?: number, height?: number): void {
    assignVector(this.scissor, value, y, width, height);
  }

  setScissorTest(enabled: boolean): void {
    this.scissorTest = enabled;
  }

  setViewport(value: THREE.Vector4 | number, y?: number, width?: number, height?: number): void {
    assignVector(this.viewport, value, y, width, height);
  }
}

function assignVector(
  target: THREE.Vector4,
  value: THREE.Vector4 | number,
  y?: number,
  width?: number,
  height?: number,
): void {
  if (value instanceof THREE.Vector4) {
    target.copy(value);
    return;
  }
  target.set(value, y ?? 0, width ?? 0, height ?? 0);
}
