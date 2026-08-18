import * as THREE from 'three';
import * as SkeletonUtils from 'three/examples/jsm/utils/SkeletonUtils.js';
import type { RenderHandle, TextureDescriptor } from '@rusty-engine/render-contracts';

import type { ThreeRenderer } from './three-renderer.js';
import {
  VOXEL_SPRITE_CAPTURE_MAX_RESOLUTION,
  VOXEL_SPRITE_CAPTURE_MIN_RESOLUTION,
  VoxelSpriteFrame,
} from './voxel-sprite-capture.js';
import {
  VoxelSpriteEnhancement,
  type VoxelSpriteEnhancementConfig,
  type VoxelSpriteEnhancementMode,
  type VoxelSpriteEnhancementReadout,
} from './voxel-sprite-enhancement.js';
import { VoxelSpriteRuntimeCapture } from './voxel-sprite-capture.js';
import {
  GhostPlatePresentation,
  type GhostPlateConfig,
  type GhostPlateReadout,
} from './voxel-sprite-ghost-plate.js';

export type RendererThreeVoxelSpriteMode = VoxelSpriteEnhancementMode | 'ghost-plate';
export type RendererThreeVoxelSpriteGhostAnchorPolicy = 'bounds-center' | 'bounds-normalized';
export type RendererThreeVoxelSpriteGhostPlateMapping = 'plate-locked' | 'projective-surface';

export interface RendererThreeVoxelSpriteGhostConfig {
  /** Fraction of source-view depth retained by the crushed display mesh. */
  readonly ghostDepthRetention: number;
  /** Selects the fixed source-view depth around which relief is compressed. */
  readonly ghostAnchorPolicy: RendererThreeVoxelSpriteGhostAnchorPolicy;
  /** Front-to-back interpolation used only by `bounds-normalized`. */
  readonly ghostAnchorValue: number;
  /** Chooses screen-locked plate coordinates or source-projective coordinates. */
  readonly ghostPlateMapping: RendererThreeVoxelSpriteGhostPlateMapping;
}

export type RendererThreeVoxelSpriteConfigPatch = Partial<
  VoxelSpriteEnhancementConfig & RendererThreeVoxelSpriteGhostConfig
>;

export interface RendererThreeVoxelSpriteTransform {
  readonly position: readonly [number, number, number];
  readonly width: number;
  readonly height: number;
}

export interface RendererThreeVoxelSpriteCaptureSettings {
  readonly resolution: number;
  readonly azimuthDegrees: number;
  readonly elevationDegrees: number;
  readonly near: number;
  readonly far: number;
  /** Perspective source-view field of view; defaults to the legacy 35 degrees. */
  readonly fieldOfViewDegrees?: number;
  /** Defaults to an isolated, readable capture-light rig. */
  readonly lighting?: RendererThreeVoxelSpriteCaptureLighting;
}

export type RendererThreeVoxelSpriteCaptureLighting =
  | { readonly mode: 'scene' }
  | {
      readonly mode: 'isolated';
      readonly ambientColor?: readonly [number, number, number];
      readonly ambientIntensity?: number;
      /** View-relative direction from the subject toward the key light. */
      readonly keyDirection?: readonly [number, number, number];
      readonly keyColor?: readonly [number, number, number];
      readonly keyIntensity?: number;
      /** View-relative direction from the subject toward the fill light. */
      readonly fillDirection?: readonly [number, number, number];
      readonly fillColor?: readonly [number, number, number];
      readonly fillIntensity?: number;
    };

export interface RendererThreeVoxelSpritePreparedFrame {
  readonly width: number;
  readonly height: number;
  readonly textures: {
    readonly color: string;
    readonly depth: string;
    readonly normal: string;
    readonly coverage: string;
  };
  readonly depth: { readonly near: number; readonly far: number };
  readonly capture: {
    readonly projection: 'perspective' | 'orthographic';
    readonly position: readonly [number, number, number];
    readonly right: readonly [number, number, number];
    readonly up: readonly [number, number, number];
    readonly forward: readonly [number, number, number];
    readonly boundsMinimum: readonly [number, number, number];
    readonly boundsMaximum: readonly [number, number, number];
  };
}

export type RendererThreeVoxelSpriteSource =
  | {
      readonly kind: 'retained';
      readonly handle: RenderHandle;
      readonly capture: RendererThreeVoxelSpriteCaptureSettings;
    }
  | {
      readonly kind: 'prepared';
      readonly frame: RendererThreeVoxelSpritePreparedFrame;
    };

export interface RendererThreeVoxelSpriteDefinition {
  readonly id: string;
  readonly source: RendererThreeVoxelSpriteSource;
  readonly transform: RendererThreeVoxelSpriteTransform;
  readonly mode: RendererThreeVoxelSpriteMode;
  readonly config?: Omit<RendererThreeVoxelSpriteConfigPatch, 'mode' | 'width' | 'height'>;
}

export interface RendererThreeVoxelSpriteDiagnostic {
  readonly code: 'disposed' | 'duplicate_id' | 'invalid_definition' | 'missing_source' | 'capture_failed' | 'unknown_id';
  readonly message: string;
}

