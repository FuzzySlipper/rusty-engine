// Renderer-neutral retained render-diff application.
//
// This module applies generated render diffs to a typed retained projection
// model. It owns no authority, imports no renderer implementation, and never
// touches raw runtime transports. Browser/Three/WebGPU bindings consume the
// returned neutral instructions or inspect the retained snapshot.

import {
  MAX_RENDER_LIGHT_INTENSITY,
} from '@rusty-engine/render-contracts';
import type {
  AnimatedMeshAsset,
  AnimatedMeshInstanceDescriptor,
  AnimatedMeshPlaybackCommand,
  LightDescriptor,
  Material,
  MaterialInstanceParameters,
  MeshPayloadDescriptor,
  MeshPickHit,
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
  StaticMeshInstanceDescriptor,
  TextureDescriptor,
  Transform,
  VoxelObjectInstanceDescriptor,
  VoxelObjectRenderAsset,
} from '@rusty-engine/render-contracts';

/** Raised when a render diff cannot be applied to the retained projection. */
export class RenderProjectionError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'RenderProjectionError';
  }
}

/** Hard retained-state limits for the camera-relative presentation channel. */
export const MAX_VIEWMODEL_NODES = 128;
export const MAX_VIEWMODEL_DISTINCT_ASSETS = 16;
export const MAX_VIEWMODEL_ASSET_EXTENT = 16;
export const MAX_VIEWMODEL_TRANSLATION_COMPONENT = 16;
export const MAX_VIEWMODEL_ROTATION_COMPONENT = 1;
export const MAX_VIEWMODEL_SCALE_COMPONENT = 64;
export const MAX_RETAINED_LIGHTS = 256;

export type RenderProjectionNodeKind = 'primitive' | 'staticMesh' | 'animatedMesh' | 'voxelObject' | 'sprite';

export interface RenderProjectionNodeBase {
  readonly handle: RenderHandle;
  readonly parent: RenderHandle | null;
  readonly children: readonly RenderHandle[];
  readonly kind: RenderProjectionNodeKind;
  readonly layer: RenderLayer;
  readonly transform: Transform;
  readonly visible: boolean;
  readonly metadata: RenderMetadata;
  readonly material: Material | null;
  readonly meshPayload: MeshPayloadDescriptor | null;
}

export interface PrimitiveProjectionNode extends RenderProjectionNodeBase {
  readonly kind: 'primitive';
  readonly node: RenderNode;
}

export interface StaticMeshProjectionNode extends RenderProjectionNodeBase {
  readonly kind: 'staticMesh';
  readonly asset: string;
  readonly instance: StaticMeshInstanceDescriptor;
  readonly materialParameters: readonly MaterialInstanceParameterBinding[];
}

export interface MaterialInstanceParameterBinding {
  readonly slot: number;
  readonly parameters: MaterialInstanceParameters;
}

export interface AnimatedMeshProjectionNode extends RenderProjectionNodeBase {
  readonly kind: 'animatedMesh';
  readonly asset: string;
  readonly instance: AnimatedMeshInstanceDescriptor;
  readonly playback: AnimatedMeshPlaybackCommand | null;
}

export interface VoxelObjectProjectionNode extends RenderProjectionNodeBase {
  readonly kind: 'voxelObject';
  readonly asset: string;
  readonly instance: VoxelObjectInstanceDescriptor;
  readonly frame: number;
}

export interface SpriteProjectionNode extends RenderProjectionNodeBase {
  readonly kind: 'sprite';
  readonly sprite: SpriteInstanceDescriptor;
  readonly frameUv: readonly [number, number, number, number];
  readonly frameSize: readonly [number, number];
  readonly renderOrder: number;
}

export type RenderProjectionNode =
  | PrimitiveProjectionNode
  | StaticMeshProjectionNode
  | AnimatedMeshProjectionNode
  | VoxelObjectProjectionNode
  | SpriteProjectionNode;

export type RenderProjectionInstruction =
  | { readonly op: 'defineMaterial'; readonly material: RenderMaterialDescriptor }
  | { readonly op: 'defineTexture'; readonly texture: TextureDescriptor }
  | { readonly op: 'defineSpriteAtlas'; readonly atlas: SpriteAtlasDescriptor }
  | { readonly op: 'defineStaticMesh'; readonly asset: StaticMeshAsset }
  | { readonly op: 'defineAnimatedMesh'; readonly asset: AnimatedMeshAsset }
  | { readonly op: 'defineVoxelObject'; readonly asset: VoxelObjectRenderAsset }
  | { readonly op: 'releaseVoxelObject'; readonly asset: string }
  | { readonly op: 'upsertLight'; readonly light: RenderProjectionLight }
  | { readonly op: 'upsertNode'; readonly node: RenderProjectionNode }
  | { readonly op: 'removeLight'; readonly handle: RenderHandle }
  | { readonly op: 'removeNode'; readonly handle: RenderHandle };

export interface RenderProjectionLight {
  readonly handle: RenderHandle;
  readonly parent: RenderHandle | null;
  readonly light: LightDescriptor;
}

export interface RenderProjectionSnapshot {
  readonly nodes: readonly RenderProjectionNode[];
  readonly lights: readonly RenderProjectionLight[];
  readonly materials: readonly RenderMaterialDescriptor[];
  readonly textures: readonly TextureDescriptor[];
  readonly spriteAtlases: readonly SpriteAtlasDescriptor[];
  readonly staticMeshes: readonly StaticMeshAsset[];
  readonly animatedMeshes: readonly AnimatedMeshAsset[];
  readonly voxelObjects: readonly VoxelObjectRenderAsset[];
}

/**
 * Bounded diagnostics for the most recently committed fail-atomic frame stage.
 *
 * Definition records counted as shared are immutable retained values reused by
 * the stage. Only records named by a mutating operation are copied.
 */
export interface RenderProjectionStagingStatistics {
  readonly copiedNodeRecords: number;
  readonly copiedLightRecords: number;
  readonly copiedResourceRecords: number;
  readonly sharedDefinitionRecords: number;
}

interface MutableStagingStatistics {
  copiedNodeRecords: number;
  copiedLightRecords: number;
  copiedResourceRecords: number;
  sharedDefinitionRecords: number;
}

type NodeRecord = MutablePrimitiveNode | MutableStaticMeshNode | MutableAnimatedMeshNode | MutableVoxelObjectNode | MutableSpriteNode;

interface MutableNodeBase {
  handle: RenderHandle;
  parent: RenderHandle | null;
  children: Set<RenderHandle>;
  kind: RenderProjectionNodeKind;
  layer: RenderLayer;
  transform: Transform;
  visible: boolean;
  metadata: RenderMetadata;
  material: Material | null;
  meshPayload: MeshPayloadDescriptor | null;
}

interface MutablePrimitiveNode extends MutableNodeBase {
  kind: 'primitive';
  node: RenderNode;
}

interface MutableStaticMeshNode extends MutableNodeBase {
  kind: 'staticMesh';
  asset: string;
  instance: StaticMeshInstanceDescriptor;
  materialParameters: Map<number, MaterialInstanceParameters>;
}

interface MutableAnimatedMeshNode extends MutableNodeBase {
  kind: 'animatedMesh';
  asset: string;
  instance: AnimatedMeshInstanceDescriptor;
  playback: AnimatedMeshPlaybackCommand | null;
}

interface MutableVoxelObjectNode extends MutableNodeBase {
  kind: 'voxelObject';
  asset: string;
  instance: VoxelObjectInstanceDescriptor;
  frame: number;
}

interface MutableSpriteNode extends MutableNodeBase {
  kind: 'sprite';
  sprite: SpriteInstanceDescriptor;
  frameUv: [number, number, number, number];
  frameSize: [number, number];
  renderOrder: number;
}

interface StaticMeshRecord {
  asset: StaticMeshAsset;
  refCount: number;
}

interface AnimatedMeshRecord {
  asset: AnimatedMeshAsset;
  refCount: number;
}

interface VoxelObjectRecord {
  asset: VoxelObjectRenderAsset;
  refCount: number;
}

interface MutableLight {
  handle: RenderHandle;
  parent: RenderHandle | null;
  light: LightDescriptor;
}

/** A retained renderer-neutral projection driven only by render diffs. */
export class RenderProjection {
  #nodes = new Map<RenderHandle, NodeRecord>();
  #lights = new Map<RenderHandle, MutableLight>();
  #materials = new Map<string, RenderMaterialDescriptor>();
  #textures = new Map<string, TextureDescriptor>();
  #spriteAtlases = new Map<string, SpriteAtlasDescriptor>();
  #staticMeshes = new Map<string, StaticMeshRecord>();
  #animatedMeshes = new Map<string, AnimatedMeshRecord>();
  #voxelObjects = new Map<string, VoxelObjectRecord>();
  #publishedRevisions = new Map<string, number>();
  #stagingStatistics: MutableStagingStatistics = emptyStagingStatistics();
  #collectStagingStatistics = false;

