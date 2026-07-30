// Retained Three.js scene projector for Rusty Engine render diffs.

import * as THREE from 'three';
import { decodeRenderFrameDiff } from '@rusty-engine/render-contracts';
import {
  RenderProjection,
  RenderProjectionError,
} from '@rusty-engine/render-projection';
import type {
  Geometry,
  LightDescriptor,
  Material,
  MaterialInstanceParameters,
  MeshCollisionPolicy,
  MeshMaterialSlot,
  MeshPickHit,
  MeshPayloadDescriptor,
  MeshProvenance,
  RenderDiff,
  RenderFrameDiff,
  RenderHandle,
  RenderLayer,
  RenderMaterialDescriptor,
  RenderMetadata,
  RenderNode,
  SpriteAtlasDescriptor,
  SpriteInstanceDescriptor,
  SpritePickHit,
  StaticMeshAsset,
  TextureDescriptor,
  Transform,
  VoxelObjectInstanceDescriptor,
  VoxelObjectRenderAsset,
} from '@rusty-engine/render-contracts';
import {
  AnimatedMeshApplyError,
  AnimatedMeshRegistry,
  type AnimatedMeshAssetSource,
  type AnimatedMeshControllerClip,
  type AnimatedMeshPlaybackReadout,
} from './animated-mesh.js';
import {
  applyLightDescriptor,
  buildLight,
  disposeLight,
  lightShadowStatus,
  projectionParentHandle,
  validateLightDescriptor,
  type RendererLightReadout,
} from './lighting.js';
export type {
  RendererLightReadout,
  RendererLightShadowStatus,
} from './lighting.js';
import {
  MaterialFallback,
  meshMaterials,
  type RendererMeshPresentationReadout,
} from './mesh-presentation.js';

/** Raised when a diff cannot be applied (duplicate, unknown, or stale handle). */
export class RenderApplyError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'RenderApplyError';
  }
}

/**
 * The capability the renderer needs to upload a shared-buffer mesh payload.
 *
 * Lifetime semantics: **borrow → copy → release**. The renderer borrows the
 * provider-owned bytes with {@link acquireBuffer}, copies every declared stream out into
 * fresh, renderer-owned typed arrays, and then returns the borrow with
 * {@link releaseBuffer} — on both the success and the failure path. It never retains
 * the borrowed view, never mutates gameplay state, and never owns the provider's bytes.
 */
export interface MeshBufferView {
  readonly bytes: Uint8Array;
}

export interface MeshBufferSource {
  acquireBuffer(buffer: number): MeshBufferView;
  releaseBuffer(buffer: number): void;
}

/** Explicit provider for durable content-addressed mesh bytes. Resource ids
 * are renderer-neutral identities; providers own their location and cache. */
export interface MeshResourceSource {
  acquireResource(resource: string, contentHash: string, byteLength: number): MeshBufferView;
  releaseResource(resource: string): void;
}

export type RenderResourceErrorCode = 'missing' | 'expired' | 'invalid' | 'providerFailure';

/** Typed failure raised by an explicit renderer resource provider. */
export class RenderResourceError extends Error {
  constructor(
    readonly code: RenderResourceErrorCode,
    readonly resource: number | string,
    message: string,
  ) {
    super(message);
    this.name = 'RenderResourceError';
  }
}

type NodeKind = 'primitive' | 'staticMesh' | 'animatedMesh' | 'voxelObject' | 'sprite' | 'light';

interface NodeEntry {
  readonly object: THREE.Object3D;
  readonly kind: NodeKind;
  /** Primitive shape, for `kind === 'primitive'`. */
  readonly shape: Geometry['kind'];
  /** Source asset id, for static mesh instances and sprites. */
  readonly asset?: string;
  /** Whether destroying this node may dispose its geometry (false = shared). */
  readonly ownsGeometry: boolean;
  /** The full sprite descriptor, for `kind === 'sprite'` (frame/tint/pick). */
  sprite?: SpriteInstanceDescriptor;
  /**
   * The authority provenance of this node's uploaded mesh payload (set on
   * `replaceMeshPayload`), so a renderer mesh pick can trace the handle back to its
   * authority source. Absent until a payload is uploaded.
   */
  meshProvenance?: MeshProvenance;
  /** Retained projection style applied to primitive and uploaded voxel meshes. */
  viewMaterial?: Material;
  /** Material slot corresponding to each uploaded mesh material. */
  meshMaterialSlots?: number[];
  /** Complete generic descriptor for `kind === 'light'`. */
  light?: LightDescriptor;
  /**
   * Defined material id behind each entry of a static-mesh instance's material
   * array (parallel to `mesh.material`), so a live `defineMaterial` redefine can
   * find and replace exactly the affected materials. `null` = unmanaged.
   */
  materialIds?: (string | null)[];
  /** Material-array indices whose material object belongs only to this instance. */
  ownedMaterialIndices?: Set<number>;
  /** Complete per-slot feedback overrides, keyed by material-array index. */
  materialParameterOverrides?: Map<number, MaterialInstanceParameters>;
  /** Durable descriptor-side selection; renderer clocks never own voxel playback. */
  voxelFrame?: number;
  voxelMaterialOverrides?: readonly MeshMaterialSlot[];
}

export interface RendererProjectionIdentity {
  readonly handle: RenderHandle;
  readonly layer: RenderLayer;
  readonly metadata: RenderMetadata;
}

export interface RendererVoxelObjectFrameReadout {
  readonly handle: RenderHandle;
  readonly asset: string;
  readonly frame: number;
  readonly frameId: string;
  readonly mesh: number;
}

/** Exact renderer-owned retained resources; no gameplay or GPU-completion meaning. */
export interface ThreeRendererResourceStatistics {
  readonly renderHandleCount: number;
  readonly geometryResourceCount: number;
  readonly materialResourceCount: number;
  readonly textureResourceCount: number;
  readonly animatedInstanceCount: number;
}

/** A retained static mesh definition: shared resources plus a live-instance count. */
interface StaticMeshDef {
  readonly geometry: THREE.BufferGeometry;
  readonly materials: THREE.Material[];
  /** material slot index → position in `materials`. */
  readonly slotIndex: Map<number, number>;
  readonly materialSlots: readonly MeshMaterialSlot[];
  readonly collision: MeshCollisionPolicy;
  refCount: number;
}

interface VoxelObjectDef {
  readonly geometries: THREE.BufferGeometry[];
  readonly frames: VoxelObjectRenderAsset['frames'];
  readonly meshMaterialSlots: readonly (readonly number[])[];
  readonly materials: THREE.Material[];
  readonly slotIndex: Map<number, number>;
  readonly materialSlots: readonly MeshMaterialSlot[];
  refCount: number;
}

/**
 * A retained Three.js scene driven entirely by render diffs.
 *
 * Nodes are addressed by `RenderHandle`; the registry maps each handle to a
 * Three.js `Object3D`. World layers share `scene`; camera-relative presentation
 * is retained in `viewmodelScene` for an explicit after-depth host pass.
 */
export class ThreeRenderer {
  readonly scene = new THREE.Scene();
  readonly viewmodelScene = new THREE.Scene();
  readonly #sceneGroup = new THREE.Group();
  readonly #debugGroup = new THREE.Group();
  readonly #uiGroup = new THREE.Group();
  readonly #viewmodelGroup = new THREE.Group();
  readonly #handles = new Map<RenderHandle, NodeEntry>();
  /** Retained static mesh definitions, keyed by asset id. */
  readonly #staticMeshes = new Map<string, StaticMeshDef>();
  readonly #voxelObjects = new Map<string, VoxelObjectDef>();
  /** Per-material-slot colours for the initial flat/debug material strategy. */
  readonly #slotColors = new Map<number, THREE.Color>();
  /** Material descriptors defined by retained operations, keyed by asset id. */
  readonly #materials = new Map<string, RenderMaterialDescriptor>();
  /** How many times a slot fell back to a placeholder (no defined descriptor). */
  #fallbackMaterialCount = 0;
  /** Material ids that fell back to a placeholder (fallback diagnostic). */
  readonly #fallbackMaterials = new Set<string>();
  /** Texture descriptors, keyed by texture asset id. */
  readonly #textures = new Map<string, TextureDescriptor>();
  /** Sprite atlas descriptors, keyed by sprite-sheet asset id. */
  readonly #atlases = new Map<string, SpriteAtlasDescriptor>();
  /** How many times a sprite frame fell back to full UVs (no atlas/frame). */
  #spriteFallbackCount = 0;
  /**
   * Optional resource source for shared-buffer mesh payloads. When absent,
   * shared-buffer sources fail closed (the inline fixture path still works for goldens).
   */
  readonly #meshBufferSource: MeshBufferSource | undefined;
  readonly #meshResourceSource: MeshResourceSource | undefined;
  readonly #animatedMeshSource: AnimatedMeshAssetSource | undefined;
  readonly #animatedMeshes: AnimatedMeshRegistry;
  readonly #shadowsEnabled: boolean;
  readonly #projection = new RenderProjection();
  readonly #geometryResources = new Set<THREE.BufferGeometry>();
  readonly #materialResources = new Set<THREE.Material>();
  readonly #textureResourceReferences = new Map<THREE.Texture, number>();
  #disposed = false;

