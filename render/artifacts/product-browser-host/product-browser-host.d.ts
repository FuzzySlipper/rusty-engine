import { mountRustyApplication, type RustyApplicationFrame, type RustyApplicationAnimationCueDefinition, type RustyApplicationHost, type RustyApplicationHostReadout, type RustyApplicationPresentationFrame, type RustyApplicationRendererOptions, type RustyApplicationRuntimeIdentity, type RustyApplicationRuntimeInputEnvelope, type RustyApplicationRuntimeInputOptions, type RustyApplicationUiMount, type RustyApplicationUiProjectionEnvelope, type RustyApplicationPresentationAspectBounds, type RustyApplicationViewComposition } from '@rusty-engine/application-host';
/** Fixed current artifact identity; compatibility follows actual code changes. */
export declare const PRODUCT_BROWSER_HOST_ARTIFACT: "rusty.product.browser-host";
export type ProductBrowserRuntimeMode = 'realtime' | 'demand' | 'external';
/**
 * Selects the owner that admits fixed-step realtime work.
 *
 * Browser products use the Engine renderer cadence by default. A packaged
 * product with an in-process Rust service can select `rust-host`; the
 * WebView still drains typed input on each animation frame and receives
 * retained outputs through the runtime subscription, but it never asks the
 * runtime to advance from its presentation clock.
 */
export type ProductBrowserRealtimeAdvanceOwner = 'browser' | 'rust-host';
/**
 * The fixed lifecycle operation vocabulary carried by the local bridge. The
 * operation union is deliberately closed: product code cannot turn the
 * bridge into a method-name RPC or invoke an arbitrary runtime operation.
 */
export type ProductBrowserLifecycleOperation = {
    readonly kind: 'start';
} | {
    readonly kind: 'pause';
} | {
    readonly kind: 'resume';
} | {
    readonly kind: 'restart';
} | {
    readonly kind: 'shutdown';
} | {
    readonly kind: 'report-fault';
};
export type ProductBrowserRuntimeOperationKind = ProductBrowserLifecycleOperation['kind'] | 'replace-control' | 'connect' | 'advance-realtime' | 'admit-demand-step' | 'admit-external-step';
/**
 * Closed Engine recovery posture for a completed local-host operation. The
 * accompanying stable code identifies the exact condition; callers must not
 * infer recovery policy from diagnostics.
 */
