import * as THREE from 'three';
import { GLTFLoader, type GLTF } from 'three/examples/jsm/loaders/GLTFLoader.js';
import * as SkeletonUtils from 'three/examples/jsm/utils/SkeletonUtils.js';
import { sha256 } from '@noble/hashes/sha2.js';
import { bytesToHex } from '@noble/hashes/utils.js';
import type {
  AnimatedMeshAsset,
  AnimationClipPack,
  AnimatedMeshInstanceDescriptor,
  AnimatedMeshPlaybackCommand,
  MeshMaterialSlot,
  RenderHandle,
} from '@rusty-engine/render-contracts';

export class AnimatedMeshApplyError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'AnimatedMeshApplyError';
  }
}

/** Locale-independent canonical ordering for serialized and cross-host readouts. */
function codeUnitCompare(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}

export interface AnimatedMeshResource {
  readonly asset: string;
  readonly contentHash?: string | null;
  readonly scene: THREE.Object3D;
  readonly clips: readonly THREE.AnimationClip[];
  /** Engine-facing slot to the exact canonical GLB material association. */
  readonly embeddedMaterialSlots?: ReadonlyMap<number, AnimatedMeshEmbeddedMaterial>;
}

export interface AnimatedMeshEmbeddedMaterial {
  readonly sourceMaterialSlot: number;
  /** Every Three material object that GLTFLoader associated with this source index. */
  readonly materials: readonly THREE.Material[];
}

export interface AnimationClipPackResource {
  readonly asset: string;
  readonly contentHash?: string | null;
  readonly scene: THREE.Object3D;
  readonly clips: readonly THREE.AnimationClip[];
}

export interface AnimatedMeshAssetSource {
  getAnimatedMeshResource(asset: AnimatedMeshAsset): AnimatedMeshResource | undefined;
  getAnimationClipPackResource(pack: AnimationClipPack): AnimationClipPackResource | undefined;
}

export interface AnimatedMeshPlaybackReadout {
  readonly handle: RenderHandle;
  readonly asset: string;
  readonly status: 'not_started' | 'playing' | 'paused' | 'sampled' | 'stopped';
  readonly currentClip: string | null;
  /** Exact held sample, distinct from a resumable playback pause. */
  readonly heldSample: { readonly clip: string; readonly normalizedTime: number } | null;
  readonly mixerTimeSeconds: number;
  readonly actionTimeSeconds: number | null;
  readonly running: boolean;
  readonly paused: boolean;
  readonly loop: 'once' | 'repeat' | 'pingPong' | null;
  readonly speed: number | null;
  readonly weight: number | null;
  readonly commandSelected: boolean;
  readonly poseSample: AnimatedMeshPoseSample;
  readonly diagnostics: readonly string[];
  readonly controllerClips: readonly AnimatedMeshControllerClip[];
  readonly effectiveClips: readonly AnimatedMeshEffectiveClipReadout[];
}

export interface AnimatedMeshEffectiveClipReadout {
  readonly id: string;
  readonly origin: 'embedded' | 'pack';
  readonly durationSeconds: number;
}

export type AnimatedMeshSampleDiagnosticCode =
  | 'bone_matrix_non_finite'
  | 'bone_matrix_singular'
  | 'node_quaternion_invalid'
  | 'node_scale_invalid'
  | 'node_transform_non_finite'
  | 'sampled_bounds_implausible'
  | 'vertex_budget_exceeded';

export interface AnimatedMeshSampleDiagnostic {
  readonly code: AnimatedMeshSampleDiagnosticCode;
  readonly message: string;
  readonly node: string | null;
}

export interface AnimatedMeshSampleBounds {
  readonly min: readonly [number, number, number];
  readonly max: readonly [number, number, number];
}

export interface AnimatedMeshSampleReadout {
  readonly handle: RenderHandle;
  readonly asset: string;
  readonly contentHash: string | null;
  readonly clip: string;
  readonly normalizedTime: number;
  readonly durationSeconds: number;
  readonly assetBounds: AnimatedMeshSampleBounds;
  readonly sampledWorldBounds: AnimatedMeshSampleBounds | null;
  readonly sampledVertexCount: number;
  readonly boneCount: number;
  readonly skinningFacts: AnimatedMeshSkinningFacts;
  readonly diagnostics: readonly AnimatedMeshSampleDiagnostic[];
}

export interface AnimatedMeshSkinningFacts {
  readonly joints: readonly {
    readonly name: string;
    readonly parent: string | null;
    readonly restLocalMatrix: readonly number[];
    readonly inverseBindMatrix: readonly number[] | null;
  }[];
  readonly skinnedMeshCount: number;
  readonly inverseBindMatrixCount: number;
  readonly inverseBindMatricesFinite: boolean;
  readonly weightedVertexCount: number;
  readonly invalidWeightVertexCount: number;
  readonly maximumWeightSumError: number;
  readonly weightsNormalized: boolean;
  readonly interpolationModes: readonly ('discrete' | 'linear' | 'smooth')[];
  readonly instanceRootDistinctFromTemplate: boolean;
  readonly skeletonsIndependentFromTemplate: boolean;
  readonly sharedGeometryCount: number;
  readonly sharedMaterialCount: number;
}

export interface AnimatedMeshControllerClip {
  readonly clip: string;
  readonly weight: number;
  readonly speed: number;
  /** Current Engine-derived sample time for a fresh controller realization. */
  readonly timeSeconds?: number | undefined;
}

export interface AnimatedMeshPoseSample {
  readonly rootTranslation: readonly [number, number, number];
  readonly rootRotation: readonly [number, number, number, number];
  readonly rootScale: readonly [number, number, number];
  readonly hierarchyNodeCount: number;
  readonly hierarchyTranslationSum: readonly [number, number, number];
  readonly hierarchyRotationSum: readonly [number, number, number, number];
  readonly hierarchyScaleSum: readonly [number, number, number];
}

/**
 * A short-lived, independently posed clone for backend-local capture. It never
 * aliases the live retained instance's mixer, skeleton, or playback state.
 */
export interface AnimatedMeshCaptureAppearance {
  readonly object: THREE.Object3D;
  readonly source: {
    readonly asset: string;
    readonly generation: number;
    readonly handle: RenderHandle;
    readonly contentHash: string | null;
    readonly clip: string;
    readonly origin: 'embedded' | 'pack';
    readonly pack: { readonly asset: string; readonly contentHash: string | null } | null;
    readonly normalizedTime: number;
    readonly durationSeconds: number;
    readonly instanceTransform: {
      readonly position: readonly [number, number, number];
      readonly quaternion: readonly [number, number, number, number];
      readonly scale: readonly [number, number, number];
    };
  };
  dispose(): void;
}

interface AnimatedMeshAssetRecord {
  readonly asset: AnimatedMeshAsset;
  readonly resource: AnimatedMeshResource;
  readonly scene: THREE.Object3D;
  readonly embeddedMaterialSlots: ReadonlyMap<number, AnimatedMeshEmbeddedMaterial>;
  readonly packs: readonly AnimationClipPackResource[];
  readonly generation: number;
  refCount: number;
}

interface AnimatedMeshInstanceRecord {
  readonly handle: RenderHandle;
  readonly asset: string;
  readonly object: THREE.Object3D;
  readonly embeddedMaterialSlots: ReadonlyMap<number, AnimatedMeshEmbeddedMaterial>;
  readonly materialOverrides: ReadonlyMap<number, AnimatedMeshInstanceMaterialOverride>;
  readonly mixer: THREE.AnimationMixer;
  readonly actions: ReadonlyMap<string, THREE.AnimationAction>;
  readonly clipOrigins: ReadonlyMap<string, 'embedded' | 'pack'>;
  /** Product logical identity copied from retained descriptor metadata. */
  readonly sourceEntity: number | null;
  /** Monotonic renderer realization generation for this logical object. */
  readonly generation: number;
  completionEpoch: number;
  completionToken: AnimatedMeshCompletionToken | null;
  finishedListener: ((event: { readonly action?: THREE.AnimationAction }) => void) | null;
  currentClip: string | null;
  heldSample: { readonly clip: string; readonly normalizedTime: number } | null;
  commandSelected: boolean;
  status: AnimatedMeshPlaybackReadout['status'];
  loop: AnimatedMeshPlaybackReadout['loop'];
  speed: number | null;
  weight: number | null;
  controllerClips: readonly AnimatedMeshControllerClip[];
}

interface AnimatedMeshCompletionToken {
  readonly epoch: number;
  readonly action: THREE.AnimationAction;
  readonly clip: string;
}

/** A backend-neutral observation. No retained handle or Three object escapes. */
export interface AnimatedMeshNaturalCompletion {
  readonly objectId: number;
  readonly generation: number;
  readonly clip: string;
}

interface AnimatedMeshInstanceMaterialOverride {
  readonly binding: MeshMaterialSlot;
  materials: THREE.Material[];
}

export type AnimatedMeshMaterialFactory = (binding: MeshMaterialSlot) => THREE.Material;

export class MapAnimatedMeshAssetSource implements AnimatedMeshAssetSource {
  readonly #resources = new Map<string, AnimatedMeshResource>();

  readonly #packs = new Map<string, AnimationClipPackResource>();