export interface RendererThreeVoxelSpriteReceipt {
  readonly applied: boolean;
  readonly diagnostics: readonly RendererThreeVoxelSpriteDiagnostic[];
  readonly readout: RendererThreeVoxelSpriteSceneReadout;
}

export interface RendererThreeVoxelSpriteEntryReadout {
  readonly id: string;
  readonly source: 'retained' | 'prepared';
  readonly sourceHandle: number | null;
  readonly capture: RendererThreeVoxelSpriteCaptureSettings | null;
  readonly fallbackPreservedCount: number;
  readonly presentation: 'enhancement' | 'ghost-plate';
  readonly enhancement: VoxelSpriteEnhancementReadout | null;
  readonly ghostPlate: GhostPlateReadout | null;
}

export interface RendererThreeVoxelSpriteSceneReadout {
  readonly schemaVersion: 1;
  readonly revision: number;
  readonly entries: readonly RendererThreeVoxelSpriteEntryReadout[];
  readonly disposed: boolean;
}

export interface RendererThreeVoxelSpriteBackend {
  readonly scene: THREE.Scene;
  objectFor(handle: RenderHandle): THREE.Object3D | undefined;
  textureDescriptor(id: string): TextureDescriptor | undefined;
  textureObjectFor(id: string): THREE.Texture | undefined;
}

interface Entry {
  readonly id: string;
  readonly enhancement: VoxelSpriteEnhancement | null;
  readonly ghostPlate: GhostPlatePresentation | null;
  readonly frame: VoxelSpriteFrame;
  readonly source: RendererThreeVoxelSpriteSource;
  readonly transform: RendererThreeVoxelSpriteTransform;
  readonly runtimeCapture: VoxelSpriteRuntimeCapture | null;
  readonly retainedObject: THREE.Object3D | null;
  readonly retainedOriginalVisibility: boolean | null;
  releaseCanonicalSuppression: (() => void) | null;
  captureSettings: RendererThreeVoxelSpriteCaptureSettings | null;
  fallbackPreservedCount: number;
}

