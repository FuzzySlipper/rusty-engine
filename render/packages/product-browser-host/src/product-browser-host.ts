import {
  mountRustyApplication,
  type RustyApplicationFrame,
  type RustyApplicationAnimationCueDefinition,
  type RustyApplicationContent,
  type RustyApplicationHost,
  type RustyApplicationHostReadout,
  type RustyApplicationPresentationFrame,
  type RustyApplicationRendererOptions,
  type RustyApplicationRuntimeIdentity,
  type RustyApplicationRuntimeInputEnvelope,
  type RustyApplicationRuntimeInputOptions,
  type RustyApplicationUiMount,
  type RustyApplicationUiProjectionEnvelope,
  type RustyApplicationUiProjectionOptions,
  type RustyApplicationPresentationAspectBounds,
  type RustyApplicationViewComposition,
} from '@rusty-engine/application-host';
import { type RenderPublicationFrontier } from '@rusty-engine/render-contracts';
import { createProductBrowserCadence, type ProductBrowserCadence } from './realtime-cadence.js';

/** Fixed current artifact identity; compatibility follows actual code changes. */
export const PRODUCT_BROWSER_HOST_ARTIFACT = 'rusty.product.browser-host' as const;

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
export type ProductBrowserLifecycleOperation =
  | { readonly kind: 'start' }
  | { readonly kind: 'pause' }
  | { readonly kind: 'resume' }
  | { readonly kind: 'restart' }
  | { readonly kind: 'shutdown' }
  | { readonly kind: 'report-fault' };

export type ProductBrowserRuntimeOperationKind =
  | ProductBrowserLifecycleOperation['kind']
  | 'replace-control'
  | 'connect'
  | 'advance-realtime'
  | 'admit-demand-step'
  | 'admit-external-step';

/**
 * Closed Engine recovery posture for a completed local-host operation. The
 * accompanying stable code identifies the exact condition; callers must not
 * infer recovery policy from diagnostics.
 */
export type ProductBrowserHostFaultDisposition =
  | 'accepted'
  | 'rejected-recoverable'
  | 'degraded'
  | 'resync-required'
  | 'terminal';

/** Closed source-owned recovery facts for a rejected runtime result. */
export interface ProductBrowserRuntimeRecovery {
  readonly mutation: 'not-applied' | 'committed' | 'unknown';
  readonly invalidatedScope: 'none' | 'input' | 'outputs' | 'incarnation';
  readonly nextAction: 'continue' | 'rebaseline' | 'replace-incarnation';
}

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
  readonly recovery?: ProductBrowserRuntimeRecovery;
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
  readonly recovery?: ProductBrowserRuntimeRecovery;
  readonly diagnostic?: string;
}

/** Closed browser-to-runtime audio realization feedback; no browser objects cross this boundary. */
export type ProductBrowserAudioFeedbackFact =
  | {
      readonly kind: 'naturalCompletion';
      readonly source: 'oneShot';
      readonly factId: string;
      readonly sequence: number;
      readonly signalHandle: string;
    }
  | {
      readonly kind: 'naturalCompletion';
      readonly source: 'retainedVoice';
      readonly factId: string;
      readonly sequence: number;
      readonly voiceHandle: string;
    }
  | {
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
  readonly recovery?: ProductBrowserRuntimeRecovery;
  readonly diagnostic?: string;
}

/** Closed renderer-observation feedback; this is not an animation command route. */
export type ProductBrowserAnimationFeedbackFact =
  | { readonly kind: 'playbackObservation'; readonly factId: string; readonly objectId: string; readonly generation: string; readonly sequence: number; readonly status: string; readonly selectedClip: string | null; readonly sampledAtSeconds: number | null }
  | { readonly kind: 'naturalCompletion'; readonly factId: string; readonly objectId: string; readonly generation: string; readonly clip: string }
  | { readonly kind: 'diagnostic'; readonly factId: string; readonly objectId: string | null; readonly generation: string | null; readonly code: string; readonly sequence: number }
  | { readonly kind: 'cue'; readonly factId: string; readonly objectId: string; readonly generation: string; readonly cueId: string; readonly clip: string; readonly markerSeconds: number; readonly sampledAtSeconds: number; readonly signalDomain: 'audio' | 'particle'; readonly signalId: string }
  | { readonly kind: 'stopped'; readonly factId: string; readonly objectId: string; readonly generation: string; readonly sequence: number; readonly reason: 'destroyed' | 'teardown' };

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
  readonly recovery?: ProductBrowserRuntimeRecovery;
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
  readonly recovery?: ProductBrowserRuntimeRecovery;
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
  readonly recovery?: ProductBrowserRuntimeRecovery;
  readonly diagnostic?: string;
}

export interface ProductBrowserTimelineCompletion {
  /** Canonical decimal u64 ticket issued by runtime-timeline. */
  readonly ticket: string;
  readonly runtime: RustyApplicationRuntimeIdentity;
  readonly correlation: string;
  readonly outcome:
    | { readonly kind: 'success'; readonly data?: ProductBrowserJson }
    | { readonly kind: 'failure'; readonly data?: ProductBrowserJson };
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
  readonly recovery?: ProductBrowserRuntimeRecovery;
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
  /**
   * Active renderer stream frontiers captured when this binding's complete
   * retained baseline was committed. They seed the replacement projection
   * before any new-epoch trailing frame is allowed through.
   */
  readonly publicationFrontiers?: readonly RenderPublicationFrontier[];
}

export type ProductBrowserRuntimeOutput =
  | ProductBrowserRuntimeBindingOutput
  /** Fixed host evidence that one Rust-owned realtime advance was accepted. */
  | { readonly kind: 'runtime-progress'; readonly owner: 'rust-host' }
  /** Later Engine admission receipt for an input batch accepted by the Rust-host mailbox. */
  | {
      readonly kind: 'runtime-input-result';
      readonly result: ProductBrowserRuntimeInputResult;
    }
  | { readonly kind: 'frame'; readonly frame: RustyApplicationFrame }
  | { readonly kind: 'view-composition'; readonly composition: RustyApplicationViewComposition }
  | {
      readonly kind: 'animation-cue-definitions';
      readonly definitions: readonly RustyApplicationAnimationCueDefinition[];
    }
  | {
      readonly kind: 'presentation';
      readonly frame: RustyApplicationPresentationFrame;
    }
  | {
      readonly kind: 'ui-projection';
      readonly envelope: RustyApplicationUiProjectionEnvelope;
    }
  | { readonly kind: 'runtime-readout'; readonly readout: ProductBrowserRuntimeReadout };

/**
 * Buffers semantic runtime outputs while the renderer is mounting. Realtime
 * progress is only a liveness pulse and has no state to replay once the host
 * becomes ready; runtime readouts are snapshots, so only the newest one is
 * useful. Retained presentation outputs preserve their original ordering.
 *
 * @internal
 */
export function bufferProductBrowserPreMountOutput(
  pendingOutputs: ProductBrowserRuntimeOutput[],
  output: ProductBrowserRuntimeOutput,
  maximumPendingOutputs: number,
): boolean {
  if (output.kind === 'runtime-progress') return true;
  if (output.kind === 'runtime-readout'
    || output.kind === 'view-composition'
    || output.kind === 'animation-cue-definitions') {
    const previousSnapshot = pendingOutputs.findIndex((pending) => pending.kind === output.kind);
    if (previousSnapshot >= 0) {
      pendingOutputs[previousSnapshot] = output;
      return true;
    }
  }
  if (output.kind === 'ui-projection') {
    const previousProjection = pendingOutputs.findIndex((pending) =>
      pending.kind === 'ui-projection'
      && pending.envelope.stream === output.envelope.stream
      && pending.envelope.contract === output.envelope.contract);
    if (previousProjection >= 0) {
      pendingOutputs[previousProjection] = output;
      return true;
    }
  }
  if (pendingOutputs.length >= maximumPendingOutputs) return false;
  pendingOutputs.push(output);
  return true;
}

export type ProductBrowserRuntimeOutputListener = (
  output: ProductBrowserRuntimeOutput,
) => void;

export type ProductBrowserRuntimeOutputBatchListener = (
  outputs: readonly ProductBrowserRuntimeOutput[],
  metadata: ProductBrowserRuntimeOutputBatchMetadata,
) => void;

/**
 * Browser-local projection framing for one ordered output delivery. The epoch
 * is deliberately local to the attached EventSource; it is not a second
 * runtime identity or an ABI field. A recovery marker has no outputs and
 * keeps the host gated until the following complete baseline arrives.
 */
export interface ProductBrowserRuntimeOutputBatchMetadata {
  readonly epoch: number;
  readonly baseline: boolean;
  readonly recovery: 'none' | 'fresh-baseline-required';
}

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
export interface ProductBrowserAttachmentBaseline {
  readonly runtime: RustyApplicationRuntimeIdentity;
  readonly nextInputSequence: string;
  readonly publicationFrontiers: readonly RenderPublicationFrontier[];
}

export interface ProductBrowserAttachmentEvidence {
  readonly id: string;
  readonly replaces?: string;
  readonly baseline?: ProductBrowserAttachmentBaseline;
}

export interface ProductBrowserDiagnosticsReport {
  readonly attachment?: ProductBrowserAttachmentEvidence;
  readonly hostState: 'loading' | 'ready' | 'degraded' | 'failed' | 'disposed';
  readonly runtimeProgress: string;
  readonly transportState: 'open' | 'closed';
  readonly outputState: 'open' | 'closed';
  readonly lastRendererSequence?: string;
  readonly rendererObservationAgeMs?: string;
  readonly firstTerminal?: { readonly code: string; readonly message: string };
  /** One bounded, typed operation observation that was deliberately dropped. */
  readonly recoverableEvent?: {
    readonly code:
      | 'CSHARP_LIFECYCLE_CLOCK_REGRESSION'
      | 'BROWSER_RENDERER_DIAGNOSTICS_UNAVAILABLE'
      | 'BROWSER_LOCAL_REQUEST_UNAVAILABLE';
    readonly message: string;
  };
  readonly pageEvents: readonly { readonly kind: 'error' | 'unhandled-rejection'; readonly code: string; readonly message: string }[];
}

export interface ProductBrowserDiagnosticsResult {
  readonly accepted: boolean;
  readonly reported: number;
}

export type ProductBrowserRuntimeTerminalFailureListener = (
  failure: ProductBrowserRuntimeTerminalFailure,
) => void;

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
  readonly lifecycle: (
    operation: ProductBrowserLifecycleOperation,
  ) => Promise<ProductBrowserRuntimeOperationResult>;
  /** Advances only the current input control fence; it does not fault or restart the product. */
  readonly replaceControl?: (
    runtime: RustyApplicationRuntimeIdentity,
  ) => Promise<ProductBrowserRuntimeOperationResult>;
  readonly input: (
    batch: readonly RustyApplicationRuntimeInputEnvelope[],
  ) => Promise<ProductBrowserRuntimeInputResult>;
  readonly reportAudioFeedback: (
    feedback: ProductBrowserAudioFeedback,
  ) => Promise<ProductBrowserAudioFeedbackResult>;
  readonly reportAnimationFeedback: (
    feedback: ProductBrowserAnimationFeedback,
  ) => Promise<ProductBrowserAnimationFeedbackResult>;
  readonly reportGhostPlateFeedback: (
    feedback: ProductBrowserGhostPlateFeedback,
  ) => Promise<ProductBrowserGhostPlateFeedbackResult>;
  readonly reportRendererDiagnostics?: (
    feedback: ProductBrowserRendererDiagnosticsFeedback,
  ) => Promise<ProductBrowserRendererDiagnosticsFeedbackResult>;
  readonly reportBrowserDiagnostics?: (
    report: ProductBrowserDiagnosticsReport,
  ) => Promise<ProductBrowserDiagnosticsResult>;
  readonly advanceRealtime: (
    observedTimeNs: string,
  ) => Promise<ProductBrowserRuntimeOperationResult>;
  readonly admitDemandStep?: () => Promise<ProductBrowserRuntimeOperationResult>;
  readonly admitExternalStep?: (
    step: string,
  ) => Promise<ProductBrowserRuntimeOperationResult>;
  readonly completeTimeline?: (
    completion: ProductBrowserTimelineCompletion,
  ) => Promise<ProductBrowserTimelineCompletionResult>;
  readonly subscribeTerminalFailures?: (
    listener: ProductBrowserRuntimeTerminalFailureListener,
  ) => () => void;
  readonly subscribeOutputs: (
    listener: ProductBrowserRuntimeOutputListener,
  ) => () => void;
  /** One callback per ordered host receipt or complete connection baseline. */
  readonly subscribeOutputBatches?: (
    listener: ProductBrowserRuntimeOutputBatchListener,
  ) => () => void;
  /** Resolves once an asynchronous output subscription can receive runtime publications. */
  readonly waitUntilOutputSubscriptionReady?: () => Promise<void>;
  /** Reattach through the local transport's existing single-flight fresh-baseline path. */
  readonly recoverOutputProjection?: () => Promise<void>;
  /** Confirms physical installation, after the renderer's baseline tail settles. */
  readonly confirmOutputBaseline?: (epoch: number) => void;
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
  readonly recoverOutputProjection?: NonNullable<ProductBrowserRuntimeAdapter['recoverOutputProjection']>;
  readonly confirmOutputBaseline?: NonNullable<ProductBrowserRuntimeAdapter['confirmOutputBaseline']>;
  readonly dispose: ProductBrowserRuntimeAdapter['dispose'];
}

