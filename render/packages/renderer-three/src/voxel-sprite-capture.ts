import * as THREE from 'three';

export const VOXEL_SPRITE_CAPTURE_MIN_RESOLUTION = 8;
export const VOXEL_SPRITE_CAPTURE_MAX_RESOLUTION = 4096;

export type VoxelSpriteFrameProvenance = 'prepared' | 'runtime-capture';

export interface VoxelSpriteFrameTextures {
  readonly color: THREE.Texture;
  readonly depth: THREE.Texture;
  readonly normal: THREE.Texture;
  readonly coverage: THREE.Texture;
}

export interface VoxelSpriteCaptureBasis {
  readonly position: readonly [number, number, number];
  readonly right: readonly [number, number, number];
  readonly up: readonly [number, number, number];
  readonly forward: readonly [number, number, number];
}

export interface VoxelSpriteFrameBounds {
  readonly minimum: readonly [number, number, number];
  readonly maximum: readonly [number, number, number];
}

export interface VoxelSpriteFrameDescriptor {
  readonly schemaVersion: 1;
  readonly width: number;
  readonly height: number;
  readonly textures: VoxelSpriteFrameTextures;
  readonly provenance: VoxelSpriteFrameProvenance;
  readonly depth: {
    /** Linear view distance normalized from near to far and stored in RGBA8 red. */
    readonly encoding: 'linear-view-depth-unorm8';
    readonly near: number;
    readonly far: number;
  };
  readonly normalSpace: 'view';
  readonly capture: {
    readonly projection: 'perspective' | 'orthographic';
    readonly basis: VoxelSpriteCaptureBasis;
    readonly bounds: VoxelSpriteFrameBounds;
  };
}

export interface VoxelSpriteFrameReadout {
  readonly schemaVersion: 1;
  readonly width: number;
  readonly height: number;
  readonly provenance: VoxelSpriteFrameProvenance;
  readonly estimatedTextureBytes: number;
  readonly disposed: boolean;
}

export interface PreparedVoxelSpriteFrameInput
  extends Omit<VoxelSpriteFrameDescriptor, 'schemaVersion' | 'provenance'> {
  /** Borrowed textures remain caller-owned and are never disposed by the frame. */
  readonly textures: VoxelSpriteFrameTextures;
}

/**
 * Backend-local texture bundle shared by prepared and runtime-captured producers.
 * It deliberately carries Three resources and is not a renderer-neutral contract.
 */
export class VoxelSpriteFrame {
  readonly descriptor: VoxelSpriteFrameDescriptor;
  readonly #disposeOwnedResources: (() => void) | null;
  #disposed = false;

  private constructor(
    descriptor: VoxelSpriteFrameDescriptor,
    disposeOwnedResources: (() => void) | null,
  ) {
    this.descriptor = descriptor;
    this.#disposeOwnedResources = disposeOwnedResources;
  }

  static borrowed(input: PreparedVoxelSpriteFrameInput): VoxelSpriteFrame {
    return new VoxelSpriteFrame(
      validatedDescriptor({ ...input, schemaVersion: 1, provenance: 'prepared' }),
      null,
    );
  }

  static owned(
    descriptor: VoxelSpriteFrameDescriptor,
    disposeOwnedResources: () => void,
  ): VoxelSpriteFrame {
    return new VoxelSpriteFrame(validatedDescriptor(descriptor), disposeOwnedResources);
  }

  get disposed(): boolean {
    return this.#disposed;
  }

  readout(): VoxelSpriteFrameReadout {
    return Object.freeze({
      schemaVersion: 1,
      width: this.descriptor.width,
      height: this.descriptor.height,
      provenance: this.descriptor.provenance,
      estimatedTextureBytes: this.descriptor.width * this.descriptor.height * 4 * 4,
      disposed: this.#disposed,
    });
  }

