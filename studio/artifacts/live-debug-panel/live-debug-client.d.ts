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
/** Bounded process-owned diagnostic event; it is not the presentation stream. */
export interface LiveDebugDiagnosticEvent {
    readonly sequence: string;
    readonly monotonicNanoseconds: string;
    readonly severity: 'debug' | 'info' | 'warning' | 'error';
    readonly disposition: 'accepted' | 'rejected-recoverable' | 'degraded' | 'resync-required' | 'terminal';
    readonly source: string;
    readonly code: string;
    readonly message: string;
    readonly fields?: readonly {
        readonly key: string;
        readonly value: string;
    }[];
}
export interface LiveDebugDiagnosticsBatch {
    readonly events: readonly LiveDebugDiagnosticEvent[];
    readonly floorSequence: string;
    readonly throughSequence: string;
    readonly nextCursor: string;
    readonly readMonotonicNanoseconds: string;
    readonly lagged: boolean;
    readonly warningCount: string;
    readonly errorCount: string;
    readonly droppedCount: string;
    /** Optional host-owned product lane facts; renderer telemetry is separate. */
    readonly telemetry?: LiveDebugTelemetrySnapshot;
}
export type LiveDebugOperationKind = 'connect' | 'start' | 'pause' | 'resume' | 'restart' | 'shutdown' | 'report-fault' | 'replace-control' | 'release-control' | 'input' | 'advance-realtime' | 'admit-demand-step' | 'admit-external-step' | 'complete-timeline' | 'report-audio-feedback' | 'report-animation-feedback' | 'report-ghost-plate-feedback' | 'report-renderer-diagnostics' | 'execute-debug';
/** Bounded product/runtime lane observations returned by the Engine host. */
export interface LiveDebugTelemetrySnapshot {
    readonly inFlightOperation: LiveDebugOperationKind | null;
    readonly inFlightAgeMs: string | null;
    readonly lastProductAdmissionLatencyMs: string | null;
    readonly lastInputAdmissionLatencyMs: string | null;
    readonly queuedInputBatches: number;
    readonly queuedInputEvents: number;
    readonly inputBatchCapacity: number;
    readonly oldestInputAgeMs: string | null;
    readonly inputOverflowPending: boolean;
    /** Progress rate in millihertz (1000 = one update per second). */
    readonly runtimeProgressRateMillihertz: string | null;
    readonly runtimeProgressAgeMs: string | null;
    readonly connections: number;
    readonly subscribers: number;
    readonly outputQueueItems: number;
    readonly outputQueueCapacity: number;
    readonly outputQueueFloor: string;
    readonly outputBindingActive: boolean;
}
export interface LiveDebugTransport {
    catalog(signal?: AbortSignal): Promise<LiveDebugCatalog>;
    execute(command: string, signal?: AbortSignal): Promise<LiveDebugResult>;
    diagnostics?(after?: string, signal?: AbortSignal): Promise<LiveDebugDiagnosticsBatch>;
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
/** Computes a browser renderer observation age from the process-owned sink clock. */
export declare function diagnosticRendererObservationAgeMilliseconds(batch: LiveDebugDiagnosticsBatch, event: LiveDebugDiagnosticEvent): number | null;
/**
 * Computes how old a diagnostic event is at the response read clock. This is
 * distinct from any age fact carried by the event itself (for example the
 * browser host's renderer observation age).
 */
export declare function diagnosticEventAgeMilliseconds(batch: LiveDebugDiagnosticsBatch, event: LiveDebugDiagnosticEvent): number | null;
