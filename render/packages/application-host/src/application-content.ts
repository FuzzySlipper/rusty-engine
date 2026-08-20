import {
  RUSTY_RENDERER_MESH_RESOURCE_MAX_BYTES,
  RUSTY_RENDERER_MESH_RESOURCE_MAX_COUNT,
  RUSTY_RENDERER_MESH_RESOURCE_MAX_TOTAL_BYTES,
  RUSTY_RENDERER_TEXTURE_RESOURCE_MAX_BYTES,
  RUSTY_RENDERER_TEXTURE_RESOURCE_MAX_COUNT,
  RUSTY_RENDERER_TEXTURE_RESOURCE_MAX_TOTAL_BYTES,
  type RendererMeshResourceDescriptor,
  type RendererMeshResourceManifest,
  type RendererAudioResourceResolver,
  type RendererAnimatedMeshResourceDescriptor,
  type RendererAnimationClipPackResourceDescriptor,
  type RendererAnimatedMeshResourceManifest,
  type RendererAnimatedMeshResourceResolver,
  type RendererTextureResourceDescriptor,
  type RendererTextureResourceManifest,
} from '@rusty-engine/renderer-host';

import type { RustyApplicationFrame } from './application-host.js';

export type RustyApplicationResourceKind = 'audio' | 'mesh' | 'clipPack' | 'texture';

export const RUSTY_APPLICATION_AUDIO_RESOURCE_MAX_BYTES = 8 * 1024 * 1024;
export const RUSTY_APPLICATION_AUDIO_RESOURCE_MAX_COUNT = 64;
export const RUSTY_APPLICATION_AUDIO_RESOURCE_MAX_TOTAL_BYTES = 32 * 1024 * 1024;

export interface RustyApplicationResource {
  readonly identity: string;
  readonly contentHash: string;
  readonly mediaType: string;
  readonly bytes: Uint8Array;
}

export interface RustyApplicationContent {
  readonly frame: RustyApplicationFrame;
  readonly resources?: readonly RustyApplicationResource[];
}

export type RustyApplicationContentDiagnosticCode =
  | 'content_invalid'
  | 'resource_duplicate'
  | 'resource_identity_invalid'
  | 'resource_limit_exceeded'
  | 'resource_media_type_unsupported';

export class RustyApplicationContentError extends Error {
  constructor(
    readonly code: RustyApplicationContentDiagnosticCode,
    readonly resource: string | null,
    message: string,
  ) {
    super(message);
    this.name = 'RustyApplicationContentError';
  }
}

export interface PreparedRustyApplicationResource {
  readonly identity: string;
  readonly contentHash: string;
  readonly mediaType: string;
  readonly bytes: ArrayBuffer;
  readonly kind: RustyApplicationResourceKind;
}

export interface PreparedRustyApplicationContent {
  readonly frame: RustyApplicationFrame;
  readonly resources: readonly PreparedRustyApplicationResource[];
  readonly resourceBytes: number;
}

export interface RustyApplicationSurfaceResourceOptions {
  readonly animatedMeshManifest?: RendererAnimatedMeshResourceManifest;
  readonly resolveAnimatedMeshResource?: RendererAnimatedMeshResourceResolver;
  readonly meshResourceManifest?: RendererMeshResourceManifest;
  readonly resolveMeshResource?: (
    descriptor: RendererMeshResourceDescriptor,
  ) => Promise<ArrayBuffer>;
  readonly textureResourceManifest?: RendererTextureResourceManifest;
  readonly resolveTextureResource?: (
    descriptor: RendererTextureResourceDescriptor,
  ) => Promise<ArrayBuffer>;
}

const SHA256_IDENTITY = /^(audio|mesh|clip-pack|texture)-resource\/([0-9a-f]{64})$/u;