export function createProductBrowserRuntimeTransport(
  adapter: ProductBrowserRuntimeAdapter,
): ProductBrowserRuntimeTransport {
  if (adapter === null || typeof adapter !== 'object') {
    throw new TypeError('Product Browser Host runtime adapter must be an object');
  }
  requireFunction(adapter.lifecycle, 'lifecycle');
  if (adapter.connect !== undefined) {
    requireFunction(adapter.connect, 'connect');
  }
  if (adapter.replaceControl !== undefined) {
    requireFunction(adapter.replaceControl, 'replaceControl');
  }
  requireFunction(adapter.input, 'input');
  requireFunction(adapter.reportAudioFeedback, 'reportAudioFeedback');
  requireFunction(adapter.reportAnimationFeedback, 'reportAnimationFeedback');
  requireFunction(adapter.reportGhostPlateFeedback, 'reportGhostPlateFeedback');
  if (adapter.reportRendererDiagnostics !== undefined) {
    requireFunction(adapter.reportRendererDiagnostics, 'reportRendererDiagnostics');
  }
  if (adapter.reportBrowserDiagnostics !== undefined) {
    requireFunction(adapter.reportBrowserDiagnostics, 'reportBrowserDiagnostics');
  }
  requireFunction(adapter.advanceRealtime, 'advanceRealtime');
  if (adapter.admitDemandStep !== undefined) {
    requireFunction(adapter.admitDemandStep, 'admitDemandStep');
  }
  if (adapter.admitExternalStep !== undefined) {
    requireFunction(adapter.admitExternalStep, 'admitExternalStep');
  }
  if (adapter.completeTimeline !== undefined) {
    requireFunction(adapter.completeTimeline, 'completeTimeline');
  }
  if (adapter.subscribeTerminalFailures !== undefined) {
    requireFunction(adapter.subscribeTerminalFailures, 'subscribeTerminalFailures');
  }
  requireFunction(adapter.subscribeOutputs, 'subscribeOutputs');
  if (adapter.subscribeOutputBatches !== undefined) {
    requireFunction(adapter.subscribeOutputBatches, 'subscribeOutputBatches');
  }
  if (adapter.waitUntilOutputSubscriptionReady !== undefined) {
    requireFunction(adapter.waitUntilOutputSubscriptionReady, 'waitUntilOutputSubscriptionReady');
  }
  if (adapter.recoverOutputProjection !== undefined) {
    requireFunction(adapter.recoverOutputProjection, 'recoverOutputProjection');
  }
  requireFunction(adapter.dispose, 'dispose');
  return Object.freeze({
    ...(adapter.connect === undefined ? {} : { connect: adapter.connect }),
    lifecycle: adapter.lifecycle,
    ...(adapter.replaceControl === undefined ? {} : { replaceControl: adapter.replaceControl }),
    input: adapter.input,
    reportAudioFeedback: adapter.reportAudioFeedback,
    reportAnimationFeedback: adapter.reportAnimationFeedback,
    reportGhostPlateFeedback: adapter.reportGhostPlateFeedback,
    ...(adapter.reportRendererDiagnostics === undefined
      ? {}
      : { reportRendererDiagnostics: adapter.reportRendererDiagnostics }),
    ...(adapter.reportBrowserDiagnostics === undefined
      ? {}
      : { reportBrowserDiagnostics: adapter.reportBrowserDiagnostics }),
    advanceRealtime: adapter.advanceRealtime,
    ...(adapter.admitDemandStep === undefined ? {} : { admitDemandStep: adapter.admitDemandStep }),
    ...(adapter.admitExternalStep === undefined ? {} : { admitExternalStep: adapter.admitExternalStep }),
    ...(adapter.completeTimeline === undefined ? {} : { completeTimeline: adapter.completeTimeline }),
    ...(adapter.subscribeTerminalFailures === undefined
      ? {}
      : { subscribeTerminalFailures: adapter.subscribeTerminalFailures }),
    subscribeOutputs: adapter.subscribeOutputs,
    ...(adapter.subscribeOutputBatches === undefined
      ? {}
      : { subscribeOutputBatches: adapter.subscribeOutputBatches }),
    ...(adapter.waitUntilOutputSubscriptionReady === undefined
      ? {}
      : { waitUntilOutputSubscriptionReady: adapter.waitUntilOutputSubscriptionReady }),
    ...(adapter.recoverOutputProjection === undefined
      ? {}
      : { recoverOutputProjection: adapter.recoverOutputProjection }),
    ...(adapter.confirmOutputBaseline === undefined
      ? {}
      : { confirmOutputBaseline: adapter.confirmOutputBaseline }),
    dispose: adapter.dispose,
  });
}

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

const PRODUCT_BROWSER_INITIAL_RENDERER_FRAME_TIMEOUT_MS = 10_000;
const PRODUCT_BROWSER_RENDERER_DIAGNOSTICS_INTERVAL_MS = 750;

/** @internal Reports whether admitted animation bytes still need their first semantic frame. */
export function productBrowserInitialRendererFrameRequired(
  renderer: ProductBrowserHostOptions['renderer'],
): boolean {
  const content = renderer?.initialContent;
  if (content === undefined || !Array.isArray(content.resources)) return false;
  const hasAnimationPreload = content.resources.some((resource) =>
    /^(animated-mesh|clip-pack)-resource\//u.test(resource.identity));
  if (!hasAnimationPreload || !Array.isArray(content.frame['ops'])) return false;
  return !content.frame['ops'].some((operation) => typeof operation === 'object'
    && operation !== null
    && (operation as { readonly op?: unknown }).op === 'defineAnimatedMesh');
}

/** @internal Binds admitted preload bytes to one retained frame without mutating caller state. */
export function bindProductBrowserInitialRendererFrame(
  renderer: NonNullable<ProductBrowserHostOptions['renderer']>,
  frame: RustyApplicationFrame,
  publicationFrontiers?: readonly RenderPublicationFrontier[],
): NonNullable<ProductBrowserHostOptions['renderer']> {
  if (renderer.initialContent === undefined) {
    throw new ProductBrowserHostError(
      'invalid_options',
      'initial renderer frame binding requires admitted initial content',
    );
  }
  return Object.freeze({
    ...renderer,
    initialContent: Object.freeze({
      ...renderer.initialContent,
      frame,
      ...(publicationFrontiers === undefined ? {} : { publicationFrontiers }),
    }),
  });
}

/** @internal Folds one completed transport baseline into the mount frame. */
export function prepareProductBrowserInitialRendererBaseline(
  outputs: readonly ProductBrowserRuntimeOutput[],
  requiredFrame: RustyApplicationFrame,
  options: {
    /** `true` only for a transport batch marked as one complete baseline. */
    readonly complete: boolean;
    readonly publicationFrontiers?: readonly RenderPublicationFrontier[];
  } = { complete: false },
): {
  readonly frame: RustyApplicationFrame;
  readonly remainingOutputs: readonly ProductBrowserRuntimeOutput[];
  readonly publicationFrontiers: readonly RenderPublicationFrontier[];
} {
  const firstPublishedFrameIndex = outputs.findIndex((output) => output.kind === 'frame'
    && output.frame['publication'] !== undefined);
  const seedLimit = options.complete
    ? outputs.length
    : firstPublishedFrameIndex < 0 ? outputs.length : firstPublishedFrameIndex;
  const seedIndexes = new Set<number>();
  const seedFrames: RustyApplicationFrame[] = [];
  for (let index = 0; index < seedLimit; index += 1) {
    const output = outputs[index];
    if (output?.kind !== 'frame') continue;
    seedIndexes.add(index);
    seedFrames.push(output.frame);
  }
  if (!seedFrames.some((frame) => frame === requiredFrame)) {
    throw new ProductBrowserHostError(
      'startup_failed',
      'initial retained renderer frame was not preserved by the completed transport baseline',
    );
  }
  const seedOps = seedFrames.flatMap((frame) => [...(frame['ops'] as readonly unknown[])]);
  const seededDefinitions = new Set(seedOps.flatMap((operation) => {
    const signature = retainedDefinitionSignature(operation);
    return signature === null ? [] : [signature];
  }));
  const frame = Object.freeze({
    schemaVersion: 1,
    ops: Object.freeze(seedOps),
  }) as RustyApplicationFrame;
  const remainingOutputs = outputs.flatMap((output, index): ProductBrowserRuntimeOutput[] => {
    if (seedIndexes.has(index)) return [];
    if (output.kind !== 'frame') return [output];
    const originalOps = output.frame['ops'] as readonly unknown[];
    const ops = originalOps.filter((operation) => {
      const signature = retainedDefinitionSignature(operation);
      return signature === null || !seededDefinitions.has(signature);
    });
    if (ops.length === originalOps.length) return [output];
    const publication = output.frame['publication'];
    return [{
      ...output,
      frame: Object.freeze({
        ...output.frame,
        ops: Object.freeze(ops),
        ...(typeof publication === 'object' && publication !== null
          ? {
              publication: Object.freeze({
                ...publication,
                operationCount: ops.length,
              }),
            }
          : {}),
      }),
    }];
  });
  return Object.freeze({
    frame,
    remainingOutputs: Object.freeze(remainingOutputs),
    publicationFrontiers: Object.freeze([...(options.publicationFrontiers ?? [])]),
  });
}

function retainedDefinitionSignature(operation: unknown): string | null {
  if (typeof operation !== 'object' || operation === null) return null;
  const kind = (operation as { readonly op?: unknown }).op;
  return typeof kind === 'string' && kind.startsWith('define')
    ? JSON.stringify(operation)
    : null;
}

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
  readonly completeTimeline: (
    completion: ProductBrowserTimelineCompletion,
  ) => Promise<ProductBrowserTimelineCompletionResult>;
  readonly admitDemandStep: () => Promise<ProductBrowserRuntimeOperationResult>;
  readonly admitExternalStep: (
    step: string,
  ) => Promise<ProductBrowserRuntimeOperationResult>;
  readonly dispose: () => Promise<void>;
}

export class ProductBrowserHostError extends Error {
  readonly code:
    | 'invalid_options'
    | 'startup_failed'
    | 'output_failed'
    | 'transport_failed'
    | 'timeline_unavailable'
    | 'disposed';

  constructor(
    code: ProductBrowserHostError['code'],
    message: string,
    options?: ErrorOptions,
  ) {
    super(message, options);
    this.name = 'ProductBrowserHostError';
    this.code = code;
  }
}

type ProductBrowserJson =
  | null
  | boolean
  | number
  | string
  | readonly ProductBrowserJson[]
  | { readonly [key: string]: ProductBrowserJson };

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

function isRecoverableReportRejection(result: {
  readonly accepted: boolean;
  readonly disposition: ProductBrowserHostFaultDisposition;
}): boolean {
  return !result.accepted && result.disposition === 'rejected-recoverable';
}

/** This is the sole browser cadence observation that can be safely dropped. */
export function isDroppedClockRegression(result: ProductBrowserRuntimeOperationResult): boolean {
  return !result.accepted
    && result.disposition === 'rejected-recoverable'
    && result.code === 'CSHARP_LIFECYCLE_CLOCK_REGRESSION'
    && result.operation === 'advance-realtime';
}

/** @internal Closed policy for atomic frame, view, and cue outputs. */
export function productBrowserAtomicReceiptMayContinue(
  outcome: 'applied' | 'rejected_atomic' | 'terminal',
): boolean {
  return outcome !== 'terminal';
}

/** @internal Presentation can be partial because later domains already ran. */
export function productBrowserPresentationReceiptMayContinue(
  outcome: 'applied' | 'partial' | 'rejected_atomic' | 'terminal',
): boolean {
  return outcome !== 'terminal';
}

/** @internal Closed coordinator used by the host; exported from this module for focused proof only. */
export function createProductBrowserAudioFeedbackReporter(options: {
  readonly renderer: Pick<RustyApplicationHost['renderer'],
    'audioRealizedFacts' | 'acknowledgeAudioRealizedFacts' | 'resetAudioRealizationOwner'>;
  readonly report: ProductBrowserRuntimeTransport['reportAudioFeedback'];
  readonly initialRuntime?: RustyApplicationRuntimeIdentity;
}): ProductBrowserAudioFeedbackReporter {
  let currentBinding: RustyApplicationRuntimeIdentity | null = options.initialRuntime ?? null;
  // A fresh browser owner must replace any feedback retained by a prior page,
  // even when it rejoins with an identical runtime identity.
  let replaceOwnerPending = currentBinding !== null;
  let lastReportedEvictionCount = 0;

  const bindRuntime = (runtime: RustyApplicationRuntimeIdentity): void => {
    if (currentBinding === null || !sameRuntimeBinding(currentBinding, runtime)) {
      options.renderer.resetAudioRealizationOwner();
      replaceOwnerPending = true;
    }
    currentBinding = runtime;
  };

  const flush = async (): Promise<void> => {
    const binding = currentBinding;
    if (binding === null) return;
    const factsReadout = options.renderer.audioRealizedFacts();
    const realizedFacts = factsReadout?.facts ?? [];
    const facts = realizedFacts.map(snapshotAudioFeedbackFact);
    const evictedFactCount = factsReadout?.evictedFactCount ?? 0;
    if (!replaceOwnerPending
      && facts.length === 0
      && evictedFactCount === lastReportedEvictionCount) {
      return;
    }
    const submittedThroughFactId = realizedFacts.length === 0
      ? undefined
      : realizedFacts[realizedFacts.length - 1]!.factId;
    const result = await options.report(Object.freeze({
      runtime: binding,
      replaceOwner: replaceOwnerPending,
      evictedFactCount: canonicalSafeU64(evictedFactCount, 'audio feedback evictedFactCount'),
      facts: Object.freeze(facts),
    }));
    if (!sameRuntimeBinding(currentBinding, binding) || !sameRuntimeBinding(result.runtime, binding)) {
      throw new ProductBrowserHostError(
        'transport_failed',
        'audio feedback result did not match the current Product runtime binding',
      );
    }
    if (isRecoverableReportRejection(result)) return;
    if (!result.accepted) {
      throw new ProductBrowserHostError(
        'transport_failed',
        result.diagnostic ?? 'audio feedback was rejected by the runtime',
      );
    }
    if (result.diagnostic !== undefined) {
      throw new ProductBrowserHostError('transport_failed', 'accepted audio feedback cannot include a diagnostic');
    }
    const expectedThroughFactId = submittedThroughFactId === undefined
      ? undefined
      : canonicalSafeU64(submittedThroughFactId, 'audio feedback factId');
    if (result.acceptedThroughFactId !== expectedThroughFactId) {
      throw new ProductBrowserHostError(
        'transport_failed',
        'audio feedback acknowledgement boundary did not match the submitted facts',
      );
    }
    if (submittedThroughFactId !== undefined) {
      options.renderer.acknowledgeAudioRealizedFacts(submittedThroughFactId);
    }
    replaceOwnerPending = false;
    lastReportedEvictionCount = evictedFactCount;
  };

  return Object.freeze({ bindRuntime, flush });
}

