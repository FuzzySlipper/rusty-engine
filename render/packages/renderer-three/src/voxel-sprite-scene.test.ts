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
  assert.equal(created.readout.entries[0]?.enhancement?.expectedDrawCalls, 2);
  assert.deepEqual(scene.children[0]?.position.toArray(), [1, 2, 3]);

  const configured = attachment.configure('hero', { mode: 'relit', depthAmplitude: 0.5 });
  assert.equal(configured.applied, true);
  assert.equal(configured.readout.entries[0]?.enhancement?.mode, 'relit');

  const failedReplacement = attachment.replace(definition('hero', 'missing'));
  assert.equal(failedReplacement.applied, false);
  assert.equal(failedReplacement.diagnostics[0]?.code, 'missing_source');
  assert.equal(failedReplacement.readout.entries[0]?.enhancement?.mode, 'relit');

  const replacement = attachment.replace({ ...definition('hero'), mode: 'full-splat' });
  assert.equal(replacement.applied, true);
  assert.equal(replacement.readout.entries[0]?.enhancement?.mode, 'full-splat');
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
  assert.equal(failed.readout.entries[0]?.enhancement?.mode, 'sprite-splat');
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

void test('runtime capture defaults to an isolated studio rig and restores scene lights', () => {
  const scene = new THREE.Scene();
  const source = new THREE.Mesh(
    new THREE.BoxGeometry(1, 2, 1),
    new THREE.MeshStandardMaterial({ color: 0x6688aa }),
  );
  const authoredLight = new THREE.PointLight(0xff0000, 7);
  scene.add(source, authoredLight);
  const renderer = new FakeRenderer();
  const attachment = new RendererThreeVoxelSpriteScene({
    webgl: renderer as unknown as THREE.WebGLRenderer,
    backend: {
      scene,
      objectFor: (handle) => Number(handle) === 9 ? source : undefined,
      textureDescriptor: () => undefined,
      textureObjectFor: () => undefined,
    },
  });

  const created = attachment.create({
    id: 'lit-capture',
    source: {
      kind: 'retained',
      handle: 9 as never,
      capture: {
        resolution: 16,
        azimuthDegrees: 0,
        elevationDegrees: 10,
        near: 0.1,
        far: 20,
      },
    },
    transform: { position: [0, 0, 0], width: 1, height: 2 },
    mode: 'sprite',
    config: { lightingMode: 'normal', outputGain: 1.25 },
  });
  assert.equal(created.applied, true);
  assert.equal(created.readout.entries[0]?.capture?.lighting?.mode, 'isolated');
  assert.deepEqual(renderer.visibleLightSnapshots[0], [
    'AmbientLight',
    'DirectionalLight',
    'DirectionalLight',
  ]);
  assert.equal(authoredLight.visible, true);
  assert.equal(scene.children.includes(authoredLight), true);
  assert.equal(scene.children.some((child) => child instanceof THREE.AmbientLight), false);
  assert.equal(created.readout.entries[0]?.enhancement?.config.lightingMode, 'normal');

  const nextColorRender = renderer.renderCalls;
  const recaptured = attachment.recapture('lit-capture', {
    resolution: 16,
    azimuthDegrees: 0,
    elevationDegrees: 10,
    near: 0.1,
    far: 20,
    lighting: { mode: 'scene' },
  });
  assert.equal(recaptured.applied, true);
  assert.deepEqual(renderer.visibleLightSnapshots[nextColorRender], ['PointLight']);
  assert.equal(recaptured.readout.entries[0]?.capture?.lighting?.mode, 'scene');

  const rejected = attachment.recapture('lit-capture', {
    resolution: 16,
    azimuthDegrees: 0,
    elevationDegrees: 10,
    near: 0.1,
    far: 20,
    lighting: { mode: 'isolated', keyIntensity: 8.01 },
  });
  assert.equal(rejected.applied, false);
  assert.equal(rejected.diagnostics[0]?.code, 'invalid_definition');
  assert.equal(authoredLight.visible, true);
  assert.equal(scene.children.some((child) => child instanceof THREE.AmbientLight), false);

  attachment.dispose();
  source.geometry.dispose();
  (source.material as THREE.Material).dispose();
});