  constructor(options: {
    meshBufferSource?: MeshBufferSource;
    meshResourceSource?: MeshResourceSource;
    animatedMeshSource?: AnimatedMeshAssetSource;
    shadowsEnabled?: boolean;
  } = {}) {
    this.#meshBufferSource = options.meshBufferSource;
    this.#meshResourceSource = options.meshResourceSource;
    this.#animatedMeshSource = options.animatedMeshSource;
    this.#animatedMeshes = new AnimatedMeshRegistry(this.#animatedMeshSource);
    this.#shadowsEnabled = options.shadowsEnabled ?? false;
    this.#sceneGroup.name = 'scene';
    this.#debugGroup.name = 'debug';
    this.#uiGroup.name = 'ui';
    this.#viewmodelGroup.name = 'viewmodel';
    this.viewmodelScene.name = 'viewmodel';
    this.scene.add(this.#sceneGroup, this.#debugGroup, this.#uiGroup);
    this.viewmodelScene.add(this.#viewmodelGroup);
  }

  #layerGroup(layer: RenderLayer): THREE.Group {
    switch (layer) {
      case 'scene': return this.#sceneGroup;
      case 'debug': return this.#debugGroup;
      case 'ui': return this.#uiGroup;
      case 'viewmodel': return this.#viewmodelGroup;
    }
  }

  /**
   * Apply a whole frame of diffs in order.
   *
   * The complete retained transition and every fallible mesh/animated resource
   * are preflighted before the first Three object is mutated. A rejected later
   * operation therefore leaves handles, resources, and scene objects unchanged.
   */
  applyFrame(frame: RenderFrameDiff): void {
    if (this.#disposed) {
      throw new RenderApplyError('renderer is disposed');
    }
    try {
      this.#projection.validateFrame(frame);
    } catch (cause) {
      if (cause instanceof RenderProjectionError) {
        throw new RenderApplyError(cause.message);
      }
      throw cause;
    }
    const preparedGeometry = this.#prepareFrame(frame);
    const recursivelyDestroyed = new Set<RenderHandle>();
    try {
      for (let index = 0; index < frame.ops.length; index += 1) {
        const op = frame.ops[index]!;
        if (op.op === 'destroy') {
          if (!this.#handles.has(op.handle) && recursivelyDestroyed.has(op.handle)) {
            continue;
          }
          this.#destroy(op, recursivelyDestroyed);
        } else {
          this.#applyDiff(op, preparedGeometry.get(index));
          preparedGeometry.delete(index);
        }
      }
    } catch (cause) {
      disposePreparedGeometry(preparedGeometry);
      throw cause;
    }
    disposePreparedGeometry(preparedGeometry);
    this.#projection.applyFrame(frame);
  }

  /** Strictly decode a versioned contract payload and apply it. */
  applyEncodedFrame(payload: unknown): void {
    this.applyFrame(decodeRenderFrameDiff(payload));
  }

  /** Apply a single diff. Throws `RenderApplyError` on a bad handle. */
  applyDiff(diff: RenderDiff): void {
    this.applyFrame({ schemaVersion: 1, ops: [diff] });
  }

  #applyDiff(diff: RenderDiff, preparedGeometry?: readonly THREE.BufferGeometry[]): void {
    switch (diff.op) {
      case 'create':
        this.#create(diff);
        break;
      case 'update':
        this.#update(diff);
        break;
      case 'destroy':
        this.#destroy(diff);
        break;
      case 'replaceMeshPayload':
        this.#replaceMeshPayload(diff, preparedGeometry?.[0]);
        break;
      case 'createLight':
        this.#createLight(diff);
        break;
      case 'updateLight':
        this.#updateLight(diff);
        break;
      case 'defineMaterial':
        this.#defineMaterial(diff.material);
        break;
      case 'setMaterialInstanceParameters':
        this.#setMaterialInstanceParameters(diff);
        break;
      case 'defineTexture':
        this.#textures.set(diff.texture.id, diff.texture);
        break;
      case 'defineSpriteAtlas':
        this.#atlases.set(diff.atlas.id, diff.atlas);
        break;
      case 'defineStaticMesh':
        this.#defineStaticMesh(diff.asset, preparedGeometry?.[0]);
        break;
      case 'defineAnimatedMesh':
        this.#defineAnimatedMesh(diff);
        break;
      case 'createAnimatedMeshInstance':
        this.#createAnimatedMeshInstance(diff);
        break;
      case 'setAnimatedMeshPlayback':
        this.#setAnimatedMeshPlayback(diff);
        break;
      case 'defineVoxelObject':
        this.#defineVoxelObject(diff.asset, preparedGeometry);
        break;
      case 'releaseVoxelObject':
        this.#releaseVoxelObject(diff.asset);
        break;
      case 'createVoxelObjectInstance':
        this.#createVoxelObjectInstance(diff);
        break;
      case 'setVoxelObjectFrame':
        this.#setVoxelObjectFrame(diff);
        break;
      case 'createStaticMeshInstance':
        this.#createStaticMeshInstance(diff);
        break;
      case 'createSprite':
        this.#createSprite(diff);
        break;
      case 'updateSprite':
        this.#updateSprite(diff);
        break;
    }
  }

  #prepareFrame(frame: RenderFrameDiff): Map<number, readonly THREE.BufferGeometry[]> {
    const prepared = new Map<number, readonly THREE.BufferGeometry[]>();
    const selectedAnimatedClips = new Map<RenderHandle, string | null>();
    try {
      for (let index = 0; index < frame.ops.length; index += 1) {
        const operation = frame.ops[index]!;
        if (operation.op === 'defineStaticMesh') {
          prepared.set(index, [buildMeshGeometry(
            operation.asset.payload,
            this.#meshBufferSource,
            this.#meshResourceSource,
            'defineStaticMesh',
          )]);
        } else if (operation.op === 'replaceMeshPayload') {
          prepared.set(index, [buildMeshGeometry(
            operation.payload,
            this.#meshBufferSource,
            this.#meshResourceSource,
            'replaceMeshPayload',
          )]);
        } else if (operation.op === 'defineVoxelObject') {
          prepared.set(index, buildVoxelObjectGeometries(
            operation.asset,
            this.#meshBufferSource,
            this.#meshResourceSource,
          ));
        } else if (operation.op === 'defineAnimatedMesh') {
          // Definition performs all source/hash/clip checks without creating a
          // live instance, so it is safe to run before the retained mutation.
          new AnimatedMeshRegistry(this.#animatedMeshSource).define(operation.asset);
        } else if (
          operation.op === 'createAnimatedMeshInstance'
          && operation.instance.materialOverrides.length > 0
        ) {
          throw new RenderApplyError(
            `createAnimatedMeshInstance: material overrides are not implemented for animated mesh ${operation.instance.asset}`,
          );
        } else if (operation.op === 'createAnimatedMeshInstance') {
          const playback = operation.instance.playback;
          if (playback?.kind === 'pause' || playback?.kind === 'resume') {
            throw new RenderApplyError(
              `createAnimatedMeshInstance.${playback.kind}: no current clip on ${operation.instance.asset}`,
            );
          }
          selectedAnimatedClips.set(
            operation.handle,
            playback?.kind === 'play' ? playback.clip : null,
          );
        } else if (operation.op === 'setAnimatedMeshPlayback') {
          const currentClip = selectedAnimatedClips.has(operation.handle)
            ? selectedAnimatedClips.get(operation.handle) ?? null
            : this.#animatedMeshes.playback(operation.handle)?.currentClip ?? null;
          if (
            (operation.playback.kind === 'pause' || operation.playback.kind === 'resume')
            && currentClip === null
          ) {
            throw new RenderApplyError(
              `setAnimatedMeshPlayback.${operation.playback.kind}: no current clip`,
            );
          }
          if (operation.playback.kind === 'play') {
            selectedAnimatedClips.set(operation.handle, operation.playback.clip);
          } else if (operation.playback.kind === 'stop') {
            selectedAnimatedClips.set(operation.handle, null);
          }
        }
      }
      return prepared;
    } catch (cause) {
      disposePreparedGeometry(prepared);
      throw animatedMeshError(cause);
    }
  }

  /**
   * Register the flat colour used for a material slot (the initial flat/debug
   * material strategy. Unregistered slots fall back to a deterministic
   * per-slot colour, so a payload always maps to *some* visible material.
   */
  registerSlotColor(slot: number, r: number, g: number, b: number): void {
    this.#slotColors.set(slot, new THREE.Color(r, g, b));
  }