/** @internal Fixed animation observation coordinator, intentionally parallel to audio. */
export function createProductBrowserAnimationFeedbackReporter(options: {
  readonly renderer: Pick<RustyApplicationHost['renderer'],
    'animationRealizedFacts' | 'acknowledgeAnimationRealizedFacts' | 'resetAnimationRealizationOwner'>;
  readonly report: ProductBrowserRuntimeTransport['reportAnimationFeedback'];
  readonly initialRuntime?: RustyApplicationRuntimeIdentity;
}): ProductBrowserAnimationFeedbackReporter {
  let currentBinding: RustyApplicationRuntimeIdentity | null = options.initialRuntime ?? null;
  let replaceOwnerPending = currentBinding !== null;
  let lastReportedEvictionCount = 0;
  const bindRuntime = (runtime: RustyApplicationRuntimeIdentity): void => {
    if (currentBinding === null || !sameRuntimeBinding(currentBinding, runtime)) {
      options.renderer.resetAnimationRealizationOwner();
      replaceOwnerPending = true;
    }
    currentBinding = runtime;
  };
  const flush = async (): Promise<void> => {
    const binding = currentBinding;
    if (binding === null) return;
    const readout = options.renderer.animationRealizedFacts();
    const realizedFacts = readout?.facts ?? [];
    const facts = realizedFacts.map(snapshotAnimationFeedbackFact);
    const evictedFactCount = readout?.evictedFactCount ?? 0;
    if (!replaceOwnerPending && facts.length === 0 && evictedFactCount === lastReportedEvictionCount) return;
    const submittedThrough = realizedFacts.at(-1)?.factId;
    const result = await options.report(Object.freeze({
      runtime: binding,
      replaceOwner: replaceOwnerPending,
      evictedFactCount: canonicalSafeU64(evictedFactCount, 'animation feedback evictedFactCount'),
      facts: Object.freeze(facts),
    }));
    if (!sameRuntimeBinding(currentBinding, binding) || !sameRuntimeBinding(result.runtime, binding)) {
      throw new ProductBrowserHostError('transport_failed', 'animation feedback result did not match the current Product runtime binding');
    }
    if (isRecoverableReportRejection(result)) return;
    if (!result.accepted) throw new ProductBrowserHostError('transport_failed', result.diagnostic ?? 'animation feedback was rejected by the runtime');
    if (result.diagnostic !== undefined) throw new ProductBrowserHostError('transport_failed', 'accepted animation feedback cannot include a diagnostic');
    const expectedThrough = submittedThrough === undefined ? undefined : canonicalSafeU64(submittedThrough, 'animation feedback factId');
    if (result.acceptedThroughFactId !== expectedThrough) {
      throw new ProductBrowserHostError('transport_failed', 'animation feedback acknowledgement boundary did not match submitted facts');
    }
    if (submittedThrough !== undefined) options.renderer.acknowledgeAnimationRealizedFacts(submittedThrough);
    replaceOwnerPending = false;
    lastReportedEvictionCount = evictedFactCount;
  };
  return Object.freeze({ bindRuntime, flush });
}

/** @internal Latest-state ghost realization reporter; it has no renderer command path. */
export function createProductBrowserGhostPlateFeedbackReporter(options: {
  readonly renderer: Pick<RustyApplicationHost['renderer'], 'ghostPlateReadout'>;
  readonly report: ProductBrowserRuntimeTransport['reportGhostPlateFeedback'];
  readonly initialRuntime?: RustyApplicationRuntimeIdentity;
}): ProductBrowserGhostPlateFeedbackReporter {
  let currentBinding: RustyApplicationRuntimeIdentity | null = options.initialRuntime ?? null;
  let replaceOwnerPending = currentBinding !== null;
  const bindRuntime = (runtime: RustyApplicationRuntimeIdentity): void => {
    if (currentBinding === null || !sameRuntimeBinding(currentBinding, runtime)) replaceOwnerPending = true;
    currentBinding = runtime;
  };
  const flush = async (): Promise<void> => {
    const binding = currentBinding;
    if (binding === null) return;
    const plates = options.renderer.ghostPlateReadout()?.plates ?? [];
    const facts: ProductBrowserGhostPlateFeedbackFact[] = plates.map((plate) => Object.freeze({
      presentation: canonicalSafeU64(Number(plate.handle), 'ghost plate presentation'),
      sourceMatches: plate.sourceMatch,
      currentSector: plate.currentSector,
      localAngularOffsetDegrees: plate.localAzimuthDegrees,
      fallbackActive: plate.fallbackActive,
      fallbackReason: plate.fallbackReason === 'prepared-source-unsupported'
        ? 'preparedSourceUnsupported'
        : plate.fallbackReason === null ? 'none' : 'realizationFailed',
      limitationMask: plate.limitationMask,
      preparationCpuMilliseconds: plate.preparationCpuMilliseconds,
      captureCpuSubmissionMilliseconds: plate.captureCpuSubmissionMilliseconds,
      retainedSectorCount: plate.retainedResourceCounts.sectors,
      retainedMeshCount: plate.retainedResourceCounts.meshes,
      retainedMaterialCount: plate.retainedResourceCounts.materials,
      retainedBorrowedTextureCount: plate.retainedResourceCounts.borrowedTextures,
    }));
    const result = await options.report(Object.freeze({
      runtime: binding,
      replaceOwner: replaceOwnerPending,
      facts: Object.freeze(facts),
    }));
    if (!sameRuntimeBinding(currentBinding, binding) || !sameRuntimeBinding(result.runtime, binding)) {
      throw new ProductBrowserHostError('transport_failed', 'ghost plate feedback result did not match the current Product runtime binding');
    }
    if (isRecoverableReportRejection(result)) return;
    if (!result.accepted || result.diagnostic !== undefined) {
      throw new ProductBrowserHostError('transport_failed', result.diagnostic ?? 'ghost plate feedback was rejected by the runtime');
    }
    replaceOwnerPending = false;
  };
  return Object.freeze({ bindRuntime, flush });
}

/** @internal Publishes one latest immutable renderer snapshot without scheduling renderer work. */
export function createProductBrowserRendererDiagnosticsReporter(options: {
  readonly renderer: Pick<RustyApplicationHost['renderer'], 'diagnosticsReadout'>;
  readonly report: NonNullable<ProductBrowserRuntimeTransport['reportRendererDiagnostics']>;
  readonly initialRuntime?: RustyApplicationRuntimeIdentity;
  readonly onObservation?: (renderSequence: number) => void;
  readonly productFrames?: () => ProductBrowserProductFrameObservationSample;
}): ProductBrowserRendererDiagnosticsReporter {
  let currentBinding: RustyApplicationRuntimeIdentity | null = options.initialRuntime ?? null;
  let lastRenderSequence: number | null = null;
  const bindRuntime = (runtime: RustyApplicationRuntimeIdentity): void => {
    if (currentBinding === null || !sameRuntimeBinding(currentBinding, runtime)) {
      lastRenderSequence = null;
    }
    currentBinding = runtime;
  };
  const flush = async (): Promise<void> => {
    const binding = currentBinding;
    if (binding === null) return;
    const rendererSnapshot = options.renderer.diagnosticsReadout();
    if (rendererSnapshot.submission.renderSequence === lastRenderSequence) return;
    const snapshot = options.productFrames === undefined
      ? rendererSnapshot
      : Object.freeze({ ...rendererSnapshot, productFrames: options.productFrames() });
    const result = await options.report(Object.freeze({ runtime: binding, snapshot }));
    if (!sameRuntimeBinding(currentBinding, binding) || !sameRuntimeBinding(result.runtime, binding)) {
      throw new ProductBrowserHostError('transport_failed', 'renderer diagnostics result did not match the current Product runtime binding');
    }
    if (isRecoverableReportRejection(result)) return;
    if (!result.accepted || result.diagnostic !== undefined) {
      throw new ProductBrowserHostError('transport_failed', result.diagnostic ?? 'renderer diagnostics were rejected by the runtime');
    }
    lastRenderSequence = snapshot.submission.renderSequence;
    options.onObservation?.(snapshot.submission.renderSequence);
  };
  return Object.freeze({ bindRuntime, flush });
}

const PRODUCT_BROWSER_FRAME_OBSERVATION_HISTORY_LIMIT = 256;

/** @internal Passive receipt/apply timing attached to existing renderer snapshots. */
export function createProductBrowserProductFrameObservation(
  now: () => number = () => globalThis.performance?.now() ?? Date.now(),
): {
  readonly received: () => number;
  readonly applied: (receivedAtMs: number) => void;
  readonly sample: () => ProductBrowserProductFrameObservationSample;
} {
  let receivedCount = 0;
  let appliedCount = 0;
  let firstReceivedAtMs: number | null = null;
  let lastReceivedAtMs: number | null = null;
  let lastAppliedAtMs: number | null = null;
  const receivedIntervals: number[] = [];
  const appliedIntervals: number[] = [];
  const applyLatency: number[] = [];
  const retain = (values: number[], value: number): void => {
    if (!Number.isFinite(value) || value < 0) return;
    values.push(value);
    if (values.length > PRODUCT_BROWSER_FRAME_OBSERVATION_HISTORY_LIMIT) values.shift();
  };
  const received = (): number => {
    const observedAtMs = now();
    receivedCount += 1;
    if (firstReceivedAtMs === null) firstReceivedAtMs = observedAtMs;
    if (lastReceivedAtMs !== null) retain(receivedIntervals, observedAtMs - lastReceivedAtMs);
    lastReceivedAtMs = observedAtMs;
    return observedAtMs;
  };
  const applied = (receivedAtMs: number): void => {
    const observedAtMs = now();
    appliedCount += 1;
    if (lastAppliedAtMs !== null) retain(appliedIntervals, observedAtMs - lastAppliedAtMs);
    retain(applyLatency, observedAtMs - receivedAtMs);
    lastAppliedAtMs = observedAtMs;
  };
  const sample = (): ProductBrowserProductFrameObservationSample => Object.freeze({
    schemaVersion: 1,
    observedAtMs: now(),
    receivedCount,
    appliedCount,
    firstReceivedAtMs,
    lastReceivedAtMs,
    lastAppliedAtMs,
    recentReceivedIntervalsMs: Object.freeze([...receivedIntervals]),
    recentAppliedIntervalsMs: Object.freeze([...appliedIntervals]),
    recentApplyLatencyMs: Object.freeze([...applyLatency]),
  });
  return Object.freeze({ received, applied, sample });
}

/** @internal One batch boundary produces no more than one host-cadence wake. */
export function productBrowserOutputBatchNeedsRustHostPulse(
  outputs: readonly ProductBrowserRuntimeOutput[],
): boolean {
  return outputs.some((output) => output.kind === 'runtime-progress' || output.kind === 'runtime-readout');
}

/** @internal Applies stable browser health attributes without redundant writes. */
export function syncProductBrowserHealthDatasets(
  roots: readonly Pick<HTMLElement, 'dataset'>[],
  values: {
    readonly state: ProductBrowserHostReadout['state'];
    readonly mode: ProductBrowserRuntimeMode;
    readonly progress: string;
    readonly failure: string | null;
  },
  writeProgress: boolean,
): void {
  for (const root of roots) {
    if (root.dataset['rustyProductHostState'] !== values.state) {
      root.dataset['rustyProductHostState'] = values.state;
    }
    if (root.dataset['rustyProductRuntimeMode'] !== values.mode) {
      root.dataset['rustyProductRuntimeMode'] = values.mode;
    }
    if (writeProgress && root.dataset['rustyProductRuntimeProgress'] !== values.progress) {
      root.dataset['rustyProductRuntimeProgress'] = values.progress;
    }
    if (values.failure === null) {
      if (root.dataset['rustyProductRuntimeFailure'] !== undefined) {
        delete root.dataset['rustyProductRuntimeFailure'];
      }
    } else if (root.dataset['rustyProductRuntimeFailure'] !== values.failure) {
      root.dataset['rustyProductRuntimeFailure'] = values.failure;
    }
  }
}

/** @internal Coalesces diagnostics work from the existing renderer cadence without owning a loop. */
export function createProductBrowserRendererDiagnosticsCadenceSampler(options: {
  readonly enqueueOperation: ProductBrowserOperationQueue['enqueue'];
  readonly flush: () => Promise<void>;
  readonly onFailure: (cause: unknown) => void;
}): ProductBrowserRendererDiagnosticsCadenceSampler {
  let lastSampledAtMs: number | null = null;
  let maximumObservedTimeMs = 0;
  let pendingCadenceTimeMs: number | null = null;
  let inFlight: Promise<void> | null = null;
  let disposed = false;

  const start = (timeMs: number): void => {
    lastSampledAtMs = timeMs;
    inFlight = options.enqueueOperation(async () => {
      if (disposed) return;
      await options.flush();
    }).then(
      () => finish(),
      (cause: unknown) => {
        options.onFailure(cause);
        finish();
      },
    );
  };

  const finish = (): void => {
    inFlight = null;
    const pendingTimeMs = pendingCadenceTimeMs;
    pendingCadenceTimeMs = null;
    if (disposed || pendingTimeMs === null || lastSampledAtMs === null) return;
    if (pendingTimeMs - lastSampledAtMs >= PRODUCT_BROWSER_RENDERER_DIAGNOSTICS_INTERVAL_MS) {
      start(pendingTimeMs);
    }
  };

  const sample = (timeMs: number): void => {
    if (disposed) return;
    const orderedTimeMs = Number.isFinite(timeMs) && timeMs >= 0 ? timeMs : 0;
    const monotonicTimeMs = Math.max(maximumObservedTimeMs, orderedTimeMs);
    maximumObservedTimeMs = monotonicTimeMs;
    if (lastSampledAtMs !== null
      && monotonicTimeMs - lastSampledAtMs < PRODUCT_BROWSER_RENDERER_DIAGNOSTICS_INTERVAL_MS) {
      return;
    }
    if (inFlight !== null) {
      pendingCadenceTimeMs = monotonicTimeMs;
      return;
    }
    start(monotonicTimeMs);
  };

  return Object.freeze({
    sample,
    settle: async (): Promise<void> => {
      while (inFlight !== null) await inFlight;
    },
    dispose: (): void => {
      disposed = true;
      pendingCadenceTimeMs = null;
    },
  });
}

/** @internal Keeps the fixed feedback lane ahead of an operation that enters C# Update. */
export async function flushProductBrowserAudioFeedbackBeforeUpdate<T>(
  flush: () => Promise<void>,
  update: () => Promise<T>,
): Promise<T> {
  await flush();
  return update();
}

/** @internal Flushes both fixed renderer feedback families before C# update work. */
export async function flushProductBrowserRendererFeedbackBeforeUpdate<T>(
  flush: () => Promise<void>, update: () => Promise<T>,
): Promise<T> {
  await flush();
  return update();
}

/**
 * Mounts the one Engine-owned application composition root. The browser host
 * has no renderer implementation, product state, evaluator, or own cadence;
 * it drains the public input port from the application-host's existing
 * renderer cadence callback. Browser-owned realtime products also advance
 * from that callback; `realtimeAdvanceOwner: 'rust-host'` leaves advancement
 * to the packaged Rust host while subscribed outputs continue to drive the
 * retained presentation.
 */
export async function mountProductBrowserHost(
  options: ProductBrowserHostOptions,
): Promise<ProductBrowserHost> {
  return mountProductBrowserHostWithApplication(options, mountRustyApplication);
}

