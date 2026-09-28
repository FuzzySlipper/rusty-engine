import type { RustyApplicationContent } from '@rusty-engine/application-host';

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
  // The descriptor changes with every restage; the bodies it names do not.
  const descriptorResponse = await fetcher(descriptorUrl, { cache: 'no-store' });
  if (!descriptorResponse.ok) {
    throw new Error('Product renderer preload descriptor is unavailable');
  }
  // This manifest and its bodies are supplied by the trusted Engine host.
  // Format decoding belongs to the consuming renderer, not the transport.
  const descriptor = await descriptorResponse.json() as RendererPreloadDescriptor;
  const resources = await Promise.all(descriptor.resources.map((resource) =>
    loadRendererResource(resource, descriptorUrl, fetcher)));
  return Object.freeze({
    frame: Object.freeze({ schemaVersion: 1, ops: Object.freeze([]) }),
    resources: Object.freeze(resources),
  });
}

async function loadRendererResource(
  resource: RendererPreloadResourceDescriptor,
  descriptorUrl: URL,
  fetcher: typeof globalThis.fetch,
) {
  // Naming the body by its content hash makes the URL immutable, so the
  // browser may reuse it; changed bytes arrive under a new hash and URL.
  const url = new URL(`./${resource.path}`, descriptorUrl);
  url.searchParams.set('content', resource.contentHash);
  const response = await fetcher(url);
  if (!response.ok) {
    throw new Error(`Product renderer resource ${resource.identity} is unavailable`);
  }
  const data = await response.arrayBuffer();
  const bytes = new Uint8Array(data);
  return Object.freeze({
    identity: resource.identity,
    contentHash: resource.contentHash,
    mediaType: resource.mediaType,
    bytes,
  });
}
