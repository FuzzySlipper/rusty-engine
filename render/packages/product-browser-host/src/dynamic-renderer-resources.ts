import type { RustyApplicationResource } from '@rusty-engine/application-host';

const RESOURCE_ROUTE = '/__rusty/product/runtime/resource';
const RESOURCE_IDENTITY = /^(animated-mesh|audio|mesh|clip-pack|texture)-resource\/([0-9a-f]{64})$/u;
const FONT_IDENTITY = /^font\/([0-9a-f]{64})$/u;

export interface ProductBrowserDynamicRendererResourceFetcher {
  (input: string | URL, init?: RequestInit): Promise<Response>;
}

/**
 * Browser-owned immutable byte cache for renderer resources delivered after
 * product startup. The runtime generation fences every request: cached bytes
 * are never carried across a replacement runtime owner.
 */
export class ProductBrowserDynamicRendererResources {
  readonly #loaded = new Map<string, RustyApplicationResource>();
  readonly #inFlight = new Map<string, Promise<RustyApplicationResource>>();
  readonly #fetcher: ProductBrowserDynamicRendererResourceFetcher;
  readonly #route: string;
  #generation: string | null = null;

  constructor(fetcher: ProductBrowserDynamicRendererResourceFetcher = globalThis.fetch, route = RESOURCE_ROUTE) {
    this.#fetcher = fetcher;
    this.#route = route;
  }

  /**
   * Ensures the exact immutable bodies are present for `generation`. Repeated
   * identities share one fetch while it is in progress and afterwards.
   */
  async ensure(
    identities: readonly string[],
    generation: string,
  ): Promise<readonly RustyApplicationResource[]> {
    if (this.#generation !== generation) {
      this.#generation = generation;
      this.#loaded.clear();
      this.#inFlight.clear();
    }
    const unique = [...new Set(identities)];
    return Promise.all(unique.map((identity) => this.#ensureOne(identity, generation)));
  }

  /**
   * Drops bodies that are no longer referenced by the current projected
   * resource closure. In-flight fetches are retained until settlement so an
   * already-issued request cannot be duplicated or incorrectly aborted.
   */
  retainOnly(identities: ReadonlySet<string>, generation: string): void {
    if (this.#generation !== generation) return;
    for (const identity of this.#loaded.keys()) {
      if (!identities.has(identity)) this.#loaded.delete(identity);
    }
  }

  resources(identities: readonly string[], generation: string): readonly RustyApplicationResource[] {
    if (this.#generation !== generation) {
      throw new Error('renderer resource cache generation is stale');
    }
    return Object.freeze(identities.map((identity) => {
      const resource = this.#loaded.get(identity);
      if (resource === undefined) throw new Error(`renderer resource ${identity} was not prefetched`);
      return resource;
    }));
  }

  clear(): void {
    this.#generation = null;
    this.#loaded.clear();
    this.#inFlight.clear();
  }

  #ensureOne(identity: string, generation: string): Promise<RustyApplicationResource> {
    const existing = this.#loaded.get(identity);
    if (existing !== undefined) return Promise.resolve(existing);
    const pending = this.#inFlight.get(identity);
    if (pending !== undefined) return pending;
    let request!: Promise<RustyApplicationResource>;
    request = loadResource(this.#fetcher, this.#route, identity, generation)
      .then((resource) => {
        if (this.#generation !== generation) {
          throw new Error('renderer resource response belongs to a stale runtime generation');
        }
        this.#loaded.set(identity, resource);
        return resource;
      })
      .finally(() => {
        if (this.#inFlight.get(identity) === request) this.#inFlight.delete(identity);
      });
    this.#inFlight.set(identity, request);
    return request;
  }
}

async function loadResource(
  fetcher: ProductBrowserDynamicRendererResourceFetcher,
  route: string,
  identity: string,
  generation: string,
): Promise<RustyApplicationResource> {
  const descriptor = resourceDescriptor(identity);
  const query = new URLSearchParams({ identity, generation });
  const response = await fetcher(`${route}?${query.toString()}`, { cache: 'no-store' });
  if (!response.ok) throw new Error(`renderer resource ${identity} is unavailable`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  const expectedHash = `sha256:${descriptor.hash}`;
  return Object.freeze({
    identity,
    contentHash: expectedHash,
    mediaType: descriptor.mediaType,
    bytes,
  });
}

function resourceDescriptor(identity: string): { readonly hash: string; readonly mediaType: string } {
  const regular = RESOURCE_IDENTITY.exec(identity);
  if (regular !== null) {
    const mediaType = regular[1] === 'texture' ? 'image/png'
      : regular[1] === 'audio' ? 'audio/wav'
        : regular[1] === 'mesh' ? 'application/octet-stream'
          : 'model/gltf-binary';
    return { hash: regular[2]!, mediaType };
  }
  const font = FONT_IDENTITY.exec(identity);
  if (font !== null) return { hash: font[1]!, mediaType: 'font/woff2' };
  throw new Error(`renderer resource identity ${identity} is invalid`);
}
