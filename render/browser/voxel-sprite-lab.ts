import * as THREE from 'three';
import {
  VOXEL_SPRITE_ENHANCEMENT_MODES,
  VoxelSpriteEnhancement,
  VoxelSpriteFrame,
  VoxelSpriteRuntimeCapture,
  type VoxelSpriteEnhancementConfig,
  type VoxelSpriteEnhancementMode,
} from '@rusty-engine/renderer-three';

export interface VoxelSpriteLabHandle {
  dispose(): void;
}

type DisplayMode = 'comparison' | VoxelSpriteEnhancementMode;
type CaptureColorMode = 'authored' | 'silhouette' | 'faceted';

export function mountVoxelSpriteLab(root: HTMLElement): VoxelSpriteLabHandle {
  return new VoxelSpriteLab(root);
}

class VoxelSpriteLab implements VoxelSpriteLabHandle {
  readonly #root: HTMLElement;
  readonly #renderer: THREE.WebGLRenderer;
  readonly #scene = new THREE.Scene();
  readonly #viewerCamera: THREE.PerspectiveCamera;
  readonly #captureCamera = new THREE.PerspectiveCamera(35, 1, 0.1, 10);
  readonly #capture: VoxelSpriteRuntimeCapture;
  readonly #sourceModel = createSourceModel();
  readonly #prepared = createPreparedFrame(96, 128);
  readonly #enhancements: VoxelSpriteEnhancement[] = [];
  #animationFrame: number | null = null;
  #disposed = false;
  #displayMode: DisplayMode = 'comparison';
  #liveRevision = 0;
  #lastMetricsAt = 0;
  #provenance = '';

