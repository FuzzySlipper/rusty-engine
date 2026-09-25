import {
  type RendererMeshResourceDescriptor,
  type RendererMeshResourceManifest,
  type RendererAudioResourceResolver,
  type RendererVideoResourceResolver,
  type RendererAnimatedMeshResourceDescriptor,
  type RendererAnimationClipPackResourceDescriptor,
  type RendererAnimatedMeshResourceManifest,
  type RendererAnimatedMeshResourceResolver,
  type RendererTextureResourceDescriptor,
  type RendererTextureResourceManifest,
  RendererMutableAnimatedMeshResourceSource,
  RendererMutableMeshResourceSource,
  RendererMutableTextureResourceSource,
} from '@rusty-engine/renderer-host';

import type { RustyApplicationFrame } from './application-host.js';
import type { RenderPublicationFrontier } from '@rusty-engine/render-contracts';

export type RustyApplicationResourceKind = 'animatedMesh' | 'audio' | 'video' | 'mesh' | 'clipPack' | 'texture' | 'font';

export interface RustyApplicationResource {
  readonly identity: string;
  readonly contentHash: string;
  readonly mediaType: string;
  /** Engine-owned immutable bytes; callers must not mutate after publication. */
  readonly bytes: Uint8Array;
}

export interface RustyApplicationContent {
  readonly frame: RustyApplicationFrame;
  readonly resources?: readonly RustyApplicationResource[];
  readonly publicationFrontiers?: readonly RenderPublicationFrontier[];
}

export type RustyApplicationContentDiagnosticCode = 'content_invalid';

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
  readonly publicationFrontiers: readonly RenderPublicationFrontier[];
}

/** Renderer resolvers borrow prepared bytes. Consumers must not mutate or detach
 * them; renderer resource loaders snapshot them when admitting resources. */
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
  const publicationFrontiers = structuredClone(content.publicationFrontiers ?? []);
  const resources = (content.resources ?? []).map((resource) => {
    const family = resource.identity.split('/')[0];
    const kind: RustyApplicationResourceKind = family === 'font' ? 'font'
      : family === 'clip-pack-resource' ? 'clipPack'
      : family === 'animated-mesh-resource' ? 'animatedMesh'
      : family === 'audio-resource' ? 'audio'
      : family === 'video-resource' ? 'video'
      : family === 'texture-resource' ? 'texture' : 'mesh';
    return Object.freeze({
      identity: resource.identity,
      contentHash: resource.contentHash,
      mediaType: resource.mediaType,
      // Borrow Engine-owned immutable storage. A subview needs an exact
      // buffer because the consuming APIs accept ArrayBuffer, not a range.
      bytes: resource.bytes.buffer instanceof ArrayBuffer
        && resource.bytes.byteOffset === 0
        && resource.bytes.byteLength === resource.bytes.buffer.byteLength
        ? resource.bytes.buffer : resource.bytes.slice().buffer,
      kind,
    });
  });
  return Object.freeze({
    frame,
    resources: Object.freeze(resources),
    resourceBytes: resources.reduce((total, resource) => total + resource.bytes.byteLength, 0),
    publicationFrontiers,
  });
}

/** One mutable Engine-owned resource catalog shared by a mounted surface and
 * its presentation hosts. Product Browser admits immutable bytes here before
 * applying the output group that names them. */
export class RustyApplicationResourceCatalog {
  readonly #resources = new Map<string, PreparedRustyApplicationResource>();
  readonly #byHash = new Map<string, PreparedRustyApplicationResource>();
  readonly meshSource = new RendererMutableMeshResourceSource();
  readonly textureSource = new RendererMutableTextureResourceSource();
  readonly animatedSource = new RendererMutableAnimatedMeshResourceSource();

  async admit(
    resources: readonly (RustyApplicationResource | PreparedRustyApplicationResource)[],
    frame?: RustyApplicationFrame,
  ): Promise<void> {
    const prepared = resources.length === 0 || resources[0]!.bytes instanceof Uint8Array
      ? prepareRustyApplicationContent({
          frame: frame ?? { schemaVersion: 1, ops: [] },
          resources: resources as readonly RustyApplicationResource[],
        }).resources
      : resources as readonly PreparedRustyApplicationResource[];
    for (const resource of prepared) {
      const existing = this.#resources.get(resource.identity);
      if (existing !== undefined) continue;
      if (resource.kind === 'mesh') await this.meshSource.admit(resource.identity, resource.contentHash, resource.bytes);
      if (resource.kind === 'texture') await this.textureSource.admit(resource.identity, resource.contentHash, resource.bytes);
      this.#resources.set(resource.identity, resource);
      this.#byHash.set(resource.contentHash, resource);
    }
    if (frame === undefined) return;
    for (const descriptor of animatedMeshDescriptors(frame)) {
      const resource = this.#byHash.get(descriptor.contentHash);
      if (resource !== undefined) await this.animatedSource.admitAnimatedMesh(descriptor, resource.bytes);
    }
    for (const descriptor of animationClipPacks(frame)) {
      const resource = this.#byHash.get(descriptor.contentHash);
      if (resource !== undefined) await this.animatedSource.admitClipPack(descriptor, resource.bytes);
    }
  }

  resource(identity: string, hash?: string): PreparedRustyApplicationResource | undefined {
    return this.#resources.get(identity) ?? (hash === undefined ? undefined : this.#byHash.get(hash));
  }