/** Three-local scene attachment behind renderer-host and application-host ports. */
export class RendererThreeVoxelSpriteScene {
  readonly #webgl: THREE.WebGLRenderer;
  readonly #backend: RendererThreeVoxelSpriteBackend;
  readonly #invalidate: () => void;
  readonly #onDispose: (() => void) | null;
  readonly #entries = new Map<string, Entry>();
  readonly #canonicalSuppressions = new Map<THREE.Object3D, {
    count: number;
    readonly layerZeroEnabled: ReadonlyMap<THREE.Object3D, boolean>;
  }>();
  #disposed = false;
  #revision = 0;

  constructor(options: {
    readonly webgl: THREE.WebGLRenderer;
    readonly backend: ThreeRenderer | RendererThreeVoxelSpriteBackend;
    readonly invalidate?: () => void;
    readonly onDispose?: () => void;
  }) {
    this.#webgl = options.webgl;
    this.#backend = options.backend;
    this.#invalidate = options.invalidate ?? (() => undefined);
    this.#onDispose = options.onDispose ?? null;
  }

  create(definition: RendererThreeVoxelSpriteDefinition): RendererThreeVoxelSpriteReceipt {
    if (this.#disposed) return this.#rejected('disposed', 'voxel sprite scene is disposed');
    if (this.#entries.has(definition.id)) return this.#rejected('duplicate_id', `duplicate voxel sprite id ${definition.id}`);
    let entry: Entry;
    try {
      entry = this.#buildEntry(validatedDefinition(definition));
    } catch (cause) {
      return this.#rejected(classifyCause(cause), messageFrom(cause));
    }
    this.#entries.set(entry.id, entry);
    this.#backend.scene.add(presentationObject(entry));
    if (entry.ghostPlate !== null && entry.retainedObject !== null) {
      entry.releaseCanonicalSuppression = this.#acquireCanonicalSuppression(entry.retainedObject);
    }
    if (entry.enhancement !== null && entry.retainedObject !== null) entry.retainedObject.visible = false;
    this.#revision += 1;
    this.#invalidate();
    return this.#applied();
  }

  /** Build first and publish second so a failed source/config replacement preserves the live entry. */
  replace(definition: RendererThreeVoxelSpriteDefinition): RendererThreeVoxelSpriteReceipt {
    if (this.#disposed) return this.#rejected('disposed', 'voxel sprite scene is disposed');
    const previous = this.#entries.get(definition.id);
    if (previous === undefined) {
      return this.#rejected('unknown_id', `unknown voxel sprite id ${definition.id}`);
    }
    let candidate: Entry;
    try {
      candidate = this.#buildEntry(validatedDefinition(definition));
    } catch (cause) {
      return this.#rejected(classifyCause(cause), messageFrom(cause));
    }

    this.#disposeEntry(previous);
    this.#entries.set(candidate.id, candidate);
    this.#backend.scene.add(presentationObject(candidate));
    if (candidate.ghostPlate !== null && candidate.retainedObject !== null) {
      candidate.releaseCanonicalSuppression = this.#acquireCanonicalSuppression(candidate.retainedObject);
    }
    if (candidate.enhancement !== null && candidate.retainedObject !== null) candidate.retainedObject.visible = false;
    this.#revision += 1;
    this.#invalidate();
    return this.#applied();
  }

  configure(
    id: string,
    patch: RendererThreeVoxelSpriteConfigPatch,
  ): RendererThreeVoxelSpriteReceipt {
    if (this.#disposed) return this.#rejected('disposed', 'voxel sprite scene is disposed');
    const entry = this.#entries.get(id);
    if (entry === undefined) return this.#rejected('unknown_id', `unknown voxel sprite id ${id}`);
    try {
      if (entry.ghostPlate !== null) entry.ghostPlate.configure(ghostConfigPatch(patch));
      else entry.enhancement!.configure(enhancementConfigPatch(patch));
    } catch (cause) {
      return this.#rejected('invalid_definition', messageFrom(cause));
    }
    this.#revision += 1;
    this.#invalidate();
    return this.#applied();
  }

  recapture(
    id: string,
    settings?: RendererThreeVoxelSpriteCaptureSettings,
  ): RendererThreeVoxelSpriteReceipt {
    if (this.#disposed) return this.#rejected('disposed', 'voxel sprite scene is disposed');
    const entry = this.#entries.get(id);
    if (entry === undefined) return this.#rejected('unknown_id', `unknown voxel sprite id ${id}`);
    if (entry.source.kind !== 'retained'
      || entry.runtimeCapture === null
      || entry.retainedObject === null) {
      return this.#rejected('invalid_definition', `voxel sprite ${id} is not a retained capture source`);
    }
    let captureSettings: RendererThreeVoxelSpriteCaptureSettings;
    try {
      captureSettings = validatedCapture(settings ?? entry.captureSettings!);
    } catch (cause) {
      return this.#rejected('invalid_definition', messageFrom(cause));
    }
    if (entry.ghostPlate !== null) {
      let candidate: Entry;
      try {
        const ghost = entry.ghostPlate.readout();
        candidate = this.#buildEntry({
          id,
          source: { ...entry.source, capture: captureSettings },
          transform: entry.transform,
          mode: 'ghost-plate',
          config: {
            ghostDepthRetention: ghost.depthRetention,
            ghostAnchorPolicy: ghost.anchorPolicy,
            ghostAnchorValue: ghost.anchorValue,
            ghostPlateMapping: ghost.plateMapping,
          },
        });
      } catch (cause) {
        entry.fallbackPreservedCount += 1;
        return this.#rejected(classifyCause(cause), messageFrom(cause));
      }
      this.#disposeEntry(entry);
      this.#entries.set(id, candidate);
      this.#backend.scene.add(presentationObject(candidate));
      candidate.releaseCanonicalSuppression = this.#acquireCanonicalSuppression(candidate.retainedObject!);
      this.#revision += 1;
      this.#invalidate();
      return this.#applied();
    }

    const receipt = this.#captureRetained(entry.runtimeCapture, entry.retainedObject, captureSettings);
    if (!receipt.applied || receipt.frame === null) {
      entry.fallbackPreservedCount += 1;
      return this.#rejected('capture_failed', receipt.diagnostics[0]?.message ?? 'runtime recapture failed');
    }
    entry.enhancement!.replaceSource({
      frame: receipt.frame,
      captureCpuSubmissionMilliseconds: receipt.readout.cpuSubmissionMilliseconds,
    });
    entry.captureSettings = captureSettings;
    this.#revision += 1;
    this.#invalidate();
    return this.#applied();
  }

  destroy(id: string): RendererThreeVoxelSpriteReceipt {
    if (this.#disposed) return this.#rejected('disposed', 'voxel sprite scene is disposed');
    const entry = this.#entries.get(id);
    if (entry === undefined) return this.#rejected('unknown_id', `unknown voxel sprite id ${id}`);
    this.#disposeEntry(entry);
    this.#entries.delete(id);
    this.#revision += 1;
    this.#invalidate();
    return this.#applied();
  }

  prepare(camera: THREE.Camera): void {
    if (this.#disposed) return;
    for (const entry of this.#entries.values()) {
      if (entry.ghostPlate !== null) entry.ghostPlate.prepare(camera);
      else entry.enhancement!.prepare(camera);
    }
  }

  recordCpuSubmission(milliseconds: number): void {
    if (this.#disposed) return;
    for (const entry of this.#entries.values()) {
      entry.enhancement?.recordSteadyStateFrame(milliseconds);
    }
  }

  readout(): RendererThreeVoxelSpriteSceneReadout {
    return Object.freeze({
      schemaVersion: 1,
      revision: this.#revision,
      entries: Object.freeze([...this.#entries.values()]
        .map((entry) => Object.freeze({
          id: entry.id,
          source: entry.source.kind,
          sourceHandle: entry.source.kind === 'retained' ? entry.source.handle : null,
          capture: entry.captureSettings === null ? null : Object.freeze({ ...entry.captureSettings }),
          fallbackPreservedCount: entry.fallbackPreservedCount,
          presentation: entry.ghostPlate === null ? 'enhancement' as const : 'ghost-plate' as const,
          enhancement: entry.enhancement?.readout() ?? null,
          ghostPlate: entry.ghostPlate?.readout() ?? null,
        }))
        .sort((left, right) => left.id.localeCompare(right.id))),
      disposed: this.#disposed,
    });
  }

  dispose(): void {
    if (this.#disposed) return;
    for (const entry of this.#entries.values()) this.#disposeEntry(entry);
    this.#entries.clear();
    this.#disposed = true;
    this.#revision += 1;
    this.#invalidate();
    this.#onDispose?.();
  }

  #buildEntry(definition: RendererThreeVoxelSpriteDefinition): Entry {
    if (definition.mode === 'ghost-plate') return this.#buildGhostEntry(definition);
    let frame: VoxelSpriteFrame;
    let runtimeCapture: VoxelSpriteRuntimeCapture | null = null;
    let retainedObject: THREE.Object3D | null = null;
    let retainedOriginalVisibility: boolean | null = null;
    let captureCpuSubmissionMilliseconds: number | null = null;
    if (definition.source.kind === 'prepared') {
      frame = this.#preparedFrame(definition.source.frame);
    } else {
      retainedObject = this.#backend.objectFor(definition.source.handle) ?? null;
      if (retainedObject === null) throw new MissingSourceError(`retained handle ${String(definition.source.handle)} is unavailable`);
      retainedOriginalVisibility = retainedObject.visible;
      runtimeCapture = new VoxelSpriteRuntimeCapture(this.#webgl);
      const receipt = this.#captureRetained(runtimeCapture, retainedObject, definition.source.capture);
      if (!receipt.applied || receipt.frame === null) {
        runtimeCapture.dispose();
        throw new CaptureSourceError(receipt.diagnostics[0]?.message ?? 'runtime capture failed');
      }
      frame = receipt.frame;
      captureCpuSubmissionMilliseconds = receipt.readout.cpuSubmissionMilliseconds;
    }
    let enhancement: VoxelSpriteEnhancement;
    try {
      enhancement = new VoxelSpriteEnhancement(
        { frame, captureCpuSubmissionMilliseconds },
        {
          ...enhancementConfigPatch(definition.config ?? {}),
          mode: definition.mode,
          width: definition.transform.width,
          height: definition.transform.height,
        },
      );
    } catch (cause) {
      runtimeCapture?.dispose();
      if (runtimeCapture === null) frame.dispose();
      throw cause;
    }
    enhancement.object.position.set(...definition.transform.position);
    return {
      id: definition.id,
      enhancement,
      ghostPlate: null,
      frame,
      source: definition.source,
      transform: definition.transform,
      runtimeCapture,
      retainedObject,
      retainedOriginalVisibility,
      releaseCanonicalSuppression: null,
      captureSettings: definition.source.kind === 'retained' ? definition.source.capture : null,
      fallbackPreservedCount: 0,
    };
  }

  #buildGhostEntry(definition: RendererThreeVoxelSpriteDefinition): Entry {
    if (definition.source.kind !== 'retained') {
      throw new TypeError('ghost-plate requires a retained source; prepared frames remain available to ordinary proxy modes');
    }
    const retainedObject = this.#backend.objectFor(definition.source.handle) ?? null;
    if (retainedObject === null) {
      throw new MissingSourceError(`retained handle ${String(definition.source.handle)} is unavailable`);
    }
    const runtimeCapture = new VoxelSpriteRuntimeCapture(this.#webgl);
    const appearanceRoot = SkeletonUtils.clone(retainedObject);
    let presentation: GhostPlatePresentation | null = null;
    try {
      retainedObject.updateWorldMatrix(true, true);
      const clonedLights: THREE.Light[] = [];
      appearanceRoot.traverse((object) => {
        if (object instanceof THREE.Light) clonedLights.push(object);
      });
      for (const light of clonedLights) light.removeFromParent();
      appearanceRoot.matrix.copy(retainedObject.matrixWorld);
      appearanceRoot.matrixAutoUpdate = false;
      appearanceRoot.visible = true;
      appearanceRoot.traverse((object) => {
        if (isRenderable(object)) object.layers.enable(0);
      });
      appearanceRoot.updateWorldMatrix(true, true);
      const captureScene = new THREE.Scene();
      captureScene.add(appearanceRoot);
      const bounds = new THREE.Box3().setFromObject(appearanceRoot, true);
      if (bounds.isEmpty()) throw new CaptureSourceError('retained source bounds are empty');
      const center = bounds.getCenter(new THREE.Vector3());
      const size = bounds.getSize(new THREE.Vector3());
      const camera = captureCamera(definition.source.capture, center, size);
      const disposeLighting = definition.source.capture.lighting?.mode === 'scene'
        ? addClonedSceneLights(this.#backend.scene, captureScene)
        : addStudioRig(
            captureScene,
            camera,
            center,
            size,
            studioLighting(definition.source.capture.lighting),
          );
      const receipt = runtimeCapture.capture({
        scene: captureScene,
        camera,
        width: definition.source.capture.resolution,
        height: definition.source.capture.resolution,
        bounds,
      });
      disposeLighting();
      captureScene.remove(appearanceRoot);
      if (!receipt.applied || receipt.frame === null) {
        throw new CaptureSourceError(receipt.diagnostics[0]?.message ?? 'runtime capture failed');
      }
      presentation = new GhostPlatePresentation({
        appearanceRoot,
        colorTexture: receipt.frame.descriptor.textures.color,
        coverageTexture: receipt.frame.descriptor.textures.coverage,
        projectionKind: camera instanceof THREE.PerspectiveCamera ? 'perspective' : 'orthographic',
        ghostCameraWorld: camera.matrixWorld.clone(),
        ghostProjection: camera.projectionMatrix.clone(),
        bounds,
        transform: definition.transform,
        config: validatedGhostConfig(definition.config ?? {}),
      });
      return {
        id: definition.id,
        enhancement: null,
        ghostPlate: presentation,
        frame: receipt.frame,
        source: definition.source,
        transform: definition.transform,
        runtimeCapture,
        retainedObject,
        retainedOriginalVisibility: null,
        releaseCanonicalSuppression: null,
        captureSettings: definition.source.capture,
        fallbackPreservedCount: 0,
      };
    } catch (cause) {
      presentation?.dispose();
      if (presentation === null) disposeClonedSkeletons(appearanceRoot);
      runtimeCapture.dispose();
      throw cause;
    }
  }

  #preparedFrame(input: RendererThreeVoxelSpritePreparedFrame): VoxelSpriteFrame {
    const color = this.#backend.textureObjectFor(input.textures.color);
    const depth = this.#backend.textureObjectFor(input.textures.depth);
    const normal = this.#backend.textureObjectFor(input.textures.normal);
    const coverage = this.#backend.textureObjectFor(input.textures.coverage);
    if (color === undefined || depth === undefined || normal === undefined || coverage === undefined) {
      throw new MissingSourceError('one or more prepared voxel sprite textures are unavailable');
    }
    const descriptors = {
      color: this.#backend.textureDescriptor(input.textures.color),
      depth: this.#backend.textureDescriptor(input.textures.depth),
      normal: this.#backend.textureDescriptor(input.textures.normal),
      coverage: this.#backend.textureDescriptor(input.textures.coverage),
    };
    if (Object.values(descriptors).some((descriptor) => descriptor === undefined)) {
      throw new MissingSourceError('one or more prepared voxel sprite texture descriptors are unavailable');
    }
    for (const [channel, descriptor] of Object.entries(descriptors)) {
      if (descriptor!.width !== input.width || descriptor!.height !== input.height) {
        throw new TypeError(`prepared ${channel} texture dimensions do not match frame dimensions`);
      }
    }
    if (descriptors.color!.payload?.colorSpace !== 'srgb') {
      throw new TypeError('prepared color texture must use sRGB color space');
    }
    for (const [channel, descriptor] of Object.entries({
      depth: descriptors.depth!,
      normal: descriptors.normal!,
      coverage: descriptors.coverage!,
    })) {
      if (descriptor.payload?.colorSpace !== 'linear') {
        throw new TypeError(`prepared ${channel} texture must use linear color space`);
      }
    }
    return VoxelSpriteFrame.borrowed({
      width: input.width,
      height: input.height,
      textures: { color, depth, normal, coverage },
      depth: { encoding: 'linear-view-depth-unorm8', ...input.depth },
      normalSpace: 'view',
      capture: {
        projection: input.capture.projection,
        basis: {
          position: input.capture.position,
          right: input.capture.right,
          up: input.capture.up,
          forward: input.capture.forward,
        },
        bounds: {
          minimum: input.capture.boundsMinimum,
          maximum: input.capture.boundsMaximum,
        },
      },
    });
  }

  #captureRetained(
    capture: VoxelSpriteRuntimeCapture,
    source: THREE.Object3D,
    settings: RendererThreeVoxelSpriteCaptureSettings,
  ): ReturnType<VoxelSpriteRuntimeCapture['capture']> {
    const visibility = new Map<THREE.Object3D, boolean>();
    const lightVisibility = new Map<THREE.Light, boolean>();
    const sourceLayers = new Map<THREE.Object3D, number>();
    this.#backend.scene.traverse((object) => {
      if (isRenderable(object)) visibility.set(object, object.visible);
      if (object instanceof THREE.Light) lightVisibility.set(object, object.visible);
    });
    const sourceVisibility = source.visible;
    let disposeStudioRig: () => void = () => undefined;
    try {
      for (const object of visibility.keys()) {
        if (!isDescendantOrSelf(object, source)) object.visible = false;
      }
      source.traverse((object) => {
        if (!isRenderable(object)) return;
        sourceLayers.set(object, object.layers.mask);
        object.layers.enable(0);
      });
      source.visible = true;
      source.updateWorldMatrix(true, true);
      const bounds = new THREE.Box3().setFromObject(source, true);
      if (bounds.isEmpty()) throw new CaptureSourceError('retained source bounds are empty');
      const center = bounds.getCenter(new THREE.Vector3());
      const size = bounds.getSize(new THREE.Vector3());
      const camera = captureCamera(settings, center, size);
      if (settings.lighting?.mode !== 'scene') {
        for (const light of lightVisibility.keys()) light.visible = false;
        disposeStudioRig = addStudioRig(
          this.#backend.scene,
          camera,
          center,
          size,
          studioLighting(settings.lighting),
        );
      }
      return capture.capture({
        scene: this.#backend.scene,
        camera,
        width: settings.resolution,
        height: settings.resolution,
        bounds,
      });
    } finally {
      disposeStudioRig();
      for (const [light, visible] of lightVisibility) light.visible = visible;
      for (const [object, visible] of visibility) object.visible = visible;
      for (const [object, mask] of sourceLayers) object.layers.mask = mask;
      source.visible = sourceVisibility;
    }
  }

  #disposeEntry(entry: Entry): void {
    this.#backend.scene.remove(presentationObject(entry));
    entry.enhancement?.dispose();
    entry.ghostPlate?.dispose();
    entry.runtimeCapture?.dispose();
    entry.releaseCanonicalSuppression?.();
    entry.releaseCanonicalSuppression = null;
    if (entry.runtimeCapture === null) entry.frame.dispose();
    if (entry.retainedObject !== null && entry.retainedOriginalVisibility !== null) {
      entry.retainedObject.visible = entry.retainedOriginalVisibility;
    }
  }

  #acquireCanonicalSuppression(source: THREE.Object3D): () => void {
    const existing = this.#canonicalSuppressions.get(source);
    if (existing !== undefined) {
      existing.count += 1;
      return () => this.#releaseCanonicalSuppression(source);
    }
    const layerZeroEnabled = new Map<THREE.Object3D, boolean>();
    source.traverse((object) => {
      if (!isRenderable(object)) return;
      layerZeroEnabled.set(object, (object.layers.mask & 1) !== 0);
      object.layers.disable(0);
    });
    this.#canonicalSuppressions.set(source, { count: 1, layerZeroEnabled });
    return () => this.#releaseCanonicalSuppression(source);
  }

  #releaseCanonicalSuppression(source: THREE.Object3D): void {
    const suppression = this.#canonicalSuppressions.get(source);
    if (suppression === undefined) return;
    suppression.count -= 1;
    if (suppression.count > 0) return;
    for (const [object, enabled] of suppression.layerZeroEnabled) {
      if (enabled) object.layers.enable(0);
      else object.layers.disable(0);
    }
    this.#canonicalSuppressions.delete(source);
  }

  #applied(): RendererThreeVoxelSpriteReceipt {
    return Object.freeze({ applied: true, diagnostics: Object.freeze([]), readout: this.readout() });
  }

  #rejected(
    code: RendererThreeVoxelSpriteDiagnostic['code'],
    message: string,
  ): RendererThreeVoxelSpriteReceipt {
    return Object.freeze({
      applied: false,
      diagnostics: Object.freeze([Object.freeze({ code, message })]),
      readout: this.readout(),
    });
  }
}

