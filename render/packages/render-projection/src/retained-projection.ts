// Renderer-neutral retained render-diff application.
//
// This module applies generated render diffs to a typed retained projection
// model. It owns no authority, imports no renderer implementation, and never
// touches raw runtime transports. Browser/Three/WebGPU bindings consume the
// returned neutral instructions or inspect the retained snapshot.

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
} from '@rusty-engine/render-contracts';

/** Raised when a render diff cannot be applied to the retained projection. */
export class RenderProjectionError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'RenderProjectionError';
  }
}

export type RenderProjectionNodeKind = 'primitive' | 'staticMesh' | 'animatedMesh' | 'sprite';

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

export interface SpriteProjectionNode extends RenderProjectionNodeBase {
  readonly kind: 'sprite';
  readonly sprite: SpriteInstanceDescriptor;
  readonly frameUv: readonly [number, number, number, number];
  readonly renderOrder: number;
}

export type RenderProjectionNode =
  | PrimitiveProjectionNode
  | StaticMeshProjectionNode
  | AnimatedMeshProjectionNode
  | SpriteProjectionNode;

export type RenderProjectionInstruction =
  | { readonly op: 'defineMaterial'; readonly material: RenderMaterialDescriptor }
  | { readonly op: 'defineTexture'; readonly texture: TextureDescriptor }
  | { readonly op: 'defineSpriteAtlas'; readonly atlas: SpriteAtlasDescriptor }
  | { readonly op: 'defineStaticMesh'; readonly asset: StaticMeshAsset }
  | { readonly op: 'defineAnimatedMesh'; readonly asset: AnimatedMeshAsset }
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
}

type NodeRecord = MutablePrimitiveNode | MutableStaticMeshNode | MutableAnimatedMeshNode | MutableSpriteNode;

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

interface MutableSpriteNode extends MutableNodeBase {
  kind: 'sprite';
  sprite: SpriteInstanceDescriptor;
  frameUv: [number, number, number, number];
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

interface MutableLight {
  handle: RenderHandle;
  parent: RenderHandle | null;
  light: LightDescriptor;
}

/** A retained renderer-neutral projection driven only by render diffs. */
export class RenderProjection {
  readonly #nodes = new Map<RenderHandle, NodeRecord>();
  readonly #lights = new Map<RenderHandle, MutableLight>();
  readonly #materials = new Map<string, RenderMaterialDescriptor>();
  readonly #textures = new Map<string, TextureDescriptor>();
  readonly #spriteAtlases = new Map<string, SpriteAtlasDescriptor>();
  readonly #staticMeshes = new Map<string, StaticMeshRecord>();
  readonly #animatedMeshes = new Map<string, AnimatedMeshRecord>();