  dispose(): void {
    if (this.#disposed) return;
    this.#disposeOwnedResources?.();
    this.#disposed = true;
  }
}

export type VoxelSpriteCaptureCamera = THREE.PerspectiveCamera | THREE.OrthographicCamera;

export interface VoxelSpriteCaptureRequest {
  readonly scene: THREE.Scene;
  readonly camera: VoxelSpriteCaptureCamera;
  readonly width: number;
  readonly height: number;
  readonly bounds?: THREE.Box3;
  readonly coverageAlphaCutoff?: number;
}

export type VoxelSpriteCaptureDiagnosticCode =
  | 'capture_disposed'
  | 'invalid_capture_request'
  | 'capture_failed';

export interface VoxelSpriteCaptureDiagnostic {
  readonly code: VoxelSpriteCaptureDiagnosticCode;
  readonly message: string;
}

export interface VoxelSpriteCaptureReadout {
  readonly schemaVersion: 1;
  readonly revision: number;
  readonly captureCount: number;
  readonly rejectedCaptureCount: number;
  readonly cpuSubmissionMilliseconds: number | null;
  readonly currentFrame: VoxelSpriteFrameReadout | null;
  readonly disposed: boolean;
  readonly limitations: readonly [
    'rendered-color-not-albedo',
    'rgba8-linear-depth',
    'view-space-normal-pass',
    'normal-pass-uses-separate-coverage-mask',
    'gpu-time-not-measured',
  ];
}

export interface VoxelSpriteCaptureReceipt {
  readonly applied: boolean;
  readonly revision: number;
  readonly frame: VoxelSpriteFrame | null;
  readonly diagnostics: readonly VoxelSpriteCaptureDiagnostic[];
  readonly readout: VoxelSpriteCaptureReadout;
}

interface CaptureTargets {
  readonly color: THREE.WebGLRenderTarget;
  readonly depth: THREE.WebGLRenderTarget;
  readonly normal: THREE.WebGLRenderTarget;
  readonly coverage: THREE.WebGLRenderTarget;
  readonly hardwareDepth: THREE.DepthTexture;
}

interface RendererState {
  readonly autoClear: boolean;
  readonly clearAlpha: number;
  readonly clearColor: THREE.Color;
  readonly renderTarget: THREE.WebGLRenderTarget | null;
  readonly scissor: THREE.Vector4;
  readonly scissorTest: boolean;
  readonly viewport: THREE.Vector4;
  readonly xrEnabled: boolean;
}

const FULLSCREEN_CAMERA = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
const FULLSCREEN_GEOMETRY = new THREE.PlaneGeometry(2, 2);
const LIMITATIONS = Object.freeze([
  'rendered-color-not-albedo',
  'rgba8-linear-depth',
  'view-space-normal-pass',
  'normal-pass-uses-separate-coverage-mask',
  'gpu-time-not-measured',
] as const);

/** Explicit, atomic, deliberately triggered runtime capture owner. */
export class VoxelSpriteRuntimeCapture {
  readonly #renderer: THREE.WebGLRenderer;
  readonly #normalMaterial = new THREE.MeshNormalMaterial({
    side: THREE.DoubleSide,
    blending: THREE.NoBlending,
  });
  readonly #depthResolveMaterial = depthResolveMaterial();
  readonly #coverageResolveMaterial = coverageResolveMaterial();
  readonly #depthResolveScene = fullscreenScene(this.#depthResolveMaterial);
  readonly #coverageResolveScene = fullscreenScene(this.#coverageResolveMaterial);
  #captureCount = 0;
  #currentFrame: VoxelSpriteFrame | null = null;
  #disposed = false;
  #lastCpuSubmissionMilliseconds: number | null = null;
  #rejectedCaptureCount = 0;
  #revision = 0;

  constructor(renderer: THREE.WebGLRenderer) {
    this.#renderer = renderer;
  }

  capture(request: VoxelSpriteCaptureRequest): VoxelSpriteCaptureReceipt {
    if (this.#disposed) return this.#rejected('capture_disposed', 'voxel sprite capture is disposed');

    let validated: Required<Omit<VoxelSpriteCaptureRequest, 'bounds'>> & {
      readonly bounds: THREE.Box3;
    };
    try {
      validated = validateCaptureRequest(request);
    } catch (cause) {
      return this.#rejected('invalid_capture_request', messageFrom(cause));
    }

    const targets = createCaptureTargets(validated.width, validated.height);
    const state = rendererState(this.#renderer);
    const originalOverride = validated.scene.overrideMaterial;
    const originalBackground = validated.scene.background;
    const originalFog = validated.scene.fog;
    const started = nowMilliseconds();
    let nextFrame: VoxelSpriteFrame | null = null;

    try {
      this.#prepareRenderer(validated.width, validated.height);
      validated.scene.background = null;
      validated.scene.fog = null;

      validated.scene.overrideMaterial = originalOverride;
      this.#renderScene(targets.color, validated.scene, validated.camera);

      validated.scene.overrideMaterial = this.#normalMaterial;
      this.#renderScene(targets.normal, validated.scene, validated.camera);

      this.#resolveDepth(targets, validated.camera);
      this.#resolveCoverage(targets, validated.coverageAlphaCutoff);

      const descriptor = capturedDescriptor(validated, targets);
      nextFrame = VoxelSpriteFrame.owned(descriptor, () => disposePersistentTargets(targets));
    } catch (cause) {
      disposeCaptureTargets(targets);
      this.#lastCpuSubmissionMilliseconds = nowMilliseconds() - started;
      return this.#rejected('capture_failed', messageFrom(cause));
    } finally {
      validated.scene.overrideMaterial = originalOverride;
      validated.scene.background = originalBackground;
      validated.scene.fog = originalFog;
      restoreRendererState(this.#renderer, state);
    }

    targets.hardwareDepth.dispose();
    targets.color.depthTexture = null;
    const previous = this.#currentFrame;
    this.#currentFrame = nextFrame;
    this.#captureCount += 1;
    this.#revision += 1;
    this.#lastCpuSubmissionMilliseconds = nowMilliseconds() - started;
    previous?.dispose();
    return Object.freeze({
      applied: true,
      revision: this.#revision,
      frame: nextFrame,
      diagnostics: Object.freeze([]),
      readout: this.readout(),
    });
  }

  currentFrame(): VoxelSpriteFrame | null {
    return this.#currentFrame;
  }

  readout(): VoxelSpriteCaptureReadout {
    return Object.freeze({
      schemaVersion: 1,
      revision: this.#revision,
      captureCount: this.#captureCount,
      rejectedCaptureCount: this.#rejectedCaptureCount,
      cpuSubmissionMilliseconds: this.#lastCpuSubmissionMilliseconds,
      currentFrame: this.#currentFrame?.readout() ?? null,
      disposed: this.#disposed,
      limitations: LIMITATIONS,
    });
  }

  dispose(): void {
    if (this.#disposed) return;
    this.#currentFrame?.dispose();
    this.#currentFrame = null;
    this.#normalMaterial.dispose();
    this.#depthResolveMaterial.dispose();
    this.#coverageResolveMaterial.dispose();
    this.#disposed = true;
    this.#revision += 1;
  }

  #prepareRenderer(width: number, height: number): void {
    this.#renderer.xr.enabled = false;
    this.#renderer.autoClear = true;
    this.#renderer.setScissorTest(false);
    this.#renderer.setViewport(0, 0, width, height);
    this.#renderer.setScissor(0, 0, width, height);
    this.#renderer.setClearColor(0x000000, 0);
  }

  #renderScene(
    target: THREE.WebGLRenderTarget,
    scene: THREE.Scene,
    camera: VoxelSpriteCaptureCamera,
  ): void {
    this.#renderer.setRenderTarget(target);
    this.#renderer.clear(true, true, true);
    this.#renderer.render(scene, camera);
  }

  #resolveDepth(targets: CaptureTargets, camera: VoxelSpriteCaptureCamera): void {
    this.#depthResolveMaterial.uniforms['sourceDepth']!.value = targets.hardwareDepth;
    this.#depthResolveMaterial.uniforms['cameraNear']!.value = camera.near;
    this.#depthResolveMaterial.uniforms['cameraFar']!.value = camera.far;
    this.#depthResolveMaterial.uniforms['isPerspective']!.value = camera instanceof THREE.PerspectiveCamera;
    this.#renderer.setRenderTarget(targets.depth);
    this.#renderer.clear(true, false, false);
    this.#renderer.render(this.#depthResolveScene, FULLSCREEN_CAMERA);
  }

  #resolveCoverage(targets: CaptureTargets, alphaCutoff: number): void {
    this.#coverageResolveMaterial.uniforms['sourceColor']!.value = targets.color.texture;
    this.#coverageResolveMaterial.uniforms['alphaCutoff']!.value = alphaCutoff;
    this.#renderer.setRenderTarget(targets.coverage);
    this.#renderer.clear(true, false, false);
    this.#renderer.render(this.#coverageResolveScene, FULLSCREEN_CAMERA);
  }

  #rejected(
    code: VoxelSpriteCaptureDiagnosticCode,
    message: string,
  ): VoxelSpriteCaptureReceipt {
    this.#rejectedCaptureCount += 1;
    const diagnostic = Object.freeze({ code, message });
    return Object.freeze({
      applied: false,
      revision: this.#revision,
      frame: this.#currentFrame,
      diagnostics: Object.freeze([diagnostic]),
      readout: this.readout(),
    });
  }
}