class MissingSourceError extends Error {}
class CaptureSourceError extends Error {}

function validatedDefinition(input: RendererThreeVoxelSpriteDefinition): RendererThreeVoxelSpriteDefinition {
  if (!/^[a-zA-Z0-9][a-zA-Z0-9._-]{0,63}$/.test(input.id)) throw new TypeError('voxel sprite id is invalid');
  finiteTuple(input.transform.position, 'transform position');
  bounded(input.transform.width, 0.05, 64, 'transform width');
  bounded(input.transform.height, 0.05, 64, 'transform height');
  if (input.source.kind === 'retained') {
    return Object.freeze({
      ...input,
      source: Object.freeze({ ...input.source, capture: validatedCapture(input.source.capture) }),
    });
  }
  return input;
}

function validatedCapture(input: RendererThreeVoxelSpriteCaptureSettings): RendererThreeVoxelSpriteCaptureSettings {
  if (!Number.isInteger(input.resolution)
    || input.resolution < VOXEL_SPRITE_CAPTURE_MIN_RESOLUTION
    || input.resolution > VOXEL_SPRITE_CAPTURE_MAX_RESOLUTION) {
    throw new RangeError(
      `capture resolution must be an integer from ${String(VOXEL_SPRITE_CAPTURE_MIN_RESOLUTION)}`
      + ` to ${String(VOXEL_SPRITE_CAPTURE_MAX_RESOLUTION)}`,
    );
  }
  bounded(input.azimuthDegrees, -360, 360, 'capture azimuth');
  bounded(input.elevationDegrees, -89, 89, 'capture elevation');
  bounded(input.near, 0.001, 100, 'capture near');
  bounded(input.far, input.near + 0.001, 10_000, 'capture far');
  bounded(input.fieldOfViewDegrees ?? 35, 10, 120, 'capture field of view');
  const lighting = input.lighting?.mode === 'scene'
    ? Object.freeze({ mode: 'scene' as const })
    : studioLighting(input.lighting);
  return Object.freeze({ ...input, lighting });
}

