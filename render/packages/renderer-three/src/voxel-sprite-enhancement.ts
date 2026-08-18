import * as THREE from 'three';

import { VoxelSpriteFrame, type VoxelSpriteCaptureBasis } from './voxel-sprite-capture.js';

export const VOXEL_SPRITE_ENHANCEMENT_MODES = Object.freeze([
  'sprite',
  'relit',
  'depth-parallax',
  'sprite-splat',
  'full-splat',
] as const);

export type VoxelSpriteEnhancementMode = typeof VOXEL_SPRITE_ENHANCEMENT_MODES[number];
export type VoxelSpriteDepthScale = 'normalized' | 'world';
export type VoxelSpriteLightingMode = 'captured' | 'normal';
export type VoxelSpriteSplatBlendMode = 'depth-write' | 'alpha-blend' | 'additive';
export type VoxelSpriteOrientationPolicy =
  | 'camera-facing'
  | 'capture-held'
  | 'capture-camera-blend';
export type VoxelSpriteOrientationElevationPolicy = 'capture' | 'world-upright';
export type VoxelSpriteRepresentationTransition = 'opaque' | 'dither' | 'alpha';

export interface VoxelSpriteEnhancementConfig {
  readonly mode: VoxelSpriteEnhancementMode;
  readonly width: number;
  readonly height: number;
  readonly sampleColumns: number;
  readonly sampleRows: number;
  /** Independent construction-time sampling grid used by instanced splats. */
  readonly splatColumns: number;
  readonly splatRows: number;
  /** Expands subject-relative front-to-back relief from the rear card plane. */
  readonly depthAmplitude: number;
  /** Expands captured surface variation around the subject midpoint before amplitude is applied. */
  readonly depthContrast: number;
  /** Maximum subject-relative depth contribution before scale and amplitude are applied. */
  readonly depthClamp: number;
  readonly depthScale: VoxelSpriteDepthScale;
  /** Zero preserves continuous depth; positive values quantize the visible displacement only. */
  readonly depthQuantizationSteps: number;
  /** Maximum UV travel for bounded parallax-occlusion lookup on the connected card. */
  readonly parallaxOcclusionScale: number;
  /** Zero disables POM; otherwise the fixed lookup budget is 4 through 32 steps. */
  readonly parallaxOcclusionSteps: number;
  readonly depthDilationTexels: number;
  readonly depthConfidenceThreshold: number;
  readonly splatFootprint: number;
  readonly splatOverlap: number;
  readonly splatOpacity: number;
  readonly splatBlendMode: VoxelSpriteSplatBlendMode;
  readonly normalInfluence: number;
  readonly normalOrientationBlend: number;
  /** Controls whether the admitted capture view is held as the viewer moves locally. */
  readonly orientationPolicy: VoxelSpriteOrientationPolicy;
  /** Camera-facing contribution used only by `capture-camera-blend`. */
  readonly orientationBlend: number;
  /** Preserve the captured elevation or retain only its azimuth. */
  readonly orientationElevationPolicy: VoxelSpriteOrientationElevationPolicy;
  /** World-yaw correction for aligning neighboring captures onto one held card. */
  readonly orientationAzimuthOffsetDegrees: number;
  /** Per-representation transition used by angle-conditioned multi-view consumers. */
  readonly representationTransition: VoxelSpriteRepresentationTransition;
  readonly representationWeight: number;
  /** Start of this representation's complementary dither interval in [0, 1]. */
  readonly representationDitherOffset: number;
  readonly baseSpriteContribution: number;
  readonly viewAngleFalloff: number;
  /** `captured` preserves capture color; `normal` modulates it with the normal pass. */
  readonly lightingMode: VoxelSpriteLightingMode;
  readonly ambientLight: number;
  readonly diffuseLight: number;
  readonly outputGain: number;
  readonly ambientColor: readonly [number, number, number];
  readonly lightColor: readonly [number, number, number];
  readonly lightDirection: readonly [number, number, number];
}

export interface VoxelSpriteEnhancementSource {
  readonly frame: VoxelSpriteFrame;
  readonly captureCpuSubmissionMilliseconds?: number | null;
}

export interface VoxelSpriteEnhancementReadout {
  readonly schemaVersion: 1;
  readonly revision: number;
  readonly mode: VoxelSpriteEnhancementMode;
  readonly config: VoxelSpriteEnhancementConfig;
  readonly captureCpuSubmissionMilliseconds: number | null;
  readonly steadyStateCpuSubmissionMilliseconds: number | null;
  /** Admitted world-space capture basis; contains no backend objects. */
  readonly captureBasis: VoxelSpriteCaptureBasis;
  /** Unsigned angle between the admitted capture view and current object-to-camera direction. */
  readonly angularOffsetDegrees: number | null;
  readonly expectedDrawCalls: number;
  readonly geometrySampleCount: number;
  readonly frameTextureBytes: number;
  readonly geometryResourceCount: number;
  readonly materialResourceCount: number;
  readonly borrowedTextureCount: number;
  readonly baseSpriteVisible: boolean;
  readonly splatVisible: boolean;
  readonly composition:
    | 'opaque-depth-writing-base'
    | 'base-blend-then-depth-writing-splats'
    | 'base-blend-then-alpha-blended-splats'
    | 'base-blend-then-additive-splats'
    | 'depth-writing-splats'
    | 'alpha-blended-splats'
    | 'additive-splats';
  readonly disposed: boolean;
  readonly limitations: readonly [
    'single-capture-view',
    'view-space-normals',
    'rgba8-depth',
    'approximate-splat-orientation',
    'unsorted-transparent-splats',
    'gpu-time-not-measured',
  ];
}