  /**
   * Apply a frame in authored order and return renderer-neutral instructions.
   * The frame is fail-atomic: a rejected later operation cannot retain any
   * state from earlier operations in the same frame.
   */
  applyFrame(frame: RenderFrameDiff): readonly RenderProjectionInstruction[] {
    const staged = this.#clone();
    const instructions: RenderProjectionInstruction[] = [];
    for (const diff of frame.ops) {
      instructions.push(...staged.applyDiff(diff));
    }
    this.#replaceWith(staged);
    return instructions;
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
      case 'createStaticMeshInstance':
        return [this.#createStaticMeshInstance(diff)];
      case 'createAnimatedMeshInstance':
        return [this.#createAnimatedMeshInstance(diff)];
      case 'setAnimatedMeshPlayback':
        return [this.#setAnimatedMeshPlayback(diff)];
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

  staticMeshRefCount(asset: string): number {
    return this.#staticMeshes.get(asset)?.refCount ?? 0;
  }

  animatedMeshRefCount(asset: string): number {
    return this.#animatedMeshes.get(asset)?.refCount ?? 0;
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
    this.#insert(record);
    return { op: 'upsertNode', node: snapshotNode(record) };
  }

  #update(diff: Extract<RenderDiff, { op: 'update' }>): RenderProjectionInstruction {
    const record = this.#require(diff.handle, 'update');
    if (diff.transform !== null) {
      record.transform = clone(diff.transform);
      if (record.kind === 'primitive') {
        record.node = { ...record.node, transform: clone(diff.transform) };
      } else if (record.kind === 'staticMesh') {
        record.instance = { ...record.instance, transform: clone(diff.transform) };
      } else if (record.kind === 'animatedMesh') {
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
        this.#require(light.parent, 'destroyLight.parent').children.delete(handle);
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
      this.#require(record.parent, 'destroy.parent').children.delete(handle);
    }
    if (record.kind === 'staticMesh') {
      const mesh = this.#staticMeshes.get(record.asset);
      if (mesh !== undefined) {
        mesh.refCount -= 1;
      }
    } else if (record.kind === 'animatedMesh') {
      const mesh = this.#animatedMeshes.get(record.asset);
      if (mesh !== undefined) {
        mesh.refCount -= 1;
      }
    }
    instructions.push({ op: 'removeNode', handle });
    return instructions;
  }

  #replaceMeshPayload(
    diff: Extract<RenderDiff, { op: 'replaceMeshPayload' }>,
  ): RenderProjectionInstruction {
    const record = this.#require(diff.handle, 'replaceMeshPayload');
    if (record.kind === 'sprite') {
      throw new RenderProjectionError(`replaceMeshPayload: handle ${diff.handle} is a sprite`);
    }
    validateMeshPayload(diff.payload, 'replaceMeshPayload.payload');
    record.meshPayload = clone(diff.payload);
    return { op: 'upsertNode', node: snapshotNode(record) };
  }

  #createLight(diff: Extract<RenderDiff, { op: 'createLight' }>): RenderProjectionInstruction {
    this.#ensureFree(diff.handle, 'createLight');
    const parent = this.#parentHandle(diff.parent, 'createLight.parent');
    validateLight(diff.light, 'createLight.light');
    const record: MutableLight = {
      handle: diff.handle,
      parent,
      light: clone(diff.light),
    };
    this.#lights.set(diff.handle, record);
    if (parent !== null) {
      this.#require(parent, 'createLight.parent').children.add(diff.handle);
    }
    return { op: 'upsertLight', light: snapshotLight(record) };
  }

  #updateLight(diff: Extract<RenderDiff, { op: 'updateLight' }>): RenderProjectionInstruction {
    const record = this.#requireLight(diff.handle, 'updateLight');
    validateLight(diff.light, 'updateLight.light');
    if (record.light.kind !== diff.light.kind) {
      throw new RenderProjectionError(
        `updateLight: handle ${diff.handle} cannot change kind from ${record.light.kind} to ${diff.light.kind}`,
      );
    }
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
    asset.refCount += 1;
    this.#insert(record);
    return { op: 'upsertNode', node: snapshotNode(record) };
  }

  #setMaterialInstanceParameters(
    diff: Extract<RenderDiff, { op: 'setMaterialInstanceParameters' }>,
  ): RenderProjectionInstruction {
    const record = this.#require(diff.handle, 'setMaterialInstanceParameters');
    if (record.kind !== 'staticMesh') {
      throw new RenderProjectionError(
        `setMaterialInstanceParameters: handle ${diff.handle} is not a static mesh`,
      );
    }
    const asset = this.#staticMeshes.get(record.asset);
    if (asset === undefined || !asset.asset.materialSlots.some((binding) => binding.slot === diff.slot)) {
      throw new RenderProjectionError(
        `setMaterialInstanceParameters: unbound slot ${diff.slot} on ${record.asset}`,
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
    asset.refCount += 1;
    this.#insert(record);
    return { op: 'upsertNode', node: snapshotNode(record) };
  }

  #setAnimatedMeshPlayback(
    diff: Extract<RenderDiff, { op: 'setAnimatedMeshPlayback' }>,
  ): RenderProjectionInstruction {
    const record = this.#require(diff.handle, 'setAnimatedMeshPlayback');
    if (record.kind !== 'animatedMesh') {
      throw new RenderProjectionError(`setAnimatedMeshPlayback: handle ${diff.handle} is not an animated mesh`);
    }
    const asset = this.#animatedMeshes.get(record.asset);
    if (asset === undefined) {
      throw new RenderProjectionError(`setAnimatedMeshPlayback: missing animated mesh asset ${record.asset}`);
    }
    validatePlaybackCommand(asset.asset, diff.playback, 'setAnimatedMeshPlayback.playback');
    record.playback = clone(diff.playback);
    record.instance = { ...record.instance, playback: clone(diff.playback) };
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
      renderOrder: sprite.renderOrder,
    };
    this.#insert(record);
    return { op: 'upsertNode', node: snapshotNode(record) };
  }