interface NormalizedStudioLighting {
  readonly mode: 'isolated';
  readonly ambientColor: readonly [number, number, number];
  readonly ambientIntensity: number;
  readonly keyDirection: readonly [number, number, number];
  readonly keyColor: readonly [number, number, number];
  readonly keyIntensity: number;
  readonly fillDirection: readonly [number, number, number];
  readonly fillColor: readonly [number, number, number];
  readonly fillIntensity: number;
}

function studioLighting(
  input: Exclude<RendererThreeVoxelSpriteCaptureLighting, { readonly mode: 'scene' }> | undefined,
): NormalizedStudioLighting {
  return Object.freeze({
    mode: 'isolated',
    ambientColor: colorTuple(input?.ambientColor ?? [1, 1, 1], 'capture ambientColor'),
    ambientIntensity: boundedValue(input?.ambientIntensity ?? 1.1, 0, 8, 'capture ambientIntensity'),
    keyDirection: normalizedTuple(input?.keyDirection ?? [0.55, 0.8, 1], 'capture keyDirection'),
    keyColor: colorTuple(input?.keyColor ?? [1, 0.95, 0.85], 'capture keyColor'),
    keyIntensity: boundedValue(input?.keyIntensity ?? 2.4, 0, 8, 'capture keyIntensity'),
    fillDirection: normalizedTuple(input?.fillDirection ?? [-0.7, 0.25, 0.65], 'capture fillDirection'),
    fillColor: colorTuple(input?.fillColor ?? [0.55, 0.7, 1], 'capture fillColor'),
    fillIntensity: boundedValue(input?.fillIntensity ?? 1, 0, 8, 'capture fillIntensity'),
  });
}

