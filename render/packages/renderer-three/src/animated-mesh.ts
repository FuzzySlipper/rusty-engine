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
  readonly status: 'not_started' | 'playing' | 'paused' | 'stopped';
  readonly currentClip: string | null;
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
  readonly packs: readonly AnimationClipPackResource[];
  readonly generation: number;
  refCount: number;
}

interface AnimatedMeshInstanceRecord {
  readonly handle: RenderHandle;
  readonly asset: string;
  readonly object: THREE.Object3D;
  readonly mixer: THREE.AnimationMixer;
  readonly actions: ReadonlyMap<string, THREE.AnimationAction>;
  readonly clipOrigins: ReadonlyMap<string, 'embedded' | 'pack'>;
  currentClip: string | null;
  commandSelected: boolean;
  status: AnimatedMeshPlaybackReadout['status'];
  loop: AnimatedMeshPlaybackReadout['loop'];
  speed: number | null;
  weight: number | null;
  controllerClips: readonly AnimatedMeshControllerClip[];
}

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
  };
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

  constructor(assetSource: AnimatedMeshAssetSource | undefined) {
    this.#assetSource = assetSource;
  }

  get instanceCount(): number {
    return this.#instances.size;
  }

  define(asset: AnimatedMeshAsset): void {
    const existing = this.#assets.get(asset.asset);
    if (existing && existing.refCount > 0) {
      throw new AnimatedMeshApplyError(
        `defineAnimatedMesh: asset ${asset.asset} is in use by ${existing.refCount} instance(s)`,
      );
    }
    const { resource, packs } = this.#validatedResource(asset);
    const scene = createAnimatedMeshAssetScene(resource.scene);
    if (existing) {
      disposeAnimatedMeshAssetScene(existing.scene);
    }
    const generation = (this.#assetGenerations.get(asset.asset) ?? 0) + 1;
    this.#assetGenerations.set(asset.asset, generation);
    this.#assets.set(asset.asset, { asset, resource, scene, packs, generation, refCount: 0 });
  }

  validateDefinition(asset: AnimatedMeshAsset): void {
    this.#validatedResource(asset);
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

  create(handle: RenderHandle, instance: AnimatedMeshInstanceDescriptor): AnimatedMeshInstanceRecord {
    const record = this.#assets.get(instance.asset);
    if (!record) {
      throw new AnimatedMeshApplyError(`createAnimatedMeshInstance: undefined animated mesh asset ${instance.asset}`);
    }
    if (instance.materialOverrides.length > 0) {
      throw new AnimatedMeshApplyError(
        `createAnimatedMeshInstance: material overrides are not implemented for animated mesh ${instance.asset}`,
      );
    }
    const object = SkeletonUtils.clone(record.scene);
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
    const instanceRecord: AnimatedMeshInstanceRecord = {
      handle,
      asset: instance.asset,
      object,
      mixer,
      actions,
      clipOrigins,
      currentClip: null,
      commandSelected: false,
      status: 'not_started',
      loop: null,
      speed: null,
      weight: null,
      controllerClips: [],
    };
    // Validate optional initial playback against a detached instance first;
    // rejected creation must not publish an instance or consume a refcount.
    if (instance.playback) applyPlaybackCommand(instanceRecord, instance.playback);
    this.#instances.set(handle, instanceRecord);
    record.refCount += 1;
    return instanceRecord;
  }

  setPlayback(handle: RenderHandle, command: AnimatedMeshPlaybackCommand): void {
    const instance = this.#requireInstance(handle, 'setAnimatedMeshPlayback');
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
    instance.mixer.stopAllAction();
    instance.currentClip = null;
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
    if (!Number.isFinite(normalizedTime) || normalizedTime < 0 || normalizedTime > 1) {
      throw new AnimatedMeshApplyError(
        'sampleAnimatedMesh: normalizedTime must be finite and between 0 and 1',
      );
    }
    const instance = this.#requireInstance(handle, 'sampleAnimatedMesh');
    const action = instance.actions.get(clipId);
    if (action === undefined) {
      throw new AnimatedMeshApplyError(
        `sampleAnimatedMesh: missing clip ${clipId} on ${instance.asset}`,
      );
    }
    const durationSeconds = action.getClip().duration;
    if (!Number.isFinite(durationSeconds) || durationSeconds <= 0) {
      throw new AnimatedMeshApplyError(
        `sampleAnimatedMesh: clip ${clipId} has an invalid duration`,
      );
    }
    const asset = this.#assets.get(instance.asset);
    if (asset === undefined) {
      throw new AnimatedMeshApplyError(
        `sampleAnimatedMesh: missing defined asset ${instance.asset}`,
      );
    }
    // Skinning inspection is a bounded preflight. It must complete before the
    // disposable mixer or playback record changes so rejection is fail-atomic.
    const skinningFacts = animatedMeshSkinningFacts(
      instance.object,
      asset.scene,
      action.getClip(),
    );
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
    instance.commandSelected = true;
    instance.status = 'paused';
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
      },
    });
  }

  release(handle: RenderHandle): void {
    const instance = this.#instances.get(handle);
    if (!instance) {
      return;
    }
    instance.mixer.stopAllAction();
    instance.mixer.uncacheRoot(instance.object);
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

  #requireInstance(handle: RenderHandle, ctx: string): AnimatedMeshInstanceRecord {
    const instance = this.#instances.get(handle);
    if (!instance) {
      throw new AnimatedMeshApplyError(`${ctx}: handle ${handle} is not an animated mesh`);
    }
    return instance;
  }
}

function createAnimatedMeshAssetScene(source: THREE.Object3D): THREE.Object3D {
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
  return scene;
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
  const roots = pack.rig.joints.filter((joint) => joint.parent === null);
  if (roots.length !== 1) throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: incompatible rig signature (root)`);
  for (const [, clip] of requireDescriptorClips(resource, pack.clips)) {
    assertClipChannels(pack, clip, new Set(pack.rig.joints.map((joint) => joint.id)));
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
  const translated = new Set<string>();
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
      if (joint !== pack.rig.rootJointId) throw new AnimatedMeshApplyError(`clip pack ${pack.asset}: unsupported root-motion declaration for ${clip.name}`);
      if (pack.rig.rootConvention === 'inPlace') assertInPlaceHorizontal(track, pack, clip);
      translated.add(joint);
    }
  }
  if (pack.rig.rootConvention === 'authoredRootTranslation'
    && (translated.size !== 1 || !translated.has(pack.rig.rootJointId))) {
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
      stopCurrent(instance, command.fadeSeconds);
      instance.currentClip = null;
      instance.commandSelected = true;
      instance.status = 'stopped';
      instance.loop = null;
      instance.speed = null;
      instance.weight = null;
      return;
    case 'pause':
      currentAction(instance, 'pause').paused = true;
      instance.commandSelected = true;
      instance.status = 'paused';
      return;
    case 'resume': {
      const action = currentAction(instance, 'resume');
      action.paused = false;
      action.play();
      instance.commandSelected = true;
      instance.status = 'playing';
      return;
    }
  }
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
  }
  instance.currentClip = clips.reduce((selected, clip) =>
    selected === null || clip.weight > selected.weight ? clip : selected, null as AnimatedMeshControllerClip | null)?.clip ?? null;
  instance.commandSelected = false;
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
  instance.currentClip = command.clip;
  instance.controllerClips = [];
  instance.commandSelected = true;
  instance.status = 'playing';
  instance.loop = command.loop;
  instance.speed = command.speed;
  instance.weight = command.weight;
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