export type ProductBrowserHostFaultDisposition = 'accepted' | 'rejected-recoverable' | 'degraded' | 'resync-required' | 'terminal';
export interface ProductBrowserRuntimeOperationResult {
    readonly accepted: boolean;
    readonly code: string;
    readonly disposition: ProductBrowserHostFaultDisposition;
    readonly operation: ProductBrowserRuntimeOperationKind;
    readonly binding?: RustyApplicationRuntimeIdentity;
    /** Engine-owned cursor after lifecycle input clear/rebind work. */
    readonly nextInputSequence?: string;
    /** Last simulation step admitted before a resync-required operation result. */
    readonly admittedThrough?: string;
    readonly readout?: ProductBrowserRuntimeReadout;
    readonly diagnostic?: string;
}
export interface ProductBrowserRuntimeInputResult {
    readonly accepted: boolean;
    readonly code: string;
    readonly disposition: ProductBrowserHostFaultDisposition;
    /** Number of submitted input events (the existing count field). */
    readonly count: number;
    /** Number of events admitted by the Engine input lane. */
    readonly acceptedCount?: number;
    /** Number of safe stale/duplicate events deliberately dropped. */
    readonly droppedCount?: number;
    readonly acceptedThrough?: string;
    readonly consumedThrough?: string;
    readonly nextInputSequence?: string;
    readonly binding?: RustyApplicationRuntimeIdentity;
    readonly readout?: ProductBrowserRuntimeReadout;
    readonly diagnostic?: string;
}
/** Closed browser-to-runtime audio realization feedback; no browser objects cross this boundary. */
export type ProductBrowserAudioFeedbackFact = {
    readonly kind: 'naturalCompletion';
    readonly source: 'oneShot';
    readonly factId: string;
    readonly sequence: number;
    readonly signalHandle: string;
} | {
    readonly kind: 'naturalCompletion';
    readonly source: 'retainedVoice';
    readonly factId: string;
    readonly sequence: number;
    readonly voiceHandle: string;
} | {
    readonly kind: 'diagnostic';
    readonly factId: string;
    readonly code: string;
    readonly sequence: number;
    readonly voiceHandle: string | null;
};
export interface ProductBrowserAudioFeedback {
    readonly runtime: RustyApplicationRuntimeIdentity;
    readonly replaceOwner: boolean;
    readonly evictedFactCount: string;
    readonly facts: readonly ProductBrowserAudioFeedbackFact[];
}
export interface ProductBrowserAudioFeedbackResult {
    readonly accepted: boolean;
    readonly code: string;
    readonly disposition: ProductBrowserHostFaultDisposition;
    /** The exact runtime binding which accepted or rejected this fixed report. */
    readonly runtime: RustyApplicationRuntimeIdentity;
    /** The accepted submitted boundary; absent when the fixed report had no facts. */
    readonly acceptedThroughFactId?: string;
    readonly diagnostic?: string;
}
/** Closed renderer-observation feedback; this is not an animation command route. */
export type ProductBrowserAnimationFeedbackFact = {
    readonly kind: 'playbackObservation';
    readonly factId: string;
    readonly objectId: string;
    readonly generation: string;
    readonly sequence: number;
    readonly status: string;
    readonly selectedClip: string | null;
    readonly sampledAtSeconds: number | null;
} | {
    readonly kind: 'naturalCompletion';
    readonly factId: string;
    readonly objectId: string;
    readonly generation: string;
    readonly clip: string;
} | {
    readonly kind: 'diagnostic';
    readonly factId: string;
    readonly objectId: string | null;
    readonly generation: string | null;
    readonly code: string;
    readonly sequence: number;
} | {
    readonly kind: 'cue';
    readonly factId: string;
    readonly objectId: string;
    readonly generation: string;
    readonly cueId: string;
    readonly clip: string;
    readonly markerSeconds: number;
    readonly sampledAtSeconds: number;
    readonly signalDomain: 'audio' | 'particle';
    readonly signalId: string;
} | {
    readonly kind: 'stopped';
    readonly factId: string;
    readonly objectId: string;
    readonly generation: string;
    readonly sequence: number;
    readonly reason: 'destroyed' | 'teardown';
};
export interface ProductBrowserAnimationFeedback {
    readonly runtime: RustyApplicationRuntimeIdentity;
    readonly replaceOwner: boolean;
    readonly evictedFactCount: string;
    readonly facts: readonly ProductBrowserAnimationFeedbackFact[];
}
export interface ProductBrowserAnimationFeedbackResult {
    readonly accepted: boolean;
    readonly code: string;
    readonly disposition: ProductBrowserHostFaultDisposition;
    readonly runtime: RustyApplicationRuntimeIdentity;
    readonly acceptedThroughFactId?: string;
    readonly diagnostic?: string;
}
/** Latest retained ghost-plate realization snapshot. Owner identities are opaque Engine values. */
export interface ProductBrowserGhostPlateFeedbackFact {
    readonly presentation: string;
    readonly sourceMatches: boolean;
    readonly currentSector: number;
    readonly localAngularOffsetDegrees: number | null;
    readonly fallbackActive: boolean;
    readonly fallbackReason: 'none' | 'preparedSourceUnsupported' | 'realizationFailed';
    /** Closed GhostPlateLimitationMask bits copied from the renderer host. */
    readonly limitationMask: number;
    readonly preparationCpuMilliseconds: number | null;
    readonly captureCpuSubmissionMilliseconds: number | null;
    readonly retainedSectorCount: number;
    readonly retainedMeshCount: number;
    readonly retainedMaterialCount: number;
    readonly retainedBorrowedTextureCount: number;
}
export interface ProductBrowserGhostPlateFeedback {
    readonly runtime: RustyApplicationRuntimeIdentity;
    readonly replaceOwner: boolean;
    readonly facts: readonly ProductBrowserGhostPlateFeedbackFact[];
}
export interface ProductBrowserGhostPlateFeedbackResult {
    readonly accepted: boolean;
    readonly code: string;
    readonly disposition: ProductBrowserHostFaultDisposition;
    readonly runtime: RustyApplicationRuntimeIdentity;
    readonly diagnostic?: string;
}
export interface ProductBrowserRendererDiagnosticsFeedback {
    readonly runtime: RustyApplicationRuntimeIdentity;
    readonly snapshot: ReturnType<RustyApplicationHost['renderer']['diagnosticsReadout']> & {
        readonly productFrames?: ProductBrowserProductFrameObservationSample;
    };
}
export interface ProductBrowserProductFrameObservationSample {
    readonly schemaVersion: 1;
    readonly observedAtMs: number;
    readonly receivedCount: number;
    readonly appliedCount: number;
    readonly firstReceivedAtMs: number | null;
    readonly lastReceivedAtMs: number | null;
    readonly lastAppliedAtMs: number | null;
    readonly recentReceivedIntervalsMs: readonly number[];
    readonly recentAppliedIntervalsMs: readonly number[];
    readonly recentApplyLatencyMs: readonly number[];
}
export interface ProductBrowserRendererDiagnosticsFeedbackResult {
    readonly accepted: boolean;
    readonly code: string;
    readonly disposition: ProductBrowserHostFaultDisposition;
    readonly runtime: RustyApplicationRuntimeIdentity;
    readonly diagnostic?: string;
}
export interface ProductBrowserTimelineCompletion {
    /** Canonical decimal u64 ticket issued by runtime-timeline. */
    readonly ticket: string;
    readonly runtime: RustyApplicationRuntimeIdentity;
    readonly correlation: string;
    readonly outcome: {
        readonly kind: 'success';
        readonly data?: ProductBrowserJson;
    } | {
        readonly kind: 'failure';
        readonly data?: ProductBrowserJson;
    };
    readonly provenance: {
        readonly correlation: string;
        readonly detail?: ProductBrowserJson;
    };
}
export interface ProductBrowserTimelineCompletionResult {
    readonly accepted: boolean;
    readonly code: string;
    readonly disposition: ProductBrowserHostFaultDisposition;
    /** Canonical decimal u64 ticket echoed by runtime-timeline. */
    readonly ticket: string;
    readonly binding?: RustyApplicationRuntimeIdentity;
    readonly readout?: ProductBrowserRuntimeReadout;
    readonly diagnostic?: string;
}
/** A bounded semantic-neutral readout emitted by the Rust runtime owner. */
export interface ProductBrowserRuntimeReadout {
    readonly artifact: 'rusty.product.runtime-readout';
    readonly runtime: RustyApplicationRuntimeIdentity;
    readonly mode: ProductBrowserRuntimeMode;
    readonly state: 'created' | 'running' | 'paused' | 'faulted' | 'shutdown';
    readonly admittedSimulationSteps: string;
    readonly admittedPresentations: string;
    readonly droppedRealtimeSteps: string;
    readonly clockRegressions: string;
    readonly scaledRemainder: number | null;
    readonly lastObservedTimeNs: string | null;
    readonly fault: 'owner-reported' | 'counter-exhausted' | null;
}
export interface ProductBrowserRuntimeBindingOutput {
    readonly kind: 'binding';
    readonly runtime: RustyApplicationRuntimeIdentity;
    readonly nextInputSequence: string;
}
export type ProductBrowserRuntimeOutput = ProductBrowserRuntimeBindingOutput
/** Fixed host evidence that one Rust-owned realtime advance was accepted. */
 | {
    readonly kind: 'runtime-progress';
    readonly owner: 'rust-host';
}
/** Later Engine admission receipt for an input batch accepted by the Rust-host mailbox. */
 | {
    readonly kind: 'runtime-input-result';
    readonly result: ProductBrowserRuntimeInputResult;
} | {
    readonly kind: 'frame';
    readonly frame: RustyApplicationFrame;
} | {
    readonly kind: 'view-composition';
    readonly composition: RustyApplicationViewComposition;
} | {
    readonly kind: 'animation-cue-definitions';
    readonly definitions: readonly RustyApplicationAnimationCueDefinition[];
} | {
    readonly kind: 'presentation';
    readonly frame: RustyApplicationPresentationFrame;
} | {
    readonly kind: 'ui-projection';
    readonly envelope: RustyApplicationUiProjectionEnvelope;
} | {
    readonly kind: 'runtime-readout';
    readonly readout: ProductBrowserRuntimeReadout;
};
/**
 * Buffers semantic runtime outputs while the renderer is mounting. Realtime
 * progress is only a liveness pulse and has no state to replay once the host
 * becomes ready; runtime readouts are snapshots, so only the newest one is
 * useful. Retained presentation outputs preserve their original ordering.
 *
 * @internal
 */