/** @internal Focused composition seam for host recovery tests. */
export async function mountProductBrowserHostWithApplication(
  options: ProductBrowserHostOptions,
  mountApplication: typeof mountRustyApplication,
): Promise<ProductBrowserHost> {
  validateOptions(options);
  const realtimeAdvanceOwner = options.realtimeAdvanceOwner ?? 'browser';
  const transport = options.transport;
  const queue = createOperationQueue();
  let state: ProductBrowserHostReadout['state'] = 'starting';
  let runtimeReadout: ProductBrowserRuntimeReadout | null = null;
  let application: RustyApplicationHost | null = null;
  let unsubscribeOutputs: (() => void) | null = null;
  let unsubscribeTerminalFailures: (() => void) | null = null;
  let disposal: Promise<void> | null = null;
  let started = false;
  let failure: ProductBrowserHostError | null = null;
  // This remains separate from a terminal failure so a successful follow-up
  // can restore ready while diagnostics retain the first uncertain request.
  let recoveryFailure: ProductBrowserHostError | null = null;
  let recoveryDiagnosticReported = false;
  let currentInputBinding: RustyApplicationRuntimeIdentity | null = options.runtimeInput?.binding ?? null;
  let inputRecovery: {
    readonly uncertainBinding: RustyApplicationRuntimeIdentity;
    inFlight: boolean;
  } | null = null;
  // A retained-output replacement is distinct from input certainty: the
  // runtime keeps running, but browser projection must not admit a second
  // incremental frame until its fresh baseline has replaced the old one.
  let projectionRecovery: { readonly fromEpoch: number } | null = null;
  let pendingProjectionBaseline: {
    readonly epoch: number;
    readonly outputs: readonly ProductBrowserRuntimeOutput[];
  } | null = null;
  let selectedProjectionBaselineEpoch: number | null = null;
  let pendingProjectionIncrementals: {
    readonly epoch: number;
    readonly outputs: readonly ProductBrowserRuntimeOutput[];
  } | null = null;
  let acceptedProjectionEpoch = 0;
  let browserDiagnosticsReportInFlight = false;
  let pendingHealthTransition = false;
  let transportClosed = false;
  let runtimeProgress = 0;
  let lastRendererSequence: string | null = null;
  let lastRendererObservationAtMs: number | null = null;
  let lastDiagnosticsStatusKey: string | null = null;
  let terminalDiagnosticsReported = false;
  let recoverableClockDiagnosticPending = false;
  let recoverableClockDiagnosticReported = false;
  let rendererDiagnosticsFailure: string | null = null;
  let rendererDiagnosticsFailureReported = false;
  let lastProgressDomWriteAtMs = Number.NEGATIVE_INFINITY;
  const progressDomWriteIntervalMs = 250;
  let audioFeedbackReporter: ProductBrowserAudioFeedbackReporter | null = null;
  let animationFeedbackReporter: ProductBrowserAnimationFeedbackReporter | null = null;
  let ghostPlateFeedbackReporter: ProductBrowserGhostPlateFeedbackReporter | null = null;
  let rendererDiagnosticsReporter: ProductBrowserRendererDiagnosticsReporter | null = null;
  let rendererDiagnosticsCadenceSampler: ProductBrowserRendererDiagnosticsCadenceSampler | null = null;
  const productFrameObservation = createProductBrowserProductFrameObservation();
  // Renderer calls can be asynchronous (notably presentation realization),
  // while the retained runtime output port is synchronous. Keep their typed
  // realization order private to this host so a later frame cannot overtake a
  // teardown presentation from the same product callback.
  let rendererOutputTail: Promise<void> = Promise.resolve();
  let rendererProjectionEpoch = 0;
  const pendingOutputs: ProductBrowserRuntimeOutput[] = [];
  // A transport-marked connection baseline is a complete retained graph. Keep
  // its envelope while animated resources wait for their initial definitions;
  // an arbitrary binding is never treated as a replacement on its own.
  let pendingInitialRendererBaseline: {
    readonly epoch: number;
    readonly outputs: readonly ProductBrowserRuntimeOutput[];
    readonly publicationFrontiers: readonly RenderPublicationFrontier[];
  } | null = null;
  const maximumPendingOutputs = 64;
  const requiresInitialRendererFrame = productBrowserInitialRendererFrameRequired(options.renderer);
  let initialRendererFrameGate: Promise<
    | { readonly accepted: true; readonly frame: RustyApplicationFrame }
    | { readonly accepted: false; readonly error: ProductBrowserHostError }
  > | null = null;
  let settleInitialRendererFrame: ((result:
    | { readonly accepted: true; readonly frame: RustyApplicationFrame }
    | { readonly accepted: false; readonly error: ProductBrowserHostError }
  ) => void) | null = null;
  let initialRendererFrameTimeout: ReturnType<typeof setTimeout> | null = null;

  if (requiresInitialRendererFrame) {
    if (options.autoStart === false) {
      throw new ProductBrowserHostError(
        'invalid_options',
        'animation preloads without initial definitions require automatic runtime start',
      );
    }
    initialRendererFrameGate = new Promise((resolve) => {
      settleInitialRendererFrame = resolve;
    });
    initialRendererFrameTimeout = setTimeout(() => {
      const settle = settleInitialRendererFrame;
      settleInitialRendererFrame = null;
      initialRendererFrameTimeout = null;
      settle?.({
        accepted: false,
        error: new ProductBrowserHostError(
          'startup_failed',
          'runtime did not publish an initial retained frame for admitted animation resources',
        ),
      });
    }, PRODUCT_BROWSER_INITIAL_RENDERER_FRAME_TIMEOUT_MS);
  }

  const settleInitialRendererFrameFailure = (error: ProductBrowserHostError): void => {
    if (settleInitialRendererFrame === null) return;
    if (initialRendererFrameTimeout !== null) clearTimeout(initialRendererFrameTimeout);
    initialRendererFrameTimeout = null;
    const settle = settleInitialRendererFrame;
    settleInitialRendererFrame = null;
    settle({ accepted: false, error });
  };

  // These are deliberately small, product-neutral observation markers. They
  // let an outer host prove that a mounted runtime is still making accepted
  // progress without inspecting a product's UI, facts, or content vocabulary.
  const publishHealth = (
    reportToTransport = true,
    pageEvents: readonly { readonly kind: 'error' | 'unhandled-rejection'; readonly code: string; readonly message: string }[] = [],
    forceProgress = true,
  ): void => {
    const document = options.root.ownerDocument;
    const roots = [options.root, document.body].filter((root): root is HTMLElement => root !== null);
    const now = Date.now();
    const writeProgress = forceProgress || now - lastProgressDomWriteAtMs >= progressDomWriteIntervalMs;
    syncProductBrowserHealthDatasets(roots, {
      state,
      mode: options.lifecycleMode,
      progress: String(runtimeProgress),
      failure: (failure ?? recoveryFailure) === null ? null : boundedDiagnostic((failure ?? recoveryFailure)!.message),
    }, writeProgress);
    if (writeProgress) lastProgressDomWriteAtMs = now;
    if (!reportToTransport && pageEvents.length === 0) return;
    const terminal = failure === null
      ? undefined
      : Object.freeze({
        code: `BROWSER_HOST_${failure.code.toUpperCase()}`,
        message: boundedDiagnostic(failure.message),
      });
    const includeTerminal = terminal !== undefined && !terminalDiagnosticsReported;
    const hostState = state === 'starting' ? 'loading' : state;
    const statusKey = `${hostState}/${transportClosed ? 'closed' : 'open'}/${transportClosed ? 'closed' : 'open'}`;
    const recoverableEvent = recoverableClockDiagnosticPending && !recoverableClockDiagnosticReported
      ? Object.freeze({
          code: 'CSHARP_LIFECYCLE_CLOCK_REGRESSION' as const,
          message: 'dropped a regressing browser realtime observation; awaiting a later monotonic observation',
        })
      : rendererDiagnosticsFailure !== null && !rendererDiagnosticsFailureReported
        ? Object.freeze({
            code: 'BROWSER_RENDERER_DIAGNOSTICS_UNAVAILABLE' as const,
            message: rendererDiagnosticsFailure,
          })
        : recoveryFailure !== null && !recoveryDiagnosticReported
          ? Object.freeze({
              code: 'BROWSER_LOCAL_REQUEST_UNAVAILABLE' as const,
              message: boundedDiagnostic(recoveryFailure.message),
            })
        : undefined;
    const shouldReport = reportToTransport && transport.reportBrowserDiagnostics !== undefined
      && (includeTerminal || recoverableEvent !== undefined || pageEvents.length > 0 || statusKey !== lastDiagnosticsStatusKey);
    if (!shouldReport) return;
    if (browserDiagnosticsReportInFlight) {
      // Keep only one follow-up: every diagnostic fact is derived from the
      // current host state, while first terminal/recovery facts remain held
      // until an accepted report acknowledges them.
      pendingHealthTransition = true;
      return;
    }
    {
      const age = lastRendererObservationAtMs === null
        ? undefined
        : String(Math.max(0, now - lastRendererObservationAtMs));
      const report = Object.freeze({
        hostState,
        runtimeProgress: String(runtimeProgress),
        transportState: transportClosed ? 'closed' : 'open',
        outputState: transportClosed ? 'closed' : 'open',
        ...(lastRendererSequence === null ? {} : { lastRendererSequence }),
        ...(age === undefined ? {} : { rendererObservationAgeMs: age }),
        ...(includeTerminal ? { firstTerminal: terminal } : {}),
        ...(recoverableEvent === undefined ? {} : { recoverableEvent }),
        pageEvents: Object.freeze([...pageEvents]),
      });
      browserDiagnosticsReportInFlight = true;
      void transport.reportBrowserDiagnostics(report).then(
        () => {
          browserDiagnosticsReportInFlight = false;
          lastDiagnosticsStatusKey = statusKey;
          if (includeTerminal) terminalDiagnosticsReported = true;
          if (recoverableEvent?.code === 'CSHARP_LIFECYCLE_CLOCK_REGRESSION') {
            recoverableClockDiagnosticPending = false;
            recoverableClockDiagnosticReported = true;
          } else if (recoverableEvent?.code === 'BROWSER_RENDERER_DIAGNOSTICS_UNAVAILABLE') {
            rendererDiagnosticsFailureReported = true;
          }
          if (recoverableEvent?.code === 'BROWSER_LOCAL_REQUEST_UNAVAILABLE') {
            recoveryDiagnosticReported = true;
          }
          const flushPendingHealthTransition = pendingHealthTransition;
          pendingHealthTransition = false;
          if (flushPendingHealthTransition) publishHealth();
        },
        () => {
          browserDiagnosticsReportInFlight = false;
          const flushPendingHealthTransition = pendingHealthTransition;
          pendingHealthTransition = false;
          if (flushPendingHealthTransition) publishHealth();
        },
      );
    }
  };

  const requireApplication = (): RustyApplicationHost => {
    if (application === null || state === 'disposed') {
      throw new ProductBrowserHostError(
        'disposed',
        'Product Browser Host is disposed or has not mounted',
      );
    }
    return application;
  };

  const RECOVERY_GATE = Symbol('browser-host-recovery-gate');
  const recoveryGateError = (message: string): ProductBrowserHostError => new ProductBrowserHostError(
    'transport_failed',
    message,
    { cause: RECOVERY_GATE },
  );
  const isRecoveryGateError = (cause: unknown): boolean => cause instanceof ProductBrowserHostError
    && (cause as Error & { readonly cause?: unknown }).cause === RECOVERY_GATE;

  const requireReady = (): void => {
    if (inputRecovery !== null) {
      throw recoveryGateError('Product Browser Host is reconciling an uncertain input mutation');
    }
    if (projectionRecovery !== null) {
      throw recoveryGateError(
        'Product Browser Host is replacing an invalidated retained output projection',
      );
    }
    if (state === 'ready' || state === 'degraded') return;
    throw new ProductBrowserHostError(
      state === 'disposed' ? 'disposed' : 'transport_failed',
      state === 'failed'
        ? 'Product Browser Host has failed and its runtime transport is closed'
        : 'Product Browser Host is not ready',
    );
  };

  const reportFailure = (cause: unknown, code: ProductBrowserHostError['code']): ProductBrowserHostError => {
    const error = cause instanceof ProductBrowserHostError
      ? cause
      : new ProductBrowserHostError(
        code,
        cause instanceof Error ? cause.message : String(cause),
        cause instanceof Error ? { cause } : undefined,
      );
    if (failure === null) {
      failure = recoveryFailure ?? error;
      if (state !== 'disposed') state = 'failed';
      // The DOM remains current now; the typed terminal report waits until
      // closeTransport has recorded the durable closed/closed state.
      publishHealth(false);
    }
    return failure;
  };

  let cadence: ProductBrowserCadence | null = null;
  let removePageDiagnosticListeners: (() => void) | null = null;
  const closeTransport = (): void => {
    if (transportClosed) return;
    transportClosed = true;
    started = false;
    cadence?.dispose();
    rendererDiagnosticsCadenceSampler?.dispose();
    unsubscribeTerminalFailures?.();
    unsubscribeTerminalFailures = null;
    unsubscribeOutputs?.();
    unsubscribeOutputs = null;
    removePageDiagnosticListeners?.();
    removePageDiagnosticListeners = null;
    // This exact report route remains callable after disposal. Send after the
    // state transition so stopped-host diagnostics never claim open streams.
    publishHealth();
    void Promise.resolve(transport.dispose()).catch((cause: unknown) => {
      reportFailure(cause, 'transport_failed');
    });
  };
  const failAndClose = (
    cause: unknown,
    code: ProductBrowserHostError['code'],
  ): ProductBrowserHostError => {
    const error = reportFailure(cause, code);
    settleInitialRendererFrameFailure(error);
    closeTransport();
    return error;
  };

  const isUnknownLocalMutationFailure = (cause: unknown): cause is {
    readonly name: 'ProductBrowserLocalTransportError';
    readonly mutation: { readonly certainty: 'outcome-unknown' };
  } => typeof cause === 'object'
    && cause !== null
    && (cause as { readonly name?: unknown }).name === 'ProductBrowserLocalTransportError'
    && (cause as { readonly mutation?: { readonly certainty?: unknown } }).mutation?.certainty === 'outcome-unknown';

  const isFreshOutputRecoveryMutationFailure = (cause: unknown): boolean => typeof cause === 'object'
    && cause !== null
    && (cause as { readonly name?: unknown }).name === 'ProductBrowserLocalTransportError'
    && (cause as { readonly mutation?: { readonly outputRecovery?: unknown } }).mutation?.outputRecovery
      === 'fresh-baseline-required';

  const recoverOrClose = (
    cause: unknown,
    code: ProductBrowserHostError['code'],
  ): ProductBrowserHostError => {
    const error = cause instanceof ProductBrowserHostError
      ? cause
      : new ProductBrowserHostError(
        code,
        cause instanceof Error ? cause.message : String(cause),
        cause instanceof Error ? { cause } : undefined,
      );
    if (isFreshOutputRecoveryMutationFailure(cause)) {
      if (recoveryFailure === null) recoveryFailure = error;
      publishHealth();
      return error;
    }
    if (isUnknownLocalMutationFailure(cause) && (state === 'ready' || state === 'degraded')) {
      if (recoveryFailure === null) recoveryFailure = error;
      state = 'degraded';
      publishHealth();
      return error;
    }
    return failAndClose(cause, code);
  };

  const restoreReadyAfterHealthyTransport = (): void => {
    if (state !== 'degraded' || inputRecovery !== null || projectionRecovery !== null) return;
    state = 'ready';
    publishHealth();
  };

  const hasFreshRecoveryBinding = (
    candidate: RustyApplicationRuntimeIdentity,
    uncertain: RustyApplicationRuntimeIdentity,
  ): boolean => candidate.instanceId !== uncertain.instanceId
    || BigInt(candidate.generation) > BigInt(uncertain.generation)
    || (candidate.generation === uncertain.generation
      && BigInt(candidate.controlRevision) > BigInt(uncertain.controlRevision));

  const completeInputRecovery = (
    runtime: RustyApplicationRuntimeIdentity,
    nextInputSequence: string,
  ): boolean => {
    const pending = inputRecovery;
    if (pending === null || !hasFreshRecoveryBinding(runtime, pending.uncertainBinding)) return false;
    const host = requireApplication();
    audioFeedbackReporter?.bindRuntime(runtime);
    animationFeedbackReporter?.bindRuntime(runtime);
    ghostPlateFeedbackReporter?.bindRuntime(runtime);
    rendererDiagnosticsReporter?.bindRuntime(runtime);
    currentInputBinding = runtime;
    host.input?.rebaselineRuntime({
      runtime,
      context: options.inputContext ?? 'gameplay.default',
      nextSequence: nextInputSequence,
    });
    host.uiProjection?.bindRuntime(runtime);
    inputRecovery = null;
    restoreReadyAfterHealthyTransport();
    cadence?.pulseInput(globalThis.performance?.now() ?? Date.now());
    return true;
  };

  const requestInputRecovery = (): void => {
    const pending = inputRecovery;
    if (pending === null || pending.inFlight || state === 'failed' || state === 'disposed') return;
    if (transport.replaceControl === undefined) {
      failAndClose(new ProductBrowserHostError(
        'transport_failed',
        'runtime transport did not provide the required control-replace recovery fence',
      ), 'transport_failed');
      return;
    }
    pending.inFlight = true;
    void queue.enqueue(async () => {
      const current = inputRecovery;
      if (current === null) return;
      try {
        const result = await transport.replaceControl!(current.uncertainBinding);
        if (result.binding !== undefined && result.nextInputSequence !== undefined
          && completeInputRecovery(result.binding, result.nextInputSequence)) {
          if (result.readout !== undefined) runtimeReadout = result.readout;
          return;
        }
        if (!result.accepted
          && (result.disposition === 'rejected-recoverable' || result.disposition === 'resync-required')) {
          // Keep this single episode gated. A later physical observation can
          // request another fence, but the uncertain input is never resent.
          return;
        }
        throw new ProductBrowserHostError(
          'transport_failed',
          result.diagnostic ?? 'runtime control replacement did not provide a fresh input binding',
        );
      } finally {
        const active = inputRecovery;
        if (active !== null) active.inFlight = false;
      }
    }).catch((cause: unknown) => {
      // A no-response fence attempt remains inside this recovery episode. It
      // does not recursively retry, close the host, or enable new mutation.
      recoverOrClose(cause, 'transport_failed');
    });
  };

  const beginInputRecovery = (batch: readonly RustyApplicationRuntimeInputEnvelope[]): void => {
    const first = batch[0];
    if (first === undefined || inputRecovery !== null) return;
    inputRecovery = { uncertainBinding: first.runtime, inFlight: false };
    if (state !== 'disposed') {
      state = 'degraded';
      publishHealth();
    }
    requestInputRecovery();
  };

  const pageWindow = options.root.ownerDocument.defaultView;
  if (pageWindow !== null) {
    const reportPageEvent = (
      kind: 'error' | 'unhandled-rejection',
      code: string,
      message: string,
    ): void => {
      publishHealth(true, [Object.freeze({ kind, code, message: boundedDiagnostic(message) })]);
    };
    const onError = (event: ErrorEvent): void => {
      reportPageEvent('error', 'BROWSER_PAGE_ERROR', event.message || 'page error');
    };
    const onUnhandledRejection = (event: PromiseRejectionEvent): void => {
      const reason = event.reason;
      const message = reason instanceof Error
        ? reason.message
        : typeof reason === 'string' ? reason : 'unhandled promise rejection';
      reportPageEvent('unhandled-rejection', 'BROWSER_PAGE_UNHANDLED_REJECTION', message);
    };
    pageWindow.addEventListener('error', onError);
    pageWindow.addEventListener('unhandledrejection', onUnhandledRejection);
    removePageDiagnosticListeners = () => {
      pageWindow.removeEventListener('error', onError);
      pageWindow.removeEventListener('unhandledrejection', onUnhandledRejection);
    };
  }

  const flushRendererFeedback = async (): Promise<void> => {
    requireReady();
    await audioFeedbackReporter?.flush();
    await animationFeedbackReporter?.flush();
    await ghostPlateFeedbackReporter?.flush();
  };

  const scheduleRendererFeedbackFlush = (): void => {
    if (state !== 'ready') return;
    void queue.enqueue(flushRendererFeedback).catch((cause: unknown) => {
      if (isRecoveryGateError(cause)) return;
      recoverOrClose(cause, 'transport_failed');
    });
  };

  const enqueueRendererOutput = (
    apply: () => void | Promise<void>,
    projectionEpoch = rendererProjectionEpoch,
  ): void => {
    rendererOutputTail = rendererOutputTail.then(async () => {
      // Disposal unsubscribes the source before awaiting this tail, so work
      // already accepted by the host must still drain. A terminal renderer
      // failure is the only state that invalidates the remaining queue.
      if (state === 'failed' || projectionEpoch !== rendererProjectionEpoch) return;
      try {
        await apply();
      } catch (cause) {
        failAndClose(cause, 'output_failed');
      }
    });
  };

  const applyOutput = (output: ProductBrowserRuntimeOutput, outputEpoch = acceptedProjectionEpoch): void => {
    if (application === null) {
      if (!bufferProductBrowserPreMountOutput(pendingOutputs, output, maximumPendingOutputs)) {
        failAndClose(
          new ProductBrowserHostError(
            'output_failed',
            `runtime output buffer exceeded ${String(maximumPendingOutputs)} entries before host mount`,
          ),
          'output_failed',
        );
        return;
      }
      if (output.kind === 'frame' && settleInitialRendererFrame !== null) {
        if (initialRendererFrameTimeout !== null) clearTimeout(initialRendererFrameTimeout);
        initialRendererFrameTimeout = null;
        const settle = settleInitialRendererFrame;
        settleInitialRendererFrame = null;
        settle({ accepted: true, frame: output.frame });
      }
      return;
    }
    if (state === 'failed' || state === 'disposed') return;
    try {
      const host = requireApplication();
      switch (output.kind) {
        case 'binding':
          if (inputRecovery !== null) {
            // An old binding cannot release the gate, but the runtime's fresh
            // binding publication is authoritative even if the corresponding
            // control-replace HTTP response was lost after commit.
            if (hasFreshRecoveryBinding(output.runtime, inputRecovery.uncertainBinding)) {
              completeInputRecovery(output.runtime, output.nextInputSequence);
            }
            return;
          }
          audioFeedbackReporter?.bindRuntime(output.runtime);
          animationFeedbackReporter?.bindRuntime(output.runtime);
          ghostPlateFeedbackReporter?.bindRuntime(output.runtime);
          rendererDiagnosticsReporter?.bindRuntime(output.runtime);
          currentInputBinding = output.runtime;
          host.input?.bindRuntime({
            runtime: output.runtime,
            context: options.inputContext ?? 'gameplay.default',
            nextSequence: output.nextInputSequence,
          });
          host.uiProjection?.bindRuntime(output.runtime);
          return;
        case 'runtime-progress':
          if (options.lifecycleMode !== 'realtime' || realtimeAdvanceOwner !== 'rust-host') {
            throw new ProductBrowserHostError(
              'output_failed',
              'Rust-host realtime progress is unavailable for this Product Browser Host mode',
            );
          }
          if (output.owner !== 'rust-host') {
            throw new ProductBrowserHostError('output_failed', 'runtime progress owner was invalid');
          }
          if (started && state === 'ready') {
            runtimeProgress += 1;
            publishHealth(false, [], false);
          }
          return;
        case 'runtime-input-result':
          applyInputResult(output.result);
          return;
        case 'frame': {
          const receivedAtMs = productFrameObservation.received();
          enqueueRendererOutput(() => {
            const receipt = host.renderer.applyFrame(output.frame);
            if (receipt.outcome === 'rejected_atomic' && output.frame['publication'] !== undefined) {
              const diagnostic = receipt.diagnostics.map((entry) => entry.message).join('; ')
                || 'renderer rejected a published frame';
              requestPublishedProjectionRecovery(outputEpoch, diagnostic);
              return;
            }
            if (!productBrowserAtomicReceiptMayContinue(receipt.outcome)) {
              throw new ProductBrowserHostError(
                'output_failed',
                'renderer frame reported a terminal outcome',
              );
            }
            if (receipt.outcome === 'applied') productFrameObservation.applied(receivedAtMs);
          });
          return;
        }
        case 'view-composition': {
          enqueueRendererOutput(() => {
            const receipt = host.renderer.configureViews(output.composition);
            if (!productBrowserAtomicReceiptMayContinue(receipt.outcome)) {
              throw new ProductBrowserHostError(
                'output_failed',
                'renderer view composition reported a terminal outcome',
              );
            }
          });
          return;
        }
        case 'animation-cue-definitions': {
          enqueueRendererOutput(() => {
            const receipt = host.renderer.replaceAnimationCueDefinitions(output.definitions);
            if (!productBrowserAtomicReceiptMayContinue(receipt.outcome)) {
              throw new ProductBrowserHostError(
                'output_failed',
                'renderer animation cue definitions reported a terminal outcome',
              );
            }
          });
          return;
        }
        case 'presentation':
          enqueueRendererOutput(async () => {
            const receipt = await host.renderer.applyPresentation(output.frame);
            // `unavailableHost` is emitted only for a domain without a host.
            // It is an optional realization capability and does not invalidate
            // the retained presentation projection. Every other diagnostic is
            // from a configured domain that did not realize the publication.
            const configuredDiagnostics = receipt.diagnostics.filter((diagnostic) => (
              diagnostic.code !== 'unavailableHost'
            ));
            if (output.frame['publication'] !== undefined && configuredDiagnostics.length > 0) {
              const diagnostic = configuredDiagnostics.map((entry) => entry.message).join('; ')
                || 'renderer did not apply configured presentation';
              requestPublishedProjectionRecovery(outputEpoch, diagnostic);
              return;
            }
            if (!productBrowserPresentationReceiptMayContinue(receipt.outcome)) {
              // Preserve the existing terminal presentation posture, but give
              // the fixed audio-feedback lane one serialized attempt first so
              // a just-realized audio diagnostic reaches its C# readout.
              try {
                await queue.enqueue(flushRendererFeedback);
              } catch (cause) {
                if (isRecoveryGateError(cause)) return;
                recoverOrClose(cause, 'transport_failed');
                return;
              }
              throw new ProductBrowserHostError(
                'output_failed',
                'renderer presentation reported a terminal outcome',
              );
            }
            scheduleRendererFeedbackFlush();
          });
          return;
        case 'ui-projection':
          if (host.uiProjection === undefined) {
            throw new ProductBrowserHostError(
              'output_failed',
              'runtime emitted a UI projection but no projection contract was mounted',
            );
          }
          host.uiProjection.ingest(output.envelope);
          return;
        case 'runtime-readout':
          runtimeReadout = output.readout;
          return;
        default:
          assertNever(output);
      }
    } catch (cause) {
      failAndClose(cause, 'output_failed');
    }
  };

  const beginProjectionRecovery = (epoch: number): void => {
    if (projectionRecovery !== null && epoch <= projectionRecovery.fromEpoch) return;
    projectionRecovery = { fromEpoch: epoch };
    if (pendingProjectionBaseline !== null && pendingProjectionBaseline.epoch <= epoch) {
      pendingProjectionBaseline = null;
    }
    selectedProjectionBaselineEpoch = null;
    pendingProjectionIncrementals = null;
    // Work already queued from the discarded retained projection is never
    // allowed to reach the renderer after its fresh replacement arrives.
    rendererProjectionEpoch += 1;
    if (state === 'ready') state = 'degraded';
    publishHealth();
  };

  const requestPublishedProjectionRecovery = (
    epoch: number,
    diagnostic: string,
  ): void => {
    if (recoveryFailure === null) {
      recoveryFailure = new ProductBrowserHostError('output_failed', diagnostic);
    }
    if (projectionRecovery !== null && epoch <= projectionRecovery.fromEpoch) return;
    beginProjectionRecovery(epoch);
    if (transport.recoverOutputProjection === undefined) {
      failAndClose(new ProductBrowserHostError(
        'transport_failed',
        'runtime transport did not provide the required fresh output recovery',
      ), 'transport_failed');
      return;
    }
    void transport.recoverOutputProjection().catch((cause: unknown) => {
      recoverOrClose(cause, 'transport_failed');
    });
  };

  const applyProjectionBaseline = (
    outputs: readonly ProductBrowserRuntimeOutput[],
    epoch: number,
  ): void => {
    const pending = projectionRecovery;
    if (application === null) return;
    if (pending !== null && epoch <= pending.fromEpoch) return;
    if (pending === null && epoch <= acceptedProjectionEpoch) return;
    if (selectedProjectionBaselineEpoch !== null) {
      if (epoch <= selectedProjectionBaselineEpoch) return;
      // A newer retained replacement supersedes one that was selected but has
      // not become visible yet. Its queued renderer work cannot release this
      // gate, and only the new epoch's trailing output remains relevant.
      rendererProjectionEpoch += 1;
      pendingProjectionIncrementals = null;
    }
    selectedProjectionBaselineEpoch = epoch;
    const host = requireApplication();
    const frontierBindings = outputs.filter((output): output is ProductBrowserRuntimeBindingOutput => (
      output.kind === 'binding' && output.publicationFrontiers !== undefined
    ));
    if (frontierBindings.length > 1) {
      throw new ProductBrowserHostError(
        'output_failed',
        'recovered retained projection contained multiple publication frontier bindings',
      );
    }
    const publicationFrontiers = frontierBindings[0]?.publicationFrontiers ?? [];
    const frameOps = outputs.flatMap((output) => output.kind === 'frame'
      ? [...(output.frame['ops'] as readonly unknown[])]
      : []);
    const retainedFrame = Object.freeze({
      schemaVersion: 1,
      ops: Object.freeze(frameOps),
    }) as RustyApplicationFrame;
    const retainedOutputs = outputs.filter((output) => output.kind !== 'frame'
      && output.kind !== 'runtime-progress'
      && output.kind !== 'runtime-input-result');

    const replacementEpoch = rendererProjectionEpoch;
    // Rust attaches complete-baseline frontiers to its binding in one ordered
    // group. Its Frame outputs are ordered renderer diffs, not individually
    // replaceable scenes; preserving every operation in that order is the
    // existing complete-frame representation accepted by replaceFrame.
    rendererOutputTail = rendererOutputTail.then(async () => {
      if ((pending !== null && projectionRecovery?.fromEpoch !== pending.fromEpoch)
        || (pending === null && epoch <= acceptedProjectionEpoch)
        || rendererProjectionEpoch !== replacementEpoch
        || state === 'failed'
        || state === 'disposed') return;
      try {
        const receipt = await host.renderer.replaceFrame(retainedFrame, publicationFrontiers);
        // A normal incremental frame may continue after rejected_atomic, but
        // a recovery baseline is not installed until the replacement applied.
        if (receipt.outcome !== 'applied') {
          if (recoveryFailure === null) {
            recoveryFailure = new ProductBrowserHostError(
              'output_failed',
              'renderer did not apply the recovered retained projection',
            );
          }
          if (state === 'ready') state = 'degraded';
          publishHealth();
          return;
        }

        // Replacement is now visible. Recreate only the fixed realization
        // reporters and then switch the other retained facets in original
        // baseline order. The Rust snapshot omits expired one-shots, while
        // retained presentation state must be realized with the replacement.
        host.renderer.resetAudioRealizationOwner();
        host.renderer.resetAnimationRealizationOwner();
        audioFeedbackReporter = createProductBrowserAudioFeedbackReporter({
          renderer: host.renderer,
          report: transport.reportAudioFeedback,
        });
        animationFeedbackReporter = createProductBrowserAnimationFeedbackReporter({
          renderer: host.renderer,
          report: transport.reportAnimationFeedback,
        });
        ghostPlateFeedbackReporter = createProductBrowserGhostPlateFeedbackReporter({
          renderer: host.renderer,
          report: transport.reportGhostPlateFeedback,
        });
        for (const output of retainedOutputs) {
          if (output.kind === 'view-composition') {
            const viewReceipt = host.renderer.configureViews(output.composition);
            if (viewReceipt.outcome !== 'applied') {
              throw new ProductBrowserHostError('output_failed', 'renderer did not apply recovered view composition');
            }
          } else if (output.kind === 'animation-cue-definitions') {
            const cueReceipt = host.renderer.replaceAnimationCueDefinitions(output.definitions);
            if (cueReceipt.outcome !== 'applied') {
              throw new ProductBrowserHostError('output_failed', 'renderer did not apply recovered animation cues');
            }
          } else {
            applyOutput(output, epoch);
          }
        }
        if ((pending !== null && projectionRecovery?.fromEpoch !== pending.fromEpoch)
          || (pending === null && epoch <= acceptedProjectionEpoch)
          || rendererProjectionEpoch !== replacementEpoch
          || failure !== null
          || transportClosed) return;
        acceptedProjectionEpoch = epoch;
        if (pending !== null) projectionRecovery = null;
        selectedProjectionBaselineEpoch = null;
        const trailingOutputs = pendingProjectionIncrementals?.epoch === epoch
          ? pendingProjectionIncrementals.outputs
          : [];
        pendingProjectionIncrementals = null;
        // The retained replacement is physically installed before any normal
        // output accepted behind its CompleteBaseline. This preserves the
        // current epoch rather than dropping it during the asynchronous swap.
        for (const output of trailingOutputs) applyOutput(output, epoch);
        // Retained presentation can settle asynchronously behind the graphics
        // replacement. Report recovery only after that realization tail drains.
        enqueueRendererOutput(() => {
          transport.confirmOutputBaseline?.(epoch);
          lastDiagnosticsStatusKey = null;
          publishHealth();
        }, replacementEpoch);
        restoreReadyAfterHealthyTransport();
        cadence?.pulseInput(globalThis.performance?.now() ?? Date.now());
      } catch (cause) {
        if (recoveryFailure === null) {
          recoveryFailure = cause instanceof ProductBrowserHostError
            ? cause
            : new ProductBrowserHostError(
              'output_failed',
              cause instanceof Error ? cause.message : String(cause),
              cause instanceof Error ? { cause } : undefined,
            );
        }
        if (state === 'ready') state = 'degraded';
        publishHealth();
      }
    });
  };

  const applyOutputBatch = (
    outputs: readonly ProductBrowserRuntimeOutput[],
    metadata?: ProductBrowserRuntimeOutputBatchMetadata,
  ): void => {
    if (metadata?.recovery === 'fresh-baseline-required') {
      beginProjectionRecovery(metadata.epoch);
      return;
    }
    if (application === null && requiresInitialRendererFrame && metadata?.baseline === true) {
      const frontierBindings = outputs.filter((output): output is ProductBrowserRuntimeBindingOutput => (
        output.kind === 'binding' && output.publicationFrontiers !== undefined
      ));
      if (frontierBindings.length > 1) {
        failAndClose(new ProductBrowserHostError(
          'output_failed',
          'initial retained projection contained multiple publication frontier bindings',
        ), 'output_failed');
        return;
      }
      if (pendingInitialRendererBaseline === null || metadata.epoch > pendingInitialRendererBaseline.epoch) {
        pendingInitialRendererBaseline = Object.freeze({
          epoch: metadata.epoch,
          outputs: Object.freeze([...outputs]),
          publicationFrontiers: Object.freeze([...(frontierBindings[0]?.publicationFrontiers ?? [])]),
        });
      }
    }
    if (metadata !== undefined && metadata.epoch < acceptedProjectionEpoch) return;
    if (projectionRecovery !== null) {
      if (metadata?.baseline === true && metadata.epoch > projectionRecovery.fromEpoch) {
        if (application === null) {
          if (pendingProjectionBaseline === null || metadata.epoch > pendingProjectionBaseline.epoch) {
            pendingProjectionBaseline = Object.freeze({
              epoch: metadata.epoch,
              outputs: Object.freeze([...outputs]),
            });
            // A newer retained baseline makes any trailing output staged for
            // the older pre-mount projection irrelevant.
            pendingProjectionIncrementals = null;
          }
        } else {
          applyProjectionBaseline(outputs, metadata.epoch);
        }
      } else if (metadata !== undefined && (
        selectedProjectionBaselineEpoch === metadata.epoch
        || (application === null && pendingProjectionBaseline?.epoch === metadata.epoch)
      )) {
        const trailing = pendingProjectionIncrementals;
        pendingProjectionIncrementals = Object.freeze({
          epoch: metadata.epoch,
          outputs: Object.freeze([
            ...(trailing?.epoch === metadata.epoch ? trailing.outputs : []),
            ...outputs,
          ]),
        });
      }
      return;
    }
    // A normal fresh attachment also replaces asynchronously. Do not let a
    // same-epoch delta advance the observed frontier while that replacement is
    // still queued; it belongs immediately after the complete baseline.
    if (metadata !== undefined
      && metadata.baseline !== true
      && selectedProjectionBaselineEpoch === metadata.epoch) {
      const trailing = pendingProjectionIncrementals;
      pendingProjectionIncrementals = Object.freeze({
        epoch: metadata.epoch,
        outputs: Object.freeze([
          ...(trailing?.epoch === metadata.epoch ? trailing.outputs : []),
          ...outputs,
        ]),
      });
      return;
    }
    if (metadata?.baseline === true && application !== null) {
      applyProjectionBaseline(outputs, metadata.epoch);
      return;
    }
    if (metadata !== undefined) acceptedProjectionEpoch = Math.max(acceptedProjectionEpoch, metadata.epoch);
    for (const output of outputs) {
      applyOutput(output, metadata?.epoch ?? acceptedProjectionEpoch);
    }
    if (productBrowserOutputBatchNeedsRustHostPulse(outputs) && state !== 'failed' && state !== 'disposed') {
      cadence?.pulseRustHost();
    }
    if (state !== 'failed' && state !== 'disposed' && outputs.length > 0) {
      restoreReadyAfterHealthyTransport();
    }
  };

  const applyTerminalFailure = (terminalFailure: ProductBrowserRuntimeTerminalFailure): void => {
    const failure = normalizeTerminalFailure(terminalFailure);
    failAndClose(
      new ProductBrowserHostError('transport_failed', failure.diagnostic),
      'transport_failed',
    );
  };

  const applyOperationResult = (
    result: ProductBrowserRuntimeOperationResult,
    rejectedCode: ProductBrowserHostError['code'] = 'transport_failed',
    allowDroppedClockRegression = false,
  ): boolean => {
    if (allowDroppedClockRegression && isDroppedClockRegression(result)) {
      recoverableClockDiagnosticPending = true;
      publishHealth();
      return false;
    }
    const outputs: ProductBrowserRuntimeOutput[] = [];
    if (result.binding !== undefined && result.nextInputSequence !== undefined) {
      outputs.push({
        kind: 'binding',
        runtime: result.binding,
        nextInputSequence: result.nextInputSequence,
      });
    }
    if (result.readout !== undefined) outputs.push({ kind: 'runtime-readout', readout: result.readout });
    applyOutputBatch(outputs);
    if (!result.accepted) {
      // A typed recoverable or resync receipt is a completed operation. The
      // runtime may already have consumed lifecycle work, so never replay it
      // merely because its callback did not produce a normal output.
      if (result.disposition === 'rejected-recoverable' || result.disposition === 'resync-required') {
        restoreReadyAfterHealthyTransport();
        return false;
      }
      throw new ProductBrowserHostError(
        rejectedCode,
        result.diagnostic ?? `${result.operation} was rejected by the runtime`,
      );
    }
    restoreReadyAfterHealthyTransport();
    return true;
  };

  function applyInputResult(result: ProductBrowserRuntimeInputResult): void {
    if (inputRecovery !== null) {
      // An asynchronous mailbox result for the ambiguous batch is stale by
      // construction. Do not let it synchronize an old cursor or revive a
      // drained batch while the control fence is unresolved. Only the
      // acknowledged control-replace response is allowed to establish the
      // replacement cursor and trigger a physical-state baseline.
      return;
    }
    if (result.binding !== undefined && currentInputBinding !== null
      && !sameRuntimeBinding(result.binding, currentInputBinding)
      && !hasFreshRecoveryBinding(result.binding, currentInputBinding)) {
      // Delayed results from a superseded epoch are observations only; they
      // cannot rewind the browser input cursor after a later control fence.
      return;
    }
    const outputs: ProductBrowserRuntimeOutput[] = [];
    if (result.binding !== undefined && result.nextInputSequence !== undefined) {
      outputs.push({
        kind: 'binding',
        runtime: result.binding,
        nextInputSequence: result.nextInputSequence,
      });
    }
    if (result.readout !== undefined) outputs.push({ kind: 'runtime-readout', readout: result.readout });
    applyOutputBatch(outputs);
    if (!result.accepted) {
      if (result.disposition === 'rejected-recoverable' || result.disposition === 'resync-required') {
        restoreReadyAfterHealthyTransport();
        return;
      }
      throw new ProductBrowserHostError(
        'transport_failed',
        result.diagnostic ?? 'runtime input batch was rejected by the runtime',
      );
    }
    restoreReadyAfterHealthyTransport();
  }

  const sendInput = async (batch: readonly RustyApplicationRuntimeInputEnvelope[]): Promise<void> => {
    try {
      applyInputResult(await transport.input(batch));
    } catch (cause) {
      if (isUnknownLocalMutationFailure(cause)) beginInputRecovery(batch);
      throw cause;
    }
  };

  cadence = createProductBrowserCadence({
    lifecycleMode: options.lifecycleMode,
    realtimeAdvanceOwner,
    isReady: () => inputRecovery === null
      && projectionRecovery === null
      && started
      && (state === 'ready' || state === 'degraded'),
    enqueueOperation: queue.enqueue,
    sampleInput: () => {
      if (inputRecovery !== null) return [];
      const host = requireApplication();
      host.input?.sampleController();
      return host.input?.drain() ?? [];
    },
    sendInput,
    advanceRealtime: async (observedTimeNs) => {
      requireReady();
      const accepted = applyOperationResult(await flushProductBrowserRendererFeedbackBeforeUpdate(
        flushRendererFeedback,
        () => transport.advanceRealtime(observedTimeNs),
      ), 'transport_failed', true);
      if (accepted) {
        recoverableClockDiagnosticPending = false;
        recoverableClockDiagnosticReported = false;
      }
      if (accepted && options.lifecycleMode === 'realtime' && realtimeAdvanceOwner === 'browser') {
        runtimeProgress += 1;
        publishHealth();
      }
    },
    admitDemandStep: async () => {
      requireReady();
      if (transport.admitDemandStep === undefined) {
        throw new ProductBrowserHostError(
          'transport_failed',
          'this native product did not provide a demand-step transport lane',
        );
      }
      applyOperationResult(await flushProductBrowserRendererFeedbackBeforeUpdate(
        flushRendererFeedback,
        () => transport.admitDemandStep!(),
      ));
    },
    onFailure: (cause) => {
      if (isRecoveryGateError(cause)) return;
      recoverOrClose(cause, 'transport_failed');
    },
  });

  const observeRendererCadence = (timeMs: number): void => {
    cadence?.enqueue(timeMs);
    if (started && state === 'ready') rendererDiagnosticsCadenceSampler?.sample(timeMs);
  };

  let runtimeInput: RustyApplicationRuntimeInputOptions | undefined;
  if (options.runtimeInput !== undefined) {
    const { binding, ...runtimeInputOptions } = options.runtimeInput;
    runtimeInput = {
      ...runtimeInputOptions,
      onAvailable: () => {
        if (inputRecovery !== null) requestInputRecovery();
        else cadence?.pulseInput(globalThis.performance?.now() ?? Date.now());
      },
      ...(binding === undefined
        ? {}
        : {
            binding: {
              runtime: binding,
              context: options.inputContext ?? 'gameplay.default',
            },
          }),
    };
  }
  const projection = options.uiProjection === undefined
    ? undefined
    : ({ ...options.uiProjection } as RustyApplicationUiProjectionOptions);

  try {
    unsubscribeTerminalFailures = transport.subscribeTerminalFailures?.(applyTerminalFailure) ?? null;
    unsubscribeOutputs = transport.subscribeOutputBatches?.(applyOutputBatch)
      ?? transport.subscribeOutputs((output) => applyOutputBatch([output]));
    let renderer = options.renderer;
    let runtimeStartedBeforeMount = false;
    if (requiresInitialRendererFrame) {
      await transport.waitUntilOutputSubscriptionReady?.();
      if (failure !== null) throw failure;
      const result = await queue.enqueue(() => transport.connect?.()
        ?? transport.lifecycle({ kind: 'start' }));
      applyOperationResult(result, 'startup_failed');
      if (failure !== null) throw failure;
      const initialFrameGate = initialRendererFrameGate;
      if (initialFrameGate === null) {
        throw new ProductBrowserHostError('startup_failed', 'initial renderer frame gate was unavailable');
      }
      const initialFrameResult = await initialFrameGate;
      if (initialFrameResult.accepted === false) throw initialFrameResult.error;
      // Assignment happens in the subscribed output callback, which TypeScript
      // cannot model through local flow analysis.
      const completeBaseline = pendingInitialRendererBaseline as {
        readonly epoch: number;
        readonly outputs: readonly ProductBrowserRuntimeOutput[];
        readonly publicationFrontiers: readonly RenderPublicationFrontier[];
      } | null;
      const baseline = prepareProductBrowserInitialRendererBaseline(
        completeBaseline?.outputs ?? pendingOutputs,
        initialFrameResult.frame,
        completeBaseline === null
          ? undefined
          : {
              complete: true,
              publicationFrontiers: completeBaseline.publicationFrontiers,
            },
      );
      if (completeBaseline === null) {
        pendingOutputs.splice(0, pendingOutputs.length, ...baseline.remainingOutputs);
      } else {
        // Pre-mount buffering coalesces snapshots and drops liveness pulses,
        // so the original envelope length is not a prefix length here. Remove
        // only graphics frames consumed into the initial content. Keep the
        // buffer's non-frame state (including coalescing) and every later delta.
        const baselineFrames = new Set<ProductBrowserRuntimeOutput>(
          completeBaseline.outputs.filter((output) => output.kind === 'frame'),
        );
        const remainingOutputs = pendingOutputs.filter((output) => !baselineFrames.has(output));
        pendingOutputs.splice(
          0,
          pendingOutputs.length,
          ...remainingOutputs,
        );
      }
      renderer = bindProductBrowserInitialRendererFrame(
        options.renderer as NonNullable<ProductBrowserHostOptions['renderer']>,
        baseline.frame,
        baseline.publicationFrontiers,
      );
      runtimeStartedBeforeMount = true;
    }
    let stagedRendererContent: RustyApplicationContent | undefined;
    let rendererForMount = renderer;
    if (renderer?.initialContent !== undefined) {
      const { initialContent, ...remainingRendererOptions } = renderer;
      stagedRendererContent = initialContent;
      rendererForMount = remainingRendererOptions;
    }
    application = await mountApplication({
      root: options.root,
      mountUi: options.mountUi,
      ...(options.presentationAspectBounds === undefined
        ? {}
        : { presentationAspectBounds: options.presentationAspectBounds }),
      ...(options.initialInteractionMode === undefined
        ? {}
        : { initialInteractionMode: options.initialInteractionMode }),
      ...(options.loadingLabel === undefined ? {} : { loadingLabel: options.loadingLabel }),
      ...(options.failureLabel === undefined ? {} : { failureLabel: options.failureLabel }),
      ...(runtimeInput === undefined ? {} : { runtimeInput }),
      ...(projection === undefined ? {} : { uiProjection: projection }),
      ...(rendererForMount === undefined
        ? {
            renderer: {
              onCadence: observeRendererCadence,
            },
          }
        : {
            renderer: {
              ...rendererForMount,
              onCadence: observeRendererCadence,
            },
          }),
    });
    audioFeedbackReporter = createProductBrowserAudioFeedbackReporter({
      renderer: application.renderer,
      report: transport.reportAudioFeedback,
      ...(options.runtimeInput?.binding === undefined
        ? {}
        : { initialRuntime: options.runtimeInput.binding }),
    });
    animationFeedbackReporter = createProductBrowserAnimationFeedbackReporter({
      renderer: application.renderer,
      report: transport.reportAnimationFeedback,
      ...(options.runtimeInput?.binding === undefined
        ? {}
        : { initialRuntime: options.runtimeInput.binding }),
    });
    ghostPlateFeedbackReporter = createProductBrowserGhostPlateFeedbackReporter({
      renderer: application.renderer,
      report: transport.reportGhostPlateFeedback,
      ...(options.runtimeInput?.binding === undefined
        ? {}
        : { initialRuntime: options.runtimeInput.binding }),
    });
    if (transport.reportRendererDiagnostics !== undefined) {
      rendererDiagnosticsReporter = createProductBrowserRendererDiagnosticsReporter({
        renderer: application.renderer,
        report: transport.reportRendererDiagnostics,
        productFrames: productFrameObservation.sample,
        onObservation: (renderSequence) => {
          lastRendererSequence = String(renderSequence);
          lastRendererObservationAtMs = Date.now();
        },
        ...(options.runtimeInput?.binding === undefined
          ? {}
          : { initialRuntime: options.runtimeInput.binding }),
      });
      rendererDiagnosticsCadenceSampler = createProductBrowserRendererDiagnosticsCadenceSampler({
        enqueueOperation: queue.enqueue,
        flush: rendererDiagnosticsReporter.flush,
        onFailure: (cause) => {
          // Renderer diagnostics are an auxiliary observation lane. A failed
          // sample must never stop authoritative output, input, or lifecycle
          // work. Report the first failure as a bounded warning and let the
          // existing cadence retry later snapshots.
          if (rendererDiagnosticsFailureReported || rendererDiagnosticsFailure !== null) return;
          rendererDiagnosticsFailure = boundedDiagnostic(
            `renderer diagnostics reporting was temporarily unavailable: ${cause instanceof Error ? cause.message : String(cause)}`,
          );
          publishHealth();
        },
      });
    }
    if (stagedRendererContent !== undefined) {
      const content = stagedRendererContent;
      const mountedApplication = application;
      enqueueRendererOutput(async () => {
        const receipt = await mountedApplication.renderer.replaceContent(content);
        if (!receipt.applied) {
          throw new ProductBrowserHostError(
            'output_failed',
            `initial renderer content was rejected: ${receipt.diagnostics
              .map((diagnostic) => diagnostic.message)
              .join('; ')}`,
          );
        }
      });
    }
    const stagedProjectionBaseline = pendingProjectionBaseline as {
      readonly epoch: number;
      readonly outputs: readonly ProductBrowserRuntimeOutput[];
    } | null;
    pendingProjectionBaseline = null;
    if (stagedProjectionBaseline !== null) {
      applyProjectionBaseline(stagedProjectionBaseline.outputs, stagedProjectionBaseline.epoch);
    }
    const bufferedOutputs = pendingOutputs.splice(0, pendingOutputs.length);
    applyOutputBatch(bufferedOutputs);
    await rendererOutputTail;
    if (failure !== null) throw failure;
    transport.confirmOutputBaseline?.(acceptedProjectionEpoch);
    if (options.autoStart !== false && !runtimeStartedBeforeMount) {
      await transport.waitUntilOutputSubscriptionReady?.();
      if (failure !== null) throw failure;
      const result = await queue.enqueue(() => transport.connect?.()
        ?? transport.lifecycle({ kind: 'start' }));
      applyOperationResult(result, 'startup_failed');
      if (failure !== null) throw failure;
    }
    started = true;
    state = projectionRecovery === null ? 'ready' : 'degraded';
    publishHealth();
    if (state === 'ready') scheduleRendererFeedbackFlush();
  } catch (cause) {
    // Output delivery can fail and close the shared transport while the
    // lifecycle response is still in flight. Preserve that first concrete
    // failure instead of replacing it with the resulting aborted fetch.
    const error = failure ?? reportFailure(cause, 'startup_failed');
    settleInitialRendererFrameFailure(error);
    unsubscribeTerminalFailures?.();
    unsubscribeTerminalFailures = null;
    unsubscribeOutputs?.();
    unsubscribeOutputs = null;
    try {
      await transport.dispose();
    } catch {
      // Preserve the startup cause; disposal remains best effort on a failed mount.
    }
    try {
      await application?.dispose();
    } catch {
      // The application-host mount path already records its own cleanup diagnostics.
    }
    application = null;
    throw error;
  }

  const host = application;
  if (host === null) {
    throw new ProductBrowserHostError('startup_failed', 'application host did not mount');
  }

  const readout = (): ProductBrowserHostReadout => Object.freeze({
    artifact: PRODUCT_BROWSER_HOST_ARTIFACT,
    state,
    mode: options.lifecycleMode,
    realtimeAdvanceOwner,
    host: application?.readout() ?? null,
    runtime: runtimeReadout,
    lastFailure: (failure ?? recoveryFailure)?.message ?? null,
  });

  const completeTimeline = (
    completion: ProductBrowserTimelineCompletion,
  ): Promise<ProductBrowserTimelineCompletionResult> => {
    try { requireReady(); } catch (cause) { return Promise.reject(cause); }
    if (transport.completeTimeline === undefined) {
      return Promise.reject(new ProductBrowserHostError(
        'timeline_unavailable',
        'this native product did not provide a timeline completion lane',
      ));
    }
    return queue.enqueue(async () => {
      requireReady();
      const result = await transport.completeTimeline!(completion);
      if (result.readout !== undefined) applyOutputBatch([{ kind: 'runtime-readout', readout: result.readout }]);
      if (!result.accepted) {
        if (result.disposition === 'rejected-recoverable' || result.disposition === 'resync-required') {
          // Callback-entry failures carry the current ticket/binding/readout;
          // they are completed receipts, never permission to replay the
          // product callback from the browser.
          restoreReadyAfterHealthyTransport();
          return result;
        }
        throw new ProductBrowserHostError(
          'transport_failed',
          result.diagnostic ?? 'timeline completion was rejected by the runtime',
        );
      }
      restoreReadyAfterHealthyTransport();
      return result;
    }).catch((cause: unknown) => {
      if (isRecoveryGateError(cause)) throw cause;
      throw recoverOrClose(cause, 'transport_failed');
    });
  };

  const admitDemandStep = (): Promise<ProductBrowserRuntimeOperationResult> => {
    try { requireReady(); } catch (cause) { return Promise.reject(cause); }
    if (options.lifecycleMode !== 'demand') {
      return Promise.reject(new ProductBrowserHostError(
        'invalid_options',
        'admitDemandStep is only available for demand lifecycle products',
      ));
    }
    if (transport.admitDemandStep === undefined) {
      return Promise.reject(new ProductBrowserHostError(
        'transport_failed',
        'this native product did not provide a demand-step transport lane',
      ));
    }
    return queue.enqueue(async () => {
      requireReady();
      const host = requireApplication();
      host.input?.sampleController();
      const batch = host.input?.drain() ?? [];
      if (batch.length > 0) await sendInput(batch);
      const result = await flushProductBrowserRendererFeedbackBeforeUpdate(
        flushRendererFeedback,
        () => transport.admitDemandStep!(),
      );
      applyOperationResult(result);
      return result;
    }).catch((cause: unknown) => {
      if (isRecoveryGateError(cause)) throw cause;
      throw recoverOrClose(cause, 'transport_failed');
    });
  };

  const admitExternalStep = (step: string): Promise<ProductBrowserRuntimeOperationResult> => {
    try { requireReady(); } catch (cause) { return Promise.reject(cause); }
    if (options.lifecycleMode !== 'external') {
      return Promise.reject(new ProductBrowserHostError(
        'invalid_options',
        'admitExternalStep is only available for external lifecycle products',
      ));
    }
    if (transport.admitExternalStep === undefined) {
      return Promise.reject(new ProductBrowserHostError(
        'transport_failed',
        'this native product did not provide an external-step transport lane',
      ));
    }
    return queue.enqueue(async () => {
      requireReady();
      const host = requireApplication();
      host.input?.sampleController();
      const batch = host.input?.drain() ?? [];
      if (batch.length > 0) await sendInput(batch);
      const result = await flushProductBrowserRendererFeedbackBeforeUpdate(
        flushRendererFeedback,
        () => transport.admitExternalStep!(step),
      );
      applyOperationResult(result);
      return result;
    }).catch((cause: unknown) => {
      if (isRecoveryGateError(cause)) throw cause;
      throw recoverOrClose(cause, 'transport_failed');
    });
  };

  const dispose = (): Promise<void> => {
    if (disposal !== null) return disposal;
    disposal = (async () => {
      if (state === 'disposed') return;
      state = 'disposed';
      started = false;
      transportClosed = true;
      publishHealth();
      cadence?.dispose();
      rendererDiagnosticsCadenceSampler?.dispose();
      unsubscribeTerminalFailures?.();
      unsubscribeTerminalFailures = null;
      unsubscribeOutputs?.();
      unsubscribeOutputs = null;
      removePageDiagnosticListeners?.();
      removePageDiagnosticListeners = null;
      await queue.settle();
      await rendererOutputTail;
      const failures: unknown[] = [];
      try {
        await transport.dispose();
      } catch (cause) {
        failures.push(cause);
      }
      try {
        await host.dispose();
      } catch (cause) {
        failures.push(cause);
      }
      if (failures.length > 0) {
        throw new AggregateError(failures, 'Product Browser Host disposal failed');
      }
    })();
    return disposal;
  };

  return Object.freeze({
    kind: 'rusty.product.browser-host' as const,
    application: host,
    transport,
    readout,
    completeTimeline,
    admitDemandStep,
    admitExternalStep,
    dispose,
  });
}

