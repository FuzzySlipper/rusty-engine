// Retained Three.js scene projector for Rusty Engine render diffs.

import * as THREE from 'three';
import { decodeRenderFrameDiff } from '@rusty-engine/render-contracts';
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

type NodeKind = 'primitive' | 'staticMesh' | 'animatedMesh' | 'sprite' | 'light';

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
}

export interface RendererProjectionIdentity {
  readonly handle: RenderHandle;
  readonly layer: RenderLayer;
  readonly metadata: RenderMetadata;
}

/** A defined static mesh asset: one shared geometry + materials, reference-counted. */
interface StaticMeshDef {
  readonly geometry: THREE.BufferGeometry;
  readonly materials: THREE.Material[];
  /** material slot index → position in `materials`. */
  readonly slotIndex: Map<number, number>;
  readonly materialSlots: readonly MeshMaterialSlot[];
  readonly collision: MeshCollisionPolicy;
  refCount: number;
}

/**
 * A retained Three.js scene driven entirely by render diffs.
 *
 * Nodes are addressed by `RenderHandle`; the registry maps each handle to a
 * Three.js `Object3D`. Scene and debug layers are separate groups so overlays
 * can be toggled independently.
 */
export class ThreeRenderer {
  readonly scene = new THREE.Scene();
  readonly #sceneGroup = new THREE.Group();
  readonly #debugGroup = new THREE.Group();
  readonly #uiGroup = new THREE.Group();
  readonly #handles = new Map<RenderHandle, NodeEntry>();
  /** Defined static mesh assets, keyed by asset id (shared geometry lifecycle). */
  readonly #staticMeshes = new Map<string, StaticMeshDef>();
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
  readonly #animatedMeshes: AnimatedMeshRegistry;
  readonly #shadowsEnabled: boolean;

  constructor(options: {
    meshBufferSource?: MeshBufferSource;
    animatedMeshSource?: AnimatedMeshAssetSource;
    shadowsEnabled?: boolean;
  } = {}) {
    this.#meshBufferSource = options.meshBufferSource;
    this.#animatedMeshes = new AnimatedMeshRegistry(options.animatedMeshSource);
    this.#shadowsEnabled = options.shadowsEnabled ?? false;
    this.#sceneGroup.name = 'scene';
    this.#debugGroup.name = 'debug';
    this.#uiGroup.name = 'ui';
    this.scene.add(this.#sceneGroup, this.#debugGroup, this.#uiGroup);
  }

  #layerGroup(layer: RenderLayer): THREE.Group {
    switch (layer) {
      case 'scene': return this.#sceneGroup;
      case 'debug': return this.#debugGroup;
      case 'ui': return this.#uiGroup;
    }
  }

  /** Apply a whole frame of diffs in order. */
  applyFrame(frame: RenderFrameDiff): void {
    const recursivelyDestroyed = new Set<RenderHandle>();
    for (const op of frame.ops) {
      if (op.op === 'destroy') {
        if (!this.#handles.has(op.handle) && recursivelyDestroyed.has(op.handle)) {
          continue;
        }
        this.#destroy(op, recursivelyDestroyed);
      } else {
        this.applyDiff(op);
      }
    }
  }

  /** Strictly decode a versioned contract payload and apply it. */
  applyEncodedFrame(payload: unknown): void {
    this.applyFrame(decodeRenderFrameDiff(payload));
  }

  /** Apply a single diff. Throws `RenderApplyError` on a bad handle. */
  applyDiff(diff: RenderDiff): void {
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
        this.#replaceMeshPayload(diff);
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
        this.#defineStaticMesh(diff.asset);
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
    this.scene.clear();
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
          layer: isDescendantOf(entry.object, this.#debugGroup)
            ? 'debug'
            : isDescendantOf(entry.object, this.#uiGroup) ? 'ui' : 'scene',
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
        isDescendantOf(entry.object, this.#debugGroup)
          ? 'debug'
          : isDescendantOf(entry.object, this.#uiGroup) ? 'ui' : 'scene',
      ))
      .join('\n') + '\n';
  }

  #create(diff: Extract<RenderDiff, { op: 'create' }>): void {
    if (this.#handles.has(diff.handle)) {
      throw new RenderApplyError(`create: handle ${diff.handle} already exists`);
    }
    const object = buildObject(diff.node);
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
      // Shared geometry: dispose only this instance's override materials, then
      // release the asset reference. The asset's geometry is disposed only when
      // its last instance is gone (reference-safe — never while another shares it).
      disposeInstanceMaterials(entry);
      this.#releaseStaticMesh(entry.asset);
    } else if (entry.kind === 'animatedMesh') {
      this.#animatedMeshes.release(diff.handle);
      disposeObjectRecursive(entry.object);
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
  #defineStaticMesh(asset: StaticMeshAsset): void {
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
    const geometry = buildMeshGeometry(
      asset.payload,
      this.#meshBufferSource,
      'defineStaticMesh',
    );
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
    if (def.refCount <= 0) {
      def.geometry.dispose();
      def.materials.forEach((m) => m.dispose());
      this.#staticMeshes.delete(asset);
    }
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

    for (const entry of this.#handles.values()) {
      if (entry.meshMaterialSlots?.some(slot => `voxel-material/${String(slot)}` === id)) {
        this.#applyUploadedMeshMaterial(entry, entry.viewMaterial ?? MaterialFallback);
        continue;
      }
      if (entry.kind !== 'staticMesh' || !entry.materialIds || entry.asset === undefined) {
        continue;
      }
      const def = this.#staticMeshes.get(entry.asset);
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
        const parameters = entry.materialParameterOverrides?.get(i);
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
      return standardMaterial(descriptor, parameters);
    }
    this.#fallbackMaterialCount += 1;
    this.#fallbackMaterials.add(slot.material);
    return new THREE.MeshStandardMaterial({
      color: this.#slotColor(slot.slot),
      roughness: 1,
      metalness: 0,
    });
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
  #replaceMeshPayload(diff: Extract<RenderDiff, { op: 'replaceMeshPayload' }>): void {
    const entry = this.#require(diff.handle, 'replaceMeshPayload');
    const object = entry.object;
    if (!(object instanceof THREE.Mesh)) {
      throw new RenderApplyError(`replaceMeshPayload: handle ${diff.handle} is not a mesh`);
    }
    const geometry = buildMeshGeometry(diff.payload, this.#meshBufferSource, 'replaceMeshPayload');
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
      return material;
    }
    const slotColor = this.#slotColor(slot);
    return new THREE.MeshStandardMaterial({
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
function buildMeshGeometry(
  payload: MeshPayloadDescriptor,
  bufferSource: MeshBufferSource | undefined,
  ctx: string,
): THREE.BufferGeometry {
  const streams =
    payload.source.kind === 'inline'
      ? inlineStreams(payload.source)
      : sharedBufferStreams(payload, payload.source, bufferSource, ctx);

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
  buffer: number,
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
  buffer: number,
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
  buffer: number,
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
