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

  define(asset: AnimatedMeshAsset): void {
    const existing = this.#assets.get(asset.asset);
    if (existing && existing.refCount > 0) {
      throw new AnimatedMeshApplyError(
        `defineAnimatedMesh: asset ${asset.asset} is in use by ${existing.refCount} instance(s)`,
      );
    }
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
    this.#assets.set(asset.asset, { asset, resource, refCount: 0 });
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
    const object = cloneAnimatedMeshInstance(record.resource.scene);
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

  release(handle: RenderHandle): void {
    const instance = this.#instances.get(handle);
    if (!instance) {
      return;
    }
    instance.mixer.stopAllAction();
    this.#instances.delete(handle);
    const asset = this.#assets.get(instance.asset);
    if (asset) {
      asset.refCount -= 1;
    }
  }

  #requireInstance(handle: RenderHandle, ctx: string): AnimatedMeshInstanceRecord {
    const instance = this.#instances.get(handle);
    if (!instance) {
      throw new AnimatedMeshApplyError(`${ctx}: handle ${handle} is not an animated mesh`);
    }
    return instance;
  }
}

function cloneAnimatedMeshInstance(source: THREE.Object3D): THREE.Object3D {
  const instance = SkeletonUtils.clone(source);
  instance.traverse((object) => {
    const mesh = object as THREE.Mesh;
    if (mesh.geometry instanceof THREE.BufferGeometry) {
      mesh.geometry = mesh.geometry.clone();
    }
    if (Array.isArray(mesh.material)) {
      mesh.material = mesh.material.map((material) => material.clone());
    } else if (mesh.material instanceof THREE.Material) {
      mesh.material = mesh.material.clone();
    }
  });
  return instance;
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