  #updateSprite(diff: Extract<RenderDiff, { op: 'updateSprite' }>): RenderProjectionInstruction {
    const record = this.#require(diff.handle, 'updateSprite');
    if (record.kind !== 'sprite') {
      throw new RenderProjectionError(`updateSprite: handle ${diff.handle} is not a sprite`);
    }
    if (diff.frame !== null) {
      record.sprite = { ...record.sprite, frame: diff.frame };
      record.frameUv = this.#resolveSpriteUv(record.sprite.asset, diff.frame);
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

  #insert(record: NodeRecord): void {
    this.#nodes.set(record.handle, record);
    if (record.parent !== null) {
      this.#require(record.parent, 'insert.parent').children.add(record.handle);
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

  #clone(): RenderProjection {
    const projection = new RenderProjection();
    for (const [handle, record] of this.#nodes) {
      projection.#nodes.set(handle, cloneNodeRecord(record));
    }
    for (const [handle, light] of this.#lights) {
      projection.#lights.set(handle, clone(light));
    }
    for (const [id, material] of this.#materials) {
      projection.#materials.set(id, clone(material));
    }
    for (const [id, texture] of this.#textures) {
      projection.#textures.set(id, clone(texture));
    }
    for (const [id, atlas] of this.#spriteAtlases) {
      projection.#spriteAtlases.set(id, clone(atlas));
    }
    for (const [id, record] of this.#staticMeshes) {
      projection.#staticMeshes.set(id, { asset: clone(record.asset), refCount: record.refCount });
    }
    for (const [id, record] of this.#animatedMeshes) {
      projection.#animatedMeshes.set(id, { asset: clone(record.asset), refCount: record.refCount });
    }
    return projection;
  }

  #replaceWith(projection: RenderProjection): void {
    replaceMap(this.#nodes, projection.#nodes);
    replaceMap(this.#lights, projection.#lights);
    replaceMap(this.#materials, projection.#materials);
    replaceMap(this.#textures, projection.#textures);
    replaceMap(this.#spriteAtlases, projection.#spriteAtlases);
    replaceMap(this.#staticMeshes, projection.#staticMeshes);
    replaceMap(this.#animatedMeshes, projection.#animatedMeshes);
  }
}

function cloneNodeRecord(record: NodeRecord): NodeRecord {
  const children = new Set(record.children);
  if (record.kind === 'primitive') {
    return { ...clone(record), children };
  }
  if (record.kind === 'staticMesh') {
    return {
      ...clone(record),
      children,
      materialParameters: new Map(
        [...record.materialParameters].map(([slot, parameters]) => [slot, clone(parameters)]),
      ),
    };
  }
  return { ...clone(record), children };
}

function replaceMap<K, V>(target: Map<K, V>, source: ReadonlyMap<K, V>): void {
  target.clear();
  for (const [key, value] of source) {
    target.set(key, value);
  }
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
  return {
    ...base,
    kind: 'sprite',
    sprite: clone(record.sprite),
    frameUv: clone(record.frameUv),
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

  if (payload.source.kind === 'inline') {
    requireLength(payload.source.positions, vertexCount * positionComponents, `${ctx}.source.positions`);
    requireLength(payload.source.normals, vertexCount * normalComponents, `${ctx}.source.normals`);
    requireLength(payload.source.indices, indexCount, `${ctx}.source.indices`);
    payload.source.indices.forEach((index, i) => {
      const value = requireNonNegativeInteger(index, `${ctx}.source.indices[${i}]`);
      if (value >= vertexCount) {
        throw new RenderProjectionError(
          `${ctx}.source.indices[${i}] ${value} is out of range for ${vertexCount} vertices`,
        );
      }
    });
  } else {
    requireNonNegativeInteger(payload.source.buffer, `${ctx}.source.buffer`);
    requireNonNegativeInteger(payload.source.positionsByteOffset, `${ctx}.source.positionsByteOffset`);
    requireNonNegativeInteger(payload.source.normalsByteOffset, `${ctx}.source.normalsByteOffset`);
    requireNonNegativeInteger(payload.source.indicesByteOffset, `${ctx}.source.indicesByteOffset`);
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
    case 'updateSprite':
      requireSafeHandle(diff.handle, `${diff.op}.handle`);
      return;
    case 'defineMaterial':
    case 'defineTexture':
    case 'defineSpriteAtlas':
    case 'defineStaticMesh':
    case 'defineAnimatedMesh':
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
