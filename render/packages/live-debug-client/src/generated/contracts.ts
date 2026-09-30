// Generated from the Rust wire types by product-dev-host's
// `typescript_contracts_are_current` test. Do not edit: change the Rust types
// and run scripts/generate-typescript-contracts.sh.

/**
 * A JSON u64 represented as canonical decimal text.
 */
export type CanonicalU64 = string;

/**
 * Read-only product-generated descriptor data for live-debug completion and
 * help. It is never a dispatch schema: command invocation remains the single
 * explicit `execute_debug` operation.
 */
export type ProductDevDebugCatalog = { available: boolean, commands: Array<ProductDevDebugCommandDescriptor>, };

export type ProductDevDebugCommandDescriptor = { name: string, description: string, parameters: Array<ProductDevDebugCommandParameterDescriptor>, };

export type ProductDevDebugCommandParameterDescriptor = { name: string, type: string, };

/**
 * `diagnostics/read`: diagnostics after a cursor, or the retained ones.
 */
export type ProductDevDiagnosticsReadRequest = { after?: CanonicalU64, };

/**
 * The `diagnostics/read` answer: retained diagnostics and host telemetry.
 */
export type ProductDevDiagnosticsReadResponse = { telemetry: ProductDevTelemetrySnapshot, events: Array<RuntimeDiagnosticEvent>, floorSequence: CanonicalU64, throughSequence: CanonicalU64, nextCursor: CanonicalU64, readMonotonicNanoseconds: CanonicalU64, lagged: boolean, warningCount: CanonicalU64, errorCount: CanonicalU64, droppedCount: CanonicalU64, };

export type ProductDevError = { code: string, diagnostic: string, };

/**
 * The body of every host error response.
 */
export type ProductDevErrorResponse = { accepted: false, error: ProductDevError, };

/**
 * Closed operation identities returned by direct runtime calls.
 */
export type ProductDevOperationKind = "connect" | "start" | "pause" | "resume" | "restart" | "shutdown" | "report-fault" | "replace-control" | "release-control" | "claim-control" | "input" | "advance-realtime" | "admit-demand-step" | "admit-external-step" | "complete-timeline" | "execute-debug";

/**
 * Where the runtime presents the frames it renders.
 */
export type ProductDevRenderOutput = "stream" | "window";

/**
 * The runtime renderer's adapter and what its recent frames cost.
 */
export type ProductDevRendererStatistics = { adapter: string, output: ProductDevRenderOutput, 
/**
 * The recent streamed frames; the desktop window streams nothing.
 */
stream?: ProductDevStreamStatistics, 
/**
 * Retained operations the renderer skipped, by kind.
 */
skippedOps: Record<string, number>, lastSkip: string | null, };

/**
 * Answer to `engine.renderer`, `.status`, `.show`, `.hide` and `.toggle`.
 */
export type ProductDevRendererStatus = { available: boolean, widget: ProductDevRendererWidget, 
/**
 * Why no renderer statistics are available.
 */
diagnostic?: string, renderer?: ProductDevRendererStatistics, };

/**
 * The renderer metrics widget every mounted live-debug panel shares.
 */
export type ProductDevRendererWidget = { visible: boolean, };

/**
 * Exact runtime generation binding used by browser input, operations, and outputs.
 */
export type ProductDevRuntimeBinding = { instanceId: CanonicalU64, generation: CanonicalU64, controlRevision: CanonicalU64, };

/**
 * Median milliseconds per frame for each stage of streaming it.
 */
export type ProductDevStreamMedians = { render: number, readback: number, encode: number, };

/**
 * What the recent streamed frames cost.
 */
export type ProductDevStreamStatistics = { 
/**
 * The size the most recent viewer asked for, in its pixels.
 */
viewerSize: [number, number] | null, recentFrames: number, framesPerSecond: number, medianMs: ProductDevStreamMedians, medianBytesPerFrame: number, bytesPerSecond: number, };

/**
 * Bounded host-owned product-lane telemetry returned alongside the existing
 * diagnostics batch. These are observations only; renderer cadence remains
 * in the browser renderer diagnostics report and is intentionally not folded
 * into this product-lane snapshot.
 */