void test('ghost-plate freezes an isolated multipart pose without mutating the canonical retained hierarchy', () => {
  const scene = new THREE.Scene();
  const parent = new THREE.Group();
  parent.position.set(3, 1, -2);
  parent.rotation.set(0.1, 0.4, -0.05);
  parent.scale.setScalar(1.5);
  const source = new THREE.Group();
  source.position.set(0.5, 0.25, -0.75);
  const bodyGeometry = new THREE.BoxGeometry(1, 2, 0.6);
  const attachmentGeometry = new THREE.BoxGeometry(0.3, 0.8, 0.2);
  const bodyMaterial = new THREE.MeshStandardMaterial({ color: 0x88aaff });
  const attachmentMaterial = new THREE.MeshStandardMaterial({ color: 0xffaa44 });
  const body = new THREE.Mesh(bodyGeometry, bodyMaterial);
  const rigidAttachment = new THREE.Mesh(attachmentGeometry, attachmentMaterial);
  const attachedLight = new THREE.PointLight(0xaaccff, 3);
  rigidAttachment.position.set(0.8, 0.3, 0);
  source.add(body, rigidAttachment, attachedLight);
  parent.add(source);
  scene.add(parent);
  parent.updateWorldMatrix(true, true);
  const sourceWorldBefore = source.matrixWorld.clone();
  const renderer = new FakeRenderer();
  const attachment = new RendererThreeVoxelSpriteScene({
    webgl: renderer as unknown as THREE.WebGLRenderer,
    backend: {
      scene,
      objectFor: (handle) => Number(handle) === 11 ? source : undefined,
      textureDescriptor: () => undefined,
      textureObjectFor: () => undefined,
    },
  });

  const created = attachment.create({
    id: 'ghost',
    source: {
      kind: 'retained',
      handle: 11 as never,
      capture: {
        resolution: 16,
        azimuthDegrees: 20,
        elevationDegrees: 8,
        near: 0.1,
        far: 30,
      },
    },
    transform: { position: [7, 2, 4], width: 2, height: 3 },
    mode: 'ghost-plate',
    config: {
      ghostDepthRetention: 0.12,
      ghostAnchorPolicy: 'bounds-center',
      ghostAnchorValue: 0.5,
      ghostPlateMapping: 'plate-locked',
    },
  });
  assert.equal(created.applied, true);
  assert.equal(created.readout.entries[0]?.presentation, 'ghost-plate');
  assert.equal(created.readout.entries[0]?.enhancement, null);
  assert.equal(created.readout.entries[0]?.ghostPlate?.meshCount, 2);
  assert.equal(created.readout.entries[0]?.ghostPlate?.matchedPose, true);
  assert.equal(created.readout.entries[0]?.ghostPlate?.captureBasis.forward.length, 3);
  assert.equal(source.visible, true, 'ghost capture does not acquire canonical visibility');
  assert.equal(body.layers.mask & 1, 0, 'renderer-owned layer lease suppresses canonical main color');
  assert.equal(rigidAttachment.layers.mask & 1, 0);
  assert.equal(body.material, bodyMaterial, 'canonical body material remains authored');
  assert.equal(rigidAttachment.material, attachmentMaterial, 'canonical attachment material remains authored');
  assert.ok(source.matrixWorld.equals(sourceWorldBefore), 'transformed-parent source matrix remains unchanged');
  assert.equal(scene.children.length, 2, 'main scene receives only the independent ghost presentation');
  let ghostLightCount = 0;
  scene.children[1]!.traverse((object) => {
    if (object instanceof THREE.Light) ghostLightCount += 1;
  });
  assert.equal(ghostLightCount, 0, 'source-attached lights do not leak into the ghost presentation');
  assert.equal(source.children.includes(attachedLight), true);
  assert.equal(attachedLight.visible, true);

  const configured = attachment.configure('ghost', {
    ghostDepthRetention: 1,
    ghostAnchorPolicy: 'bounds-normalized',
    ghostAnchorValue: 0,
    ghostPlateMapping: 'projective-surface',
  });
  assert.equal(configured.applied, true);
  assert.equal(configured.readout.entries[0]?.ghostPlate?.depthRetention, 1);
  assert.equal(configured.readout.entries[0]?.ghostPlate?.anchorValue, 0);
  assert.equal(configured.readout.entries[0]?.ghostPlate?.plateMapping, 'projective-surface');

  const camera = new THREE.PerspectiveCamera(60, 1, 0.1, 100);
  camera.position.set(10, 3, 8);
  camera.lookAt(7, 2, 4);
  camera.updateMatrixWorld(true);
  attachment.prepare(camera);
  assert.notEqual(attachment.readout().entries[0]?.ghostPlate?.angularOffsetDegrees, null);

  renderer.throwOnRender = renderer.renderCalls + 2;
  const failedRecapture = attachment.recapture('ghost');
  assert.equal(failedRecapture.applied, false);
  assert.equal(failedRecapture.diagnostics[0]?.code, 'capture_failed');
  assert.equal(failedRecapture.readout.entries[0]?.fallbackPreservedCount, 1);
  assert.equal(failedRecapture.readout.entries[0]?.ghostPlate?.enabled, true);
  assert.equal(scene.children.length, 2, 'failed candidate preserves the live ghost presentation');
  renderer.throwOnRender = null;

  const ordinaryReplacement = attachment.replace({
    id: 'ghost',
    source: {
      kind: 'retained',
      handle: 11 as never,
      capture: {
        resolution: 16,
        azimuthDegrees: 20,
        elevationDegrees: 8,
        near: 0.1,
        far: 30,
      },
    },
    transform: { position: [7, 2, 4], width: 2, height: 3 },
    mode: 'sprite',
  });
  assert.equal(ordinaryReplacement.applied, true, 'ordinary recapture bypasses the live ghost layer lease');
  assert.equal(ordinaryReplacement.readout.entries[0]?.presentation, 'enhancement');
  assert.equal(body.layers.mask & 1, 1);
  assert.equal(source.visible, false, 'ordinary proxy retains its existing visibility ownership');

  const preparedRejected = attachment.create({
    ...definition('prepared-ghost'),
    mode: 'ghost-plate',
  });
  assert.equal(preparedRejected.applied, false);
  assert.equal(preparedRejected.diagnostics[0]?.code, 'invalid_definition');
  assert.match(preparedRejected.diagnostics[0]?.message ?? '', /requires a retained source/);

  attachment.dispose();
  assert.equal(scene.children.length, 1);
  assert.equal(source.visible, true);
  assert.equal(body.layers.mask & 1, 1, 'final disposal restores the canonical render layer');
  assert.equal(rigidAttachment.layers.mask & 1, 1);
  assert.equal(body.material, bodyMaterial);
  assert.ok(source.matrixWorld.equals(sourceWorldBefore));
  bodyGeometry.dispose();
  attachmentGeometry.dispose();
  bodyMaterial.dispose();
  attachmentMaterial.dispose();
});

