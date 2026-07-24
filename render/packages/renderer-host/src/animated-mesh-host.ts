import type { RenderFrameDiff, RenderHandle } from '@rusty-engine/render-contracts';
import {
  MapAnimatedMeshAssetSource,
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
  readonly clipIds: readonly string[];
}

export interface RendererAnimatedMeshResourceManifest {
  readonly kind: 'rusty_renderer_animated_mesh_resources.v1';
  readonly resources: readonly RendererAnimatedMeshResourceDescriptor[];
}

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
  readonly status: 'unavailable' | 'not_started' | 'playing' | 'paused' | 'stopped';
  readonly selectedClip: string | null;
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
    const actualHash = await rendererResourceContentHash(data, descriptor.contentHash);
    if (actualHash !== descriptor.contentHash) {
      throw hostError(
        'animated_mesh_content_hash_mismatch',
        descriptor.asset,
        null,
        `expected ${descriptor.contentHash}, received ${actualHash}`,
      );
    }
    const resource = await loadAnimatedMeshGlbResource(descriptor.asset, data, descriptor.contentHash).catch((cause: unknown) => {
      throw hostError('animated_mesh_resource_unavailable', descriptor.asset, null, cause);
    });
    const availableClips = new Set(resource.clips.map((clip) => clip.name.toLowerCase()));
    const missingClip = descriptor.clipIds.find((clip) => !availableClips.has(clip.toLowerCase()));
    if (missingClip !== undefined) {
      throw hostError('animated_mesh_clip_unavailable', descriptor.asset, null, `missing clip ${missingClip}`);
    }
    return resource;
  }));
  return new MapAnimatedMeshAssetSource(resources);
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
    };
  }
  return {
    handle,
    asset: readout.asset,
    contentHash,
    status: readout.status,
    selectedClip: readout.currentClip,
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
  };
}

interface BackendAnimatedMeshPlaybackReadout {
  readonly asset: string;
  readonly status: 'not_started' | 'playing' | 'paused' | 'stopped';
  readonly currentClip: string | null;
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
    return {
      applied: false,
      diagnostics: [diagnostic('animated_mesh_frame_rejected', null, null, errorMessage(cause))],
    };
  }
}

function validateManifest(manifest: RendererAnimatedMeshResourceManifest): void {
  if (manifest.kind !== 'rusty_renderer_animated_mesh_resources.v1' || manifest.resources.length === 0) {
    throw hostError('animated_mesh_manifest_invalid', null, null, 'animated mesh resource manifest is empty or unsupported');
  }
  const assets = new Set<string>();
  for (const resource of manifest.resources) {
    const validHash = /^(?:sha256:[0-9a-f]{64}|[0-9a-f]{16})$/u.test(resource.contentHash);
    const validClips = new Set(resource.clipIds).size === resource.clipIds.length;
    if (resource.asset.length === 0 || !validHash || !validClips || assets.has(resource.asset)) {
      throw hostError('animated_mesh_manifest_invalid', resource.asset || null, null, 'animated mesh resource descriptor is invalid or duplicated');
    }
    assets.add(resource.asset);
  }
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
