import {
  rendererResourceContentHash,
  type RustyApplicationContent,
} from '@rusty-engine/application-host';

const TEXTURE_MAX_COUNT = 256;
const TEXTURE_MAX_TOTAL_BYTES = 128 * 1024 * 1024;
const TEXTURE_MAX_BYTES = 16 * 1024 * 1024;
const AUDIO_MAX_COUNT = 64;
const AUDIO_MAX_TOTAL_BYTES = 32 * 1024 * 1024;
const AUDIO_MAX_BYTES = 8 * 1024 * 1024;
const MESH_MAX_COUNT = 1024;
const MESH_MAX_TOTAL_BYTES = 64 * 1024 * 1024;
const MESH_MAX_BYTES = 16 * 1024 * 1024;

interface RendererPreloadDescriptor {
  readonly artifact: 'rusty.product.renderer-preload.v1';
  readonly resources: readonly RendererPreloadResourceDescriptor[];
}

interface RendererPreloadResourceDescriptor {
  readonly identity: string;
  readonly contentHash: string;
  readonly mediaType: string;
  readonly path: string;
  readonly byteLength: number;
}

/**
 * Loads the immutable renderer resources selected during Product Create.
 *
 * `moduleUrl` belongs to the browser composition root rather than this Engine
 * module so both generated bundles and the packaged runtime shell resolve the
 * Product-owned descriptor and resources from the same directory.
 */
export async function loadProductBrowserRendererInitialContent(
  moduleUrl: string | URL,
  fetcher: typeof globalThis.fetch = globalThis.fetch,
): Promise<RustyApplicationContent> {
  const descriptorUrl = new URL('./renderer-preload.json', moduleUrl);
  const descriptorResponse = await fetcher(descriptorUrl, { cache: 'no-store' });
  if (!descriptorResponse.ok) {
    throw new Error('Product renderer preload descriptor is unavailable');
  }
  const descriptor = decodeRendererPreload(await descriptorResponse.json());
  const resources = await Promise.all(descriptor.resources.map((resource) =>
    loadRendererResource(resource, descriptorUrl, fetcher)));
  return Object.freeze({
    frame: Object.freeze({ schemaVersion: 1, ops: Object.freeze([]) }),
    resources: Object.freeze(resources),
  });
}

function decodeRendererPreload(value: unknown): RendererPreloadDescriptor {
  if (value === null || typeof value !== 'object'
    || (value as { readonly artifact?: unknown }).artifact !== 'rusty.product.renderer-preload.v1'
    || !Array.isArray((value as { readonly resources?: unknown }).resources)) {
    throw new Error('Product renderer preload descriptor is invalid');
  }
  let textureCount = 0;
  let textureBytes = 0;
  let audioCount = 0;
  let audioBytes = 0;
  let meshCount = 0;
  let meshBytes = 0;
  const identities = new Set<string>();
  const paths = new Set<string>();
  const resources = (value as { readonly resources: readonly unknown[] }).resources.map(
    (candidate, index): RendererPreloadResourceDescriptor => {
      if (candidate === null || typeof candidate !== 'object') {
        throw new Error(`Product renderer preload resource ${String(index)} is invalid`);
      }
      const resource = candidate as Partial<RendererPreloadResourceDescriptor>;
      if (typeof resource.identity !== 'string'
        || typeof resource.contentHash !== 'string'
        || typeof resource.mediaType !== 'string'
        || typeof resource.path !== 'string'
        || !Number.isSafeInteger(resource.byteLength)) {
        throw new Error(`Product renderer preload resource ${String(index)} is invalid`);
      }
      const match = /^(animated-mesh|clip-pack|texture|audio|mesh)-resource\/([0-9a-f]{64})$/u
        .exec(resource.identity);
      if (match === null
        || resource.contentHash !== `sha256:${match[2]}`
        || !isSafeRendererPath(resource.path)
        || resource.byteLength! < 0
        || identities.has(resource.identity)
        || paths.has(resource.path)) {
        throw new Error(`Product renderer preload resource ${String(index)} is inadmissible`);
      }
      identities.add(resource.identity);
      paths.add(resource.path);
      const kind = match[1]!;
      if ((kind === 'texture' && (resource.mediaType !== 'image/png' || !resource.path.endsWith('.png')))
        || (kind === 'audio' && (resource.mediaType !== 'audio/wav' || !resource.path.endsWith('.wav')))
        || (kind === 'mesh' && (resource.mediaType !== 'application/octet-stream' || !resource.path.endsWith('.rmesh')))
        || ((kind === 'animated-mesh' || kind === 'clip-pack')
          && (resource.mediaType !== 'model/gltf-binary' || !resource.path.endsWith('.glb')))) {
        throw new Error(`Product renderer preload resource ${String(index)} media is invalid`);
      }
      if (kind === 'texture') {
        textureCount += 1;
        textureBytes += resource.byteLength!;
        if (textureCount > TEXTURE_MAX_COUNT || resource.byteLength === 0
          || resource.byteLength! > TEXTURE_MAX_BYTES || textureBytes > TEXTURE_MAX_TOTAL_BYTES) {
          throw new Error(`Product renderer preload texture ${String(index)} exceeds application-host bounds`);
        }
      } else if (kind === 'audio') {
        audioCount += 1;
        audioBytes += resource.byteLength!;
        if (audioCount > AUDIO_MAX_COUNT || resource.byteLength! < 44
          || resource.byteLength! > AUDIO_MAX_BYTES || audioBytes > AUDIO_MAX_TOTAL_BYTES) {
          throw new Error(`Product renderer preload audio ${String(index)} exceeds application-host bounds`);
        }
      } else {
        meshCount += 1;
        meshBytes += resource.byteLength!;
        const minimumBytes = kind === 'animated-mesh' || kind === 'clip-pack' ? 20 : 16;
        if (meshCount > MESH_MAX_COUNT || resource.byteLength! < minimumBytes
          || resource.byteLength! > MESH_MAX_BYTES || meshBytes > MESH_MAX_TOTAL_BYTES) {
          throw new Error(`Product renderer preload mesh ${String(index)} exceeds application-host bounds`);
        }
      }
      return Object.freeze({
        identity: resource.identity,
        contentHash: resource.contentHash,
        mediaType: resource.mediaType,
        path: resource.path,
        byteLength: resource.byteLength!,
      });
    },
  );
  return Object.freeze({
    artifact: 'rusty.product.renderer-preload.v1',
    resources: Object.freeze(resources),
  });
}

