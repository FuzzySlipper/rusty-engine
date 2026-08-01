import type { TextureResourceSource } from '@rusty-engine/renderer-three/backend';

import { rendererResourceContentHash } from './resource-content-hash.js';

export const RUSTY_RENDERER_TEXTURE_RESOURCE_MAX_BYTES = 16 * 1024 * 1024;
export const RUSTY_RENDERER_TEXTURE_RESOURCE_MAX_TOTAL_BYTES = 128 * 1024 * 1024;
export const RUSTY_RENDERER_TEXTURE_RESOURCE_MAX_COUNT = 256;

export interface RendererTextureResourceDescriptor {
  readonly resource: string;
  readonly contentHash: string;
  readonly byteLength: number;
}

export interface RendererTextureResourceManifest {
  readonly kind: 'rusty_renderer_texture_resources.v1';
  readonly resources: readonly RendererTextureResourceDescriptor[];
}

export type RendererTextureResourceResolver = (
  descriptor: RendererTextureResourceDescriptor,
) => Promise<ArrayBuffer>;

export type RendererTextureResourceErrorCode =
  | 'texture_resource_manifest_invalid'
  | 'texture_resource_unavailable'
  | 'texture_resource_byte_length_mismatch'
  | 'texture_resource_content_hash_mismatch';

export class RendererTextureResourceError extends Error {
  constructor(
    readonly code: RendererTextureResourceErrorCode,
    readonly resource: string | null,
    message: string,
  ) {
    super(message);
    this.name = 'RendererTextureResourceError';
  }
}

export async function loadRendererTextureResourceSource(
  manifest: RendererTextureResourceManifest,
  resolver: RendererTextureResourceResolver,
): Promise<TextureResourceSource> {
  validateManifest(manifest);
  const loaded = await Promise.all(manifest.resources.map(async (descriptor) => {
    let data: ArrayBuffer;
    try {
      data = await resolver(descriptor);
    } catch (cause) {
      throw resourceError('texture_resource_unavailable', descriptor.resource, cause);
    }
    const admitted = data.slice(0);
    if (admitted.byteLength !== descriptor.byteLength) {
      throw resourceError(
        'texture_resource_byte_length_mismatch',
        descriptor.resource,
        `expected ${String(descriptor.byteLength)} bytes, received ${String(admitted.byteLength)}`,
      );
    }
    const actualHash = await rendererResourceContentHash(admitted, descriptor.contentHash);
    if (actualHash !== descriptor.contentHash) {
      throw resourceError(
        'texture_resource_content_hash_mismatch',
        descriptor.resource,
        `expected ${descriptor.contentHash}, received ${actualHash}`,
      );
    }
    return [descriptor.resource, {
      descriptor,
      bytes: new Uint8Array(admitted),
    }] as const;
  }));
  const resources = new Map(loaded);
  return {
    acquireResource: (resource, contentHash, byteLength) => {
      const entry = resources.get(resource);
      if (entry === undefined) {
        throw resourceError('texture_resource_unavailable', resource, 'resource was not preloaded');
      }
      if (entry.descriptor.contentHash !== contentHash
        || entry.descriptor.byteLength !== byteLength) {
        throw resourceError(
          'texture_resource_manifest_invalid',
          resource,
          'retained descriptor does not match the admitted resource manifest',
        );
      }
      return { bytes: entry.bytes };
    },
    releaseResource: () => {
      // Host-owned encoded bytes remain available for other retained users.
    },
  };
}

function validateManifest(manifest: RendererTextureResourceManifest): void {
  if (manifest.kind !== 'rusty_renderer_texture_resources.v1'
    || manifest.resources.length === 0
    || manifest.resources.length > RUSTY_RENDERER_TEXTURE_RESOURCE_MAX_COUNT) {
    throw resourceError(
      'texture_resource_manifest_invalid',
      null,
      'texture resource manifest is empty, oversized, or unsupported',
    );
  }
  const identities = new Set<string>();
  let totalBytes = 0;
  for (const descriptor of manifest.resources) {
    const digest = /^sha256:([0-9a-f]{64})$/u.exec(descriptor.contentHash)?.[1];
    if (digest === undefined
      || descriptor.resource !== `texture-resource/${digest}`
      || !Number.isSafeInteger(descriptor.byteLength)
      || descriptor.byteLength <= 0
      || descriptor.byteLength > RUSTY_RENDERER_TEXTURE_RESOURCE_MAX_BYTES
      || identities.has(descriptor.resource)) {
      throw resourceError(
        'texture_resource_manifest_invalid',
        descriptor.resource || null,
        'texture resource descriptor is invalid or duplicated',
      );
    }
    identities.add(descriptor.resource);
    totalBytes += descriptor.byteLength;
    if (totalBytes > RUSTY_RENDERER_TEXTURE_RESOURCE_MAX_TOTAL_BYTES) {
      throw resourceError(
        'texture_resource_manifest_invalid',
        descriptor.resource,
        'texture resource manifest exceeds the aggregate byte bound',
      );
    }
  }
}

function resourceError(
  code: RendererTextureResourceErrorCode,
  resource: string | null,
  cause: unknown,
): RendererTextureResourceError {
  return new RendererTextureResourceError(
    code,
    resource,
    cause instanceof Error ? cause.message : String(cause),
  );
}
