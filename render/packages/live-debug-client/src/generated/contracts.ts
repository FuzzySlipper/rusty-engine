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
 * The projected decals the world pass lays (`CameraView.SetDecal`): what
 * they hold and what keeping them costs. Their shading is part of the
 * `world` pass's time.
 */
export type ProductHostDecalStatistics = { 
/**
 * Why this device lays no decals; absent while it can.
 */
refused?: string, decals: number, 
/**
 * Grid entries (a decal counts once in each cell bucket its box
 * reaches), and the most one bucket lists: what a fragment there tests.
 */
entries: number, busiestBucket: number, 
/**
 * GPU bytes of their buffer, and the bytes the last change uploaded.
 */
bytes: number, uploadBytes: number, 
/**
 * CPU microseconds the last rebuild of the grid and rows took.
 */
buildMicros: number, 
/**
 * A normal atlas bends the surfaces.
 */
normals: boolean, 
/**
 * Decals that emit light, and whether an emission atlas gives it.
 */
emissive: number, emission: boolean, };

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
 * Why the wgpu device was lost (wgpu's reason and the driver's
 * message), once it has been: nothing draws after that.
 */
deviceLost?: string, 
/**
 * The device has timestamp queries, so the passes are timed.
 */
timestamps: boolean, limits: ProductHostComputeLimits, 
/**
 * The timed passes, in frame order.
 */
passes: Array<ProductHostGpuPass>, ambientOcclusion: ProductHostAmbientOcclusionStatistics, distanceFields: ProductHostDistanceFieldStatistics, lightClusters: ProductHostLightClusterStatistics, gpuCulling: ProductHostGpuCullingStatistics, scatterSelection: ProductHostScatterSelectionStatistics, impostors: ProductHostImpostorStatistics, occlusion: ProductHostOcclusionStatistics, indirectLight: ProductHostIndirectLightStatistics, 
/**
 * The wetness in force (`CameraView.SetWetness`); absent while dry.
 */
wetness?: ProductHostWetnessStatistics, 
/**
 * The snow in force (`CameraView.SetSnow`); absent without snow.
 */
snow?: ProductHostSnowStatistics, decals: ProductHostDecalStatistics, water: ProductHostWaterStatistics, upscale: ProductHostUpscaleStatistics, };

/**
 * The impostor levels' baked views: how many meshes are baked, their
 * atlases' GPU bytes and the last bake's wall time.
 */
export type ProductHostImpostorStatistics = { impostors: number, bytes: number, bakeMs: number, };

/**
 * The indirect light volume (`CameraView.SetIndirectLight`): its probes and
 * the bake that keeps them current.
 */
export type ProductHostIndirectLightStatistics = { 
/**
 * A volume is requested.
 */
enabled: boolean, 
/**
 * Probes per axis of the baked volume (zeros before the first bake).
 */
dims: [number, number, number], probes: number, 
/**
 * Probes inside geometry, filled from their neighbours.
 */
invalid: number, 
/**
 * Triangles in the bricks' and the outside's BVHs.
 */
triangles: number, 
/**
 * The last batch's wall time in milliseconds, collection included.
 */
bakeMs: number, 
/**
 * Batches of bricks baked since the renderer was made.
 */
bakes: number, 
/**
 * A change waits for the debounce, or bricks are dirty or baking.
 */
pending: boolean, 
/**
 * GPU bytes the volume's texture holds.
 */
bytes: bigint, 
/**
 * 16 m bricks the volume is kept in.
 */
bricks: number, 
/**
 * Bricks dirty or baking.
 */
bricksPending: number, 
/**
 * The last brick's bake wall milliseconds.
 */
brickMs: number, 
/**
 * The slowest brick since the renderer was made, in milliseconds.
 */
brickMsMax: number, 
/**
 * Bytes the last frame that uploaded bricks wrote to the texture.
 */
uploadBytes: bigint, 
/**
 * Bricks the last batch baked.
 */
lastBatchBricks: number, };

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
 * Why the last world view looped although clustering is on: more
 * unbounded lights than the global list names. Absent while it
 * clustered.
 */