  /**
   * Apply a frame in authored order and return renderer-neutral instructions.
   * The frame is fail-atomic: a rejected later operation cannot retain any
   * state from earlier operations in the same frame.
   */
  applyFrame(frame: RenderFrameDiff): readonly RenderProjectionInstruction[] {
    const { staged, instructions } = this.#stageFrame(frame);
    this.#replaceWith(staged);
    return instructions;
  }

  /**
   * Validate and project a complete frame against a private clone without
   * committing it. Backends use this as the first phase of a composed
   * transaction so a bad later operation cannot partially mutate rendering.
   */
  validateFrame(frame: RenderFrameDiff): readonly RenderProjectionInstruction[] {
    return this.#stageFrame(frame).instructions;
  }

  /** Apply one diff. Throws `RenderProjectionError` on stale handles or malformed payloads. */
  applyDiff(diff: RenderDiff): readonly RenderProjectionInstruction[] {
    validateOperationHandles(diff);
    switch (diff.op) {
      case 'create':
        return [this.#create(diff)];
      case 'update':
        return [this.#update(diff)];
      case 'destroy':
        return this.#destroy(diff.handle);
      case 'replaceMeshPayload':
        return [this.#replaceMeshPayload(diff)];
      case 'createLight':
        return [this.#createLight(diff)];
      case 'updateLight':
        return [this.#updateLight(diff)];
      case 'defineMaterial':
        return [this.#defineMaterial(diff.material)];
      case 'setMaterialInstanceParameters':
        return [this.#setMaterialInstanceParameters(diff)];
      case 'defineTexture':
        return [this.#defineTexture(diff.texture)];
      case 'defineSpriteAtlas':
        return [this.#defineSpriteAtlas(diff.atlas)];
      case 'defineStaticMesh':
        return [this.#defineStaticMesh(diff.asset)];
      case 'defineAnimatedMesh':
        return [this.#defineAnimatedMesh(diff.asset)];
      case 'defineVoxelObject':
        return [this.#defineVoxelObject(diff.asset)];
      case 'releaseVoxelObject':
        return [this.#releaseVoxelObject(diff.asset)];
      case 'createStaticMeshInstance':
        return [this.#createStaticMeshInstance(diff)];
      case 'createAnimatedMeshInstance':
        return [this.#createAnimatedMeshInstance(diff)];
      case 'setAnimatedMeshPlayback':
        return [this.#setAnimatedMeshPlayback(diff)];
      case 'createVoxelObjectInstance':
        return [this.#createVoxelObjectInstance(diff)];
      case 'setVoxelObjectFrame':
        return [this.#setVoxelObjectFrame(diff)];
      case 'createSprite':
        return [this.#createSprite(diff)];
      case 'updateSprite':
        return [this.#updateSprite(diff)];
      default: {
        const unknown = diff as { readonly op?: unknown };
        throw new RenderProjectionError(`unsupported render diff op ${JSON.stringify(unknown.op)}`);
      }
    }
  }

  has(handle: RenderHandle): boolean {
    return this.#nodes.has(handle) || this.#lights.has(handle);
  }

  get handleCount(): number {
    return this.#nodes.size + this.#lights.size;
  }

  lastFrameStagingStatistics(): RenderProjectionStagingStatistics {
    return { ...this.#stagingStatistics };
  }

  node(handle: RenderHandle): RenderProjectionNode | undefined {
    const record = this.#nodes.get(handle);
    return record === undefined ? undefined : snapshotNode(record);
  }

  light(handle: RenderHandle): RenderProjectionLight | undefined {
    const record = this.#lights.get(handle);
    return record === undefined ? undefined : snapshotLight(record);
  }

  materialDescriptor(id: string): RenderMaterialDescriptor | undefined {
    return clone(this.#materials.get(id));
  }

  textureDescriptor(id: string): TextureDescriptor | undefined {
    return clone(this.#textures.get(id));
  }

  spriteAtlas(id: string): SpriteAtlasDescriptor | undefined {
    return clone(this.#spriteAtlases.get(id));
  }

  staticMesh(asset: string): StaticMeshAsset | undefined {
    return clone(this.#staticMeshes.get(asset)?.asset);
  }

  animatedMesh(asset: string): AnimatedMeshAsset | undefined {
    return clone(this.#animatedMeshes.get(asset)?.asset);
  }

  voxelObject(asset: string): VoxelObjectRenderAsset | undefined {
    return clone(this.#voxelObjects.get(asset)?.asset);
  }

  staticMeshRefCount(asset: string): number {
    return this.#staticMeshes.get(asset)?.refCount ?? 0;
  }

  animatedMeshRefCount(asset: string): number {
    return this.#animatedMeshes.get(asset)?.refCount ?? 0;
  }

  voxelObjectRefCount(asset: string): number {
    return this.#voxelObjects.get(asset)?.refCount ?? 0;
  }

  snapshot(): RenderProjectionSnapshot {
    return {
      nodes: sortedHandles(this.#nodes).map((handle) => snapshotNode(this.#require(handle, 'snapshot'))),
      lights: sortedHandles(this.#lights).map((handle) => snapshotLight(this.#requireLight(handle, 'snapshot'))),
      materials: sortedValues(this.#materials),
      textures: sortedValues(this.#textures),
      spriteAtlases: sortedValues(this.#spriteAtlases),
      staticMeshes: [...this.#staticMeshes.values()]
        .map((record) => clone(record.asset))
        .sort((a, b) => a.asset.localeCompare(b.asset)),
      animatedMeshes: [...this.#animatedMeshes.values()]
        .map((record) => clone(record.asset))
        .sort((a, b) => a.asset.localeCompare(b.asset)),
      voxelObjects: [...this.#voxelObjects.values()]
        .map((record) => clone(record.asset))
        .sort((a, b) => a.asset.localeCompare(b.asset)),
    };
  }

  pickMesh(handle: RenderHandle): MeshPickHit | undefined {
    const record = this.#nodes.get(handle);
    const payload = record?.meshPayload;
    if (record === undefined || payload === undefined || payload === null) {
      return undefined;
    }
    return {
      handle,
      provenance: payload.provenance,
      sourceEntity: record.metadata.sourceEntity,
      sourceSceneNode: record.metadata.sourceSceneNode,
    };
  }

  pickSprite(handle: RenderHandle): SpritePickHit | undefined {
    const record = this.#nodes.get(handle);
    if (record?.kind !== 'sprite') {
      return undefined;
    }
    const attachment = record.sprite.attachment;
    return {
      handle,
      sourceEntity: attachment.sourceEntity,
      sourceSceneNode: attachment.sourceSceneNode,
      asset: record.sprite.asset,
      attachmentPoint: attachment.attachmentPoint,
    };
  }

  #create(diff: Extract<RenderDiff, { op: 'create' }>): RenderProjectionInstruction {
    this.#ensureFree(diff.handle, 'create');
    const parent = this.#parentHandle(diff.parent, 'create.parent');
    const node = clone(diff.node);
    const record: MutablePrimitiveNode = {
      handle: diff.handle,
      parent,
      children: new Set(),
      kind: 'primitive',
      layer: parent === null ? node.layer : this.#require(parent, 'create.parent').layer,
      transform: clone(node.transform),
      visible: node.visible,
      metadata: clone(node.metadata),
      material: clone(node.material),
      meshPayload: null,
      node,
    };
    this.#validateViewmodelInsertion(record, 'create');
    this.#insert(record);
    return { op: 'upsertNode', node: snapshotNode(record) };
  }

  #update(diff: Extract<RenderDiff, { op: 'update' }>): RenderProjectionInstruction {
    const current = this.#require(diff.handle, 'update');
    if (current.layer === 'viewmodel' && diff.transform !== null) {
      validateViewmodelTransform(diff.transform, 'update.transform');
    }
    const record = this.#mutableNode(diff.handle, 'update');
    if (diff.transform !== null) {
      record.transform = clone(diff.transform);
      if (record.kind === 'primitive') {
        record.node = { ...record.node, transform: clone(diff.transform) };
      } else if (record.kind === 'staticMesh') {
        record.instance = { ...record.instance, transform: clone(diff.transform) };
      } else if (record.kind === 'animatedMesh') {
        record.instance = { ...record.instance, transform: clone(diff.transform) };
      } else if (record.kind === 'voxelObject') {
        record.instance = { ...record.instance, transform: clone(diff.transform) };
      } else {
        record.sprite = { ...record.sprite, transform: clone(diff.transform) };
      }
    }
    if (diff.material !== null) {
      record.material = clone(diff.material);
      if (record.kind === 'primitive') {
        record.node = { ...record.node, material: clone(diff.material) };
      }
    }
    if (diff.visible !== null) {
      record.visible = diff.visible;
      if (record.kind === 'primitive') {
        record.node = { ...record.node, visible: diff.visible };
      } else if (record.kind === 'staticMesh') {
        record.instance = { ...record.instance, visible: diff.visible };
      } else if (record.kind === 'animatedMesh') {
        record.instance = { ...record.instance, visible: diff.visible };
      } else if (record.kind === 'voxelObject') {
        record.instance = { ...record.instance, visible: diff.visible };
      } else {
        record.sprite = { ...record.sprite, visible: diff.visible };
      }
    }
    if (diff.metadata !== null) {
      record.metadata = clone(diff.metadata);
      if (record.kind === 'primitive') {
        record.node = { ...record.node, metadata: clone(diff.metadata) };
      } else if (record.kind === 'staticMesh') {
        record.instance = { ...record.instance, metadata: clone(diff.metadata) };
      } else if (record.kind === 'animatedMesh') {
        record.instance = { ...record.instance, metadata: clone(diff.metadata) };
      } else if (record.kind === 'voxelObject') {
        record.instance = { ...record.instance, metadata: clone(diff.metadata) };
      } else {
        record.sprite = { ...record.sprite, metadata: clone(diff.metadata) };
      }
    }
    return { op: 'upsertNode', node: snapshotNode(record) };
  }

  #destroy(handle: RenderHandle): readonly RenderProjectionInstruction[] {
    const light = this.#lights.get(handle);
    if (light !== undefined) {
      this.#lights.delete(handle);
      if (light.parent !== null) {
        this.#mutableNode(light.parent, 'destroyLight.parent').children.delete(handle);
      }
      return [{ op: 'removeLight', handle }];
    }
    const record = this.#require(handle, 'destroy');
    const instructions: RenderProjectionInstruction[] = [];
    for (const child of [...record.children].sort(numberCompare)) {
      instructions.push(...this.#destroy(child));
    }
    this.#nodes.delete(handle);
    if (record.parent !== null) {
      this.#mutableNode(record.parent, 'destroy.parent').children.delete(handle);
    }
    if (record.kind === 'staticMesh') {
      const mesh = this.#mutableStaticMesh(record.asset);
      if (mesh !== undefined) {
        mesh.refCount -= 1;
      }
    } else if (record.kind === 'animatedMesh') {
      const mesh = this.#mutableAnimatedMesh(record.asset);
      if (mesh !== undefined) {
        mesh.refCount -= 1;
      }
    } else if (record.kind === 'voxelObject') {
      const object = this.#mutableVoxelObject(record.asset);
      if (object !== undefined) {
        object.refCount -= 1;
      }
    }
    instructions.push({ op: 'removeNode', handle });
    return instructions;
  }

  #replaceMeshPayload(
    diff: Extract<RenderDiff, { op: 'replaceMeshPayload' }>,
  ): RenderProjectionInstruction {
    const current = this.#require(diff.handle, 'replaceMeshPayload');
    if (current.kind !== 'primitive' || current.node.geometry.kind === 'group') {
      throw new RenderProjectionError(
        `replaceMeshPayload: handle ${diff.handle} is not a primitive mesh`,
      );
    }
    validateMeshPayload(diff.payload, 'replaceMeshPayload.payload');
    if (current.layer === 'viewmodel') {
      validateViewmodelBounds(diff.payload.bounds, 'replaceMeshPayload.payload.bounds');
    }
    const record = this.#mutableNode(diff.handle, 'replaceMeshPayload');
    if (record.kind !== 'primitive') {
      throw new RenderProjectionError(
        `replaceMeshPayload: handle ${diff.handle} is not a primitive mesh`,
      );
    }
    record.meshPayload = clone(diff.payload);
    return { op: 'upsertNode', node: snapshotNode(record) };
  }

  #createLight(diff: Extract<RenderDiff, { op: 'createLight' }>): RenderProjectionInstruction {
    this.#ensureFree(diff.handle, 'createLight');
    if (this.#lights.size >= MAX_RETAINED_LIGHTS) {
      throw new RenderProjectionError(
        `createLight: retained light quota ${String(MAX_RETAINED_LIGHTS)} exceeded`,
      );
    }
    const parent = this.#parentHandle(diff.parent, 'createLight.parent');
    if (parent !== null && this.#require(parent, 'createLight.parent').layer === 'viewmodel') {
      throw new RenderProjectionError(
        'createLight: camera-relative presentation uses the backend-owned neutral light rig',
      );
    }
    validateLight(diff.light, 'createLight.light');
    const record: MutableLight = {
      handle: diff.handle,
      parent,
      light: clone(diff.light),
    };
    this.#lights.set(diff.handle, record);
    if (parent !== null) {
      this.#mutableNode(parent, 'createLight.parent').children.add(diff.handle);
    }
    return { op: 'upsertLight', light: snapshotLight(record) };
  }

  #updateLight(diff: Extract<RenderDiff, { op: 'updateLight' }>): RenderProjectionInstruction {
    const current = this.#requireLight(diff.handle, 'updateLight');
    validateLight(diff.light, 'updateLight.light');
    if (current.light.kind !== diff.light.kind) {
      throw new RenderProjectionError(
        `updateLight: handle ${diff.handle} cannot change kind from ${current.light.kind} to ${diff.light.kind}`,
      );
    }
    const record = this.#mutableLight(diff.handle, 'updateLight');
    record.light = clone(diff.light);
    return { op: 'upsertLight', light: snapshotLight(record) };
  }

  #defineMaterial(material: RenderMaterialDescriptor): RenderProjectionInstruction {
    this.#materials.set(material.id, clone(material));
    return { op: 'defineMaterial', material: clone(material) };
  }

  #defineTexture(texture: TextureDescriptor): RenderProjectionInstruction {
    this.#textures.set(texture.id, clone(texture));
    return { op: 'defineTexture', texture: clone(texture) };
  }

  #defineSpriteAtlas(atlas: SpriteAtlasDescriptor): RenderProjectionInstruction {
    this.#spriteAtlases.set(atlas.id, clone(atlas));
    return { op: 'defineSpriteAtlas', atlas: clone(atlas) };
  }

  #defineStaticMesh(asset: StaticMeshAsset): RenderProjectionInstruction {
    validateMeshPayload(asset.payload, `defineStaticMesh(${asset.asset}).payload`);
    const existing = this.#staticMeshes.get(asset.asset);
    if (existing !== undefined && existing.refCount > 0) {
      throw new RenderProjectionError(
        `defineStaticMesh: asset ${asset.asset} is in use by ${existing.refCount} instance(s)`,
      );
    }
    this.#staticMeshes.set(asset.asset, { asset: clone(asset), refCount: 0 });
    return { op: 'defineStaticMesh', asset: clone(asset) };
  }

  #defineAnimatedMesh(asset: AnimatedMeshAsset): RenderProjectionInstruction {
    validateAnimatedMeshAsset(asset, `defineAnimatedMesh(${asset.asset})`);
    const existing = this.#animatedMeshes.get(asset.asset);
    if (existing !== undefined && existing.refCount > 0) {
      throw new RenderProjectionError(
        `defineAnimatedMesh: asset ${asset.asset} is in use by ${existing.refCount} instance(s)`,
      );
    }
    this.#animatedMeshes.set(asset.asset, { asset: clone(asset), refCount: 0 });
    return { op: 'defineAnimatedMesh', asset: clone(asset) };
  }

  #createStaticMeshInstance(
    diff: Extract<RenderDiff, { op: 'createStaticMeshInstance' }>,
  ): RenderProjectionInstruction {
    this.#ensureFree(diff.handle, 'createStaticMeshInstance');
    const asset = this.#staticMeshes.get(diff.instance.asset);
    if (asset === undefined) {
      throw new RenderProjectionError(
        `createStaticMeshInstance: undefined static mesh asset ${diff.instance.asset}`,
      );
    }
    const parent = this.#parentHandle(diff.parent, 'createStaticMeshInstance.parent');
    const instance = clone(diff.instance);
    const boundSlots = new Set(asset.asset.materialSlots.map((binding) => binding.slot));
    for (const override of instance.materialOverrides) {
      if (!boundSlots.has(override.slot)) {
        throw new RenderProjectionError(
          `createStaticMeshInstance: override for unbound slot ${override.slot} on ${instance.asset}`,
        );
      }
    }
    const record: MutableStaticMeshNode = {
      handle: diff.handle,
      parent,
      children: new Set(),
      kind: 'staticMesh',
      layer: parent === null ? 'scene' : this.#require(parent, 'createStaticMeshInstance.parent').layer,
      transform: clone(instance.transform),
      visible: instance.visible,
      metadata: clone(instance.metadata),
      material: null,
      meshPayload: clone(asset.asset.payload),
      asset: instance.asset,
      instance,
      materialParameters: new Map(),
    };
    this.#validateViewmodelInsertion(record, 'createStaticMeshInstance');
    this.#mutableStaticMesh(instance.asset)!.refCount += 1;
    this.#insert(record);
    return { op: 'upsertNode', node: snapshotNode(record) };
  }

  #setMaterialInstanceParameters(
    diff: Extract<RenderDiff, { op: 'setMaterialInstanceParameters' }>,
  ): RenderProjectionInstruction {
    const current = this.#require(diff.handle, 'setMaterialInstanceParameters');
    if (current.kind !== 'staticMesh') {
      throw new RenderProjectionError(
        `setMaterialInstanceParameters: handle ${diff.handle} is not a static mesh`,
      );
    }
    const asset = this.#staticMeshes.get(current.asset);
    if (asset === undefined || !asset.asset.materialSlots.some((binding) => binding.slot === diff.slot)) {
      throw new RenderProjectionError(
        `setMaterialInstanceParameters: unbound slot ${diff.slot} on ${current.asset}`,
      );
    }
    const record = this.#mutableNode(diff.handle, 'setMaterialInstanceParameters');
    if (record.kind !== 'staticMesh') {
      throw new RenderProjectionError(
        `setMaterialInstanceParameters: handle ${diff.handle} is not a static mesh`,
      );
    }
    if (diff.parameters === null) {
      record.materialParameters.delete(diff.slot);
    } else {
      record.materialParameters.set(diff.slot, clone(diff.parameters));
    }
    return { op: 'upsertNode', node: snapshotNode(record) };
  }

  #createAnimatedMeshInstance(
    diff: Extract<RenderDiff, { op: 'createAnimatedMeshInstance' }>,
  ): RenderProjectionInstruction {
    this.#ensureFree(diff.handle, 'createAnimatedMeshInstance');
    const asset = this.#animatedMeshes.get(diff.instance.asset);
    if (asset === undefined) {
      throw new RenderProjectionError(
        `createAnimatedMeshInstance: undefined animated mesh asset ${diff.instance.asset}`,
      );
    }
    if (diff.instance.playback !== null) {
      validatePlaybackCommand(asset.asset, diff.instance.playback, 'createAnimatedMeshInstance.playback');
    }
    const parent = this.#parentHandle(diff.parent, 'createAnimatedMeshInstance.parent');
    const instance = clone(diff.instance);
    const record: MutableAnimatedMeshNode = {
      handle: diff.handle,
      parent,
      children: new Set(),
      kind: 'animatedMesh',
      layer: parent === null ? 'scene' : this.#require(parent, 'createAnimatedMeshInstance.parent').layer,
      transform: clone(instance.transform),
      visible: instance.visible,
      metadata: clone(instance.metadata),
      material: null,
      meshPayload: null,
      asset: instance.asset,
      instance,
      playback: clone(instance.playback),
    };
    this.#validateViewmodelInsertion(record, 'createAnimatedMeshInstance');
    this.#mutableAnimatedMesh(instance.asset)!.refCount += 1;
    this.#insert(record);
    return { op: 'upsertNode', node: snapshotNode(record) };
  }

  #setAnimatedMeshPlayback(
    diff: Extract<RenderDiff, { op: 'setAnimatedMeshPlayback' }>,
  ): RenderProjectionInstruction {
    const current = this.#require(diff.handle, 'setAnimatedMeshPlayback');
    if (current.kind !== 'animatedMesh') {
      throw new RenderProjectionError(`setAnimatedMeshPlayback: handle ${diff.handle} is not an animated mesh`);
    }
    const asset = this.#animatedMeshes.get(current.asset);
    if (asset === undefined) {
      throw new RenderProjectionError(`setAnimatedMeshPlayback: missing animated mesh asset ${current.asset}`);
    }
    validatePlaybackCommand(asset.asset, diff.playback, 'setAnimatedMeshPlayback.playback');
    const record = this.#mutableNode(diff.handle, 'setAnimatedMeshPlayback');
    if (record.kind !== 'animatedMesh') {
      throw new RenderProjectionError(`setAnimatedMeshPlayback: handle ${diff.handle} is not an animated mesh`);
    }
    record.playback = clone(diff.playback);
    record.instance = { ...record.instance, playback: clone(diff.playback) };
    return { op: 'upsertNode', node: snapshotNode(record) };
  }

  #defineVoxelObject(asset: VoxelObjectRenderAsset): RenderProjectionInstruction {
    validateVoxelObjectAsset(asset, `defineVoxelObject(${asset.asset})`);
    const existing = this.#voxelObjects.get(asset.asset);
    const liveUpdates: Array<{
      readonly payload: MeshPayloadDescriptor;
      readonly handle: RenderHandle;
    }> = [];
    if (existing !== undefined) {
      for (const record of this.#nodes.values()) {
        if (record.kind !== 'voxelObject' || record.asset !== asset.asset) continue;
        validateVoxelObjectFrame(asset, record.frame, 'defineVoxelObject.liveInstance');
        validateVoxelObjectOverrides(asset, record.instance.materialOverrides, 'defineVoxelObject.liveInstance');
        const payload = asset.meshes[asset.frames[record.frame]!.mesh]!.payload;
        if (record.layer === 'viewmodel') {
          validateViewmodelBounds(payload.bounds, 'defineVoxelObject.liveInstance.bounds');
        }
        liveUpdates.push({ payload, handle: record.handle });
      }
    }
    for (const update of liveUpdates) {
      const record = this.#mutableNode(update.handle, 'defineVoxelObject.liveInstance');
      if (record.kind !== 'voxelObject') {
        throw new RenderProjectionError(
          `defineVoxelObject.liveInstance: handle ${update.handle} is not a voxel object`,
        );
      }
      record.meshPayload = clone(update.payload);
    }
    this.#voxelObjects.set(asset.asset, {
      asset: clone(asset),
      refCount: existing?.refCount ?? 0,
    });
    return { op: 'defineVoxelObject', asset: clone(asset) };
  }

  #releaseVoxelObject(asset: string): RenderProjectionInstruction {
    const existing = this.#voxelObjects.get(asset);
    if (existing === undefined) {
      throw new RenderProjectionError(`releaseVoxelObject: undefined voxel object ${asset}`);
    }
    if (existing.refCount !== 0) {
      throw new RenderProjectionError(
        `releaseVoxelObject: ${asset} is in use by ${existing.refCount} instance(s)`,
      );
    }
    this.#voxelObjects.delete(asset);
    return { op: 'releaseVoxelObject', asset };
  }

  #createVoxelObjectInstance(
    diff: Extract<RenderDiff, { op: 'createVoxelObjectInstance' }>,
  ): RenderProjectionInstruction {
    this.#ensureFree(diff.handle, 'createVoxelObjectInstance');
    const asset = this.#voxelObjects.get(diff.instance.asset);
    if (asset === undefined) {
      throw new RenderProjectionError(
        `createVoxelObjectInstance: undefined voxel object ${diff.instance.asset}`,
      );
    }
    validateVoxelObjectFrame(asset.asset, diff.instance.frame, 'createVoxelObjectInstance.frame');
    validateVoxelObjectOverrides(
      asset.asset,
      diff.instance.materialOverrides,
      'createVoxelObjectInstance.materialOverrides',
    );
    const parent = this.#parentHandle(diff.parent, 'createVoxelObjectInstance.parent');
    const instance = clone(diff.instance);
    const record: MutableVoxelObjectNode = {
      handle: diff.handle,
      parent,
      children: new Set(),
      kind: 'voxelObject',
      layer: parent === null ? 'scene' : this.#require(parent, 'createVoxelObjectInstance.parent').layer,
      transform: clone(instance.transform),
      visible: instance.visible,
      metadata: clone(instance.metadata),
      material: null,
      meshPayload: clone(asset.asset.meshes[asset.asset.frames[instance.frame]!.mesh]!.payload),
      asset: instance.asset,
      instance,
      frame: instance.frame,
    };
    this.#validateViewmodelInsertion(record, 'createVoxelObjectInstance');
    this.#mutableVoxelObject(instance.asset)!.refCount += 1;
    this.#insert(record);
    return { op: 'upsertNode', node: snapshotNode(record) };
  }

  #setVoxelObjectFrame(
    diff: Extract<RenderDiff, { op: 'setVoxelObjectFrame' }>,
  ): RenderProjectionInstruction {
    const current = this.#require(diff.handle, 'setVoxelObjectFrame');
    if (current.kind !== 'voxelObject') {
      throw new RenderProjectionError(
        `setVoxelObjectFrame: handle ${diff.handle} is not a voxel object`,
      );
    }
    const asset = this.#voxelObjects.get(current.asset);
    if (asset === undefined) {
      throw new RenderProjectionError(`setVoxelObjectFrame: missing voxel object ${current.asset}`);
    }
    validateVoxelObjectFrame(asset.asset, diff.frame, 'setVoxelObjectFrame.frame');
    const payload = asset.asset.meshes[asset.asset.frames[diff.frame]!.mesh]!.payload;
    if (current.layer === 'viewmodel') {
      validateViewmodelBounds(payload.bounds, 'setVoxelObjectFrame.bounds');
    }
    const record = this.#mutableNode(diff.handle, 'setVoxelObjectFrame');
    if (record.kind !== 'voxelObject') {
      throw new RenderProjectionError(
        `setVoxelObjectFrame: handle ${diff.handle} is not a voxel object`,
      );
    }
    record.frame = diff.frame;
    record.instance = { ...record.instance, frame: diff.frame };
    record.meshPayload = clone(payload);
    return { op: 'upsertNode', node: snapshotNode(record) };
  }

  #createSprite(diff: Extract<RenderDiff, { op: 'createSprite' }>): RenderProjectionInstruction {
    this.#ensureFree(diff.handle, 'createSprite');
    const parent = this.#parentHandle(diff.parent, 'createSprite.parent');
    const sprite = clone(diff.sprite);
    const record: MutableSpriteNode = {
      handle: diff.handle,
      parent,
      children: new Set(),
      kind: 'sprite',
      layer: parent === null ? 'scene' : this.#require(parent, 'createSprite.parent').layer,
      transform: clone(sprite.transform),
      visible: sprite.visible,
      metadata: clone(sprite.metadata),
      material: null,
      meshPayload: null,
      sprite,
      frameUv: this.#resolveSpriteUv(sprite.asset, sprite.frame),
      frameSize: this.#resolveSpriteSize(sprite.asset, sprite.frame, sprite.size),
      renderOrder: sprite.renderOrder,
    };
    this.#validateViewmodelInsertion(record, 'createSprite');
    this.#insert(record);
    return { op: 'upsertNode', node: snapshotNode(record) };
  }

  #updateSprite(diff: Extract<RenderDiff, { op: 'updateSprite' }>): RenderProjectionInstruction {
    const current = this.#require(diff.handle, 'updateSprite');
    if (current.kind !== 'sprite') {
      throw new RenderProjectionError(`updateSprite: handle ${diff.handle} is not a sprite`);
    }
    const record = this.#mutableNode(diff.handle, 'updateSprite');
    if (record.kind !== 'sprite') {
      throw new RenderProjectionError(`updateSprite: handle ${diff.handle} is not a sprite`);
    }
    if (diff.frame !== null) {
      record.sprite = { ...record.sprite, frame: diff.frame };
      record.frameUv = this.#resolveSpriteUv(record.sprite.asset, diff.frame);
      record.frameSize = this.#resolveSpriteSize(
        record.sprite.asset,
        diff.frame,
        record.sprite.size,
      );
    }
    if (diff.tint !== null) {
      record.sprite = { ...record.sprite, tint: clone(diff.tint) };
    }
    if (diff.renderOrder !== null) {
      record.sprite = { ...record.sprite, renderOrder: diff.renderOrder };
      record.renderOrder = diff.renderOrder;
    }
    if (diff.visible !== null) {
      record.visible = diff.visible;
      record.sprite = { ...record.sprite, visible: diff.visible };
    }
    return { op: 'upsertNode', node: snapshotNode(record) };
  }

  #resolveSpriteUv(asset: string, frame: number): [number, number, number, number] {
    const atlas = this.#spriteAtlases.get(asset);
    const rect = atlas?.frames.find((candidate) => candidate.frame === frame);
    if (rect === undefined) {
      return [0, 0, 1, 1];
    }
    return [rect.uvMin[0], rect.uvMin[1], rect.uvMax[0], rect.uvMax[1]];
  }

  #resolveSpriteSize(asset: string, frame: number, fallback: readonly [number, number]): [number, number] {
    const rect = this.#spriteAtlases.get(asset)?.frames.find((candidate) => candidate.frame === frame);
    return rect?.size === undefined ? [fallback[0], fallback[1]] : [rect.size[0], rect.size[1]];
  }

  #insert(record: NodeRecord): void {
    this.#nodes.set(record.handle, record);
    if (record.parent !== null) {
      this.#mutableNode(record.parent, 'insert.parent').children.add(record.handle);
    }
  }

  #validateViewmodelInsertion(record: NodeRecord, ctx: string): void {
    if (record.layer !== 'viewmodel') {
      return;
    }
    validateViewmodelTransform(record.transform, `${ctx}.transform`);
    this.#validateViewmodelAsset(record, ctx);
    const liveViewmodelNodes = [...this.#nodes.values()]
      .filter((candidate) => candidate.layer === 'viewmodel');
    if (liveViewmodelNodes.length >= MAX_VIEWMODEL_NODES) {
      throw new RenderProjectionError(
        `${ctx}: viewmodel node capacity ${MAX_VIEWMODEL_NODES} is exhausted`,
      );
    }
    const assetKey = viewmodelAssetKey(record);
    if (assetKey === null) {
      return;
    }
    const assets = new Set(
      liveViewmodelNodes
        .map(viewmodelAssetKey)
        .filter((candidate): candidate is string => candidate !== null),
    );
    if (!assets.has(assetKey) && assets.size >= MAX_VIEWMODEL_DISTINCT_ASSETS) {
      throw new RenderProjectionError(
        `${ctx}: viewmodel asset capacity ${MAX_VIEWMODEL_DISTINCT_ASSETS} is exhausted`,
      );
    }
  }

  #validateViewmodelAsset(record: NodeRecord, ctx: string): void {
    if (record.kind === 'primitive') {
      if (record.node.geometry.kind === 'line') {
        validateViewmodelPoints(
          [record.node.geometry.a, record.node.geometry.b],
          `${ctx}.geometry`,
        );
      }
      if (record.meshPayload !== null) {
        validateViewmodelBounds(record.meshPayload.bounds, `${ctx}.meshPayload.bounds`);
      }
      return;
    }
    if (record.kind === 'animatedMesh') {
      const asset = this.#animatedMeshes.get(record.asset);
      if (asset === undefined) {
        throw new RenderProjectionError(`${ctx}: missing animated mesh asset ${record.asset}`);
      }
      validateViewmodelBounds(asset.asset.bounds, `${ctx}.asset.bounds`);
      return;
    }
    if (record.kind === 'sprite') {
      if (record.sprite.size.some((component) => component > MAX_VIEWMODEL_ASSET_EXTENT)) {
        throw new RenderProjectionError(
          `${ctx}.sprite.size: viewmodel dimensions must not exceed ${MAX_VIEWMODEL_ASSET_EXTENT}`,
        );
      }
      return;
    }
    if (record.meshPayload !== null) {
      validateViewmodelBounds(record.meshPayload.bounds, `${ctx}.asset.bounds`);
    }
  }

  #ensureFree(handle: RenderHandle, ctx: string): void {
    if (this.#nodes.has(handle) || this.#lights.has(handle)) {
      throw new RenderProjectionError(`${ctx}: handle ${handle} already exists`);
    }
  }

  #parentHandle(parent: RenderHandle | null, ctx: string): RenderHandle | null {
    if (parent !== null) {
      this.#require(parent, ctx);
    }
    return parent;
  }

  #require(handle: RenderHandle, ctx: string): NodeRecord {
    const record = this.#nodes.get(handle);
    if (record === undefined) {
      throw new RenderProjectionError(`${ctx}: unknown handle ${handle}`);
    }
    return record;
  }

  #requireLight(handle: RenderHandle, ctx: string): MutableLight {
    const record = this.#lights.get(handle);
    if (record === undefined) {
      throw new RenderProjectionError(`${ctx}: unknown light handle ${handle}`);
    }
    return record;
  }

  #mutableNode(handle: RenderHandle, ctx: string): NodeRecord {
    const record = copyNodeRecord(this.#require(handle, ctx));
    this.#nodes.set(handle, record);
    if (this.#collectStagingStatistics) {
      this.#stagingStatistics.copiedNodeRecords += 1;
    }
    return record;
  }

  #mutableLight(handle: RenderHandle, ctx: string): MutableLight {
    const record = { ...this.#requireLight(handle, ctx) };
    this.#lights.set(handle, record);
    if (this.#collectStagingStatistics) {
      this.#stagingStatistics.copiedLightRecords += 1;
    }
    return record;
  }

  #mutableStaticMesh(asset: string): StaticMeshRecord | undefined {
    const current = this.#staticMeshes.get(asset);
    if (current === undefined) return undefined;
    const record = { ...current };
    this.#staticMeshes.set(asset, record);
    if (this.#collectStagingStatistics) {
      this.#stagingStatistics.copiedResourceRecords += 1;
    }
    return record;
  }

  #mutableAnimatedMesh(asset: string): AnimatedMeshRecord | undefined {
    const current = this.#animatedMeshes.get(asset);
    if (current === undefined) return undefined;
    const record = { ...current };
    this.#animatedMeshes.set(asset, record);
    if (this.#collectStagingStatistics) {
      this.#stagingStatistics.copiedResourceRecords += 1;
    }
    return record;
  }

  #mutableVoxelObject(asset: string): VoxelObjectRecord | undefined {
    const current = this.#voxelObjects.get(asset);
    if (current === undefined) return undefined;
    const record = { ...current };
    this.#voxelObjects.set(asset, record);
    if (this.#collectStagingStatistics) {
      this.#stagingStatistics.copiedResourceRecords += 1;
    }
    return record;
  }

  #fork(): RenderProjection {
    const projection = new RenderProjection();
    projection.#nodes = new Map(this.#nodes);
    projection.#lights = new Map(this.#lights);
    projection.#materials = new Map(this.#materials);
    projection.#textures = new Map(this.#textures);
    projection.#spriteAtlases = new Map(this.#spriteAtlases);
    projection.#staticMeshes = new Map(this.#staticMeshes);
    projection.#animatedMeshes = new Map(this.#animatedMeshes);
    projection.#voxelObjects = new Map(this.#voxelObjects);
    projection.#publishedRevisions = new Map(this.#publishedRevisions);
    projection.#stagingStatistics = {
      ...emptyStagingStatistics(),
      sharedDefinitionRecords:
        this.#materials.size
        + this.#textures.size
        + this.#spriteAtlases.size
        + this.#staticMeshes.size
        + this.#animatedMeshes.size
        + this.#voxelObjects.size,
    };
    projection.#collectStagingStatistics = true;
    return projection;
  }

  #replaceWith(projection: RenderProjection): void {
    this.#nodes = projection.#nodes;
    this.#lights = projection.#lights;
    this.#materials = projection.#materials;
    this.#textures = projection.#textures;
    this.#spriteAtlases = projection.#spriteAtlases;
    this.#staticMeshes = projection.#staticMeshes;
    this.#animatedMeshes = projection.#animatedMeshes;
    this.#voxelObjects = projection.#voxelObjects;
    this.#publishedRevisions = projection.#publishedRevisions;
    this.#stagingStatistics = projection.#stagingStatistics;
    this.#collectStagingStatistics = false;
  }

  #stageFrame(frame: RenderFrameDiff): {
    readonly staged: RenderProjection;
    readonly instructions: readonly RenderProjectionInstruction[];
  } {
    const staged = this.#fork();
    if (frame.publication !== undefined) {
      if (frame.publication.operationCount !== frame.ops.length) {
        throw new RenderProjectionError(
          `publication ${frame.publication.stream} operationCount does not match frame`,
        );
      }
      if (frame.publication.revision !== frame.publication.baseRevision + 1) {
        throw new RenderProjectionError(
          `publication gap for ${frame.publication.stream}; revision ${String(frame.publication.revision)} must immediately follow base ${String(frame.publication.baseRevision)}`,
        );
      }
      const previous = staged.#publishedRevisions.get(frame.publication.stream);
      if (previous !== undefined && frame.publication.revision <= previous) {
        throw new RenderProjectionError(
          `stale publication ${frame.publication.stream} revision ${String(frame.publication.revision)}; latest is ${String(previous)}`,
        );
      }
      const expectedBase = previous ?? 0;
      if (frame.publication.baseRevision !== expectedBase) {
        throw new RenderProjectionError(
          `publication gap for ${frame.publication.stream}; expected base ${String(expectedBase)}, received ${String(frame.publication.baseRevision)}`,
        );
      }
      staged.#publishedRevisions.set(frame.publication.stream, frame.publication.revision);
    }
    const instructions: RenderProjectionInstruction[] = [];
    for (const diff of frame.ops) {
      instructions.push(...staged.applyDiff(diff));
    }
    return { staged, instructions };
  }
}

