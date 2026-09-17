import type { RustyApplicationResource } from '@rusty-engine/application-host';
export interface ProductBrowserDynamicRendererResourceFetcher {
    (input: string | URL, init?: RequestInit): Promise<Response>;
}
/**
 * Browser-owned immutable byte cache for renderer resources delivered after
 * product startup. The runtime generation fences every request: cached bytes
 * are never carried across a replacement runtime owner.
 */
export declare class ProductBrowserDynamicRendererResources {
    #private;
    constructor(fetcher?: ProductBrowserDynamicRendererResourceFetcher, route?: string);
    /**
     * Ensures the exact immutable bodies are present for `generation`. Repeated
     * identities share one fetch while it is in progress and afterwards.
     */
    ensure(identities: readonly string[], generation: string): Promise<readonly RustyApplicationResource[]>;
    /**
     * Drops bodies that are no longer referenced by the current projected
     * resource closure. In-flight fetches are retained until settlement so an
     * already-issued request cannot be duplicated or incorrectly aborted.
     */
    retainOnly(identities: ReadonlySet<string>, generation: string): void;
    resources(identities: readonly string[], generation: string): readonly RustyApplicationResource[];
    clear(): void;
}