  #slotColor(slot: number): THREE.Color {
    const registered = this.#slotColors.get(slot);
    if (registered) {
      return registered.clone();
    }
    // Deterministic fallback hue per slot (golden angle), so missing slots are
    // visible and stable rather than silently skipped.
    const hue = (slot * 0.61803398875) % 1;
    return new THREE.Color().setHSL(hue, 0.7, 0.5);
  }

  /** Whether a handle is currently live in the scene. */
  has(handle: RenderHandle): boolean {
    return this.#handles.has(handle);
  }

  /** Number of live scene handles. */
  get handleCount(): number {
    return this.#handles.size;
  }

  /** Constant-time immutable readout cached after each accepted retained mutation. */
  resourceStatistics(): ThreeRendererResourceStatistics {
    return Object.freeze({
      renderHandleCount: this.#handles.size,
      geometryResourceCount: this.#geometryResources.size,
      materialResourceCount: this.#materialResources.size,
      textureResourceCount: this.#textureResourceReferences.size,
      animatedInstanceCount: this.#animatedMeshes.instanceCount,
    });
  }

  #trackObjectResources(root: THREE.Object3D): void {
    root.traverse((object) => {
      const resource = object as Partial<{
        geometry: THREE.BufferGeometry;
        material: THREE.Material | THREE.Material[];
      }>;
      if (resource.geometry instanceof THREE.BufferGeometry) {
        this.#trackGeometryResource(resource.geometry);
      }
      if (Array.isArray(resource.material)) {
        resource.material.forEach((material) => this.#trackMaterialResource(material));
      } else if (resource.material instanceof THREE.Material) {
        this.#trackMaterialResource(resource.material);
      }
    });
  }

  #trackGeometryResource(geometry: THREE.BufferGeometry): void {
    if (this.#geometryResources.has(geometry)) return;
    this.#geometryResources.add(geometry);
    geometry.addEventListener('dispose', () => this.#geometryResources.delete(geometry));
  }

  #trackMaterialResource(material: THREE.Material): void {
    if (this.#materialResources.has(material)) return;
    this.#materialResources.add(material);
    const textures = materialTextures(material);
    for (const texture of textures) {
      const references = this.#textureResourceReferences.get(texture) ?? 0;
      if (references === 0) {
        texture.addEventListener('dispose', () => this.#textureResourceReferences.delete(texture));
      }
      this.#textureResourceReferences.set(texture, references + 1);
    }
    material.addEventListener('dispose', () => {
      if (!this.#materialResources.delete(material)) return;
      for (const texture of textures) {
        const references = this.#textureResourceReferences.get(texture);
        if (references === undefined || references <= 1) {
          this.#textureResourceReferences.delete(texture);
        } else {
          this.#textureResourceReferences.set(texture, references - 1);
        }
      }
    });
  }

  /** Renderer-local diagnostics/readback; never authority. */
  lightReadout(): readonly RendererLightReadout[] {
    return [...this.#handles.entries()]
      .filter((entry): entry is [RenderHandle, NodeEntry & { light: LightDescriptor }] =>
        entry[1].kind === 'light' && entry[1].light !== undefined)
      .sort(([left], [right]) => left - right)
      .map(([handle, entry]) => ({
        descriptor: structuredClone(entry.light),
        handle,
        parent: projectionParentHandle(entry.object.parent, this.#handles),
        shadowStatus: lightShadowStatus(entry.light, this.#shadowsEnabled),
      }));
  }

  /** Lit/wireframe state for uploaded mesh payloads, for Studio diagnostics. */
  meshPresentationReadout(): readonly RendererMeshPresentationReadout[] {
    return [...this.#handles.entries()]
      .filter(([, entry]) => entry.meshProvenance !== undefined)
      .sort(([left], [right]) => left - right)
      .map(([handle, entry]) => ({
        handle,
        lit: meshMaterials(entry.object).every((material) => material instanceof THREE.MeshStandardMaterial),
        materialSlots: [...(entry.meshMaterialSlots ?? [])],
        opacity: entry.viewMaterial?.color[3] ?? 1,
        wireframe: entry.viewMaterial?.wireframe ?? false,
      }));
  }

  /** Release every retained renderer object and GPU-owned resource. */
  dispose(): void {
    if (this.#disposed) return;
    const handlesByDepth = [...this.#handles.entries()]
      .sort((left, right) => objectDepth(right[1].object) - objectDepth(left[1].object))
      .map(([handle]) => handle);
    for (const handle of handlesByDepth) {
      if (this.#handles.has(handle)) {
        this.#destroy({ op: 'destroy', handle });
      }
    }
    for (const definition of this.#staticMeshes.values()) {
      definition.geometry.dispose();
      definition.materials.forEach((material) => material.dispose());
    }
    this.#staticMeshes.clear();
    for (const definition of this.#voxelObjects.values()) {
      definition.geometries.forEach((geometry) => geometry.dispose());
      definition.materials.forEach((material) => material.dispose());
    }
    this.#voxelObjects.clear();
    this.#animatedMeshes.dispose();
    this.#slotColors.clear();
    this.#materials.clear();
    this.#fallbackMaterials.clear();
    this.#textures.clear();
    this.#atlases.clear();
    this.scene.clear();
    this.viewmodelScene.clear();
    this.#geometryResources.clear();
    this.#materialResources.clear();
    this.#textureResourceReferences.clear();
    this.#disposed = true;
  }

  /** The Three.js object for a handle, for inspection/tests. */
  objectFor(handle: RenderHandle): THREE.Object3D | undefined {
    return this.#handles.get(handle)?.object;
  }

  /**
   * Resolve a renderer object (or one of its backend-owned descendants) to the
   * retained projection identity that created it. This is disposable picking
   * evidence only: callers receive generated handle/metadata values and no
   * mutable Three.js object or authority capability.
   */
  projectionIdentityForObject(object: THREE.Object3D): RendererProjectionIdentity | undefined {
    let candidate: THREE.Object3D | null = object;
    while (candidate !== null) {
      for (const [handle, entry] of this.#handles.entries()) {
        if (entry.object !== candidate) {
          continue;
        }
        return {
          handle,
          layer: this.#layerForObject(entry.object),
          metadata: readMetadata(entry.object),
        };
      }
      candidate = candidate.parent;
    }
    return undefined;
  }

  /** Advance projection-only animation mixers by an explicit renderer frame delta. */
  advanceAnimation(deltaSeconds: number): void {
    try {
      this.#animatedMeshes.advance(deltaSeconds);
    } catch (cause) {
      throw animatedMeshError(cause);
    }
    for (const [handle, entry] of this.#handles.entries()) {
      if (entry.kind === 'animatedMesh') {
        this.#syncAnimatedMeshPlayback(handle, entry);
      }
    }
  }

  /** Projection/debug readback for animated mesh playback; never authority. */
  animatedMeshPlayback(handle: RenderHandle): AnimatedMeshPlaybackReadout | undefined {
    return this.#animatedMeshes.playback(handle);
  }

  /** Apply renderer-local clip weights resolved from an authority controller projection. */
  setAnimationControllerWeights(
    handle: RenderHandle,
    clips: readonly AnimatedMeshControllerClip[],
  ): void {
    try {
      this.#animatedMeshes.setControllerWeights(handle, clips);
      this.#syncAnimatedMeshPlayback(handle, this.#require(handle, 'setAnimationControllerWeights'));
    } catch (cause) {
      throw animatedMeshError(cause);
    }
  }

  hasAnimationControllerClips(handle: RenderHandle, clipIds: readonly string[]): boolean {
    return this.#animatedMeshes.hasClips(handle, clipIds);
  }

  clearAnimationControllerWeights(handle: RenderHandle): void {
    try {
      this.#animatedMeshes.clearControllerWeights(handle);
      this.#syncAnimatedMeshPlayback(handle, this.#require(handle, 'clearAnimationControllerWeights'));
    } catch (cause) {
      throw animatedMeshError(cause);
    }
  }

  /**
   * A deterministic textual snapshot of the rendered scene — one line per live
   * handle (sorted), capturing layer, shape, transform, visibility, and colour.
   *
   * This is the "render artifact" the golden check diffs. It is a structural
   * snapshot rather than a pixel screenshot: GPU pixel output is
   * non-deterministic across drivers and headless GL is a heavy native
   * dependency, whereas this is exact, reviewable, and needs no GL context.
   */
  snapshot(): string {
    const entries = [...this.#handles.entries()].sort((a, b) => a[0] - b[0]);
    if (entries.length === 0) {
      return '(empty scene)\n';
    }
    return entries
      .map(([handle, entry]) => snapshotLine(
        handle,
        entry,
        this.#layerForObject(entry.object),
      ))
      .join('\n') + '\n';
  }

  #create(diff: Extract<RenderDiff, { op: 'create' }>): void {
    if (this.#handles.has(diff.handle)) {
      throw new RenderApplyError(`create: handle ${diff.handle} already exists`);
    }
    const object = buildObject(diff.node);
    this.#trackObjectResources(object);
    const parent =
      diff.parent === null
        ? this.#layerGroup(diff.node.layer)
        : this.#require(diff.parent, 'create.parent').object;
    parent.add(object);
    this.#handles.set(diff.handle, {
      object,
      kind: 'primitive',
      shape: diff.node.geometry.kind,
      ownsGeometry: diff.node.geometry.kind !== 'group',
      viewMaterial: diff.node.material,
    });
  }

  #layerForObject(object: THREE.Object3D): RenderLayer {
    if (isDescendantOf(object, this.#viewmodelGroup)) {
      return 'viewmodel';
    }
    if (isDescendantOf(object, this.#debugGroup)) {
      return 'debug';
    }
    if (isDescendantOf(object, this.#uiGroup)) {
      return 'ui';
    }
    return 'scene';
  }

  #update(diff: Extract<RenderDiff, { op: 'update' }>): void {
    const entry = this.#require(diff.handle, 'update');
    if (diff.transform) {
      applyTransform(entry.object, diff.transform);
    }
    if (diff.material) {
      if (entry.meshProvenance !== undefined) {
        this.#applyUploadedMeshMaterial(entry, diff.material);
      } else {
        applyMaterial(entry, diff.material);
      }
      entry.viewMaterial = diff.material;
      this.#trackObjectResources(entry.object);
    }
    if (diff.visible !== null) {
      entry.object.visible = diff.visible;
    }
    if (diff.metadata) {
      applyMetadata(entry.object, diff.metadata);
    }
  }

  #destroy(
    diff: Extract<RenderDiff, { op: 'destroy' }>,
    recursivelyDestroyed?: Set<RenderHandle>,
  ): void {
    const entry = this.#require(diff.handle, 'destroy');
    const childHandles = [...this.#handles.entries()]
      .filter(([, candidate]) => candidate.object.parent === entry.object)
      .map(([handle]) => handle)
      .sort((left, right) => left - right);
    for (const child of childHandles) {
      this.#destroy({ op: 'destroy', handle: child }, recursivelyDestroyed);
    }
    entry.object.parent?.remove(entry.object);
    if (entry.kind === 'staticMesh' && entry.asset !== undefined) {
      // Shared definitions outlive their instances. Destroy only this instance's
      // override materials and release its live-instance count; a later retained
      // create may reuse the definition without a second define.
      disposeInstanceMaterials(entry);
      this.#releaseStaticMesh(entry.asset);
    } else if (entry.kind === 'animatedMesh') {
      this.#animatedMeshes.release(diff.handle);
      disposeObjectRecursive(entry.object);
    } else if (entry.kind === 'voxelObject' && entry.asset !== undefined) {
      disposeInstanceMaterials(entry);
      const definition = this.#voxelObjects.get(entry.asset);
      if (definition !== undefined) definition.refCount -= 1;
    } else if (entry.kind === 'light') {
      disposeLight(entry.object);
    } else {
      disposeObject(entry.object);
    }
    this.#handles.delete(diff.handle);
    recursivelyDestroyed?.add(diff.handle);
  }

  // ── Static mesh assets + instances (render-asset-04) ────────────────────────

  /**
   * Define (or redefine) a static mesh asset's shared geometry + slot materials.
   * Idempotent per asset id: a redefine while instances exist is rejected (it
   * would orphan shared geometry); a redefine of an unused asset replaces it.
   */
  #defineStaticMesh(asset: StaticMeshAsset, preparedGeometry?: THREE.BufferGeometry): void {
    const existing = this.#staticMeshes.get(asset.asset);
    if (existing) {
      if (existing.refCount > 0) {
        throw new RenderApplyError(
          `defineStaticMesh: asset ${asset.asset} is in use by ${existing.refCount} instance(s)`,
        );
      }
      existing.geometry.dispose();
      existing.materials.forEach((m) => m.dispose());
    }
    // Inline and shared-buffer payloads both upload here: a shared-buffer
    // static mesh asset borrows the provider buffer, copies its bytes out, and
    // releases the borrow. A missing provider / unknown / stale / too-small buffer
    // fails closed below — never silently producing empty geometry.
    const geometry = preparedGeometry ?? buildMeshGeometry(
      asset.payload,
      this.#meshBufferSource,
      this.#meshResourceSource,
      'defineStaticMesh',
    );
    this.#trackGeometryResource(geometry);
    const slotIndex = new Map<number, number>();
    const materials = asset.materialSlots.map((s, i) => {
      slotIndex.set(s.slot, i);
      return this.#materialFor(s);
    });
    this.#staticMeshes.set(asset.asset, {
      geometry,
      materials,
      slotIndex,
      materialSlots: asset.materialSlots,
      collision: asset.collision,
      refCount: 0,
    });
  }

  #createStaticMeshInstance(diff: Extract<RenderDiff, { op: 'createStaticMeshInstance' }>): void {
    if (this.#handles.has(diff.handle)) {
      throw new RenderApplyError(`createStaticMeshInstance: handle ${diff.handle} already exists`);
    }
    const def = this.#staticMeshes.get(diff.instance.asset);
    if (!def) {
      throw new RenderApplyError(
        `createStaticMeshInstance: undefined static mesh asset ${diff.instance.asset}`,
      );
    }
    // Materials default to the asset's; per-instance overrides clone-replace just
    // the named slots, so two instances of one asset can differ in material while
    // sharing one BufferGeometry.
    const materials = def.materials.slice();
    // Defined material id behind each material-array entry (for live redefine).
    const materialIds: (string | null)[] = def.materialSlots.map((s) => s.material);
    const ownedMaterialIndices = new Set<number>();
    for (const ov of diff.instance.materialOverrides) {
      const idx = def.slotIndex.get(ov.slot);
      if (idx === undefined) {
        throw new RenderApplyError(
          `createStaticMeshInstance: override for unbound slot ${ov.slot} on ${diff.instance.asset}`,
        );
      }
      const m = this.#materialFor(ov);
      materials[idx] = m;
      materialIds[idx] = ov.material;
      ownedMaterialIndices.add(idx);
    }
    const mesh = new THREE.Mesh(def.geometry, materials.length === 1 ? materials[0]! : materials);
    applyTransform(mesh, diff.instance.transform);
    applyMetadata(mesh, diff.instance.metadata);
    mesh.visible = diff.instance.visible;

    const parent =
      diff.parent === null ? this.#sceneGroup : this.#require(diff.parent, 'createStaticMeshInstance.parent').object;
    parent.add(mesh);
    def.refCount += 1;
    this.#handles.set(diff.handle, {
      object: mesh,
      kind: 'staticMesh',
      shape: 'quad',
      asset: diff.instance.asset,
      ownsGeometry: false,
      materialIds,
      ownedMaterialIndices,
      materialParameterOverrides: new Map(),
    });
  }

  #releaseStaticMesh(asset: string): void {
    const def = this.#staticMeshes.get(asset);
    if (!def) {
      return;
    }
    def.refCount -= 1;
  }

  // ── Animated mesh assets + named playback (projection-only) ────────────────

  #defineAnimatedMesh(diff: Extract<RenderDiff, { op: 'defineAnimatedMesh' }>): void {
    try {
      this.#animatedMeshes.define(diff.asset);
    } catch (cause) {
      throw animatedMeshError(cause);
    }
  }

  #createAnimatedMeshInstance(diff: Extract<RenderDiff, { op: 'createAnimatedMeshInstance' }>): void {
    if (this.#handles.has(diff.handle)) {
      throw new RenderApplyError(`createAnimatedMeshInstance: handle ${diff.handle} already exists`);
    }
    let record: { readonly object: THREE.Object3D };
    try {
      record = this.#animatedMeshes.create(diff.handle, diff.instance);
    } catch (cause) {
      throw animatedMeshError(cause);
    }
    applyTransform(record.object, diff.instance.transform);
    applyMetadata(record.object, diff.instance.metadata);
    record.object.visible = diff.instance.visible;
    this.#trackObjectResources(record.object);
    const parent =
      diff.parent === null ? this.#sceneGroup : this.#require(diff.parent, 'createAnimatedMeshInstance.parent').object;
    parent.add(record.object);
    this.#handles.set(diff.handle, {
      object: record.object,
      kind: 'animatedMesh',
      shape: 'quad',
      asset: diff.instance.asset,
      ownsGeometry: true,
    });
    this.#syncAnimatedMeshPlayback(diff.handle, this.#require(diff.handle, 'createAnimatedMeshInstance'));
  }

  #setAnimatedMeshPlayback(diff: Extract<RenderDiff, { op: 'setAnimatedMeshPlayback' }>): void {
    const entry = this.#require(diff.handle, 'setAnimatedMeshPlayback');
    try {
      this.#animatedMeshes.setPlayback(diff.handle, diff.playback);
    } catch (cause) {
      throw animatedMeshError(cause);
    }
    this.#syncAnimatedMeshPlayback(diff.handle, entry);
  }

  #syncAnimatedMeshPlayback(handle: RenderHandle, entry: NodeEntry): void {
    entry.object.userData['animatedMeshPlayback'] = this.#animatedMeshes.playback(handle);
  }

  // ── Voxel-object resources + caller-driven frame swaps ────────────────────

  #defineVoxelObject(
    asset: VoxelObjectRenderAsset,
    preparedGeometries?: readonly THREE.BufferGeometry[],
  ): void {
    const geometries = preparedGeometries === undefined
      ? buildVoxelObjectGeometries(asset, this.#meshBufferSource, this.#meshResourceSource)
      : [...preparedGeometries];
    if (geometries.length !== asset.meshes.length) {
      throw new RenderApplyError(
        `defineVoxelObject: prepared ${geometries.length} meshes for ${asset.meshes.length} descriptors`,
      );
    }
    const slotIndex = new Map<number, number>();
    const materials = asset.materialSlots.map((slot, index) => {
      slotIndex.set(slot.slot, index);
      return this.#materialFor(slot);
    });
    geometries.forEach((geometry) => this.#trackGeometryResource(geometry));
    const existing = this.#voxelObjects.get(asset.asset);
    const next: VoxelObjectDef = {
      geometries,
      frames: asset.frames,
      meshMaterialSlots: asset.meshes.map((mesh) =>
        mesh.payload.groups.map((group) => group.materialSlot)),
      materials,
      slotIndex,
      materialSlots: asset.materialSlots,
      refCount: existing?.refCount ?? 0,
    };

    if (existing !== undefined) {
      for (const entry of this.#handles.values()) {
        if (entry.kind !== 'voxelObject' || entry.asset !== asset.asset) continue;
        const frame = entry.voxelFrame ?? 0;
        const descriptor = next.frames[frame];
        const geometry = descriptor === undefined ? undefined : next.geometries[descriptor.mesh];
        if (descriptor === undefined || geometry === undefined) {
          geometries.forEach((candidate) => candidate.dispose());
          materials.forEach((candidate) => candidate.dispose());
          throw new RenderApplyError(
            `defineVoxelObject: live frame ${frame} is unavailable on ${asset.asset}`,
          );
        }
        const instanceMaterials = this.#voxelObjectInstanceMaterials(
          next,
          entry.voxelMaterialOverrides ?? [],
        );
        disposeInstanceMaterials(entry);
        const mesh = entry.object as THREE.Mesh;
        mesh.geometry = geometry;
        mesh.material = instanceMaterials.materials.length === 1
          ? instanceMaterials.materials[0]!
          : instanceMaterials.materials;
        entry.materialIds = instanceMaterials.materialIds;
        entry.ownedMaterialIndices = instanceMaterials.ownedMaterialIndices;
        entry.meshMaterialSlots = asset.meshes[descriptor.mesh]!.payload.groups
          .map((group) => group.materialSlot);
      }
      existing.geometries.forEach((geometry) => geometry.dispose());
      existing.materials.forEach((material) => material.dispose());
    }
    this.#voxelObjects.set(asset.asset, next);
  }

  #releaseVoxelObject(asset: string): void {
    const definition = this.#voxelObjects.get(asset);
    if (definition === undefined) {
      throw new RenderApplyError(`releaseVoxelObject: undefined voxel object ${asset}`);
    }
    if (definition.refCount !== 0) {
      throw new RenderApplyError(
        `releaseVoxelObject: ${asset} is in use by ${definition.refCount} instance(s)`,
      );
    }
    definition.geometries.forEach((geometry) => geometry.dispose());
    definition.materials.forEach((material) => material.dispose());
    this.#voxelObjects.delete(asset);
  }

  #createVoxelObjectInstance(
    diff: Extract<RenderDiff, { op: 'createVoxelObjectInstance' }>,
  ): void {
    if (this.#handles.has(diff.handle)) {
      throw new RenderApplyError(
        `createVoxelObjectInstance: handle ${diff.handle} already exists`,
      );
    }
    const definition = this.#voxelObjects.get(diff.instance.asset);
    if (definition === undefined) {
      throw new RenderApplyError(
        `createVoxelObjectInstance: undefined voxel object ${diff.instance.asset}`,
      );
    }
    const frame = definition.frames[diff.instance.frame];
    const geometry = frame === undefined ? undefined : definition.geometries[frame.mesh];
    if (geometry === undefined) {
      throw new RenderApplyError(
        `createVoxelObjectInstance: frame ${diff.instance.frame} unavailable on ${diff.instance.asset}`,
      );
    }
    const instanceMaterials = this.#voxelObjectInstanceMaterials(
      definition,
      diff.instance.materialOverrides,
    );
    const mesh = new THREE.Mesh(
      geometry,
      instanceMaterials.materials.length === 1
        ? instanceMaterials.materials[0]!
        : instanceMaterials.materials,
    );
    applyTransform(mesh, diff.instance.transform);
    applyMetadata(mesh, diff.instance.metadata);
    mesh.visible = diff.instance.visible;
    const parent = diff.parent === null
      ? this.#sceneGroup
      : this.#require(diff.parent, 'createVoxelObjectInstance.parent').object;
    parent.add(mesh);
    definition.refCount += 1;
    this.#handles.set(diff.handle, {
      object: mesh,
      kind: 'voxelObject',
      shape: 'quad',
      asset: diff.instance.asset,
      ownsGeometry: false,
      materialIds: instanceMaterials.materialIds,
      ownedMaterialIndices: instanceMaterials.ownedMaterialIndices,
      meshProvenance: 'voxelObject',
      meshMaterialSlots: this.#voxelObjectMeshSlots(diff.instance.asset, diff.instance.frame),
      voxelFrame: diff.instance.frame,
      voxelMaterialOverrides: structuredClone(diff.instance.materialOverrides),
    });
  }

  #setVoxelObjectFrame(diff: Extract<RenderDiff, { op: 'setVoxelObjectFrame' }>): void {
    const entry = this.#require(diff.handle, 'setVoxelObjectFrame');
    if (entry.kind !== 'voxelObject' || entry.asset === undefined) {
      throw new RenderApplyError(`setVoxelObjectFrame: handle ${diff.handle} is not a voxel object`);
    }
    const definition = this.#voxelObjects.get(entry.asset);
    const frame = definition?.frames[diff.frame];
    const geometry = frame === undefined ? undefined : definition?.geometries[frame.mesh];
    if (definition === undefined || frame === undefined || geometry === undefined) {
      throw new RenderApplyError(
        `setVoxelObjectFrame: frame ${diff.frame} unavailable on ${entry.asset}`,
      );
    }
    (entry.object as THREE.Mesh).geometry = geometry;
    entry.voxelFrame = diff.frame;
    entry.meshMaterialSlots = this.#voxelObjectMeshSlots(entry.asset, diff.frame);
    entry.object.userData['voxelObjectFrame'] = diff.frame;
  }

  #voxelObjectMeshSlots(asset: string, frame: number): number[] {
    const definition = this.#voxelObjects.get(asset);
    const frameDescriptor = definition?.frames[frame];
    if (definition === undefined || frameDescriptor === undefined) return [];
    return [...(definition.meshMaterialSlots[frameDescriptor.mesh] ?? [])];
  }

  #voxelObjectInstanceMaterials(
    definition: VoxelObjectDef,
    overrides: VoxelObjectInstanceDescriptor['materialOverrides'],
  ): {
    readonly materials: THREE.Material[];
    readonly materialIds: (string | null)[];
    readonly ownedMaterialIndices: Set<number>;
  } {
    const materials = definition.materials.slice();
    const materialIds: (string | null)[] = definition.materialSlots.map((slot) => slot.material);
    const ownedMaterialIndices = new Set<number>();
    for (const override of overrides) {
      const index = definition.slotIndex.get(override.slot);
      if (index === undefined) {
        throw new RenderApplyError(
          `voxel object material override uses unbound slot ${override.slot}`,
        );
      }
      materials[index] = this.#materialFor(override);
      materialIds[index] = override.material;
      ownedMaterialIndices.add(index);
    }
    return { materials, materialIds, ownedMaterialIndices };
  }

  /** Current renderer-side frame selection; never gameplay authority. */
  voxelObjectFrame(handle: RenderHandle): RendererVoxelObjectFrameReadout | undefined {
    const entry = this.#handles.get(handle);
    if (entry?.kind !== 'voxelObject' || entry.asset === undefined || entry.voxelFrame === undefined) {
      return undefined;
    }
    const definition = this.#voxelObjects.get(entry.asset);
    const frame = definition?.frames[entry.voxelFrame];
    if (frame === undefined) return undefined;
    return {
      handle,
      asset: entry.asset,
      frame: entry.voxelFrame,
      frameId: frame.id,
      mesh: frame.mesh,
    };
  }

  /** How many live instances reference a defined static mesh asset (0 if undefined). */
  instanceCountFor(asset: string): number {
    return this.#staticMeshes.get(asset)?.refCount ?? 0;
  }

  /**
   * Register (or replace) a retained material descriptor by id. A
   * *redefine* of an already-registered id is a live visual-only update: every
   * static-mesh material bound to that id is rebuilt from the new descriptor and
   * the old material disposed (leak-safe), so a visual edit changes the rendered
   * output deterministically without a destroy+create. This renderer owns only
   * presentation state; downstream authority decides which definitions it emits.
   */
  #defineMaterial(material: RenderMaterialDescriptor): void {
    this.#materials.set(material.id, material);
    this.#replaceLiveMaterial(material.id);
  }

  /** Rebuild shared bases and every live instance material bound to `id`. */
  #replaceLiveMaterial(id: string): void {
    const replacedSharedMaterials = new Set<THREE.Material>();
    for (const def of this.#staticMeshes.values()) {
      for (let index = 0; index < def.materialSlots.length; index += 1) {
        const slot = def.materialSlots[index]!;
        if (slot.material !== id) {
          continue;
        }
        replacedSharedMaterials.add(def.materials[index]!);
        def.materials[index] = this.#materialFor(slot);
      }
    }
    for (const def of this.#voxelObjects.values()) {
      for (let index = 0; index < def.materialSlots.length; index += 1) {
        const slot = def.materialSlots[index]!;
        if (slot.material !== id) continue;
        replacedSharedMaterials.add(def.materials[index]!);
        def.materials[index] = this.#materialFor(slot);
      }
    }

    for (const entry of this.#handles.values()) {
      if (entry.meshMaterialSlots?.some(slot => `voxel-material/${String(slot)}` === id)) {
        this.#applyUploadedMeshMaterial(entry, entry.viewMaterial ?? MaterialFallback);
        continue;
      }
      if (
        (entry.kind !== 'staticMesh' && entry.kind !== 'voxelObject')
        || !entry.materialIds
        || entry.asset === undefined
      ) {
        continue;
      }
      const def = entry.kind === 'staticMesh'
        ? this.#staticMeshes.get(entry.asset)
        : this.#voxelObjects.get(entry.asset);
      if (def === undefined) {
        continue;
      }
      const mesh = entry.object as THREE.Mesh;
      const arr = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
      let changed = false;
      for (let i = 0; i < entry.materialIds.length; i += 1) {
        if (entry.materialIds[i] !== id) {
          continue;
        }
        if (entry.ownedMaterialIndices?.has(i)) {
          arr[i]?.dispose();
        }
        const parameters = entry.kind === 'staticMesh'
          ? entry.materialParameterOverrides?.get(i)
          : undefined;
        const baseSlot = def.materialSlots[i];
        const usesSharedBase = parameters === undefined && baseSlot?.material === id;
        const replacement = usesSharedBase
          ? def.materials[i]!
          : this.#materialFor({ slot: baseSlot?.slot ?? i, material: id }, parameters);
        arr[i] = replacement;
        if (usesSharedBase) {
          entry.ownedMaterialIndices?.delete(i);
        } else {
          entry.ownedMaterialIndices?.add(i);
        }
        changed = true;
      }
      if (changed) {
        mesh.material = arr.length === 1 ? arr[0]! : arr;
      }
    }
    replacedSharedMaterials.forEach((material) => material.dispose());
  }

  #setMaterialInstanceParameters(
    diff: Extract<RenderDiff, { op: 'setMaterialInstanceParameters' }>,
  ): void {
    const entry = this.#require(diff.handle, 'setMaterialInstanceParameters');
    if (entry.kind !== 'staticMesh' || entry.asset === undefined || entry.materialIds === undefined) {
      throw new RenderApplyError(
        `setMaterialInstanceParameters: handle ${diff.handle} is not a static-mesh instance`,
      );
    }
    const def = this.#staticMeshes.get(entry.asset);
    const index = def?.slotIndex.get(diff.slot);
    if (def === undefined || index === undefined) {
      throw new RenderApplyError(
        `setMaterialInstanceParameters: unbound slot ${diff.slot} on ${entry.asset}`,
      );
    }
    const materialId = entry.materialIds[index];
    if (materialId === null || materialId === undefined) {
      throw new RenderApplyError(
        `setMaterialInstanceParameters: slot ${diff.slot} on ${entry.asset} has no material`,
      );
    }

    const mesh = entry.object as THREE.Mesh;
    const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
    if (entry.ownedMaterialIndices?.has(index)) {
      materials[index]?.dispose();
    }

    const baseSlot = def.materialSlots[index]!;
    if (diff.parameters === null) {
      entry.materialParameterOverrides?.delete(index);
      if (baseSlot.material === materialId) {
        materials[index] = def.materials[index]!;
        entry.ownedMaterialIndices?.delete(index);
      } else {
        materials[index] = this.#materialFor({ slot: diff.slot, material: materialId });
        entry.ownedMaterialIndices?.add(index);
      }
    } else {
      entry.materialParameterOverrides?.set(index, diff.parameters);
      materials[index] = this.#materialFor(
        { slot: diff.slot, material: materialId },
        diff.parameters,
      );
      entry.ownedMaterialIndices?.add(index);
    }
    mesh.material = materials.length === 1 ? materials[0]! : materials;
  }

  /** A registered retained material descriptor by id, for inspection/tests. */
  materialDescriptor(id: string): RenderMaterialDescriptor | undefined {
    return this.#materials.get(id);
  }

  /** Total placeholder-fallback material resolutions so far (fallback diagnostic). */
  get fallbackMaterialCount(): number {
    return this.#fallbackMaterialCount;
  }

  /** Material ids that resolved to a placeholder fallback (no descriptor). */
  fallbackMaterials(): string[] {
    return [...this.#fallbackMaterials].sort();
  }

  #materialFor(
    slot: MeshMaterialSlot,
    parameters?: MaterialInstanceParameters,
  ): THREE.MeshStandardMaterial {
    // Resolve the slot's material id to the retained RenderMaterialDescriptor from
    // defineMaterial. A descriptor drives the real colour; a missing one
    // falls back deterministically to the per-slot hue and is recorded (id + count)
    // so the gap is an observable diagnostic rather than silent.
    const descriptor = this.#materials.get(slot.material);
    if (descriptor) {
      const material = standardMaterial(descriptor, parameters);
      this.#trackMaterialResource(material);
      return material;
    }
    this.#fallbackMaterialCount += 1;
    this.#fallbackMaterials.add(slot.material);
    const material = new THREE.MeshStandardMaterial({
      color: this.#slotColor(slot.slot),
      roughness: 1,
      metalness: 0,
    });
    this.#trackMaterialResource(material);
    return material;
  }

  /** A registered texture descriptor by id, for inspection/tests. */
  textureDescriptor(id: string): TextureDescriptor | undefined {
    return this.#textures.get(id);
  }

  /** A registered sprite atlas by id, for inspection/tests. */
  spriteAtlas(id: string): SpriteAtlasDescriptor | undefined {
    return this.#atlases.get(id);
  }

  /** Total sprite-frame fallbacks (no atlas / unknown frame) so far. */
  get spriteFallbackCount(): number {
    return this.#spriteFallbackCount;
  }

  /**
   * Resolve a sprite asset + frame to its atlas UV sub-rectangle and write it into
   * the plane geometry's `uv` attribute. A missing atlas or unknown frame
   * falls back deterministically to full `[0,1]` UVs and is counted, so the gap is
   * observable rather than a silent wrong-frame. Returns the resolved rect
   * `[u0,v0,u1,v1]` (or the full-UV fallback) for the snapshot.
   */
  #applySpriteUv(
    geometry: THREE.BufferGeometry,
    asset: string,
    frame: number,
  ): [number, number, number, number] {
    const atlas = this.#atlases.get(asset);
    const rect = atlas?.frames.find((f) => f.frame === frame);
    if (!rect) {
      if (atlas !== undefined || this.#textures.size > 0 || frame !== 0) {
        this.#spriteFallbackCount += 1;
      }
      return [0, 0, 1, 1];
    }
    const [u0, v0] = rect.uvMin;
    const [u1, v1] = rect.uvMax;
    // PlaneGeometry vertex order: top-left, top-right, bottom-left, bottom-right.
    const uv = geometry.getAttribute('uv') as THREE.BufferAttribute;
    uv.setXY(0, u0, v1);
    uv.setXY(1, u1, v1);
    uv.setXY(2, u0, v0);
    uv.setXY(3, u1, v0);
    uv.needsUpdate = true;
    return [u0, v0, u1, v1];
  }

  // ── Sprites / billboards (render-asset-05/06) ───────────────────────────────

  #createSprite(diff: Extract<RenderDiff, { op: 'createSprite' }>): void {
    if (this.#handles.has(diff.handle)) {
      throw new RenderApplyError(`createSprite: handle ${diff.handle} already exists`);
    }
    const s = diff.sprite;
    // Plane BufferGeometry (NOT THREE.Sprite) so the node fits the retained handle
    // lifecycle and future batching. Pivot shifts the plane so the anchor sits at
    // the node origin.
    const geometry = new THREE.PlaneGeometry(s.size[0], s.size[1]);
    geometry.translate((0.5 - s.pivot[0]) * s.size[0], (0.5 - s.pivot[1]) * s.size[1], 0);
    const material = new THREE.MeshBasicMaterial({
      color: new THREE.Color(s.tint[0], s.tint[1], s.tint[2]),
      opacity: s.tint[3],
      transparent: s.tint[3] < 1,
      depthTest: s.depth !== 'depthTestOff',
      depthWrite: s.depth === 'default',
    });
    const mesh = new THREE.Mesh(geometry, material);
    this.#trackObjectResources(mesh);
    mesh.renderOrder = s.renderOrder;
    applyTransform(mesh, s.transform);
    applyMetadata(mesh, s.metadata);
    mesh.visible = s.visible;
    mesh.userData['frame'] = s.frame;
    mesh.userData['billboard'] = s.billboard;
    mesh.userData['uv'] = this.#applySpriteUv(geometry, s.asset, s.frame);

    const parent =
      diff.parent === null ? this.#sceneGroup : this.#require(diff.parent, 'createSprite.parent').object;
    parent.add(mesh);
    this.#handles.set(diff.handle, {
      object: mesh,
      kind: 'sprite',
      shape: 'quad',
      asset: s.asset,
      ownsGeometry: true,
      sprite: s,
    });
  }

  /**
   * Deterministic, projection-driven sprite update. Frame/tint/order/visibility
   * come from an authority tick — never renderer wall-clock animation — so the
   * same diff sequence always produces the same scene.
   */
  #updateSprite(diff: Extract<RenderDiff, { op: 'updateSprite' }>): void {
    const entry = this.#require(diff.handle, 'updateSprite');
    if (entry.kind !== 'sprite' || !entry.sprite) {
      throw new RenderApplyError(`updateSprite: handle ${diff.handle} is not a sprite`);
    }
    const mesh = entry.object as THREE.Mesh;
    const material = mesh.material as THREE.MeshBasicMaterial;
    if (diff.frame !== null) {
      entry.sprite = { ...entry.sprite, frame: diff.frame };
      mesh.userData['frame'] = diff.frame;
      // Re-resolve the atlas UV rect for the new frame (deterministic, no anim).
      mesh.userData['uv'] = this.#applySpriteUv(mesh.geometry, entry.sprite.asset, diff.frame);
    }
    if (diff.tint !== null) {
      entry.sprite = { ...entry.sprite, tint: diff.tint };
      material.color.setRGB(diff.tint[0], diff.tint[1], diff.tint[2]);
      material.opacity = diff.tint[3];
      material.transparent = diff.tint[3] < 1;
    }
    if (diff.renderOrder !== null) {
      entry.sprite = { ...entry.sprite, renderOrder: diff.renderOrder };
      mesh.renderOrder = diff.renderOrder;
    }
    if (diff.visible !== null) {
      mesh.visible = diff.visible;
      entry.sprite = { ...entry.sprite, visible: diff.visible };
    }
  }

  /**
   * Resolve a renderer-side sprite pick to an authority-facing trace: render
   * handle + source entity/scene-node ids + asset ref + attachment point. The
   * renderer decides no gameplay action — authority revalidates and acts.
   */
  pickSprite(handle: RenderHandle): SpritePickHit | undefined {
    const entry = this.#handles.get(handle);
    if (!entry || entry.kind !== 'sprite' || !entry.sprite) {
      return undefined;
    }
    const a = entry.sprite.attachment;
    return {
      handle,
      sourceEntity: a.sourceEntity,
      sourceSceneNode: a.sourceSceneNode,
      asset: entry.sprite.asset,
      attachmentPoint: a.attachmentPoint,
    };
  }

  /**
   * Replace a node's geometry with an uploaded voxel mesh payload. Uploads the
   * descriptor's attribute/index streams directly into a `BufferGeometry` (typed-
   * array views only — no per-frame transcoding) and maps material slots to flat
   * materials. The old geometry + materials are disposed.
   */
  #replaceMeshPayload(
    diff: Extract<RenderDiff, { op: 'replaceMeshPayload' }>,
    preparedGeometry?: THREE.BufferGeometry,
  ): void {
    const entry = this.#require(diff.handle, 'replaceMeshPayload');
    const object = entry.object;
    if (!(object instanceof THREE.Mesh)) {
      throw new RenderApplyError(`replaceMeshPayload: handle ${diff.handle} is not a mesh`);
    }
    const geometry = preparedGeometry
      ?? buildMeshGeometry(
        diff.payload,
        this.#meshBufferSource,
        this.#meshResourceSource,
        'replaceMeshPayload',
      );
    this.#trackGeometryResource(geometry);
    const viewMaterial = entry.viewMaterial ?? MaterialFallback;
    const materials = diff.payload.groups.map((group) =>
      this.#uploadedMeshMaterial(group.materialSlot, viewMaterial));

    const oldGeometry = object.geometry as THREE.BufferGeometry;
    const oldMaterial = object.material as THREE.Material | THREE.Material[];
    object.geometry = geometry;
    // A multi-group geometry uses an array of materials indexed by group order.
    object.material = materials.length === 1 ? materials[0]! : materials;

    oldGeometry.dispose();
    if (Array.isArray(oldMaterial)) {
      oldMaterial.forEach((m) => m.dispose());
    } else {
      oldMaterial.dispose();
    }
    // Remember the authority source that produced this mesh so a pick can trace the
    // handle back to it. The renderer holds the provenance, never the coordinates.
    entry.meshProvenance = diff.payload.provenance;
    entry.meshMaterialSlots = diff.payload.groups.map((group) => group.materialSlot);
    entry.viewMaterial = viewMaterial;
  }

  #uploadedMeshMaterial(slot: number, view: Material): THREE.MeshStandardMaterial {
    const descriptor = this.#materials.get(`voxel-material/${String(slot)}`);
    if (descriptor !== undefined) {
      const material = standardMaterial(descriptor);
      material.color.multiply(new THREE.Color(view.color[0], view.color[1], view.color[2]));
      material.opacity *= view.color[3];
      material.transparent = material.opacity < 1;
      material.wireframe = view.wireframe;
      this.#trackMaterialResource(material);
      return material;
    }
    const slotColor = this.#slotColor(slot);
    const material = new THREE.MeshStandardMaterial({
      color: new THREE.Color(
        slotColor.r * view.color[0],
        slotColor.g * view.color[1],
        slotColor.b * view.color[2],
      ),
      opacity: view.color[3],
      transparent: view.color[3] < 1,
      wireframe: view.wireframe,
      roughness: 1,
      metalness: 0,
    });
    this.#trackMaterialResource(material);
    return material;
  }

  #applyUploadedMeshMaterial(entry: NodeEntry, view: Material): void {
    const mesh = entry.object as THREE.Mesh;
    const previous = meshMaterials(mesh);
    const next = (entry.meshMaterialSlots ?? []).map((slot) => this.#uploadedMeshMaterial(slot, view));
    mesh.material = next.length === 1 ? next[0]! : next;
    previous.forEach((material) => material.dispose());
  }

  #createLight(diff: Extract<RenderDiff, { op: 'createLight' }>): void {
    if (this.#handles.has(diff.handle)) {
      throw new RenderApplyError(`createLight: handle ${diff.handle} already exists`);
    }
    validateLightDescriptor(diff.light, 'createLight.light', (message) => new RenderApplyError(message));
    const object = buildLight(diff.light, this.#shadowsEnabled);
    const parent = diff.parent === null
      ? this.#sceneGroup
      : this.#require(diff.parent, 'createLight.parent').object;
    parent.add(object);
    this.#handles.set(diff.handle, {
      object,
      kind: 'light',
      shape: 'point',
      ownsGeometry: false,
      light: structuredClone(diff.light),
    });
  }

  #updateLight(diff: Extract<RenderDiff, { op: 'updateLight' }>): void {
    const entry = this.#require(diff.handle, 'updateLight');
    if (entry.kind !== 'light' || entry.light === undefined) {
      throw new RenderApplyError(`updateLight: handle ${diff.handle} is not a light`);
    }
    validateLightDescriptor(diff.light, 'updateLight.light', (message) => new RenderApplyError(message));
    if (entry.light.kind !== diff.light.kind) {
      throw new RenderApplyError(
        `updateLight: handle ${diff.handle} cannot change kind from ${entry.light.kind} to ${diff.light.kind}`,
      );
    }
    applyLightDescriptor(entry.object, diff.light, this.#shadowsEnabled);
    entry.light = structuredClone(diff.light);
  }

  /**
   * Resolve a renderer-side mesh pick to an authority source trace: the render handle
   * + the provenance of the uploaded mesh. Only a **hint** — authority picking
   * (`pickVoxel`) revalidates before any selection/edit acts on it. Returns
   * `undefined` for a handle with no uploaded mesh, or a stale/destroyed/unknown
   * handle (fail closed — the renderer never invents a source for missing metadata).
   */
  pickMesh(handle: RenderHandle): MeshPickHit | undefined {
    const entry = this.#handles.get(handle);
    if (!entry || entry.meshProvenance === undefined) {
      return undefined;
    }
    const metadata = readMetadata(entry.object);
    return {
      handle,
      provenance: entry.meshProvenance,
      sourceEntity: metadata.sourceEntity,
      sourceSceneNode: metadata.sourceSceneNode,
    };
  }

  #require(handle: RenderHandle, ctx: string): NodeEntry {
    const entry = this.#handles.get(handle);
    if (entry === undefined) {
      throw new RenderApplyError(`${ctx}: unknown handle ${handle}`);
    }
    return entry;
  }
}