  constructor(resources: readonly AnimatedMeshResource[], packs: readonly AnimationClipPackResource[] = []) {
    for (const resource of resources) {
      if (this.#resources.has(resource.asset)) throw new AnimatedMeshApplyError(`duplicate animated mesh resource ${resource.asset}`);
      this.#resources.set(resource.asset, resource);
    }
    for (const pack of packs) {
      if (this.#packs.has(pack.asset)) throw new AnimatedMeshApplyError(`duplicate animation clip pack resource ${pack.asset}`);
      this.#packs.set(pack.asset, pack);
    }
  }

  getAnimatedMeshResource(asset: AnimatedMeshAsset): AnimatedMeshResource | undefined {
    return this.#resources.get(asset.asset);
  }

  getAnimationClipPackResource(pack: AnimationClipPack): AnimationClipPackResource | undefined {
    return this.#packs.get(pack.asset);
  }
}

export async function loadAnimatedMeshGlbResource(
  asset: string,
  data: ArrayBuffer,
  contentHash?: string,
  embeddedMaterialSlots: readonly { readonly slot: number; readonly sourceMaterialSlot: number }[] = [],
): Promise<AnimatedMeshResource> {
  const loader = new GLTFLoader();
  const gltf = await new Promise<GLTF>((resolve, reject) => {
    loader.parse(data, '', resolve, reject);
  });
  return {
    asset,
    ...(contentHash === undefined ? {} : { contentHash }),
    scene: gltf.scene,
    clips: gltf.animations,
    embeddedMaterialSlots: resolveEmbeddedMaterialSlots(gltf, embeddedMaterialSlots),
  };
}

/**
 * Resolves importer-owned GLB material indices through GLTFLoader's parser
 * associations. Associations are source-indexed, so this never depends on the
 * order Three happens to visit meshes in the scene graph.
 */
function resolveEmbeddedMaterialSlots(
  gltf: GLTF,
  bindings: readonly { readonly slot: number; readonly sourceMaterialSlot: number }[],
): ReadonlyMap<number, AnimatedMeshEmbeddedMaterial> {
  const sources = new Set<number>();
  bindings.forEach((binding, index) => {
    if (!Number.isSafeInteger(binding.slot) || binding.slot !== index
      || !Number.isSafeInteger(binding.sourceMaterialSlot) || binding.sourceMaterialSlot < 0
      || binding.sourceMaterialSlot > 65_535 || sources.has(binding.sourceMaterialSlot)) {
      throw new AnimatedMeshApplyError('loadAnimatedMeshGlbResource: embedded material slots are invalid');
    }
    sources.add(binding.sourceMaterialSlot);
  });
  const materialsBySourceSlot = new Map<number, Set<THREE.Material>>();
  for (const [candidate, association] of gltf.parser.associations) {
    if (!(candidate instanceof THREE.Mesh)) continue;
    const sourceMaterialSlot = sourceMaterialSlotForMesh(gltf, association);
    if (sourceMaterialSlot === undefined) continue;
    const materials = materialsBySourceSlot.get(sourceMaterialSlot) ?? new Set<THREE.Material>();
    if (Array.isArray(candidate.material)) {
      candidate.material.forEach((material) => materials.add(material));
    } else if (candidate.material instanceof THREE.Material) {
      materials.add(candidate.material);
    }
    materialsBySourceSlot.set(sourceMaterialSlot, materials);
  }
  return new Map(bindings.map((binding) => {
    const materials = materialsBySourceSlot.get(binding.sourceMaterialSlot);
    if (materials === undefined || materials.size === 0) {
      throw new AnimatedMeshApplyError(
        `loadAnimatedMeshGlbResource: source material ${String(binding.sourceMaterialSlot)} is missing from the admitted GLB`,
      );
    }
    return [binding.slot, Object.freeze({
      sourceMaterialSlot: binding.sourceMaterialSlot,
      materials: Object.freeze([...materials]),
    })] as const;
  }));
}

function sourceMaterialSlotForMesh(
  gltf: GLTF,
  association: { readonly meshes?: number; readonly primitives?: number },
): number | undefined {
  const meshIndex = association.meshes;
  const primitiveIndex = association.primitives;
  if (typeof meshIndex !== 'number' || !Number.isSafeInteger(meshIndex)
    || typeof primitiveIndex !== 'number' || !Number.isSafeInteger(primitiveIndex)) {
    return undefined;
  }
  const mesh = (gltf.parser.json as { readonly meshes?: readonly {
    readonly primitives?: readonly { readonly material?: unknown }[];
  }[] }).meshes?.[meshIndex];
  const material = mesh?.primitives?.[primitiveIndex]?.material;
  return typeof material === 'number' && Number.isSafeInteger(material) && material >= 0
    ? material
    : undefined;
}

export async function loadAnimationClipPackGlbResource(
  asset: string, data: ArrayBuffer, contentHash?: string,
): Promise<AnimationClipPackResource> {
  const resource = await loadAnimatedMeshGlbResource(asset, data, contentHash);
  return resource;
}

export class AnimatedMeshRegistry {
  readonly #assetSource: AnimatedMeshAssetSource | undefined;
  readonly #assets = new Map<string, AnimatedMeshAssetRecord>();
  readonly #instances = new Map<RenderHandle, AnimatedMeshInstanceRecord>();
  readonly #assetGenerations = new Map<string, number>();
  readonly #nextGenerationByObject = new Map<number, number>();
  readonly #naturalCompletionListeners = new Set<(completion: AnimatedMeshNaturalCompletion) => void>();

  constructor(assetSource: AnimatedMeshAssetSource | undefined) {
    this.#assetSource = assetSource;
  }

  get instanceCount(): number {
    return this.#instances.size;
  }

  /** Subscribe to actual Three LoopOnce completion events with no handle escape. */
  subscribeNaturalCompletions(
    listener: (completion: AnimatedMeshNaturalCompletion) => void,
  ): () => void {
    this.#naturalCompletionListeners.add(listener);
    return () => this.#naturalCompletionListeners.delete(listener);
  }

  define(asset: AnimatedMeshAsset): void {
    const existing = this.#assets.get(asset.asset);
    if (existing && existing.refCount > 0) {
      throw new AnimatedMeshApplyError(
        `defineAnimatedMesh: asset ${asset.asset} is in use by ${existing.refCount} instance(s)`,
      );
    }
    const { resource, packs } = this.#validatedResource(asset);
    const template = createAnimatedMeshAssetScene(resource.scene, resource.embeddedMaterialSlots);
    if (existing) {
      disposeAnimatedMeshAssetScene(existing.scene);
    }
    const generation = (this.#assetGenerations.get(asset.asset) ?? 0) + 1;
    this.#assetGenerations.set(asset.asset, generation);
    this.#assets.set(asset.asset, {
      asset,
      resource,
      scene: template.scene,
      embeddedMaterialSlots: template.embeddedMaterialSlots,
      packs,
      generation,
      refCount: 0,
    });
  }

  validateDefinition(asset: AnimatedMeshAsset): void {
    this.#validatedResource(asset);
  }

  /** Run every fallible creation path on a detached instance during frame preflight. */
  validateInitialSample(instance: AnimatedMeshInstanceDescriptor): void {
    const probeHandle = -1 as RenderHandle;
    const probe = {
      ...instance,
      // A preflight must not consume a source-entity generation.
      metadata: { ...instance.metadata, sourceEntity: null },
    };
    let created = false;
    try {
      this.create(
        probeHandle,
        probe,
        probe.materialOverrides.length === 0
          ? undefined
          : () => new THREE.MeshBasicMaterial(),
      );
      created = true;
    } finally {
      if (created) this.release(probeHandle);
    }
  }

  /** Preflight creation for an asset defined earlier in this frame. */
  validateInitialSampleForDefinition(
    asset: AnimatedMeshAsset,
    instance: AnimatedMeshInstanceDescriptor,
  ): void {
    const staged = new AnimatedMeshRegistry(this.#assetSource);
    try {
      staged.define(asset);
      staged.validateInitialSample(instance);
    } finally {
      staged.dispose();
    }
  }

  /** Preflight a held sample update against one already-retained instance. */
  validateSample(handle: RenderHandle, clip: string, normalizedTime: number): void {
    const instance = this.#requireInstance(handle, 'setAnimatedMeshPlayback');
    this.validateInitialSample({
      asset: instance.asset,
      transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
      materialOverrides: [...instance.materialOverrides.values()].map((override) => override.binding),
      playback: { kind: 'sample', clip, normalizedTime },
      visible: true,
      metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label: null },
    });
  }

  /** Renderer-internal proof surface for the admitted template/instance map. */
  embeddedMaterialSlots(
    handle: RenderHandle,
  ): ReadonlyMap<number, AnimatedMeshEmbeddedMaterial> | undefined {
    return this.#instances.get(handle)?.embeddedMaterialSlots;
  }

  #validatedResource(asset: AnimatedMeshAsset): { resource: AnimatedMeshResource; packs: readonly AnimationClipPackResource[] } {
    if (asset.runtimeFormat !== 'glb') {
      throw new AnimatedMeshApplyError(`defineAnimatedMesh: unsupported runtime format ${asset.runtimeFormat}`);
    }
    const resource = this.#assetSource?.getAnimatedMeshResource(asset);
    if (!resource) {
      throw new AnimatedMeshApplyError(`defineAnimatedMesh: missing animated mesh resource ${asset.asset}`);
    }
    if (resource.contentHash !== undefined && resource.contentHash !== asset.contentHash) {
      throw new AnimatedMeshApplyError(
        `defineAnimatedMesh: content hash mismatch for ${asset.asset}; expected ${resource.contentHash}, received ${asset.contentHash}`,
      );
    }
    const requestedEmbeddedSlots = asset.embeddedMaterialSlots ?? [];
    const resolvedEmbeddedSlots = resource.embeddedMaterialSlots
      ?? new Map<number, AnimatedMeshEmbeddedMaterial>();
    if (requestedEmbeddedSlots.length !== resolvedEmbeddedSlots.size
      || requestedEmbeddedSlots.some((binding) => (
        resolvedEmbeddedSlots.get(binding.slot)?.sourceMaterialSlot !== binding.sourceMaterialSlot
      ))) {
      throw new AnimatedMeshApplyError(
        `defineAnimatedMesh: embedded material slot mapping is unavailable for ${asset.asset}`,
      );
    }
    assertClipDescriptors(asset, resource);
    const packs = (asset.clipPacks ?? []).map((pack) => {
      const clipPack = this.#assetSource?.getAnimationClipPackResource(pack);
      if (!clipPack) throw new AnimatedMeshApplyError(`defineAnimatedMesh: missing animation clip pack resource ${pack.asset}`);
      if (clipPack.contentHash !== undefined && clipPack.contentHash !== pack.contentHash) {
        throw new AnimatedMeshApplyError(`defineAnimatedMesh: clip pack content hash mismatch for ${pack.asset}`);
      }
      assertClipPack(pack, resource.scene, clipPack);
      return clipPack;
    });
    return { resource, packs };
  }

