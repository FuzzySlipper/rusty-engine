// Generated from the Rust wire types by product-host's
// `typescript_contracts_are_current` test. Do not edit: change the Rust types
// and run scripts/generate-typescript-contracts.sh.

/** The runtime's route prefix. */
export const RUNTIME_BASE_PATH = "/__rusty/product/runtime/";

/** Where the page reads its `ProductHostBrowserBootstrap`. */
export const BOOTSTRAP_PATH = "product-bootstrap.json";

/**
 * A JSON u64 represented as canonical decimal text.
 */
export type CanonicalU64 = string;

export type ControllerAxis = "axis-0" | "axis-1" | "axis-2" | "axis-3";

export type ControllerButton = "button-0" | "button-1" | "button-2" | "button-3" | "button-4" | "button-5" | "button-6" | "button-7" | "button-8" | "button-9" | "button-10" | "button-11" | "button-12" | "button-13" | "button-14" | "button-15";

export type JsonValue = number | string | boolean | Array<JsonValue> | { [key in string]?: JsonValue } | null;

export type KeyboardControl = "key-a" | "key-b" | "key-c" | "key-d" | "key-e" | "key-f" | "key-g" | "key-h" | "key-i" | "key-j" | "key-k" | "key-l" | "key-m" | "key-n" | "key-o" | "key-p" | "key-q" | "key-r" | "key-s" | "key-t" | "key-u" | "key-v" | "key-w" | "key-x" | "key-y" | "key-z" | "digit-0" | "digit-1" | "digit-2" | "digit-3" | "digit-4" | "digit-5" | "digit-6" | "digit-7" | "digit-8" | "digit-9" | "space" | "enter" | "escape" | "shift-left" | "shift-right" | "control-left" | "control-right" | "alt-left" | "alt-right" | "arrow-up" | "arrow-down" | "arrow-left" | "arrow-right";

export type PointerButton = "primary" | "secondary" | "middle";

export type ProductHostBootstrapInput = { cursorMode: ProductHostCursorMode, };

export type ProductHostBootstrapLifecycle = { mode: ProductHostRuntimeMode, };

export type ProductHostBootstrapProduct = { id: string, title: string, };

export type ProductHostBootstrapRenderer = { 
/**
 * The page shows the runtime's frames, or lets the desktop window show
 * through.
 */
output: ProductHostRenderOutput, };

export type ProductHostBootstrapUi = { 
/**
 * The product UI module, relative to the page: `product-ui/...`.
 */
entry: string, };

/**
 * The product UI projection stream and contract the page admits.
 */
export type ProductHostBootstrapUiProjection = { expectedStream: string, expectedContract: string, };

/**
 * A browser connection identity and, after a fresh successful attachment, its
 * retained runtime boundary. This is deliberately limited to the one browser
 * attachment that may correlate a previously lost HTTP response.
 */
export type ProductHostBrowserAttachment = { id: string, replaces?: string, baseline?: ProductHostBrowserAttachmentBaseline, };

/**
 * The fixed runtime facts that prove a browser attachment has a fresh output
 * baseline. It is not a general browser state snapshot.
 */
export type ProductHostBrowserAttachmentBaseline = { runtime: ProductHostRuntimeBinding, nextInputSequence: CanonicalU64, };

/**
 * What the runtime pack's page needs to mount a product: the product host
 * writes it from the product's manifest.
 */
export type ProductHostBrowserBootstrap = { product: ProductHostBootstrapProduct, ui: ProductHostBootstrapUi, lifecycle: ProductHostBootstrapLifecycle, input: ProductHostBootstrapInput, uiProjection?: ProductHostBootstrapUiProjection, renderer: ProductHostBootstrapRenderer, };

export type ProductHostBrowserConnectionState = "open" | "closed";

/**
 * Fixed browser-host observation batch. This is deliberately a small health
 * report, not a browser console or generic diagnostic transport.
 */
export type ProductHostBrowserDiagnosticsReport = { hostState: ProductHostBrowserHostState, runtimeProgress: CanonicalU64, transportState: ProductHostBrowserConnectionState, outputState: ProductHostBrowserConnectionState, firstTerminal?: ProductHostBrowserTerminalDiagnostic, recoverableEvent?: ProductHostBrowserTerminalDiagnostic, pageEvents: Array<ProductHostBrowserPageDiagnostic>, attachment?: ProductHostBrowserAttachment, };