function isDescendantOf(object: THREE.Object3D, ancestor: THREE.Object3D): boolean {
  let candidate = object.parent;
  while (candidate !== null) {
    if (candidate === ancestor) {
      return true;
    }
    candidate = candidate.parent;
  }
  return false;
}

function snapshotLine(handle: number, entry: NodeEntry, layer: RenderLayer): string {
  const o = entry.object;
  const head = `handle ${handle}  layer ${layer}`;
  if (entry.kind === 'light' && entry.light !== undefined) {
    return [
      head,
      `kind light/${entry.light.kind}`,
      `enabled ${entry.light.enabled}`,
      `intensity ${fmtNum(entry.light.intensity)}`,
      `color ${entry.light.color.map(fmtNum).join(',')}`,
      `shadow ${entry.light.shadowIntent}`,
    ].join('  ');
  }
  if (entry.kind === 'staticMesh') {
    return [
      head,
      `kind staticMesh`,
      `asset ${entry.asset}`,
      `pos ${fmtVec(o.position)}`,
      `scale ${fmtVec(o.scale)}`,
      `visible ${o.visible}`,
      `materials ${fmtMaterials(o)}`,
      `label ${JSON.stringify(o.name)}`,
    ].join('  ');
  }
  if (entry.kind === 'sprite' && entry.sprite) {
    const s = entry.sprite;
    const a = s.attachment;
    return [
      head,
      `kind sprite`,
      `asset ${s.asset}`,
      `frame ${s.frame}`,
      `uv ${((o.userData['uv'] as number[]) ?? [0, 0, 1, 1]).map(fmtNum).join(',')}`,
      `pos ${fmtVec(o.position)}`,
      `size ${fmtNum(s.size[0])},${fmtNum(s.size[1])}`,
      `pivot ${fmtNum(s.pivot[0])},${fmtNum(s.pivot[1])}`,
      `billboard ${s.billboard}`,
      `tint ${s.tint.map(fmtNum).join(',')}`,
      `order ${o.renderOrder}`,
      `depth ${s.depth}`,
      `shading ${s.shading}`,
      `visible ${o.visible}`,
      `attach ${a.sourceEntity ?? '-'}/${a.sourceSceneNode ?? '-'}/${a.attachmentPoint ?? '-'}`,
      `label ${JSON.stringify(o.name)}`,
    ].join('  ');
  }
  if (entry.kind === 'animatedMesh') {
    const playback = (o.userData['animatedMeshPlayback'] as AnimatedMeshPlaybackReadout | undefined) ?? null;
    return [
      head,
      `kind animatedMesh`,
      `asset ${entry.asset}`,
      `clip ${playback?.currentClip ?? '-'}`,
      `time ${fmtNum(playback?.actionTimeSeconds ?? 0)}`,
      `pos ${fmtVec(o.position)}`,
      `scale ${fmtVec(o.scale)}`,
      `visible ${o.visible}`,
      `label ${JSON.stringify(o.name)}`,
    ].join('  ');
  }
  if (entry.kind === 'voxelObject') {
    return [
      head,
      `kind voxelObject`,
      `asset ${entry.asset}`,
      `frame ${entry.voxelFrame ?? 0}`,
      `pos ${fmtVec(o.position)}`,
      `scale ${fmtVec(o.scale)}`,
      `visible ${o.visible}`,
      `materials ${fmtMaterials(o)}`,
      `label ${JSON.stringify(o.name)}`,
    ].join('  ');
  }
  return [
    head,
    `shape ${entry.shape}`,
    `pos ${fmtVec(o.position)}`,
    `scale ${fmtVec(o.scale)}`,
    `visible ${o.visible}`,
    `color ${fmtColor(o)}`,
    `label ${JSON.stringify(o.name)}`,
  ].join('  ');
}