export function prepareRustyApplicationContent(
  content: RustyApplicationContent,
): PreparedRustyApplicationContent {
  if (typeof content !== 'object' || content === null || typeof content.frame !== 'object'
    || content.frame === null) {
    throw contentError('content_invalid', null, 'application content must include one frame');
  }
  if (content.resources !== undefined && !Array.isArray(content.resources)) {
    throw contentError('content_invalid', null, 'application content resources must be an array');
  }
  const frame = structuredClone(content.frame);
  const identities = new Set<string>();
  let meshCount = 0;
  let meshBytes = 0;
  let audioCount = 0;
  let audioBytes = 0;
  let textureCount = 0;
  let textureBytes = 0;
  const resources = (content.resources ?? []).map((resource, index) => {
    if (typeof resource !== 'object' || resource === null
      || typeof resource.identity !== 'string'
      || typeof resource.contentHash !== 'string'
      || typeof resource.mediaType !== 'string'
      || !(resource.bytes instanceof Uint8Array)) {
      throw contentError(
        'content_invalid',
        null,
        `application content resource ${String(index)} is malformed`,
      );
    }
    const match = SHA256_IDENTITY.exec(resource.identity);
    const digest = /^sha256:([0-9a-f]{64})$/u.exec(resource.contentHash)?.[1];
    if (match === null || digest === undefined || match[2] !== digest) {
      throw contentError(
        'resource_identity_invalid',
        resource.identity || null,
        'application resource identity must match its lowercase SHA-256 content hash',
      );
    }
    if (identities.has(resource.identity)) {
      throw contentError(
        'resource_duplicate',
        resource.identity,
        'application resource identity is duplicated',
      );
    }
    identities.add(resource.identity);
    const kind = match[1] === 'clip-pack' ? 'clipPack' : match[1] as RustyApplicationResourceKind;
    if (kind === 'audio') {
      if (resource.mediaType !== 'audio/wav') {
        throw contentError(
          'resource_media_type_unsupported',
          resource.identity,
          'audio resources must use audio/wav',
        );
      }
      audioCount += 1;
      audioBytes += resource.bytes.byteLength;
      if (audioCount > RUSTY_APPLICATION_AUDIO_RESOURCE_MAX_COUNT
        || resource.bytes.byteLength < 44
        || resource.bytes.byteLength > RUSTY_APPLICATION_AUDIO_RESOURCE_MAX_BYTES
        || audioBytes > RUSTY_APPLICATION_AUDIO_RESOURCE_MAX_TOTAL_BYTES) {
        throw contentError(
          'resource_limit_exceeded',
          resource.identity,
          'audio resource count or byte length exceeds the application-host bound',
        );
      }
    } else if (kind === 'texture') {
      if (resource.mediaType !== 'image/png') {
        throw contentError(
          'resource_media_type_unsupported',
          resource.identity,
          'texture resources must use image/png',
        );
      }
      textureCount += 1;
      textureBytes += resource.bytes.byteLength;
      if (textureCount > RUSTY_RENDERER_TEXTURE_RESOURCE_MAX_COUNT
        || resource.bytes.byteLength === 0
        || resource.bytes.byteLength > RUSTY_RENDERER_TEXTURE_RESOURCE_MAX_BYTES
        || textureBytes > RUSTY_RENDERER_TEXTURE_RESOURCE_MAX_TOTAL_BYTES) {
        throw contentError(
          'resource_limit_exceeded',
          resource.identity,
          'texture resource count or byte length exceeds the application-host bound',
        );
      }
    } else {
      if (resource.mediaType !== 'application/octet-stream') {
        throw contentError(
          'resource_media_type_unsupported',
          resource.identity,
          'mesh resources must use application/octet-stream',
        );
      }
      meshCount += 1;
      meshBytes += resource.bytes.byteLength;
      if (meshCount > RUSTY_RENDERER_MESH_RESOURCE_MAX_COUNT
        || resource.bytes.byteLength < 16
        || resource.bytes.byteLength > RUSTY_RENDERER_MESH_RESOURCE_MAX_BYTES
        || meshBytes > RUSTY_RENDERER_MESH_RESOURCE_MAX_TOTAL_BYTES) {
        throw contentError(
          'resource_limit_exceeded',
          resource.identity,
          'mesh resource count or byte length exceeds the application-host bound',
        );
      }
    }
    return Object.freeze({
      identity: resource.identity,
      contentHash: resource.contentHash,
      mediaType: resource.mediaType,
      bytes: resource.bytes.slice().buffer,
      kind,
    });
  });
  return Object.freeze({
    frame,
    resources: Object.freeze(resources),
    resourceBytes: audioBytes + meshBytes + textureBytes,
  });
}

export function rustyApplicationAudioResourceResolver(
  content: PreparedRustyApplicationContent,
): RendererAudioResourceResolver | null {
  const audio = content.resources.filter((resource) => resource.kind === 'audio');
  if (audio.length === 0) return null;
  const entries = new Map(audio.map((resource) => [resource.contentHash, resource]));
  return (clip) => {
    const entry = entries.get(clip.contentHash);
    if (entry === undefined) {
      return Promise.reject(new Error(
        `audio resource ${clip.asset} (${clip.contentHash}) is unavailable`,
      ));
    }
    return Promise.resolve({
      bytes: entry.bytes.slice(0),
      contentHash: entry.contentHash,
    });
  };
}

export function rustyApplicationSurfaceResourceOptions(
  content: PreparedRustyApplicationContent,
): RustyApplicationSurfaceResourceOptions {
  const entries = new Map(content.resources.map((resource) => [resource.identity, resource]));
  const animated = animatedMeshDescriptors(content.frame);
  const clipPacks = animationClipPacks(content.frame);
  const clipPackAssets = new Set(clipPacks.map((pack) => pack.asset));
  const mesh = content.resources.filter((resource) => resource.kind === 'mesh');
  const textures = content.resources.filter((resource) => resource.kind === 'texture');
  return Object.freeze({
    ...(animated.length === 0 ? {} : {
      animatedMeshManifest: {
        kind: 'rusty_renderer_animated_mesh_resources.v1' as const,
        resources: Object.freeze(animated),
        ...(clipPacks.length === 0 ? {} : { clipPacks: Object.freeze(clipPacks) }),
      },
      resolveAnimatedMeshResource: (descriptor: RendererAnimatedMeshResourceDescriptor) =>
        resolveResource(entries, `${clipPackAssets.has(descriptor.asset) ? 'clip-pack' : 'mesh'}-resource/${descriptor.contentHash.slice('sha256:'.length)}`),
    }),
    ...(mesh.length === 0 ? {} : {
      meshResourceManifest: {
        kind: 'rusty_renderer_mesh_resources.v1' as const,
        resources: Object.freeze(mesh.map(resourceDescriptor)),
      },
      resolveMeshResource: (descriptor: RendererMeshResourceDescriptor) =>
        resolveResource(entries, descriptor.resource),
    }),
    ...(textures.length === 0 ? {} : {
      textureResourceManifest: {
        kind: 'rusty_renderer_texture_resources.v1' as const,
        resources: Object.freeze(textures.map(resourceDescriptor)),
      },
      resolveTextureResource: (descriptor: RendererTextureResourceDescriptor) =>
        resolveResource(entries, descriptor.resource),
    }),
  });
}

