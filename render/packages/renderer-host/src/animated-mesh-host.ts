import type { RenderFrameDiff, RenderHandle } from '@rusty-engine/render-contracts';
import {
  MapAnimatedMeshAssetSource,
  loadAnimationClipPackGlbResource,
  ThreeRenderer,
  loadAnimatedMeshGlbResource,
  type AnimatedMeshAssetSource,
} from '@rusty-engine/renderer-three/backend';
import { rendererResourceContentHash } from './resource-content-hash.js';

export type RendererHostDiagnosticCode =
  | 'animated_mesh_manifest_invalid'
  | 'animated_mesh_resource_unavailable'
  | 'animated_mesh_content_hash_mismatch'
  | 'animated_mesh_clip_unavailable'
  | 'animated_mesh_frame_rejected'
  | 'animated_mesh_incompatible_rig'
  | 'animated_mesh_missing_joint'
  | 'animated_mesh_malformed_channels'
  | 'animated_mesh_unsupported_root_policy'
  | 'animated_mesh_clip_pack_budget_exceeded'
  | 'renderer_lighting_policy_rejected'
  | 'animated_mesh_handle_unavailable'
  | 'animation_not_started'
  | 'animation_paused'
  | 'animation_stopped';

export interface RendererHostDiagnostic {
  readonly code: RendererHostDiagnosticCode;
  readonly message: string;
  readonly asset: string | null;
  readonly handle: RenderHandle | null;
}

export class RendererHostError extends Error {
  readonly diagnostics: readonly RendererHostDiagnostic[];

  constructor(diagnostics: readonly RendererHostDiagnostic[]) {
    super(diagnostics.map((diagnostic) => diagnostic.message).join('; '));
    this.name = 'RendererHostError';
    this.diagnostics = diagnostics;
  }
}

export interface RendererAnimatedMeshResourceDescriptor {
  readonly asset: string;
  readonly contentHash: string;
  /** Decoded GLB clip names in the same order as effective clip IDs. */
  readonly clipSourceNames?: readonly string[];
  readonly clipIds: readonly string[];
  /** Dense Engine slots mapped to explicit source GLB material indices. */
  readonly embeddedMaterialSlots?: readonly RendererAnimatedMeshEmbeddedMaterialSlot[];
}

export interface RendererAnimatedMeshEmbeddedMaterialSlot {
  readonly slot: number;
  readonly sourceMaterialSlot: number;
}

export interface RendererAnimationClipPackResourceDescriptor {
  readonly asset: string;
  readonly contentHash: string;
  /** Decoded GLB clip names in the same order as effective clip IDs. */
  readonly clipSourceNames?: readonly string[];
  readonly clipIds: readonly string[];
}

export interface RendererAnimatedMeshResourceManifest {
  readonly kind: 'rusty_renderer_animated_mesh_resources.v1';
  readonly resources: readonly RendererAnimatedMeshResourceDescriptor[];
  readonly clipPacks?: readonly RendererAnimationClipPackResourceDescriptor[];
}

/** Direct optional clip packs are deliberately bounded independently of base meshes. */
export const RUSTY_RENDERER_ANIMATED_CLIP_PACK_MAX_COUNT = 16;
export const RUSTY_RENDERER_ANIMATED_CLIP_PACK_MAX_TOTAL_BYTES = 32 * 1024 * 1024;

export type RendererAnimatedMeshResourceResolver = (
  descriptor: RendererAnimatedMeshResourceDescriptor,
) => Promise<ArrayBuffer>;

export interface RendererAnimatedMeshFrameReceipt {
  readonly applied: boolean;
  readonly diagnostics: readonly RendererHostDiagnostic[];
}

export interface RendererAnimatedMeshPoseSample {
  readonly rootTranslation: readonly [number, number, number];
  readonly rootRotation: readonly [number, number, number, number];
  readonly rootScale: readonly [number, number, number];
  readonly hierarchyNodeCount: number;
  readonly hierarchyTranslationSum: readonly [number, number, number];
  readonly hierarchyRotationSum: readonly [number, number, number, number];
  readonly hierarchyScaleSum: readonly [number, number, number];
}