const MAXIMUM_HEALTH_DIAGNOSTIC_BYTES = 512;

function snapshotAudioFeedbackFact(
  value: NonNullable<ReturnType<RustyApplicationHost['renderer']['audioRealizedFacts']>>['facts'][number],
): ProductBrowserAudioFeedbackFact {
  if (value.kind === 'naturalCompletion' && value.source === 'oneShot') {
    return Object.freeze({
      kind: 'naturalCompletion',
      source: 'oneShot',
      factId: canonicalSafeU64(value.factId, 'audio feedback factId'),
      sequence: requireAudioFeedbackSequence(value.sequence),
      signalHandle: canonicalSafeU64(value.signalHandle, 'audio feedback signalHandle'),
    });
  }
  if (value.kind === 'naturalCompletion' && value.source === 'retainedVoice') {
    return Object.freeze({
      kind: 'naturalCompletion',
      source: 'retainedVoice',
      factId: canonicalSafeU64(value.factId, 'audio feedback factId'),
      sequence: requireAudioFeedbackSequence(value.sequence),
      voiceHandle: canonicalSafeU64(value.handle, 'audio feedback voiceHandle'),
    });
  }
  return Object.freeze({
    kind: 'diagnostic',
    factId: canonicalSafeU64(value.factId, 'audio feedback factId'),
    code: value.diagnostic.code,
    sequence: requireAudioFeedbackSequence(value.diagnostic.sequence),
    voiceHandle: value.diagnostic.handle === null
      ? null
      : canonicalSafeU64(value.diagnostic.handle, 'audio feedback diagnostic voiceHandle'),
  });
}