fallback?: string, 
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
 * The occlusion test of the last world view's selected copies
 * (`RendererSettings` `OcclusionCulling`): its farthest-depth pyramid's
 * level-0 size and levels, and the samples per pixel of the occluder depth
 * it was reduced from (the world pass's).
 */
export type ProductHostOcclusionStatistics = { enabled: boolean, refused: string | null, size: [number, number], levels: number, samples: number, };

/**
 * Closed operation identities returned by direct runtime calls.
 */
export type ProductHostOperationKind = "connect" | "start" | "pause" | "resume" | "restart" | "shutdown" | "report-fault" | "replace-control" | "release-control" | "claim-control" | "input" | "advance-realtime" | "complete-timeline" | "execute-debug";

/**
 * A frame the desktop window presented: when, in Unix milliseconds, the
 * simulation step it showed, and the presentation time it drew at, which
 * runs on between steps while the world runs.
 */
export type ProductHostPresentedFrame = { atUnixMs: number, step: number, presentationSeconds: number, };

/**
 * Where the runtime presents the frames it renders.
 */
export type ProductHostRenderOutput = "stream" | "window";

export type ProductHostRendererSettingValues = { shadows: boolean, backdropShadows: boolean, shadowBudget: number | null, 
/**
 * `disabled`, `screenSpace` or `distanceField`.
 */
ambientOcclusion: string, ambientOcclusionStrength: number, ambientOcclusionRadius: number, 
/**
 * `off`, `2x`, `4x` (samples per pixel of the primary destination),
 * `temporal` (the Engine's temporal upscale stage) or `dlss` (NVIDIA
 * DLSS as that stage).
 */
antialiasing: string, 
/**
 * The fraction of the primary destination's size the world draws at.
 */
renderScale: number, vsync: boolean, 
/**
 * `windowed`, `maximized` or `fullscreen`.
 */
windowMode: string, clusteredLighting: boolean, gpuCulling: boolean, occlusionCulling: boolean, 
/**
 * `off`, `low` or `high`.
 */
volumetricFog: string, 
/**
 * `off`, `low` or `high`.
 */
volumetricClouds: string, 
/**
 * `off`, `low` or `high`.
 */
waterReflections: string, };

/**
 * The renderer settings (`RendererSettings`): what the product or its
 * manifest asked for, what draws, and why each refused setting differs.
 */
export type ProductHostRendererSettings = { requested: ProductHostRendererSettingValues, effective: ProductHostRendererSettingValues, 
/**
 * By setting name, why the device draws it differently.
 */
refused: Record<string, string>, };

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
skippedOps: Record<string, number>, lastSkip: string | null, shadows: ProductHostShadowStatistics, 
/**
 * The renderer's GPU passes: each timed pass's cost, the adapter's
 * compute limits, and the ambient occlusion the last world view took.
 */
gpu: ProductHostGpuStatistics, 
/**
 * The renderer settings in effect (`RendererSettings`).
 */
settings: ProductHostRendererSettings, 
/**
 * The scatter patches taken in so far.
 */
scatter: ProductHostScatterStatistics, };

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
 * The per-copy selection of the last world view's scatter runs: grass,
 * plants and vegetation copies chosen by distance, thinning and the
 * frustum, at their levels of detail.
 */
export type ProductHostScatterSelectionStatistics = { 
/**
 * The last world view selected its scatter runs on the GPU.
 */
enabled: boolean, 
/**
 * Why this device cannot; absent while it can.
 */
refused?: string, 
/**
 * Runs, parts and draws selected, and the copies they hold.
 */
runs: number, parts: number, draws: number, copies: number, 
/**
 * The commands the camera's draws take (a run sharing pass, features
 * and material is one multi-draw), and the bytes of the mesh pool they
 * draw from.
 */
commands: number, poolBytes: number, 
/**
 * From the last read-back pass: copies tested, drawn for the camera (a
 * crossfading copy twice), drawn for the shadows, thinned away,
 * outside the frustum and inside it but hidden behind the view's
 * opaque depth, and the camera's draws at levels 1 to 3.
 */
tested: number, drawn: number, shadow: number, thinned: number, outside: number, occluded: number, levels: [number, number, number], 
/**
 * Copies left undrawn because they would pass the device's storage
 * binding limit.
 */
overLimit: number, };

/**
 * Scatter patches (grass, plants, vegetation tiles) the renderer has taken
 * in since it was made; read twice and subtract for a span's upload.
 */
export type ProductHostScatterStatistics = { patches: number, copies: number, 
/**
 * CPU microseconds turning them into drawable parts.
 */
applyMicroseconds: number, 
/**
 * GPU row bytes those parts upload.
 */
rowBytes: number, };

/**
 * The scene's shadow layers and which requesting lights cast.
 */