function validatedDescriptor(input: VoxelSpriteFrameDescriptor): VoxelSpriteFrameDescriptor {
  validateResolution(input.width, 'width');
  validateResolution(input.height, 'height');
  if (!Number.isFinite(input.depth.near)
    || !Number.isFinite(input.depth.far)
    || input.depth.near < 0
    || input.depth.far <= input.depth.near) {
    throw new RangeError('voxel sprite frame depth range must be finite and increasing');
  }
  for (const [name, texture] of Object.entries(input.textures)) {
    if (!(texture instanceof THREE.Texture)) throw new TypeError(`${name} must be a Three texture`);
  }
  validateBasis(input.capture.basis);
  validateBounds(input.capture.bounds);
  return Object.freeze({
    ...input,
    textures: Object.freeze({ ...input.textures }),
    depth: Object.freeze({ ...input.depth }),
    capture: Object.freeze({
      ...input.capture,
      basis: freezeBasis(input.capture.basis),
      bounds: freezeBounds(input.capture.bounds),
    }),
  });
}

function validateCaptureRequest(
  request: VoxelSpriteCaptureRequest,
): Required<Omit<VoxelSpriteCaptureRequest, 'bounds'>> & { readonly bounds: THREE.Box3 } {
  if (!(request.scene instanceof THREE.Scene)) throw new TypeError('capture scene must be a Three scene');
  if (!(request.camera instanceof THREE.PerspectiveCamera)
    && !(request.camera instanceof THREE.OrthographicCamera)) {
    throw new TypeError('capture camera must be perspective or orthographic');
  }
  validateResolution(request.width, 'width');
  validateResolution(request.height, 'height');
  if (!Number.isFinite(request.camera.near)
    || !Number.isFinite(request.camera.far)
    || request.camera.near < 0
    || request.camera.far <= request.camera.near) {
    throw new RangeError('capture camera near/far range must be finite and increasing');
  }
  const coverageAlphaCutoff = request.coverageAlphaCutoff ?? 0.001;
  if (!Number.isFinite(coverageAlphaCutoff)
    || coverageAlphaCutoff < 0
    || coverageAlphaCutoff > 1) {
    throw new RangeError('coverageAlphaCutoff must be between zero and one');
  }
  request.camera.updateWorldMatrix(true, false);
  const bounds = request.bounds?.clone() ?? new THREE.Box3().setFromObject(request.scene, true);
  if (bounds.isEmpty() || !finiteVector(bounds.min) || !finiteVector(bounds.max)) {
    throw new RangeError('capture bounds must be finite and nonempty');
  }
  return {
    scene: request.scene,
    camera: request.camera,
    width: request.width,
    height: request.height,
    bounds,
    coverageAlphaCutoff,
  };
}