function snapshotAnimationFeedbackFact(
  value: NonNullable<ReturnType<RustyApplicationHost['renderer']['animationRealizedFacts']>>['facts'][number],
): ProductBrowserAnimationFeedbackFact {
  if (value.kind === 'playbackObservation') return Object.freeze({
    kind: value.kind, factId: canonicalSafeU64(value.factId, 'animation feedback factId'),
    objectId: canonicalSafeU64(value.objectId, 'animation feedback objectId'),
    generation: canonicalSafeU64(value.generation, 'animation feedback generation'),
    sequence: requireAudioFeedbackSequence(value.sequence), status: value.status,
    selectedClip: value.selectedClip, sampledAtSeconds: value.sampledAtSeconds,
  });
  if (value.kind === 'naturalCompletion') return Object.freeze({
    kind: value.kind, factId: canonicalSafeU64(value.factId, 'animation feedback factId'),
    objectId: canonicalSafeU64(value.objectId, 'animation feedback objectId'),
    generation: canonicalSafeU64(value.generation, 'animation feedback generation'),
    clip: value.clip,
  });
  if (value.kind === 'diagnostic') return Object.freeze({
    kind: value.kind, factId: canonicalSafeU64(value.factId, 'animation feedback factId'),
    objectId: value.objectId === null ? null : canonicalSafeU64(value.objectId, 'animation feedback objectId'),
    generation: value.generation === null ? null : canonicalSafeU64(value.generation, 'animation feedback generation'),
    code: value.diagnostic.code, sequence: requireAudioFeedbackSequence(value.diagnostic.sequence),
  });
  if (value.kind === 'cue') return Object.freeze({
    kind: value.kind, factId: canonicalSafeU64(value.factId, 'animation feedback factId'),
    objectId: canonicalSafeU64(value.objectId, 'animation feedback objectId'),
    generation: canonicalSafeU64(value.generation, 'animation feedback generation'),
    cueId: value.cueId, clip: value.clip, markerSeconds: value.markerSeconds,
    sampledAtSeconds: value.sampledAtSeconds, signalDomain: value.signal.domain,
    signalId: value.signal.id,
  });
  return Object.freeze({
    kind: value.kind, factId: canonicalSafeU64(value.factId, 'animation feedback factId'),
    objectId: canonicalSafeU64(value.objectId, 'animation feedback objectId'),
    generation: canonicalSafeU64(value.generation, 'animation feedback generation'),
    sequence: requireAudioFeedbackSequence(value.sequence), reason: value.reason,
  });
}