function fmtMaterials(object: THREE.Object3D): string {
  const material = (object as THREE.Mesh).material;
  const list = Array.isArray(material) ? material : [material];
  return (
    '[' +
    list
      .map((m) => {
        const c = (m as THREE.MeshBasicMaterial).color;
        if (!c) {
          return 'none';
        }
        const color = `${fmtNum(c.r)},${fmtNum(c.g)},${fmtNum(c.b)}`;
        if (
          !(m instanceof THREE.MeshStandardMaterial)
          || m.emissiveIntensity === 0
          || (m.emissive.r === 0 && m.emissive.g === 0 && m.emissive.b === 0)
        ) {
          return color;
        }
        const emission = `${fmtNum(m.emissive.r)},${fmtNum(m.emissive.g)},${fmtNum(m.emissive.b)}`;
        return `${color}~emit(${emission}*${fmtNum(m.emissiveIntensity)})`;
      })
      .join(' ') +
    ']'
  );
}

/** Dispose just this instance's owned materials, leaving shared asset bases alone. */
function disposeInstanceMaterials(entry: NodeEntry): void {
  const mesh = entry.object as THREE.Mesh;
  const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
  entry.ownedMaterialIndices?.forEach((index) => materials[index]?.dispose());
}

function standardMaterial(
  descriptor: RenderMaterialDescriptor,
  parameters?: MaterialInstanceParameters,
): THREE.MeshStandardMaterial {
  const tint = parameters?.textureTint ?? descriptor.textureTint;
  const emissionColor = parameters?.emissionColor ?? descriptor.emissionColor;
  const emissionIntensity = parameters?.emissionIntensity ?? descriptor.emissionIntensity;
  const color = new THREE.Color(
    descriptor.color[0] * tint[0],
    descriptor.color[1] * tint[1],
    descriptor.color[2] * tint[2],
  );
  const opacity = descriptor.color[3] * tint[3];
  return new THREE.MeshStandardMaterial({
    color,
    emissive: new THREE.Color(emissionColor[0], emissionColor[1], emissionColor[2]),
    emissiveIntensity: emissionIntensity,
    metalness: 0,
    opacity,
    roughness: descriptor.roughness,
    transparent: opacity < 1,
  });
}

