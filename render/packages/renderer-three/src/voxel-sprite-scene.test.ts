import assert from 'node:assert/strict';
import test from 'node:test';

import * as THREE from 'three';

import {
  RendererThreeVoxelSpriteScene,
  type RendererThreeVoxelSpriteBackend,
  type RendererThreeVoxelSpriteDefinition,
  type RendererThreeVoxelSpritePreparedFrame,
} from './voxel-sprite-scene.js';

function preparedFrame(textureId: string): RendererThreeVoxelSpritePreparedFrame {
  return {
    width: 8,
    height: 8,
    textures: {
      color: `${textureId}-color`,
      depth: `${textureId}-depth`,
      normal: `${textureId}-normal`,
      coverage: `${textureId}-coverage`,
    },
    depth: { near: 0.1, far: 10 },
    capture: {
      projection: 'perspective',
      position: [0, 0, 3],
      right: [1, 0, 0],
      up: [0, 1, 0],
      forward: [0, 0, -1],
      boundsMinimum: [-1, -1, -1],
      boundsMaximum: [1, 1, 1],
    },
  };
}

function definition(id: string, textureId = 'frame'): RendererThreeVoxelSpriteDefinition {
  return {
    id,
    source: { kind: 'prepared', frame: preparedFrame(textureId) },
    transform: { position: [1, 2, 3], width: 2, height: 3 },
    mode: 'sprite-splat',
    config: { sampleColumns: 8, sampleRows: 8 },
  };
}

void test('prepared voxel-sprite scene owns enhancement resources but borrows retained textures', () => {
  const scene = new THREE.Scene();
  const texture = new THREE.DataTexture(new Uint8Array(8 * 8 * 4), 8, 8);
  let invalidations = 0;
  const backend: RendererThreeVoxelSpriteBackend = {
    scene,
    objectFor: () => undefined,
    textureDescriptor: (id) => id.startsWith('frame-') ? {
      id,
      width: 8,
      height: 8,
      filter: 'nearest',
      wrap: 'clamp',
      contentHash: null,
      version: 1,
      payload: {
        encoding: 'pngRgba8',
        colorSpace: id.endsWith('-color') ? 'srgb' : 'linear',
        contentHash: 'sha256:test',
        byteLength: 1,
        source: { kind: 'inline', encodedBytes: [0] },
      },
    } : undefined,
    textureObjectFor: (id) => id.startsWith('frame-') ? texture : undefined,
  };
  const attachment = new RendererThreeVoxelSpriteScene({
    webgl: {} as THREE.WebGLRenderer,
    backend,
    invalidate: () => { invalidations += 1; },
  });

  const created = attachment.create(definition('hero'));
  assert.equal(created.applied, true);
  assert.equal(created.readout.entries[0]?.source, 'prepared');
  assert.equal(created.readout.entries[0]?.enhancement.expectedDrawCalls, 2);
  assert.deepEqual(scene.children[0]?.position.toArray(), [1, 2, 3]);

  const configured = attachment.configure('hero', { mode: 'relit', depthAmplitude: 0.5 });
  assert.equal(configured.applied, true);
  assert.equal(configured.readout.entries[0]?.enhancement.mode, 'relit');

  const failedReplacement = attachment.replace(definition('hero', 'missing'));
  assert.equal(failedReplacement.applied, false);
  assert.equal(failedReplacement.diagnostics[0]?.code, 'missing_source');
  assert.equal(failedReplacement.readout.entries[0]?.enhancement.mode, 'relit');

  const replacement = attachment.replace({ ...definition('hero'), mode: 'full-splat' });
  assert.equal(replacement.applied, true);
  assert.equal(replacement.readout.entries[0]?.enhancement.mode, 'full-splat');
  assert.equal(scene.children.length, 1);

  attachment.dispose();
  assert.equal(attachment.readout().disposed, true);
  assert.equal(scene.children.length, 0);
  assert.equal(texture.image.width, 8, 'caller-owned prepared texture remains live');
  assert.equal(invalidations, 4);
  texture.dispose();
});