async function loadRendererResource(
  resource: RendererPreloadResourceDescriptor,
  descriptorUrl: URL,
  fetcher: typeof globalThis.fetch,
) {
  const url = new URL(`./${resource.path}`, descriptorUrl);
  if (url.origin !== descriptorUrl.origin) {
    throw new Error('Product renderer resource must remain same-origin');
  }
  const response = await fetcher(url, { cache: 'no-store' });
  if (!response.ok) {
    throw new Error(`Product renderer resource ${resource.identity} is unavailable`);
  }
  const data = await response.arrayBuffer();
  const bytes = new Uint8Array(data);
  if (bytes.byteLength !== resource.byteLength) {
    throw new Error(`Product renderer resource ${resource.identity} length mismatch`);
  }
  if (bytes.byteLength === 0
    || (resource.mediaType === 'image/png' && !hasPngSignature(bytes))
    || (resource.mediaType === 'audio/wav' && !hasWavSignature(bytes))
    || (resource.mediaType === 'application/octet-stream' && !hasMeshResourceHeader(bytes))
    || (resource.mediaType === 'model/gltf-binary' && !hasGlbHeader(bytes))) {
    throw new Error(`Product renderer resource ${resource.identity} media mismatch`);
  }
  const digest = await rendererResourceContentHash(data, resource.contentHash);
  if (resource.contentHash !== digest) {
    throw new Error(`Product renderer resource ${resource.identity} hash mismatch`);
  }
  return Object.freeze({
    identity: resource.identity,
    contentHash: resource.contentHash,
    mediaType: resource.mediaType,
    bytes,
  });
}

function isSafeRendererPath(path: string): boolean {
  return path.startsWith('content/')
    && new TextEncoder().encode(path).byteLength <= 512
    && !path.startsWith('/')
    && !path.startsWith('//')
    && !path.includes('\\')
    && !path.includes('%')
    && !path.includes(':')
    && !/[\u0000-\u001f\u007f]/u.test(path)
    && !/\s/u.test(path)
    && path.split('/').every((part) => part.length > 0 && part !== '.' && part !== '..');
}

function hasPngSignature(bytes: Uint8Array): boolean {
  return bytes.byteLength >= 8
    && bytes[0] === 137 && bytes[1] === 80 && bytes[2] === 78 && bytes[3] === 71
    && bytes[4] === 13 && bytes[5] === 10 && bytes[6] === 26 && bytes[7] === 10;
}

function hasWavSignature(bytes: Uint8Array): boolean {
  return bytes.byteLength >= 44
    && bytes[0] === 82 && bytes[1] === 73 && bytes[2] === 70 && bytes[3] === 70
    && bytes[8] === 87 && bytes[9] === 65 && bytes[10] === 86 && bytes[11] === 69;
}

function hasMeshResourceHeader(bytes: Uint8Array): boolean {
  if (bytes.byteLength < 16) return false;
  const magic = [82, 77, 83, 72, 76, 69, 48];
  const version = bytes[7];
  if ((version !== 49 && version !== 50 && version !== 51)
    || magic.some((byte, index) => bytes[index] !== byte)) return false;
  const header = new DataView(bytes.buffer, bytes.byteOffset, 16);
  return header.getUint32(8, true) === bytes.byteLength && header.getUint32(12, true) !== 0;
}

function hasGlbHeader(bytes: Uint8Array): boolean {
  return bytes.byteLength >= 20
    && bytes[0] === 103 && bytes[1] === 108 && bytes[2] === 84 && bytes[3] === 70;
}
