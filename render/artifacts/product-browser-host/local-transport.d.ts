import type { ProductBrowserRuntimeAdapter } from './product-browser-host.js';
/**
 * Fixed same-origin endpoint family for the generated local Product runtime.
 * The endpoint is deliberately an operation-specific route set, rather than
 * a method-name RPC endpoint or a generic message tunnel.
 */
export declare const PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH: "/__rusty/product/runtime/";
/** Fixed identity for the Engine-owned browser-to-local-runtime transport. */
export declare const PRODUCT_BROWSER_LOCAL_TRANSPORT_ARTIFACT: "rusty.product.local-runtime-transport";
export type ProductBrowserLocalFetch = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
/** Minimal EventSource shape kept injectable for deterministic headless tests. */
export interface ProductBrowserLocalEventSource {
    onopen: ((event: unknown) => void) | null;
    onmessage: ((event: {
        readonly data: string;
        readonly lastEventId: string;
    }) => void) | null;
    onerror: ((event: unknown) => void) | null;
    readonly addEventListener?: (type: 'rusty-output-lag' | 'rusty-output-fragment' | 'rusty-output-baseline', listener: (event: {
        readonly data: string;
        readonly lastEventId: string;
    }) => void) => void;
    readonly removeEventListener?: (type: 'rusty-output-lag' | 'rusty-output-fragment' | 'rusty-output-baseline', listener: (event: {
        readonly data: string;
        readonly lastEventId: string;
    }) => void) => void;
    readonly close: () => void;
}
export interface ProductBrowserLocalEventSourceConstructor {
    new (url: string): ProductBrowserLocalEventSource;
}
export interface ProductBrowserLocalTransportOptions {
    /** Same-origin absolute path; defaults to the fixed Engine route family. */
    readonly basePath?: string;
    /** Injectable only for tests or a host-owned fetch implementation. */
    readonly fetch?: ProductBrowserLocalFetch;
    /** Injectable only for headless tests. Browser builds use EventSource. */
    readonly eventSource?: ProductBrowserLocalEventSourceConstructor;
    readonly maximumResponseBytes?: number;
    readonly maximumOutputBytes?: number;
    /** Stream errors are surfaced here; the operation surface remains closed. */
    readonly onTransportError?: (error: ProductBrowserLocalTransportError) => void;
}
export type ProductBrowserLocalTransportErrorCode = 'invalid_options' | 'disposed' | 'request_failed' | 'response_decode_failed' | 'output_decode_failed' | 'stream_failed';
/**
 * What the browser can prove about a mutating request after an error.
 *
 * `outcome-unknown` is deliberately not retry advice: callers must fence and
 * rebaseline state before sending another mutation. A committed response can
 * still require an output resynchronization when its headers say so. Output
 * projection consumes that resynchronization in #7761; this transport only
 * preserves the proof when response-body delivery later fails.
 */
export interface ProductBrowserLocalTransportMutationState {
    readonly certainty: 'outcome-unknown' | 'not-applied' | 'committed';
    readonly outputRecovery: 'none' | 'fresh-baseline-required';
    /** Canonical retained-output cursor observed with the response, if any. */
    readonly outputThrough: string | null;
}
export declare class ProductBrowserLocalTransportError extends Error {
    readonly code: ProductBrowserLocalTransportErrorCode;
    readonly route: string | null;
    /** Explicit request-outcome and required output-recovery posture. */
    readonly mutation: ProductBrowserLocalTransportMutationState;
    constructor(code: ProductBrowserLocalTransportErrorCode, message: string, options?: ErrorOptions & {
        readonly route?: string;
        readonly mutation?: ProductBrowserLocalTransportMutationState;
    });
}
/**
 * Creates the Engine-owned local transport used by generated Product Bundles.
 * Rust serves the fixed operation routes and one bounded SSE output stream on
 * the same origin. The adapter only knows the typed route families below; it
 * cannot dispatch an arbitrary method or carry product state.
 */
export declare function createProductBrowserLocalHttpAdapter(options?: ProductBrowserLocalTransportOptions): ProductBrowserRuntimeAdapter;