void test('retained capture isolates visibility and preserves the prior representation on recapture failure', () => {
  const scene = new THREE.Scene();
  const source = new THREE.Mesh(
    new THREE.BoxGeometry(1, 2, 1),
    new THREE.MeshBasicMaterial({ color: 0x88aaff }),
  );
  const sibling = new THREE.Mesh(
    new THREE.BoxGeometry(1, 1, 1),
    new THREE.MeshBasicMaterial({ color: 0xff00ff }),
  );
  sibling.visible = false;
  scene.add(source, sibling);
  const renderer = new FakeRenderer();
  const backend: RendererThreeVoxelSpriteBackend = {
    scene,
    objectFor: (handle) => Number(handle) === 7 ? source : undefined,
    textureDescriptor: () => undefined,
    textureObjectFor: () => undefined,
  };
  const attachment = new RendererThreeVoxelSpriteScene({
    webgl: renderer as unknown as THREE.WebGLRenderer,
    backend,
  });
  const created = attachment.create({
    id: 'captured',
    source: {
      kind: 'retained',
      handle: 7 as never,
      capture: {
        resolution: 16,
        azimuthDegrees: 0,
        elevationDegrees: 0,
        near: 0.1,
        far: 20,
      },
    },
    transform: { position: [0, 0, 0], width: 1, height: 2 },
    mode: 'sprite-splat',
  });
  assert.equal(created.applied, true);
  assert.equal(source.visible, false, 'successful capture hides only the represented source');
  assert.equal(sibling.visible, false, 'capture restores unrelated visibility');
  assert.equal(scene.children.length, 3);

  const camera = new THREE.PerspectiveCamera();
  camera.rotation.set(0.2, 0.4, 0);
  camera.updateMatrixWorld(true);
  attachment.prepare(camera);
  assert.ok(scene.children[2]!.quaternion.angleTo(camera.quaternion) < 1e-6);

  renderer.throwOnRender = renderer.renderCalls + 2;
  const failed = attachment.recapture('captured');
  assert.equal(failed.applied, false);
  assert.equal(failed.diagnostics[0]?.code, 'capture_failed');
  assert.equal(failed.readout.entries[0]?.fallbackPreservedCount, 1);
  assert.equal(failed.readout.entries[0]?.enhancement.mode, 'sprite-splat');
  assert.equal(scene.children.length, 3, 'failed recapture reuses the live enhancement');
  assert.equal(source.visible, false);
  assert.equal(sibling.visible, false);

  attachment.dispose();
  assert.equal(source.visible, true, 'final disposal restores authored source visibility');
  source.geometry.dispose();
  (source.material as THREE.Material).dispose();
  sibling.geometry.dispose();
  (sibling.material as THREE.Material).dispose();
});

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

  clear(): void {}
  getClearAlpha(): number { return this.clearAlpha; }
  getClearColor(target: THREE.Color): THREE.Color { return target.copy(this.clearColor); }
  getRenderTarget(): THREE.WebGLRenderTarget | null { return this.renderTarget; }
  getScissor(target: THREE.Vector4): THREE.Vector4 { return target.copy(this.scissor); }
  getScissorTest(): boolean { return this.scissorTest; }
  getViewport(target: THREE.Vector4): THREE.Vector4 { return target.copy(this.viewport); }
  render(): void {
    this.renderCalls += 1;
    if (this.renderCalls === this.throwOnRender) throw new Error('synthetic render failure');
  }
  setClearColor(color: THREE.ColorRepresentation | THREE.Color, alpha = 1): void {
    if (color instanceof THREE.Color) this.clearColor.copy(color);
    else this.clearColor.set(color);
    this.clearAlpha = alpha;
  }
  setRenderTarget(target: THREE.WebGLRenderTarget | null): void { this.renderTarget = target; }
  setScissor(value: THREE.Vector4 | number, y?: number, width?: number, height?: number): void {
    assignVector(this.scissor, value, y, width, height);
  }
  setScissorTest(enabled: boolean): void { this.scissorTest = enabled; }
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
  if (value instanceof THREE.Vector4) target.copy(value);
  else target.set(value, y ?? 0, width ?? 0, height ?? 0);
}