function copyNodeRecord(record: NodeRecord): NodeRecord {
  const children = new Set(record.children);
  if (record.kind === 'staticMesh') {
    return {
      ...record,
      children,
      materialParameters: new Map(record.materialParameters),
    };
  }
  return { ...record, children };
}

function validateViewmodelTransform(transform: Transform, ctx: string): void {
  const translationOutOfRange = transform.translation.some(
    (component) => Math.abs(component) > MAX_VIEWMODEL_TRANSLATION_COMPONENT,
  );
  if (translationOutOfRange) {
    throw new RenderProjectionError(
      `${ctx}: viewmodel translation components must be within +/−${MAX_VIEWMODEL_TRANSLATION_COMPONENT}`,
    );
  }
  const rotationOutOfRange = transform.rotation.some(
    (component) => Math.abs(component) > MAX_VIEWMODEL_ROTATION_COMPONENT,
  );
  if (rotationOutOfRange) {
    throw new RenderProjectionError(
      `${ctx}: viewmodel rotation components must be within +/−${MAX_VIEWMODEL_ROTATION_COMPONENT}`,
    );
  }
  const scaleOutOfRange = transform.scale.some(
    (component) => Math.abs(component) > MAX_VIEWMODEL_SCALE_COMPONENT,
  );
  if (scaleOutOfRange) {
    throw new RenderProjectionError(
      `${ctx}: viewmodel scale components must be within +/−${MAX_VIEWMODEL_SCALE_COMPONENT}`,
    );
  }
}

