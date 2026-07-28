import type { MeshResourceSource } from '@rusty-engine/renderer-three/backend';

import { rendererResourceContentHash } from './resource-content-hash.js';

export const RUSTY_RENDERER_MESH_RESOURCE_MAX_BYTES = 64 * 1024 * 1024;
export const RUSTY_RENDERER_MESH_RESOURCE_MAX_TOTAL_BYTES = 256 * 1024 * 1024;
export const RUSTY_RENDERER_MESH_RESOURCE_MAX_COUNT = 1024;

export interface RendererMeshResourceDescriptor {
  readonly resource: string;
  readonly contentHash: string;
  readonly byteLength: number;
}

export interface RendererMeshResourceManifest {
  readonly kind: 'rusty_renderer_mesh_resources.v1';
  readonly resources: readonly RendererMeshResourceDescriptor[];
}

export type RendererMeshResourceResolver = (
  descriptor: RendererMeshResourceDescriptor,
) => Promise<ArrayBuffer>;

export type RendererMeshResourceErrorCode =
  | 'mesh_resource_manifest_invalid'
  | 'mesh_resource_unavailable'
  | 'mesh_resource_byte_length_mismatch'
  | 'mesh_resource_content_hash_mismatch';

export class RendererMeshResourceError extends Error {
  constructor(
    readonly code: RendererMeshResourceErrorCode,
    readonly resource: string | null,
    message: string,
  ) {
    super(message);
    this.name = 'RendererMeshResourceError';
  }
}

export async function loadRendererMeshResourceSource(
  manifest: RendererMeshResourceManifest,
  resolver: RendererMeshResourceResolver,
): Promise<MeshResourceSource> {
  validateManifest(manifest);
  const loaded = await Promise.all(manifest.resources.map(async (descriptor) => {
    let data: ArrayBuffer;
    try {
      data = await resolver(descriptor);
    } catch (cause) {
      throw resourceError('mesh_resource_unavailable', descriptor.resource, cause);
    }
    if (data.byteLength !== descriptor.byteLength) {
      throw resourceError(
        'mesh_resource_byte_length_mismatch',
        descriptor.resource,
        `expected ${String(descriptor.byteLength)} bytes, received ${String(data.byteLength)}`,
      );
    }
    const actualHash = await rendererResourceContentHash(data, descriptor.contentHash);
    if (actualHash !== descriptor.contentHash) {
      throw resourceError(
        'mesh_resource_content_hash_mismatch',
        descriptor.resource,
        `expected ${descriptor.contentHash}, received ${actualHash}`,
      );
    }
    return [descriptor.resource, {
      descriptor,
      bytes: new Uint8Array(data),
    }] as const;
  }));
  const resources = new Map(loaded);
  return {
    acquireResource: (resource, contentHash, byteLength) => {
      const entry = resources.get(resource);
      if (entry === undefined) {
        throw resourceError('mesh_resource_unavailable', resource, 'resource was not preloaded');
      }
      if (entry.descriptor.contentHash !== contentHash
        || entry.descriptor.byteLength !== byteLength) {
        throw resourceError(
          'mesh_resource_manifest_invalid',
          resource,
          'retained descriptor does not match the admitted resource manifest',
        );
      }
      return { bytes: entry.bytes };
    },
    releaseResource: () => {
      // Loaded host bytes remain cached for other meshes/frames in the same
      // retained resource. Three owns only its copied typed arrays.
    },
  };
}

function validateManifest(manifest: RendererMeshResourceManifest): void {
  if (manifest.kind !== 'rusty_renderer_mesh_resources.v1'
    || manifest.resources.length === 0
    || manifest.resources.length > RUSTY_RENDERER_MESH_RESOURCE_MAX_COUNT) {
    throw resourceError(
      'mesh_resource_manifest_invalid',
      null,
      'mesh resource manifest is empty, oversized, or unsupported',
    );
  }
  const identities = new Set<string>();
  let totalBytes = 0;
  for (const descriptor of manifest.resources) {
    const digest = /^sha256:([0-9a-f]{64})$/u.exec(descriptor.contentHash)?.[1];
    if (digest === undefined
      || descriptor.resource !== `mesh-resource/${digest}`
      || !Number.isSafeInteger(descriptor.byteLength)
      || descriptor.byteLength < 16
      || descriptor.byteLength > RUSTY_RENDERER_MESH_RESOURCE_MAX_BYTES
      || identities.has(descriptor.resource)) {
      throw resourceError(
        'mesh_resource_manifest_invalid',
        descriptor.resource || null,
        'mesh resource descriptor is invalid or duplicated',
      );
    }
    identities.add(descriptor.resource);
    totalBytes += descriptor.byteLength;
    if (totalBytes > RUSTY_RENDERER_MESH_RESOURCE_MAX_TOTAL_BYTES) {
      throw resourceError(
        'mesh_resource_manifest_invalid',
        descriptor.resource,
        'mesh resource manifest exceeds the aggregate byte bound',
      );
    }
  }
}

function resourceError(
  code: RendererMeshResourceErrorCode,
  resource: string | null,
  cause: unknown,
): RendererMeshResourceError {
  return new RendererMeshResourceError(
    code,
    resource,
    cause instanceof Error ? cause.message : String(cause),
  );
}