export interface RendererAnimatedMeshPlaybackReadout {
  readonly handle: RenderHandle;
  readonly asset: string | null;
  readonly contentHash: string | null;
  readonly status: 'unavailable' | 'not_started' | 'playing' | 'paused' | 'sampled' | 'stopped';
  readonly selectedClip: string | null;
  /** Exact presentation sample held by the current retained frame, if any. */
  readonly heldSample: { readonly clip: string; readonly normalizedTime: number } | null;
  readonly mixerTimeSeconds: number;
  readonly actionTimeSeconds: number | null;
  readonly commandSelected: boolean;
  readonly running: boolean;
  readonly paused: boolean;
  readonly loop: 'once' | 'repeat' | 'pingPong' | null;
  readonly speed: number | null;
  readonly weight: number | null;
  readonly poseSample: RendererAnimatedMeshPoseSample | null;
  readonly diagnostics: readonly RendererHostDiagnostic[];
  readonly projectionOnly: true;
  readonly controllerClips: readonly RendererAnimationControllerClip[];
  readonly effectiveClips: readonly RendererAnimatedMeshEffectiveClip[];
}

export interface RendererAnimatedMeshEffectiveClip {
  readonly id: string;
  readonly origin: 'embedded' | 'pack';
  readonly durationSeconds: number;
}

/** Typed renderer observation with product identity but no renderer handle. */
export interface RendererAnimatedMeshNaturalCompletion {
  readonly objectId: number;
  readonly generation: number;
  readonly clip: string;
}

export interface RendererAnimatedMeshProjection {
  readonly kind: 'rusty_renderer_animated_mesh_projection.v1';
  readonly applyFrame: (frame: RenderFrameDiff) => RendererAnimatedMeshFrameReceipt;
  readonly advance: (deltaSeconds: number) => RendererAnimatedMeshFrameReceipt;
  readonly playback: (handle: RenderHandle) => RendererAnimatedMeshPlaybackReadout;
  readonly snapshot: () => string;
  readonly hasAnimationTarget: (handle: RenderHandle) => boolean;
  readonly setAnimationControllerWeights: (
    handle: RenderHandle,
    clips: readonly RendererAnimationControllerClip[],
  ) => void;
  readonly hasAnimationClips: (handle: RenderHandle, clipIds: readonly string[]) => boolean;
  readonly clearAnimationControllerWeights: (handle: RenderHandle) => void;
  readonly subscribeNaturalCompletions: (
    listener: (completion: RendererAnimatedMeshNaturalCompletion) => void,
  ) => () => void;
}

export interface RendererAnimationControllerClip {
  readonly clip: string;
  readonly weight: number;
  readonly speed: number;
}

export interface RendererAnimatedMeshProjectionOptions {
  readonly manifest: RendererAnimatedMeshResourceManifest;
  readonly resolveResource: RendererAnimatedMeshResourceResolver;
}

export async function createRendererAnimatedMeshProjection(
  options: RendererAnimatedMeshProjectionOptions,
): Promise<RendererAnimatedMeshProjection> {
  const source = await loadRendererAnimatedMeshSource(options.manifest, options.resolveResource);
  const renderer = new ThreeRenderer({ animatedMeshSource: source });
  return createProjectionController(renderer, contentHashesByAsset(options.manifest));
}