export type ProductHostBrowserDiagnosticsResult = { accepted: boolean, reported: number, };

export type ProductHostBrowserHostState = "loading" | "ready" | "degraded" | "failed" | "disposed";

export type ProductHostBrowserPageDiagnostic = { kind: ProductHostBrowserPageDiagnosticKind, code: string, message: string, };

export type ProductHostBrowserPageDiagnosticKind = "error" | "unhandled-rejection";

export type ProductHostBrowserTerminalDiagnostic = { code: string, message: string, };

/**
 * A camera pose as `engine.renderer.camera` reports it.
 */
export type ProductHostCameraPose = { position: [number, number, number], pitchDegrees: number, yawDegrees: number, };

/**
 * The `rusty-output-baseline` event that ends a connection's baseline.
 */
export type ProductHostConnectionBaseline = { 
/**
 * The output sequence at the baseline, so a caller can wait for a
 * later operation's outputs.
 */
outputThrough: CanonicalU64, accepted: boolean, code: string, disposition: ProductHostFaultDisposition, operation: ProductHostOperationKind, binding?: ProductHostRuntimeBinding, nextInputSequence?: CanonicalU64, 
/**
 * Last lifecycle simulation step admitted before this operation result.
 * It is present on a resync receipt when admission has already advanced
 * but the downstream callback/update could not be completed.
 */
admittedThrough?: CanonicalU64, readout?: ProductHostRuntimeReadout, diagnostic?: string, };

/**
 * `control/claim`: a harness takes input from `runtime`'s owner under a
 * fresh binding, labelled for any attached page, for `leaseMs` after its
 * last input.
 */
export type ProductHostControlClaimRequest = { runtime: ProductHostRuntimeBinding, label: string, leaseMs: CanonicalU64, };

/**
 * `control/replace`: advance the input control fence of `runtime`.
 */
export type ProductHostControlRequest = { runtime: ProductHostRuntimeBinding, };

/**
 * How gameplay holds the pointer.
 */
export type ProductHostCursorMode = "pointer-lock" | "unlocked";

/**
 * Read-only product-generated descriptor data for live-debug completion and
 * help. It is never a dispatch schema: command invocation remains the single
 * explicit `execute_debug` operation.
 */
export type ProductHostDebugCatalog = { available: boolean, commands: Array<ProductHostDebugCommandDescriptor>, };

export type ProductHostDebugCommandDescriptor = { name: string, description: string, parameters: Array<ProductHostDebugCommandParameterDescriptor>, };

export type ProductHostDebugCommandParameterDescriptor = { name: string, type: string, };

export type ProductHostDrawingMode = "continuous" | "on-demand";

/**
 * A streamed frame: its sequence on the frame route and the step it shows.
 */
export type ProductHostDrawnFrame = { sequence: number, step: number, };

/**
 * The body of a route that takes no arguments: `{}`.
 */
export type ProductHostEmptyRequest = Record<string, never>;

/**
 * `admit-external-step`: the step an external clock admits.
 */
export type ProductHostExternalRequest = { step: CanonicalU64, };

/**
 * Closed recovery posture for one host operation result. The code identifies
 * the precise failure while this value tells a host what it may safely do
 * next without interpreting the diagnostic text.
 */
export type ProductHostFaultDisposition = "accepted" | "rejected-recoverable" | "degraded" | "resync-required" | "terminal";

/**
 * `input`: one ordered input batch.
 */
export type ProductHostInputRequest = { batch: Array<RuntimeInputWireEvent>, };

/**
 * Typed input result supplied by the generated runtime.
 */
export type ProductHostInputResult = { accepted: boolean, code: string, disposition: ProductHostFaultDisposition, 
/**
 * Number of submitted events in this batch. Kept as `count` for
 * compatibility with existing host adapters. A strict-decode rejection
 * preserves the host-bounded submitted count even when it is above the
 * smaller admitted-wire-event limit.
 */
count: number, 
/**
 * Number of events admitted into the input lane. A safe stale/duplicate
 * drop makes this less than `count` while retaining the current cursor.
 */
acceptedCount: number, droppedCount: number, acceptedThrough?: CanonicalU64, consumedThrough?: CanonicalU64, nextInputSequence?: CanonicalU64, binding?: ProductHostRuntimeBinding, readout?: ProductHostRuntimeReadout, diagnostic?: string, };