  create(
    handle: RenderHandle,
    instance: AnimatedMeshInstanceDescriptor,
    materialFactory?: AnimatedMeshMaterialFactory,
  ): AnimatedMeshInstanceRecord {
    const record = this.#assets.get(instance.asset);
    if (!record) {
      throw new AnimatedMeshApplyError(`createAnimatedMeshInstance: undefined animated mesh asset ${instance.asset}`);
    }
    if (instance.materialOverrides.length > 0 && materialFactory === undefined) {
      throw new AnimatedMeshApplyError(
        `createAnimatedMeshInstance: material overrides require an Engine material factory for ${instance.asset}`,
      );
    }
    const object = SkeletonUtils.clone(record.scene);
    const materialOverrides = materialFactory === undefined
      ? new Map<number, AnimatedMeshInstanceMaterialOverride>()
      : applyMaterialOverrides(object, record.embeddedMaterialSlots, instance.materialOverrides, materialFactory);
    const mixer = new THREE.AnimationMixer(object);
    const actions = new Map<string, THREE.AnimationAction>();
    const clipOrigins = new Map<string, 'embedded' | 'pack'>();
    for (const clip of record.asset.clips) {
      actions.set(clip.id, mixer.clipAction(requireClip(record.resource, clip.id, clip.name)));
      clipOrigins.set(clip.id, 'embedded');
    }
    for (const pack of record.asset.clipPacks ?? []) {
      const resource = record.packs.find((candidate) => candidate.asset === pack.asset);
      if (!resource) throw new AnimatedMeshApplyError(`createAnimatedMeshInstance: missing admitted clip pack ${pack.asset}`);
      for (const clip of pack.clips) {
        if (actions.has(clip.id)) throw new AnimatedMeshApplyError(`createAnimatedMeshInstance: effective clip collision ${clip.id}`);
        actions.set(clip.id, mixer.clipAction(requireClip(resource, clip.id, clip.name)));
        clipOrigins.set(clip.id, 'pack');
      }
    }
    const sourceEntity = instance.metadata.sourceEntity;
    const generation = sourceEntity === null ? 0 : this.#nextGeneration(sourceEntity);
    const instanceRecord: AnimatedMeshInstanceRecord = {
      handle,
      asset: instance.asset,
      object,
      embeddedMaterialSlots: record.embeddedMaterialSlots,
      materialOverrides,
      mixer,
      actions,
      clipOrigins,
      sourceEntity,
      generation,
      completionEpoch: 0,
      completionToken: null,
      finishedListener: null,
      currentClip: null,
      heldSample: null,
      commandSelected: false,
      status: 'not_started',
      loop: null,
      speed: null,
      weight: null,
      controllerClips: [],
    };
    instanceRecord.finishedListener = (event) => {
      const token = instanceRecord.completionToken;
      if (token === null || event.action !== token.action || token.epoch !== instanceRecord.completionEpoch) return;
      instanceRecord.completionToken = null;
      instanceRecord.status = 'stopped';
      if (instanceRecord.sourceEntity !== null) {
        const completion = {
          objectId: instanceRecord.sourceEntity,
          generation: instanceRecord.generation,
          clip: token.clip,
        };
        for (const listener of this.#naturalCompletionListeners) listener(completion);
      }
    };
    mixer.addEventListener('finished', instanceRecord.finishedListener);
    // Validate optional initial playback against a detached instance first;
    // rejected creation must not publish an instance or consume a refcount.
    if (instance.playback?.kind === 'sample') {
      holdSample(instanceRecord, record, instance.playback.clip, instance.playback.normalizedTime);
    } else if (instance.playback?.kind === 'samplePose') {
      holdPose(instanceRecord, instance.playback);
    } else if (instance.playback) {
      applyPlaybackCommand(instanceRecord, instance.playback);
    }
    this.#instances.set(handle, instanceRecord);
    record.refCount += 1;
    return instanceRecord;
  }

  setPlayback(handle: RenderHandle, command: AnimatedMeshPlaybackCommand): void {
    const instance = this.#requireInstance(handle, 'setAnimatedMeshPlayback');
    if (command.kind === 'sample') {
      const asset = this.#assets.get(instance.asset);
      if (asset === undefined) {
        throw new AnimatedMeshApplyError(`setAnimatedMeshPlayback: missing defined asset ${instance.asset}`);
      }
      holdSample(instance, asset, command.clip, command.normalizedTime);
      return;
    }
    if (command.kind === 'samplePose') {
      holdPose(instance, command);
      return;
    }
    applyPlaybackCommand(instance, command);
  }

  setControllerWeights(
    handle: RenderHandle,
    clips: readonly AnimatedMeshControllerClip[],
  ): void {
    const instance = this.#requireInstance(handle, 'setAnimationControllerWeights');
    applyControllerWeights(instance, clips);
  }

  hasClips(handle: RenderHandle, clipIds: readonly string[]): boolean {
    const instance = this.#instances.get(handle);
    return instance !== undefined && clipIds.every((clipId) => instance.actions.has(clipId));
  }

  clearControllerWeights(handle: RenderHandle): void {
    const instance = this.#requireInstance(handle, 'clearAnimationControllerWeights');
    invalidateNaturalCompletion(instance);
    instance.mixer.stopAllAction();
    instance.currentClip = null;
    instance.heldSample = null;
    instance.controllerClips = [];
    instance.commandSelected = false;
    instance.status = 'stopped';
    instance.loop = null;
    instance.speed = null;
    instance.weight = null;
  }

  advance(deltaSeconds: number): void {
    if (!Number.isFinite(deltaSeconds) || deltaSeconds < 0) {
      throw new AnimatedMeshApplyError(`advanceAnimation: deltaSeconds must be finite and non-negative`);
    }
    for (const instance of this.#instances.values()) {
      instance.mixer.update(deltaSeconds);
    }
  }

  playback(handle: RenderHandle): AnimatedMeshPlaybackReadout | undefined {
    const instance = this.#instances.get(handle);
    if (!instance) {
      return undefined;
    }
    const action = instance.currentClip === null ? null : instance.actions.get(instance.currentClip) ?? null;
    return {
      handle,
      asset: instance.asset,
      status: instance.status,
      currentClip: instance.currentClip,
      heldSample: instance.heldSample === null ? null : { ...instance.heldSample },
      mixerTimeSeconds: instance.mixer.time,
      actionTimeSeconds: action?.time ?? null,
      running: action?.isRunning() ?? false,
      paused: action?.paused ?? false,
      loop: instance.loop,
      speed: instance.speed,
      weight: instance.weight,
      commandSelected: instance.commandSelected,
      poseSample: poseSample(instance.object),
      diagnostics: playbackDiagnostics(instance, action),
      controllerClips: instance.controllerClips,
      effectiveClips: [...instance.actions.entries()]
        .map(([id, action]) => ({ id, origin: instance.clipOrigins.get(id) ?? 'embedded', durationSeconds: action.getClip().duration }))
        .sort((left, right) => codeUnitCompare(left.id, right.id)),
    };
  }

  sample(handle: RenderHandle, clipId: string, normalizedTime: number): AnimatedMeshSampleReadout {
    const instance = this.#requireInstance(handle, 'sampleAnimatedMesh');
    const asset = this.#assets.get(instance.asset);
    if (asset === undefined) {
      throw new AnimatedMeshApplyError(
        `sampleAnimatedMesh: missing defined asset ${instance.asset}`,
      );
    }
    return holdSample(instance, asset, clipId, normalizedTime);
  }

  /** Pose an independent clone at one exact normalized time for a bounded capture operation. */
  createCaptureAppearance(
    handle: RenderHandle,
    clipId: string,
    normalizedTime: number,
  ): AnimatedMeshCaptureAppearance {
    if (!Number.isFinite(normalizedTime) || normalizedTime < 0 || normalizedTime > 1) {
      throw new AnimatedMeshApplyError(
        'createAnimatedMeshCaptureAppearance: normalizedTime must be finite and between 0 and 1',
      );
    }
    const instance = this.#requireInstance(handle, 'createAnimatedMeshCaptureAppearance');
    if (!finiteTransform(instance.object)) {
      throw new AnimatedMeshApplyError(
        'createAnimatedMeshCaptureAppearance: animated instance transform must be finite',
      );
    }
    const record = this.#assets.get(instance.asset);
    if (record === undefined) {
      throw new AnimatedMeshApplyError(`createAnimatedMeshCaptureAppearance: missing defined asset ${instance.asset}`);
    }
    const liveAction = instance.actions.get(clipId);
    if (liveAction === undefined) {
      throw new AnimatedMeshApplyError(`createAnimatedMeshCaptureAppearance: missing clip ${clipId} on ${instance.asset}`);
    }
    const clip = liveAction.getClip();
    if (!Number.isFinite(clip.duration) || clip.duration <= 0) {
      throw new AnimatedMeshApplyError(`createAnimatedMeshCaptureAppearance: clip ${clipId} has an invalid duration`);
    }
    // Clone the concrete retained appearance, not merely the admitted asset
    // template. This keeps capture material/node realization identical to the
    // source instance while SkeletonUtils still gives the capture lease an
    // independent skeleton and mixer.
    const object = SkeletonUtils.clone(instance.object);
    const captureOwnedMaterials = cloneCaptureOverrideMaterials(object, instance.materialOverrides);
    object.visible = true;
    const mixer = new THREE.AnimationMixer(object);
    const action = mixer.clipAction(clip);
    action.reset();
    action.enabled = true;
    action.paused = false;
    action.clampWhenFinished = true;
    action.setLoop(THREE.LoopOnce, 1);
    action.setEffectiveTimeScale(1);
    action.setEffectiveWeight(1);
    action.play();
    mixer.setTime(clip.duration * normalizedTime);
    action.paused = true;
    object.updateMatrixWorld(true);
    const origin = instance.clipOrigins.get(clipId) ?? 'embedded';
    const pack = origin === 'pack'
      ? record.asset.clipPacks?.find((candidate) => candidate.clips.some((candidateClip) => candidateClip.id === clipId))
      : undefined;
    let disposed = false;
    return Object.freeze({
      object,
      source: Object.freeze({
        asset: record.asset.asset,
        generation: record.generation,
        handle,
        contentHash: record.asset.contentHash,
        clip: clipId,
        origin,
        pack: pack === undefined ? null : Object.freeze({ asset: pack.asset, contentHash: pack.contentHash }),
        normalizedTime,
        durationSeconds: clip.duration,
        // This exact finite tuple participates in the held-bank key and is
        // checked at every stepped capture; JSON canonicalization makes -0
        // deterministic without rounding away a visible transform change.
        instanceTransform: Object.freeze({
          position: Object.freeze([instance.object.position.x, instance.object.position.y, instance.object.position.z] as const),
          quaternion: Object.freeze([instance.object.quaternion.x, instance.object.quaternion.y, instance.object.quaternion.z, instance.object.quaternion.w] as const),
          scale: Object.freeze([instance.object.scale.x, instance.object.scale.y, instance.object.scale.z] as const),
        }),
      }),
      dispose: () => {
        if (disposed) return;
        disposed = true;
        mixer.stopAllAction();
        mixer.uncacheRoot(object);
        // SkeletonUtils.clone intentionally shares geometry and materials with
        // the admitted template. A capture lease owns only its cloned skeletons
        // and mixer; releasing shared render resources here would corrupt the
        // canonical retained animated instance.
        object.traverse((node) => {
          if (node instanceof THREE.SkinnedMesh) node.skeleton.dispose();
        });
        captureOwnedMaterials.forEach((material) => material.dispose());
      },
    });
  }

  release(handle: RenderHandle): void {
    const instance = this.#instances.get(handle);
    if (!instance) {
      return;
    }
    invalidateNaturalCompletion(instance);
    if (instance.finishedListener !== null) {
      instance.mixer.removeEventListener('finished', instance.finishedListener);
      instance.finishedListener = null;
    }
    instance.mixer.stopAllAction();
    instance.mixer.uncacheRoot(instance.object);
    instance.materialOverrides.forEach((override) => {
      override.materials.forEach((material) => material.dispose());
    });
    this.#instances.delete(handle);
    const asset = this.#assets.get(instance.asset);
    if (asset) {
      asset.refCount -= 1;
    }
  }

  dispose(): void {
    for (const handle of [...this.#instances.keys()]) {
      this.release(handle);
    }
    for (const asset of this.#assets.values()) {
      disposeAnimatedMeshAssetScene(asset.scene);
    }
    this.#assets.clear();
  }

  /** Rebuild every instance-owned override that selects one redefined material. */
  replaceLiveMaterial(id: string, materialFactory: AnimatedMeshMaterialFactory): void {
    for (const instance of this.#instances.values()) {
      for (const override of instance.materialOverrides.values()) {
        if (override.binding.material !== id) continue;
        const replacements = override.materials.map(() => materialFactory(override.binding));
        let applied = 0;
        try {
          override.materials.forEach((material, index) => {
            const references = replaceMaterialReferences(instance.object, material, replacements[index]!);
            if (references === 0) {
              throw new AnimatedMeshApplyError(
                `replaceLiveMaterial: instance material override for ${id} is no longer attached`,
              );
            }
            applied += 1;
          });
        } catch (cause) {
          for (let index = 0; index < applied; index += 1) {
            replaceMaterialReferences(instance.object, replacements[index]!, override.materials[index]!);
          }
          replacements.forEach((material) => material.dispose());
          throw cause;
        }
        override.materials.forEach((material) => material.dispose());
        override.materials = replacements;
      }
    }
  }

  #requireInstance(handle: RenderHandle, ctx: string): AnimatedMeshInstanceRecord {
    const instance = this.#instances.get(handle);
    if (!instance) {
      throw new AnimatedMeshApplyError(`${ctx}: handle ${handle} is not an animated mesh`);
    }
    return instance;
  }

  #nextGeneration(objectId: number): number {
    const generation = this.#nextGenerationByObject.get(objectId) ?? 1;
    this.#nextGenerationByObject.set(objectId, generation + 1);
    return generation;
  }
}