export declare function bufferProductBrowserPreMountOutput(pendingOutputs: ProductBrowserRuntimeOutput[], output: ProductBrowserRuntimeOutput, maximumPendingOutputs: number): boolean;
export type ProductBrowserRuntimeOutputListener = (output: ProductBrowserRuntimeOutput) => void;
export type ProductBrowserRuntimeOutputBatchListener = (outputs: readonly ProductBrowserRuntimeOutput[]) => void;
/**
 * Terminal local-transport failures. A dropped retained-output diff cannot
 * be recovered by EventSource retry; the host must stop until a fresh runtime
 * snapshot is mounted.
 */
export interface ProductBrowserRuntimeTerminalFailure {
    /** The fixed Engine failure lane; products never supply an arbitrary event name. */
    readonly kind: 'output-lag' | 'runtime-failure';
    readonly diagnostic: string;
}
/** Fixed health facts copied into the Engine diagnostic ring; never console data. */
export interface ProductBrowserDiagnosticsReport {
    readonly hostState: 'loading' | 'ready' | 'degraded' | 'failed' | 'disposed';
    readonly runtimeProgress: string;
    readonly transportState: 'open' | 'closed';
    readonly outputState: 'open' | 'closed';
    readonly lastRendererSequence?: string;
    readonly rendererObservationAgeMs?: string;
    readonly firstTerminal?: {
        readonly code: string;
        readonly message: string;
    };
    /** One bounded, typed operation observation that was deliberately dropped. */
    readonly recoverableEvent?: {
        readonly code: 'CSHARP_LIFECYCLE_CLOCK_REGRESSION' | 'BROWSER_RENDERER_DIAGNOSTICS_UNAVAILABLE' | 'BROWSER_LOCAL_REQUEST_UNAVAILABLE';
        readonly message: string;
    };
    readonly pageEvents: readonly {
        readonly kind: 'error' | 'unhandled-rejection';
        readonly code: string;
        readonly message: string;
    }[];
}
export interface ProductBrowserDiagnosticsResult {
    readonly accepted: boolean;
    readonly reported: number;
}
export type ProductBrowserRuntimeTerminalFailureListener = (failure: ProductBrowserRuntimeTerminalFailure) => void;
/**
 * A source-linked local runtime adapter. The implementation may be a Rust
 * worker, an in-process native bridge, or a deterministic test adapter, but
 * the operation surface is fixed and named. It has no generic `call` or
 * arbitrary message method.
 */