  constructor(root: HTMLElement) {
    this.#root = root;
    const canvas = element<HTMLCanvasElement>(root, 'lab-canvas');
    this.#renderer = new THREE.WebGLRenderer({ canvas, antialias: false, alpha: false });
    this.#renderer.setPixelRatio(1);
    this.#renderer.setSize(canvas.width, canvas.height, false);
    this.#renderer.setClearColor(0x111923, 1);
    this.#viewerCamera = new THREE.PerspectiveCamera(35, canvas.width / canvas.height, 0.1, 50);
    this.#viewerCamera.position.set(0, 0.2, 11.5);
    this.#viewerCamera.lookAt(0, 0, 0);
    this.#viewerCamera.updateMatrixWorld(true);
    this.#capture = new VoxelSpriteRuntimeCapture(this.#renderer);

    const initial = this.#captureRuntime();
    for (const [index, mode] of VOXEL_SPRITE_ENHANCEMENT_MODES.entries()) {
      const enhancement = new VoxelSpriteEnhancement(initial, {
        ...this.#liveConfig(),
        mode,
        width: 1.65,
        height: 2.4,
        sampleColumns: 24,
        sampleRows: 32,
      });
      enhancement.object.position.x = (index - 2) * 2;
      this.#scene.add(enhancement.object);
      this.#enhancements.push(enhancement);
    }
    this.#provenance = this.#runtimeProvenance();
    this.#bindControls();
    this.#applyLayout();
    this.#setStatus('Ready');
    this.#updateMetrics(performance.now());
    this.#animationFrame = requestAnimationFrame(this.#render);
  }

  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    if (this.#animationFrame !== null) cancelAnimationFrame(this.#animationFrame);
    this.#animationFrame = null;
    for (const enhancement of this.#enhancements) enhancement.dispose();
    this.#capture.dispose();
    this.#sourceModel.dispose();
    this.#prepared.dispose();
    this.#renderer.dispose();
    this.#setText('draw-count', '0');
    this.#setText('sample-count', '0');
    this.#setText('texture-bytes', '0 B');
    this.#setStatus('Disposed');
    button(this.#root, 'recapture').disabled = true;
    button(this.#root, 'dispose-lab').disabled = true;
    for (const control of this.#root.querySelectorAll<HTMLInputElement | HTMLSelectElement>('input, select')) {
      control.disabled = true;
    }
  }

  readonly #render = (now: number): void => {
    if (this.#disposed) return;
    for (const enhancement of this.#enhancements) {
      if (enhancement.object.visible) enhancement.faceCamera(this.#viewerCamera);
    }
    const started = performance.now();
    this.#renderer.setRenderTarget(null);
    this.#renderer.setViewport(0, 0, this.#renderer.domElement.width, this.#renderer.domElement.height);
    this.#renderer.clear(true, true, true);
    this.#renderer.render(this.#scene, this.#viewerCamera);
    const elapsed = performance.now() - started;
    for (const enhancement of this.#enhancements) {
      if (enhancement.object.visible) enhancement.recordSteadyStateFrame(elapsed);
    }
    if (now - this.#lastMetricsAt >= 200) this.#updateMetrics(now);
    this.#animationFrame = requestAnimationFrame(this.#render);
  };

  #captureRuntime(): { readonly frame: VoxelSpriteFrame; readonly captureCpuSubmissionMilliseconds: number | null } {
    const resolution = numericSelect(this.#root, 'capture-resolution');
    const near = numericInput(this.#root, 'capture-near');
    const far = numericInput(this.#root, 'capture-far');
    const azimuth = THREE.MathUtils.degToRad(numericInput(this.#root, 'capture-azimuth'));
    const elevation = THREE.MathUtils.degToRad(numericInput(this.#root, 'capture-elevation'));
    const colorMode = selectValue<CaptureColorMode>(this.#root, 'capture-color-mode');
    this.#sourceModel.setColorMode(colorMode);
    this.#captureCamera.near = near;
    this.#captureCamera.far = far;
    this.#captureCamera.aspect = 1;
    this.#captureCamera.updateProjectionMatrix();
    const radius = 4.2;
    this.#captureCamera.position.set(
      Math.sin(azimuth) * Math.cos(elevation) * radius,
      Math.sin(elevation) * radius,
      Math.cos(azimuth) * Math.cos(elevation) * radius,
    );
    this.#captureCamera.lookAt(0, 0, 0);
    this.#captureCamera.updateMatrixWorld(true);
    const receipt = this.#capture.capture({
      scene: this.#sourceModel.scene,
      camera: this.#captureCamera,
      width: resolution,
      height: resolution,
      bounds: this.#sourceModel.bounds(),
    });
    if (!receipt.applied || receipt.frame === null) {
      throw new Error(receipt.diagnostics[0]?.message ?? 'runtime capture failed');
    }
    return {
      frame: receipt.frame,
      captureCpuSubmissionMilliseconds: receipt.readout.cpuSubmissionMilliseconds,
    };
  }

  #bindControls(): void {
    select(this.#root, 'source-kind').addEventListener('change', () => {
      if (this.#disposed) return;
      try {
        const source = selectValue<'runtime' | 'prepared'>(this.#root, 'source-kind');
        if (source === 'prepared') {
          this.#replaceSource({ frame: this.#prepared.frame, captureCpuSubmissionMilliseconds: null });
          this.#provenance = 'Prepared frame · caller-owned procedural RGBA8 textures';
        } else {
          const frame = this.#capture.currentFrame();
          if (frame === null) throw new Error('no runtime capture is available');
          this.#replaceSource({
            frame,
            captureCpuSubmissionMilliseconds: this.#capture.readout().cpuSubmissionMilliseconds,
          });
          this.#provenance = this.#runtimeProvenance();
        }
        this.#liveRevision += 1;
        this.#setStatus('Source switched without recapture');
        this.#updateMetrics(performance.now());
      } catch (cause) {
        this.#setStatus(messageFrom(cause));
      }
    });

    button(this.#root, 'recapture').addEventListener('click', () => {
      if (this.#disposed) return;
      try {
        const source = this.#captureRuntime();
        this.#replaceSource(source);
        select(this.#root, 'source-kind').value = 'runtime';
        this.#provenance = this.#runtimeProvenance();
        this.#setStatus('Runtime recapture applied');
        this.#updateMetrics(performance.now());
      } catch (cause) {
        this.#setStatus(messageFrom(cause));
      }
    });

    select(this.#root, 'display-mode').addEventListener('change', () => {
      if (this.#disposed) return;
      this.#displayMode = selectValue<DisplayMode>(this.#root, 'display-mode');
      this.#liveRevision += 1;
      this.#applyLayout();
      this.#setStatus('Display mode updated immediately');
      this.#updateMetrics(performance.now());
    });

    for (const id of LIVE_CONTROL_IDS) {
      const control = element<HTMLInputElement | HTMLSelectElement>(this.#root, id);
      control.addEventListener('input', this.#applyLiveControls);
      control.addEventListener('change', this.#applyLiveControls);
    }
    button(this.#root, 'dispose-lab').addEventListener('click', () => this.dispose());
  }

  readonly #applyLiveControls = (): void => {
    if (this.#disposed) return;
    try {
      const config = this.#liveConfig();
      for (const enhancement of this.#enhancements) enhancement.configure(config);
      this.#liveRevision += 1;
      this.#setStatus('Reconstruction updated without recapture');
      this.#updateMetrics(performance.now());
    } catch (cause) {
      this.#setStatus(messageFrom(cause));
    }
  };

  #liveConfig(): Partial<VoxelSpriteEnhancementConfig> {
    return {
      depthScale: selectValue<'normalized' | 'world'>(this.#root, 'depth-scale'),
      depthAmplitude: numericInput(this.#root, 'depth-amplitude'),
      depthClamp: numericInput(this.#root, 'depth-clamp'),
      depthQuantizationSteps: numericInput(this.#root, 'depth-quantization'),
      depthDilationTexels: numericInput(this.#root, 'depth-dilation'),
      depthConfidenceThreshold: numericInput(this.#root, 'depth-confidence'),
      splatFootprint: numericInput(this.#root, 'splat-footprint'),
      splatOverlap: numericInput(this.#root, 'splat-overlap'),
      normalOrientationBlend: numericInput(this.#root, 'orientation-blend'),
      normalInfluence: numericInput(this.#root, 'normal-influence'),
      baseSpriteContribution: numericInput(this.#root, 'base-contribution'),
      viewAngleFalloff: numericInput(this.#root, 'view-falloff'),
    };
  }

  #replaceSource(source: { readonly frame: VoxelSpriteFrame; readonly captureCpuSubmissionMilliseconds: number | null }): void {
    for (const enhancement of this.#enhancements) enhancement.replaceSource(source);
  }

  #applyLayout(): void {
    const comparison = this.#displayMode === 'comparison';
    for (const [index, enhancement] of this.#enhancements.entries()) {
      const mode = enhancement.readout().mode;
      enhancement.object.visible = comparison || mode === this.#displayMode;
      enhancement.object.position.x = comparison ? (index - 2) * 2 : 0;
      enhancement.object.scale.setScalar(comparison ? 1 : 1.65);
    }
    for (const label of this.#root.querySelectorAll<HTMLElement>('[data-mode]')) {
      label.hidden = !comparison && label.dataset['mode'] !== this.#displayMode;
    }
    const labels = element<HTMLElement>(this.#root, 'mode-labels');
    labels.style.gridTemplateColumns = comparison ? 'repeat(5, 1fr)' : '1fr';
  }

  #runtimeProvenance(): string {
    const readout = this.#capture.readout();
    return `Runtime capture #${String(readout.captureCount)} · retained Three model · ${select(this.#root, 'capture-resolution').value}²`;
  }

  #updateMetrics(now: number): void {
    this.#lastMetricsAt = now;
    const visible = this.#enhancements.filter((enhancement) => enhancement.object.visible);
    const source = visible[0]?.readout() ?? this.#enhancements[0]?.readout();
    this.#setText('source-provenance', this.#provenance);
    this.#setText('capture-revision', String(this.#capture.readout().captureCount));
    this.#setText('live-revision', String(this.#liveRevision));
    this.#setText(
      'capture-ms',
      source?.captureCpuSubmissionMilliseconds === null || source === undefined
        ? 'prepared / n/a'
        : `${source.captureCpuSubmissionMilliseconds.toFixed(2)} ms`,
    );
    this.#setText('texture-bytes', source === undefined ? '0 B' : formatBytes(source.frameTextureBytes));
    this.#setText('draw-count', String(visible.reduce((sum, item) => sum + item.readout().expectedDrawCalls, 0)));
    this.#setText('sample-count', String(visible.reduce((sum, item) => sum + item.readout().geometrySampleCount, 0)));
    const steady = source?.steadyStateCpuSubmissionMilliseconds;
    this.#setText('steady-ms', steady === null || steady === undefined ? '—' : `${steady.toFixed(2)} ms`);
  }

  #setStatus(value: string): void {
    this.#setText('lab-status', value);
  }

  #setText(id: string, value: string): void {
    element<HTMLOutputElement>(this.#root, id).textContent = value;
  }
}

const LIVE_CONTROL_IDS = Object.freeze([
  'depth-scale',
  'depth-amplitude',
  'depth-clamp',
  'depth-quantization',
  'depth-dilation',
  'depth-confidence',
  'splat-footprint',
  'splat-overlap',
  'orientation-blend',
  'normal-influence',
  'base-contribution',
  'view-falloff',
] as const);

interface SourceModel {
  readonly scene: THREE.Scene;
  bounds(): THREE.Box3;
  setColorMode(mode: CaptureColorMode): void;
  dispose(): void;
}

function createSourceModel(): SourceModel {
  const scene = new THREE.Scene();
  const root = new THREE.Group();
  const geometries: THREE.BufferGeometry[] = [];
  const materials: THREE.MeshBasicMaterial[] = [];
  const authored: THREE.ColorRepresentation[] = [];
  const addPart = (
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
    authored.push(color);
    root.add(mesh);
  };
  addPart(new THREE.IcosahedronGeometry(0.38, 1), 0xecc28d, [0, 0.84, 0.02]);
  addPart(new THREE.BoxGeometry(0.86, 0.95, 0.4), 0x3c76b8, [0, 0.13, 0]);
  addPart(new THREE.BoxGeometry(0.24, 0.9, 0.24), 0x315b91, [-0.57, 0.15, 0], [0, 0, -0.18]);
  addPart(new THREE.BoxGeometry(0.24, 0.9, 0.24), 0x315b91, [0.57, 0.15, 0], [0, 0, 0.18]);
  addPart(new THREE.BoxGeometry(0.3, 1, 0.32), 0x57452f, [-0.25, -0.83, 0]);
  addPart(new THREE.BoxGeometry(0.3, 1, 0.32), 0x57452f, [0.25, -0.83, 0]);
  addPart(new THREE.ConeGeometry(0.38, 0.62, 6), 0x8f4058, [0, 1.24, 0], [0, 0, 0.12]);
  scene.add(root);
  scene.updateMatrixWorld(true);
  return {
    scene,
    bounds: () => new THREE.Box3().setFromObject(root, true),
    setColorMode: (mode) => {
      for (const [index, material] of materials.entries()) {
        if (mode === 'authored') material.color.set(authored[index]!);
        else if (mode === 'silhouette') material.color.set(0xd8e6ef);
        else material.color.setHSL((index * 0.13 + 0.55) % 1, 0.55, 0.48 + index % 2 * 0.12);
      }
    },
    dispose: () => {
      for (const geometry of geometries) geometry.dispose();
      for (const material of materials) material.dispose();
    },
  };
}

function createPreparedFrame(width: number, height: number): { readonly frame: VoxelSpriteFrame; dispose(): void } {
  const color = new Uint8Array(width * height * 4);
  const depth = new Uint8Array(width * height * 4);
  const normal = new Uint8Array(width * height * 4);
  const coverage = new Uint8Array(width * height * 4);
  for (let row = 0; row < height; row += 1) {
    for (let column = 0; column < width; column += 1) {
      const x = (column + 0.5) / width * 2 - 1;
      const y = (row + 0.5) / height * 2 - 1;
      const head = Math.hypot(x / 0.34, (y - 0.48) / 0.28) <= 1;
      const torso = Math.abs(x) <= 0.44 && y > -0.4 && y <= 0.32;
      const legs = y <= -0.4 && y > -0.96 && (Math.abs(x - 0.2) < 0.14 || Math.abs(x + 0.2) < 0.14);
      const covered = head || torso || legs;
      const offset = (row * width + column) * 4;
      const radial = Math.min(1, Math.hypot(x * 0.8, y * 0.55));
      writePixel(color, offset, covered ? (head ? [236, 183, 133] : torso ? [166, 74, 131] : [64, 48, 38]) : [0, 0, 0], covered ? 255 : 0);
      const depthValue = Math.round((0.28 + radial * 0.38) * 255);
      writePixel(depth, offset, [depthValue, depthValue, depthValue], covered ? 255 : 0);
      const nx = THREE.MathUtils.clamp(x * 0.7, -0.8, 0.8);
      const ny = THREE.MathUtils.clamp(y * 0.35, -0.65, 0.65);
      const nz = Math.sqrt(Math.max(0.05, 1 - nx * nx - ny * ny));
      writePixel(normal, offset, [Math.round((nx * 0.5 + 0.5) * 255), Math.round((ny * 0.5 + 0.5) * 255), Math.round((nz * 0.5 + 0.5) * 255)], covered ? 255 : 0);
      writePixel(coverage, offset, [covered ? 255 : 0, covered ? 255 : 0, covered ? 255 : 0], covered ? 255 : 0);
    }
  }
  const textures = [
    dataTexture(color, width, height, true),
    dataTexture(depth, width, height, false),
    dataTexture(normal, width, height, false),
    dataTexture(coverage, width, height, false),
  ] as const;
  const frame = VoxelSpriteFrame.borrowed({
    width,
    height,
    textures: { color: textures[0], depth: textures[1], normal: textures[2], coverage: textures[3] },
    depth: { encoding: 'linear-view-depth-unorm8', near: 0.1, far: 10 },
    normalSpace: 'view',
    capture: {
      projection: 'orthographic',
      basis: { position: [0, 0, 4], right: [1, 0, 0], up: [0, 1, 0], forward: [0, 0, -1] },
      bounds: { minimum: [-1, -1.4, -0.5], maximum: [1, 1.4, 0.5] },
    },
  });
  return {
    frame,
    dispose: () => {
      frame.dispose();
      for (const texture of textures) texture.dispose();
    },
  };
}

function writePixel(
  target: Uint8Array,
  offset: number,
  rgb: readonly [number, number, number],
  alpha: number,
): void {
  target[offset] = rgb[0];
  target[offset + 1] = rgb[1];
  target[offset + 2] = rgb[2];
  target[offset + 3] = alpha;
}

function dataTexture(data: Uint8Array, width: number, height: number, srgb: boolean): THREE.DataTexture {
  const texture = new THREE.DataTexture(data, width, height, THREE.RGBAFormat, THREE.UnsignedByteType);
  texture.colorSpace = srgb ? THREE.SRGBColorSpace : THREE.NoColorSpace;
  texture.minFilter = THREE.NearestFilter;
  texture.magFilter = THREE.NearestFilter;
  texture.generateMipmaps = false;
  texture.needsUpdate = true;
  return texture;
}

function element<T extends HTMLElement>(root: HTMLElement, id: string): T {
  const found = root.querySelector(`#${id}`);
  if (!(found instanceof HTMLElement)) throw new Error(`missing #${id}`);
  return found as T;
}

function select(root: HTMLElement, id: string): HTMLSelectElement {
  const found = element<HTMLElement>(root, id);
  if (!(found instanceof HTMLSelectElement)) throw new TypeError(`#${id} must be a select`);
  return found;
}

function button(root: HTMLElement, id: string): HTMLButtonElement {
  const found = element<HTMLElement>(root, id);
  if (!(found instanceof HTMLButtonElement)) throw new TypeError(`#${id} must be a button`);
  return found;
}

function selectValue<T extends string>(root: HTMLElement, id: string): T {
  return select(root, id).value as T;
}

function numericSelect(root: HTMLElement, id: string): number {
  const value = Number(select(root, id).value);
  if (!Number.isFinite(value)) throw new TypeError(`#${id} must contain a finite number`);
  return value;
}

function numericInput(root: HTMLElement, id: string): number {
  const found = element<HTMLElement>(root, id);
  if (!(found instanceof HTMLInputElement)) throw new TypeError(`#${id} must be an input`);
  const value = found.valueAsNumber;
  if (!Number.isFinite(value)) throw new TypeError(`#${id} must contain a finite number`);
  return value;
}

function formatBytes(value: number): string {
  return value < 1024 ? `${String(value)} B` : `${(value / 1024).toFixed(1)} KiB`;
}

function messageFrom(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}