/**
 * A lifecycle route's body. `runtime` names the binding the operation is
 * meant for; without it, the current one.
 */
export type ProductHostLifecycleRequest = { runtime?: ProductHostRuntimeBinding, };

/**
 * Closed operation identities returned by direct runtime calls.
 */
export type ProductHostOperationKind = "connect" | "start" | "pause" | "resume" | "restart" | "shutdown" | "report-fault" | "replace-control" | "release-control" | "claim-control" | "input" | "advance-realtime" | "admit-demand-step" | "admit-external-step" | "complete-timeline" | "execute-debug";

/**
 * Direct operation result supplied by the generated runtime.
 */
export type ProductHostOperationResult = { accepted: boolean, code: string, disposition: ProductHostFaultDisposition, operation: ProductHostOperationKind, binding?: ProductHostRuntimeBinding, nextInputSequence?: CanonicalU64, 
/**
 * Last lifecycle simulation step admitted before this operation result.
 * It is present on a resync receipt when admission has already advanced
 * but the downstream callback/update could not be completed.
 */
admittedThrough?: CanonicalU64, readout?: ProductHostRuntimeReadout, diagnostic?: string, };

/**
 * `advance-realtime`: the page's monotonic clock, in nanoseconds.
 */
export type ProductHostRealtimeRequest = { observedTimeNs: CanonicalU64, };

/**
 * Where the runtime presents the frames it renders.
 */
export type ProductHostRenderOutput = "stream" | "window";

/**
 * Answer to `engine.renderer.camera`, `.drawing` and `.frame`.
 */
export type ProductHostRendererInspection = { drawing: ProductHostDrawingMode, output: ProductHostRenderOutput, 
/**
 * The simulation is held, so frames show one step.
 */
held: boolean, 
/**
 * An observer camera replaces the product's primary camera.
 */
observer: boolean, 
/**
 * The observer camera, else the primary camera, when known.
 */
camera: ProductHostCameraPose | null, 
/**
 * The frame the command drew, else the last one drawn. The desktop
 * window does not number its frames.
 */
frame: ProductHostDrawnFrame | null, };

/**
 * Exact runtime generation binding used by browser input, operations, and outputs.
 */
export type ProductHostRuntimeBinding = { instanceId: CanonicalU64, generation: CanonicalU64, controlRevision: CanonicalU64, };

export type ProductHostRuntimeFault = "owner-reported" | "counter-exhausted";

export type ProductHostRuntimeMode = "realtime" | "demand" | "external";

export type ProductHostRuntimeOutput = { "kind": "binding", runtime: ProductHostRuntimeBinding, nextInputSequence: CanonicalU64, 
/**
 * The harness holding input, when one has claimed it: the page shows
 * it and sends no input until a binding without a claim arrives.
 */
inputClaim?: string, } | { "kind": "ui-projection", envelope: RuntimeUiProjectionEnvelope, } | { "kind": "runtime-readout", readout: ProductHostRuntimeReadout, } | { "kind": "runtime-input-result", result: ProductHostInputResult, };

/**
 * Minimal local readout passed through from the generated runtime owner.
 */
export type ProductHostRuntimeReadout = { artifact: "rusty.product.runtime-readout", runtime: ProductHostRuntimeBinding, mode: ProductHostRuntimeMode, state: ProductHostRuntimeState, admittedSimulationSteps: CanonicalU64, admittedPresentations: CanonicalU64, droppedRealtimeSteps: CanonicalU64, clockRegressions: CanonicalU64, scaledRemainder: number | null, lastObservedTimeNs: CanonicalU64 | null, fault: ProductHostRuntimeFault | null, };

export type ProductHostRuntimeState = "created" | "running" | "paused" | "faulted" | "shutdown";

/**
 * Answer to `engine.time`, `engine.time.mode` and `engine.time.advance`.
 */