function animationClipPacks(frame: RustyApplicationFrame): readonly RendererAnimationClipPackResourceDescriptor[] {
  if (!Array.isArray(frame['ops'])) return [];
  const packs: RendererAnimationClipPackResourceDescriptor[] = [];
  const identities = new Set<string>();
  frame['ops'].forEach((operation) => {
    if (typeof operation !== 'object' || operation === null || (operation as { readonly op?: unknown }).op !== 'defineAnimatedMesh') return;
    const candidate = (operation as { readonly asset?: { readonly clipPacks?: unknown } }).asset;
    if (!candidate || !Array.isArray(candidate.clipPacks)) return;
    candidate.clipPacks.forEach((pack) => {
      if (typeof pack !== 'object' || pack === null) return;
      const value = pack as { readonly asset?: unknown; readonly contentHash?: unknown; readonly clips?: unknown };
      if (typeof value.asset !== 'string' || typeof value.contentHash !== 'string' || !Array.isArray(value.clips) || identities.has(value.asset)) return;
      const clips = value.clips.map((clip) => typeof clip === 'object' && clip !== null
        ? { id: (clip as { readonly id?: unknown }).id, name: (clip as { readonly name?: unknown }).name }
        : undefined);
      if (!clips.every((clip): clip is { readonly id: string; readonly name: string | null } => clip !== undefined
        && typeof clip.id === 'string' && (typeof clip.name === 'string' || clip.name === null))) return;
      identities.add(value.asset);
      packs.push({
        asset: value.asset,
        contentHash: value.contentHash,
        clipIds: Object.freeze(clips.map((clip) => clip.id)),
        clipSourceNames: Object.freeze(clips.map((clip) => clip.name ?? clip.id)),
      });
    });
  });
  return packs;
}

function animatedMeshDescriptors(
  frame: RustyApplicationFrame,
): readonly RendererAnimatedMeshResourceDescriptor[] {
  if (!Array.isArray(frame['ops'])) return [];
  return frame['ops'].flatMap((operation): RendererAnimatedMeshResourceDescriptor[] => {
    if (typeof operation !== 'object' || operation === null
      || (operation as { readonly op?: unknown }).op !== 'defineAnimatedMesh') return [];
    const asset = (operation as { readonly asset?: unknown }).asset;
    if (typeof asset !== 'object' || asset === null) return [];
    const candidate = asset as {
      readonly asset?: unknown;
      readonly contentHash?: unknown;
      readonly clips?: unknown;
    };
    if (typeof candidate.asset !== 'string'
      || typeof candidate.contentHash !== 'string'
      || !Array.isArray(candidate.clips)) return [];
    const clips = candidate.clips.map((clip) => (
      typeof clip === 'object' && clip !== null
        ? { id: (clip as { readonly id?: unknown }).id, name: (clip as { readonly name?: unknown }).name }
        : undefined
    ));
    if (!clips.every((clip): clip is { readonly id: string; readonly name: string | null } => clip !== undefined
      && typeof clip.id === 'string' && (typeof clip.name === 'string' || clip.name === null))) return [];
    return [{
      asset: candidate.asset,
      contentHash: candidate.contentHash,
      clipIds: Object.freeze(clips.map((clip) => clip.id)),
      clipSourceNames: Object.freeze(clips.map((clip) => clip.name ?? clip.id)),
    }];
  });
}

function resourceDescriptor(resource: PreparedRustyApplicationResource): {
  readonly resource: string;
  readonly contentHash: string;
  readonly byteLength: number;
} {
  return Object.freeze({
    resource: resource.identity,
    contentHash: resource.contentHash,
    byteLength: resource.bytes.byteLength,
  });
}

function resolveResource(
  entries: ReadonlyMap<string, PreparedRustyApplicationResource>,
  identity: string,
): Promise<ArrayBuffer> {
  const entry = entries.get(identity);
  if (entry === undefined) return Promise.reject(new Error(`resource ${identity} is unavailable`));
  return Promise.resolve(entry.bytes.slice(0));
}

function contentError(
  code: RustyApplicationContentDiagnosticCode,
  resource: string | null,
  message: string,
): RustyApplicationContentError {
  return new RustyApplicationContentError(code, resource, message);
}