function validateViewmodelBounds(
  bounds: MeshPayloadDescriptor['bounds'],
  ctx: string,
): void {
  validateViewmodelPoints([bounds.min, bounds.max], ctx);
}

function validateViewmodelPoints(
  points: readonly (readonly [number, number, number])[],
  ctx: string,
): void {
  if (points.some((point) =>
    point.some((component) => Math.abs(component) > MAX_VIEWMODEL_ASSET_EXTENT))) {
    throw new RenderProjectionError(
      `${ctx}: viewmodel asset coordinates must be within +/−${MAX_VIEWMODEL_ASSET_EXTENT}`,
    );
  }
}

function viewmodelAssetKey(record: NodeRecord): string | null {
  switch (record.kind) {
    case 'primitive':
      return null;
    case 'staticMesh':
      return `staticMesh:${record.asset}`;
    case 'animatedMesh':
      return `animatedMesh:${record.asset}`;
    case 'voxelObject':
      return `voxelObject:${record.asset}`;
    case 'sprite':
      return `sprite:${record.sprite.asset}`;
  }
}

function emptyStagingStatistics(): MutableStagingStatistics {
  return {
    copiedNodeRecords: 0,
    copiedLightRecords: 0,
    copiedResourceRecords: 0,
    sharedDefinitionRecords: 0,
  };
}

