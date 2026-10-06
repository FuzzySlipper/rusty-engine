// Generated from the Rust wire types by product-host's
// `typescript_contracts_are_current` test. Do not edit: change the Rust types
// and run scripts/generate-typescript-contracts.sh.

/**
 * A JSON u64 represented as canonical decimal text.
 */
export type CanonicalU64 = string;

export type ProductHostAmbientOcclusionPath = "off" | "compute" | "raster" | "distanceField";

/**
 * The screen-space ambient occlusion of the last world view.
 */
export type ProductHostAmbientOcclusionStatistics = { path: ProductHostAmbientOcclusionPath, 
/**
 * Why the compute path cannot run on this device; absent while it can.
 */
computeRefused?: string, 
/**
 * Workgroups the last occlusion dispatch took; 0 on the raster path.
 */
workgroups: number, 
/**
 * The occlusion texture of the last view, in texels.
 */
texture: [number, number], };

/**
 * The adapter's compute limits; the renderer's device takes wgpu's defaults.
 */
export type ProductHostComputeLimits = { workgroupSize: [number, number, number], invocationsPerWorkgroup: number, workgroupsPerDimension: number, workgroupStorageBytes: number, storageBufferBindingBytes: number, };

/**
 * Read-only product-generated descriptor data for live-debug completion and
 * help. It is never a dispatch schema: command invocation remains the single
 * explicit `execute_debug` operation.
 */
export type ProductHostDebugCatalog = { available: boolean, commands: Array<ProductHostDebugCommandDescriptor>, };

export type ProductHostDebugCommandDescriptor = { name: string, description: string, parameters: Array<ProductHostDebugCommandParameterDescriptor>, };

export type ProductHostDebugCommandParameterDescriptor = { name: string, type: string, };

/**
 * `diagnostics/read`: diagnostics after a cursor, or the retained ones.
 */
export type ProductHostDiagnosticsReadRequest = { after?: CanonicalU64, };

/**
 * The `diagnostics/read` answer: retained diagnostics and host telemetry.
 */
export type ProductHostDiagnosticsReadResponse = { telemetry: ProductHostTelemetrySnapshot, events: Array<RuntimeDiagnosticEvent>, floorSequence: CanonicalU64, throughSequence: CanonicalU64, nextCursor: CanonicalU64, readMonotonicNanoseconds: CanonicalU64, lagged: boolean, warningCount: CanonicalU64, errorCount: CanonicalU64, droppedCount: CanonicalU64, };

/**
 * The chunk distance field atlas the `distanceField` ambient occlusion
 * path cone-traces.
 */
export type ProductHostDistanceFieldStatistics = { 
/**
 * Why this device cannot trace the fields; absent while it can.
 */
refused?: string, 
/**
 * Chunk fields resident in the atlas.
 */
residentFields: number, 
/**
 * Bricks the atlas holds room for.
 */
atlasBricks: number, atlasBytes: bigint, 
/**
 * Fields placed around the camera for the last world view.
 */
lookupEntries: number, };

export type ProductHostErrorBody = { code: string, diagnostic: string, };

/**
 * The body of every host error response.
 */
export type ProductHostErrorResponse = { accepted: false, error: ProductHostErrorBody, };

/**
 * The GPU visibility of the last view pass.
 */
export type ProductHostGpuCullingStatistics = { 
/**
 * The last view pass drew its opaque batches from GPU-culled runs.
 */
enabled: boolean, 
/**
 * Why this device cannot cull on the GPU; absent while it can.
 */
refused?: string, 
/**
 * Opaque candidates the last pass tested.
 */
candidates: number, 
/**
 * Opaque batches the last pass drew indirectly.
 */
batches: number, 
/**
 * Instances the last read-back pass found visible.
 */
visible: number, 
/**
 * Runs of batches drawn with one multi-draw each.
 */
multiDraws: number, };

/**
 * One timed pass's GPU cost over the recent frames.
 */
export type ProductHostGpuPass = { pass: string, 
/**
 * Recent frames whose GPU time was read back.
 */
timedFrames: number, 
/**
 * Median GPU milliseconds of the pass over those frames; 0 with none.
 */
medianGpuMs: number, };

