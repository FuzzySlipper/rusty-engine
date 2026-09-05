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
    /** Why this host cannot currently report runtime progress, if known. */
    readonly runtimeProgressUnavailableReason: string | null;
    /** Most recent completed worker update and shell-local publication phases. */
    readonly workerUpdate: LiveDebugWorkerUpdateSnapshot | null;
    readonly connections: number;
    readonly subscribers: number;
    readonly outputQueueItems: number;
    readonly outputQueueCapacity: number;
    readonly outputQueueFloor: string;
    readonly outputBindingActive: boolean;
    /** Completed C# update samples. Service time is nested in callback time. */
    readonly updateAttribution: LiveDebugUpdateAttributionSnapshot | null;
}
export interface LiveDebugUpdateAttribution {
    /** Runtime incarnation that produced this completed callback, if available. */
    readonly runtime: LiveDebugRuntimeBinding | null;
    readonly simulationStep: string;
    readonly admittedStepCount: string;
    /** Rust staging/reduction/conversion/completion after the callback returns. */
    readonly postCallbackDurationUs: string;
    /** Inclusive C# callback duration, including native service calls. */
    readonly callbackDurationUs: string;
    readonly characterStepCalls: string;
    readonly characterStepDurationUs: string;
    /** Logical character-controller casts, not narrow-phase work. */
    readonly characterStepCastCount: string;
    /** Eligible world projection entries and call-local active obstacles. */
    readonly characterStepCandidateCount: string;
    /** Actual Parry character cast/contact calls. */
    readonly characterStepNarrowPhaseCount: string;
    readonly voxelResidencyCalls: string;
    readonly voxelResidencyDurationUs: string;
    readonly voxelScenePresentationCalls: string;
    readonly voxelScenePresentationDurationUs: string;
}
/** Exact runtime incarnation carried by worker readouts and update samples. */
export interface LiveDebugRuntimeBinding {
    readonly instanceId: string;
    readonly generation: string;
    readonly controlRevision: string;
}
/** Product runtime readout passed through unchanged from the worker owner. */
export interface LiveDebugRuntimeReadout {
    readonly artifact: string;
    readonly runtime: LiveDebugRuntimeBinding;
    readonly mode: 'realtime' | 'demand' | 'external';
    readonly state: 'created' | 'running' | 'paused' | 'faulted' | 'shutdown';
    readonly admittedSimulationSteps: string;
    readonly admittedPresentations: string;
    readonly droppedRealtimeSteps: string;
    readonly clockRegressions: string;
    readonly scaledRemainder: number | null;
    readonly lastObservedTimeNs: string | null;
    readonly fault: 'owner-reported' | 'counter-exhausted' | null;
}
/** Timings measured entirely in one worker process; they are not additive. */
export interface LiveDebugWorkerPhases {
    /** Includes callback, post-callback work, input, and lifecycle work. */
    readonly operationDurationUs: string;
    readonly outputConversionDurationUs: string;
    readonly outputEncodeWriteDurationUs: string;
    readonly inputQueueAgeUs: string | null;
}
/** A worker completion plus shell-local delivery, decode, queue, and publication facts. */
export interface LiveDebugWorkerUpdateSnapshot {
    readonly workerPid: string;
    readonly readout: LiveDebugRuntimeReadout | null;
    readonly phases: LiveDebugWorkerPhases;
    /** Shell-local interval spanning worker work and delivery; it is not network latency. */
    readonly shellDeliveryIntervalUs: string | null;
    readonly shellOutputDecodeDurationUs: string;
    readonly shellOutputQueueDurationUs: string;
    readonly shellPublicationDurationUs: string;
    readonly ageMs: string;
}
export interface LiveDebugUpdateAttributionSnapshot {
    readonly sampleCount: string;
    readonly callbackDurationUsP50: string;
    readonly callbackDurationUsP95: string;
    readonly callbackDurationUsMax: string;
    readonly latest: LiveDebugUpdateAttribution;
    readonly rollingSlowest: LiveDebugUpdateAttribution;
    readonly rollingSlowestAgeMs: string;
    readonly slowest: LiveDebugUpdateAttribution;
    readonly slowestAgeMs: string;
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
