import { GENERATED_DEVELOPER_COMMAND_CONTRACT } from './generated-developer-command-contract.js';
/**
 * Public, transport-neutral developer-command client and optional application-host
 * pull-down console.  It intentionally knows no gameplay semantics: a product
 * supplies a bounded adapter, discovery snapshot, and (where it wants a form)
 * an explicit wire schema.  Descriptor help is deliberately not a schema.
 */
export declare const RUSTY_DEVELOPER_COMMAND_PROTOCOL_VERSION: 1;
export type RustyDeveloperCommandLane = typeof GENERATED_DEVELOPER_COMMAND_CONTRACT.lanes[number];
export type RustyDeveloperCommandValueSchema = {
    readonly kind: 'boolean';
} | {
    readonly kind: 'decimalU64';
} | {
    readonly kind: 'integer';
    readonly minimum?: number;
    readonly maximum?: number;
} | {
    readonly kind: 'string';
    readonly maximumBytes: number;
    readonly pattern?: 'identifier';
} | {
    readonly kind: 'array';
    readonly items: RustyDeveloperCommandValueSchema;
    readonly maximumItems: number;
} | {
    readonly kind: 'object';
    readonly fields: Readonly<Record<string, RustyDeveloperCommandWireField>>;
} | {
    readonly kind: 'enum';
    readonly values: readonly string[];
} | {
    readonly kind: 'taggedUnion';
    readonly tag: string;
    readonly variants: Readonly<Record<string, RustyDeveloperCommandValueSchema>>;
} | {
    readonly kind: 'opaqueJson';
    readonly maximumBytes: number;
    readonly maximumNodes: number;
};
export interface RustyDeveloperCommandWireField {
    readonly required: boolean;
    readonly value: RustyDeveloperCommandValueSchema;
}
/**
 * An explicit value codec supplied by a Rust/product host adapter.  This is
 * deliberately separate from `developer-command::TypeDescriptor`, which is a
 * bounded help/discovery summary and cannot safely describe all owner DTOs.
 */
export interface RustyDeveloperCommandWireSchema {
    readonly request: RustyDeveloperCommandValueSchema;
    readonly result: RustyDeveloperCommandValueSchema;
    readonly error: RustyDeveloperCommandValueSchema;
}
export interface RustyDeveloperCommandDescriptor {
    readonly id: string;
    readonly aliases: readonly string[];
    readonly lane: RustyDeveloperCommandLane;
    readonly summary: string;
    /** Discovery/help only; never used to encode a request. */
    readonly helpOnly?: boolean;
}
export interface RustyDeveloperCommandDiscovery {
    readonly protocolVersion: typeof RUSTY_DEVELOPER_COMMAND_PROTOCOL_VERSION;
    readonly runtime: string;
    readonly profile: string;
    readonly permittedLanes: readonly RustyDeveloperCommandLane[];
    readonly revision: string;
    readonly catalogEpoch: string;
    readonly contractFingerprint: string;
    readonly commands: readonly RustyDeveloperCommandDescriptor[];
}
export interface RustyDeveloperCommandRequest {
    readonly protocolVersion: typeof RUSTY_DEVELOPER_COMMAND_PROTOCOL_VERSION;
    readonly command: string;
    readonly correlation: string;
    readonly runtime: string;
    readonly expected: {
        readonly profile: string;
        readonly revision: string;
        readonly catalogEpoch: string;
    };
    readonly payload: unknown;
}
export type RustyDeveloperCommandOutcome = {
    readonly kind: 'success';
    readonly value: unknown;
    readonly receiptRefs: readonly string[];
} | {
    readonly kind: 'error';
    readonly code: string;
    readonly message: string;
    readonly details?: unknown;
};
export interface RustyDeveloperCommandResponse {
    readonly correlation: string;
    readonly runtime: string;
    readonly profile: string;
    readonly revision: string;
    readonly catalogEpoch: string;
    readonly outcome: RustyDeveloperCommandOutcome;
}
/** Product-owned adapter: dispatch, authorization, safe points and mutation stay behind it. */
export interface RustyDeveloperCommandAdapter {
    readonly discover: (signal?: AbortSignal) => Promise<unknown>;
    readonly execute: (request: Readonly<RustyDeveloperCommandRequest>, signal?: AbortSignal) => Promise<unknown>;
}
/** A product-owned codec attachment for a command the host already discovers. */
export interface RustyDeveloperCommandSchemaBinding {
    readonly command: string;
    readonly lane: RustyDeveloperCommandLane;
    readonly profile: string;
    readonly schema: RustyDeveloperCommandWireSchema;
}
/**
 * Schema-only product extension. Discovery remains the sole executable
 * catalog; bindings are reconciled against every accepted discovery snapshot.
 */