// ── Builders (contract → Three.js) ────────────────────────────────────────────

function buildObject(node: RenderNode): THREE.Object3D {
  let object: THREE.Object3D;
  switch (node.geometry.kind) {
    case 'group':
      object = new THREE.Group();
      break;
    case 'cube':
      object = new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1), buildMaterial('cube', node.material));
      break;
    case 'sphere':
      object = new THREE.Mesh(new THREE.SphereGeometry(0.5, 8, 8), buildMaterial('sphere', node.material));
      break;
    case 'quad':
      object = new THREE.Mesh(new THREE.PlaneGeometry(1, 1), buildMaterial('quad', node.material));
      break;
    case 'point':
      object = new THREE.Points(pointGeometry(), buildMaterial('point', node.material));
      break;
    case 'line':
      object = new THREE.LineSegments(
        lineGeometry(node.geometry.a, node.geometry.b),
        buildMaterial('line', node.material),
      );
      break;
    default: {
      const exhaustive: never = node.geometry;
      throw new RenderApplyError(`unhandled geometry ${JSON.stringify(exhaustive)}`);
    }
  }
  applyTransform(object, node.transform);
  object.visible = node.visible;
  applyMetadata(object, node.metadata);
  return object;
}

function buildMaterial(shape: Geometry['kind'], material: Material): THREE.Material {
  const color = new THREE.Color(material.color[0], material.color[1], material.color[2]);
  const opacity = material.color[3];
  const transparent = opacity < 1;
  switch (shape) {
    case 'point':
      return new THREE.PointsMaterial({ color, opacity, transparent, size: 0.1 });
    case 'line':
      return new THREE.LineBasicMaterial({ color, opacity, transparent });
    default:
      return new THREE.MeshBasicMaterial({
        color,
        opacity,
        transparent,
        wireframe: material.wireframe,
      });
  }
}