function createAnimatedMeshAssetScene(
  source: THREE.Object3D,
  sourceEmbeddedMaterialSlots: ReadonlyMap<number, AnimatedMeshEmbeddedMaterial> | undefined,
): {
  readonly scene: THREE.Object3D;
  readonly embeddedMaterialSlots: ReadonlyMap<number, AnimatedMeshEmbeddedMaterial>;
} {
  const scene = SkeletonUtils.clone(source);
  const geometries = new Map<THREE.BufferGeometry, THREE.BufferGeometry>();
  const materials = new Map<THREE.Material, THREE.Material>();
  scene.traverse((object) => {
    const mesh = object as THREE.Mesh;
    if (mesh.geometry instanceof THREE.BufferGeometry) {
      const sourceGeometry = mesh.geometry;
      let geometry = geometries.get(sourceGeometry);
      if (geometry === undefined) {
        geometry = sourceGeometry.clone();
        geometries.set(sourceGeometry, geometry);
      }
      mesh.geometry = geometry;
    }
    if (Array.isArray(mesh.material)) {
      mesh.material = mesh.material.map((material) => cloneSharedMaterial(material, materials));
    } else if (mesh.material instanceof THREE.Material) {
      mesh.material = cloneSharedMaterial(mesh.material, materials);
    }
  });
  const embeddedMaterialSlots = new Map<number, AnimatedMeshEmbeddedMaterial>();
  for (const [slot, sourceBinding] of sourceEmbeddedMaterialSlots ?? []) {
    const templateMaterials = sourceBinding.materials.map((sourceMaterial) => materials.get(sourceMaterial));
    if (templateMaterials.some((material) => material === undefined)) {
      throw new AnimatedMeshApplyError(
        `createAnimatedMeshAssetScene: source material mapping for slot ${String(slot)} is not part of the GLB template`,
      );
    }
    embeddedMaterialSlots.set(slot, Object.freeze({
      sourceMaterialSlot: sourceBinding.sourceMaterialSlot,
      materials: Object.freeze(templateMaterials as THREE.Material[]),
    }));
  }
  return { scene, embeddedMaterialSlots };
}

function applyMaterialOverrides(
  object: THREE.Object3D,
  embeddedMaterialSlots: ReadonlyMap<number, AnimatedMeshEmbeddedMaterial>,
  bindings: readonly MeshMaterialSlot[],
  materialFactory: AnimatedMeshMaterialFactory,
): Map<number, AnimatedMeshInstanceMaterialOverride> {
  const overrides = new Map<number, AnimatedMeshInstanceMaterialOverride>();
  const sourceMaterials = new Set<THREE.Material>();
  try {
    for (const binding of bindings) {
      if (overrides.has(binding.slot)) {
        throw new AnimatedMeshApplyError(
          `createAnimatedMeshInstance: repeated material override slot ${String(binding.slot)}`,
        );
      }
      const source = embeddedMaterialSlots.get(binding.slot);
      if (source === undefined) {
        throw new AnimatedMeshApplyError(
          `createAnimatedMeshInstance: override for unbound embedded material slot ${String(binding.slot)}`,
        );
      }
      const materials: THREE.Material[] = [];
      try {
        for (const templateMaterial of source.materials) {
          if (sourceMaterials.has(templateMaterial)) {
            throw new AnimatedMeshApplyError(
              `createAnimatedMeshInstance: embedded material slot ${String(binding.slot)} overlaps another source slot`,
            );
          }
          sourceMaterials.add(templateMaterial);
          const replacement = materialFactory(binding);
          const references = replaceMaterialReferences(object, templateMaterial, replacement);
          if (references === 0) {
            replacement.dispose();
            throw new AnimatedMeshApplyError(
              `createAnimatedMeshInstance: embedded material slot ${String(binding.slot)} is absent from the cloned instance`,
            );
          }
          materials.push(replacement);
        }
      } catch (cause) {
        materials.forEach((material) => material.dispose());
        throw cause;
      }
      overrides.set(binding.slot, { binding, materials });
    }
    return overrides;
  } catch (cause) {
    overrides.forEach((override) => override.materials.forEach((material) => material.dispose()));
    throw cause;
  }
}

function replaceMaterialReferences(
  object: THREE.Object3D,
  prior: THREE.Material,
  replacement: THREE.Material,
): number {
  let references = 0;
  object.traverse((node) => {
    if (!(node instanceof THREE.Mesh)) return;
    if (Array.isArray(node.material)) {
      const materials = node.material.slice();
      let changed = false;
      for (let index = 0; index < materials.length; index += 1) {
        if (materials[index] !== prior) continue;
        materials[index] = replacement;
        references += 1;
        changed = true;
      }
      if (changed) node.material = materials;
    } else if (node.material === prior) {
      node.material = replacement;
      references += 1;
    }
  });
  return references;
}

/**
 * Captures borrow the template materials but clone per-instance replacements.
 * The capture can therefore outlive its source instance without a broad
 * shared-resource accounting layer: it owns only these short-lived clones.
 */
function cloneCaptureOverrideMaterials(
  object: THREE.Object3D,
  overrides: ReadonlyMap<number, AnimatedMeshInstanceMaterialOverride>,
): THREE.Material[] {
  const owned: THREE.Material[] = [];
  for (const override of overrides.values()) {
    for (const material of override.materials) {
      const clone = material.clone();
      if (replaceMaterialReferences(object, material, clone) === 0) {
        clone.dispose();
        continue;
      }
      owned.push(clone);
    }
  }
  return owned;
}