function requireAudioFeedbackSequence(value: number): number {
  if (!Number.isSafeInteger(value) || value < 0 || value > 4_294_967_295) {
    throw new ProductBrowserHostError('transport_failed', 'renderer audio feedback sequence is outside u32 range');
  }
  return value;
}

function canonicalSafeU64(value: number, name: string): string {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new ProductBrowserHostError('transport_failed', `${name} is outside the safe u64 bridge range`);
  }
  return String(value);
}

function sameRuntimeBinding(
  left: RustyApplicationRuntimeIdentity | null,
  right: RustyApplicationRuntimeIdentity,
): boolean {
  return left !== null
    && left.instanceId === right.instanceId
    && left.generation === right.generation
    && left.controlRevision === right.controlRevision;
}

function boundedDiagnostic(value: string): string {
  let diagnostic = '';
  let bytes = 0;
  const encoder = new TextEncoder();
  for (const character of value) {
    const characterBytes = encoder.encode(character).byteLength;
    if (bytes + characterBytes > MAXIMUM_HEALTH_DIAGNOSTIC_BYTES) break;
    diagnostic += character;
    bytes += characterBytes;
  }
  return diagnostic;
}

function normalizeTerminalFailure(value: ProductBrowserRuntimeTerminalFailure): ProductBrowserRuntimeTerminalFailure {
  if (value === null || typeof value !== 'object') {
    return { kind: 'runtime-failure', diagnostic: 'runtime terminal failure was malformed' };
  }
  if (value.kind !== 'output-lag' && value.kind !== 'runtime-failure') {
    return { kind: 'runtime-failure', diagnostic: 'runtime terminal failure kind was invalid' };
  }
  if (typeof value.diagnostic !== 'string' || value.diagnostic.length === 0) {
    return { kind: 'runtime-failure', diagnostic: 'runtime terminal failure diagnostic was invalid' };
  }
  if (new TextEncoder().encode(value.diagnostic).byteLength > MAXIMUM_HEALTH_DIAGNOSTIC_BYTES) {
    return { kind: 'runtime-failure', diagnostic: 'runtime terminal failure diagnostic exceeded host bounds' };
  }
  return value;
}