export async function loadRendererAnimatedMeshSource(
  manifest: RendererAnimatedMeshResourceManifest,
  resolver: RendererAnimatedMeshResourceResolver,
): Promise<AnimatedMeshAssetSource> {
  validateManifest(manifest);
  const resources = await Promise.all(manifest.resources.map(async (descriptor) => {
    let data: ArrayBuffer;
    try {
      data = await resolver(descriptor);
    } catch (cause) {
      throw hostError('animated_mesh_resource_unavailable', descriptor.asset, null, cause);
    }
    const immutableData = data.slice(0);
    const actualHash = await rendererResourceContentHash(immutableData, descriptor.contentHash);
    if (actualHash !== descriptor.contentHash) {
      throw hostError(
        'animated_mesh_content_hash_mismatch',
        descriptor.asset,
        null,
        `expected ${descriptor.contentHash}, received ${actualHash}`,
      );
    }
    const resource = await loadAnimatedMeshGlbResource(
      descriptor.asset,
      immutableData,
      descriptor.contentHash,
      descriptor.embeddedMaterialSlots,
    ).catch((cause: unknown) => {
      throw hostError('animated_mesh_resource_unavailable', descriptor.asset, null, cause);
    });
    const missingClip = missingDeclaredSourceClip(resource.clips, descriptor);
    if (missingClip !== undefined) {
      throw hostError('animated_mesh_clip_unavailable', descriptor.asset, null, `missing clip ${missingClip}`);
    }
    return resource;
  }));
  // Packs are optional add-ons. Admit them sequentially so an unbounded
  // Promise.all fanout cannot retain many decoded candidates before the first
  // failing pack is observed.
  const packs = [];
  let packBytes = 0;
  for (const descriptor of manifest.clipPacks ?? []) {
    let data: ArrayBuffer;
    try { data = await resolver(descriptor); } catch (cause) {
      throw hostError('animated_mesh_resource_unavailable', descriptor.asset, null, cause);
    }
    const immutableData = data.slice(0);
    packBytes += immutableData.byteLength;
    if (packBytes > RUSTY_RENDERER_ANIMATED_CLIP_PACK_MAX_TOTAL_BYTES) {
      throw hostError(
        'animated_mesh_clip_pack_budget_exceeded', descriptor.asset, null,
        `animated clip packs exceed ${String(RUSTY_RENDERER_ANIMATED_CLIP_PACK_MAX_TOTAL_BYTES)} bytes`,
      );
    }
    const actualHash = await rendererResourceContentHash(immutableData, descriptor.contentHash);
    if (actualHash !== descriptor.contentHash) {
      throw hostError('animated_mesh_content_hash_mismatch', descriptor.asset, null, `expected ${descriptor.contentHash}, received ${actualHash}`);
    }
    const resource = await loadAnimationClipPackGlbResource(descriptor.asset, immutableData, descriptor.contentHash).catch((cause: unknown) => {
      throw hostError('animated_mesh_resource_unavailable', descriptor.asset, null, cause);
    });
    const missingClip = missingDeclaredSourceClip(resource.clips, descriptor);
    if (missingClip !== undefined) throw hostError('animated_mesh_clip_unavailable', descriptor.asset, null, `missing clip ${missingClip}`);
    packs.push(resource);
  }
  return new MapAnimatedMeshAssetSource(resources, packs);
}

function missingDeclaredSourceClip(
  clips: readonly { readonly name: string }[],
  descriptor: RendererAnimatedMeshResourceDescriptor | RendererAnimationClipPackResourceDescriptor,
): string | undefined {
  const sourceNames = descriptor.clipSourceNames ?? descriptor.clipIds;
  if (sourceNames.length !== descriptor.clipIds.length || new Set(sourceNames).size !== sourceNames.length) {
    return 'invalid source clip declaration';
  }
  const counts = new Map<string, number>();
  clips.forEach((clip) => counts.set(clip.name, (counts.get(clip.name) ?? 0) + 1));
  return sourceNames.find((sourceName) => counts.get(sourceName) !== 1);
}