export interface ProductBrowserRuntimeAdapter {
    /**
     * Resolves the Engine-owned fresh connection baseline. Local generated
     * hosts use this instead of issuing `start` on every browser mount.
     */
    readonly connect?: () => Promise<ProductBrowserRuntimeOperationResult>;
    readonly lifecycle: (operation: ProductBrowserLifecycleOperation) => Promise<ProductBrowserRuntimeOperationResult>;
    /** Advances only the current input control fence; it does not fault or restart the product. */
    readonly replaceControl?: (runtime: RustyApplicationRuntimeIdentity) => Promise<ProductBrowserRuntimeOperationResult>;
    readonly input: (batch: readonly RustyApplicationRuntimeInputEnvelope[]) => Promise<ProductBrowserRuntimeInputResult>;
    readonly reportAudioFeedback: (feedback: ProductBrowserAudioFeedback) => Promise<ProductBrowserAudioFeedbackResult>;
    readonly reportAnimationFeedback: (feedback: ProductBrowserAnimationFeedback) => Promise<ProductBrowserAnimationFeedbackResult>;
    readonly reportGhostPlateFeedback: (feedback: ProductBrowserGhostPlateFeedback) => Promise<ProductBrowserGhostPlateFeedbackResult>;
    readonly reportRendererDiagnostics?: (feedback: ProductBrowserRendererDiagnosticsFeedback) => Promise<ProductBrowserRendererDiagnosticsFeedbackResult>;
    readonly reportBrowserDiagnostics?: (report: ProductBrowserDiagnosticsReport) => Promise<ProductBrowserDiagnosticsResult>;
    readonly advanceRealtime: (observedTimeNs: string) => Promise<ProductBrowserRuntimeOperationResult>;
    readonly admitDemandStep?: () => Promise<ProductBrowserRuntimeOperationResult>;
    readonly admitExternalStep?: (step: string) => Promise<ProductBrowserRuntimeOperationResult>;
    readonly completeTimeline?: (completion: ProductBrowserTimelineCompletion) => Promise<ProductBrowserTimelineCompletionResult>;
    readonly subscribeTerminalFailures?: (listener: ProductBrowserRuntimeTerminalFailureListener) => () => void;
    readonly subscribeOutputs: (listener: ProductBrowserRuntimeOutputListener) => () => void;
    /** One callback per ordered host receipt or complete connection baseline. */
    readonly subscribeOutputBatches?: (listener: ProductBrowserRuntimeOutputBatchListener) => () => void;
    /** Resolves once an asynchronous output subscription can receive runtime publications. */
    readonly waitUntilOutputSubscriptionReady?: () => Promise<void>;
    readonly dispose: () => Promise<void> | void;
}
/** The transport kept by the generated bridge and consumed by the host. */
export interface ProductBrowserRuntimeTransport {
    readonly connect?: NonNullable<ProductBrowserRuntimeAdapter['connect']>;
    readonly lifecycle: ProductBrowserRuntimeAdapter['lifecycle'];
    readonly replaceControl?: NonNullable<ProductBrowserRuntimeAdapter['replaceControl']>;
    readonly input: ProductBrowserRuntimeAdapter['input'];
    readonly reportAudioFeedback: ProductBrowserRuntimeAdapter['reportAudioFeedback'];
    readonly reportAnimationFeedback: ProductBrowserRuntimeAdapter['reportAnimationFeedback'];
    readonly reportGhostPlateFeedback: ProductBrowserRuntimeAdapter['reportGhostPlateFeedback'];
    readonly reportRendererDiagnostics?: NonNullable<ProductBrowserRuntimeAdapter['reportRendererDiagnostics']>;
    readonly reportBrowserDiagnostics?: NonNullable<ProductBrowserRuntimeAdapter['reportBrowserDiagnostics']>;
    readonly advanceRealtime: ProductBrowserRuntimeAdapter['advanceRealtime'];
    readonly admitDemandStep?: NonNullable<ProductBrowserRuntimeAdapter['admitDemandStep']>;
    readonly admitExternalStep?: NonNullable<ProductBrowserRuntimeAdapter['admitExternalStep']>;
    readonly completeTimeline?: NonNullable<ProductBrowserRuntimeAdapter['completeTimeline']>;
    readonly subscribeTerminalFailures?: NonNullable<ProductBrowserRuntimeAdapter['subscribeTerminalFailures']>;
    readonly subscribeOutputs: ProductBrowserRuntimeAdapter['subscribeOutputs'];
    readonly subscribeOutputBatches?: NonNullable<ProductBrowserRuntimeAdapter['subscribeOutputBatches']>;
    readonly waitUntilOutputSubscriptionReady?: NonNullable<ProductBrowserRuntimeAdapter['waitUntilOutputSubscriptionReady']>;
    readonly dispose: ProductBrowserRuntimeAdapter['dispose'];
}
export declare function createProductBrowserRuntimeTransport(adapter: ProductBrowserRuntimeAdapter): ProductBrowserRuntimeTransport;
export interface ProductBrowserHostOptions {
    readonly root: HTMLElement;
    readonly transport: ProductBrowserRuntimeTransport;
    readonly lifecycleMode: ProductBrowserRuntimeMode;
    /**
     * Owner of realtime simulation admission. Defaults to `browser`; use
     * `rust-host` only when a packaged in-process Rust host advances the runtime
     * and publishes outputs through `transport.subscribeOutputs`.
     */
    readonly realtimeAdvanceOwner?: ProductBrowserRealtimeAdvanceOwner;
    readonly mountUi: RustyApplicationUiMount;
    readonly runtimeInput?: Omit<RustyApplicationRuntimeInputOptions, 'binding' | 'onAvailable'> & {
        readonly binding?: RustyApplicationRuntimeIdentity;
    };
    readonly uiProjection?: Omit<ProductBrowserUiProjectionOptions, 'binding'> & {
        readonly binding?: RustyApplicationRuntimeIdentity;
    };
    readonly renderer?: Omit<RustyApplicationRendererOptions, 'onCadence'>;
    readonly presentationAspectBounds?: RustyApplicationPresentationAspectBounds;
    readonly initialInteractionMode?: 'gameplay' | 'interface' | 'modal';
    readonly inputContext?: string;
    readonly loadingLabel?: string;
    readonly failureLabel?: string;
    /** Start the Rust runtime after the Engine host has mounted. Defaults true. */
    readonly autoStart?: boolean;
}
/** @internal Reports whether admitted animation bytes still need their first semantic frame. */
export declare function productBrowserInitialRendererFrameRequired(renderer: ProductBrowserHostOptions['renderer']): boolean;
/** @internal Binds admitted preload bytes to one retained frame without mutating caller state. */
export declare function bindProductBrowserInitialRendererFrame(renderer: NonNullable<ProductBrowserHostOptions['renderer']>, frame: RustyApplicationFrame): NonNullable<ProductBrowserHostOptions['renderer']>;
/** @internal Folds only the pre-publication retained diffs into the mount frame. */
export declare function prepareProductBrowserInitialRendererBaseline(outputs: readonly ProductBrowserRuntimeOutput[], requiredFrame: RustyApplicationFrame): {
    readonly frame: RustyApplicationFrame;
    readonly remainingOutputs: readonly ProductBrowserRuntimeOutput[];
};
export interface ProductBrowserUiProjectionOptions {
    readonly expectedStream?: string;
    readonly expectedContract: string;
    readonly maximumBytes?: number;
    readonly maximumWireBytes?: number;
    readonly maximumNodes?: number;
    readonly maximumDepth?: number;
    readonly maximumStringBytes?: number;
    readonly maximumArrayLength?: number;
    readonly maximumObjectKeys?: number;
    readonly maximumSubscribers?: number;
}
export interface ProductBrowserHostReadout {
    readonly artifact: typeof PRODUCT_BROWSER_HOST_ARTIFACT;
    readonly state: 'starting' | 'ready' | 'degraded' | 'failed' | 'disposed';
    readonly mode: ProductBrowserRuntimeMode;
    readonly realtimeAdvanceOwner: ProductBrowserRealtimeAdvanceOwner;
    readonly host: RustyApplicationHostReadout | null;
    readonly runtime: ProductBrowserRuntimeReadout | null;
    readonly lastFailure: string | null;
}
export interface ProductBrowserHost {
    readonly kind: 'rusty.product.browser-host';
    readonly application: RustyApplicationHost;
    readonly transport: ProductBrowserRuntimeTransport;
    readonly readout: () => ProductBrowserHostReadout;
    readonly completeTimeline: (completion: ProductBrowserTimelineCompletion) => Promise<ProductBrowserTimelineCompletionResult>;
    readonly admitDemandStep: () => Promise<ProductBrowserRuntimeOperationResult>;
    readonly admitExternalStep: (step: string) => Promise<ProductBrowserRuntimeOperationResult>;
    readonly dispose: () => Promise<void>;
}
export declare class ProductBrowserHostError extends Error {
    readonly code: 'invalid_options' | 'startup_failed' | 'output_failed' | 'transport_failed' | 'timeline_unavailable' | 'disposed';
    constructor(code: ProductBrowserHostError['code'], message: string, options?: ErrorOptions);
}
type ProductBrowserJson = null | boolean | number | string | readonly ProductBrowserJson[] | {
    readonly [key: string]: ProductBrowserJson;
};
interface ProductBrowserOperationQueue {
    readonly enqueue: <T>(operation: () => Promise<T>) => Promise<T>;
    readonly settle: () => Promise<void>;
}
interface ProductBrowserAudioFeedbackReporter {
    readonly bindRuntime: (runtime: RustyApplicationRuntimeIdentity) => void;
    readonly flush: () => Promise<void>;
}
interface ProductBrowserAnimationFeedbackReporter {
    readonly bindRuntime: (runtime: RustyApplicationRuntimeIdentity) => void;
    readonly flush: () => Promise<void>;
}
interface ProductBrowserGhostPlateFeedbackReporter {
    readonly bindRuntime: (runtime: RustyApplicationRuntimeIdentity) => void;
    readonly flush: () => Promise<void>;
}
interface ProductBrowserRendererDiagnosticsReporter {
    readonly bindRuntime: (runtime: RustyApplicationRuntimeIdentity) => void;
    readonly flush: () => Promise<void>;
}
interface ProductBrowserRendererDiagnosticsCadenceSampler {
    readonly sample: (timeMs: number) => void;
    readonly settle: () => Promise<void>;
    readonly dispose: () => void;
}
/** This is the sole browser cadence observation that can be safely dropped. */
export declare function isDroppedClockRegression(result: ProductBrowserRuntimeOperationResult): boolean;
/** @internal Closed policy for atomic frame, view, and cue outputs. */
export declare function productBrowserAtomicReceiptMayContinue(outcome: 'applied' | 'rejected_atomic' | 'terminal'): boolean;
/** @internal Presentation can be partial because later domains already ran. */
export declare function productBrowserPresentationReceiptMayContinue(outcome: 'applied' | 'partial' | 'rejected_atomic' | 'terminal'): boolean;
/** @internal Closed coordinator used by the host; exported from this module for focused proof only. */
export declare function createProductBrowserAudioFeedbackReporter(options: {
    readonly renderer: Pick<RustyApplicationHost['renderer'], 'audioRealizedFacts' | 'acknowledgeAudioRealizedFacts' | 'resetAudioRealizationOwner'>;
    readonly report: ProductBrowserRuntimeTransport['reportAudioFeedback'];
    readonly initialRuntime?: RustyApplicationRuntimeIdentity;
}): ProductBrowserAudioFeedbackReporter;
/** @internal Fixed animation observation coordinator, intentionally parallel to audio. */
export declare function createProductBrowserAnimationFeedbackReporter(options: {
    readonly renderer: Pick<RustyApplicationHost['renderer'], 'animationRealizedFacts' | 'acknowledgeAnimationRealizedFacts' | 'resetAnimationRealizationOwner'>;
    readonly report: ProductBrowserRuntimeTransport['reportAnimationFeedback'];
    readonly initialRuntime?: RustyApplicationRuntimeIdentity;
}): ProductBrowserAnimationFeedbackReporter;
/** @internal Latest-state ghost realization reporter; it has no renderer command path. */
export declare function createProductBrowserGhostPlateFeedbackReporter(options: {
    readonly renderer: Pick<RustyApplicationHost['renderer'], 'ghostPlateReadout'>;
    readonly report: ProductBrowserRuntimeTransport['reportGhostPlateFeedback'];
    readonly initialRuntime?: RustyApplicationRuntimeIdentity;
}): ProductBrowserGhostPlateFeedbackReporter;
/** @internal Publishes one latest immutable renderer snapshot without scheduling renderer work. */
export declare function createProductBrowserRendererDiagnosticsReporter(options: {
    readonly renderer: Pick<RustyApplicationHost['renderer'], 'diagnosticsReadout'>;
    readonly report: NonNullable<ProductBrowserRuntimeTransport['reportRendererDiagnostics']>;
    readonly initialRuntime?: RustyApplicationRuntimeIdentity;
    readonly onObservation?: (renderSequence: number) => void;
    readonly productFrames?: () => ProductBrowserProductFrameObservationSample;
}): ProductBrowserRendererDiagnosticsReporter;
/** @internal Passive receipt/apply timing attached to existing renderer snapshots. */
export declare function createProductBrowserProductFrameObservation(now?: () => number): {
    readonly received: () => number;
    readonly applied: (receivedAtMs: number) => void;
    readonly sample: () => ProductBrowserProductFrameObservationSample;
};
/** @internal One batch boundary produces no more than one host-cadence wake. */
export declare function productBrowserOutputBatchNeedsRustHostPulse(outputs: readonly ProductBrowserRuntimeOutput[]): boolean;
/** @internal Applies stable browser health attributes without redundant writes. */
export declare function syncProductBrowserHealthDatasets(roots: readonly Pick<HTMLElement, 'dataset'>[], values: {
    readonly state: ProductBrowserHostReadout['state'];
    readonly mode: ProductBrowserRuntimeMode;
    readonly progress: string;
    readonly failure: string | null;
}, writeProgress: boolean): void;
/** @internal Coalesces diagnostics work from the existing renderer cadence without owning a loop. */
export declare function createProductBrowserRendererDiagnosticsCadenceSampler(options: {
    readonly enqueueOperation: ProductBrowserOperationQueue['enqueue'];
    readonly flush: () => Promise<void>;
    readonly onFailure: (cause: unknown) => void;
}): ProductBrowserRendererDiagnosticsCadenceSampler;
/** @internal Keeps the fixed feedback lane ahead of an operation that enters C# Update. */
export declare function flushProductBrowserAudioFeedbackBeforeUpdate<T>(flush: () => Promise<void>, update: () => Promise<T>): Promise<T>;
/** @internal Flushes both fixed renderer feedback families before C# update work. */
export declare function flushProductBrowserRendererFeedbackBeforeUpdate<T>(flush: () => Promise<void>, update: () => Promise<T>): Promise<T>;
/**
 * Mounts the one Engine-owned application composition root. The browser host
 * has no renderer implementation, product state, evaluator, or own cadence;
 * it drains the public input port from the application-host's existing
 * renderer cadence callback. Browser-owned realtime products also advance
 * from that callback; `realtimeAdvanceOwner: 'rust-host'` leaves advancement
 * to the packaged Rust host while subscribed outputs continue to drive the
 * retained presentation.
 */