const DEFAULT_CONFIG: VoxelSpriteEnhancementConfig = Object.freeze({
  mode: 'sprite',
  width: 2,
  height: 2,
  sampleColumns: 32,
  sampleRows: 32,
  splatColumns: 32,
  splatRows: 32,
  depthAmplitude: 0.35,
  depthContrast: 4,
  depthClamp: 1,
  depthScale: 'normalized',
  depthQuantizationSteps: 8,
  parallaxOcclusionScale: 0.06,
  parallaxOcclusionSteps: 16,
  depthDilationTexels: 0,
  depthConfidenceThreshold: 0.5,
  splatFootprint: 1,
  splatOverlap: 0.15,
  splatOpacity: 1,
  splatBlendMode: 'depth-write',
  normalInfluence: 0.65,
  normalOrientationBlend: 0.35,
  orientationPolicy: 'camera-facing',
  orientationBlend: 0.5,
  orientationElevationPolicy: 'capture',
  orientationAzimuthOffsetDegrees: 0,
  representationTransition: 'opaque',
  representationWeight: 1,
  representationDitherOffset: 0,
  baseSpriteContribution: 0.7,
  viewAngleFalloff: 0,
  lightingMode: 'captured',
  ambientLight: 0.35,
  diffuseLight: 0.9,
  outputGain: 1,
  ambientColor: Object.freeze([1, 1, 1]) as readonly [number, number, number],
  lightColor: Object.freeze([1, 1, 1]) as readonly [number, number, number],
  lightDirection: Object.freeze([0.4, 0.7, 1]) as readonly [number, number, number],
});

const LIMITATIONS = Object.freeze([
  'single-capture-view',
  'view-space-normals',
  'rgba8-depth',
  'approximate-splat-orientation',
  'unsorted-transparent-splats',
  'gpu-time-not-measured',
] as const);

const CONFIG_KEYS = new Set(Object.keys(DEFAULT_CONFIG));

interface EnhancementUniforms extends Record<string, THREE.IUniform> {
  readonly colorTexture: THREE.IUniform<THREE.Texture>;
  readonly depthTexture: THREE.IUniform<THREE.Texture>;
  readonly normalTexture: THREE.IUniform<THREE.Texture>;
  readonly coverageTexture: THREE.IUniform<THREE.Texture>;
  readonly textureTexelSize: THREE.IUniform<THREE.Vector2>;
  readonly objectSize: THREE.IUniform<THREE.Vector2>;
  readonly sampleGrid: THREE.IUniform<THREE.Vector2>;
  readonly depthAmplitude: THREE.IUniform<number>;
  readonly depthContrast: THREE.IUniform<number>;
  readonly depthClamp: THREE.IUniform<number>;
  readonly captureNear: THREE.IUniform<number>;
  readonly captureDepthRange: THREE.IUniform<number>;
  readonly reliefRearDepth: THREE.IUniform<number>;
  readonly reliefDepthRange: THREE.IUniform<number>;
  readonly useWorldDepth: THREE.IUniform<number>;
  readonly depthQuantizationSteps: THREE.IUniform<number>;
  readonly parallaxOcclusionScale: THREE.IUniform<number>;
  readonly parallaxOcclusionSteps: THREE.IUniform<number>;
  readonly parallaxOcclusionEnabled: THREE.IUniform<number>;
  readonly viewerPositionLocal: THREE.IUniform<THREE.Vector3>;
  readonly depthDilationTexels: THREE.IUniform<number>;
  readonly depthConfidenceThreshold: THREE.IUniform<number>;
  readonly splatFootprint: THREE.IUniform<number>;
  readonly splatOverlap: THREE.IUniform<number>;
  readonly splatOpacity: THREE.IUniform<number>;
  readonly normalInfluence: THREE.IUniform<number>;
  readonly normalOrientationBlend: THREE.IUniform<number>;
  readonly baseSpriteContribution: THREE.IUniform<number>;
  readonly baseDepthDisplacement: THREE.IUniform<number>;
  readonly viewAngleFalloff: THREE.IUniform<number>;
  readonly ambientLight: THREE.IUniform<number>;
  readonly diffuseLight: THREE.IUniform<number>;
  readonly outputGain: THREE.IUniform<number>;
  readonly ambientColor: THREE.IUniform<THREE.Color>;
  readonly lightColor: THREE.IUniform<THREE.Color>;
  readonly lightDirection: THREE.IUniform<THREE.Vector3>;
  readonly normalLighting: THREE.IUniform<number>;
  readonly representationTransitionMode: THREE.IUniform<number>;
  readonly representationWeight: THREE.IUniform<number>;
  readonly representationDitherOffset: THREE.IUniform<number>;
}

/**
 * Experimental Three-local consumer for one prepared or runtime-captured voxel-sprite frame.
 * The object borrows its frame textures; callers retain frame lifetime authority.
 */
export class VoxelSpriteEnhancement {
  readonly object = new THREE.Group();
  readonly #baseGeometry: THREE.PlaneGeometry;
  readonly #baseMaterial: THREE.ShaderMaterial;
  readonly #baseMesh: THREE.Mesh;
  readonly #splatGeometry: THREE.InstancedBufferGeometry;
  readonly #splatMaterial: THREE.ShaderMaterial;
  readonly #splatMesh: THREE.Mesh;
  readonly #uniforms: EnhancementUniforms;
  #captureCpuSubmissionMilliseconds: number | null;
  #angularOffsetDegrees: number | null = null;
  #config: VoxelSpriteEnhancementConfig;
  #disposed = false;
  #frame: VoxelSpriteFrame;
  #revision = 1;
  #steadyStateCpuSubmissionMilliseconds: number | null = null;