export function animationPlaybackReadout(
  handle: RenderHandle,
  readout: BackendAnimatedMeshPlaybackReadout | undefined,
  contentHash: string | null = null,
): RendererAnimatedMeshPlaybackReadout {
  if (readout === undefined) {
    return {
      handle,
      asset: null,
      contentHash: null,
      status: 'unavailable',
      selectedClip: null,
      heldSample: null,
      mixerTimeSeconds: 0,
      actionTimeSeconds: null,
      commandSelected: false,
      running: false,
      paused: false,
      loop: null,
      speed: null,
      weight: null,
      poseSample: null,
      diagnostics: [diagnostic('animated_mesh_handle_unavailable', null, handle, `animated mesh handle ${handle} is unavailable`)],
      projectionOnly: true,
      controllerClips: [],
      effectiveClips: [],
    };
  }
  return {
    handle,
    asset: readout.asset,
    contentHash,
    status: readout.status,
    selectedClip: readout.currentClip,
    heldSample: readout.heldSample,
    mixerTimeSeconds: readout.mixerTimeSeconds,
    actionTimeSeconds: readout.actionTimeSeconds,
    commandSelected: readout.commandSelected,
    running: readout.running,
    paused: readout.paused,
    loop: readout.loop,
    speed: readout.speed,
    weight: readout.weight,
    poseSample: readout.poseSample,
    diagnostics: readout.diagnostics.map((code) => diagnostic(animationDiagnosticCode(code), readout.asset, handle, code)),
    projectionOnly: true,
    controllerClips: readout.controllerClips,
    effectiveClips: readout.effectiveClips,
  };
}

interface BackendAnimatedMeshPlaybackReadout {
  readonly asset: string;
  readonly status: 'not_started' | 'playing' | 'paused' | 'sampled' | 'stopped';
  readonly currentClip: string | null;
  readonly heldSample: { readonly clip: string; readonly normalizedTime: number } | null;
  readonly mixerTimeSeconds: number;
  readonly actionTimeSeconds: number | null;
  readonly commandSelected: boolean;
  readonly running: boolean;
  readonly paused: boolean;
  readonly loop: 'once' | 'repeat' | 'pingPong' | null;
  readonly speed: number | null;
  readonly weight: number | null;
  readonly poseSample: RendererAnimatedMeshPoseSample;
  readonly diagnostics: readonly string[];
  readonly controllerClips: readonly RendererAnimationControllerClip[];
  readonly effectiveClips: readonly RendererAnimatedMeshEffectiveClip[];
}

function createProjectionController(
  renderer: ThreeRenderer,
  contentHashes: ReadonlyMap<string, string>,
): RendererAnimatedMeshProjection {
  return {
    kind: 'rusty_renderer_animated_mesh_projection.v1',
    applyFrame: (frame) => applyRendererOperation(() => renderer.applyFrame(frame)),
    advance: (deltaSeconds) => applyRendererOperation(() => renderer.advanceAnimation(deltaSeconds)),
    playback: (handle) => {
      const playback = renderer.animatedMeshPlayback(handle);
      return animationPlaybackReadout(
        handle,
        playback,
        playback === undefined ? null : contentHashes.get(playback.asset) ?? null,
      );
    },
    snapshot: () => renderer.snapshot(),
    hasAnimationTarget: (handle) => renderer.has(handle),
    setAnimationControllerWeights: (handle, clips) => {
      renderer.setAnimationControllerWeights(handle, clips);
    },
    hasAnimationClips: (handle, clipIds) => renderer.hasAnimationControllerClips(handle, clipIds),
    clearAnimationControllerWeights: (handle) => renderer.clearAnimationControllerWeights(handle),
    subscribeNaturalCompletions: (listener) => renderer.subscribeAnimatedMeshNaturalCompletions(listener),
  };
}

function contentHashesByAsset(
  manifest: RendererAnimatedMeshResourceManifest,
): ReadonlyMap<string, string> {
  return new Map(
    manifest.resources.map((resource) => [resource.asset, resource.contentHash] as const),
  );
}

function applyRendererOperation(operation: () => void): RendererAnimatedMeshFrameReceipt {
  try {
    operation();
    return { applied: true, diagnostics: [] };
  } catch (cause) {
    const message = errorMessage(cause);
    return {
      applied: false,
      diagnostics: [diagnostic(animationResourceDiagnosticCode(message), null, null, message)],
    };
  }
}

function animationResourceDiagnosticCode(message: string): RendererHostDiagnosticCode {
  if (message.includes('missing target joint') || message.includes('missing source joint')) return 'animated_mesh_missing_joint';
  if (message.includes('malformed or unsupported channels')) return 'animated_mesh_malformed_channels';
  if (message.includes('unsupported root-motion declaration')) return 'animated_mesh_unsupported_root_policy';
  if (message.includes('incompatible rig signature') || message.includes('inverse bind')) return 'animated_mesh_incompatible_rig';
  return 'animated_mesh_frame_rejected';
}