/**
 * Build a `THREE.BufferGeometry` from a mesh payload descriptor. Inline sources
 * wrap the contract number arrays as typed arrays directly; shared-buffer sources resolve
 * provider-owned bytes through the optional {@link MeshBufferSource} and slice the
 * attribute/index streams out by byte offset. A shared-buffer source with no provider, an
 * unknown/stale handle, or a buffer too small for the declared layout fails closed
 * with a classified `RenderApplyError` — never a silent empty mesh.
 */
function buildVoxelObjectGeometries(
  asset: VoxelObjectRenderAsset,
  bufferSource: MeshBufferSource | undefined,
  resourceSource: MeshResourceSource | undefined,
): THREE.BufferGeometry[] {
  const geometries: THREE.BufferGeometry[] = [];
  const slotIndices = new Map(asset.materialSlots.map((slot, index) => [slot.slot, index]));
  try {
    asset.meshes.forEach((mesh, index) => {
      const geometry = buildMeshGeometry(
        mesh.payload,
        bufferSource,
        resourceSource,
        `defineVoxelObject.meshes[${String(index)}]`,
      );
      geometry.clearGroups();
      mesh.payload.groups.forEach((group) => {
        const materialIndex = slotIndices.get(group.materialSlot);
        if (materialIndex === undefined) {
          geometry.dispose();
          throw new RenderApplyError(
            `defineVoxelObject.meshes[${String(index)}]: unbound material slot ${group.materialSlot}`,
          );
        }
        geometry.addGroup(group.start, group.count, materialIndex);
      });
      geometries.push(geometry);
    });
    return geometries;
  } catch (cause) {
    geometries.forEach((geometry) => geometry.dispose());
    throw cause;
  }
}

function buildMeshGeometry(
  payload: MeshPayloadDescriptor,
  bufferSource: MeshBufferSource | undefined,
  resourceSource: MeshResourceSource | undefined,
  ctx: string,
): THREE.BufferGeometry {
  const streams = payload.source.kind === 'inline'
    ? inlineStreams(payload.source)
    : payload.source.kind === 'sharedBuffer'
      ? sharedBufferStreams(payload, payload.source, bufferSource, ctx)
      : resourceStreams(payload, payload.source, resourceSource, ctx);

  const positionComponents = attributeComponents(payload, 'position');
  const normalComponents = attributeComponents(payload, 'normal');

  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.BufferAttribute(streams.positions, positionComponents));
  geometry.setAttribute('normal', new THREE.BufferAttribute(streams.normals, normalComponents));
  geometry.setIndex(new THREE.BufferAttribute(streams.indices, 1));
  // One draw group per material slot (BufferGeometry.addGroup(start, count, index)).
  payload.groups.forEach((g, i) => geometry.addGroup(g.start, g.count, i));
  geometry.boundingBox = new THREE.Box3(
    new THREE.Vector3(payload.bounds.min[0], payload.bounds.min[1], payload.bounds.min[2]),
    new THREE.Vector3(payload.bounds.max[0], payload.bounds.max[1], payload.bounds.max[2]),
  );
  return geometry;
}

interface MeshStreams {
  readonly positions: Float32Array;
  readonly normals: Float32Array;
  readonly indices: Uint32Array;
}

/** Wrap inline contract number arrays as typed arrays (the golden-fixture path). */
function inlineStreams(source: Extract<MeshPayloadDescriptor['source'], { kind: 'inline' }>): MeshStreams {
  return {
    positions: new Float32Array(source.positions),
    normals: new Float32Array(source.normals),
    indices: new Uint32Array(source.indices),
  };
}

/**
 * Resolve a shared-buffer payload's bytes under the **borrow → copy → release**
 * contract: borrow the buffer, copy every declared stream out immediately
 * (so the borrow is never retained), then release the borrow. The borrow is
 * released on both the success and the failure path; a missing provider, an
 * unknown/stale/expired handle, an out-of-bounds window, or an out-of-range index
 * all fail closed with a classified `RenderApplyError` — never empty geometry.
 */
function sharedBufferStreams(
  payload: MeshPayloadDescriptor,
  source: Extract<MeshPayloadDescriptor['source'], { kind: 'sharedBuffer' }>,
  bufferSource: MeshBufferSource | undefined,
  ctx: string,
): MeshStreams {
  if (bufferSource === undefined) {
    throw new RenderApplyError(
      `${ctx}: shared-buffer payload needs a mesh buffer provider (buffer ${source.buffer})`,
    );
  }

  const buffer = source.buffer;
  let view: MeshBufferView;
  try {
    view = bufferSource.acquireBuffer(buffer);
  } catch (cause) {
    // No borrow was acquired, so nothing to release. Classify and fail closed.
    throw classifyBufferError(cause, source.buffer, ctx, 'unavailable');
  }

  // Borrow acquired — copy out, then release exactly once on every exit path.
  let streams: MeshStreams;
  try {
    streams = copySharedBufferStreams(view, payload, source, ctx);
  } catch (cause) {
    releaseBorrowBestEffort(bufferSource, buffer); // failure path: never mask the cause
    throw cause;
  }
  // Success path: release and surface a classified error if release itself fails.
  releaseBorrow(bufferSource, buffer, ctx);
  return streams;
}

function resourceStreams(
  payload: MeshPayloadDescriptor,
  source: Extract<MeshPayloadDescriptor['source'], { kind: 'resource' }>,
  resourceSource: MeshResourceSource | undefined,
  ctx: string,
): MeshStreams {
  if (resourceSource === undefined) {
    throw new RenderApplyError(
      `${ctx}: resource payload needs a mesh resource provider (${source.resource})`,
    );
  }
  let view: MeshBufferView;
  try {
    view = resourceSource.acquireResource(
      source.resource,
      source.contentHash,
      source.byteLength,
    );
  } catch (cause) {
    throw classifyResourceError(cause, source.resource, ctx, 'unavailable');
  }
  let streams: MeshStreams;
  try {
    validatePackedResourceHeader(view.bytes, source, ctx);
    streams = copyResourceStreams(view, payload, source, ctx);
  } catch (cause) {
    try {
      resourceSource.releaseResource(source.resource);
    } catch {
      // The resource decode failure already in flight remains primary.
    }
    throw cause;
  }
  try {
    resourceSource.releaseResource(source.resource);
  } catch (cause) {
    throw classifyResourceError(cause, source.resource, ctx, 'release failed');
  }
  return streams;
}