function validateOptions(options: ProductBrowserHostOptions): void {
  if (options === null || typeof options !== 'object') {
    throw new ProductBrowserHostError('invalid_options', 'Product Browser Host options must be an object');
  }
  if (!(options.root instanceof HTMLElement)) {
    throw new ProductBrowserHostError('invalid_options', 'Product Browser Host root must be an HTMLElement');
  }
  if (options.root.childNodes.length > 0) {
    throw new ProductBrowserHostError('invalid_options', 'Product Browser Host root must be empty');
  }
  if (options.lifecycleMode !== 'realtime'
    && options.lifecycleMode !== 'demand'
    && options.lifecycleMode !== 'external') {
    throw new ProductBrowserHostError('invalid_options', 'Product Browser Host lifecycle mode is invalid');
  }
  if (options.realtimeAdvanceOwner !== undefined
    && options.realtimeAdvanceOwner !== 'browser'
    && options.realtimeAdvanceOwner !== 'rust-host') {
    throw new ProductBrowserHostError('invalid_options', 'Product Browser Host realtime advance owner is invalid');
  }
  if (options.realtimeAdvanceOwner === 'rust-host' && options.lifecycleMode !== 'realtime') {
    throw new ProductBrowserHostError(
      'invalid_options',
      'Product Browser Host rust-host realtime advance ownership requires realtime lifecycle mode',
    );
  }
  if (typeof options.mountUi !== 'function') {
    throw new ProductBrowserHostError('invalid_options', 'Product Browser Host mountUi must be a function');
  }
  if (options.uiProjection !== undefined && typeof options.uiProjection.expectedContract !== 'string') {
    throw new ProductBrowserHostError('invalid_options', 'Product Browser Host UI projection requires expectedContract');
  }
}

function requireFunction(value: unknown, name: string): void {
  if (typeof value !== 'function') throw new TypeError(`Product Browser Host adapter ${name} must be a function`);
}

function assertNever(value: never): never {
  throw new ProductBrowserHostError('output_failed', `unknown Product Browser Host output: ${String(value)}`);
}

function createOperationQueue(): ProductBrowserOperationQueue {
  let tail: Promise<void> = Promise.resolve();
  return {
    enqueue: <T>(operation: () => Promise<T>): Promise<T> => {
      const result = tail.then(operation, operation);
      tail = result.then(() => undefined, () => undefined);
      return result;
    },
    settle: () => tail,
  };
}

/** Fixed relative location of the complete Engine runtime closure. */
export const PRODUCT_BROWSER_BUNDLE_ENGINE_MODULE = 'engine/product-browser-host.js' as const;

export type ProductBrowserBundleAssetName =
  | 'index.html'
  | 'main.js'
  | 'bridge.js'
  | typeof PRODUCT_BROWSER_BUNDLE_ENGINE_MODULE;

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
export function productBrowserBundleAssets(
  options: ProductBrowserBundleTemplateOptions,
): readonly ProductBrowserBundleAsset[] {
  validateBundleEngineModule(options.engineHostModule);
  validateBundleModulePath(options.uiModule, 'uiModule');
  validateBundleModulePath(options.runtimeAdapterModule, 'runtimeAdapterModule');
  if (options.lifecycleMode !== 'realtime'
    && options.lifecycleMode !== 'demand'
    && options.lifecycleMode !== 'external') {
    throw new RangeError('lifecycleMode must be realtime, demand, or external');
  }
  if (options.realtimeAdvanceOwner !== undefined
    && options.realtimeAdvanceOwner !== 'browser'
    && options.realtimeAdvanceOwner !== 'rust-host') {
    throw new RangeError('realtimeAdvanceOwner must be browser or rust-host');
  }
  if (options.realtimeAdvanceOwner === 'rust-host' && options.lifecycleMode !== 'realtime') {
    throw new RangeError('rust-host realtimeAdvanceOwner requires realtime lifecycle mode');
  }
  if (options.uiProjection !== undefined && options.uiProjection !== null) {
    validateBundleIdentity(options.uiProjection.expectedStream, 'expectedStream');
    validateBundleIdentity(options.uiProjection.expectedContract, 'expectedContract');
  }
  return Object.freeze([
    Object.freeze({
      name: 'index.html' as const,
      content: '<!doctype html>\n<html lang="en">\n  <head>\n    <meta charset="UTF-8" />\n    <meta name="viewport" content="width=device-width, initial-scale=1.0" />\n    <title>Rusty Product</title>\n  </head>\n  <body>\n    <div id="application"></div>\n    <script type="module" src="./main.js"></script>\n  </body>\n</html>\n',
    }),
    Object.freeze({
      name: 'main.js' as const,
      content: [
        `import { loadProductBrowserRendererInitialContent, mountProductBrowserHost } from './${PRODUCT_BROWSER_BUNDLE_ENGINE_MODULE}';`,
        "import { createProductBridge } from './bridge.js';",
        `import { mountProductUi } from '${options.uiModule}';`,
        '',
        "const root = document.querySelector('#application');",
        "if (root === null) throw new Error('generated Product Browser Host root is missing');",
        'const bridge = createProductBridge();',
        'const rendererInitialContent = await loadProductBrowserRendererInitialContent(import.meta.url);',
        'const host = await mountProductBrowserHost({',
        '  root,',
        '  transport: bridge.transport,',
        '  lifecycleMode: bridge.lifecycleMode,',
        '  realtimeAdvanceOwner: bridge.realtimeAdvanceOwner,',
        "  initialInteractionMode: 'gameplay',",
        '  mountUi: mountProductUi,',
        '  uiProjection: bridge.uiProjection,',
        '  runtimeInput: bridge.runtimeInput,',
        '  renderer: { initialContent: rendererInitialContent },',
        '});',
        'void host;',
        '',
      ].join('\n'),
    }),
    Object.freeze({
      name: 'bridge.js' as const,
      content: [
        `import { createProductBrowserLocalHttpAdapter, createProductBrowserRuntimeTransport } from './${PRODUCT_BROWSER_BUNDLE_ENGINE_MODULE}';`,
        `import { PRODUCT_RUNTIME_HTTP_BASE_PATH } from '${options.runtimeAdapterModule}';`,
        '',
        'export function createProductBridge() {',
        '  const adapter = createProductBrowserLocalHttpAdapter({',
        '    basePath: PRODUCT_RUNTIME_HTTP_BASE_PATH,',
        '  });',
        '  return {',
        '    transport: createProductBrowserRuntimeTransport(adapter),',
        `    lifecycleMode: ${JSON.stringify(options.lifecycleMode)},`,
        `    realtimeAdvanceOwner: ${JSON.stringify(options.realtimeAdvanceOwner
          ?? (options.lifecycleMode === 'realtime' ? 'rust-host' : 'browser'))},`,
        ...(options.uiProjection === undefined || options.uiProjection === null
          ? ['    uiProjection: undefined,']
          : [`    uiProjection: ${JSON.stringify(options.uiProjection)},`]),
        '    runtimeInput: { maximumPointerDelta: 32, maximumWheelDelta: 64 },',
        '  };',
        '}',
        '',
      ].join('\n'),
    }),
    Object.freeze({
      name: PRODUCT_BROWSER_BUNDLE_ENGINE_MODULE,
      content: options.engineHostModule,
    }),
  ]);
}

/**
 * Returns the exact ordered byte descriptor used by the generator. The
 * descriptor intentionally contains no version counter: consumers can hash
 * each UTF-8 file and compare actual template changes when contracts evolve.
 */
export function productBrowserBundleDescriptor(
  options: ProductBrowserBundleTemplateOptions,
): ProductBrowserBundleDescriptor {
  const assets = productBrowserBundleAssets(options);
  const encoder = new TextEncoder();
  return Object.freeze({
    artifact: 'rusty.product.bundle' as const,
    files: Object.freeze(assets.map((asset) => Object.freeze({
      name: asset.name,
      content: asset.content,
      utf8Bytes: encoder.encode(asset.content).byteLength,
    }))),
  });
}

function validateBundleModulePath(value: string, field: string): void {
  if (typeof value !== 'string' || value.length === 0 || value.length > 256) {
    throw new RangeError(`${field} must be a non-empty relative module path`);
  }
  if (value.startsWith('/') || value.includes('\\') || value.includes(':') || value.includes('..')) {
    throw new RangeError(`${field} must not escape the generated Product Bundle`);
  }
  if (!value.startsWith('./')) {
    throw new RangeError(`${field} must start with ./`);
  }
}

function validateBundleEngineModule(value: string): void {
  if (typeof value !== 'string' || value.length === 0 || value.length > 16 * 1024 * 1024) {
    throw new RangeError('engineHostModule must be a bounded compiled JavaScript closure');
  }
  if (value.includes('\u0000')) {
    throw new RangeError('engineHostModule must not contain NUL bytes');
  }
  for (const line of value.split('\n')) {
    if (/^\s*(?:import|export)\b/u.test(line)
      && /['"]@rusty-engine\//u.test(line)) {
      throw new RangeError('engineHostModule must not contain bare Engine package imports');
    }
  }
}

function validateBundleIdentity(value: string, field: string): void {
  if (typeof value !== 'string' || value.length === 0 || value.length > 128
    || !/^[A-Za-z0-9][A-Za-z0-9._-]*$/u.test(value)) {
    throw new RangeError(`${field} must be a bounded product identity`);
  }
}