  snapshot(): readonly PreparedRustyApplicationResource[] {
    return Object.freeze([...this.#resources.values()]);
  }

  retainOnly(identities: ReadonlySet<string>): void {
    for (const [identity, resource] of this.#resources) {
      if (identities.has(identity)) continue;
      this.#resources.delete(identity);
    }
    this.#byHash.clear();
    for (const resource of this.#resources.values()) this.#byHash.set(resource.contentHash, resource);
    this.meshSource.retainOnly(identities);
    this.textureSource.retainOnly(identities);
    this.animatedSource.retainOnly(identities);
  }

  readout(): { readonly resources: number; readonly animated: number; readonly clipPacks: number } {
    const counts = this.animatedSource.resourceCounts();
    return { resources: this.#resources.size, animated: counts.animatedMeshes, clipPacks: counts.clipPacks };
  }

  clear(): void { this.retainOnly(new Set()); }

  audioResolver(): RendererAudioResourceResolver {
    return (clip) => {
      const resource = this.#byHash.get(clip.contentHash);
      return resource?.kind === 'audio'
        ? Promise.resolve({ bytes: resource.bytes, contentHash: resource.contentHash, mediaType: resource.mediaType })
        : Promise.reject(new Error(`audio resource ${clip.asset} (${clip.contentHash}) is unavailable`));
    };
  }

  videoResolver(): RendererVideoResourceResolver {
    return (clip) => {
      const resource = this.#byHash.get(clip.contentHash);
      return resource?.kind === 'video' && resource.mediaType === 'video/webm'
        ? Promise.resolve({ bytes: resource.bytes, contentHash: resource.contentHash, mediaType: 'video/webm' })
        : Promise.reject(new Error(`video resource ${clip.asset} (${clip.contentHash}) is unavailable`));
    };
  }
}

export function rustyApplicationAudioResourceResolver(
  content: PreparedRustyApplicationContent,
): RendererAudioResourceResolver {
  const audio = content.resources.filter((resource) => resource.kind === 'audio');
  const entries = new Map(audio.map((resource) => [resource.contentHash, resource]));
  return (clip) => {
    const entry = entries.get(clip.contentHash);
    if (entry === undefined) {
      return Promise.reject(new Error(
        `audio resource ${clip.asset} (${clip.contentHash}) is unavailable`,
      ));
    }
    return Promise.resolve({
      bytes: entry.bytes,
      contentHash: entry.contentHash,
    });
  };
}

export function rustyApplicationSurfaceResourceOptions(
  content: PreparedRustyApplicationContent,
): RustyApplicationSurfaceResourceOptions {
  const entries = new Map(content.resources.map((resource) => [resource.identity, resource]));
  const animationEntriesByHash = new Map(content.resources
    .filter((resource) => resource.kind === 'animatedMesh' || resource.kind === 'clipPack')
    .map((resource) => [resource.contentHash, resource]));
  const animated = animatedMeshDescriptors(content.frame);
  const clipPacks = animationClipPacks(content.frame);
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
        resolveResourceByHash(animationEntriesByHash, descriptor.contentHash),
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
      readonly embeddedMaterialSlots?: unknown;
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
    const embeddedMaterialSlots = animatedEmbeddedMaterialSlots(candidate.embeddedMaterialSlots);
    if (embeddedMaterialSlots === undefined) return [];
    return [{
      asset: candidate.asset,
      contentHash: candidate.contentHash,
      clipIds: Object.freeze(clips.map((clip) => clip.id)),
      clipSourceNames: Object.freeze(clips.map((clip) => clip.name ?? clip.id)),
      embeddedMaterialSlots,
    }];
  });
}

function animatedEmbeddedMaterialSlots(
  candidate: unknown,
): readonly { readonly slot: number; readonly sourceMaterialSlot: number }[] | undefined {
  if (candidate === undefined) return Object.freeze([]);
  if (!Array.isArray(candidate)) return undefined;
  const sources = new Set<number>();
  const slots = candidate.map((value, index) => {
    if (typeof value !== 'object' || value === null) return undefined;
    const binding = value as { readonly slot?: unknown; readonly sourceMaterialSlot?: unknown };
    const slot = binding.slot;
    const sourceMaterialSlot = binding.sourceMaterialSlot;
    if (typeof slot !== 'number' || !Number.isSafeInteger(slot) || slot !== index
      || typeof sourceMaterialSlot !== 'number' || !Number.isSafeInteger(sourceMaterialSlot) || sourceMaterialSlot < 0
      || sourceMaterialSlot > 65_535 || sources.has(sourceMaterialSlot)) {
      return undefined;
    }
    sources.add(sourceMaterialSlot);
    return Object.freeze({ slot, sourceMaterialSlot });
  });
  return slots.every((slot): slot is { readonly slot: number; readonly sourceMaterialSlot: number } => slot !== undefined)
    ? Object.freeze(slots)
    : undefined;
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
  return Promise.resolve(entry.bytes);
}

function resolveResourceByHash(
  entries: ReadonlyMap<string, PreparedRustyApplicationResource>,
  contentHash: string,
): Promise<ArrayBuffer> {
  const resource = entries.get(contentHash);
  return resource === undefined
    ? Promise.reject(new Error(`application resource ${contentHash} is unavailable`))
    : Promise.resolve(resource.bytes);
}

function contentError(
  code: RustyApplicationContentDiagnosticCode,
  resource: string | null,
  message: string,
): RustyApplicationContentError {
  return new RustyApplicationContentError(code, resource, message);
}
