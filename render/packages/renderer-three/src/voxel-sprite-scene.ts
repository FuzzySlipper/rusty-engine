import * as THREE from 'three';
import type { RenderHandle, TextureDescriptor } from '@rusty-engine/render-contracts';

import type { ThreeRenderer } from './three-renderer.js';
import { VoxelSpriteFrame } from './voxel-sprite-capture.js';
import {
  VoxelSpriteEnhancement,
  type VoxelSpriteEnhancementConfig,
  type VoxelSpriteEnhancementMode,
  type VoxelSpriteEnhancementReadout,
} from './voxel-sprite-enhancement.js';
import { VoxelSpriteRuntimeCapture } from './voxel-sprite-capture.js';

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
}

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
  readonly mode: VoxelSpriteEnhancementMode;
  readonly config?: Partial<Omit<VoxelSpriteEnhancementConfig, 'mode' | 'width' | 'height'>>;
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
  readonly enhancement: VoxelSpriteEnhancementReadout;
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
  readonly enhancement: VoxelSpriteEnhancement;
  readonly frame: VoxelSpriteFrame;
  readonly source: RendererThreeVoxelSpriteSource;
  readonly runtimeCapture: VoxelSpriteRuntimeCapture | null;
  readonly retainedObject: THREE.Object3D | null;
  readonly retainedOriginalVisibility: boolean | null;
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
    this.#backend.scene.add(entry.enhancement.object);
    if (entry.retainedObject !== null) entry.retainedObject.visible = false;
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
    this.#backend.scene.add(candidate.enhancement.object);
    if (candidate.retainedObject !== null) candidate.retainedObject.visible = false;
    this.#revision += 1;
    this.#invalidate();
    return this.#applied();
  }

  configure(
    id: string,
    patch: Partial<VoxelSpriteEnhancementConfig>,
  ): RendererThreeVoxelSpriteReceipt {
    if (this.#disposed) return this.#rejected('disposed', 'voxel sprite scene is disposed');
    const entry = this.#entries.get(id);
    if (entry === undefined) return this.#rejected('unknown_id', `unknown voxel sprite id ${id}`);
    try {
      entry.enhancement.configure(patch);
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
    const receipt = this.#captureRetained(entry.runtimeCapture, entry.retainedObject, captureSettings);
    if (!receipt.applied || receipt.frame === null) {
      entry.fallbackPreservedCount += 1;
      return this.#rejected('capture_failed', receipt.diagnostics[0]?.message ?? 'runtime recapture failed');
    }
    entry.enhancement.replaceSource({
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
    for (const entry of this.#entries.values()) entry.enhancement.faceCamera(camera);
  }

  recordCpuSubmission(milliseconds: number): void {
    if (this.#disposed) return;
    for (const entry of this.#entries.values()) {
      entry.enhancement.recordSteadyStateFrame(milliseconds);
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
          enhancement: entry.enhancement.readout(),
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
          ...definition.config,
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
      frame,
      source: definition.source,
      runtimeCapture,
      retainedObject,
      retainedOriginalVisibility,
      captureSettings: definition.source.kind === 'retained' ? definition.source.capture : null,
      fallbackPreservedCount: 0,
    };
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
    this.#backend.scene.traverse((object) => {
      if (isRenderable(object)) visibility.set(object, object.visible);
    });
    const sourceVisibility = source.visible;
    try {
      for (const object of visibility.keys()) {
        if (!isDescendantOrSelf(object, source)) object.visible = false;
      }
      source.visible = true;
      source.updateWorldMatrix(true, true);
      const bounds = new THREE.Box3().setFromObject(source, true);
      if (bounds.isEmpty()) throw new CaptureSourceError('retained source bounds are empty');
      const center = bounds.getCenter(new THREE.Vector3());
      const size = bounds.getSize(new THREE.Vector3());
      const camera = captureCamera(settings, center, size);
      return capture.capture({
        scene: this.#backend.scene,
        camera,
        width: settings.resolution,
        height: settings.resolution,
        bounds,
      });
    } finally {
      for (const [object, visible] of visibility) object.visible = visible;
      source.visible = sourceVisibility;
    }
  }

  #disposeEntry(entry: Entry): void {
    this.#backend.scene.remove(entry.enhancement.object);
    entry.enhancement.dispose();
    entry.runtimeCapture?.dispose();
    if (entry.runtimeCapture === null) entry.frame.dispose();
    if (entry.retainedObject !== null && entry.retainedOriginalVisibility !== null) {
      entry.retainedObject.visible = entry.retainedOriginalVisibility;
    }
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
  if (input.source.kind === 'retained') validatedCapture(input.source.capture);
  return input;
}

function validatedCapture(input: RendererThreeVoxelSpriteCaptureSettings): RendererThreeVoxelSpriteCaptureSettings {
  if (!Number.isInteger(input.resolution) || input.resolution < 8 || input.resolution > 1024) {
    throw new RangeError('capture resolution must be an integer from 8 to 1024');
  }
  bounded(input.azimuthDegrees, -360, 360, 'capture azimuth');
  bounded(input.elevationDegrees, -89, 89, 'capture elevation');
  bounded(input.near, 0.001, 100, 'capture near');
  bounded(input.far, input.near + 0.001, 10_000, 'capture far');
  return Object.freeze({ ...input });
}

function captureCamera(
  settings: RendererThreeVoxelSpriteCaptureSettings,
  center: THREE.Vector3,
  size: THREE.Vector3,
): THREE.PerspectiveCamera {
  const camera = new THREE.PerspectiveCamera(35, 1, settings.near, settings.far);
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