function addStudioRig(
  scene: THREE.Scene,
  camera: THREE.Camera,
  center: THREE.Vector3,
  size: THREE.Vector3,
  lighting: NormalizedStudioLighting,
): () => void {
  camera.updateMatrixWorld(true);
  const distance = Math.max(2, size.length() * 2);
  const ambient = new THREE.AmbientLight(
    new THREE.Color().setRGB(...lighting.ambientColor),
    lighting.ambientIntensity,
  );
  const key = studioDirectionalLight(
    camera,
    center,
    distance,
    lighting.keyDirection,
    lighting.keyColor,
    lighting.keyIntensity,
  );
  const fill = studioDirectionalLight(
    camera,
    center,
    distance,
    lighting.fillDirection,
    lighting.fillColor,
    lighting.fillIntensity,
  );
  scene.add(ambient, key.light, key.target, fill.light, fill.target);
  return () => scene.remove(ambient, key.light, key.target, fill.light, fill.target);
}

function studioDirectionalLight(
  camera: THREE.Camera,
  center: THREE.Vector3,
  distance: number,
  direction: readonly [number, number, number],
  color: readonly [number, number, number],
  intensity: number,
): { readonly light: THREE.DirectionalLight; readonly target: THREE.Object3D } {
  const towardLight = new THREE.Vector3(...direction).applyQuaternion(camera.quaternion).normalize();
  const light = new THREE.DirectionalLight(new THREE.Color().setRGB(...color), intensity);
  const target = new THREE.Object3D();
  target.position.copy(center);
  light.position.copy(center).addScaledVector(towardLight, distance);
  light.target = target;
  return { light, target };
}