function validateResolution(value: number, name: string): void {
  if (!Number.isInteger(value)
    || value < VOXEL_SPRITE_CAPTURE_MIN_RESOLUTION
    || value > VOXEL_SPRITE_CAPTURE_MAX_RESOLUTION) {
    throw new RangeError(
      `${name} must be an integer from ${String(VOXEL_SPRITE_CAPTURE_MIN_RESOLUTION)}`
      + ` to ${String(VOXEL_SPRITE_CAPTURE_MAX_RESOLUTION)}`,
    );
  }
}

function validateBasis(basis: VoxelSpriteCaptureBasis): void {
  for (const value of [basis.position, basis.right, basis.up, basis.forward]) validateTuple(value);
}

function validateBounds(bounds: VoxelSpriteFrameBounds): void {
  validateTuple(bounds.minimum);
  validateTuple(bounds.maximum);
  if (bounds.maximum.some((value, index) => value < bounds.minimum[index]!)) {
    throw new RangeError('voxel sprite frame bounds must be increasing');
  }
}

function validateTuple(tuple: readonly number[]): void {
  if (tuple.length !== 3 || tuple.some((value) => !Number.isFinite(value))) {
    throw new TypeError('voxel sprite frame vectors must contain three finite values');
  }
}

function freezeBasis(basis: VoxelSpriteCaptureBasis): VoxelSpriteCaptureBasis {
  return Object.freeze({
    position: Object.freeze([...basis.position]) as unknown as readonly [number, number, number],
    right: Object.freeze([...basis.right]) as unknown as readonly [number, number, number],
    up: Object.freeze([...basis.up]) as unknown as readonly [number, number, number],
    forward: Object.freeze([...basis.forward]) as unknown as readonly [number, number, number],
  });
}

