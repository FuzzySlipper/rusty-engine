import * as THREE from 'three';
import {
  VOXEL_SPRITE_ENHANCEMENT_MODES,
  VoxelSpriteEnhancement,
  VoxelSpriteRuntimeCapture,
  type VoxelSpriteEnhancementMode,
  type VoxelSpriteEnhancementReadout,
} from '@rusty-engine/renderer-three';

interface EnhancementProofReadout {
  readonly captureCount: number;
  readonly initialChecksums: readonly number[];
  readonly finalChecksums: readonly number[];
  readonly distinctInitialModes: number;
  readonly controlsChangedWithoutRecapture: boolean;
  readonly modes: Readonly<Record<VoxelSpriteEnhancementMode, VoxelSpriteEnhancementReadout>>;
  readonly totalExpectedDrawCalls: number;
  readonly totalGeometrySamples: number;
  readonly disposed: boolean;
}

interface EnhancementProofApi {
  readonly ready: true;
  snapshot(): EnhancementProofReadout;
  capture(): string;
  dispose(): { readonly disposed: true };
}

declare global {
  interface Window {
    voxelSpriteEnhancementProof?: EnhancementProofApi;
  }
}

void boot();

function boot(): void {
  const canvas = requiredElement<HTMLCanvasElement>('proof');
  const readoutElement = requiredElement<HTMLPreElement>('readout');
  const renderer = new THREE.WebGLRenderer({
    canvas,
    antialias: false,
    alpha: false,
    preserveDrawingBuffer: true,
  });
  renderer.setPixelRatio(1);
  renderer.setSize(canvas.width, canvas.height, false);
  renderer.setClearColor(0x111923, 1);

  const source = sourceCharacter();
  const captureCamera = new THREE.OrthographicCamera(-1.15, 1.15, 1.45, -1.45, 0.1, 10);
  captureCamera.position.set(0.25, 0.15, 4);
  captureCamera.lookAt(0, 0, 0);
  captureCamera.updateMatrixWorld(true);
  const capture = new VoxelSpriteRuntimeCapture(renderer);
  const receipt = capture.capture({
    scene: source.scene,
    camera: captureCamera,
    width: 96,
    height: 128,
    bounds: new THREE.Box3().setFromObject(source.root, true),
  });
  if (!receipt.applied || receipt.frame === null) {
    throw new Error(receipt.diagnostics[0]?.message ?? 'runtime character capture failed');
  }

  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(35, canvas.width / canvas.height, 0.1, 50);
  camera.position.set(0, 0.1, 12);
  camera.lookAt(0, 0, 0);
  camera.updateMatrixWorld(true);
  const enhancements = VOXEL_SPRITE_ENHANCEMENT_MODES.map((mode, index) => {
    const enhancement = new VoxelSpriteEnhancement(
      {
        frame: receipt.frame!,
        captureCpuSubmissionMilliseconds: receipt.readout.cpuSubmissionMilliseconds,
      },
      {
        mode,
        width: 1.75,
        height: 2.45,
        sampleColumns: 32,
        sampleRows: 44,
        depthAmplitude: 0.42,
        depthQuantizationSteps: 7,
        depthDilationTexels: 1,
        splatFootprint: 1.15,
        splatOverlap: 0.2,
        normalInfluence: 0.72,
        normalOrientationBlend: 0.4,
        baseSpriteContribution: 0.58,
      },
    );
    enhancement.object.position.set((index - 2) * 2.05, 0, 0);
    scene.add(enhancement.object);
    return enhancement;
  });
  const proofTarget = new THREE.WebGLRenderTarget(canvas.width, canvas.height, {
    type: THREE.UnsignedByteType,
    format: THREE.RGBAFormat,
    minFilter: THREE.NearestFilter,
    magFilter: THREE.NearestFilter,
    depthBuffer: true,
    stencilBuffer: false,
  });

  const initialChecksums = renderAndMeasure(renderer, proofTarget, scene, camera, enhancements);
  for (const enhancement of enhancements) {
    enhancement.configure({
      depthAmplitude: 0.7,
      depthQuantizationSteps: 4,
      splatFootprint: 1.45,
      splatOverlap: 0.45,
      normalInfluence: 0.92,
      normalOrientationBlend: 0.75,
      baseSpriteContribution: 0.4,
      viewAngleFalloff: 1.5,
      lightDirection: [-0.8, 0.35, 1],
    });
  }
  camera.position.set(1.3, 0.55, 11.5);
  camera.lookAt(0, 0.1, 0);
  camera.updateMatrixWorld(true);
  const finalChecksums = renderAndMeasure(renderer, proofTarget, scene, camera, enhancements);

  let disposed = false;
  function snapshot(): EnhancementProofReadout {
    const byMode = Object.fromEntries(enhancements.map((enhancement) => [
      enhancement.readout().mode,
      enhancement.readout(),
    ])) as unknown as Readonly<Record<VoxelSpriteEnhancementMode, VoxelSpriteEnhancementReadout>>;
    const readout = Object.freeze({
      captureCount: capture.readout().captureCount,
      initialChecksums,
      finalChecksums,
      distinctInitialModes: new Set(initialChecksums).size,
      controlsChangedWithoutRecapture: capture.readout().captureCount === 1
        && enhancements.every((enhancement) => enhancement.readout().revision === 2),
      modes: byMode,
      totalExpectedDrawCalls: enhancements.reduce(
        (total, enhancement) => total + enhancement.readout().expectedDrawCalls,
        0,
      ),
      totalGeometrySamples: enhancements.reduce(
        (total, enhancement) => total + enhancement.readout().geometrySampleCount,
        0,
      ),
      disposed,
    });
    readoutElement.textContent = JSON.stringify(readout, null, 2);
    return readout;
  }

  window.voxelSpriteEnhancementProof = {
    ready: true,
    snapshot,
    capture: () => canvas.toDataURL('image/png'),
    dispose: () => {
      if (!disposed) {
        for (const enhancement of enhancements) enhancement.dispose();
        capture.dispose();
        proofTarget.dispose();
        source.dispose();
        renderer.dispose();
        disposed = true;
      }
      snapshot();
      return { disposed: true };
    },
  };
  snapshot();
}

