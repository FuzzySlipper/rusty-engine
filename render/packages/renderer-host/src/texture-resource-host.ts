import type { TextureResourceSource } from '@rusty-engine/renderer-three/backend';

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

/** Mutable Engine source used when immutable ProductContent arrives after mount. */
export class RendererMutableTextureResourceSource implements TextureResourceSource {
  readonly #resources = new Map<string, { readonly contentHash: string; readonly bytes: Uint8Array }>();

  async admit(resource: string, contentHash: string, data: ArrayBuffer): Promise<void> {
    validateManifest({ kind: 'rusty_renderer_texture_resources.v1', resources: [{
      resource, contentHash, byteLength: data.byteLength,
    }] });
    const bytes = new Uint8Array(data);
    const existing = this.#resources.get(resource);
    if (existing !== undefined && existing.contentHash !== contentHash) {
      throw resourceError('texture_resource_manifest_invalid', resource, 'resource identity was admitted with a different hash');
    }
    this.#resources.set(resource, { contentHash, bytes });
  }

  acquireResource(resource: string, contentHash: string, byteLength: number): { readonly bytes: Uint8Array } {
    const entry = this.#resources.get(resource);
    if (entry === undefined) throw resourceError('texture_resource_unavailable', resource, 'resource was not admitted');
    if (entry.contentHash !== contentHash || entry.bytes.byteLength !== byteLength) {
      throw resourceError('texture_resource_manifest_invalid', resource, 'retained descriptor does not match admitted resource');
    }
    return { bytes: entry.bytes };
  }

  releaseResource(): void {}

  retainOnly(identities: ReadonlySet<string>): void {
    for (const identity of this.#resources.keys()) if (!identities.has(identity)) this.#resources.delete(identity);
  }
}

export type RendererTextureResourceErrorCode =
  | 'texture_resource_manifest_invalid'
  | 'texture_resource_unavailable'
  | 'texture_resource_byte_length_mismatch';

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
    if (data.byteLength !== descriptor.byteLength) {
      throw resourceError(
        'texture_resource_byte_length_mismatch',
        descriptor.resource,
        `expected ${String(descriptor.byteLength)} bytes, received ${String(data.byteLength)}`,
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
  if (manifest.kind !== 'rusty_renderer_texture_resources.v1' || manifest.resources.length === 0) {
    throw resourceError(
      'texture_resource_manifest_invalid',
      null,
      'texture resource manifest is empty or unsupported',
    );
  }
  const identities = new Set<string>();
  for (const descriptor of manifest.resources) {
    const digest = /^sha256:([0-9a-f]{64})$/u.exec(descriptor.contentHash)?.[1];
    if (digest === undefined
      || descriptor.resource !== `texture-resource/${digest}`
      || !Number.isSafeInteger(descriptor.byteLength)
      || descriptor.byteLength <= 0
      || identities.has(descriptor.resource)) {
      throw resourceError(
        'texture_resource_manifest_invalid',
        descriptor.resource || null,
        'texture resource descriptor is invalid or duplicated',
      );
    }
    identities.add(descriptor.resource);
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