function cloneSharedMaterial(
  source: THREE.Material,
  materials: Map<THREE.Material, THREE.Material>,
): THREE.Material {
  let material = materials.get(source);
  if (material === undefined) {
    material = source.clone();
    materials.set(source, material);
  }
  return material;
}

function disposeAnimatedMeshAssetScene(scene: THREE.Object3D): void {
  const geometries = new Set<THREE.BufferGeometry>();
  const materials = new Set<THREE.Material>();
  scene.traverse((object) => {
    const mesh = object as THREE.Mesh;
    if (mesh.geometry instanceof THREE.BufferGeometry) {
      geometries.add(mesh.geometry);
    }
    if (Array.isArray(mesh.material)) {
      mesh.material.forEach((material) => materials.add(material));
    } else if (mesh.material instanceof THREE.Material) {
      materials.add(mesh.material);
    }
  });
  geometries.forEach((geometry) => geometry.dispose());
  materials.forEach((material) => material.dispose());
}

function assertClipDescriptors(asset: AnimatedMeshAsset, resource: AnimatedMeshResource): void {
  requireDescriptorClips(resource, asset.clips);
}

function assertClipPack(
  pack: AnimationClipPack,
  targetScene: THREE.Object3D,
  resource: AnimationClipPackResource,
): void {
  const targetJoints = jointHierarchy(targetScene);
  for (const expected of pack.rig.joints) {
    const actual = targetJoints.get(expected.id);
    if (actual === undefined) throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: missing target joint ${expected.id}`);
    if (actual !== expected.parent) throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: incompatible rig signature (parent for ${expected.id})`);
  }
  const packJoints = jointHierarchy(resource.scene);
  for (const joint of pack.rig.joints) {
    if (!packJoints.has(joint.id)) throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: missing source joint ${joint.id}`);
    if (packJoints.get(joint.id) !== joint.parent) throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: incompatible rig signature (source parent for ${joint.id})`);
  }
  assertMatchingRestPose(pack, targetScene, resource.scene);
  const targetFingerprint = animationRigFingerprint(targetScene);
  if (pack.rig.bindRestHash !== targetFingerprint) {
    throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: incompatible rig signature (bind/rest fingerprint)`);
  }
  assertRigPolicy(pack);
  for (const [, clip] of requireDescriptorClips(resource, pack.clips)) {
    assertClipChannels(pack, clip, new Set(pack.rig.joints.map((joint) => joint.id)));
  }
}

function assertRigPolicy(pack: AnimationClipPack): void {
  const structuralRootIds = pack.rig.joints
    .filter((joint) => joint.parent === null)
    .map((joint) => joint.id)
    .sort();
  if (structuralRootIds.length !== pack.rig.structuralRootIds.length
    || structuralRootIds.some((id, index) => id !== pack.rig.structuralRootIds[index])) {
    throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: incompatible rig signature (structural roots)`);
  }
  const structuralRoots = new Set(pack.rig.structuralRootIds);
  const joints = new Set(pack.rig.joints.map((joint) => joint.id));
  const motionRoots = new Set(pack.rig.designatedMotionRootIds);
  const poseTranslations = new Set(pack.rig.authoredPoseTranslationJointIds);
  if ([...motionRoots].some((id) => !structuralRoots.has(id))
    || [...poseTranslations].some((id) => !joints.has(id) || motionRoots.has(id))) {
    throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: incompatible rig signature (translation policy)`);
  }
  if (!joints.has(pack.rig.rootJointId) || !structuralRoots.has(pack.rig.rootJointId)
    || (pack.rig.rootConvention === 'authoredRootTranslation' && !motionRoots.has(pack.rig.rootJointId))) {
    throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: incompatible rig signature (designated root)`);
  }
}

function jointHierarchy(scene: THREE.Object3D): Map<string, string | null> {
  const result = new Map<string, string | null>();
  scene.traverse((node) => {
    if (!(node instanceof THREE.Bone)) return;
    if (node.name.length === 0 || result.has(node.name)) {
      throw new AnimatedMeshApplyError('animated skeleton has missing or duplicate joint identities');
    }
    result.set(node.name, node.parent instanceof THREE.Bone ? node.parent.name : null);
  });
  return result;
}

