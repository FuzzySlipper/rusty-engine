/**
 * Transport-neutral client for the product-owned generated live-debug catalog.
 * Descriptor data is read-only help/completion data; this client never derives
 * command schemas or dispatches anything except one command-line string.
 */
export interface LiveDebugParameterDescriptor {
    readonly name: string;
    readonly type: string;
}
export interface LiveDebugCommandDescriptor {
    readonly name: string;
    readonly description: string;
    readonly parameters: readonly LiveDebugParameterDescriptor[];
}
export interface LiveDebugCatalog {
    readonly available: boolean;
    readonly commands: readonly LiveDebugCommandDescriptor[];
}
export interface LiveDebugResult {
    readonly succeeded: boolean;
    readonly message: string;
}
export interface LiveDebugTransport {
    catalog(signal?: AbortSignal): Promise<LiveDebugCatalog>;
    execute(command: string, signal?: AbortSignal): Promise<LiveDebugResult>;
}
export interface LiveDebugHttpTransportOptions {
    /** Defaults to the current page origin, preserving same-origin dev-host use. */
    readonly origin?: string;
    readonly fetch?: typeof globalThis.fetch;
}
/** Creates the default same-origin HTTP transport without owning UI state. */
export declare function createLiveDebugHttpTransport(options?: LiveDebugHttpTransportOptions): LiveDebugTransport;
/** Small UI/CLI-neutral helper for catalog-derived completion. */
export declare function completeLiveDebug(catalog: LiveDebugCatalog, prefix: string): readonly LiveDebugCommandDescriptor[];