function boundedValue(value: number, minimum: number, maximum: number, name: string): number {
  bounded(value, minimum, maximum, name);
  return value;
}

function normalizedTuple(
  value: readonly [number, number, number],
  name: string,
): readonly [number, number, number] {
  finiteTuple(value, name);
  const vector = new THREE.Vector3(...value);
  if (vector.lengthSq() < 1e-8) throw new RangeError(`${name} must be nonzero`);
  vector.normalize();
  return Object.freeze(vector.toArray()) as unknown as readonly [number, number, number];
}

function colorTuple(
  value: readonly [number, number, number],
  name: string,
): readonly [number, number, number] {
  finiteTuple(value, name);
  if (value.some((component) => component < 0 || component > 1)) {
    throw new RangeError(`${name} values must be from 0 to 1`);
  }
  return Object.freeze([...value]) as unknown as readonly [number, number, number];
}

function captureCamera(
  settings: RendererThreeVoxelSpriteCaptureSettings,
  center: THREE.Vector3,
  size: THREE.Vector3,
): THREE.PerspectiveCamera {
  const camera = new THREE.PerspectiveCamera(
    settings.fieldOfViewDegrees ?? 35,
    1,
    settings.near,
    settings.far,
  );
  const azimuth = THREE.MathUtils.degToRad(settings.azimuthDegrees);
  const elevation = THREE.MathUtils.degToRad(settings.elevationDegrees);
  const radius = Math.max(size.length() * 1.7, 1);
  camera.position.set(
    center.x + Math.sin(azimuth) * Math.cos(elevation) * radius,
    center.y + Math.sin(elevation) * radius,
    center.z + Math.cos(azimuth) * Math.cos(elevation) * radius,
  );
  camera.lookAt(center);
  camera.updateMatrixWorld(true);
  return camera;
}