function freezeBounds(bounds: VoxelSpriteFrameBounds): VoxelSpriteFrameBounds {
  return Object.freeze({
    minimum: Object.freeze([...bounds.minimum]) as unknown as readonly [number, number, number],
    maximum: Object.freeze([...bounds.maximum]) as unknown as readonly [number, number, number],
  });
}

function createCaptureTargets(width: number, height: number): CaptureTargets {
  const color = renderTarget('voxel-sprite-color', width, height, true);
  const hardwareDepth = new THREE.DepthTexture(width, height, THREE.UnsignedIntType);
  hardwareDepth.name = 'voxel-sprite-hardware-depth';
  hardwareDepth.format = THREE.DepthFormat;
  hardwareDepth.minFilter = THREE.NearestFilter;
  hardwareDepth.magFilter = THREE.NearestFilter;
  color.depthTexture = hardwareDepth;
  color.texture.colorSpace = THREE.SRGBColorSpace;
  return {
    color,
    depth: renderTarget('voxel-sprite-linear-depth', width, height, false),
    normal: renderTarget('voxel-sprite-view-normal', width, height, true),
    coverage: renderTarget('voxel-sprite-coverage', width, height, false),
    hardwareDepth,
  };
}

function renderTarget(
  name: string,
  width: number,
  height: number,
  depthBuffer: boolean,
): THREE.WebGLRenderTarget {
  const target = new THREE.WebGLRenderTarget(width, height, {
    type: THREE.UnsignedByteType,
    format: THREE.RGBAFormat,
    minFilter: THREE.NearestFilter,
    magFilter: THREE.NearestFilter,
    depthBuffer,
    stencilBuffer: false,
    generateMipmaps: false,
  });
  target.texture.name = name;
  target.texture.colorSpace = THREE.NoColorSpace;
  target.texture.wrapS = THREE.ClampToEdgeWrapping;
  target.texture.wrapT = THREE.ClampToEdgeWrapping;
  target.texture.generateMipmaps = false;
  return target;
}

function depthResolveMaterial(): THREE.ShaderMaterial {
  return new THREE.ShaderMaterial({
    name: 'voxel-sprite-linear-depth-resolve',
    uniforms: {
      sourceDepth: { value: null },
      cameraNear: { value: 0.1 },
      cameraFar: { value: 100 },
      isPerspective: { value: true },
    },
    vertexShader: `
      varying vec2 voxelSpriteUv;
      void main() {
        voxelSpriteUv = uv;
        gl_Position = vec4(position.xy, 0.0, 1.0);
      }
    `,
    fragmentShader: `
      #include <packing>
      uniform sampler2D sourceDepth;
      uniform float cameraNear;
      uniform float cameraFar;
      uniform bool isPerspective;
      varying vec2 voxelSpriteUv;
      void main() {
        float depth = texture2D(sourceDepth, voxelSpriteUv).x;
        float viewZ = isPerspective
          ? perspectiveDepthToViewZ(depth, cameraNear, cameraFar)
          : orthographicDepthToViewZ(depth, cameraNear, cameraFar);
        float linearDepth = clamp((-viewZ - cameraNear) / (cameraFar - cameraNear), 0.0, 1.0);
        gl_FragColor = vec4(linearDepth, linearDepth, linearDepth, depth < 1.0 ? 1.0 : 0.0);
      }
    `,
    depthTest: false,
    depthWrite: false,
    blending: THREE.NoBlending,
  });
}

