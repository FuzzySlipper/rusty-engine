import * as THREE from 'three';
import { GLTFLoader, type GLTF } from 'three/examples/jsm/loaders/GLTFLoader.js';
import * as SkeletonUtils from 'three/examples/jsm/utils/SkeletonUtils.js';
import type {
  AnimatedMeshAsset,
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

export interface AnimatedMeshResource {
  readonly asset: string;
  readonly contentHash?: string | null;
  readonly scene: THREE.Object3D;
  readonly clips: readonly THREE.AnimationClip[];
}

export interface AnimatedMeshAssetSource {
  getAnimatedMeshResource(asset: AnimatedMeshAsset): AnimatedMeshResource | undefined;
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

interface AnimatedMeshAssetRecord {
  readonly asset: AnimatedMeshAsset;
  readonly resource: AnimatedMeshResource;
  readonly scene: THREE.Object3D;
  refCount: number;
}

interface AnimatedMeshInstanceRecord {
  readonly handle: RenderHandle;
  readonly asset: string;
  readonly object: THREE.Object3D;
  readonly mixer: THREE.AnimationMixer;
  readonly actions: ReadonlyMap<string, THREE.AnimationAction>;
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

  constructor(resources: readonly AnimatedMeshResource[]) {
    for (const resource of resources) {
      this.#resources.set(resource.asset, resource);
    }
  }

  getAnimatedMeshResource(asset: AnimatedMeshAsset): AnimatedMeshResource | undefined {
    return this.#resources.get(asset.asset);
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

export class AnimatedMeshRegistry {
  readonly #assetSource: AnimatedMeshAssetSource | undefined;
  readonly #assets = new Map<string, AnimatedMeshAssetRecord>();
  readonly #instances = new Map<RenderHandle, AnimatedMeshInstanceRecord>();

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
    const resource = this.#validatedResource(asset);
    const scene = createAnimatedMeshAssetScene(resource.scene);
    if (existing) {
      disposeAnimatedMeshAssetScene(existing.scene);
    }
    this.#assets.set(asset.asset, { asset, resource, scene, refCount: 0 });
  }

  validateDefinition(asset: AnimatedMeshAsset): void {
    this.#validatedResource(asset);
  }

  #validatedResource(asset: AnimatedMeshAsset): AnimatedMeshResource {
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
    return resource;
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
    for (const clip of record.asset.clips) {
      actions.set(clip.id, mixer.clipAction(requireClip(record.resource, clip.id, clip.name)));
    }
    const instanceRecord: AnimatedMeshInstanceRecord = {
      handle,
      asset: instance.asset,
      object,
      mixer,
      actions,
      currentClip: null,
      commandSelected: false,
      status: 'not_started',
      loop: null,
      speed: null,
      weight: null,
      controllerClips: [],
    };
    this.#instances.set(handle, instanceRecord);
    record.refCount += 1;
    if (instance.playback) {
      this.setPlayback(handle, instance.playback);
    }
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
    const asset = this.#assets.get(instance.asset);
    if (asset === undefined) {
      throw new AnimatedMeshApplyError(
        `sampleAnimatedMesh: missing defined asset ${instance.asset}`,
      );
    }
    return diagnoseAnimatedMeshSample(
      instance,
      asset.scene,
      action.getClip(),
      clipId,
      normalizedTime,
      durationSeconds,
      asset.asset.bounds,
      asset.asset.contentHash,
    );
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
  for (const clip of asset.clips) {
    requireClip(resource, clip.id, clip.name);
  }
}

function requireClip(
  resource: AnimatedMeshResource,
  id: string,
  name: string | null,
): THREE.AnimationClip {
  const clip = resource.clips.find((candidate) => candidate.name === id || (name !== null && candidate.name === name));
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
  assetTemplate: THREE.Object3D,
  clip: THREE.AnimationClip,
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
  const facts = animatedMeshSkinningFacts(instance.object, assetTemplate, clip);

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
    skinningFacts: facts,
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
      if (sum > 0) {
        weightedVertexCount += 1;
        maximumWeightSumError = Math.max(maximumWeightSumError, Math.abs(sum - 1));
      }
    }
  });

  const interpolationModes = [...new Set(clip.tracks.map((track) => {
    switch (track.getInterpolation()) {
      case THREE.InterpolateDiscrete: return 'discrete' as const;
      case THREE.InterpolateSmooth: return 'smooth' as const;
      default: return 'linear' as const;
    }
  }))].sort();
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
    maximumWeightSumError,
    weightsNormalized: weightedVertexCount > 0
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