function snapshotLight(record: MutableLight): RenderProjectionLight {
  return { handle: record.handle, parent: record.parent, light: clone(record.light) };
}

function snapshotNode(record: NodeRecord): RenderProjectionNode {
  const base = {
    handle: record.handle,
    parent: record.parent,
    children: [...record.children].sort(numberCompare),
    layer: record.layer,
    transform: clone(record.transform),
    visible: record.visible,
    metadata: clone(record.metadata),
    material: clone(record.material),
    meshPayload: clone(record.meshPayload),
  };
  if (record.kind === 'primitive') {
    return { ...base, kind: 'primitive', node: clone(record.node) };
  }
  if (record.kind === 'staticMesh') {
    return {
      ...base,
      kind: 'staticMesh',
      asset: record.asset,
      instance: clone(record.instance),
      materialParameters: [...record.materialParameters.entries()]
        .sort(([left], [right]) => left - right)
        .map(([slot, parameters]) => ({ slot, parameters: clone(parameters) })),
    };
  }
  if (record.kind === 'animatedMesh') {
    return {
      ...base,
      kind: 'animatedMesh',
      asset: record.asset,
      instance: clone(record.instance),
      playback: clone(record.playback),
    };
  }
  if (record.kind === 'voxelObject') {
    return {
      ...base,
      kind: 'voxelObject',
      asset: record.asset,
      instance: clone(record.instance),
      frame: record.frame,
    };
  }
  return {
    ...base,
    kind: 'sprite',
    sprite: clone(record.sprite),
    frameUv: clone(record.frameUv),
    frameSize: clone(record.frameSize),
    renderOrder: record.renderOrder,
  };
}