/**
 * What the renderer's GPU passes cost, from the device's timestamp
 * queries, with the adapter's compute limits.
 */
export type ProductHostGpuStatistics = { 
/**
 * The device has timestamp queries, so the passes are timed.
 */
timestamps: boolean, limits: ProductHostComputeLimits, 
/**
 * The timed passes, in frame order.
 */
passes: Array<ProductHostGpuPass>, ambientOcclusion: ProductHostAmbientOcclusionStatistics, distanceFields: ProductHostDistanceFieldStatistics, lightClusters: ProductHostLightClusterStatistics, gpuCulling: ProductHostGpuCullingStatistics, };

/**
 * The light clustering of the last world view.
 */
export type ProductHostLightClusterStatistics = { 
/**
 * The last world view's lighting read its clusters.
 */
enabled: boolean, 
/**
 * Why this device cannot cluster; absent while it can.
 */
refused?: string, 
/**
 * Tiles across, tiles down, depth slices.
 */
grid: [number, number, number], 
/**
 * Lights each cluster can hold.
 */
clusterCapacity: number, 
/**
 * Light rows the last binning placed in clusters, counted per cluster.
 */
binnedLights: number, 
/**
 * Lights every fragment sees: ambient, hemisphere, directional, and
 * point or spot lights without a range.
 */
globalLights: number, 
/**
 * Clusters that had more lights than they hold.
 */
overflowedClusters: number, };

/**
 * Closed operation identities returned by direct runtime calls.
 */
export type ProductHostOperationKind = "connect" | "start" | "pause" | "resume" | "restart" | "shutdown" | "report-fault" | "replace-control" | "release-control" | "claim-control" | "input" | "advance-realtime" | "admit-demand-step" | "admit-external-step" | "complete-timeline" | "execute-debug";

/**
 * Where the runtime presents the frames it renders.
 */
export type ProductHostRenderOutput = "stream" | "window";

/**
 * The runtime renderer's adapter and what its recent frames cost.
 */
export type ProductHostRendererStatistics = { adapter: string, output: ProductHostRenderOutput, 
/**
 * The recent streamed frames; the desktop window streams nothing.
 */
stream?: ProductHostStreamStatistics, 
/**
 * The recent frames the desktop window presented.
 */
window?: ProductHostWindowStatistics, 
/**
 * When each recent product update that received input finished, and
 * the step it simulated: the first frame showing that step or a later
 * one is the first to show the input.
 */
inputSteps: Array<ProductHostTimedStep>, 
/**
 * Retained operations the renderer skipped, by kind.
 */
skippedOps: Record<string, number>, lastSkip: string | null, 
/**
 * The renderer's GPU passes: each timed pass's cost, the adapter's
 * compute limits, and the ambient occlusion the last world view took.
 */
gpu: ProductHostGpuStatistics, };

/**
 * Answer to `engine.renderer`, `.status`, `.show`, `.hide` and `.toggle`.
 */
export type ProductHostRendererStatus = { available: boolean, widget: ProductHostRendererWidget, 
/**
 * Why no renderer statistics are available.
 */
diagnostic?: string, renderer?: ProductHostRendererStatistics, };

/**
 * The renderer metrics widget every mounted live-debug panel shares.
 */
export type ProductHostRendererWidget = { visible: boolean, };

/**
 * Exact runtime generation binding used by browser input, operations, and outputs.
 */
export type ProductHostRuntimeBinding = { instanceId: CanonicalU64, generation: CanonicalU64, controlRevision: CanonicalU64, };

/**
 * Median milliseconds per frame for each stage of streaming it.
 */
export type ProductHostStreamMedians = { render: number, readback: number, encode: number, };

/**
 * What the recent streamed frames cost.
 */
export type ProductHostStreamStatistics = { 
/**
 * The size the most recent viewer asked for, in its pixels.
 */
viewerSize: [number, number] | null, recentFrames: number, framesPerSecond: number, medianMs: ProductHostStreamMedians, medianBytesPerFrame: number, bytesPerSecond: number, 
/**
 * When each recent frame was published, and the step it showed.
 */
shown: Array<ProductHostTimedStep>, };