export type ProductHostShadowStatistics = { 
/**
 * Layers in the shadow atlas, and the 2048² pages holding them.
 */
layers: number, pages: number, 
/**
 * `RustyEngineProductShadowBudget` in layers, if the manifest sets one.
 */
budget: number | null, 
/**
 * Lights casting, and the renderer handles of those requesting a
 * shadow that the budget left out.
 */
castingLights: number, skippedLights: number[], 
/**
 * Layers re-rendered in the last frame and the casters drawn into them.
 */
renderedLayers: number, renderedCasters: number, 
/**
 * The GPU bytes of the atlas's depth pages, allocated ones included.
 */
atlasBytes: number, 
/**
 * The static cache's bytes, the size of the atlas once a light's layer
 * has moving casters (their layers redraw only those); 0 before.
 */
staticCacheBytes: number, };

/**
 * The snow lying on the scene's lit surfaces, as the world pass draws it.
 */
export type ProductHostSnowStatistics = { 
/**
 * How much lies, 0 to 1.
 */
amount: number, 
/**
 * Its linear colour.
 */
color: [number, number, number], 
/**
 * The texels per metre its edge steps at (0: a smooth edge).
 */
texelsPerMetre: number, };

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
 * The UI page's repaints the window imported: how many, how many imports
 * failed, the median host-thread cost of a recent one (for Chromium
 * importing it, for Servo painting it too), and when the frames first
 * drawing recent ones were presented, in Unix milliseconds.
 */
export type ProductHostUiRepaints = { count: number, errors: number, medianMs: number, shownAtUnixMs: Array<number>, };

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
 * The last primary frame's render resolution, the output it was brought
 * to, and the upscale stage that did it.
 */
export type ProductHostUpscaleStatistics = { 
/**
 * `bilinear`, `taau` or `dlss`; absent while the views draw straight
 * into the output.
 */
method?: string, 
/**
 * Why DLSS does not draw while the settings ask for it, in NGX's words
 * where it has them; TAAU draws in its place.
 */
dlssRefused?: string, 
/**
 * The render resolution the primary views drew at, in pixels.
 */
render: [number, number], 
/**
 * The output's size, in pixels.
 */
output: [number, number], };

/**
 * The water pass's view of the scene: whether water materials can absorb,
 * refract and reflect what lies behind them on this device.
 */
export type ProductHostWaterStatistics = { 
/**
 * Why this device has no copy of the opaque scene's colour (it binds
 * too few textures), so water draws without absorption, refraction and
 * reflections; absent while it has one.
 */
refused?: string, };

/**
 * How wet the scene's lit surfaces are, as the world pass draws them.
 */
export type ProductHostWetnessStatistics = { 
/**
 * How wet, 0 to 1.
 */
wetness: number, 
/**
 * How much standing water gathers in puddles, 0 to 1.
 */
puddles: number, 
/**
 * How far the puddles are frozen, 0 (water) to 1 (ice).
 */
frozen: number, };

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
export type ProductHostWindowStatistics = { 
/**
 * The web engine drawing the UI page: `chromium`, or Servo's version and
 * the GL renderer it draws with.
 */
ui: string | null, recentFrames: number, framesPerSecond: number, medianMs: ProductHostWindowMedians, 
/**
 * When each recent frame was presented, the step it showed and the
 * presentation time it drew at.
 */
shown: Array<ProductHostPresentedFrame>, 
/**
 * When recent key and mouse button events reached the window, in Unix
 * milliseconds.
 */
inputsReceivedAtUnixMs: Array<number>, uiRepaints: ProductHostUiRepaints, };

export type RuntimeDiagnosticDisposition = "accepted" | "rejected-recoverable" | "degraded" | "resync-required" | "terminal";

export type RuntimeDiagnosticEvent = { sequence: CanonicalU64, monotonicNanoseconds: CanonicalU64, severity: RuntimeDiagnosticSeverity, disposition: RuntimeDiagnosticDisposition, source: string, code: string, message: string, runtime?: RuntimeDiagnosticRuntimeBinding, correlation?: string, fields?: Array<RuntimeDiagnosticField>, };

export type RuntimeDiagnosticField = { key: string, value: string, };

/**
 * Runtime provenance attached to a diagnostic without depending on a host
 * transport binding type.
 */
export type RuntimeDiagnosticRuntimeBinding = { instanceId: CanonicalU64, generation: CanonicalU64, controlRevision: CanonicalU64, };

export type RuntimeDiagnosticSeverity = "debug" | "info" | "warning" | "error";