void test('ghost-plate clones a frozen skinned pose with an independent skeleton', () => {
  const scene = new THREE.Scene();
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.Float32BufferAttribute([
    -0.5, -0.5, 0,
    0.5, -0.5, 0,
    0, 0.75, 0,
  ], 3));
  geometry.setAttribute('skinIndex', new THREE.Uint16BufferAttribute([
    0, 0, 0, 0,
    0, 0, 0, 0,
    0, 0, 0, 0,
  ], 4));
  geometry.setAttribute('skinWeight', new THREE.Float32BufferAttribute([
    1, 0, 0, 0,
    1, 0, 0, 0,
    1, 0, 0, 0,
  ], 4));
  geometry.setIndex([0, 1, 2]);
  geometry.computeVertexNormals();
  const material = new THREE.MeshStandardMaterial({ color: 0xaaccff });
  const bone = new THREE.Bone();
  bone.name = 'held-bone';
  bone.position.set(0.2, 0.35, 0.1);
  const source = new THREE.SkinnedMesh(geometry, material);
  source.add(bone);
  source.bind(new THREE.Skeleton([bone]));
  scene.add(source);
  source.updateWorldMatrix(true, true);
  const canonicalBoneMatrix = bone.matrixWorld.clone();
  const renderer = new FakeRenderer();
  const attachment = new RendererThreeVoxelSpriteScene({
    webgl: renderer as unknown as THREE.WebGLRenderer,
    backend: {
      scene,
      objectFor: (handle) => Number(handle) === 12 ? source : undefined,
      textureDescriptor: () => undefined,
      textureObjectFor: () => undefined,
    },
  });
  const created = attachment.create({
    id: 'skinned-ghost',
    source: {
      kind: 'retained',
      handle: 12 as never,
      capture: {
        resolution: 16,
        azimuthDegrees: 0,
        elevationDegrees: 0,
        near: 0.1,
        far: 20,
      },
    },
    transform: { position: [0, 0, 0], width: 1, height: 1.5 },
    mode: 'ghost-plate',
  });
  assert.equal(created.applied, true);
  const clonedSkinned = scene.children[1]?.getObjectByName(source.name) instanceof THREE.SkinnedMesh
    ? scene.children[1]?.getObjectByName(source.name) as THREE.SkinnedMesh
    : (() => {
        let found: THREE.SkinnedMesh | null = null;
        scene.children[1]?.traverse((object) => {
          if (object instanceof THREE.SkinnedMesh) found = object;
        });
        return found;
      })();
  assert.ok(clonedSkinned instanceof THREE.SkinnedMesh);
  assert.notEqual(clonedSkinned.skeleton, source.skeleton);
  assert.notEqual(clonedSkinned.skeleton.bones[0], bone);
  assert.deepEqual(clonedSkinned.skeleton.bones[0]?.position.toArray(), bone.position.toArray());
  assert.ok(bone.matrixWorld.equals(canonicalBoneMatrix));
  assert.equal(source.material, material);
  attachment.dispose();
  assert.ok(bone.matrixWorld.equals(canonicalBoneMatrix));
  assert.equal(source.material, material);
  source.skeleton.dispose();
  geometry.dispose();
  material.dispose();
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
  readonly visibleLightSnapshots: string[][] = [];

  clear(): void {}
  getClearAlpha(): number { return this.clearAlpha; }
  getClearColor(target: THREE.Color): THREE.Color { return target.copy(this.clearColor); }
  getRenderTarget(): THREE.WebGLRenderTarget | null { return this.renderTarget; }
  getScissor(target: THREE.Vector4): THREE.Vector4 { return target.copy(this.scissor); }
  getScissorTest(): boolean { return this.scissorTest; }
  getViewport(target: THREE.Vector4): THREE.Vector4 { return target.copy(this.viewport); }
  render(scene?: THREE.Scene): void {
    this.visibleLightSnapshots.push(scene === undefined ? [] : scene.children
      .filter((child) => child instanceof THREE.Light && child.visible)
      .map((child) => child.type));
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