/**
 * Bounded host-owned product-lane telemetry returned alongside the existing
 * diagnostics batch. These are observations only; renderer statistics are
 * answered by `engine.renderer` and are not folded into this product-lane
 * snapshot.
 */
export type ProductHostTelemetrySnapshot = { inFlightOperation: ProductHostOperationKind | null, inFlightAgeMs: CanonicalU64 | null, lastProductAdmissionLatencyMs: CanonicalU64 | null, lastInputAdmissionLatencyMs: CanonicalU64 | null, queuedInputBatches: number, queuedInputEvents: number, inputBatchCapacity: number, oldestInputAgeMs: CanonicalU64 | null, inputOverflowPending: boolean, 
/**
 * Progress rate in millihertz, retaining useful values below one update
 * per second without introducing floating point into the wire snapshot.
 */
runtimeProgressRateMillihertz: CanonicalU64 | null, runtimeProgressAgeMs: CanonicalU64 | null, runtimeProgressUnavailableReason: string | null, connections: number, subscribers: number, outputQueueItems: number, outputQueueCapacity: number, outputBindingActive: boolean, 
/**
 * Bounded attribution for completed C# update callbacks. Service totals
 * are nested within the callback duration, not additional frame time.
 */
updateAttribution: ProductHostUpdateAttributionSnapshot | null, };

/**
 * A simulation step and when something happened to it, in Unix
 * milliseconds.
 */
export type ProductHostTimedStep = { atUnixMs: number, step: number, };

/**
 * One complete C# update callback observation. Durations are integer
 * microseconds so the diagnostics wire remains canonical and float-free.
 */
export type ProductHostUpdateAttribution = { runtime: ProductHostRuntimeBinding | null, simulationStep: CanonicalU64, admittedStepCount: CanonicalU64, postCallbackDurationUs: CanonicalU64, callbackDurationUs: CanonicalU64, characterStepCalls: CanonicalU64, characterStepDurationUs: CanonicalU64, 
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
export type ProductHostUpdateAttributionSnapshot = { sampleCount: CanonicalU64, callbackDurationUsP50: CanonicalU64, callbackDurationUsP95: CanonicalU64, callbackDurationUsMax: CanonicalU64, latest: ProductHostUpdateAttribution, 
/**
 * Slowest complete callback retained in the current rolling window.
 */
rollingSlowest: ProductHostUpdateAttribution, rollingSlowestAgeMs: CanonicalU64, 
/**
 * Slowest complete callback observed for this host lifetime.
 */
slowest: ProductHostUpdateAttribution, slowestAgeMs: CanonicalU64, };

/**
 * Median milliseconds per frame for each stage of presenting it: waiting
 * for the swapchain image, waiting for the scene, encoding and submitting
 * the frame, and presenting it.
 */
export type ProductHostWindowMedians = { acquire: number, lock: number, draw: number, present: number, };

/**
 * What the recent frames the desktop window presented cost, and when they
 * reached it.
 */
export type ProductHostWindowStatistics = { recentFrames: number, framesPerSecond: number, medianMs: ProductHostWindowMedians, 
/**
 * When each recent frame was presented, and the step it showed.
 */
shown: Array<ProductHostTimedStep>, 
/**
 * When recent key and mouse button events reached the window, in Unix
 * milliseconds.
 */
inputsReceivedAtUnixMs: Array<number>, };

export type RuntimeDiagnosticDisposition = "accepted" | "rejected-recoverable" | "degraded" | "resync-required" | "terminal";

export type RuntimeDiagnosticEvent = { sequence: CanonicalU64, monotonicNanoseconds: CanonicalU64, severity: RuntimeDiagnosticSeverity, disposition: RuntimeDiagnosticDisposition, source: string, code: string, message: string, runtime?: RuntimeDiagnosticRuntimeBinding, correlation?: string, fields?: Array<RuntimeDiagnosticField>, };

export type RuntimeDiagnosticField = { key: string, value: string, };

/**
 * Runtime provenance attached to a diagnostic without depending on a host
 * transport binding type.
 */
export type RuntimeDiagnosticRuntimeBinding = { instanceId: CanonicalU64, generation: CanonicalU64, controlRevision: CanonicalU64, };

export type RuntimeDiagnosticSeverity = "debug" | "info" | "warning" | "error";
