import { type RustyApplicationContent } from '@rusty-engine/application-host';
/**
 * Loads the immutable renderer resources selected during Product Create.
 *
 * `moduleUrl` belongs to the browser composition root rather than this Engine
 * module so both generated bundles and the packaged runtime shell resolve the
 * Product-owned descriptor and resources from the same directory.
 */
export declare function loadProductBrowserRendererInitialContent(moduleUrl: string | URL, fetcher?: typeof globalThis.fetch): Promise<RustyApplicationContent>;