export type ProductDevTelemetrySnapshot = { inFlightOperation: ProductDevOperationKind | null, inFlightAgeMs: CanonicalU64 | null, lastProductAdmissionLatencyMs: CanonicalU64 | null, lastInputAdmissionLatencyMs: CanonicalU64 | null, queuedInputBatches: number, queuedInputEvents: number, inputBatchCapacity: number, oldestInputAgeMs: CanonicalU64 | null, inputOverflowPending: boolean, 
/**
 * Progress rate in millihertz, retaining useful values below one update
 * per second without introducing floating point into the wire snapshot.
 */
runtimeProgressRateMillihertz: CanonicalU64 | null, runtimeProgressAgeMs: CanonicalU64 | null, runtimeProgressUnavailableReason: string | null, connections: number, subscribers: number, outputQueueItems: number, outputQueueCapacity: number, outputBindingActive: boolean, 
/**
 * Bounded attribution for completed C# update callbacks. Service totals
 * are nested within the callback duration, not additional frame time.
 */
updateAttribution: ProductDevUpdateAttributionSnapshot | null, };

/**
 * One complete C# update callback observation. Durations are integer
 * microseconds so the diagnostics wire remains canonical and float-free.
 */
export type ProductDevUpdateAttribution = { runtime: ProductDevRuntimeBinding | null, simulationStep: CanonicalU64, admittedStepCount: CanonicalU64, postCallbackDurationUs: CanonicalU64, callbackDurationUs: CanonicalU64, characterStepCalls: CanonicalU64, characterStepDurationUs: CanonicalU64, 
/**
 * Controller casts reported by character-step receipts; this is not a
 * low-level narrow-phase counter.
 */
characterStepCastCount: CanonicalU64, 
/**
 * Projection entries admitted by conservative character-query bounds,
 * plus active obstacles that have no cached projection bound.
 */
characterStepCandidateCount: CanonicalU64, 
/**
 * Actual Parry character cast/contact calls, nested within the logical
 * controller cast count and candidate count.
 */
characterStepNarrowPhaseCount: CanonicalU64, voxelResidencyCalls: CanonicalU64, voxelResidencyDurationUs: CanonicalU64, voxelScenePresentationCalls: CanonicalU64, voxelScenePresentationDurationUs: CanonicalU64, };

/**
 * Host-owned rolling distribution and long-lived slowest complete update.
 */
export type ProductDevUpdateAttributionSnapshot = { sampleCount: CanonicalU64, callbackDurationUsP50: CanonicalU64, callbackDurationUsP95: CanonicalU64, callbackDurationUsMax: CanonicalU64, latest: ProductDevUpdateAttribution, 
/**
 * Slowest complete callback retained in the current rolling window.
 */
rollingSlowest: ProductDevUpdateAttribution, rollingSlowestAgeMs: CanonicalU64, 
/**
 * Slowest complete callback observed for this host lifetime.
 */
slowest: ProductDevUpdateAttribution, slowestAgeMs: CanonicalU64, };

export type RuntimeDiagnosticDisposition = "accepted" | "rejected-recoverable" | "degraded" | "resync-required" | "terminal";

export type RuntimeDiagnosticEvent = { sequence: CanonicalU64, monotonicNanoseconds: CanonicalU64, severity: RuntimeDiagnosticSeverity, disposition: RuntimeDiagnosticDisposition, source: string, code: string, message: string, runtime?: RuntimeDiagnosticRuntimeBinding, correlation?: string, fields?: Array<RuntimeDiagnosticField>, };

export type RuntimeDiagnosticField = { key: string, value: string, };

/**
 * Runtime provenance attached to a diagnostic without depending on a host
 * transport binding type.
 */
export type RuntimeDiagnosticRuntimeBinding = { instanceId: CanonicalU64, generation: CanonicalU64, controlRevision: CanonicalU64, };

export type RuntimeDiagnosticSeverity = "debug" | "info" | "warning" | "error";