function coverageResolveMaterial(): THREE.ShaderMaterial {
  return new THREE.ShaderMaterial({
    name: 'voxel-sprite-coverage-resolve',
    uniforms: {
      sourceColor: { value: null },
      alphaCutoff: { value: 0.001 },
    },
    vertexShader: `
      varying vec2 voxelSpriteUv;
      void main() {
        voxelSpriteUv = uv;
        gl_Position = vec4(position.xy, 0.0, 1.0);
      }
    `,
    fragmentShader: `
      uniform sampler2D sourceColor;
      uniform float alphaCutoff;
      varying vec2 voxelSpriteUv;
      void main() {
        float alpha = texture2D(sourceColor, voxelSpriteUv).a;
        float covered = step(alphaCutoff, alpha);
        gl_FragColor = vec4(covered, covered, covered, covered);
      }
    `,
    depthTest: false,
    depthWrite: false,
    blending: THREE.NoBlending,
  });
}

function fullscreenScene(material: THREE.Material): THREE.Scene {
  const scene = new THREE.Scene();
  const mesh = new THREE.Mesh(FULLSCREEN_GEOMETRY, material);
  mesh.frustumCulled = false;
  scene.add(mesh);
  return scene;
}

function capturedDescriptor(
  request: Required<Omit<VoxelSpriteCaptureRequest, 'bounds'>> & { readonly bounds: THREE.Box3 },
  targets: CaptureTargets,
): VoxelSpriteFrameDescriptor {
  const quaternion = request.camera.getWorldQuaternion(new THREE.Quaternion());
  const position = request.camera.getWorldPosition(new THREE.Vector3());
  const right = new THREE.Vector3(1, 0, 0).applyQuaternion(quaternion).normalize();
  const up = new THREE.Vector3(0, 1, 0).applyQuaternion(quaternion).normalize();
  const forward = new THREE.Vector3(0, 0, -1).applyQuaternion(quaternion).normalize();
  return {
    schemaVersion: 1,
    width: request.width,
    height: request.height,
    textures: {
      color: targets.color.texture,
      depth: targets.depth.texture,
      normal: targets.normal.texture,
      coverage: targets.coverage.texture,
    },
    provenance: 'runtime-capture',
    depth: {
      encoding: 'linear-view-depth-unorm8',
      near: request.camera.near,
      far: request.camera.far,
    },
    normalSpace: 'view',
    capture: {
      projection: request.camera instanceof THREE.PerspectiveCamera ? 'perspective' : 'orthographic',
      basis: {
        position: tuple(position),
        right: tuple(right),
        up: tuple(up),
        forward: tuple(forward),
      },
      bounds: {
        minimum: tuple(request.bounds.min),
        maximum: tuple(request.bounds.max),
      },
    },
  };
}

function rendererState(renderer: THREE.WebGLRenderer): RendererState {
  return {
    autoClear: renderer.autoClear,
    clearAlpha: renderer.getClearAlpha(),
    clearColor: renderer.getClearColor(new THREE.Color()).clone(),
    renderTarget: renderer.getRenderTarget(),
    scissor: renderer.getScissor(new THREE.Vector4()).clone(),
    scissorTest: renderer.getScissorTest(),
    viewport: renderer.getViewport(new THREE.Vector4()).clone(),
    xrEnabled: renderer.xr.enabled,
  };
}

function restoreRendererState(renderer: THREE.WebGLRenderer, state: RendererState): void {
  renderer.setRenderTarget(state.renderTarget);
  renderer.setViewport(state.viewport);
  renderer.setScissor(state.scissor);
  renderer.setScissorTest(state.scissorTest);
  renderer.setClearColor(state.clearColor, state.clearAlpha);
  renderer.autoClear = state.autoClear;
  renderer.xr.enabled = state.xrEnabled;
}

function disposePersistentTargets(targets: CaptureTargets): void {
  targets.color.dispose();
  targets.depth.dispose();
  targets.normal.dispose();
  targets.coverage.dispose();
}

function disposeCaptureTargets(targets: CaptureTargets): void {
  targets.hardwareDepth.dispose();
  disposePersistentTargets(targets);
}

function finiteVector(value: THREE.Vector3): boolean {
  return Number.isFinite(value.x) && Number.isFinite(value.y) && Number.isFinite(value.z);
}

function tuple(value: THREE.Vector3): readonly [number, number, number] {
  return Object.freeze([value.x, value.y, value.z]);
}

function messageFrom(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

function nowMilliseconds(): number {
  return typeof performance === 'undefined' ? Date.now() : performance.now();
}