export declare function mountProductBrowserHost(options: ProductBrowserHostOptions): Promise<ProductBrowserHost>;
/** @internal Focused composition seam for host recovery tests. */
export declare function mountProductBrowserHostWithApplication(options: ProductBrowserHostOptions, mountApplication: typeof mountRustyApplication): Promise<ProductBrowserHost>;
/** Fixed relative location of the complete Engine runtime closure. */
export declare const PRODUCT_BROWSER_BUNDLE_ENGINE_MODULE: "engine/product-browser-host.js";
export type ProductBrowserBundleAssetName = 'index.html' | 'main.js' | 'bridge.js' | typeof PRODUCT_BROWSER_BUNDLE_ENGINE_MODULE;
export interface ProductBrowserBundleAsset {
    readonly name: ProductBrowserBundleAssetName;
    readonly content: string;
}
export interface ProductBrowserBundleDescriptor {
    readonly artifact: 'rusty.product.bundle';
    readonly files: readonly {
        readonly name: ProductBrowserBundleAssetName;
        readonly content: string;
        readonly utf8Bytes: number;
    }[];
}
export interface ProductBrowserBundleTemplateOptions {
    /**
     * Exact built Engine-owned browser-host closure. The closure is copied into
     * the generated bundle instead of being resolved through a package manager
     * or a runtime import map. It must be ordinary JavaScript with no bare
     * package imports.
     */
    readonly engineHostModule: string;
    /** Product-relative compiled UI module, e.g. `./ui/main.js`. */
    readonly uiModule: string;
    /**
     * Product-relative generated Rust runtime route descriptor, e.g.
     * `./runtime-adapter.js`. It exports `PRODUCT_RUNTIME_HTTP_BASE_PATH`.
     */
    readonly runtimeAdapterModule: string;
    readonly lifecycleMode: ProductBrowserRuntimeMode;
    /** Defaults to `rust-host` for realtime bundles and `browser` otherwise. */
    readonly realtimeAdvanceOwner?: ProductBrowserRealtimeAdvanceOwner;
    readonly uiProjection?: {
        readonly expectedStream: string;
        readonly expectedContract: string;
    } | null;
}
/**
 * Deterministic fixed host assets copied by product build scripts into an
 * ignored generated bundle. Only source-linked module paths are
 * substituted; the HTML, main, bridge, and host topology remain Engine-owned.
 */
export declare function productBrowserBundleAssets(options: ProductBrowserBundleTemplateOptions): readonly ProductBrowserBundleAsset[];
/**
 * Returns the exact ordered byte descriptor used by the generator. The
 * descriptor intentionally contains no version counter: consumers can hash
 * each UTF-8 file and compare actual template changes when contracts evolve.
 */
export declare function productBrowserBundleDescriptor(options: ProductBrowserBundleTemplateOptions): ProductBrowserBundleDescriptor;
export {};
