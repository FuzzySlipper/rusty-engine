import {
  RUSTY_RENDERER_MESH_RESOURCE_MAX_BYTES,
  RUSTY_RENDERER_MESH_RESOURCE_MAX_COUNT,
  RUSTY_RENDERER_MESH_RESOURCE_MAX_TOTAL_BYTES,
  RUSTY_RENDERER_TEXTURE_RESOURCE_MAX_BYTES,
  RUSTY_RENDERER_TEXTURE_RESOURCE_MAX_COUNT,
  RUSTY_RENDERER_TEXTURE_RESOURCE_MAX_TOTAL_BYTES,
  type RendererMeshResourceDescriptor,
  type RendererMeshResourceManifest,
  type RendererAnimatedMeshResourceDescriptor,
  type RendererAnimatedMeshResourceManifest,
  type RendererAnimatedMeshResourceResolver,
  type RendererTextureResourceDescriptor,
  type RendererTextureResourceManifest,
} from '@rusty-engine/renderer-host';

import type { RustyApplicationFrame } from './application-host.js';

export type RustyApplicationResourceKind = 'mesh' | 'texture';

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

const SHA256_IDENTITY = /^(mesh|texture)-resource\/([0-9a-f]{64})$/u;

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
    const kind = match[1] as RustyApplicationResourceKind;
    if (kind === 'texture') {
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
    resourceBytes: meshBytes + textureBytes,
  });
}

export function rustyApplicationSurfaceResourceOptions(
  content: PreparedRustyApplicationContent,
): RustyApplicationSurfaceResourceOptions {
  const entries = new Map(content.resources.map((resource) => [resource.identity, resource]));
  const animated = animatedMeshDescriptors(content.frame);
  const mesh = content.resources.filter((resource) => resource.kind === 'mesh');
  const textures = content.resources.filter((resource) => resource.kind === 'texture');
  return Object.freeze({
    ...(animated.length === 0 ? {} : {
      animatedMeshManifest: {
        kind: 'rusty_renderer_animated_mesh_resources.v1' as const,
        resources: Object.freeze(animated),
      },
      resolveAnimatedMeshResource: (descriptor: RendererAnimatedMeshResourceDescriptor) =>
        resolveResourceByContentHash(entries, descriptor.contentHash),
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
    const clipIds = candidate.clips.map((clip) => (
      typeof clip === 'object' && clip !== null
        ? (clip as { readonly id?: unknown }).id
        : undefined
    ));
    if (!clipIds.every((clip): clip is string => typeof clip === 'string')) return [];
    return [{
      asset: candidate.asset,
      contentHash: candidate.contentHash,
      clipIds: Object.freeze(clipIds),
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

function resolveResourceByContentHash(
  entries: ReadonlyMap<string, PreparedRustyApplicationResource>,
  contentHash: string,
): Promise<ArrayBuffer> {
  const entry = [...entries.values()].find((candidate) => candidate.contentHash === contentHash);
  if (entry === undefined) {
    return Promise.reject(new Error(`resource ${contentHash} is unavailable`));
  }
  return Promise.resolve(entry.bytes.slice(0));
}

function contentError(
  code: RustyApplicationContentDiagnosticCode,
  resource: string | null,
  message: string,
): RustyApplicationContentError {
  return new RustyApplicationContentError(code, resource, message);
}