  constructor(
    source: VoxelSpriteEnhancementSource,
    config: Partial<VoxelSpriteEnhancementConfig> = {},
  ) {
    this.#frame = validatedFrame(source.frame);
    this.#captureCpuSubmissionMilliseconds = optionalMilliseconds(
      source.captureCpuSubmissionMilliseconds ?? null,
      'captureCpuSubmissionMilliseconds',
    );
    rejectUnknownConfig(config);
    this.#config = validatedConfig({
      ...DEFAULT_CONFIG,
      ...config,
      splatColumns: config.splatColumns ?? config.sampleColumns ?? DEFAULT_CONFIG.splatColumns,
      splatRows: config.splatRows ?? config.sampleRows ?? DEFAULT_CONFIG.splatRows,
      lightingMode: config.lightingMode
        ?? (config.mode !== undefined && config.mode !== 'sprite' ? 'normal' : 'captured'),
    });
    this.#uniforms = createUniforms(this.#frame, this.#config);
    this.#baseGeometry = new THREE.PlaneGeometry(
      1,
      1,
      this.#config.sampleColumns - 1,
      this.#config.sampleRows - 1,
    );
    this.#baseMaterial = baseMaterial(this.#uniforms);
    this.#baseMesh = new THREE.Mesh(this.#baseGeometry, this.#baseMaterial);
    this.#baseMesh.name = 'voxel-sprite-base';
    this.#baseMesh.frustumCulled = false;
    this.#baseMesh.renderOrder = 0;

    this.#splatGeometry = splatGeometry(this.#config.splatColumns, this.#config.splatRows);
    this.#splatMaterial = splatMaterial(this.#uniforms);
    this.#splatMesh = new THREE.Mesh(this.#splatGeometry, this.#splatMaterial);
    this.#splatMesh.name = 'voxel-sprite-splats';
    this.#splatMesh.frustumCulled = false;
    this.#splatMesh.renderOrder = 1;
    this.object.name = 'voxel-sprite-enhancement';
    this.object.add(this.#baseMesh, this.#splatMesh);
    this.#applyMode();
  }

  configure(patch: Partial<VoxelSpriteEnhancementConfig>): VoxelSpriteEnhancementReadout {
    this.#assertLive();
    rejectUnknownConfig(patch);
    rejectGridMutation(patch, this.#config);
    this.#config = validatedConfig({ ...this.#config, ...patch });
    applyUniformConfig(this.#uniforms, this.#config);
    this.#applyMode();
    this.#revision += 1;
    return this.readout();
  }

  replaceSource(source: VoxelSpriteEnhancementSource): VoxelSpriteEnhancementReadout {
    this.#assertLive();
    const frame = validatedFrame(source.frame);
    const captureMilliseconds = optionalMilliseconds(
      source.captureCpuSubmissionMilliseconds ?? null,
      'captureCpuSubmissionMilliseconds',
    );
    this.#frame = frame;
    this.#captureCpuSubmissionMilliseconds = captureMilliseconds;
    bindFrame(this.#uniforms, frame);
    this.#revision += 1;
    return this.readout();
  }

  /** Resolves the configured representation orientation without taking camera or scene authority. */
  prepare(camera: THREE.Camera): void {
    this.#assertLive();
    if (!(camera instanceof THREE.Camera)) throw new TypeError('camera must be a Three camera');
    camera.updateWorldMatrix(true, false);
    this.object.updateWorldMatrix(true, false);
    const cameraWorld = camera.getWorldQuaternion(new THREE.Quaternion());
    let heldWorld = captureHeldQuaternion(
      this.#frame.descriptor.capture.basis,
      this.#config.orientationElevationPolicy,
    );
    if (heldWorld !== null && this.#config.orientationAzimuthOffsetDegrees !== 0) {
      heldWorld = new THREE.Quaternion().setFromAxisAngle(
        new THREE.Vector3(0, 1, 0),
        THREE.MathUtils.degToRad(this.#config.orientationAzimuthOffsetDegrees),
      ).multiply(heldWorld).normalize();
    }
    const captureViewerDirection = tupleVector(this.#frame.descriptor.capture.basis.forward)
      .multiplyScalar(-1);
    const currentViewerDirection = camera.getWorldPosition(new THREE.Vector3())
      .sub(this.object.getWorldPosition(new THREE.Vector3()));
    this.#angularOffsetDegrees = finiteAngleDegrees(captureViewerDirection, currentViewerDirection);

    let target = cameraWorld;
    if (heldWorld !== null && this.#config.orientationPolicy === 'capture-held') {
      target = heldWorld;
    } else if (heldWorld !== null && this.#config.orientationPolicy === 'capture-camera-blend') {
      target = heldWorld.clone().slerp(cameraWorld, this.#config.orientationBlend).normalize();
    }
    if (this.object.parent !== null) {
      const parent = this.object.parent.getWorldQuaternion(new THREE.Quaternion()).invert();
      target = target.clone().premultiply(parent);
    }
    this.object.quaternion.copy(target);
    this.object.updateWorldMatrix(true, false);
    this.#uniforms.viewerPositionLocal.value.copy(
      this.object.worldToLocal(camera.getWorldPosition(new THREE.Vector3())),
    );
  }

  recordSteadyStateFrame(milliseconds: number): VoxelSpriteEnhancementReadout {
    this.#assertLive();
    this.#steadyStateCpuSubmissionMilliseconds = requiredMilliseconds(
      milliseconds,
      'steady-state CPU submission',
    );
    return this.readout();
  }

  readout(): VoxelSpriteEnhancementReadout {
    const mode = this.#config.mode;
    const baseVisible = !this.#disposed && mode !== 'full-splat';
    const splatVisible = !this.#disposed && (mode === 'sprite-splat' || mode === 'full-splat');
    return Object.freeze({
      schemaVersion: 1,
      revision: this.#revision,
      mode,
      config: this.#config,
      captureCpuSubmissionMilliseconds: this.#captureCpuSubmissionMilliseconds,
      steadyStateCpuSubmissionMilliseconds: this.#steadyStateCpuSubmissionMilliseconds,
      captureBasis: this.#frame.descriptor.capture.basis,
      angularOffsetDegrees: this.#angularOffsetDegrees,
      expectedDrawCalls: Number(baseVisible) + Number(splatVisible),
      geometrySampleCount:
        (baseVisible ? this.#config.sampleColumns * this.#config.sampleRows : 0)
        + (splatVisible ? this.#config.splatColumns * this.#config.splatRows : 0),
      frameTextureBytes: this.#frame.readout().estimatedTextureBytes,
      geometryResourceCount: this.#disposed ? 0 : 2,
      materialResourceCount: this.#disposed ? 0 : 2,
      borrowedTextureCount: this.#disposed ? 0 : 4,
      baseSpriteVisible: baseVisible,
      splatVisible,
      composition: composition(mode, this.#config.splatBlendMode),
      disposed: this.#disposed,
      limitations: LIMITATIONS,
    });
  }

  dispose(): void {
    if (this.#disposed) return;
    this.object.remove(this.#baseMesh, this.#splatMesh);
    this.#baseGeometry.dispose();
    this.#baseMaterial.dispose();
    this.#splatGeometry.dispose();
    this.#splatMaterial.dispose();
    this.#disposed = true;
    this.#revision += 1;
  }

  #applyMode(): void {
    const mode = this.#config.mode;
    this.#baseMesh.visible = mode !== 'full-splat';
    this.#splatMesh.visible = mode === 'sprite-splat' || mode === 'full-splat';
    const pomEnabled = mode === 'depth-parallax' && this.#config.parallaxOcclusionSteps > 0;
    this.#uniforms.baseDepthDisplacement.value = mode === 'depth-parallax' && !pomEnabled ? 1 : 0;
    this.#uniforms.parallaxOcclusionEnabled.value = pomEnabled ? 1 : 0;
    this.#uniforms.baseSpriteContribution.value = mode === 'sprite-splat'
      ? this.#config.baseSpriteContribution
      : 1;
    const alphaTransition = this.#config.representationTransition === 'alpha';
    this.#baseMaterial.depthWrite = mode !== 'sprite-splat' && !alphaTransition;
    this.#baseMaterial.transparent = mode === 'sprite-splat' || alphaTransition;
    this.#splatMaterial.depthWrite = this.#config.splatBlendMode === 'depth-write'
      && !alphaTransition;
    this.#splatMaterial.blending = this.#config.splatBlendMode === 'additive'
      ? THREE.AdditiveBlending
      : THREE.NormalBlending;
    this.#splatMaterial.needsUpdate = true;
    this.#baseMaterial.needsUpdate = true;
  }

  #assertLive(): void {
    if (this.#disposed) throw new Error('voxel sprite enhancement is disposed');
  }
}

function validatedFrame(frame: VoxelSpriteFrame): VoxelSpriteFrame {
  if (!(frame instanceof VoxelSpriteFrame)) throw new TypeError('frame must be a VoxelSpriteFrame');
  if (frame.disposed) throw new Error('voxel sprite frame is disposed');
  return frame;
}

function rejectUnknownConfig(config: Partial<VoxelSpriteEnhancementConfig>): void {
  const unknown = Object.keys(config).filter((key) => !CONFIG_KEYS.has(key));
  if (unknown.length > 0) throw new TypeError(`unknown enhancement config fields: ${unknown.join(', ')}`);
}

function validatedConfig(input: VoxelSpriteEnhancementConfig): VoxelSpriteEnhancementConfig {
  if (!VOXEL_SPRITE_ENHANCEMENT_MODES.includes(input.mode)) throw new RangeError('unknown enhancement mode');
  if (input.depthScale !== 'normalized' && input.depthScale !== 'world') {
    throw new RangeError('depthScale must be normalized or world');
  }
  bounded(input.width, 0.05, 64, 'width');
  bounded(input.height, 0.05, 64, 'height');
  integer(input.sampleColumns, 8, 128, 'sampleColumns');
  integer(input.sampleRows, 8, 128, 'sampleRows');
  integer(input.splatColumns, 8, 512, 'splatColumns');
  integer(input.splatRows, 8, 512, 'splatRows');
  bounded(input.depthAmplitude, 0, 4, 'depthAmplitude');
  bounded(input.depthContrast, 1, 16, 'depthContrast');
  bounded(input.depthClamp, 0, 1, 'depthClamp');
  integer(input.depthQuantizationSteps, 0, 64, 'depthQuantizationSteps');
  bounded(input.parallaxOcclusionScale, 0, 0.25, 'parallaxOcclusionScale');
  if (input.parallaxOcclusionSteps !== 0) {
    integer(input.parallaxOcclusionSteps, 4, 32, 'parallaxOcclusionSteps');
  }
  bounded(input.depthDilationTexels, 0, 4, 'depthDilationTexels');
  bounded(input.depthConfidenceThreshold, 0, 0.99, 'depthConfidenceThreshold');
  bounded(input.splatFootprint, 0.25, 4, 'splatFootprint');
  bounded(input.splatOverlap, 0, 2, 'splatOverlap');
  bounded(input.splatOpacity, 0, 1, 'splatOpacity');
  if (!['depth-write', 'alpha-blend', 'additive'].includes(input.splatBlendMode)) {
    throw new RangeError('splatBlendMode must be depth-write, alpha-blend, or additive');
  }
  bounded(input.normalInfluence, 0, 1, 'normalInfluence');
  bounded(input.normalOrientationBlend, 0, 1, 'normalOrientationBlend');
  if (!['camera-facing', 'capture-held', 'capture-camera-blend'].includes(input.orientationPolicy)) {
    throw new RangeError('orientationPolicy must be camera-facing, capture-held, or capture-camera-blend');
  }
  bounded(input.orientationBlend, 0, 1, 'orientationBlend');
  if (!['capture', 'world-upright'].includes(input.orientationElevationPolicy)) {
    throw new RangeError('orientationElevationPolicy must be capture or world-upright');
  }
  bounded(input.orientationAzimuthOffsetDegrees, -45, 45, 'orientationAzimuthOffsetDegrees');
  if (!['opaque', 'dither', 'alpha'].includes(input.representationTransition)) {
    throw new RangeError('representationTransition must be opaque, dither, or alpha');
  }
  bounded(input.representationWeight, 0, 1, 'representationWeight');
  bounded(input.representationDitherOffset, 0, 1, 'representationDitherOffset');
  bounded(input.baseSpriteContribution, 0, 1, 'baseSpriteContribution');
  bounded(input.viewAngleFalloff, 0, 16, 'viewAngleFalloff');
  if (input.lightingMode !== 'captured' && input.lightingMode !== 'normal') {
    throw new RangeError('lightingMode must be captured or normal');
  }
  bounded(input.ambientLight, 0, 4, 'ambientLight');
  bounded(input.diffuseLight, 0, 4, 'diffuseLight');
  bounded(input.outputGain, 0.1, 4, 'outputGain');
  const ambientColor = colorTuple(input.ambientColor, 'ambientColor');
  const lightColor = colorTuple(input.lightColor, 'lightColor');
  const lightDirection = normalizedTuple(input.lightDirection, 'lightDirection');
  return Object.freeze({ ...input, ambientColor, lightColor, lightDirection });
}

function rejectGridMutation(
  patch: Partial<VoxelSpriteEnhancementConfig>,
  current: VoxelSpriteEnhancementConfig,
): void {
  if ((patch.sampleColumns !== undefined && patch.sampleColumns !== current.sampleColumns)
    || (patch.sampleRows !== undefined && patch.sampleRows !== current.sampleRows)
    || (patch.splatColumns !== undefined && patch.splatColumns !== current.splatColumns)
    || (patch.splatRows !== undefined && patch.splatRows !== current.splatRows)) {
    throw new Error('base and splat sample grids are construction-time geometry and cannot be reconfigured');
  }
}

function bounded(value: number, minimum: number, maximum: number, name: string): void {
  if (!Number.isFinite(value) || value < minimum || value > maximum) {
    throw new RangeError(`${name} must be finite from ${String(minimum)} to ${String(maximum)}`);
  }
}

function integer(value: number, minimum: number, maximum: number, name: string): void {
  if (!Number.isInteger(value) || value < minimum || value > maximum) {
    throw new RangeError(`${name} must be an integer from ${String(minimum)} to ${String(maximum)}`);
  }
}

function normalizedTuple(
  value: readonly [number, number, number],
  name: string,
): readonly [number, number, number] {
  if (value.length !== 3 || value.some((component) => !Number.isFinite(component))) {
    throw new TypeError(`${name} must contain three finite values`);
  }
  const vector = new THREE.Vector3(...value);
  if (vector.lengthSq() < 1e-8) throw new RangeError(`${name} must be nonzero`);
  vector.normalize();
  return Object.freeze([vector.x, vector.y, vector.z]);
}

function colorTuple(
  value: readonly [number, number, number],
  name: string,
): readonly [number, number, number] {
  if (value.length !== 3 || value.some((component) => !Number.isFinite(component)
    || component < 0 || component > 1)) {
    throw new RangeError(`${name} must contain three finite values from 0 to 1`);
  }
  return Object.freeze([...value]) as unknown as readonly [number, number, number];
}

function optionalMilliseconds(value: number | null, name: string): number | null {
  return value === null ? null : requiredMilliseconds(value, name);
}

function requiredMilliseconds(value: number, name: string): number {
  if (!Number.isFinite(value) || value < 0) throw new RangeError(`${name} must be finite and nonnegative`);
  return value;
}

function captureHeldQuaternion(
  basis: VoxelSpriteCaptureBasis,
  elevationPolicy: VoxelSpriteOrientationElevationPolicy,
): THREE.Quaternion | null {
  const backward = tupleVector(basis.forward).multiplyScalar(-1);
  if (elevationPolicy === 'world-upright') backward.y = 0;
  if (backward.lengthSq() < 1e-8) return null;
  backward.normalize();

  const admittedRight = tupleVector(basis.right);
  const admittedUp = tupleVector(basis.up);
  const right = elevationPolicy === 'world-upright'
    ? new THREE.Vector3(0, 1, 0).cross(backward)
    : admittedRight.clone().addScaledVector(backward, -admittedRight.dot(backward));
  if (right.lengthSq() < 1e-8) {
    right.copy(admittedUp).cross(backward);
  }
  if (right.lengthSq() < 1e-8) return null;
  right.normalize();
  const up = backward.clone().cross(right).normalize();
  if (elevationPolicy === 'capture' && up.dot(admittedUp) < 0) {
    right.multiplyScalar(-1);
    up.multiplyScalar(-1);
  }
  return new THREE.Quaternion().setFromRotationMatrix(
    new THREE.Matrix4().makeBasis(right, up, backward),
  ).normalize();
}

function tupleVector(value: readonly [number, number, number]): THREE.Vector3 {
  return new THREE.Vector3(value[0], value[1], value[2]);
}

function finiteAngleDegrees(left: THREE.Vector3, right: THREE.Vector3): number | null {
  if (left.lengthSq() < 1e-8 || right.lengthSq() < 1e-8) return null;
  const cosine = THREE.MathUtils.clamp(left.normalize().dot(right.normalize()), -1, 1);
  const angle = THREE.MathUtils.radToDeg(Math.acos(cosine));
  return Number.isFinite(angle) ? angle : null;
}

function createUniforms(
  frame: VoxelSpriteFrame,
  config: VoxelSpriteEnhancementConfig,
): EnhancementUniforms {
  const uniforms: EnhancementUniforms = {
    colorTexture: { value: frame.descriptor.textures.color },
    depthTexture: { value: frame.descriptor.textures.depth },
    normalTexture: { value: frame.descriptor.textures.normal },
    coverageTexture: { value: frame.descriptor.textures.coverage },
    textureTexelSize: { value: new THREE.Vector2() },
    objectSize: { value: new THREE.Vector2() },
    sampleGrid: { value: new THREE.Vector2() },
    depthAmplitude: { value: 0 },
    depthContrast: { value: 1 },
    depthClamp: { value: 1 },
    captureNear: { value: 0 },
    captureDepthRange: { value: 1 },
    reliefRearDepth: { value: 1 },
    reliefDepthRange: { value: 1 },
    useWorldDepth: { value: 0 },
    depthQuantizationSteps: { value: 0 },
    parallaxOcclusionScale: { value: 0 },
    parallaxOcclusionSteps: { value: 0 },
    parallaxOcclusionEnabled: { value: 0 },
    viewerPositionLocal: { value: new THREE.Vector3(0, 0, 1) },
    depthDilationTexels: { value: 0 },
    depthConfidenceThreshold: { value: 0 },
    splatFootprint: { value: 1 },
    splatOverlap: { value: 0 },
    splatOpacity: { value: 1 },
    normalInfluence: { value: 0 },
    normalOrientationBlend: { value: 0 },
    baseSpriteContribution: { value: 1 },
    baseDepthDisplacement: { value: 0 },
    viewAngleFalloff: { value: 0 },
    ambientLight: { value: 0 },
    diffuseLight: { value: 0 },
    outputGain: { value: 1 },
    ambientColor: { value: new THREE.Color() },
    lightColor: { value: new THREE.Color() },
    lightDirection: { value: new THREE.Vector3() },
    normalLighting: { value: 0 },
    representationTransitionMode: { value: 0 },
    representationWeight: { value: 1 },
    representationDitherOffset: { value: 0 },
  };
  bindFrame(uniforms, frame);
  applyUniformConfig(uniforms, config);
  return uniforms;
}

function bindFrame(uniforms: EnhancementUniforms, frame: VoxelSpriteFrame): void {
  uniforms.colorTexture.value = frame.descriptor.textures.color;
  uniforms.depthTexture.value = frame.descriptor.textures.depth;
  uniforms.normalTexture.value = frame.descriptor.textures.normal;
  uniforms.coverageTexture.value = frame.descriptor.textures.coverage;
  uniforms.textureTexelSize.value.set(1 / frame.descriptor.width, 1 / frame.descriptor.height);
  const relief = captureReliefDepth(frame);
  uniforms.captureNear.value = frame.descriptor.depth.near;
  uniforms.captureDepthRange.value = frame.descriptor.depth.far - frame.descriptor.depth.near;
  uniforms.reliefRearDepth.value = relief.rear;
  uniforms.reliefDepthRange.value = relief.range;
}

function applyUniformConfig(
  uniforms: EnhancementUniforms,
  config: VoxelSpriteEnhancementConfig,
): void {
  uniforms.objectSize.value.set(config.width, config.height);
  uniforms.sampleGrid.value.set(config.splatColumns, config.splatRows);
  uniforms.depthAmplitude.value = config.depthAmplitude;
  uniforms.depthContrast.value = config.depthContrast;
  uniforms.depthClamp.value = config.depthClamp;
  uniforms.useWorldDepth.value = config.depthScale === 'world' ? 1 : 0;
  uniforms.depthQuantizationSteps.value = config.depthQuantizationSteps;
  uniforms.parallaxOcclusionScale.value = config.parallaxOcclusionScale;
  uniforms.parallaxOcclusionSteps.value = config.parallaxOcclusionSteps;
  uniforms.depthDilationTexels.value = config.depthDilationTexels;
  uniforms.depthConfidenceThreshold.value = config.depthConfidenceThreshold;
  uniforms.splatFootprint.value = config.splatFootprint;
  uniforms.splatOverlap.value = config.splatOverlap;
  uniforms.splatOpacity.value = config.splatOpacity;
  uniforms.normalInfluence.value = config.normalInfluence;
  uniforms.normalOrientationBlend.value = config.normalOrientationBlend;
  uniforms.baseSpriteContribution.value = config.baseSpriteContribution;
  uniforms.viewAngleFalloff.value = config.viewAngleFalloff;
  uniforms.ambientLight.value = config.ambientLight;
  uniforms.diffuseLight.value = config.diffuseLight;
  uniforms.outputGain.value = config.outputGain;
  uniforms.ambientColor.value.setRGB(...config.ambientColor);
  uniforms.lightColor.value.setRGB(...config.lightColor);
  uniforms.lightDirection.value.set(...config.lightDirection);
  uniforms.normalLighting.value = config.lightingMode === 'normal' ? 1 : 0;
  uniforms.representationTransitionMode.value = config.representationTransition === 'opaque'
    ? 0
    : config.representationTransition === 'dither' ? 1 : 2;
  uniforms.representationWeight.value = config.representationWeight;
  uniforms.representationDitherOffset.value = config.representationDitherOffset;
}

function baseMaterial(uniforms: EnhancementUniforms): THREE.ShaderMaterial {
  return new THREE.ShaderMaterial({
    name: 'voxel-sprite-base-material',
    uniforms,
    vertexShader: `${SHARED_VERTEX_HEADER}
      varying vec2 voxelSpriteUv;
      varying float voxelSpriteConfidence;
      void main() {
        voxelSpriteUv = uv;
        vec2 depthCoverage = sampleDilatedDepth(uv);
        voxelSpriteConfidence = confidenceFor(depthCoverage.y);
        vec3 transformed = position;
        transformed.xy *= objectSize;
        transformed.z += visibleDepthOffset(depthCoverage.x)
          * voxelSpriteConfidence
          * baseDepthDisplacement;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(transformed, 1.0);
      }
    `,
    fragmentShader: `${SHARED_FRAGMENT_HEADER}
      varying vec2 voxelSpriteUv;
      varying float voxelSpriteConfidence;
      void main() {
        vec2 presentationUv = parallaxOcclusionUv(voxelSpriteUv);
        vec4 color = texture2D(colorTexture, presentationUv);
        float coverage = texture2D(coverageTexture, presentationUv).r * voxelSpriteConfidence;
        if (coverage < 0.01 || color.a < 0.01) discard;
        vec3 normal = decodedNormal(presentationUv);
        vec3 lighting = lightingFor(normal);
        float contribution = baseSpriteContribution;
        gl_FragColor = vec4(
          color.rgb * lighting * outputGain,
          representationAlpha(color.a * coverage * contribution)
        );
      }
    `,
    transparent: false,
    depthTest: true,
    depthWrite: true,
    side: THREE.DoubleSide,
    blending: THREE.NormalBlending,
  });
}

function splatMaterial(uniforms: EnhancementUniforms): THREE.ShaderMaterial {
  return new THREE.ShaderMaterial({
    name: 'voxel-sprite-splat-material',
    uniforms,
    vertexShader: `${SHARED_VERTEX_HEADER}
      attribute vec2 instanceUv;
      varying vec2 voxelSpriteUv;
      varying float voxelSpriteConfidence;
      varying float voxelSpriteViewWeight;
      void main() {
        voxelSpriteUv = instanceUv;
        vec2 depthCoverage = sampleDilatedDepth(instanceUv);
        voxelSpriteConfidence = confidenceFor(depthCoverage.y);
        vec3 normal = decodedNormal(instanceUv);
        vec3 flatRight = vec3(1.0, 0.0, 0.0);
        vec3 flatUp = vec3(0.0, 1.0, 0.0);
        float safeZ = max(abs(normal.z), 0.2);
        vec3 normalRight = normalize(vec3(1.0, 0.0, -normal.x / safeZ));
        vec3 normalUp = normalize(vec3(0.0, 1.0, -normal.y / safeZ));
        vec3 right = normalize(mix(flatRight, normalRight, normalOrientationBlend));
        vec3 up = normalize(mix(flatUp, normalUp, normalOrientationBlend));
        vec2 cellSize = objectSize / sampleGrid;
        vec2 splatSize = cellSize * splatFootprint * (1.0 + splatOverlap);
        vec3 center = vec3((instanceUv - 0.5) * objectSize, visibleDepthOffset(depthCoverage.x));
        vec3 transformed = center
          + right * position.x * splatSize.x
          + up * position.y * splatSize.y;
        voxelSpriteViewWeight = pow(max(abs(normal.z), 0.001), viewAngleFalloff);
        gl_Position = projectionMatrix * modelViewMatrix * vec4(transformed, 1.0);
      }
    `,
    fragmentShader: `${SHARED_FRAGMENT_HEADER}
      varying vec2 voxelSpriteUv;
      varying float voxelSpriteConfidence;
      varying float voxelSpriteViewWeight;
      void main() {
        vec4 color = texture2D(colorTexture, voxelSpriteUv);
        float coverage = texture2D(coverageTexture, voxelSpriteUv).r * voxelSpriteConfidence;
        if (coverage < 0.01 || color.a < 0.01) discard;
        vec3 normal = decodedNormal(voxelSpriteUv);
        vec3 lighting = lightingFor(normal);
        gl_FragColor = vec4(
          color.rgb * lighting * outputGain,
          representationAlpha(color.a * coverage * voxelSpriteViewWeight * splatOpacity)
        );
      }
    `,
    transparent: true,
    depthTest: true,
    depthWrite: true,
    side: THREE.DoubleSide,
    blending: THREE.NormalBlending,
  });
}

const SHARED_VERTEX_HEADER = `
  uniform sampler2D depthTexture;
  uniform sampler2D normalTexture;
  uniform sampler2D coverageTexture;
  uniform vec2 textureTexelSize;
  uniform vec2 objectSize;
  uniform vec2 sampleGrid;
  uniform float depthAmplitude;
  uniform float depthContrast;
  uniform float depthClamp;
  uniform float captureNear;
  uniform float captureDepthRange;
  uniform float reliefRearDepth;
  uniform float reliefDepthRange;
  uniform float useWorldDepth;
  uniform float depthQuantizationSteps;
  uniform float depthDilationTexels;
  uniform float depthConfidenceThreshold;
  uniform float baseDepthDisplacement;
  uniform float splatFootprint;
  uniform float splatOverlap;
  uniform float normalOrientationBlend;
  uniform float viewAngleFalloff;
  vec2 sampleDilatedDepth(vec2 sourceUv) {
    vec2 offsetUv = textureTexelSize * depthDilationTexels;
    vec2 best = vec2(texture2D(depthTexture, sourceUv).r, texture2D(coverageTexture, sourceUv).r);
    vec2 candidate = vec2(texture2D(depthTexture, sourceUv + vec2(offsetUv.x, 0.0)).r, texture2D(coverageTexture, sourceUv + vec2(offsetUv.x, 0.0)).r);
    if (candidate.y > best.y || (candidate.y == best.y && candidate.x < best.x)) best = candidate;
    candidate = vec2(texture2D(depthTexture, sourceUv - vec2(offsetUv.x, 0.0)).r, texture2D(coverageTexture, sourceUv - vec2(offsetUv.x, 0.0)).r);
    if (candidate.y > best.y || (candidate.y == best.y && candidate.x < best.x)) best = candidate;
    candidate = vec2(texture2D(depthTexture, sourceUv + vec2(0.0, offsetUv.y)).r, texture2D(coverageTexture, sourceUv + vec2(0.0, offsetUv.y)).r);
    if (candidate.y > best.y || (candidate.y == best.y && candidate.x < best.x)) best = candidate;
    candidate = vec2(texture2D(depthTexture, sourceUv - vec2(0.0, offsetUv.y)).r, texture2D(coverageTexture, sourceUv - vec2(0.0, offsetUv.y)).r);
    if (candidate.y > best.y || (candidate.y == best.y && candidate.x < best.x)) best = candidate;
    return best;
  }
  float confidenceFor(float coverage) {
    return smoothstep(depthConfidenceThreshold, min(depthConfidenceThreshold + 0.01, 1.0), coverage);
  }
  float visibleDepthOffset(float depth) {
    float sampledViewDepth = captureNear + depth * captureDepthRange;
    float subjectDepth = clamp(
      (reliefRearDepth - sampledViewDepth) / reliefDepthRange,
      0.0,
      1.0
    );
    float visibleDepth = clamp((subjectDepth - 0.5) * depthContrast + 0.5, 0.0, 1.0);
    if (depthQuantizationSteps > 0.5) {
      visibleDepth = floor(visibleDepth * depthQuantizationSteps + 0.5) / depthQuantizationSteps;
    }
    float scale = mix(1.0, reliefDepthRange, useWorldDepth);
    float centeredDepth = min(visibleDepth, depthClamp) - 0.5;
    return centeredDepth * depthAmplitude * scale;
  }
  vec3 decodedNormal(vec2 sourceUv) {
    return normalize(texture2D(normalTexture, sourceUv).xyz * 2.0 - 1.0);
  }
`;

function captureReliefDepth(frame: VoxelSpriteFrame): { readonly rear: number; readonly range: number } {
  const { basis, bounds } = frame.descriptor.capture;
  const forward = tupleVector(basis.forward).normalize();
  const camera = tupleVector(basis.position);
  const minimum = tupleVector(bounds.minimum);
  const maximum = tupleVector(bounds.maximum);
  const center = minimum.clone().add(maximum).multiplyScalar(0.5);
  const extents = maximum.clone().sub(minimum).multiplyScalar(0.5);
  const centerDepth = center.sub(camera).dot(forward);
  const radius = Math.abs(forward.x) * extents.x
    + Math.abs(forward.y) * extents.y
    + Math.abs(forward.z) * extents.z;
  const captureNear = frame.descriptor.depth.near;
  const captureFar = frame.descriptor.depth.far;
  const front = THREE.MathUtils.clamp(centerDepth - radius, captureNear, captureFar);
  const rear = THREE.MathUtils.clamp(centerDepth + radius, front, captureFar);
  return { rear, range: Math.max(rear - front, 1e-4) };
}

const SHARED_FRAGMENT_HEADER = `
  uniform sampler2D colorTexture;
  uniform sampler2D depthTexture;
  uniform sampler2D normalTexture;
  uniform sampler2D coverageTexture;
  uniform vec2 objectSize;
  uniform float captureNear;
  uniform float captureDepthRange;
  uniform float reliefRearDepth;
  uniform float reliefDepthRange;
  uniform float depthContrast;
  uniform float depthClamp;
  uniform float depthQuantizationSteps;
  uniform float parallaxOcclusionScale;
  uniform float parallaxOcclusionSteps;
  uniform float parallaxOcclusionEnabled;
  uniform vec3 viewerPositionLocal;
  uniform float representationTransitionMode;
  uniform float representationWeight;
  uniform float representationDitherOffset;
  uniform float normalInfluence;
  uniform float baseSpriteContribution;
  uniform float splatOpacity;
  uniform float ambientLight;
  uniform float diffuseLight;
  uniform float outputGain;
  uniform float normalLighting;
  uniform vec3 ambientColor;
  uniform vec3 lightColor;
  uniform vec3 lightDirection;
  float representationAlpha(float sourceAlpha) {
    if (representationWeight <= 0.0001) discard;
    if (representationTransitionMode < 0.5) {
      if (representationWeight < 0.5) discard;
      return sourceAlpha;
    }
    if (representationTransitionMode < 1.5) {
      float threshold = fract(52.9829189 * fract(dot(
        floor(gl_FragCoord.xy),
        vec2(0.06711056, 0.00583715)
      )));
      threshold = fract(threshold - representationDitherOffset + 1.0);
      if (threshold >= representationWeight) discard;
      return sourceAlpha;
    }
    return sourceAlpha * representationWeight;
  }
  float reliefHeight(vec2 sourceUv) {
    float depth = texture2D(depthTexture, sourceUv).r;
    float sampledViewDepth = captureNear + depth * captureDepthRange;
    float subjectDepth = clamp(
      (reliefRearDepth - sampledViewDepth) / reliefDepthRange,
      0.0,
      1.0
    );
    float visibleDepth = clamp((subjectDepth - 0.5) * depthContrast + 0.5, 0.0, 1.0);
    if (depthQuantizationSteps > 0.5) {
      visibleDepth = floor(visibleDepth * depthQuantizationSteps + 0.5) / depthQuantizationSteps;
    }
    return min(visibleDepth, depthClamp);
  }
  vec2 parallaxOcclusionUv(vec2 sourceUv) {
    if (parallaxOcclusionEnabled < 0.5 || parallaxOcclusionSteps < 0.5) return sourceUv;
    vec3 surfacePosition = vec3((sourceUv - 0.5) * objectSize, 0.0);
    vec3 viewDirection = normalize(viewerPositionLocal - surfacePosition);
    float layerDepth = 1.0 / parallaxOcclusionSteps;
    vec2 uvDelta = (viewDirection.xy / max(abs(viewDirection.z), 0.2))
      * parallaxOcclusionScale / parallaxOcclusionSteps;
    vec2 currentUv = sourceUv;
    float traversedDepth = 0.0;
    for (int index = 0; index < 32; index += 1) {
      if (float(index) >= parallaxOcclusionSteps || traversedDepth >= reliefHeight(currentUv)) break;
      currentUv += uvDelta;
      traversedDepth += layerDepth;
    }
    vec2 previousUv = currentUv - uvDelta;
    float afterDepth = reliefHeight(currentUv) - traversedDepth;
    float beforeDepth = reliefHeight(previousUv) - (traversedDepth - layerDepth);
    float denominator = afterDepth - beforeDepth;
    float weight = abs(denominator) < 0.0001 ? 0.0 : clamp(afterDepth / denominator, 0.0, 1.0);
    vec2 resolvedUv = mix(currentUv, previousUv, weight);
    return any(lessThan(resolvedUv, vec2(0.0))) || any(greaterThan(resolvedUv, vec2(1.0)))
      ? sourceUv
      : resolvedUv;
  }
  vec3 decodedNormal(vec2 sourceUv) {
    return normalize(texture2D(normalTexture, sourceUv).xyz * 2.0 - 1.0);
  }
  vec3 lightingFor(vec3 normal) {
    float diffuse = max(dot(normal, normalize(lightDirection)), 0.0);
    vec3 relitValue = ambientColor * ambientLight + lightColor * diffuse * diffuseLight;
    return mix(vec3(1.0), relitValue, normalInfluence * normalLighting);
  }
`;

function splatGeometry(columns: number, rows: number): THREE.InstancedBufferGeometry {
  const geometry = new THREE.InstancedBufferGeometry();
  geometry.setAttribute('position', new THREE.Float32BufferAttribute([
    -0.5, -0.5, 0,
    0.5, -0.5, 0,
    0.5, 0.5, 0,
    -0.5, 0.5, 0,
  ], 3));
  geometry.setIndex([0, 1, 2, 0, 2, 3]);
  const sampleUvs = new Float32Array(columns * rows * 2);
  let offset = 0;
  for (let row = 0; row < rows; row += 1) {
    for (let column = 0; column < columns; column += 1) {
      sampleUvs[offset] = (column + 0.5) / columns;
      sampleUvs[offset + 1] = (row + 0.5) / rows;
      offset += 2;
    }
  }
  geometry.setAttribute('instanceUv', new THREE.InstancedBufferAttribute(sampleUvs, 2));
  geometry.instanceCount = columns * rows;
  return geometry;
}

function composition(
  mode: VoxelSpriteEnhancementMode,
  blendMode: VoxelSpriteSplatBlendMode,
): VoxelSpriteEnhancementReadout['composition'] {
  if (mode === 'sprite-splat') {
    if (blendMode === 'alpha-blend') return 'base-blend-then-alpha-blended-splats';
    if (blendMode === 'additive') return 'base-blend-then-additive-splats';
    return 'base-blend-then-depth-writing-splats';
  }
  if (mode === 'full-splat') {
    if (blendMode === 'alpha-blend') return 'alpha-blended-splats';
    if (blendMode === 'additive') return 'additive-splats';
    return 'depth-writing-splats';
  }
  return 'opaque-depth-writing-base';
}