function renderAndMeasure(
  renderer: THREE.WebGLRenderer,
  target: THREE.WebGLRenderTarget,
  scene: THREE.Scene,
  camera: THREE.PerspectiveCamera,
  enhancements: readonly VoxelSpriteEnhancement[],
): readonly number[] {
  for (const enhancement of enhancements) enhancement.faceCamera(camera);
  const started = performance.now();
  renderer.setRenderTarget(target);
  renderer.setViewport(0, 0, target.width, target.height);
  renderer.clear(true, true, true);
  renderer.render(scene, camera);
  const elapsed = performance.now() - started;
  for (const enhancement of enhancements) enhancement.recordSteadyStateFrame(elapsed);
  const pixels = new Uint8Array(target.width * target.height * 4);
  renderer.readRenderTargetPixels(target, 0, 0, target.width, target.height, pixels);
  const checksums = regionChecksums(pixels, target.width, target.height, enhancements.length);
  renderer.setRenderTarget(null);
  renderer.setViewport(0, 0, renderer.domElement.width, renderer.domElement.height);
  renderer.clear(true, true, true);
  renderer.render(scene, camera);
  return Object.freeze(checksums);
}

function regionChecksums(
  pixels: Uint8Array,
  width: number,
  height: number,
  regions: number,
): number[] {
  const checksums = new Array<number>(regions).fill(0);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const region = Math.min(regions - 1, Math.floor(x / width * regions));
      const offset = (y * width + x) * 4;
      const value = pixels[offset]! + pixels[offset + 1]! * 3 + pixels[offset + 2]! * 7;
      checksums[region] = (checksums[region]! + value * ((x + y * 13) % 251 + 1)) % 2_147_483_647;
    }
  }
  return checksums;
}

function sourceCharacter(): { readonly scene: THREE.Scene; readonly root: THREE.Group; dispose(): void } {
  const scene = new THREE.Scene();
  const root = new THREE.Group();
  const geometries: THREE.BufferGeometry[] = [];
  const materials: THREE.Material[] = [];
  const part = (
    geometry: THREE.BufferGeometry,
    color: THREE.ColorRepresentation,
    position: readonly [number, number, number],
    rotation: readonly [number, number, number] = [0, 0, 0],
  ): void => {
    const material = new THREE.MeshBasicMaterial({ color });
    const mesh = new THREE.Mesh(geometry, material);
    mesh.position.set(...position);
    mesh.rotation.set(...rotation);
    geometries.push(geometry);
    materials.push(material);
    root.add(mesh);
  };
  part(new THREE.IcosahedronGeometry(0.38, 1), 0xf0c997, [0, 0.84, 0.02]);
  part(new THREE.BoxGeometry(0.85, 0.95, 0.38), 0x3f76b6, [0, 0.12, 0]);
  part(new THREE.BoxGeometry(0.24, 0.9, 0.24), 0x2f568d, [-0.57, 0.14, 0], [0, 0, -0.18]);
  part(new THREE.BoxGeometry(0.24, 0.9, 0.24), 0x2f568d, [0.57, 0.14, 0], [0, 0, 0.18]);
  part(new THREE.BoxGeometry(0.3, 1, 0.32), 0x55442f, [-0.25, -0.83, 0]);
  part(new THREE.BoxGeometry(0.3, 1, 0.32), 0x55442f, [0.25, -0.83, 0]);
  part(new THREE.ConeGeometry(0.38, 0.62, 6), 0x8b3f54, [0, 1.24, 0], [0, 0, 0.12]);
  root.rotation.y = -0.22;
  scene.add(root);
  scene.updateMatrixWorld(true);
  return {
    scene,
    root,
    dispose: () => {
      for (const geometry of geometries) geometry.dispose();
      for (const material of materials) material.dispose();
    },
  };
}

function requiredElement<T extends HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (element === null) throw new Error(`missing #${id}`);
  return element as T;
}