function validateAnimatedMeshAsset(asset: AnimatedMeshAsset, ctx: string): void {
  if (asset.asset.length === 0) {
    throw new RenderProjectionError(`${ctx}.asset must be non-empty`);
  }
  if (asset.runtimeFormat !== 'glb') {
    throw new RenderProjectionError(`${ctx}.runtimeFormat unsupported: ${asset.runtimeFormat}`);
  }
  const clips = new Set<string>();
  for (let i = 0; i < asset.clips.length; i += 1) {
    const clip = asset.clips[i]!;
    if (clip.id.length === 0) {
      throw new RenderProjectionError(`${ctx}.clips[${i}].id must be non-empty`);
    }
    if (clips.has(clip.id)) {
      throw new RenderProjectionError(`${ctx}.clips duplicate clip ${clip.id}`);
    }
    clips.add(clip.id);
  }
  if (asset.defaultClip !== null && !clips.has(asset.defaultClip)) {
    throw new RenderProjectionError(`${ctx}.defaultClip ${asset.defaultClip} is not declared`);
  }
  const materialSlots = new Set<number>();
  for (let i = 0; i < asset.materialSlots.length; i += 1) {
    const slot = requireNonNegativeInteger(asset.materialSlots[i]!.slot, `${ctx}.materialSlots[${i}].slot`);
    if (materialSlots.has(slot)) {
      throw new RenderProjectionError(`${ctx}.materialSlots duplicate slot ${slot}`);
    }
    materialSlots.add(slot);
  }
}