function presentationObject(entry: Entry): THREE.Object3D {
  return entry.ghostPlate?.object ?? entry.enhancement!.object;
}

function enhancementConfigPatch(
  patch: RendererThreeVoxelSpriteConfigPatch,
): Partial<VoxelSpriteEnhancementConfig> {
  const {
    ghostDepthRetention: _depthRetention,
    ghostAnchorPolicy: _anchorPolicy,
    ghostAnchorValue: _anchorValue,
    ghostPlateMapping: _plateMapping,
    ...enhancement
  } = patch;
  return enhancement;
}

function ghostConfigPatch(patch: RendererThreeVoxelSpriteConfigPatch): Partial<GhostPlateConfig> {
  const allowed = new Set([
    'ghostDepthRetention',
    'ghostAnchorPolicy',
    'ghostAnchorValue',
    'ghostPlateMapping',
  ]);
  const unsupported = Object.keys(patch).filter((key) => !allowed.has(key));
  if (unsupported.length > 0) {
    throw new TypeError(`ghost-plate configure does not accept ${unsupported.join(', ')}`);
  }
  return {
    ...(patch.ghostDepthRetention === undefined ? {} : { depthRetention: patch.ghostDepthRetention }),
    ...(patch.ghostAnchorPolicy === undefined ? {} : { anchorPolicy: patch.ghostAnchorPolicy }),
    ...(patch.ghostAnchorValue === undefined ? {} : { anchorValue: patch.ghostAnchorValue }),
    ...(patch.ghostPlateMapping === undefined ? {} : { plateMapping: patch.ghostPlateMapping }),
  };
}

function validatedGhostConfig(patch: RendererThreeVoxelSpriteConfigPatch): GhostPlateConfig {
  const config: GhostPlateConfig = {
    depthRetention: patch.ghostDepthRetention ?? 0.12,
    anchorPolicy: patch.ghostAnchorPolicy ?? 'bounds-center',
    anchorValue: patch.ghostAnchorValue ?? 0.5,
    plateMapping: patch.ghostPlateMapping ?? 'plate-locked',
  };
  ghostConfigPatch(patch);
  return config;
}

function addClonedSceneLights(source: THREE.Scene, target: THREE.Scene): () => void {
  const clones: THREE.Object3D[] = [];
  source.updateWorldMatrix(true, true);
  source.traverse((object) => {
    if (!(object instanceof THREE.Light) || !object.visible) return;
    const clone = object.clone(false) as THREE.Light;
    object.matrixWorld.decompose(clone.position, clone.quaternion, clone.scale);
    clone.matrixAutoUpdate = true;
    target.add(clone);
    clones.push(clone);
    if ((object instanceof THREE.DirectionalLight || object instanceof THREE.SpotLight)
      && (clone instanceof THREE.DirectionalLight || clone instanceof THREE.SpotLight)) {
      object.target.updateWorldMatrix(true, false);
      const targetClone = new THREE.Object3D();
      object.target.matrixWorld.decompose(targetClone.position, targetClone.quaternion, targetClone.scale);
      clone.target = targetClone;
      target.add(targetClone);
      clones.push(targetClone);
    }
  });
  return () => target.remove(...clones);
}

function disposeClonedSkeletons(root: THREE.Object3D): void {
  const skeletons = new Set<THREE.Skeleton>();
  root.traverse((object) => {
    if (object instanceof THREE.SkinnedMesh) skeletons.add(object.skeleton);
  });
  for (const skeleton of skeletons) skeleton.dispose();
}

function isRenderable(object: THREE.Object3D): boolean {
  return object instanceof THREE.Mesh
    || object instanceof THREE.Line
    || object instanceof THREE.Points
    || object instanceof THREE.Sprite;
}

function isDescendantOrSelf(object: THREE.Object3D, ancestor: THREE.Object3D): boolean {
  let current: THREE.Object3D | null = object;
  while (current !== null) {
    if (current === ancestor) return true;
    current = current.parent;
  }
  return false;
}

function finiteTuple(value: readonly number[], name: string): void {
  if (value.length !== 3 || value.some((component) => !Number.isFinite(component))) {
    throw new TypeError(`${name} must contain three finite values`);
  }
}

function bounded(value: number, minimum: number, maximum: number, name: string): void {
  if (!Number.isFinite(value) || value < minimum || value > maximum) {
    throw new RangeError(`${name} must be finite from ${String(minimum)} to ${String(maximum)}`);
  }
}

function classifyCause(cause: unknown): RendererThreeVoxelSpriteDiagnostic['code'] {
  if (cause instanceof MissingSourceError) return 'missing_source';
  if (cause instanceof CaptureSourceError) return 'capture_failed';
  return 'invalid_definition';
}

function messageFrom(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}