export type ProductHostTimeAnswer = { mode: ProductHostTimeMode, 
/**
 * The last admitted simulation step.
 */
simulationStep: CanonicalU64, fixedStepHz: number, 
/**
 * Simulation time this command advanced.
 */
advancedMs: number, 
/**
 * The world waits for commands to advance it.
 */
worldHeld: boolean, };

/**
 * Who advances simulation time. Inspection time moves only forward.
 */
export type ProductHostTimeMode = "realtime" | "manual" | "action-driven";

export type ProductHostTimelineCompletion = { ticket: CanonicalU64, runtime: ProductHostRuntimeBinding, correlation: string, outcome: ProductHostTimelineOutcome, provenance: ProductHostTimelineProvenance, };

/**
 * Completion result supplied by the generated runtime.
 */
export type ProductHostTimelineCompletionResult = { accepted: boolean, code: string, disposition: ProductHostFaultDisposition, ticket: CanonicalU64, binding?: ProductHostRuntimeBinding, readout?: ProductHostRuntimeReadout, diagnostic?: string, };

export type ProductHostTimelineOutcome = { "kind": "success", data?: JsonValue, } | { "kind": "failure", data?: JsonValue, };

export type ProductHostTimelineProvenance = { correlation: string, detail?: JsonValue, };

/**
 * The runtime binding an envelope belongs to, as canonical decimal text.
 */
export type RuntimeInputWireBinding = { instanceId: string, generation: string, controlRevision: string, };

/**
 * Why a host cleared held input.
 */
export type RuntimeInputWireClearReason = "focus-loss" | "interaction-mode-loss" | "pointer-lock-loss" | "restart" | "control-revision-change" | "dispose" | "ingress-overflow";

export type RuntimeInputWireEdge = "pressed" | "released";

/**
 * One input envelope a host submits, in observation order: a physical fact
 * or an intent claimed by product UI.
 */
export type RuntimeInputWireEvent = RuntimeInputWirePhysical | RuntimeInputWireIntentClaim;

/**
 * A physical fact from the closed Engine input catalog.
 */
export type RuntimeInputWireFact = { "kind": "key", code: KeyboardControl, edge: RuntimeInputWireEdge, } | { "kind": "pointer-button", button: PointerButton, edge: RuntimeInputWireEdge, } | { "kind": "pointer-delta", x: number, y: number, } | { "kind": "wheel", x: number, y: number, } | { "kind": "controller-button", button: ControllerButton, edge: RuntimeInputWireEdge, } | { "kind": "controller-axis", axis: ControllerAxis, value: number, } | { "kind": "controller-button-value", button: ControllerButton, value: number, } | { "kind": "clear", reason: RuntimeInputWireClearReason, };

/**
 * A product-declared intent claimed by product UI, in the same ordered lane.
 */
export type RuntimeInputWireIntentClaim = { runtime: RuntimeInputWireBinding, 
/**
 * Canonical decimal u64, increasing within the binding.
 */
sequence: string, 
/**
 * Product-declared input context.
 */
context: string, 
/**
 * Product-declared intent name.
 */
intent: string, value: RuntimeInputWireIntentValue, };

export type RuntimeInputWireIntentValue = { "kind": "digital", active: boolean, } | { "kind": "axis", value: number, } | { "kind": "product-payload", contract: string, data: JsonValue, };

/**
 * A physical input fact observed for one runtime binding.
 */
export type RuntimeInputWirePhysical = { runtime: RuntimeInputWireBinding, 
/**
 * Canonical decimal u64, increasing within the binding.
 */
sequence: string, 
/**
 * Product-declared input context.
 */
context: string, fact: RuntimeInputWireFact, };

/**
 * The wire shape of a UI projection envelope: what the browser host
 * receives and passes to the mounted product UI.
 */
export type RuntimeUiProjectionEnvelope = { artifact: "rusty.product.ui-projection", runtime: RuntimeUiRuntimeWire, 
/**
 * Canonical decimal u64, increasing within the runtime binding.
 */
sequence: string, stream: string, 
/**
 * The product's projection contract identity.
 */
contract: string, 
/**
 * The product-owned projection value.
 */
value: JsonValue, };

/**
 * The runtime binding a projection belongs to, as canonical decimal text.
 */
export type RuntimeUiRuntimeWire = { instanceId: string, generation: string, controlRevision: string, };