function validateVoxelObjectAsset(asset: VoxelObjectRenderAsset, ctx: string): void {
  if (asset.asset.length === 0 || asset.contentHash.length === 0) {
    throw new RenderProjectionError(`${ctx} asset and contentHash must be non-empty`);
  }
  if (asset.meshes.length === 0 || asset.meshes.length > 8_193) {
    throw new RenderProjectionError(`${ctx}.meshes must contain 1..=8193 entries`);
  }
  if (asset.frames.length === 0 || asset.frames.length > 8_193) {
    throw new RenderProjectionError(`${ctx}.frames must contain 1..=8193 entries`);
  }
  const slots = new Set<number>();
  asset.materialSlots.forEach((binding, index) => {
    const slot = requireNonNegativeInteger(binding.slot, `${ctx}.materialSlots[${index}].slot`);
    if (slots.has(slot)) {
      throw new RenderProjectionError(`${ctx}.materialSlots duplicate slot ${slot}`);
    }
    slots.add(slot);
  });
  let totalVertices = 0;
  let totalIndices = 0;
  asset.meshes.forEach((mesh, index) => {
    validateMeshPayload(mesh.payload, `${ctx}.meshes[${index}].payload`);
    totalVertices += mesh.payload.layout.vertexCount;
    totalIndices += mesh.payload.layout.indexCount;
    mesh.payload.groups.forEach((group, groupIndex) => {
      if (!slots.has(group.materialSlot)) {
        throw new RenderProjectionError(
          `${ctx}.meshes[${index}].payload.groups[${groupIndex}] uses unbound slot ${group.materialSlot}`,
        );
      }
    });
  });
  if (totalVertices > 8_000_000 || totalIndices > 12_000_000) {
    throw new RenderProjectionError(`${ctx}.meshes exceeds aggregate vertex/index work limits`);
  }
  const frameIds = new Set<string>();
  asset.frames.forEach((frame, index) => {
    if (frame.id.length === 0 || frameIds.has(frame.id)) {
      throw new RenderProjectionError(`${ctx}.frames[${index}].id must be non-empty and unique`);
    }
    frameIds.add(frame.id);
    validateVoxelObjectFrame(asset, index, `${ctx}.frames[${index}]`);
  });
}

function validateVoxelObjectFrame(asset: VoxelObjectRenderAsset, frame: number, ctx: string): void {
  const index = requireNonNegativeInteger(frame, ctx);
  const descriptor = asset.frames[index];
  if (descriptor === undefined || asset.meshes[descriptor.mesh] === undefined) {
    throw new RenderProjectionError(
      `${ctx} ${index} is outside voxel object ${asset.asset} frame resources`,
    );
  }
}

function validateVoxelObjectOverrides(
  asset: VoxelObjectRenderAsset,
  overrides: VoxelObjectInstanceDescriptor['materialOverrides'],
  ctx: string,
): void {
  const slots = new Set(asset.materialSlots.map((binding) => binding.slot));
  const seen = new Set<number>();
  overrides.forEach((binding, index) => {
    if (seen.has(binding.slot)) {
      throw new RenderProjectionError(`${ctx}[${index}] duplicates slot ${binding.slot}`);
    }
    if (!slots.has(binding.slot)) {
      throw new RenderProjectionError(`${ctx}[${index}] uses unbound slot ${binding.slot}`);
    }
    seen.add(binding.slot);
  });
}

function validatePlaybackCommand(
  asset: AnimatedMeshAsset,
  command: AnimatedMeshPlaybackCommand,
  ctx: string,
): void {
  if (command.kind !== 'play') {
    return;
  }
  if (!asset.clips.some((clip) => clip.id === command.clip)) {
    throw new RenderProjectionError(`${ctx}.clip ${command.clip} is not defined on ${asset.asset}`);
  }
  if (command.speed <= 0) {
    throw new RenderProjectionError(`${ctx}.speed must be positive`);
  }
  if (command.weight < 0 || command.weight > 1) {
    throw new RenderProjectionError(`${ctx}.weight must be in 0..=1`);
  }
}

function validateLight(light: LightDescriptor, ctx: string): void {
  requireColor(light.color, `${ctx}.color`);
  requireFiniteNonNegative(light.intensity, `${ctx}.intensity`);
  if (light.intensity > MAX_RENDER_LIGHT_INTENSITY) {
    throw new RenderProjectionError(
      `${ctx}.intensity must not exceed ${String(MAX_RENDER_LIGHT_INTENSITY)}`,
    );
  }
  if (light.kind === 'directional') {
    requireDirection(light.direction, `${ctx}.direction`);
    return;
  }
  if (light.kind === 'point' || light.kind === 'spot') {
    light.position.forEach((value, index) => requireFinite(value, `${ctx}.position[${index}]`));
    if (light.range !== null && (!Number.isFinite(light.range) || light.range <= 0)) {
      throw new RenderProjectionError(`${ctx}.range must be null or finite and positive`);
    }
    requireFiniteNonNegative(light.decay, `${ctx}.decay`);
  }
  if (light.kind === 'spot') {
    requireDirection(light.direction, `${ctx}.direction`);
    if (!Number.isFinite(light.outerAngleRadians)
      || light.outerAngleRadians <= 0
      || light.outerAngleRadians > Math.PI / 2) {
      throw new RenderProjectionError(`${ctx}.outerAngleRadians must be in (0, pi/2]`);
    }
    if (!Number.isFinite(light.penumbra) || light.penumbra < 0 || light.penumbra > 1) {
      throw new RenderProjectionError(`${ctx}.penumbra must be in 0..=1`);
    }
  }
}

function requireColor(color: readonly number[], ctx: string): void {
  color.forEach((value, index) => {
    if (!Number.isFinite(value) || value < 0 || value > 1) {
      throw new RenderProjectionError(`${ctx}[${index}] must be finite and in 0..=1`);
    }
  });
}

function requireDirection(direction: readonly number[], ctx: string): void {
  direction.forEach((value, index) => requireFinite(value, `${ctx}[${index}]`));
  if (direction.reduce((sum, value) => sum + value * value, 0) <= Number.EPSILON) {
    throw new RenderProjectionError(`${ctx} must be non-zero`);
  }
}

function requireFinite(value: number, ctx: string): void {
  if (!Number.isFinite(value)) {
    throw new RenderProjectionError(`${ctx} must be finite`);
  }
}

function requireFiniteNonNegative(value: number, ctx: string): void {
  if (!Number.isFinite(value) || value < 0) {
    throw new RenderProjectionError(`${ctx} must be finite and non-negative`);
  }
}