function validateManifest(manifest: RendererAnimatedMeshResourceManifest): void {
  if (manifest.kind !== 'rusty_renderer_animated_mesh_resources.v1' || manifest.resources.length === 0) {
    throw hostError('animated_mesh_manifest_invalid', null, null, 'animated mesh resource manifest is empty or unsupported');
  }
  const assets = new Set<string>();
  for (const resource of manifest.resources) {
    const validHash = /^(?:sha256:[0-9a-f]{64}|[0-9a-f]{16})$/u.test(resource.contentHash);
    const sourceNames = resource.clipSourceNames ?? resource.clipIds;
    const validClips = new Set(resource.clipIds).size === resource.clipIds.length
      && sourceNames.length === resource.clipIds.length && new Set(sourceNames).size === sourceNames.length;
    if (resource.asset.length === 0 || !validHash || !validClips
      || !validEmbeddedMaterialSlots(resource.embeddedMaterialSlots ?? [])
      || assets.has(resource.asset)) {
      throw hostError('animated_mesh_manifest_invalid', resource.asset || null, null, 'animated mesh resource descriptor is invalid or duplicated');
    }
    assets.add(resource.asset);
  }
  const packs = new Set<string>();
  if ((manifest.clipPacks?.length ?? 0) > RUSTY_RENDERER_ANIMATED_CLIP_PACK_MAX_COUNT) {
    throw hostError('animated_mesh_clip_pack_budget_exceeded', null, null, 'animated clip pack count exceeds the aggregate limit');
  }
  for (const resource of manifest.clipPacks ?? []) {
    const validHash = /^(?:sha256:[0-9a-f]{64}|[0-9a-f]{16})$/u.test(resource.contentHash);
    const sourceNames = resource.clipSourceNames ?? resource.clipIds;
    const validClips = resource.clipIds.length > 0 && resource.clipIds.length <= 256
      && new Set(resource.clipIds).size === resource.clipIds.length
      && sourceNames.length === resource.clipIds.length && new Set(sourceNames).size === sourceNames.length;
    if (resource.asset.length === 0 || !validHash || !validClips || packs.has(resource.asset) || assets.has(resource.asset)) {
      throw hostError('animated_mesh_manifest_invalid', resource.asset || null, null, 'animation clip pack descriptor is invalid or duplicated');
    }
    packs.add(resource.asset);
  }
}

export function validEmbeddedMaterialSlots(
  slots: readonly RendererAnimatedMeshEmbeddedMaterialSlot[],
): boolean {
  const sources = new Set<number>();
  for (const [index, binding] of slots.entries()) {
    if (!Number.isSafeInteger(binding.slot)
      || binding.slot !== index
      || !Number.isSafeInteger(binding.sourceMaterialSlot)
      || binding.sourceMaterialSlot < 0
      || binding.sourceMaterialSlot > 65_535
      || sources.has(binding.sourceMaterialSlot)) {
      return false;
    }
    sources.add(binding.sourceMaterialSlot);
  }
  return true;
}

function animationDiagnosticCode(code: string): RendererHostDiagnosticCode {
  switch (code) {
    case 'animation_not_started':
    case 'animation_paused':
    case 'animation_stopped':
      return code;
    default:
      return 'animated_mesh_frame_rejected';
  }
}

function hostError(
  code: RendererHostDiagnosticCode,
  asset: string | null,
  handle: RenderHandle | null,
  cause: unknown,
): RendererHostError {
  return new RendererHostError([diagnostic(code, asset, handle, errorMessage(cause))]);
}

function diagnostic(
  code: RendererHostDiagnosticCode,
  asset: string | null,
  handle: RenderHandle | null,
  message: string,
): RendererHostDiagnostic {
  return { code, message, asset, handle };
}

function errorMessage(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}