function validatePackedResourceHeader(
  bytes: Uint8Array,
  source: Extract<MeshPayloadDescriptor['source'], { kind: 'resource' }>,
  ctx: string,
): void {
  const magic = [0x52, 0x4d, 0x53, 0x48, 0x4c, 0x45, 0x30, 0x31]; // RMSHLE01
  if (bytes.byteLength !== source.byteLength
    || magic.some((byte, index) => bytes[index] !== byte)
    || bytes.byteLength < 16) {
    throw new RenderApplyError(`${ctx}: mesh resource ${source.resource} has an invalid v1 header`);
  }
  const header = new DataView(bytes.buffer, bytes.byteOffset, 16);
  if (header.getUint32(8, true) !== bytes.byteLength || header.getUint32(12, true) === 0) {
    throw new RenderApplyError(`${ctx}: mesh resource ${source.resource} has an invalid v1 header`);
  }
}

function copyResourceStreams(
  view: MeshBufferView,
  payload: MeshPayloadDescriptor,
  source: Extract<MeshPayloadDescriptor['source'], { kind: 'resource' }>,
  ctx: string,
): MeshStreams {
  const { vertexCount, indexCount } = payload.layout;
  const positions = sliceFloat32(
    view,
    source.positionsByteOffset,
    vertexCount * attributeComponents(payload, 'position'),
    'positions',
    source.resource,
    ctx,
  );
  const normals = sliceFloat32(
    view,
    source.normalsByteOffset,
    vertexCount * attributeComponents(payload, 'normal'),
    'normals',
    source.resource,
    ctx,
  );
  const indices = sliceUint32(
    view,
    source.indicesByteOffset,
    indexCount,
    source.resource,
    ctx,
  );
  for (const index of indices) {
    if (index >= vertexCount) {
      throw new RenderApplyError(
        `${ctx}: index ${index} out of range for ${vertexCount} vertices (resource ${source.resource})`,
      );
    }
  }
  return { positions, normals, indices };
}

function classifyResourceError(
  cause: unknown,
  resource: string,
  ctx: string,
  what: string,
): RenderApplyError {
  if (cause instanceof RenderResourceError) {
    return new RenderApplyError(
      `${ctx}: resource ${resource} ${what} [${cause.code}]: ${cause.message}`,
    );
  }
  const message = cause instanceof Error ? cause.message : String(cause);
  return new RenderApplyError(
    `${ctx}: resource ${resource} ${what} [providerFailure]: ${message}`,
  );
}

/** Copy + validate the three streams out of a borrowed view (no borrow retained). */
function copySharedBufferStreams(
  view: MeshBufferView,
  payload: MeshPayloadDescriptor,
  source: Extract<MeshPayloadDescriptor['source'], { kind: 'sharedBuffer' }>,
  ctx: string,
): MeshStreams {
  const { vertexCount, indexCount } = payload.layout;
  const positionComponents = attributeComponents(payload, 'position');
  const normalComponents = attributeComponents(payload, 'normal');

  const positions = sliceFloat32(
    view,
    source.positionsByteOffset,
    vertexCount * positionComponents,
    'positions',
    source.buffer,
    ctx,
  );
  const normals = sliceFloat32(
    view,
    source.normalsByteOffset,
    vertexCount * normalComponents,
    'normals',
    source.buffer,
    ctx,
  );
  const indices = sliceUint32(view, source.indicesByteOffset, indexCount, source.buffer, ctx);

  for (let i = 0; i < indices.length; i++) {
    if ((indices[i] as number) >= vertexCount) {
      throw new RenderApplyError(
        `${ctx}: index ${indices[i]} out of range for ${vertexCount} vertices (buffer ${source.buffer})`,
      );
    }
  }
  return { positions, normals, indices };
}

/** Map a provider error to a renderer-boundary `RenderApplyError`. */
function classifyBufferError(cause: unknown, buffer: number, ctx: string, what: string): unknown {
  if (cause instanceof RenderResourceError) {
    return new RenderApplyError(
      `${ctx}: buffer ${buffer} ${what} [${cause.code}]: ${cause.message}`,
    );
  }
  const message = cause instanceof Error ? cause.message : String(cause);
  return new RenderApplyError(`${ctx}: buffer ${buffer} ${what} [providerFailure]: ${message}`);
}

/** Release a borrow on the success path; a release failure is classified, not hidden. */
function releaseBorrow(
  bufferSource: MeshBufferSource,
  buffer: number,
  ctx: string,
): void {
  try {
    bufferSource.releaseBuffer(buffer);
  } catch (cause) {
    throw classifyBufferError(cause, buffer, ctx, 'release failed');
  }
}

/** Release a borrow on a failure path; swallow release errors so the original
 *  failure (the reason we are unwinding) is the one the caller sees. */
function releaseBorrowBestEffort(bufferSource: MeshBufferSource, buffer: number): void {
  try {
    bufferSource.releaseBuffer(buffer);
  } catch {
    // best-effort: the copy/validation error already in flight is the primary one
  }
}

/** Components-per-vertex for a declared attribute (defaults to 3 if unspecified). */
function attributeComponents(payload: MeshPayloadDescriptor, name: 'position' | 'normal'): number {
  const attribute = payload.layout.attributes.find((a) => a.name === name);
  return attribute?.components ?? 3;
}

/** Copy `count` f32s out of a borrowed buffer at `byteOffset`, failing closed if out of bounds. */
function sliceFloat32(
  view: MeshBufferView,
  byteOffset: number,
  count: number,
  label: string,
  buffer: number | string,
  ctx: string,
): Float32Array {
  const byteLength = count * Float32Array.BYTES_PER_ELEMENT;
  const bytes = requireBytes(view, byteOffset, byteLength, label, buffer, ctx);
  return new Float32Array(bytes.buffer, bytes.byteOffset, count);
}

/** Copy `count` u32s out of a borrowed buffer at `byteOffset`, failing closed if out of bounds. */
function sliceUint32(
  view: MeshBufferView,
  byteOffset: number,
  count: number,
  buffer: number | string,
  ctx: string,
): Uint32Array {
  const byteLength = count * Uint32Array.BYTES_PER_ELEMENT;
  const bytes = requireBytes(view, byteOffset, byteLength, 'indices', buffer, ctx);
  return new Uint32Array(bytes.buffer, bytes.byteOffset, count);
}

/**
 * Copy a `[byteOffset, byteOffset+byteLength)` window out of the borrowed view into
 * a fresh, alignment-safe buffer. Throws a classified `RenderApplyError` if the
 * window does not fit — a stale/wrong-layout handle must not read past its bytes.
 */
function requireBytes(
  view: MeshBufferView,
  byteOffset: number,
  byteLength: number,
  label: string,
  buffer: number | string,
  ctx: string,
): Uint8Array {
  if (byteOffset < 0 || byteOffset + byteLength > view.bytes.length) {
    throw new RenderApplyError(
      `${ctx}: ${label} window [${byteOffset}, ${byteOffset + byteLength}) ` +
        `exceeds buffer ${buffer} length ${view.bytes.length}`,
    );
  }
  // slice() returns a fresh ArrayBuffer at offset 0 — a copy-out that drops the
  // borrow and guarantees 4-byte alignment for the typed-array views above.
  return view.bytes.slice(byteOffset, byteOffset + byteLength);
}

function pointGeometry(): THREE.BufferGeometry {
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.Float32BufferAttribute([0, 0, 0], 3));
  return geometry;
}

function lineGeometry(
  a: readonly [number, number, number],
  b: readonly [number, number, number],
): THREE.BufferGeometry {
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute(
    'position',
    new THREE.Float32BufferAttribute([a[0], a[1], a[2], b[0], b[1], b[2]], 3),
  );
  return geometry;
}

function fmtNum(x: number): string {
  // Round to tame float noise; String(-0) is "0", keeping snapshots stable.
  return String(Number(x.toFixed(4)));
}

function fmtVec(v: THREE.Vector3): string {
  return `${fmtNum(v.x)},${fmtNum(v.y)},${fmtNum(v.z)}`;
}

function fmtColor(object: THREE.Object3D): string {
  const material = (object as THREE.Mesh).material;
  const single = Array.isArray(material) ? material[0] : material;
  const color = (single as THREE.MeshBasicMaterial | undefined)?.color;
  return color ? `${fmtNum(color.r)},${fmtNum(color.g)},${fmtNum(color.b)}` : 'none';
}

function applyTransform(object: THREE.Object3D, t: Transform): void {
  object.position.set(t.translation[0], t.translation[1], t.translation[2]);
  object.quaternion.set(t.rotation[0], t.rotation[1], t.rotation[2], t.rotation[3]);
  object.scale.set(t.scale[0], t.scale[1], t.scale[2]);
}

function applyMetadata(object: THREE.Object3D, metadata: RenderMetadata): void {
  object.name = metadata.label ?? '';
  object.userData['renderMetadata'] = structuredClone(metadata);
}

function readMetadata(object: THREE.Object3D): RenderMetadata {
  const metadata = object.userData['renderMetadata'] as RenderMetadata | undefined;
  return metadata === undefined
    ? { sourceEntity: null, sourceSceneNode: null, tags: [], label: null }
    : structuredClone(metadata);
}

function applyMaterial(entry: NodeEntry, material: Material): void {
  if (entry.shape === 'group') {
    return;
  }
  const object = entry.object as THREE.Mesh | THREE.Points | THREE.LineSegments;
  const previous = object.material;
  object.material = buildMaterial(entry.shape, material);
  if (Array.isArray(previous)) {
    previous.forEach((m) => m.dispose());
  } else {
    previous.dispose();
  }
}

function disposeObject(object: THREE.Object3D): void {
  const disposable = object as Partial<{
    geometry: THREE.BufferGeometry;
    material: THREE.Material | THREE.Material[];
  }>;
  disposable.geometry?.dispose();
  if (Array.isArray(disposable.material)) {
    disposable.material.forEach((m) => m.dispose());
  } else {
    disposable.material?.dispose();
  }
}

function disposeObjectRecursive(object: THREE.Object3D): void {
  object.traverse((child) => disposeObject(child));
}

function materialTextures(material: THREE.Material): ReadonlySet<THREE.Texture> {
  const textures = new Set<THREE.Texture>();
  for (const value of Object.values(material)) {
    if (value instanceof THREE.Texture) {
      textures.add(value);
    } else if (Array.isArray(value)) {
      for (const candidate of value) {
        if (candidate instanceof THREE.Texture) textures.add(candidate);
      }
    }
  }
  return textures;
}

function objectDepth(object: THREE.Object3D): number {
  let depth = 0;
  let parent = object.parent;
  while (parent !== null) {
    depth += 1;
    parent = parent.parent;
  }
  return depth;
}

function animatedMeshError(cause: unknown): RenderApplyError {
  if (cause instanceof AnimatedMeshApplyError) {
    return new RenderApplyError(cause.message);
  }
  throw cause;
}

function disposePreparedGeometry(
  prepared: ReadonlyMap<number, readonly THREE.BufferGeometry[]>,
): void {
  for (const geometries of prepared.values()) {
    geometries.forEach((geometry) => geometry.dispose());
  }
}
