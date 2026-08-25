import type { RustyApplicationRuntimeIdentity } from './input-ingress.js';
/** The one Product UI projection artifact admitted by the application host. */
export declare const RUSTY_APPLICATION_UI_PROJECTION_ARTIFACT: "rusty.product.ui-projection";
export declare const RUSTY_APPLICATION_UI_PROJECTION_DEFAULT_STREAM = "product.ui";
export declare const RUSTY_APPLICATION_UI_PROJECTION_MAX_BYTES = 65536;
export declare const RUSTY_APPLICATION_UI_PROJECTION_MAX_WIRE_BYTES = 262144;
export declare const RUSTY_APPLICATION_UI_PROJECTION_MAX_NODES = 2048;
export declare const RUSTY_APPLICATION_UI_PROJECTION_MAX_DEPTH = 16;
export declare const RUSTY_APPLICATION_UI_PROJECTION_MAX_STRING_BYTES = 8192;
export declare const RUSTY_APPLICATION_UI_PROJECTION_MAX_ARRAY_LENGTH = 512;
export declare const RUSTY_APPLICATION_UI_PROJECTION_MAX_OBJECT_KEYS = 256;
export declare const RUSTY_APPLICATION_UI_PROJECTION_MAX_SUBSCRIBERS = 64;
export declare const RUSTY_APPLICATION_UI_PROJECTION_U64_MAXIMUM = 18446744073709551615n;
export type RustyApplicationUiProjectionJson = null | boolean | number | string | readonly RustyApplicationUiProjectionJson[] | {
    readonly [key: string]: RustyApplicationUiProjectionJson;
};
/**
 * A strict worker-to-DOM projection envelope. The value is detached and
 * deeply frozen before it crosses into a mounted product UI.
 */
export interface RustyApplicationUiProjectionEnvelope {
    readonly artifact: typeof RUSTY_APPLICATION_UI_PROJECTION_ARTIFACT;
    readonly runtime: RustyApplicationRuntimeIdentity;
    readonly sequence: string;
    readonly stream: string;
    readonly contract: string;
    readonly value: RustyApplicationUiProjectionJson;
}
export interface RustyApplicationUiProjectionReadout {
    readonly artifact: typeof RUSTY_APPLICATION_UI_PROJECTION_ARTIFACT;
    readonly expectedStream: string;
    readonly expectedContract: string;
    readonly runtime: RustyApplicationRuntimeIdentity | null;
    readonly sequence: string | null;
    readonly hasCurrent: boolean;
    readonly acceptedCount: number;
    readonly rejectedCount: number;
    readonly subscriberCount: number;
    readonly state: 'ready' | 'disposed';
}
export interface RustyApplicationUiProjectionView {
    /** Returns the current immutable envelope, or null before the first value. */
    readonly current: () => RustyApplicationUiProjectionEnvelope | null;
    /** Subscribe to the current value. Rebinding publishes null before later values. */
    readonly subscribe: (listener: (value: RustyApplicationUiProjectionEnvelope | null) => void) => () => void;
}
export interface RustyApplicationUiProjectionPort extends RustyApplicationUiProjectionView {
    /** Rebind the projection epoch and clear the current snapshot. */
    readonly bindRuntime: (runtime: RustyApplicationRuntimeIdentity) => boolean;
    /** Admit one Rust worker envelope into the current bound epoch. */
    readonly ingest: (envelope: unknown) => boolean;
    /** Alias used by adapters that model worker messages as received values. */
    readonly receive: (envelope: unknown) => boolean;
    readonly readout: () => RustyApplicationUiProjectionReadout;
    readonly dispose: () => void;
}
export interface RustyApplicationUiProjectionOptions {
    readonly expectedStream?: string;
    /** Product/source-linked contract identity; the host never invents one. */
    readonly expectedContract: string;
    readonly binding?: RustyApplicationRuntimeIdentity;
    readonly maximumBytes?: number;
    readonly maximumWireBytes?: number;
    readonly maximumNodes?: number;
    readonly maximumDepth?: number;
    readonly maximumStringBytes?: number;
    readonly maximumArrayLength?: number;
    readonly maximumObjectKeys?: number;
    readonly maximumSubscribers?: number;
}
export type RustyApplicationUiProjectionErrorCode = 'disposed' | 'invalid_envelope' | 'invalid_runtime' | 'invalid_sequence' | 'invalid_stream' | 'invalid_contract' | 'artifact_mismatch' | 'stream_mismatch' | 'contract_mismatch' | 'runtime_unbound' | 'runtime_mismatch' | 'sequence_not_increasing' | 'value_invalid' | 'value_limit_exceeded' | 'subscriber_limit_exceeded';
export declare class RustyApplicationUiProjectionError extends Error {
    readonly code: RustyApplicationUiProjectionErrorCode;
    constructor(code: RustyApplicationUiProjectionErrorCode, message: string, options?: ErrorOptions);
}
/**
 * Creates the host-owned projection channel. This is intentionally a small
 * ingress/store, not a query bus or product-state bridge: adapters bind an
 * epoch and deliver envelopes, while mounted UI can only read and subscribe.
 */
export declare function createRustyApplicationUiProjection(options: RustyApplicationUiProjectionOptions): RustyApplicationUiProjectionPort;