function assertMatchingRestPose(pack: AnimationClipPack, target: THREE.Object3D, source: THREE.Object3D): void {
  const targetBones = bonesByName(target);
  const sourceBones = bonesByName(source);
  for (const joint of pack.rig.joints) {
    const targetBone = targetBones.get(joint.id);
    const sourceBone = sourceBones.get(joint.id);
    if (!targetBone || !sourceBone) throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: missing target joints for rest comparison`);
    targetBone.updateMatrix();
    sourceBone.updateMatrix();
    if (!targetBone.matrix.elements.every((value, index) => Math.abs(value - sourceBone.matrix.elements[index]!) <= 1e-6)) {
      throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: incompatible rig signature (bind/rest for ${joint.id})`);
    }
  }
  const targetInverses = inverseBindsByJoint(target, true);
  const sourceInverses = inverseBindsByJoint(source, false);
  for (const [joint, sourceInverse] of sourceInverses) {
    const targetInverse = targetInverses.get(joint);
    if (!targetInverse || !targetInverse.elements.every((value, index) => Math.abs(value - sourceInverse.elements[index]!) <= 1e-6)) {
      throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: incompatible rig signature (inverse bind for ${joint})`);
    }
  }
}

/** Stable SHA-256 over a coherent target rig's decoded hierarchy, rest matrices, and inverse binds. */
export function animationRigFingerprint(scene: THREE.Object3D): string {
  const bones = bonesByName(scene);
  const inverseBinds = inverseBindsByJoint(scene, true);
  const canonical = [...bones.entries()].sort(([left], [right]) => codeUnitCompare(left, right)).map(([name, bone]) => {
    bone.updateMatrix();
    const inverse = inverseBinds.get(name);
    if (!inverse) throw new AnimatedMeshApplyError(`animated skeleton is missing an inverse-bind matrix for ${name}`);
    return [name, bone.parent instanceof THREE.Bone ? bone.parent.name : null,
      ...bone.matrix.elements.map((value) => Number(value.toFixed(6))),
      ...inverse.elements.map((value) => Number(value.toFixed(6)))];
  });
  return `sha256:${bytesToHex(sha256(new TextEncoder().encode(JSON.stringify(canonical))))}`;
}

function inverseBindsByJoint(scene: THREE.Object3D, requireTargetSkin: boolean): Map<string, THREE.Matrix4> {
  const inverses = new Map<string, THREE.Matrix4>();
  let skinnedMeshes = 0;
  scene.traverse((node) => {
    if (!(node instanceof THREE.SkinnedMesh)) return;
    skinnedMeshes += 1;
    node.skeleton.bones.forEach((bone, index) => {
      const inverse = node.skeleton.boneInverses[index];
      if (!inverse) {
        throw new AnimatedMeshApplyError('animated skeleton has missing or inconsistent inverse-bind matrices');
      }
      const existing = inverses.get(bone.name);
      if (existing && !existing.elements.every((value, matrixIndex) => Math.abs(value - inverse.elements[matrixIndex]!) <= 1e-6)) {
        throw new AnimatedMeshApplyError('animated skeleton has missing or inconsistent inverse-bind matrices');
      }
      inverses.set(bone.name, inverse);
    });
  });
  if (requireTargetSkin && skinnedMeshes === 0) {
    throw new AnimatedMeshApplyError('animated target has no skinned skeleton for bind/rest verification');
  }
  return inverses;
}

function bonesByName(scene: THREE.Object3D): Map<string, THREE.Bone> {
  const bones = new Map<string, THREE.Bone>();
  scene.traverse((node) => {
    if (!(node instanceof THREE.Bone)) return;
    if (node.name.length === 0 || bones.has(node.name)) {
      throw new AnimatedMeshApplyError('animated skeleton has missing or duplicate joint identities');
    }
    bones.set(node.name, node);
  });
  return bones;
}

const ANIMATION_DURATION_TOLERANCE_SECONDS = 1e-5;
const MAX_ANIMATION_KEYS_PER_TRACK = 4_096;
const MAX_ANIMATION_KEYS_PER_CLIP = 65_536;

function assertClipChannels(pack: AnimationClipPack, clip: THREE.AnimationClip, joints: ReadonlySet<string>): void {
  if (clip.tracks.length === 0 || clip.tracks.length > 1_024) {
    throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: malformed or unsupported channels for ${clip.name}`);
  }
  const translatedMotionRoots = new Set<string>();
  const motionRoots = new Set(pack.rig.designatedMotionRootIds);
  const poseTranslations = new Set(pack.rig.authoredPoseTranslationJointIds);
  const bindings = new Set<string>();
  let totalKeys = 0;
  for (const track of clip.tracks) {
    const parsed = parsePackTrackName(track.name, joints);
    if (parsed === null) {
      throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: malformed or unsupported channels for ${clip.name}`);
    }
    const joint = parsed.nodeName;
    const property = parsed.propertyName;
    const arity = property === 'position' ? 3 : property === 'quaternion' ? 4 : null;
    if (arity === null || !joints.has(joint) || track.times.length === 0 || track.values.length === 0
      || track.times.length > MAX_ANIMATION_KEYS_PER_TRACK || totalKeys + track.times.length > MAX_ANIMATION_KEYS_PER_CLIP
      || track.values.length !== track.times.length * arity) {
      throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: malformed or unsupported channels for ${clip.name}`);
    }
    for (let index = 0; index < track.times.length; index += 1) {
      const time = track.times[index]!;
      if (!Number.isFinite(time) || (index > 0 && time <= track.times[index - 1]!)) {
        throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: malformed or unsupported channels for ${clip.name}`);
      }
    }
    for (let index = 0; index < track.values.length; index += 1) {
      if (!Number.isFinite(track.values[index]!)) {
        throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: malformed or unsupported channels for ${clip.name}`);
      }
    }
    if (property === 'quaternion') {
      for (let index = 0; index < track.values.length; index += 4) {
        const squaredLength = track.values[index]! ** 2 + track.values[index + 1]! ** 2
          + track.values[index + 2]! ** 2 + track.values[index + 3]! ** 2;
        if (!Number.isFinite(squaredLength) || squaredLength <= 1e-12) {
          throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: malformed or unsupported channels for ${clip.name}`);
        }
      }
    }
    const binding = `${joint}.${property}`;
    if (bindings.has(binding)) {
      throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: malformed or unsupported channels for ${clip.name}`);
    }
    bindings.add(binding);
    totalKeys += track.times.length;
    if (property !== 'position' && property !== 'quaternion') {
      throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: unsupported root-motion declaration or channel for ${clip.name}`);
    }
    if (property === 'position') {
      if (!motionRoots.has(joint) && !poseTranslations.has(joint)) {
        throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: unsupported translation channel for ${clip.name}`);
      }
      if (motionRoots.has(joint)) {
        if (pack.rig.rootConvention === 'inPlace') assertInPlaceHorizontal(track, pack, clip);
        translatedMotionRoots.add(joint);
      }
    }
  }
  if (pack.rig.rootConvention === 'authoredRootTranslation'
    && ([...motionRoots].some((root) => !translatedMotionRoots.has(root))
      || translatedMotionRoots.size !== motionRoots.size)) {
    throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: unsupported root-motion declaration for ${clip.name}`);
  }
}

function parsePackTrackName(
  name: string,
  joints: ReadonlySet<string>,
): { readonly nodeName: string; readonly propertyName: 'position' | 'quaternion' } | null {
  for (const propertyName of ['position', 'quaternion'] as const) {
    const suffix = `.${propertyName}`;
    if (!name.endsWith(suffix)) continue;
    const nodeName = name.slice(0, -suffix.length);
    if (nodeName.length > 0 && joints.has(nodeName)) return { nodeName, propertyName };
  }
  return null;
}

function assertInPlaceHorizontal(track: THREE.KeyframeTrack, pack: AnimationClipPack, clip: THREE.AnimationClip): void {
  if (track.getValueSize() !== 3) throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: malformed root translation for ${clip.name}`);
  const x = track.values[0];
  const z = track.values[2];
  for (let index = 0; index < track.values.length; index += 3) {
    if (Math.abs(track.values[index]! - x!) > 1e-6 || Math.abs(track.values[index + 2]! - z!) > 1e-6) {
      throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: unsupported root-motion declaration for ${clip.name}`);
    }
  }
}

function requireDescriptorClips(
  resource: AnimatedMeshResource,
  descriptors: readonly { readonly id: string; readonly name: string | null; readonly durationSeconds: number | null }[],
): readonly (readonly [{ readonly id: string; readonly name: string | null; readonly durationSeconds: number | null }, THREE.AnimationClip])[] {
  const boundNames = new Set<string>();
  return descriptors.map((descriptor) => {
    const sourceName = descriptor.name ?? descriptor.id;
    const matches = resource.clips.filter((candidate) => candidate.name === sourceName);
    if (matches.length !== 1 || boundNames.has(sourceName)) {
      throw new AnimatedMeshApplyError(`animated mesh ${resource.asset} does not contain exactly one clip named ${sourceName}`);
    }
    const clip = matches[0]!;
    if (!Number.isFinite(clip.duration) || clip.duration <= 0) {
      throw new AnimatedMeshApplyError(`animated mesh ${resource.asset} clip ${sourceName} has an invalid decoded duration`);
    }
    if (descriptor.durationSeconds !== null
      && Math.abs(clip.duration - descriptor.durationSeconds) > Math.max(
        ANIMATION_DURATION_TOLERANCE_SECONDS,
        Math.abs(descriptor.durationSeconds) * ANIMATION_DURATION_TOLERANCE_SECONDS,
      )) {
      throw new AnimatedMeshApplyError(`animated mesh ${resource.asset} clip ${sourceName} duration does not match its descriptor`);
    }
    boundNames.add(sourceName);
    return [descriptor, clip] as const;
  });
}

function requireClip(
  resource: AnimatedMeshResource,
  id: string,
  name: string | null,
): THREE.AnimationClip {
  const matches = requireDescriptorClips(resource, [{ id, name, durationSeconds: null }]);
  const clip = matches[0]?.[1];
  if (!clip) {
    throw new AnimatedMeshApplyError(`animated mesh ${resource.asset} does not contain clip ${id}`);
  }
  return clip;
}

function applyPlaybackCommand(
  instance: AnimatedMeshInstanceRecord,
  command: AnimatedMeshPlaybackCommand,
): void {
  switch (command.kind) {
    case 'play':
      playClip(instance, command);
      return;
    case 'stop':
      invalidateNaturalCompletion(instance);
      stopCurrent(instance, command.fadeSeconds);
      instance.currentClip = null;
      instance.commandSelected = true;
      instance.status = 'stopped';
      instance.loop = null;
      instance.speed = null;
      instance.weight = null;
      instance.heldSample = null;
      return;
    case 'sample':
      throw new AnimatedMeshApplyError('sample playback must be applied through the animated mesh registry');
    case 'samplePose':
      throw new AnimatedMeshApplyError('sample pose playback must be applied through the animated mesh registry');
    case 'pause': {
      const action = currentAction(instance, 'pause');
      invalidateNaturalCompletion(instance);
      action.paused = true;
      instance.commandSelected = true;
      instance.status = 'paused';
      instance.heldSample = null;
      return;
    }
    case 'resume': {
      const action = currentAction(instance, 'resume');
      invalidateNaturalCompletion(instance);
      action.paused = false;
      action.play();
      instance.commandSelected = true;
      instance.status = 'playing';
      instance.heldSample = null;
      if (instance.loop === 'once') armNaturalCompletion(instance, action, instance.currentClip!);
      return;
    }
  }
}

/**
 * Hold an instance at an exact normalized clip time. All validation and
 * skinning preflight completes before the mixer or playback readout changes.
 */
function holdSample(
  instance: AnimatedMeshInstanceRecord,
  asset: AnimatedMeshAssetRecord,
  clipId: string,
  normalizedTime: number,
): AnimatedMeshSampleReadout {
  if (!Number.isFinite(normalizedTime) || normalizedTime < 0 || normalizedTime > 1) {
    throw new AnimatedMeshApplyError(
      'sampleAnimatedMesh: normalizedTime must be finite and between 0 and 1',
    );
  }
  const action = instance.actions.get(clipId);
  if (action === undefined) {
    throw new AnimatedMeshApplyError(`sampleAnimatedMesh: missing clip ${clipId} on ${instance.asset}`);
  }
  const durationSeconds = action.getClip().duration;
  if (!Number.isFinite(durationSeconds) || durationSeconds <= 0) {
    throw new AnimatedMeshApplyError(`sampleAnimatedMesh: clip ${clipId} has an invalid duration`);
  }
  // Skinning inspection is a bounded preflight. It must complete before the
  // disposable mixer or playback record changes so rejection is fail-atomic.
  const skinningFacts = animatedMeshSkinningFacts(instance.object, asset.scene, action.getClip());
  invalidateNaturalCompletion(instance);
  instance.mixer.stopAllAction();
  action.reset();
  action.enabled = true;
  action.paused = false;
  action.clampWhenFinished = true;
  action.setLoop(THREE.LoopOnce, 1);
  action.setEffectiveTimeScale(1);
  action.setEffectiveWeight(1);
  action.play();
  instance.mixer.setTime(durationSeconds * normalizedTime);
  action.paused = true;
  instance.currentClip = clipId;
  instance.heldSample = { clip: clipId, normalizedTime };
  instance.commandSelected = true;
  instance.status = 'sampled';
  instance.loop = 'once';
  instance.speed = 1;
  instance.weight = 1;
  instance.controllerClips = [];
  return diagnoseAnimatedMeshSample(
    instance,
    skinningFacts,
    clipId,
    normalizedTime,
    durationSeconds,
    asset.asset.bounds,
    asset.asset.contentHash,
  );
}

/**
 * Hold a complete blended pose on this isolated realization. The samples are
 * already Engine-derived, so this only realizes them and never advances a
 * controller or emits completion feedback.
 */
function holdPose(
  instance: AnimatedMeshInstanceRecord,
  command: Extract<AnimatedMeshPlaybackCommand, { readonly kind: 'samplePose' }>,
): void {
  const seen = new Set<string>();
  let totalWeight = 0;
  if (command.clips.length === 0 || command.clips.length > 4) {
    throw new AnimatedMeshApplyError('sample pose requires one to four clips');
  }
  for (const sample of command.clips) {
    if (!instance.actions.has(sample.clip)
      || seen.has(sample.clip)
      || !Number.isFinite(sample.timeSeconds)
      || sample.timeSeconds < 0
      || !Number.isFinite(sample.weight)
      || sample.weight < 0
      || sample.weight > 1) {
      throw new AnimatedMeshApplyError('sample pose contains an invalid clip sample');
    }
    seen.add(sample.clip);
    totalWeight += sample.weight;
  }
  if (Math.abs(totalWeight - 1) > 0.001) {
    throw new AnimatedMeshApplyError('sample pose clip weights must sum to 1');
  }
  invalidateNaturalCompletion(instance);
  instance.mixer.stopAllAction();
  for (const sample of command.clips) {
    const action = instance.actions.get(sample.clip)!;
    action.reset();
    action.enabled = true;
    action.paused = false;
    action.clampWhenFinished = true;
    action.setLoop(THREE.LoopRepeat, Infinity);
    action.setEffectiveTimeScale(1);
    action.setEffectiveWeight(sample.weight);
    action.play();
    action.time = sample.timeSeconds;
    action.paused = true;
  }
  // Force Three to apply all clip values before the capture backend reads the
  // cloned object's pose; zero elapsed time cannot advance the frozen state.
  instance.mixer.update(0);
  instance.currentClip = command.clips.reduce((best, sample) =>
    best === null || sample.weight > best.weight ? sample : best, null as (typeof command.clips)[number] | null)?.clip ?? null;
  // `samplePose` carries absolute per-clip seconds, so it cannot truthfully
  // populate the single-clip normalized-sample readout. It is still a held
  // pose, but not the legacy `sample` command's observation.
  instance.heldSample = null;
  instance.commandSelected = true;
  instance.status = 'sampled';
  instance.loop = 'once';
  instance.speed = 1;
  instance.weight = 1;
  instance.controllerClips = [];
}

function applyControllerWeights(
  instance: AnimatedMeshInstanceRecord,
  clips: readonly AnimatedMeshControllerClip[],
): void {
  if (clips.length === 0 || clips.length > 4) {
    throw new AnimatedMeshApplyError('setAnimationControllerWeights: expected one to four clips');
  }
  const byClip = new Map<string, AnimatedMeshControllerClip>();
  let totalWeight = 0;
  for (const clip of clips) {
    if (
      byClip.has(clip.clip)
      || !Number.isFinite(clip.weight)
      || clip.weight < 0
      || clip.weight > 1
      || !Number.isFinite(clip.speed)
      || clip.speed <= 0
      || (clip.timeSeconds !== undefined
        && (!Number.isFinite(clip.timeSeconds) || clip.timeSeconds < 0))
    ) {
      throw new AnimatedMeshApplyError('setAnimationControllerWeights: invalid clip sample');
    }
    if (!instance.actions.has(clip.clip)) {
      throw new AnimatedMeshApplyError(
        `setAnimationControllerWeights: missing clip ${clip.clip} on ${instance.asset}`,
      );
    }
    byClip.set(clip.clip, clip);
    totalWeight += clip.weight;
  }
  if (Math.abs(totalWeight - 1) > 0.001) {
    throw new AnimatedMeshApplyError(
      `setAnimationControllerWeights: weights must sum to 1, received ${totalWeight}`,
    );
  }
  invalidateNaturalCompletion(instance);
  for (const [clipId, action] of instance.actions) {
    const sample = byClip.get(clipId);
    if (sample === undefined) {
      action.stop();
      continue;
    }
    action.enabled = true;
    action.paused = false;
    action.setLoop(THREE.LoopRepeat, Infinity);
    action.setEffectiveTimeScale(sample.speed);
    action.setEffectiveWeight(sample.weight);
    action.play();
    if (sample.timeSeconds !== undefined) action.time = sample.timeSeconds;
  }
  instance.currentClip = clips.reduce((selected, clip) =>
    selected === null || clip.weight > selected.weight ? clip : selected, null as AnimatedMeshControllerClip | null)?.clip ?? null;
  instance.commandSelected = false;
  instance.heldSample = null;
  instance.status = 'playing';
  instance.loop = 'repeat';
  instance.speed = null;
  instance.weight = null;
  instance.controllerClips = clips.map((clip) => ({ ...clip }));
}

function playClip(
  instance: AnimatedMeshInstanceRecord,
  command: Extract<AnimatedMeshPlaybackCommand, { readonly kind: 'play' }>,
): void {
  const action = instance.actions.get(command.clip);
  if (!action) {
    throw new AnimatedMeshApplyError(`setAnimatedMeshPlayback: missing clip ${command.clip} on ${instance.asset}`);
  }
  invalidateNaturalCompletion(instance);
  const prior = instance.currentClip === null ? null : instance.actions.get(instance.currentClip) ?? null;
  if (command.restart) {
    action.reset();
  }
  action.enabled = true;
  action.paused = false;
  action.clampWhenFinished = command.loop === 'once';
  action.setLoop(toThreeLoop(command.loop), command.loop === 'once' ? 1 : Infinity);
  action.setEffectiveTimeScale(command.speed);
  action.setEffectiveWeight(command.weight);
  if (prior && prior !== action) {
    if (command.fadeSeconds !== null && command.fadeSeconds > 0) {
      action.crossFadeFrom(prior, command.fadeSeconds, false);
    } else {
      prior.stop();
    }
  }
  action.play();
  // A canonical baseline may start this newly realized action at a retained
  // Engine timeline offset. Normal product commands omit it, so they preserve
  // the ordinary Three playback behavior.
  const restoredPastOneShotEnd = command.loop === 'once'
    && command.startOffsetSeconds !== undefined
    && command.startOffsetSeconds >= action.getClip().duration;
  if (command.startOffsetSeconds !== undefined) {
    action.time = restoredPastOneShotEnd
      ? action.getClip().duration
      : command.startOffsetSeconds;
  }
  // An installed baseline must never turn a past LoopOnce endpoint into a
  // newly observed browser completion. Hold Three at its final pose instead;
  // the Engine already owns the elapsed fact that made this terminal.
  action.paused = restoredPastOneShotEnd || command.startPaused === true;
  instance.currentClip = command.clip;
  instance.heldSample = null;
  instance.controllerClips = [];
  instance.commandSelected = true;
  instance.status = restoredPastOneShotEnd ? 'stopped' : action.paused ? 'paused' : 'playing';
  instance.loop = command.loop;
  instance.speed = command.speed;
  instance.weight = command.weight;
  if (command.loop === 'once' && !action.paused) armNaturalCompletion(instance, action, command.clip);
}

function invalidateNaturalCompletion(instance: AnimatedMeshInstanceRecord): void {
  instance.completionEpoch += 1;
  instance.completionToken = null;
}

function armNaturalCompletion(
  instance: AnimatedMeshInstanceRecord,
  action: THREE.AnimationAction,
  clip: string,
): void {
  instance.completionEpoch += 1;
  instance.completionToken = { epoch: instance.completionEpoch, action, clip };
}

function stopCurrent(instance: AnimatedMeshInstanceRecord, fadeSeconds: number | null): void {
  const action = instance.currentClip === null ? null : instance.actions.get(instance.currentClip) ?? null;
  if (!action) {
    return;
  }
  if (fadeSeconds !== null && fadeSeconds > 0) {
    action.fadeOut(fadeSeconds);
  } else {
    action.stop();
  }
}

function currentAction(instance: AnimatedMeshInstanceRecord, ctx: string): THREE.AnimationAction {
  const action = instance.currentClip === null ? null : instance.actions.get(instance.currentClip) ?? null;
  if (!action) {
    throw new AnimatedMeshApplyError(`setAnimatedMeshPlayback.${ctx}: no current clip on ${instance.asset}`);
  }
  return action;
}

function toThreeLoop(loop: 'once' | 'repeat' | 'pingPong'): THREE.AnimationActionLoopStyles {
  switch (loop) {
    case 'once':
      return THREE.LoopOnce;
    case 'repeat':
      return THREE.LoopRepeat;
    case 'pingPong':
      return THREE.LoopPingPong;
  }
}

function poseSample(object: THREE.Object3D): AnimatedMeshPoseSample {
  const translation = [0, 0, 0] as [number, number, number];
  const rotation = [0, 0, 0, 0] as [number, number, number, number];
  const scale = [0, 0, 0] as [number, number, number];
  let nodeCount = 0;
  object.traverse((node) => {
    nodeCount += 1;
    translation[0] += node.position.x;
    translation[1] += node.position.y;
    translation[2] += node.position.z;
    rotation[0] += node.quaternion.x;
    rotation[1] += node.quaternion.y;
    rotation[2] += node.quaternion.z;
    rotation[3] += node.quaternion.w;
    scale[0] += node.scale.x;
    scale[1] += node.scale.y;
    scale[2] += node.scale.z;
  });
  return {
    rootTranslation: [object.position.x, object.position.y, object.position.z],
    rootRotation: [object.quaternion.x, object.quaternion.y, object.quaternion.z, object.quaternion.w],
    rootScale: [object.scale.x, object.scale.y, object.scale.z],
    hierarchyNodeCount: nodeCount,
    hierarchyTranslationSum: translation,
    hierarchyRotationSum: rotation,
    hierarchyScaleSum: scale,
  };
}

function playbackDiagnostics(
  instance: AnimatedMeshInstanceRecord,
  action: THREE.AnimationAction | null,
): readonly string[] {
  if (!instance.commandSelected) {
    return ['animation_not_started'];
  }
  if (instance.status === 'stopped') {
    return ['animation_stopped'];
  }
  if (instance.status === 'sampled') {
    return ['animation_sampled'];
  }
  if (action?.paused || instance.status === 'paused') {
    return ['animation_paused'];
  }
  return [];
}

const ANIMATED_MESH_SAMPLE_MAX_VERTICES = 1_000_000;
const ANIMATED_MESH_SAMPLE_MAX_DIAGNOSTICS = 64;

function diagnoseAnimatedMeshSample(
  instance: AnimatedMeshInstanceRecord,
  skinningFacts: AnimatedMeshSkinningFacts,
  clipId: string,
  normalizedTime: number,
  durationSeconds: number,
  assetBounds: AnimatedMeshSampleBounds,
  contentHash: string | null,
): AnimatedMeshSampleReadout {
  const diagnostics: AnimatedMeshSampleDiagnostic[] = [];
  const appendDiagnostic = (
    code: AnimatedMeshSampleDiagnosticCode,
    message: string,
    node: THREE.Object3D | null,
  ): void => {
    if (diagnostics.length < ANIMATED_MESH_SAMPLE_MAX_DIAGNOSTICS) {
      diagnostics.push({ code, message, node: nodeName(node) });
    }
  };
  let boneCount = 0;
  let sampledVertexCount = 0;
  let vertexBudgetExceeded = false;
  const sampledBounds = new THREE.Box3();
  const vertex = new THREE.Vector3();
  const skinMatrix = new THREE.Matrix4();

  instance.object.updateMatrixWorld(true);
  instance.object.traverse((node) => {
    if (!finiteTransform(node)) {
      appendDiagnostic('node_transform_non_finite', 'node transform contains a non-finite value', node);
    }
    const quaternionLengthSquared = node.quaternion.lengthSq();
    if (!Number.isFinite(quaternionLengthSquared) || quaternionLengthSquared < 1e-12) {
      appendDiagnostic('node_quaternion_invalid', 'node quaternion is non-finite or has zero length', node);
    }
    if (
      !Number.isFinite(node.scale.x)
      || !Number.isFinite(node.scale.y)
      || !Number.isFinite(node.scale.z)
      || Math.abs(node.scale.x) < 1e-12
      || Math.abs(node.scale.y) < 1e-12
      || Math.abs(node.scale.z) < 1e-12
    ) {
      appendDiagnostic('node_scale_invalid', 'node scale is non-finite or singular', node);
    }

    if (node instanceof THREE.Bone) {
      boneCount += 1;
    }
    if (!(node instanceof THREE.Mesh)) {
      return;
    }
    const positions = node.geometry.getAttribute('position');
    if (positions === undefined) {
      return;
    }
    if (sampledVertexCount + positions.count > ANIMATED_MESH_SAMPLE_MAX_VERTICES) {
      vertexBudgetExceeded = true;
      return;
    }
    if (node instanceof THREE.SkinnedMesh) {
      node.skeleton.update();
      for (let boneIndex = 0; boneIndex < node.skeleton.bones.length; boneIndex += 1) {
        const bone = node.skeleton.bones[boneIndex];
        const inverse = node.skeleton.boneInverses[boneIndex];
        if (bone === undefined || inverse === undefined) {
          continue;
        }
        skinMatrix.multiplyMatrices(bone.matrixWorld, inverse);
        if (!skinMatrix.elements.every(Number.isFinite)) {
          appendDiagnostic('bone_matrix_non_finite', 'bone skin matrix contains a non-finite value', bone);
        } else if (Math.abs(skinMatrix.determinant()) < 1e-12) {
          appendDiagnostic('bone_matrix_singular', 'bone skin matrix is singular', bone);
        }
      }
    }
    for (let index = 0; index < positions.count; index += 1) {
      vertex.fromBufferAttribute(positions, index);
      if (node instanceof THREE.SkinnedMesh) {
        node.applyBoneTransform(index, vertex);
      }
      node.localToWorld(vertex);
      sampledBounds.expandByPoint(vertex);
    }
    sampledVertexCount += positions.count;
  });

  if (vertexBudgetExceeded) {
    appendDiagnostic(
      'vertex_budget_exceeded',
      `sample contains more than ${ANIMATED_MESH_SAMPLE_MAX_VERTICES} vertices`,
      null,
    );
  }
  const readoutBounds = !sampledBounds.isEmpty() && !vertexBudgetExceeded
    ? boxReadout(sampledBounds)
    : null;
  if (
    readoutBounds !== null
    && boundsExpansionIsImplausible(assetBounds, readoutBounds, instance.object)
  ) {
    appendDiagnostic(
      'sampled_bounds_implausible',
      'sampled world bounds expand beyond eight times the admitted asset extent',
      null,
    );
  }
  return {
    handle: instance.handle,
    asset: instance.asset,
    contentHash,
    clip: clipId,
    normalizedTime,
    durationSeconds,
    assetBounds: {
      min: [...assetBounds.min],
      max: [...assetBounds.max],
    },
    sampledWorldBounds: readoutBounds,
    sampledVertexCount,
    boneCount,
    skinningFacts,
    diagnostics,
  };
}

const ANIMATED_MESH_SAMPLE_MAX_JOINTS = 256;
const NORMALIZED_WEIGHT_TOLERANCE = 1e-4;

function animatedMeshSkinningFacts(
  instance: THREE.Object3D,
  assetTemplate: THREE.Object3D,
  clip: THREE.AnimationClip,
): AnimatedMeshSkinningFacts {
  const templateBones = new Map<string, THREE.Bone>();
  const templateInverses = new Map<string, THREE.Matrix4>();
  const templateSkeletons = new Set<THREE.Skeleton>();
  const templateMeshes = new Map<string, THREE.Mesh>();
  assetTemplate.updateMatrixWorld(true);
  assetTemplate.traverse((node) => {
    if (node instanceof THREE.Bone) templateBones.set(node.name, node);
    if (node instanceof THREE.Mesh) templateMeshes.set(node.name, node);
    if (node instanceof THREE.SkinnedMesh) {
      templateSkeletons.add(node.skeleton);
      node.skeleton.bones.forEach((bone, index) => {
        const inverse = node.skeleton.boneInverses[index];
        if (inverse !== undefined) templateInverses.set(bone.name, inverse);
      });
    }
  });
  if (templateBones.size > ANIMATED_MESH_SAMPLE_MAX_JOINTS) {
    throw new AnimatedMeshApplyError(
      `sampleAnimatedMesh: joint count exceeds ${ANIMATED_MESH_SAMPLE_MAX_JOINTS}`,
    );
  }

  let skinnedMeshCount = 0;
  let inverseBindMatrixCount = 0;
  let inverseBindMatricesFinite = true;
  let weightedVertexCount = 0;
  let invalidWeightVertexCount = 0;
  let maximumWeightSumError = 0;
  let skeletonsIndependentFromTemplate = true;
  let sharedGeometryCount = 0;
  let sharedMaterialCount = 0;
  instance.updateMatrixWorld(true);
  instance.traverse((node) => {
    if (!(node instanceof THREE.Mesh)) return;
    const templateMesh = templateMeshes.get(node.name);
    if (templateMesh?.geometry === node.geometry) sharedGeometryCount += 1;
    if (templateMesh?.material === node.material) sharedMaterialCount += 1;
    if (!(node instanceof THREE.SkinnedMesh)) return;
    skinnedMeshCount += 1;
    if (templateSkeletons.has(node.skeleton)) skeletonsIndependentFromTemplate = false;
    node.skeleton.bones.forEach((bone, index) => {
      if (templateBones.get(bone.name) === bone) skeletonsIndependentFromTemplate = false;
      const inverse = node.skeleton.boneInverses[index];
      if (inverse !== undefined) {
        inverseBindMatrixCount += 1;
        if (!inverse.elements.every(Number.isFinite)) inverseBindMatricesFinite = false;
      }
    });
    const weights = node.geometry.getAttribute('skinWeight');
    if (weights === undefined) return;
    for (let index = 0; index < weights.count; index += 1) {
      const sum = weights.getX(index)
        + (weights.itemSize > 1 ? weights.getY(index) : 0)
        + (weights.itemSize > 2 ? weights.getZ(index) : 0)
        + (weights.itemSize > 3 ? weights.getW(index) : 0);
      weightedVertexCount += 1;
      if (!Number.isFinite(sum) || sum <= 0) {
        invalidWeightVertexCount += 1;
        continue;
      }
      maximumWeightSumError = Math.max(maximumWeightSumError, Math.abs(sum - 1));
    }
  });

  const interpolationModes = [...new Set(clip.tracks.map((track) => {
    switch (track.getInterpolation()) {
      case THREE.InterpolateDiscrete: return 'discrete' as const;
      case THREE.InterpolateSmooth: return 'smooth' as const;
      default: return 'linear' as const;
    }
  }))].sort(codeUnitCompare);
  return {
    joints: [...templateBones.values()].map((bone) => ({
      name: bone.name,
      parent: bone.parent instanceof THREE.Bone ? bone.parent.name : null,
      restLocalMatrix: [...bone.matrix.elements],
      inverseBindMatrix: templateInverses.has(bone.name)
        ? [...(templateInverses.get(bone.name) as THREE.Matrix4).elements]
        : null,
    })),
    skinnedMeshCount,
    inverseBindMatrixCount,
    inverseBindMatricesFinite,
    weightedVertexCount,
    invalidWeightVertexCount,
    maximumWeightSumError,
    weightsNormalized: weightedVertexCount > 0
      && invalidWeightVertexCount === 0
      && maximumWeightSumError <= NORMALIZED_WEIGHT_TOLERANCE,
    interpolationModes,
    instanceRootDistinctFromTemplate: instance !== assetTemplate,
    skeletonsIndependentFromTemplate,
    sharedGeometryCount,
    sharedMaterialCount,
  };
}

function finiteTransform(node: THREE.Object3D): boolean {
  return [
    ...node.position.toArray(),
    ...node.quaternion.toArray(),
    ...node.scale.toArray(),
    ...node.matrix.elements,
    ...node.matrixWorld.elements,
  ].every(Number.isFinite);
}

function nodeName(node: THREE.Object3D | null): string | null {
  if (node === null) return null;
  return node.name.length > 0 ? node.name : `${node.type}:${node.id}`;
}

function boxReadout(bounds: THREE.Box3): AnimatedMeshSampleBounds {
  return {
    min: bounds.min.toArray(),
    max: bounds.max.toArray(),
  };
}

function boundsExpansionIsImplausible(
  asset: AnimatedMeshSampleBounds,
  sampled: AnimatedMeshSampleBounds,
  object: THREE.Object3D,
): boolean {
  const assetExtent = Math.max(
    asset.max[0] - asset.min[0],
    asset.max[1] - asset.min[1],
    asset.max[2] - asset.min[2],
    1e-6,
  );
  const sampledExtent = Math.max(
    sampled.max[0] - sampled.min[0],
    sampled.max[1] - sampled.min[1],
    sampled.max[2] - sampled.min[2],
  );
  const worldScale = object.getWorldScale(new THREE.Vector3());
  const maximumWorldScale = Math.max(
    Math.abs(worldScale.x),
    Math.abs(worldScale.y),
    Math.abs(worldScale.z),
    1e-6,
  );
  return sampledExtent > assetExtent * maximumWorldScale * 8;
}
