import * as THREE from 'three';
import {
  VoxelSpriteRuntimeCapture,
  type VoxelSpriteFrame,
  type VoxelSpriteFrameTextures,
} from '@rusty-engine/renderer-three';

interface TextureSample {
  readonly checksum: number;
  readonly nonzeroPixels: number;
}

interface CaptureProofReadout {
  readonly first: Readonly<Record<keyof VoxelSpriteFrameTextures, TextureSample>>;
  readonly second: Readonly<Record<keyof VoxelSpriteFrameTextures, TextureSample>>;
  readonly captureCount: number;
  readonly rejectedCaptureCount: number;
  readonly currentFrameBytes: number | null;
  readonly colorChanged: boolean;
  readonly normalChanged: boolean;
  readonly disposed: boolean;
}

interface CaptureProofApi {
  readonly ready: true;
  snapshot(): CaptureProofReadout;
  capture(): string;
  dispose(): { readonly disposed: true };
}

declare global {
  interface Window {
    voxelSpriteCaptureProof?: CaptureProofApi;
  }
}

void boot();

function boot(): void {
  const canvas = requiredElement<HTMLCanvasElement>('proof');
  const readoutElement = requiredElement<HTMLPreElement>('readout');
  const renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: false });
  renderer.setPixelRatio(1);
  renderer.setSize(canvas.width, canvas.height, false);
  renderer.setClearColor(0x111923, 1);

  const source = new THREE.Scene();
  const material = new THREE.MeshBasicMaterial({ color: 0xe34b4b });
  const mesh = new THREE.Mesh(new THREE.BoxGeometry(1.2, 1.7, 0.8), material);
  source.add(mesh);
  const camera = new THREE.OrthographicCamera(-1.4, 1.4, 1.4, -1.4, 0.1, 10);
  camera.position.set(0, 0, 4);
  camera.lookAt(0, 0, 0);
  camera.updateMatrixWorld(true);
  const capture = new VoxelSpriteRuntimeCapture(renderer);

  const firstReceipt = capture.capture({ scene: source, camera, width: 64, height: 64 });
  if (!firstReceipt.applied || firstReceipt.frame === null) {
    throw new Error(firstReceipt.diagnostics[0]?.message ?? 'first runtime capture failed');
  }
  const first = sampleFrame(renderer, firstReceipt.frame);

  material.color.set(0x42d982);
  mesh.rotation.set(0.18, 0.62, 0.12);
  mesh.position.x = 0.18;
  mesh.updateMatrixWorld(true);
  const secondReceipt = capture.capture({ scene: source, camera, width: 64, height: 64 });
  if (!secondReceipt.applied || secondReceipt.frame === null) {
    throw new Error(secondReceipt.diagnostics[0]?.message ?? 'second runtime capture failed');
  }
  const second = sampleFrame(renderer, secondReceipt.frame);
  renderTextures(renderer, secondReceipt.frame.descriptor.textures);

  let disposed = false;
  function snapshot(): CaptureProofReadout {
    const captureReadout = capture.readout();
    const value = Object.freeze({
      first,
      second,
      captureCount: captureReadout.captureCount,
      rejectedCaptureCount: captureReadout.rejectedCaptureCount,
      currentFrameBytes: captureReadout.currentFrame?.estimatedTextureBytes ?? null,
      colorChanged: first.color.checksum !== second.color.checksum,
      normalChanged: first.normal.checksum !== second.normal.checksum,
      disposed,
    });
    readoutElement.textContent = JSON.stringify(value, null, 2);
    return value;
  }

  window.voxelSpriteCaptureProof = {
    ready: true,
    snapshot,
    capture: () => canvas.toDataURL('image/png'),
    dispose: () => {
      if (!disposed) {
        capture.dispose();
        material.dispose();
        mesh.geometry.dispose();
        renderer.dispose();
        disposed = true;
      }
      snapshot();
      return { disposed: true };
    },
  };
  snapshot();
}

function sampleFrame(
  renderer: THREE.WebGLRenderer,
  frame: VoxelSpriteFrame,
): Readonly<Record<keyof VoxelSpriteFrameTextures, TextureSample>> {
  return Object.freeze({
    color: sampleTexture(renderer, frame.descriptor.textures.color),
    depth: sampleTexture(renderer, frame.descriptor.textures.depth),
    normal: sampleTexture(renderer, frame.descriptor.textures.normal),
    coverage: sampleTexture(renderer, frame.descriptor.textures.coverage),
  });
}

function sampleTexture(renderer: THREE.WebGLRenderer, texture: THREE.Texture): TextureSample {
  const target = new THREE.WebGLRenderTarget(32, 32, {
    type: THREE.UnsignedByteType,
    format: THREE.RGBAFormat,
    minFilter: THREE.NearestFilter,
    magFilter: THREE.NearestFilter,
    depthBuffer: false,
    stencilBuffer: false,
  });
  const material = new THREE.MeshBasicMaterial({ map: texture, toneMapped: false });
  const scene = fullscreenScene(material);
  const camera = fullscreenCamera();
  const pixels = new Uint8Array(32 * 32 * 4);
  const priorTarget = renderer.getRenderTarget();
  renderer.setRenderTarget(target);
  renderer.setViewport(0, 0, 32, 32);
  renderer.setClearColor(0x000000, 0);
  renderer.clear(true, true, true);
  renderer.render(scene, camera);
  renderer.readRenderTargetPixels(target, 0, 0, 32, 32, pixels);
  renderer.setRenderTarget(priorTarget);
  let checksum = 0;
  let nonzeroPixels = 0;
  for (let index = 0; index < pixels.length; index += 4) {
    const pixel = pixels[index]! + pixels[index + 1]! + pixels[index + 2]! + pixels[index + 3]!;
    if (pixel > 0) nonzeroPixels += 1;
    checksum = (checksum + pixel * ((index / 4) % 251 + 1)) % 2_147_483_647;
  }
  material.dispose();
  (scene.children[0] as THREE.Mesh).geometry.dispose();
  target.dispose();
  return Object.freeze({ checksum, nonzeroPixels });
}

function renderTextures(renderer: THREE.WebGLRenderer, textures: VoxelSpriteFrameTextures): void {
  const scene = new THREE.Scene();
  const geometry = new THREE.PlaneGeometry(1.8, 1.8);
  const entries = Object.entries(textures) as [keyof VoxelSpriteFrameTextures, THREE.Texture][];
  for (const [index, [name, texture]] of entries.entries()) {
    const material = new THREE.MeshBasicMaterial({ map: texture, toneMapped: false });
    material.name = name;
    const mesh = new THREE.Mesh(geometry, material);
    mesh.position.x = -2.85 + index * 1.9;
    scene.add(mesh);
  }
  const camera = new THREE.OrthographicCamera(-3.8, 3.8, 0.95, -0.95, 0.1, 10);
  camera.position.z = 2;
  renderer.setRenderTarget(null);
  renderer.setViewport(0, 0, 768, 192);
  renderer.setClearColor(0x111923, 1);
  renderer.clear(true, true, true);
  renderer.render(scene, camera);
  for (const child of scene.children) ((child as THREE.Mesh).material as THREE.Material).dispose();
  geometry.dispose();
}

function fullscreenScene(material: THREE.Material): THREE.Scene {
  const scene = new THREE.Scene();
  scene.add(new THREE.Mesh(new THREE.PlaneGeometry(2, 2), material));
  return scene;
}

function fullscreenCamera(): THREE.OrthographicCamera {
  const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
  camera.position.z = 0.5;
  return camera;
}

function requiredElement<T extends HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (element === null) throw new Error(`missing #${id}`);
  return element as T;
}