export interface RustyDeveloperCommandExtension {
    readonly namespace: string;
    readonly schemas: readonly RustyDeveloperCommandSchemaBinding[];
}
export interface RustyDeveloperCommandHistoryEntry {
    readonly phase: 'completed';
    readonly request: RustyDeveloperCommandRequest;
    readonly lane: RustyDeveloperCommandLane;
    readonly outcome: RustyDeveloperCommandOutcome;
    readonly receiptRefs: readonly string[];
    readonly runtime: string;
    readonly profile: string;
    readonly revision: string;
    readonly catalogEpoch: string;
    readonly at: number;
}
export interface RustyDeveloperCommandLocalFailure {
    readonly phase: 'pre-dispatch' | 'transport' | 'post-dispatch';
    readonly lane: RustyDeveloperCommandLane | null;
    readonly code: RustyDeveloperCommandClientError['code'];
    readonly message: string;
    /** Present only once the transport-bound request has been issued. */
    readonly request?: RustyDeveloperCommandRequest;
    readonly receiptRefs: readonly [];
    readonly at: number;
}
/** A portable command transcript, deliberately not a deterministic replay format. */
export interface RustyDeveloperCommandSequence {
    readonly kind: 'rusty_developer_command.sequence.v1';
    readonly note: 'portable command intent/history; not deterministic replay';
    readonly entries: readonly RustyDeveloperCommandHistoryEntry[];
}
export declare class RustyDeveloperCommandClientError extends Error {
    readonly code: 'disposed' | 'malformed' | 'unavailable' | 'correlation_reused' | 'stale_context' | 'unknown_command' | 'invalid_payload' | 'codec_unavailable' | 'cancelled' | 'invalid_extension' | 'invalid_schema';
    constructor(code: RustyDeveloperCommandClientError['code'], message: string, options?: ErrorOptions);
}
export interface RustyDeveloperCommandClient {
    readonly discover: (signal?: AbortSignal) => Promise<RustyDeveloperCommandDiscovery>;
    readonly execute: (command: string, payload: unknown, signal?: AbortSignal) => Promise<RustyDeveloperCommandResponse>;
    readonly descriptor: (commandOrAlias: string) => RustyDeveloperCommandDescriptor | null;
    readonly schema: (command: string) => RustyDeveloperCommandWireSchema | null;
    readonly history: () => readonly (RustyDeveloperCommandHistoryEntry | RustyDeveloperCommandLocalFailure)[];
    readonly exportSequence: () => RustyDeveloperCommandSequence;
    readonly dispose: () => void;
}
export interface RustyDeveloperCommandClientOptions {
    readonly adapter: RustyDeveloperCommandAdapter;
    readonly schemas?: Readonly<Record<string, RustyDeveloperCommandWireSchema>>;
    readonly extensions?: readonly RustyDeveloperCommandExtension[];
    readonly createCorrelation?: () => string;
    readonly now?: () => number;
}
export declare function createRustyDeveloperCommandClient(options: RustyDeveloperCommandClientOptions): RustyDeveloperCommandClient;
export declare function validateRustyDeveloperCommandWireValue(value: unknown, schema: RustyDeveloperCommandValueSchema): void;