function validateMeshPayload(payload: MeshPayloadDescriptor, ctx: string): void {
  const vertexCount = requireNonNegativeInteger(payload.layout.vertexCount, `${ctx}.layout.vertexCount`);
  const indexCount = requireNonNegativeInteger(payload.layout.indexCount, `${ctx}.layout.indexCount`);
  const positionComponents = attributeComponents(payload, 'position', ctx);
  const normalComponents = attributeComponents(payload, 'normal', ctx);
  const uvAttribute = payload.layout.attributes.find((attribute) => attribute.name === 'uv');
  const hasUvs = uvAttribute !== undefined;

  if (payload.source.kind === 'inline') {
    requireLength(payload.source.positions, vertexCount * positionComponents, `${ctx}.source.positions`);
    requireLength(payload.source.normals, vertexCount * normalComponents, `${ctx}.source.normals`);
    if (hasUvs !== (payload.source.uvs !== undefined)) {
      throw new RenderProjectionError(`${ctx}.source.uvs must match the declared uv attribute`);
    }
    if (payload.source.uvs !== undefined) {
      requireLength(payload.source.uvs, vertexCount * 2, `${ctx}.source.uvs`);
      payload.source.uvs.forEach((value, index) => requireFinite(value, `${ctx}.source.uvs[${index}]`));
      if ((payload.provenance === 'voxelChunk' || payload.provenance === 'voxelObject')
        && payload.source.uvs.some((value) => Math.abs(value) > 16_777_216)) {
        throw new RenderProjectionError(`${ctx}.source.uvs exceeds the voxel tile-coordinate range`);
      }
    }
    requireLength(payload.source.indices, indexCount, `${ctx}.source.indices`);
    payload.source.indices.forEach((index, i) => {
      const value = requireNonNegativeInteger(index, `${ctx}.source.indices[${i}]`);
      if (value >= vertexCount) {
        throw new RenderProjectionError(
          `${ctx}.source.indices[${i}] ${value} is out of range for ${vertexCount} vertices`,
        );
      }
    });
  } else if (payload.source.kind === 'sharedBuffer') {
    requireNonNegativeInteger(payload.source.buffer, `${ctx}.source.buffer`);
    requireNonNegativeInteger(payload.source.positionsByteOffset, `${ctx}.source.positionsByteOffset`);
    requireNonNegativeInteger(payload.source.normalsByteOffset, `${ctx}.source.normalsByteOffset`);
    if (hasUvs !== (payload.source.uvsByteOffset !== undefined)) {
      throw new RenderProjectionError(`${ctx}.source.uvsByteOffset must match the declared uv attribute`);
    }
    if (payload.source.uvsByteOffset !== undefined) {
      requireNonNegativeInteger(payload.source.uvsByteOffset, `${ctx}.source.uvsByteOffset`);
    }
    requireNonNegativeInteger(payload.source.indicesByteOffset, `${ctx}.source.indicesByteOffset`);
  } else {
    const digest = /^sha256:([0-9a-f]{64})$/u.exec(payload.source.contentHash)?.[1];
    if (digest === undefined || payload.source.resource !== `mesh-resource/${digest}`) {
      throw new RenderProjectionError(`${ctx}.source has an invalid content-addressed identity`);
    }
    const byteLength = requireNonNegativeInteger(
      payload.source.byteLength,
      `${ctx}.source.byteLength`,
    );
    if (byteLength < 16 || byteLength > 64 * 1024 * 1024) {
      throw new RenderProjectionError(`${ctx}.source.byteLength exceeds the resource bounds`);
    }
    const positionsByteOffset = requireNonNegativeInteger(
      payload.source.positionsByteOffset,
      `${ctx}.source.positionsByteOffset`,
    );
    const normalsByteOffset = requireNonNegativeInteger(
      payload.source.normalsByteOffset,
      `${ctx}.source.normalsByteOffset`,
    );
    const uvsByteOffset = payload.source.uvsByteOffset === undefined
      ? undefined
      : requireNonNegativeInteger(
        payload.source.uvsByteOffset,
        `${ctx}.source.uvsByteOffset`,
      );
    if (hasUvs !== (uvsByteOffset !== undefined)
      || (payload.source.encoding === 'packedStreamsLeV1' && uvsByteOffset !== undefined)
      || (payload.source.encoding === 'packedStreamsLeV2' && uvsByteOffset === undefined)) {
      throw new RenderProjectionError(`${ctx}.source encoding and uv stream must agree`);
    }
    const indicesByteOffset = requireNonNegativeInteger(
      payload.source.indicesByteOffset,
      `${ctx}.source.indicesByteOffset`,
    );
    if ([positionsByteOffset, normalsByteOffset, uvsByteOffset, indicesByteOffset]
      .filter((offset): offset is number => offset !== undefined)
      .some((offset) => offset < 16 || offset % 4 !== 0)) {
      throw new RenderProjectionError(`${ctx}.source offsets must be aligned after the header`);
    }
    const positionsEnd = positionsByteOffset + vertexCount * positionComponents * 4;
    const normalsEnd = normalsByteOffset + vertexCount * normalComponents * 4;
    const uvsEnd = uvsByteOffset === undefined ? normalsEnd : uvsByteOffset + vertexCount * 2 * 4;
    const indicesEnd = indicesByteOffset + indexCount * 4;
    if (positionsEnd > byteLength || normalsEnd > byteLength || uvsEnd > byteLength
      || indicesEnd > byteLength || positionsEnd > normalsByteOffset
      || (uvsByteOffset === undefined ? normalsEnd : uvsEnd) > indicesByteOffset
      || (uvsByteOffset !== undefined && normalsEnd > uvsByteOffset)) {
      throw new RenderProjectionError(`${ctx}.source streams exceed or overlap the resource`);
    }
  }

  for (let i = 0; i < payload.groups.length; i += 1) {
    const group = payload.groups[i]!;
    const start = requireNonNegativeInteger(group.start, `${ctx}.groups[${i}].start`);
    const count = requireNonNegativeInteger(group.count, `${ctx}.groups[${i}].count`);
    requireNonNegativeInteger(group.materialSlot, `${ctx}.groups[${i}].materialSlot`);
    if (start + count > indexCount) {
      throw new RenderProjectionError(
        `${ctx}.groups[${i}] window [${start}, ${start + count}) exceeds indexCount ${indexCount}`,
      );
    }
    const expectedStart = i === 0
      ? 0
      : payload.groups[i - 1]!.start + payload.groups[i - 1]!.count;
    if (start !== expectedStart) {
      throw new RenderProjectionError(
        `${ctx}.groups[${i}] starts at ${start}; contiguous coverage requires ${expectedStart}`,
      );
    }
  }
  if (payload.groups.length > 0) {
    const last = payload.groups[payload.groups.length - 1]!;
    if (last.start + last.count !== indexCount) {
      throw new RenderProjectionError(`${ctx}.groups must cover all ${indexCount} indices`);
    }
  }
}

function validateOperationHandles(diff: RenderDiff): void {
  switch (diff.op) {
    case 'create':
    case 'createLight':
    case 'createStaticMeshInstance':
    case 'createAnimatedMeshInstance':
    case 'createVoxelObjectInstance':
    case 'createSprite':
      requireSafeHandle(diff.handle, `${diff.op}.handle`);
      if (diff.parent !== null) requireSafeHandle(diff.parent, `${diff.op}.parent`);
      return;
    case 'update':
    case 'destroy':
    case 'replaceMeshPayload':
    case 'updateLight':
    case 'setMaterialInstanceParameters':
    case 'setAnimatedMeshPlayback':
    case 'setVoxelObjectFrame':
    case 'updateSprite':
      requireSafeHandle(diff.handle, `${diff.op}.handle`);
      return;
    case 'defineMaterial':
    case 'defineTexture':
    case 'defineSpriteAtlas':
    case 'defineStaticMesh':
    case 'defineAnimatedMesh':
    case 'defineVoxelObject':
    case 'releaseVoxelObject':
      return;
  }
}

function requireSafeHandle(value: number, ctx: string): void {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new RenderProjectionError(`${ctx} must be a non-negative JSON-safe integer`);
  }
}

function attributeComponents(
  payload: MeshPayloadDescriptor,
  name: 'position' | 'normal',
  ctx: string,
): number {
  const attribute = payload.layout.attributes.find((candidate) => candidate.name === name);
  if (attribute === undefined) {
    throw new RenderProjectionError(`${ctx}.layout.attributes missing ${name}`);
  }
  return requireNonNegativeInteger(attribute.components, `${ctx}.layout.attributes.${name}.components`);
}

function requireLength(values: readonly unknown[], expected: number, ctx: string): void {
  if (values.length !== expected) {
    throw new RenderProjectionError(`${ctx} expected length ${expected}, got ${values.length}`);
  }
}

function requireNonNegativeInteger(value: number, ctx: string): number {
  if (!Number.isInteger(value) || value < 0) {
    throw new RenderProjectionError(`${ctx} must be a non-negative integer`);
  }
  return value;
}

function sortedHandles(map: ReadonlyMap<RenderHandle, unknown>): RenderHandle[] {
  return [...map.keys()].sort(numberCompare);
}

function sortedValues<T extends { readonly id: string }>(map: ReadonlyMap<string, T>): T[] {
  return [...map.values()].map((value) => clone(value)).sort((a, b) => a.id.localeCompare(b.id));
}

function numberCompare(a: number, b: number): number {
  return a - b;
}

function clone<T>(value: T): T {
  if (value === undefined) {
    return value;
  }
  return JSON.parse(JSON.stringify(value)) as T;
}
