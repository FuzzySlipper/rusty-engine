import { snapshotRustyApplicationProductPayloadJson } from '@rusty-engine/application-host';
import type {
  RustyApplicationFrame,
  RustyApplicationAnimationCueDefinition,
  RustyApplicationControllerAxis,
  RustyApplicationControllerButton,
  RustyApplicationInputClearReason,
  RustyApplicationKeyboardControl,
  RustyApplicationPresentationFrame,
  RustyApplicationPointerButton,
  RustyApplicationRuntimeIdentity,
  RustyApplicationRuntimeInputFact,
  RustyApplicationRuntimeInputEnvelope,
  RustyApplicationRuntimeIntentValue,
  RustyApplicationUiProjectionEnvelope,
} from '@rusty-engine/application-host';
import { validateRendererViewComposition, type RendererViewComposition } from '@rusty-engine/render-contracts';
import type {
  ProductBrowserLifecycleOperation,
  ProductBrowserAudioFeedback,
  ProductBrowserAudioFeedbackFact,
  ProductBrowserAudioFeedbackResult,
  ProductBrowserAnimationFeedback,
  ProductBrowserAnimationFeedbackFact,
  ProductBrowserAnimationFeedbackResult,
  ProductBrowserGhostPlateFeedback,
  ProductBrowserGhostPlateFeedbackFact,
  ProductBrowserGhostPlateFeedbackResult,
  ProductBrowserDiagnosticsReport,
  ProductBrowserDiagnosticsResult,
  ProductBrowserHostFaultDisposition,
  ProductBrowserRendererDiagnosticsFeedback,
  ProductBrowserRendererDiagnosticsFeedbackResult,
  ProductBrowserRuntimeAdapter,
  ProductBrowserRuntimeInputResult,
  ProductBrowserRuntimeOperationKind,
  ProductBrowserRuntimeOperationResult,
  ProductBrowserRuntimeOutput,
  ProductBrowserRuntimeReadout,
  ProductBrowserRuntimeTerminalFailure,
  ProductBrowserRuntimeTerminalFailureListener,
  ProductBrowserTimelineCompletion,
  ProductBrowserTimelineCompletionResult,
} from './product-browser-host.js';

/**
 * Fixed same-origin endpoint family for the generated local Product runtime.
 * The endpoint is deliberately an operation-specific route set, rather than
 * a method-name RPC endpoint or a generic message tunnel.
 */
export const PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH =
  '/__rusty/product/runtime/' as const;

/** Fixed identity for the Engine-owned browser-to-local-runtime transport. */
export const PRODUCT_BROWSER_LOCAL_TRANSPORT_ARTIFACT =
  'rusty.product.local-runtime-transport' as const;

const ROUTES = Object.freeze({
  lifecycle: Object.freeze({
    start: 'lifecycle/start',
    pause: 'lifecycle/pause',
    resume: 'lifecycle/resume',
    restart: 'lifecycle/restart',
    shutdown: 'lifecycle/shutdown',
    'report-fault': 'lifecycle/report-fault',
  }),
  input: 'input',
  advanceRealtime: 'advance-realtime',
  admitDemandStep: 'admit-demand-step',
  admitExternalStep: 'admit-external-step',
  completeTimeline: 'timeline-completion',
  audioFeedback: 'audio-feedback',
  animationFeedback: 'animation-feedback',
  ghostPlateFeedback: 'ghost-plate-feedback',
  rendererDiagnostics: 'renderer-diagnostics',
  browserDiagnostics: 'browser-diagnostics',
  outputs: 'outputs',
  freshOutputs: 'outputs/fresh',
});

const MAXIMUM_RUNTIME_RESPONSE_BYTES = 512 * 1024;
const MAXIMUM_RUNTIME_OUTPUT_EVENT_BYTES = 256 * 1024;
const MAXIMUM_RUNTIME_OUTPUT_BYTES = 16 * 1024 * 1024;
const MAXIMUM_RUNTIME_OUTPUT_FRAGMENT_DATA_BYTES = 96 * 1024;
// Mirrors ProductDevRendererDiagnosticsFeedback::MAX_SNAPSHOT_BYTES. Renderer
// observations are not direct UI product payloads and have their own closed,
// versioned transport budget.
const MAXIMUM_RENDERER_DIAGNOSTICS_SNAPSHOT_BYTES = 256 * 1024;
const MAXIMUM_RUNTIME_OUTPUT_FRAGMENTS = 256;
const MAXIMUM_CONNECTION_BASELINE_OUTPUTS = 256;
const DEFAULT_MAXIMUM_RESPONSE_BYTES = MAXIMUM_RUNTIME_RESPONSE_BYTES;
const DEFAULT_MAXIMUM_OUTPUT_BYTES = MAXIMUM_RUNTIME_OUTPUT_BYTES;
const MAXIMUM_CONFIGURED_BYTES = 16 * 1024 * 1024;
const UINT64_MAX_DECIMAL = '18446744073709551615';
const MAXIMUM_INPUT_BATCH_LENGTH = 1_024;
const MAXIMUM_AUDIO_FEEDBACK_FACTS = 128;
const MAXIMUM_ANIMATION_FEEDBACK_FACTS = 128;
const MAXIMUM_ANIMATION_CUE_DEFINITIONS = 128;
const MAXIMUM_ANIMATION_CUE_TEXT_BYTES = 96;
const MAXIMUM_JSON_DEPTH = 64;
const MAXIMUM_JSON_ARRAY_LENGTH = 1_024;
const MAXIMUM_JSON_OBJECT_KEYS = 256;
const MAXIMUM_JSON_STRING_BYTES = 64 * 1024;
// runtime-timeline::RuntimeOpaqueData owns a deliberately tighter opaque
// payload contract than the general projection JSON lane.
const MAXIMUM_TIMELINE_JSON_BYTES = 4_096;
const MAXIMUM_TIMELINE_JSON_DEPTH = 32;
const MAXIMUM_TIMELINE_JSON_NODES = 256;
const MAXIMUM_TIMELINE_JSON_ARRAY_LENGTH = 128;
const MAXIMUM_TIMELINE_JSON_OBJECT_KEYS = 128;
const KEYBOARD_CONTROLS = new Set<string>([
  ...Array.from({ length: 26 }, (_, index) => `key-${String.fromCharCode(97 + index)}`),
  ...Array.from({ length: 10 }, (_, index) => `digit-${String(index)}`),
  'space', 'enter', 'escape', 'shift-left', 'shift-right', 'control-left', 'control-right',
  'alt-left', 'alt-right',
]);
const POINTER_BUTTONS = new Set<string>(['primary', 'secondary', 'middle']);
const CONTROLLER_BUTTONS = new Set<string>(Array.from({ length: 16 }, (_, index) => `button-${String(index)}`));
const CONTROLLER_AXES = new Set<string>(Array.from({ length: 4 }, (_, index) => `axis-${String(index)}`));
const INPUT_EDGES = new Set<string>(['pressed', 'released']);
const INPUT_CLEAR_REASONS = new Set<string>([
  'focus-loss', 'ingress-overflow', 'interaction-mode-loss', 'pointer-lock-loss',
  'restart', 'control-revision-change', 'dispose',
]);
const AUDIO_DIAGNOSTIC_CODES = new Set<string>([
  'invalidDescriptor', 'assetMissing', 'assetKindMismatch', 'contentHashMismatch',
  'duplicateSignal', 'duplicateHandle', 'unknownHandle', 'unavailableHost',
  'audioContextBlocked', 'decodeFailed', 'hostFailure', 'invalidControl',
]);
const HOST_FAULT_DISPOSITIONS = new Set<string>([
  'accepted', 'rejected-recoverable', 'degraded', 'resync-required', 'terminal',
]);

interface ProductBrowserWireRecord {
  readonly [key: string]: unknown;
  readonly accepted?: unknown;
  readonly operation?: unknown;
  readonly binding?: unknown;
  readonly readout?: unknown;
  readonly diagnostic?: unknown;
  readonly count?: unknown;
  readonly ticket?: unknown;
  readonly kind?: unknown;
  readonly context?: unknown;
  readonly fact?: unknown;
  readonly intent?: unknown;
  readonly code?: unknown;
  readonly edge?: unknown;
  readonly button?: unknown;
  readonly x?: unknown;
  readonly y?: unknown;
  readonly axis?: unknown;
  readonly reason?: unknown;
  readonly active?: unknown;
  readonly outcome?: unknown;
  readonly provenance?: unknown;
  readonly data?: unknown;
  readonly detail?: unknown;
  readonly runtime?: unknown;
  readonly frame?: unknown;
  readonly composition?: unknown;
  readonly envelope?: unknown;
  readonly artifact?: unknown;
  readonly sequence?: unknown;
  readonly stream?: unknown;
  readonly contract?: unknown;
  readonly value?: unknown;
  readonly instanceId?: unknown;
  readonly generation?: unknown;
  readonly controlRevision?: unknown;
  readonly mode?: unknown;
  readonly state?: unknown;
  readonly fault?: unknown;
  readonly scaledRemainder?: unknown;
  readonly admittedSimulationSteps?: unknown;
  readonly admittedPresentations?: unknown;
  readonly droppedRealtimeSteps?: unknown;
  readonly clockRegressions?: unknown;
  readonly hostState?: unknown;
  readonly runtimeProgress?: unknown;
  readonly transportState?: unknown;
  readonly outputState?: unknown;
  readonly lastRendererSequence?: unknown;
  readonly rendererObservationAgeMs?: unknown;
  readonly firstTerminal?: unknown;
  readonly pageEvents?: unknown;
  readonly reported?: unknown;
  readonly lastObservedTimeNs?: unknown;
  readonly replaceOwner?: unknown;
  readonly evictedFactCount?: unknown;
  readonly facts?: unknown;
  readonly acceptedThroughFactId?: unknown;
  readonly source?: unknown;
  readonly signalHandle?: unknown;
  readonly voiceHandle?: unknown;
}

export type ProductBrowserLocalFetch = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>;

/** Minimal EventSource shape kept injectable for deterministic headless tests. */
export interface ProductBrowserLocalEventSource {
  onopen: ((event: unknown) => void) | null;
  onmessage: ((event: { readonly data: string; readonly lastEventId: string }) => void) | null;
  onerror: ((event: unknown) => void) | null;
  readonly addEventListener?: (
    type: 'rusty-output-lag' | 'rusty-output-fragment' | 'rusty-output-baseline',
    listener: (event: { readonly data: string; readonly lastEventId: string }) => void,
  ) => void;
  readonly removeEventListener?: (
    type: 'rusty-output-lag' | 'rusty-output-fragment' | 'rusty-output-baseline',
    listener: (event: { readonly data: string; readonly lastEventId: string }) => void,
  ) => void;
  readonly close: () => void;
}

export interface ProductBrowserLocalEventSourceConstructor {
  new (url: string): ProductBrowserLocalEventSource;
}

export interface ProductBrowserLocalTransportOptions {
  /** Same-origin absolute path; defaults to the fixed Engine route family. */
  readonly basePath?: string;
  /** Injectable only for tests or a host-owned fetch implementation. */
  readonly fetch?: ProductBrowserLocalFetch;
  /** Injectable only for headless tests. Browser builds use EventSource. */
  readonly eventSource?: ProductBrowserLocalEventSourceConstructor;
  readonly maximumResponseBytes?: number;
  readonly maximumOutputBytes?: number;
  /** Stream errors are surfaced here; the operation surface remains closed. */
  readonly onTransportError?: (error: ProductBrowserLocalTransportError) => void;
}

interface PendingOutputFragment {
  readonly transferId: string;
  readonly runtime: RustyApplicationRuntimeIdentity;
  readonly fragmentCount: number;
  readonly aggregateBytes: number;
  nextIndex: number;
  byteLength: number;
  readonly data: string[];
}

export type ProductBrowserLocalTransportErrorCode =
  | 'invalid_options'
  | 'disposed'
  | 'request_failed'
  | 'response_decode_failed'
  | 'output_decode_failed'
  | 'stream_failed';

export class ProductBrowserLocalTransportError extends Error {
  readonly code: ProductBrowserLocalTransportErrorCode;
  readonly route: string | null;

  constructor(
    code: ProductBrowserLocalTransportErrorCode,
    message: string,
    options?: ErrorOptions & { readonly route?: string },
  ) {
    super(message, options);
    this.name = 'ProductBrowserLocalTransportError';
    this.code = code;
    this.route = options?.route ?? null;
  }
}

type ProductBrowserCommitDisposition = 'committed' | 'resync-required';

function decodeCommitDisposition(
  headers: Headers,
  route: string,
): ProductBrowserCommitDisposition {
  const disposition = headers.get('x-rusty-commit-disposition');
  const resync = headers.get('x-rusty-resync-outputs');
  if (disposition === null) {
    if (resync === null) return 'committed';
    throw new ProductBrowserLocalTransportError(
      'response_decode_failed',
      `Product Browser local runtime response for ${route} named a resync without a commit disposition`,
      { route },
    );
  }
  if (disposition === 'committed' && resync === null) return disposition;
  if (disposition === 'resync-required' && resync === 'fresh') return disposition;
  throw new ProductBrowserLocalTransportError(
    'response_decode_failed',
    `Product Browser local runtime response for ${route} has an unknown or incoherent commit disposition`,
    { route },
  );
}

/**
 * Creates the Engine-owned local transport used by generated Product Bundles.
 * Rust serves the fixed operation routes and one bounded SSE output stream on
 * the same origin. The adapter only knows the typed route families below; it
 * cannot dispatch an arbitrary method or carry product state.
 */
export function createProductBrowserLocalHttpAdapter(
  options: ProductBrowserLocalTransportOptions = {},
): ProductBrowserRuntimeAdapter {
  const basePath = validateBasePath(options.basePath ?? PRODUCT_BROWSER_LOCAL_RUNTIME_BASE_PATH);
  const fetchImpl = options.fetch ?? resolveFetch();
  const eventSourceConstructor = options.eventSource ?? resolveEventSource();
  const maximumResponseBytes = validateMaximumBytes(
    options.maximumResponseBytes ?? DEFAULT_MAXIMUM_RESPONSE_BYTES,
    'maximumResponseBytes',
    MAXIMUM_RUNTIME_RESPONSE_BYTES,
  );
  const maximumOutputBytes = validateMaximumBytes(
    options.maximumOutputBytes ?? DEFAULT_MAXIMUM_OUTPUT_BYTES,
    'maximumOutputBytes',
    MAXIMUM_RUNTIME_OUTPUT_BYTES,
  );
  let disposed = false;
  let stream: ProductBrowserLocalEventSource | null = null;
  let streamLagListener: ((event: { readonly data: string; readonly lastEventId: string }) => void) | null = null;
  let streamFragmentListener: ((event: { readonly data: string; readonly lastEventId: string }) => void) | null = null;
  let streamBaselineListener: ((event: { readonly data: string; readonly lastEventId: string }) => void) | null = null;
  let pendingFragment: PendingOutputFragment | null = null;
  let currentOutputBinding: RustyApplicationRuntimeIdentity | null = null;
  let outputSubscriptionReady: Promise<void> | null = null;
  let resolveOutputSubscriptionReady: (() => void) | null = null;
  let connectionReady: Promise<ProductBrowserRuntimeOperationResult> | null = null;
  let resolveConnectionReady: ((result: ProductBrowserRuntimeOperationResult) => void) | null = null;
  let rejectConnectionReady: ((error: ProductBrowserLocalTransportError) => void) | null = null;
  let connectionBaselineComplete = false;
  let reattachingFreshBaseline = false;
  let pendingConnectionOutputs: ProductBrowserRuntimeOutput[] = [];
  let terminalFailure: ProductBrowserRuntimeTerminalFailure | null = null;
  let observedOutputSequence = 0n;
  const outputSequenceWaiters = new Set<{
    readonly through: bigint;
    readonly resolve: () => void;
  }>();
  const listeners = new Set<(output: ProductBrowserRuntimeOutput) => void>();
  const batchListeners = new Set<(outputs: readonly ProductBrowserRuntimeOutput[]) => void>();
  const terminalFailureListeners = new Set<ProductBrowserRuntimeTerminalFailureListener>();
  const abortController = new AbortController();

  const ensureOpen = (): void => {
    if (disposed) {
      throw new ProductBrowserLocalTransportError(
        'disposed',
        'Product Browser local runtime transport is disposed',
      );
    }
    if (terminalFailure !== null) {
      throw new ProductBrowserLocalTransportError(
        'stream_failed',
        terminalFailure.diagnostic,
        { route: ROUTES.outputs },
      );
    }
  };

  const reportTransportError = (
    error: ProductBrowserLocalTransportError,
  ): void => {
    try {
      options.onTransportError?.(error);
    } catch {
      // A diagnostic callback cannot become a second transport authority.
    }
  };

  const wakeOutputSequenceWaiters = (): void => {
    for (const waiter of [...outputSequenceWaiters]) {
      outputSequenceWaiters.delete(waiter);
      waiter.resolve();
    }
  };

  const settleOutputSequenceWaiters = (): void => {
    for (const waiter of [...outputSequenceWaiters]) {
      if (waiter.through > observedOutputSequence) continue;
      outputSequenceWaiters.delete(waiter);
      waiter.resolve();
    }
  };

  const observeOutputSequence = (value: string): void => {
    const sequence = decodeOutputSequence(value, 'output event id', 'output_decode_failed');
    if (sequence <= observedOutputSequence) {
      throw new ProductBrowserLocalTransportError(
        'output_decode_failed',
        'Product Browser local runtime output event ids must be strictly increasing',
        { route: ROUTES.outputs },
      );
    }
    observedOutputSequence = sequence;
    settleOutputSequenceWaiters();
  };

  const waitUntilOutputSequence = async (through: bigint): Promise<void> => {
    if (through <= observedOutputSequence) return;
    ensureOpen();
    if (stream === null) {
      throw new ProductBrowserLocalTransportError(
        'stream_failed',
        'Product Browser local runtime response named output that cannot be observed without an active subscription',
        { route: ROUTES.outputs },
      );
    }
    await new Promise<void>((resolve) => {
      outputSequenceWaiters.add({ through, resolve });
    });
    ensureOpen();
    if (through > observedOutputSequence) {
      throw new ProductBrowserLocalTransportError(
        'stream_failed',
        'Product Browser local runtime output subscription closed before the response boundary was observed',
        { route: ROUTES.outputs },
      );
    }
  };

  const reportTerminalFailure = (
    failure: ProductBrowserRuntimeTerminalFailure,
    error: ProductBrowserLocalTransportError,
  ): void => {
    if (terminalFailure !== null) return;
    terminalFailure = Object.freeze({ ...failure });
    if (stream !== null) {
      if (streamLagListener !== null) {
        stream.removeEventListener?.('rusty-output-lag', streamLagListener);
        streamLagListener = null;
      }
      if (streamFragmentListener !== null) {
        stream.removeEventListener?.('rusty-output-fragment', streamFragmentListener);
        streamFragmentListener = null;
      }
      if (streamBaselineListener !== null) {
        stream.removeEventListener?.('rusty-output-baseline', streamBaselineListener);
        streamBaselineListener = null;
      }
      stream.close();
      stream = null;
    }
    pendingFragment = null;
    pendingConnectionOutputs = [];
    wakeOutputSequenceWaiters();
    resolveOutputSubscriptionReady?.();
    resolveOutputSubscriptionReady = null;
    outputSubscriptionReady = null;
    rejectConnectionReady?.(error);
    connectionReady = null;
    resolveConnectionReady = null;
    rejectConnectionReady = null;
    reportTransportError(error);
    for (const listener of [...terminalFailureListeners]) {
      try {
        listener(terminalFailure);
      } catch (cause) {
        reportTransportError(new ProductBrowserLocalTransportError(
          'stream_failed',
          `Product Browser local runtime terminal-failure listener failed: ${cause instanceof Error ? cause.message : String(cause)}`,
          { cause, route: ROUTES.outputs },
        ));
      }
    }
  };

  const reconnectFreshOutputs = async (): Promise<void> => {
    if (stream === null || listeners.size === 0 || !connectionBaselineComplete) {
      throw new ProductBrowserLocalTransportError(
        'stream_failed',
        'Product Browser local runtime cannot resync a committed response without an established output subscription',
        { route: ROUTES.freshOutputs },
      );
    }
    if (streamLagListener !== null) stream.removeEventListener?.('rusty-output-lag', streamLagListener);
    if (streamFragmentListener !== null) stream.removeEventListener?.('rusty-output-fragment', streamFragmentListener);
    if (streamBaselineListener !== null) stream.removeEventListener?.('rusty-output-baseline', streamBaselineListener);
    stream.close();
    stream = null;
    streamLagListener = null;
    streamFragmentListener = null;
    streamBaselineListener = null;
    pendingFragment = null;
    pendingConnectionOutputs = [];
    currentOutputBinding = null;
    connectionBaselineComplete = false;
    reattachingFreshBaseline = false;
    observedOutputSequence = 0n;
    wakeOutputSequenceWaiters();
    outputSubscriptionReady = null;
    resolveOutputSubscriptionReady = null;
    connectionReady = null;
    resolveConnectionReady = null;
    rejectConnectionReady = null;

    // Reuse the one existing fresh-baseline path. The temporary listener only
    // starts the shared subscription; existing consumers remain registered,
    // and no operation request is retried.
    const unsubscribe = subscribeOutputs(() => undefined);
    const freshReady = connectionReady;
    unsubscribe();
    if (freshReady === null) {
      throw new ProductBrowserLocalTransportError(
        'stream_failed',
        'Product Browser local runtime did not establish a fresh output baseline',
        { route: ROUTES.freshOutputs },
      );
    }
    await freshReady;
    ensureOpen();
  };

  const post = async <T>(
    route: string,
    body: unknown,
    decode: (value: unknown) => T,
    allowAfterDispose = false,
  ): Promise<T> => {
    if (!allowAfterDispose) ensureOpen();
    const url = `${basePath}${route}`;
    const encodedBody = encodeRequestBody(body, maximumResponseBytes, route);
    let response: Response;
    try {
      response = await fetchImpl(url, {
        method: 'POST',
        credentials: 'same-origin',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        body: encodedBody,
        ...(allowAfterDispose ? {} : { signal: abortController.signal }),
      });
    } catch (cause) {
      throw new ProductBrowserLocalTransportError(
        'request_failed',
        `Product Browser local runtime request failed for ${route}: ${cause instanceof Error ? cause.message : String(cause)}`,
        { cause, route },
      );
    }
    const contentType = response.headers.get('content-type')?.toLowerCase() ?? '';
    if (!contentType.startsWith('application/json')) {
      throw new ProductBrowserLocalTransportError(
        'response_decode_failed',
        `Product Browser local runtime response for ${route} must use application/json`,
        { route },
      );
    }
    const text = await readResponseText(response, maximumResponseBytes, route);
    if (!response.ok) {
      throw new ProductBrowserLocalTransportError(
        'request_failed',
        `Product Browser local runtime rejected ${route} with HTTP ${String(response.status)}`,
        { route },
      );
    }
    let value: unknown;
    try {
      value = JSON.parse(text) as unknown;
    } catch (cause) {
      throw new ProductBrowserLocalTransportError(
        'response_decode_failed',
        `Product Browser local runtime returned invalid JSON for ${route}`,
        { cause, route },
      );
    }
    let decoded: T;
    try {
      if (!allowAfterDispose) ensureOpen();
      decoded = decode(value);
    } catch (cause) {
      if (cause instanceof ProductBrowserLocalTransportError) throw cause;
      throw new ProductBrowserLocalTransportError(
        'response_decode_failed',
        `Product Browser local runtime returned an invalid response for ${route}: ${cause instanceof Error ? cause.message : String(cause)}`,
        { cause, route },
      );
    }
    let commitDisposition: ProductBrowserCommitDisposition;
    try {
      commitDisposition = decodeCommitDisposition(response.headers, route);
    } catch (cause) {
      const error = cause instanceof ProductBrowserLocalTransportError
        ? cause
        : new ProductBrowserLocalTransportError(
          'response_decode_failed',
          `Product Browser local runtime returned an invalid commit disposition for ${route}`,
          { cause, route },
        );
      reportTerminalFailure({ kind: 'runtime-failure', diagnostic: error.message }, error);
      throw error;
    }
    const outputThroughHeader = response.headers.get('x-rusty-output-through');
    if (commitDisposition === 'resync-required') {
      try {
        await reconnectFreshOutputs();
      } catch (cause) {
        const error = cause instanceof ProductBrowserLocalTransportError
          ? cause
          : new ProductBrowserLocalTransportError(
            'stream_failed',
            `Product Browser local runtime fresh output resync failed for ${route}: ${cause instanceof Error ? cause.message : String(cause)}`,
            { cause, route: ROUTES.freshOutputs },
          );
        reportTerminalFailure({ kind: 'runtime-failure', diagnostic: error.message }, error);
        throw error;
      }
    } else if (outputThroughHeader !== null) {
      const outputThrough = decodeOutputSequence(
        outputThroughHeader,
        'X-Rusty-Output-Through response header',
        'response_decode_failed',
        route,
      );
      await waitUntilOutputSequence(outputThrough);
    }
    return decoded;
  };

  const lifecycle = (operation: ProductBrowserLifecycleOperation): Promise<ProductBrowserRuntimeOperationResult> =>
    post(ROUTES.lifecycle[operation.kind], {}, (value) => decodeOperationResult(value, operation.kind));

  const input = (
    batch: readonly RustyApplicationRuntimeInputEnvelope[],
  ): Promise<ProductBrowserRuntimeInputResult> =>
    post(ROUTES.input, { batch: snapshotInputBatch(batch) }, decodeInputResult);

  const reportAudioFeedback = (
    feedback: ProductBrowserAudioFeedback,
  ): Promise<ProductBrowserAudioFeedbackResult> => {
    const snapshot = snapshotAudioFeedback(feedback);
    return post(
      ROUTES.audioFeedback,
      snapshot,
      (value) => decodeAudioFeedbackResult(value, snapshot.runtime, snapshot.facts),
    );
  };

  const reportAnimationFeedback = (
    feedback: ProductBrowserAnimationFeedback,
  ): Promise<ProductBrowserAnimationFeedbackResult> => {
    const snapshot = snapshotAnimationFeedback(feedback);
    return post(
      ROUTES.animationFeedback,
      snapshot,
      (value) => decodeAnimationFeedbackResult(value, snapshot.runtime, snapshot.facts),
    );
  };

  const reportGhostPlateFeedback = (
    feedback: ProductBrowserGhostPlateFeedback,
  ): Promise<ProductBrowserGhostPlateFeedbackResult> => {
    const snapshot = snapshotGhostPlateFeedback(feedback);
    return post(
      ROUTES.ghostPlateFeedback,
      snapshot,
      (value) => decodeGhostPlateFeedbackResult(value, snapshot.runtime),
    );
  };

  const reportRendererDiagnostics = (
    feedback: ProductBrowserRendererDiagnosticsFeedback,
  ): Promise<ProductBrowserRendererDiagnosticsFeedbackResult> => {
    const snapshot = snapshotRendererDiagnosticsFeedback(feedback);
    return post(
      ROUTES.rendererDiagnostics,
      snapshot,
      (value) => decodeRendererDiagnosticsResult(value, snapshot.runtime),
    );
  };

  const reportBrowserDiagnostics = (
    report: ProductBrowserDiagnosticsReport,
  ): Promise<ProductBrowserDiagnosticsResult> => {
    const snapshot = snapshotBrowserDiagnosticsReport(report);
    // The first terminal host report must survive closing the SSE transport.
    // This exact route remains bounded and does not reopen the runtime API.
    return post(ROUTES.browserDiagnostics, snapshot, decodeBrowserDiagnosticsResult, true);
  };

  const advanceRealtime = (observedTimeNs: string): Promise<ProductBrowserRuntimeOperationResult> =>
    post(
      ROUTES.advanceRealtime,
      { observedTimeNs: requireU64Text(observedTimeNs, 'observedTimeNs') },
      (value) => decodeOperationResult(value, 'advance-realtime'),
    );

  const admitDemandStep = (): Promise<ProductBrowserRuntimeOperationResult> =>
    post(ROUTES.admitDemandStep, {}, (value) => decodeOperationResult(value, 'admit-demand-step'));

  const admitExternalStep = (step: string): Promise<ProductBrowserRuntimeOperationResult> =>
    post(
      ROUTES.admitExternalStep,
      { step: requireU64Text(step, 'step') },
      (value) => decodeOperationResult(value, 'admit-external-step'),
    );

  const completeTimeline = (
    completion: ProductBrowserTimelineCompletion,
  ): Promise<ProductBrowserTimelineCompletionResult> => {
    const snapshot = snapshotTimelineCompletion(completion);
    return post(
      ROUTES.completeTimeline,
      snapshot,
      (value) => decodeTimelineCompletionResult(value, snapshot.ticket),
    );
  };

  const publishOutputBatch = (outputs: readonly ProductBrowserRuntimeOutput[]): void => {
    const batch = Object.freeze([...outputs]);
    for (const output of batch) {
      if (output.kind === 'binding') currentOutputBinding = output.runtime;
    }
    for (const candidate of [...batchListeners]) {
      try {
        candidate(batch);
      } catch (cause) {
        reportTransportError(new ProductBrowserLocalTransportError(
          'stream_failed',
          `Product Browser local runtime output batch listener failed: ${cause instanceof Error ? cause.message : String(cause)}`,
          { cause, route: ROUTES.outputs },
        ));
      }
    }
    for (const output of batch) {
      for (const candidate of [...listeners]) {
        try {
          candidate(output);
        } catch (cause) {
          reportTransportError(new ProductBrowserLocalTransportError(
            'stream_failed',
            `Product Browser local runtime output listener failed: ${cause instanceof Error ? cause.message : String(cause)}`,
            { cause, route: ROUTES.outputs },
          ));
        }
      }
    }
  };

  const stageOrPublishOutputBatch = (outputs: readonly ProductBrowserRuntimeOutput[]): void => {
    if (connectionBaselineComplete && !reattachingFreshBaseline) {
      publishOutputBatch(outputs);
      return;
    }
    if (pendingConnectionOutputs.length + outputs.length > MAXIMUM_CONNECTION_BASELINE_OUTPUTS) {
      throw new ProductBrowserLocalTransportError(
        'output_decode_failed',
        `Product Browser local runtime connection baseline exceeds ${String(MAXIMUM_CONNECTION_BASELINE_OUTPUTS)} outputs`,
        { route: ROUTES.freshOutputs },
      );
    }
    for (const output of outputs) {
      if (output.kind === 'binding') currentOutputBinding = output.runtime;
      pendingConnectionOutputs.push(output);
    }
  };

  const failFragmentStream = (cause: unknown): void => {
    const error = cause instanceof ProductBrowserLocalTransportError
      ? cause
      : new ProductBrowserLocalTransportError(
        'output_decode_failed',
        `Product Browser local runtime emitted invalid output fragments: ${cause instanceof Error ? cause.message : String(cause)}`,
        { cause, route: ROUTES.outputs },
      );
    reportTerminalFailure({ kind: 'runtime-failure', diagnostic: error.message }, error);
  };

  const subscribeOutputs = (
    listener: (output: ProductBrowserRuntimeOutput) => void,
  ): (() => void) => {
    ensureOpen();
    if (typeof listener !== 'function') {
      throw new ProductBrowserLocalTransportError(
        'invalid_options',
        'Product Browser local runtime output listener must be a function',
      );
    }
    listeners.add(listener);
    if (stream === null) {
      try {
        outputSubscriptionReady = new Promise<void>((resolve) => {
          resolveOutputSubscriptionReady = resolve;
        });
        connectionReady = new Promise<ProductBrowserRuntimeOperationResult>((resolve, reject) => {
          resolveConnectionReady = resolve;
          rejectConnectionReady = reject;
        });
        // Output-only consumers still need terminal stream failures without being
        // forced to await connect(). Keep the shared promise observable for
        // connect callers while preventing an unhandled rejection otherwise.
        void connectionReady.catch(() => undefined);
        connectionBaselineComplete = false;
        pendingConnectionOutputs = [];
        stream = new eventSourceConstructor(`${basePath}${ROUTES.freshOutputs}`);
        stream.onopen = () => {
          resolveOutputSubscriptionReady?.();
          resolveOutputSubscriptionReady = null;
        };
        streamLagListener = (event) => {
          try {
            const failure = decodeOutputLagEvent(event.data, maximumOutputBytes);
            reportTerminalFailure(
              failure,
              new ProductBrowserLocalTransportError(
                'stream_failed',
                failure.diagnostic,
                { route: ROUTES.outputs },
              ),
            );
          } catch (cause) {
            const error = cause instanceof ProductBrowserLocalTransportError
              ? cause
              : new ProductBrowserLocalTransportError(
                'output_decode_failed',
                `Product Browser local runtime emitted an invalid output-lag event: ${cause instanceof Error ? cause.message : String(cause)}`,
                { cause, route: ROUTES.outputs },
              );
            reportTerminalFailure(
              {
                kind: 'output-lag',
                diagnostic: error.message,
              },
              error,
            );
          }
        };
        stream.addEventListener?.('rusty-output-lag', streamLagListener);
        streamFragmentListener = (event) => {
          if (terminalFailure !== null) return;
          try {
            const fragment = decodeOutputFragment(
              parseBoundedJson(event.data, MAXIMUM_RUNTIME_OUTPUT_EVENT_BYTES),
              maximumOutputBytes,
            );
            if (currentOutputBinding === null && !connectionBaselineComplete) {
              currentOutputBinding = fragment.runtime;
            }
            if (currentOutputBinding === null || !sameRuntimeIdentity(fragment.runtime, currentOutputBinding)) {
              throw new TypeError('output fragment runtime binding is stale or unavailable');
            }
            if (pendingFragment === null) {
              if (fragment.fragmentIndex !== 0) {
                throw new TypeError('output fragment transfer must begin at index zero');
              }
              pendingFragment = {
                transferId: fragment.transferId,
                runtime: fragment.runtime,
                fragmentCount: fragment.fragmentCount,
                aggregateBytes: fragment.aggregateBytes,
                nextIndex: 0,
                byteLength: 0,
                data: [],
              };
            }
            const pending = pendingFragment;
            if (pending.transferId !== fragment.transferId
              || !sameRuntimeIdentity(pending.runtime, fragment.runtime)
              || pending.fragmentCount !== fragment.fragmentCount
              || pending.aggregateBytes !== fragment.aggregateBytes
              || pending.nextIndex !== fragment.fragmentIndex) {
              throw new TypeError('output fragments are duplicated, reordered, or from another transfer');
            }
            pending.data.push(fragment.data);
            pending.byteLength += new TextEncoder().encode(fragment.data).byteLength;
            pending.nextIndex += 1;
            let completedOutputs: readonly ProductBrowserRuntimeOutput[] | null = null;
            if (pending.byteLength > pending.aggregateBytes) {
              throw new TypeError('output fragments exceed their declared aggregate length');
            }
            if (pending.nextIndex === pending.fragmentCount) {
              if (pending.byteLength !== pending.aggregateBytes) {
                throw new TypeError('output fragment transfer ended before its declared aggregate length');
              }
              const encoded = pending.data.join('');
              pendingFragment = null;
              completedOutputs = decodeRuntimeOutputBatch(parseBoundedJson(encoded, maximumOutputBytes));
            }
            if (connectionBaselineComplete && !reattachingFreshBaseline) {
              observeOutputSequence(event.lastEventId);
            }
            if (completedOutputs !== null) stageOrPublishOutputBatch(completedOutputs);
          } catch (cause) {
            failFragmentStream(cause);
          }
        };
        stream.addEventListener?.('rusty-output-fragment', streamFragmentListener);
        streamBaselineListener = (event) => {
          try {
            if (pendingFragment !== null) {
              throw new TypeError('connection baseline ended during an output fragment transfer');
            }
            if (event.lastEventId !== '') {
              throw new TypeError('connection baseline completion must not carry a reconnect cursor');
            }
            const result = decodeConnectionResult(parseBoundedJson(
              event.data,
              MAXIMUM_RUNTIME_RESPONSE_BYTES,
            ));
            if (!result.accepted) {
              throw new ProductBrowserLocalTransportError(
                'request_failed',
                result.diagnostic ?? 'Product Browser local runtime rejected the browser connection',
                { route: ROUTES.freshOutputs },
              );
            }
            if (connectionBaselineComplete) {
              if (!reattachingFreshBaseline) {
                throw new TypeError('connection baseline completion was duplicated without a reconnect');
              }
              // No retained output id existed when the completed fresh stream
              // failed, so EventSource correctly retried without a cursor and
              // the host attached again. The renderer already owns the first
              // atomic baseline; discard this replacement rather than replaying
              // duplicate Create operations into it.
              pendingConnectionOutputs = [];
              reattachingFreshBaseline = false;
              return;
            }
            connectionBaselineComplete = true;
            reattachingFreshBaseline = false;
            const baselineOutputs = pendingConnectionOutputs;
            pendingConnectionOutputs = [];
            publishOutputBatch(baselineOutputs);
            resolveConnectionReady?.(result);
            resolveConnectionReady = null;
            rejectConnectionReady = null;
          } catch (cause) {
            const error = cause instanceof ProductBrowserLocalTransportError
              ? cause
              : new ProductBrowserLocalTransportError(
                'output_decode_failed',
                `Product Browser local runtime emitted an invalid connection baseline: ${cause instanceof Error ? cause.message : String(cause)}`,
                { cause, route: ROUTES.freshOutputs },
              );
            rejectConnectionReady?.(error);
            resolveConnectionReady = null;
            rejectConnectionReady = null;
            reportTerminalFailure({ kind: 'runtime-failure', diagnostic: error.message }, error);
          }
        };
        stream.addEventListener?.('rusty-output-baseline', streamBaselineListener);
        stream.onmessage = (event) => {
          if (terminalFailure !== null) return;
          try {
            if (pendingFragment !== null) {
              throw new ProductBrowserLocalTransportError(
                'output_decode_failed',
                'Product Browser local runtime interrupted an output fragment transfer',
                { route: ROUTES.outputs },
              );
            }
            const outputs = decodeRuntimeOutputBatch(parseBoundedJson(
              event.data,
              Math.min(maximumOutputBytes, MAXIMUM_RUNTIME_OUTPUT_EVENT_BYTES),
            ));
            if (connectionBaselineComplete && !reattachingFreshBaseline) {
              observeOutputSequence(event.lastEventId);
            }
            stageOrPublishOutputBatch(outputs);
          } catch (cause) {
            const error = cause instanceof ProductBrowserLocalTransportError
              ? cause
              : new ProductBrowserLocalTransportError(
                'output_decode_failed',
                `Product Browser local runtime emitted an invalid output: ${cause instanceof Error ? cause.message : String(cause)}`,
                { cause, route: ROUTES.outputs },
              );
            failFragmentStream(error);
          }
        };
        stream.onerror = (event) => {
          if (terminalFailure !== null) return;
          if (!connectionBaselineComplete) {
            pendingFragment = null;
            pendingConnectionOutputs = [];
            currentOutputBinding = null;
          } else if (observedOutputSequence === 0n) {
            // Until one retained event supplies a cursor, retrying /fresh is
            // another detached attach. Stage and discard that replacement
            // baseline once its unnumbered completion arrives.
            pendingFragment = null;
            pendingConnectionOutputs = [];
            reattachingFreshBaseline = true;
          }
          const error = new ProductBrowserLocalTransportError(
            'stream_failed',
            `Product Browser local runtime output stream failed${event instanceof Error ? `: ${event.message}` : ''}`,
            { route: ROUTES.outputs },
          );
          // EventSource owns same-URL retry semantics. Keep the stream and
          // listeners alive so a transient local-server restart can recover.
          reportTransportError(error);
        };
      } catch (cause) {
        listeners.delete(listener);
        stream?.close();
        stream = null;
        streamLagListener = null;
        streamFragmentListener = null;
        streamBaselineListener = null;
        pendingFragment = null;
        pendingConnectionOutputs = [];
        reattachingFreshBaseline = false;
        wakeOutputSequenceWaiters();
        resolveOutputSubscriptionReady?.();
        resolveOutputSubscriptionReady = null;
        outputSubscriptionReady = null;
        connectionReady = null;
        resolveConnectionReady = null;
        rejectConnectionReady = null;
        throw new ProductBrowserLocalTransportError(
          'stream_failed',
          `Product Browser local runtime output stream could not start: ${cause instanceof Error ? cause.message : String(cause)}`,
          { cause, route: ROUTES.outputs },
        );
      }
    }
    let active = true;
    return () => {
      if (!active) return;
      active = false;
      listeners.delete(listener);
      if (listeners.size === 0) {
        if (streamLagListener !== null) {
          stream?.removeEventListener?.('rusty-output-lag', streamLagListener);
          streamLagListener = null;
        }
        if (streamFragmentListener !== null) {
          stream?.removeEventListener?.('rusty-output-fragment', streamFragmentListener);
          streamFragmentListener = null;
        }
        if (streamBaselineListener !== null) {
          stream?.removeEventListener?.('rusty-output-baseline', streamBaselineListener);
          streamBaselineListener = null;
        }
        stream?.close();
        stream = null;
        pendingFragment = null;
        pendingConnectionOutputs = [];
        reattachingFreshBaseline = false;
        wakeOutputSequenceWaiters();
        resolveOutputSubscriptionReady?.();
        resolveOutputSubscriptionReady = null;
        outputSubscriptionReady = null;
        connectionReady = null;
        resolveConnectionReady = null;
        rejectConnectionReady = null;
      }
    };
  };

  const subscribeOutputBatches = (
    listener: (outputs: readonly ProductBrowserRuntimeOutput[]) => void,
  ): (() => void) => {
    ensureOpen();
    if (typeof listener !== 'function') {
      throw new ProductBrowserLocalTransportError(
        'invalid_options',
        'Product Browser local runtime output batch listener must be a function',
      );
    }
    batchListeners.add(listener);
    // The existing stream owner remains the single lifecycle authority. Its
    // private no-op listener keeps that stream alive while delivery happens
    // once through the ordered batch callback above.
    const releaseStream = subscribeOutputs(() => undefined);
    let active = true;
    return () => {
      if (!active) return;
      active = false;
      batchListeners.delete(listener);
      releaseStream();
    };
  };

  const waitUntilOutputSubscriptionReady = async (): Promise<void> => {
    ensureOpen();
    if (stream === null || outputSubscriptionReady === null) {
      throw new ProductBrowserLocalTransportError(
        'stream_failed',
        'Product Browser local runtime output subscription has not started',
        { route: ROUTES.outputs },
      );
    }
    await outputSubscriptionReady;
    ensureOpen();
  };

  const connect = async (): Promise<ProductBrowserRuntimeOperationResult> => {
    ensureOpen();
    if (stream === null || connectionReady === null) {
      throw new ProductBrowserLocalTransportError(
        'stream_failed',
        'Product Browser local runtime connection has not started',
        { route: ROUTES.freshOutputs },
      );
    }
    const result = await connectionReady;
    ensureOpen();
    return result;
  };

  const subscribeTerminalFailures = (
    listener: ProductBrowserRuntimeTerminalFailureListener,
  ): (() => void) => {
    if (typeof listener !== 'function') {
      throw new ProductBrowserLocalTransportError(
        'invalid_options',
        'Product Browser local runtime terminal-failure listener must be a function',
      );
    }
    if (terminalFailure !== null) {
      try {
        listener(terminalFailure);
      } catch (cause) {
        reportTransportError(new ProductBrowserLocalTransportError(
          'stream_failed',
          `Product Browser local runtime terminal-failure listener failed: ${cause instanceof Error ? cause.message : String(cause)}`,
          { cause, route: ROUTES.outputs },
        ));
      }
      return () => undefined;
    }
    terminalFailureListeners.add(listener);
    let active = true;
    return () => {
      if (!active) return;
      active = false;
      terminalFailureListeners.delete(listener);
    };
  };

  const dispose = (): void => {
    if (disposed) return;
    disposed = true;
    abortController.abort();
    if (streamLagListener !== null) {
      stream?.removeEventListener?.('rusty-output-lag', streamLagListener);
      streamLagListener = null;
    }
    if (streamFragmentListener !== null) {
      stream?.removeEventListener?.('rusty-output-fragment', streamFragmentListener);
      streamFragmentListener = null;
    }
    if (streamBaselineListener !== null) {
      stream?.removeEventListener?.('rusty-output-baseline', streamBaselineListener);
      streamBaselineListener = null;
    }
    stream?.close();
    stream = null;
    pendingFragment = null;
    pendingConnectionOutputs = [];
    reattachingFreshBaseline = false;
    wakeOutputSequenceWaiters();
    resolveOutputSubscriptionReady?.();
    resolveOutputSubscriptionReady = null;
    outputSubscriptionReady = null;
    connectionReady = null;
    resolveConnectionReady = null;
    rejectConnectionReady = null;
    listeners.clear();
    batchListeners.clear();
    terminalFailureListeners.clear();
  };

  return Object.freeze({
    connect,
    lifecycle,
    input,
    reportAudioFeedback,
    reportAnimationFeedback,
    reportGhostPlateFeedback,
    reportRendererDiagnostics,
    reportBrowserDiagnostics,
    advanceRealtime,
    admitDemandStep,
    admitExternalStep,
    completeTimeline,
    subscribeTerminalFailures,
    subscribeOutputs,
    subscribeOutputBatches,
    waitUntilOutputSubscriptionReady,
    dispose,
  });
}

function resolveFetch(): ProductBrowserLocalFetch {
  const value = globalThis.fetch;
  if (typeof value !== 'function') {
    throw new ProductBrowserLocalTransportError(
      'invalid_options',
      'Product Browser local runtime transport requires fetch',
    );
  }
  return value.bind(globalThis) as ProductBrowserLocalFetch;
}

function resolveEventSource(): ProductBrowserLocalEventSourceConstructor {
  const value = (globalThis as typeof globalThis & {
    readonly EventSource?: ProductBrowserLocalEventSourceConstructor;
  }).EventSource;
  if (typeof value !== 'function') {
    throw new ProductBrowserLocalTransportError(
      'invalid_options',
      'Product Browser local runtime transport requires EventSource',
    );
  }
  return value;
}

function validateBasePath(value: string): string {
  if (typeof value !== 'string' || value.length === 0 || value.length > 256
    || !value.startsWith('/') || value.startsWith('//') || value.includes('\\')
    || value.includes('..') || value.includes('%')
    || value.includes('?') || value.includes('#') || !value.endsWith('/')) {
    throw new ProductBrowserLocalTransportError(
      'invalid_options',
      'Product Browser local runtime basePath must be a same-origin absolute path ending in /',
    );
  }
  return value;
}

function validateMaximumBytes(value: number, name: string, maximum = MAXIMUM_CONFIGURED_BYTES): number {
  if (!Number.isSafeInteger(value) || value < 1 || value > maximum) {
    throw new ProductBrowserLocalTransportError(
      'invalid_options',
      `${name} must be a positive safe integer no greater than ${String(maximum)}`,
    );
  }
  return value;
}

function decodeOutputSequence(
  value: string,
  name: string,
  code: 'response_decode_failed' | 'output_decode_failed',
  route: string = ROUTES.outputs,
): bigint {
  if (!/^(?:0|[1-9]\d{0,19})$/u.test(value)
    || (value.length === UINT64_MAX_DECIMAL.length && value > UINT64_MAX_DECIMAL)) {
    throw new ProductBrowserLocalTransportError(
      code,
      `${name} must be canonical unsigned 64-bit decimal text`,
      { route },
    );
  }
  return BigInt(value);
}

function requireU64Text(value: unknown, name: string): string {
  if (typeof value !== 'string'
    || !/^(?:0|[1-9]\d{0,19})$/u.test(value)
    || (value.length === UINT64_MAX_DECIMAL.length && value > UINT64_MAX_DECIMAL)) {
    throw new ProductBrowserLocalTransportError(
      'invalid_options',
      `${name} must be a canonical unsigned 64-bit decimal string`,
    );
  }
  return value;
}

function requireU32(value: unknown, name: string): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > 4_294_967_295) {
    throw new ProductBrowserLocalTransportError(
      'invalid_options',
      `${name} must be an unsigned 32-bit integer`,
    );
  }
  return value as number;
}

function requireIdentity(value: unknown, name: string): string {
  return requireProductIdentity(value, name);
}

function requireProductIdentity(value: unknown, name: string): string {
  if (typeof value !== 'string'
    || new TextEncoder().encode(value).byteLength > 128
    || !/^[a-z0-9](?:[a-z0-9]|[._-](?=[a-z0-9]))*$/u.test(value)) {
    throw new ProductBrowserLocalTransportError(
      'invalid_options',
      `${name} must be a 1..128 byte lowercase runtime identity`,
    );
  }
  return value;
}

function requireFaultCode(value: unknown, name: string): string {
  if (typeof value !== 'string'
    || new TextEncoder().encode(value).byteLength > 128
    || !/^[A-Z0-9][A-Z0-9._-]*$/u.test(value)) {
    throw new TypeError(`${name} must be a bounded stable host fault code`);
  }
  return value;
}

function requireBoundedString(value: unknown, name: string, maximumBytes = 256): string {
  if (typeof value !== 'string' || value.length === 0
    || new TextEncoder().encode(value).byteLength > maximumBytes
    || /[\u0000-\u001f\u007f]/u.test(value)) {
    throw new TypeError(`${name} must be a bounded string without control characters`);
  }
  return value;
}

function requireFiniteNumber(
  value: unknown,
  name: string,
  minimum = Number.NEGATIVE_INFINITY,
  maximum = Number.POSITIVE_INFINITY,
): number {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < minimum || value > maximum) {
    throw new TypeError(`${name} must be a finite number within [${String(minimum)}, ${String(maximum)}]`);
  }
  return value;
}

function requireInputEdge(value: unknown): 'pressed' | 'released' {
  return requireCatalogValue<'pressed' | 'released'>(value, 'input edge', INPUT_EDGES);
}

function requireCatalogValue<T extends string>(
  value: unknown,
  name: string,
  catalog: ReadonlySet<string>,
): T {
  if (typeof value !== 'string' || !catalog.has(value)) {
    throw new TypeError(`${name} is not in the closed runtime input catalog`);
  }
  return value as T;
}

function hasOwn(record: ProductBrowserWireRecord, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(record, key);
}

async function readResponseText(
  response: Response,
  maximumBytes: number,
  route: string,
): Promise<string> {
  const reader = response.body?.getReader();
  if (reader === undefined) {
    let text: string;
    try {
      text = await response.text();
    } catch (cause) {
      throw new ProductBrowserLocalTransportError(
        'request_failed',
        `Product Browser local runtime response could not be read for ${route}`,
        { cause, route },
      );
    }
    const bytes = new TextEncoder().encode(text).byteLength;
    if (bytes > maximumBytes) {
      throw new ProductBrowserLocalTransportError(
        'response_decode_failed',
        `Product Browser local runtime response for ${route} exceeds ${String(maximumBytes)} bytes`,
        { route },
      );
    }
    return text;
  }
  const decoder = new TextDecoder();
  let byteLength = 0;
  let text = '';
  try {
    while (true) {
      const chunk = await reader.read();
      if (chunk.done) break;
      byteLength += chunk.value.byteLength;
      if (byteLength > maximumBytes) {
        await reader.cancel();
        throw new ProductBrowserLocalTransportError(
          'response_decode_failed',
          `Product Browser local runtime response for ${route} exceeds ${String(maximumBytes)} bytes`,
          { route },
        );
      }
      text += decoder.decode(chunk.value, { stream: true });
    }
    text += decoder.decode();
  } catch (cause) {
    if (cause instanceof ProductBrowserLocalTransportError) throw cause;
    throw new ProductBrowserLocalTransportError(
      'request_failed',
      `Product Browser local runtime response could not be read for ${route}`,
      { cause, route },
    );
  }
  return text;
}

function encodeRequestBody(body: unknown, maximumBytes: number, route: string): string {
  let text: string | undefined;
  try {
    text = JSON.stringify(body);
  } catch (cause) {
    throw new ProductBrowserLocalTransportError(
      'invalid_options',
      `Product Browser local runtime request for ${route} is not JSON-safe`,
      { cause, route },
    );
  }
  if (text === undefined) {
    throw new ProductBrowserLocalTransportError(
      'invalid_options',
      `Product Browser local runtime request for ${route} is not a JSON value`,
      { route },
    );
  }
  const bytes = new TextEncoder().encode(text).byteLength;
  if (bytes > maximumBytes) {
    throw new ProductBrowserLocalTransportError(
      'invalid_options',
      `Product Browser local runtime request for ${route} exceeds ${String(maximumBytes)} bytes`,
      { route },
    );
  }
  return text;
}

type ProductBrowserLocalJson =
  | null
  | boolean
  | number
  | string
  | readonly ProductBrowserLocalJson[]
  | { readonly [key: string]: ProductBrowserLocalJson };

function snapshotInputBatch(
  batch: readonly RustyApplicationRuntimeInputEnvelope[],
): readonly RustyApplicationRuntimeInputEnvelope[] {
  const source = requirePlainArray(batch, 'runtime input batch');
  if (source.length > MAXIMUM_INPUT_BATCH_LENGTH) {
    throw new ProductBrowserLocalTransportError(
      'invalid_options',
      `runtime input batch must contain 0..${String(MAXIMUM_INPUT_BATCH_LENGTH)} entries`,
    );
  }
  const entries: RustyApplicationRuntimeInputEnvelope[] = [];
  for (let index = 0; index < source.length; index += 1) {
    const descriptor = Object.getOwnPropertyDescriptor(source, String(index));
    if (descriptor === undefined || !('value' in descriptor)) {
      throw new ProductBrowserLocalTransportError(
        'invalid_options',
        `runtime input batch entry ${String(index)} cannot be a getter or hole`,
      );
    }
    entries.push(snapshotInputEnvelope(descriptor.value));
  }
  return Object.freeze(entries);
}

function snapshotInputEnvelope(value: unknown): RustyApplicationRuntimeInputEnvelope {
  const record = requireRecord(value, 'runtime input envelope');
  const common = {
    runtime: decodeRuntimeIdentity(record.runtime),
    sequence: requireU64Text(record.sequence, 'sequence'),
    context: requireProductIdentity(record['context'], 'context'),
  };
  if (hasOwn(record, 'fact')) {
    requireKnownFields(record, ['runtime', 'sequence', 'context', 'fact'], 'runtime input ingress');
    return Object.freeze({ ...common, fact: snapshotInputFact(record['fact']) });
  }
  if (hasOwn(record, 'intent')) {
    requireKnownFields(record, ['runtime', 'sequence', 'context', 'intent', 'value'], 'runtime direct intent claim');
    return Object.freeze({
      ...common,
      intent: requireProductIdentity(record['intent'], 'intent'),
      value: snapshotIntentValue(record['value']),
    });
  }
  throw new TypeError('runtime input envelope must contain fact or intent');
}

function snapshotAudioFeedback(value: ProductBrowserAudioFeedback): ProductBrowserAudioFeedback {
  const record = requireRecord(value, 'audio feedback');
  requireKnownFields(record, ['runtime', 'replaceOwner', 'evictedFactCount', 'facts'], 'audio feedback');
  if (typeof record.replaceOwner !== 'boolean') throw new TypeError('audio feedback replaceOwner must be boolean');
  const facts = requirePlainArray(record.facts, 'audio feedback facts');
  if (facts.length > MAXIMUM_AUDIO_FEEDBACK_FACTS) {
    throw new ProductBrowserLocalTransportError(
      'invalid_options',
      `audio feedback facts must contain 0..${String(MAXIMUM_AUDIO_FEEDBACK_FACTS)} entries`,
    );
  }
  const snapshotFacts: ProductBrowserAudioFeedbackFact[] = [];
  for (let index = 0; index < facts.length; index += 1) {
    const descriptor = Object.getOwnPropertyDescriptor(facts, String(index));
    if (descriptor === undefined || !('value' in descriptor)) {
      throw new ProductBrowserLocalTransportError(
        'invalid_options',
        `audio feedback fact ${String(index)} cannot be a getter or hole`,
      );
    }
    snapshotFacts.push(snapshotAudioFeedbackFact(descriptor.value));
  }
  return Object.freeze({
    runtime: decodeRuntimeIdentity(record.runtime),
    replaceOwner: record.replaceOwner,
    evictedFactCount: requireU64Text(record.evictedFactCount, 'audio feedback evictedFactCount'),
    facts: Object.freeze(snapshotFacts),
  });
}

function snapshotAudioFeedbackFact(value: unknown): ProductBrowserAudioFeedbackFact {
  const record = requireRecord(value, 'audio feedback fact');
  const common = {
    factId: requireU64Text(record['factId'], 'audio feedback factId'),
    sequence: requireU32(record.sequence, 'audio feedback sequence'),
  };
  if (record.kind === 'naturalCompletion') {
    if (record.source === 'oneShot') {
      requireKnownFields(record, ['kind', 'source', 'factId', 'sequence', 'signalHandle'], 'one-shot audio completion');
      return Object.freeze({
        kind: 'naturalCompletion', source: 'oneShot', ...common,
        signalHandle: requireU64Text(record.signalHandle, 'audio feedback signalHandle'),
      });
    }
    if (record.source === 'retainedVoice') {
      requireKnownFields(record, ['kind', 'source', 'factId', 'sequence', 'voiceHandle'], 'retained audio completion');
      return Object.freeze({
        kind: 'naturalCompletion', source: 'retainedVoice', ...common,
        voiceHandle: requireU64Text(record.voiceHandle, 'audio feedback voiceHandle'),
      });
    }
    throw new TypeError('audio feedback natural completion source is invalid');
  }
  if (record.kind === 'diagnostic') {
    requireKnownFields(record, ['kind', 'factId', 'code', 'sequence', 'voiceHandle'], 'audio feedback diagnostic');
    return Object.freeze({
      kind: 'diagnostic', ...common,
      code: requireCatalogValue<string>(record.code, 'audio feedback diagnostic code', AUDIO_DIAGNOSTIC_CODES),
      voiceHandle: record.voiceHandle === null
        ? null
        : requireU64Text(record.voiceHandle, 'audio feedback diagnostic voiceHandle'),
    });
  }
  throw new TypeError('audio feedback fact kind is not admitted');
}

function snapshotAnimationFeedback(value: ProductBrowserAnimationFeedback): ProductBrowserAnimationFeedback {
  const record = requireRecord(value, 'animation feedback');
  requireKnownFields(record, ['runtime', 'replaceOwner', 'evictedFactCount', 'facts'], 'animation feedback');
  if (typeof record.replaceOwner !== 'boolean') throw new TypeError('animation feedback replaceOwner must be boolean');
  const facts = requirePlainArray(record.facts, 'animation feedback facts');
  if (facts.length > MAXIMUM_ANIMATION_FEEDBACK_FACTS) throw new ProductBrowserLocalTransportError('invalid_options', 'animation feedback exceeds 128 facts');
  return Object.freeze({
    runtime: decodeRuntimeIdentity(record.runtime), replaceOwner: record.replaceOwner,
    evictedFactCount: requireU64Text(record.evictedFactCount, 'animation feedback evictedFactCount'),
    facts: Object.freeze(facts.map((fact) => snapshotAnimationFeedbackFact(fact))),
  });
}

function snapshotGhostPlateFeedback(value: ProductBrowserGhostPlateFeedback): ProductBrowserGhostPlateFeedback {
  const record = requireRecord(value, 'ghost plate feedback');
  requireKnownFields(record, ['runtime', 'replaceOwner', 'facts'], 'ghost plate feedback');
  if (typeof record.replaceOwner !== 'boolean') throw new TypeError('ghost plate feedback replaceOwner must be boolean');
  const facts = requirePlainArray(record.facts, 'ghost plate feedback facts');
  if (facts.length > 128) throw new ProductBrowserLocalTransportError('invalid_options', 'ghost plate feedback exceeds 128 facts');
  return Object.freeze({
    runtime: decodeRuntimeIdentity(record.runtime),
    replaceOwner: record.replaceOwner,
    facts: Object.freeze(facts.map(snapshotGhostPlateFeedbackFact)),
  });
}

function snapshotRendererDiagnosticsFeedback(
  value: ProductBrowserRendererDiagnosticsFeedback,
): ProductBrowserRendererDiagnosticsFeedback {
  const record = requireRecord(value, 'renderer diagnostics feedback');
  requireKnownFields(record, ['runtime', 'snapshot'], 'renderer diagnostics feedback');
  const snapshot = snapshotJsonValue(record['snapshot']);
  const snapshotRecord = requireRecord(snapshot, 'renderer diagnostics snapshot');
  if (snapshotRecord['schemaVersion'] !== 1) {
    throw new TypeError('renderer diagnostics snapshot schemaVersion must equal 1');
  }
  const bytes = new TextEncoder().encode(JSON.stringify(snapshot)).byteLength;
  if (bytes > MAXIMUM_RENDERER_DIAGNOSTICS_SNAPSHOT_BYTES) {
    throw new TypeError(
      `renderer diagnostics snapshot exceeds ${String(MAXIMUM_RENDERER_DIAGNOSTICS_SNAPSHOT_BYTES)} bytes`,
    );
  }
  return Object.freeze({
    runtime: decodeRuntimeIdentity(record.runtime),
    snapshot,
  }) as unknown as ProductBrowserRendererDiagnosticsFeedback;
}

function snapshotBrowserDiagnosticsReport(
  value: ProductBrowserDiagnosticsReport,
): ProductBrowserDiagnosticsReport {
  const record = requireRecord(value, 'browser diagnostics report');
  requireKnownFields(record, [
    'hostState', 'runtimeProgress', 'transportState', 'outputState',
    'lastRendererSequence', 'rendererObservationAgeMs', 'firstTerminal', 'recoverableEvent', 'pageEvents',
  ], 'browser diagnostics report');
  const pageEvents = requirePlainArray(record.pageEvents, 'browser diagnostics page events');
  if (pageEvents.length > 8) throw new TypeError('browser diagnostics exceeds 8 page events');
  const terminal = record.firstTerminal === undefined
    ? undefined
    : snapshotBrowserDiagnostic(record.firstTerminal, 'browser terminal diagnostic');
  const recoverable = record['recoverableEvent'] === undefined
    ? undefined
    : snapshotBrowserDiagnostic(record['recoverableEvent'], 'browser recoverable diagnostic');
  if (recoverable !== undefined && recoverable.code !== 'CSHARP_LIFECYCLE_CLOCK_REGRESSION') {
    throw new TypeError('browser recoverable diagnostic code is not supported');
  }
  return Object.freeze({
    hostState: requireCatalogValue(record.hostState, 'browser host state', new Set(['loading', 'ready', 'failed', 'disposed'])),
    runtimeProgress: requireU64Text(record.runtimeProgress, 'browser runtime progress'),
    transportState: requireCatalogValue(record.transportState, 'browser transport state', new Set(['open', 'closed'])),
    outputState: requireCatalogValue(record.outputState, 'browser output state', new Set(['open', 'closed'])),
    ...(record.lastRendererSequence === undefined ? {} : { lastRendererSequence: requireU64Text(record.lastRendererSequence, 'browser renderer sequence') }),
    ...(record.rendererObservationAgeMs === undefined ? {} : { rendererObservationAgeMs: requireU64Text(record.rendererObservationAgeMs, 'browser renderer observation age') }),
    ...(terminal === undefined ? {} : { firstTerminal: terminal }),
    ...(recoverable === undefined ? {} : { recoverableEvent: recoverable }),
    pageEvents: Object.freeze(pageEvents.map((event) => {
      const eventRecord = requireRecord(event, 'browser page diagnostic');
      requireKnownFields(eventRecord, ['kind', 'code', 'message'], 'browser page diagnostic');
      return Object.freeze({
        kind: requireCatalogValue(eventRecord.kind, 'browser page diagnostic kind', new Set(['error', 'unhandled-rejection'])),
        code: requireBrowserDiagnosticCode(eventRecord.code, 'browser page diagnostic code'),
        message: requireDiagnostic(eventRecord['message']),
      });
    })),
  }) as ProductBrowserDiagnosticsReport;
}

function snapshotBrowserDiagnostic(value: unknown, name: string): { readonly code: string; readonly message: string } {
  const record = requireRecord(value, name);
  requireKnownFields(record, ['code', 'message'], name);
  const code = requireBrowserDiagnosticCode(record.code, `${name} code`);
  const message = requireDiagnostic(record['message']);
  return Object.freeze({ code, message });
}

function requireBrowserDiagnosticCode(value: unknown, name: string): string {
  if (typeof value !== 'string' || value.length === 0
    || new TextEncoder().encode(value).byteLength > 128
    || /[^A-Za-z0-9._:-]/u.test(value)) {
    throw new TypeError(`${name} is invalid`);
  }
  return value;
}

function snapshotGhostPlateFeedbackFact(value: unknown): ProductBrowserGhostPlateFeedbackFact {
  const record = requireRecord(value, 'ghost plate feedback fact');
  requireKnownFields(record, [
    'presentation', 'sourceMatches', 'currentSector', 'localAngularOffsetDegrees',
    'fallbackActive', 'fallbackReason', 'limitationMask', 'preparationCpuMilliseconds',
    'captureCpuSubmissionMilliseconds', 'retainedSectorCount', 'retainedMeshCount',
    'retainedMaterialCount', 'retainedBorrowedTextureCount',
  ], 'ghost plate feedback fact');
  const optionalFinite = (candidate: unknown, field: string, minimum: number): number | null =>
    candidate === null ? null : requireFiniteNumber(candidate, field, minimum, Number.MAX_VALUE);
  const fallbackReason = requireCatalogValue<'none' | 'preparedSourceUnsupported' | 'realizationFailed'>(
    record['fallbackReason'],
    'ghost plate fallback reason',
    new Set(['none', 'preparedSourceUnsupported', 'realizationFailed']),
  );
  if (typeof record['sourceMatches'] !== 'boolean' || typeof record['fallbackActive'] !== 'boolean') {
    throw new TypeError('ghost plate boolean observations are invalid');
  }
  return Object.freeze({
    presentation: requireU64Text(record['presentation'], 'ghost plate presentation'),
    sourceMatches: record['sourceMatches'],
    currentSector: requireU32(record['currentSector'], 'ghost plate current sector'),
    localAngularOffsetDegrees: optionalFinite(record['localAngularOffsetDegrees'], 'ghost plate local angular offset', -360),
    fallbackActive: record['fallbackActive'],
    fallbackReason,
    limitationMask: requireU32(record['limitationMask'], 'ghost plate limitation mask'),
    preparationCpuMilliseconds: optionalFinite(record['preparationCpuMilliseconds'], 'ghost plate preparation cpu milliseconds', 0),
    captureCpuSubmissionMilliseconds: optionalFinite(record['captureCpuSubmissionMilliseconds'], 'ghost plate capture cpu milliseconds', 0),
    retainedSectorCount: requireU32(record['retainedSectorCount'], 'ghost plate retained sectors'),
    retainedMeshCount: requireU32(record['retainedMeshCount'], 'ghost plate retained meshes'),
    retainedMaterialCount: requireU32(record['retainedMaterialCount'], 'ghost plate retained materials'),
    retainedBorrowedTextureCount: requireU32(record['retainedBorrowedTextureCount'], 'ghost plate retained borrowed textures'),
  });
}

function snapshotAnimationFeedbackFact(value: unknown): ProductBrowserAnimationFeedbackFact {
  const record = requireRecord(value, 'animation feedback fact');
  const factId = requireU64Text(record['factId'], 'animation feedback factId');
  if (record.kind === 'playbackObservation') {
    requireKnownFields(record, ['kind', 'factId', 'objectId', 'generation', 'sequence', 'status', 'selectedClip', 'sampledAtSeconds'], 'animation playback observation');
    return Object.freeze({ kind: 'playbackObservation', factId,
      objectId: requireU64Text(record['objectId'], 'animation feedback objectId'),
      generation: requireU64Text(record['generation'], 'animation feedback generation'),
      sequence: requireU32(record['sequence'], 'animation feedback sequence'), status: requireBoundedString(record['status'], 'animation playback status'),
      selectedClip: record['selectedClip'] === null ? null : requireBoundedString(record['selectedClip'], 'animation selected clip'),
      sampledAtSeconds: record['sampledAtSeconds'] === null ? null : requireFiniteNumber(record['sampledAtSeconds'], 'animation sample seconds', 0, Number.MAX_VALUE),
    });
  }
  if (record.kind === 'naturalCompletion') {
    requireKnownFields(record, ['kind', 'factId', 'objectId', 'generation', 'clip'], 'animation natural completion');
    return Object.freeze({ kind: 'naturalCompletion', factId,
      objectId: requireU64Text(record['objectId'], 'animation feedback objectId'),
      generation: requireU64Text(record['generation'], 'animation feedback generation'),
      clip: requireBoundedString(record['clip'], 'animation completion clip'),
    });
  }
  if (record.kind === 'cue') {
    requireKnownFields(record, ['kind', 'factId', 'objectId', 'generation', 'cueId', 'clip', 'markerSeconds', 'sampledAtSeconds', 'signalDomain', 'signalId'], 'animation cue');
    const signalDomain = requireCatalogValue<'audio' | 'particle'>(record['signalDomain'], 'animation cue signal domain', new Set(['audio', 'particle']));
    return Object.freeze({ kind: 'cue', factId,
      objectId: requireU64Text(record['objectId'], 'animation feedback objectId'),
      generation: requireU64Text(record['generation'], 'animation feedback generation'),
      cueId: requireBoundedString(record['cueId'], 'animation cue id'), clip: requireBoundedString(record['clip'], 'animation cue clip'),
      markerSeconds: requireFiniteNumber(record['markerSeconds'], 'animation cue marker', 0, Number.MAX_VALUE), sampledAtSeconds: requireFiniteNumber(record['sampledAtSeconds'], 'animation cue sample', 0, Number.MAX_VALUE), signalDomain, signalId: requireBoundedString(record['signalId'], 'animation cue signal id') });
  }
  if (record.kind === 'stopped') {
    requireKnownFields(record, ['kind', 'factId', 'objectId', 'generation', 'sequence', 'reason'], 'animation stopped observation');
    return Object.freeze({ kind: 'stopped', factId,
      objectId: requireU64Text(record['objectId'], 'animation feedback objectId'),
      generation: requireU64Text(record['generation'], 'animation feedback generation'),
      sequence: requireU32(record['sequence'], 'animation feedback sequence'), reason: requireCatalogValue<'destroyed' | 'teardown'>(record['reason'], 'animation stop reason', new Set(['destroyed', 'teardown'])) });
  }
  if (record.kind === 'diagnostic') {
    requireKnownFields(record, ['kind', 'factId', 'objectId', 'generation', 'code', 'sequence'], 'animation diagnostic');
    return Object.freeze({ kind: 'diagnostic', factId,
      objectId: record['objectId'] === null ? null : requireU64Text(record['objectId'], 'animation feedback objectId'),
      generation: record['generation'] === null ? null : requireU64Text(record['generation'], 'animation feedback generation'),
      code: requireBoundedString(record['code'], 'animation diagnostic code'), sequence: requireU32(record['sequence'], 'animation diagnostic sequence') });
  }
  throw new TypeError('animation feedback fact kind is not admitted');
}

function snapshotInputFact(value: unknown): RustyApplicationRuntimeInputFact {
  const record = requireRecord(value, 'runtime input fact');
  const kind = record.kind;
  switch (kind) {
    case 'key':
      requireKnownFields(record, ['kind', 'code', 'edge'], 'key input fact');
      return Object.freeze({
        kind,
        code: requireCatalogValue<RustyApplicationKeyboardControl>(record['code'], 'key code', KEYBOARD_CONTROLS),
        edge: requireInputEdge(record['edge']),
      });
    case 'pointer-button':
      requireKnownFields(record, ['kind', 'button', 'edge'], 'pointer-button input fact');
      return Object.freeze({
        kind,
        button: requireCatalogValue<RustyApplicationPointerButton>(record['button'], 'pointer button', POINTER_BUTTONS),
        edge: requireInputEdge(record['edge']),
      });
    case 'pointer-delta':
      requireKnownFields(record, ['kind', 'x', 'y'], 'pointer-delta input fact');
      return Object.freeze({ kind, x: requireFiniteNumber(record['x'], 'pointer delta x', -256, 256), y: requireFiniteNumber(record['y'], 'pointer delta y', -256, 256) });
    case 'wheel':
      requireKnownFields(record, ['kind', 'x', 'y'], 'wheel input fact');
      return Object.freeze({ kind, x: requireFiniteNumber(record['x'], 'wheel x', -256, 256), y: requireFiniteNumber(record['y'], 'wheel y', -256, 256) });
    case 'controller-button':
      requireKnownFields(record, ['kind', 'button', 'edge'], 'controller-button input fact');
      return Object.freeze({
        kind,
        button: requireCatalogValue<RustyApplicationControllerButton>(record['button'], 'controller button', CONTROLLER_BUTTONS),
        edge: requireInputEdge(record['edge']),
      });
    case 'controller-axis':
      requireKnownFields(record, ['kind', 'axis', 'value'], 'controller-axis input fact');
      return Object.freeze({
        kind,
        axis: requireCatalogValue<RustyApplicationControllerAxis>(record['axis'], 'controller axis', CONTROLLER_AXES),
        value: requireFiniteNumber(record['value'], 'controller axis value', -1, 1),
      });
    case 'clear':
      requireKnownFields(record, ['kind', 'reason'], 'clear input fact');
      return Object.freeze({
        kind,
        reason: requireCatalogValue<RustyApplicationInputClearReason>(record['reason'], 'input clear reason', INPUT_CLEAR_REASONS),
      });
    default:
      throw new TypeError('runtime input fact kind is not admitted');
  }
}

function snapshotIntentValue(value: unknown): RustyApplicationRuntimeIntentValue {
  const record = requireRecord(value, 'runtime intent value');
  switch (record.kind) {
    case 'digital':
      requireKnownFields(record, ['kind', 'active'], 'digital intent value');
      if (typeof record['active'] !== 'boolean') throw new TypeError('digital intent active must be boolean');
      return Object.freeze({ kind: 'digital', active: record['active'] });
    case 'axis':
      requireKnownFields(record, ['kind', 'value'], 'axis intent value');
      return Object.freeze({ kind: 'axis', value: requireFiniteNumber(record['value'], 'axis intent value', -1, 1) });
    case 'product-payload':
      requireKnownFields(record, ['kind', 'contract', 'data'], 'product payload intent value');
      return Object.freeze({
        kind: 'product-payload',
        contract: requireProductIdentity(record['contract'], 'product payload contract'),
        data: snapshotRustyApplicationProductPayloadJson(record['data']),
      });
    default:
      throw new TypeError('runtime intent value kind is not admitted');
  }
}

function snapshotTimelineCompletion(
  value: ProductBrowserTimelineCompletion,
): ProductBrowserTimelineCompletion {
  const record = requireRecord(value, 'timeline completion');
  requireKnownFields(record, ['ticket', 'runtime', 'correlation', 'outcome', 'provenance'], 'timeline completion');
  const outcomeRecord = requireRecord(record['outcome'], 'timeline completion outcome');
  requireKnownFields(outcomeRecord, ['kind', 'data'], 'timeline completion outcome');
  if (outcomeRecord.kind !== 'success' && outcomeRecord.kind !== 'failure') {
    throw new TypeError('timeline completion outcome kind is invalid');
  }
  const provenanceRecord = requireRecord(record['provenance'], 'timeline completion provenance');
  requireKnownFields(provenanceRecord, ['correlation', 'detail'], 'timeline completion provenance');
  const correlation = requireProductIdentity(record['correlation'], 'timeline correlation');
  const provenanceCorrelation = requireProductIdentity(
    provenanceRecord['correlation'],
    'provenance correlation',
  );
  if (correlation !== provenanceCorrelation) {
    throw new TypeError('timeline provenance correlation must match completion correlation');
  }
  const outcome = {
    kind: outcomeRecord.kind,
    ...(hasOwn(outcomeRecord, 'data')
      ? { data: snapshotTimelineOpaqueData(outcomeRecord.data) }
      : {}),
  } as ProductBrowserTimelineCompletion['outcome'];
  const provenance = {
    correlation: provenanceCorrelation,
    ...(hasOwn(provenanceRecord, 'detail')
      ? { detail: snapshotTimelineOpaqueData(provenanceRecord.detail) }
      : {}),
  } as ProductBrowserTimelineCompletion['provenance'];
  const snapshot = Object.freeze({
    ticket: requireU64Text(record.ticket, 'timeline ticket'),
    runtime: decodeRuntimeIdentity(record.runtime),
    correlation,
    outcome: Object.freeze(outcome),
    provenance: Object.freeze(provenance),
  });
  return snapshot;
}

function snapshotTimelineOpaqueData(value: unknown): ProductBrowserLocalJson {
  const snapshot = snapshotTimelineJsonValue(value);
  const bytes = new TextEncoder().encode(JSON.stringify(snapshot)).byteLength;
  if (bytes > MAXIMUM_TIMELINE_JSON_BYTES) {
    throw new TypeError(
      `timeline opaque data exceeds ${String(MAXIMUM_TIMELINE_JSON_BYTES)} bytes`,
    );
  }
  return snapshot;
}

function snapshotTimelineJsonValue(value: unknown, depth = 0, state = { nodes: 0 }): ProductBrowserLocalJson {
  if (depth > MAXIMUM_TIMELINE_JSON_DEPTH) {
    throw new TypeError('timeline opaque data exceeds the runtime-timeline depth bound');
  }
  state.nodes += 1;
  if (state.nodes > MAXIMUM_TIMELINE_JSON_NODES) {
    throw new TypeError('timeline opaque data exceeds the runtime-timeline node bound');
  }
  if (value === null || typeof value === 'boolean') return value;
  if (typeof value === 'string') {
    return requireBoundedString(value, 'timeline JSON string', MAXIMUM_TIMELINE_JSON_BYTES);
  }
  if (typeof value === 'number') return requireFiniteNumber(value, 'timeline JSON number');
  if (Array.isArray(value)) {
    const source = requirePlainArray(value, 'timeline JSON array');
    if (source.length > MAXIMUM_TIMELINE_JSON_ARRAY_LENGTH) {
      throw new TypeError('timeline opaque data array exceeds the runtime-timeline bound');
    }
    const entries: ProductBrowserLocalJson[] = [];
    for (let index = 0; index < source.length; index += 1) {
      const descriptor = Object.getOwnPropertyDescriptor(source, String(index));
      if (descriptor === undefined || !('value' in descriptor)) {
        throw new TypeError(`timeline JSON array entry ${String(index)} cannot be a getter or hole`);
      }
      entries.push(snapshotTimelineJsonValue(descriptor.value, depth + 1, state));
    }
    return Object.freeze(entries);
  }
  const record = requireRecord(value, 'timeline JSON object');
  const keys = Reflect.ownKeys(record);
  if (keys.length > MAXIMUM_TIMELINE_JSON_OBJECT_KEYS) {
    throw new TypeError('timeline opaque data object exceeds the runtime-timeline bound');
  }
  const result: Record<string, ProductBrowserLocalJson> = Object.create(null) as Record<string, ProductBrowserLocalJson>;
  for (const key of keys) {
    if (typeof key !== 'string') throw new TypeError('timeline JSON object cannot contain symbol keys');
    const descriptor = Object.getOwnPropertyDescriptor(record, key);
    if (descriptor === undefined || !('value' in descriptor)) {
      throw new TypeError(`timeline JSON object field ${key} cannot be a getter`);
    }
    result[key] = snapshotTimelineJsonValue(descriptor.value, depth + 1, state);
  }
  return Object.freeze(result);
}

function snapshotJsonValue(value: unknown, depth = 0): ProductBrowserLocalJson {
  if (depth > MAXIMUM_JSON_DEPTH) throw new TypeError('JSON value exceeds the transport depth bound');
  if (value === null || typeof value === 'boolean') return value;
  if (typeof value === 'string') return requireBoundedString(value, 'JSON string', MAXIMUM_JSON_STRING_BYTES);
  if (typeof value === 'number') return requireFiniteNumber(value, 'JSON number');
  if (Array.isArray(value)) {
    const source = requirePlainArray(value, 'JSON array');
    if (source.length > MAXIMUM_JSON_ARRAY_LENGTH) throw new TypeError('JSON array exceeds the transport bound');
    const entries: ProductBrowserLocalJson[] = [];
    for (let index = 0; index < source.length; index += 1) {
      const descriptor = Object.getOwnPropertyDescriptor(source, String(index));
      if (descriptor === undefined || !('value' in descriptor)) throw new TypeError(`JSON array entry ${String(index)} cannot be a getter or hole`);
      entries.push(snapshotJsonValue(descriptor.value, depth + 1));
    }
    return Object.freeze(entries);
  }
  const record = requireRecord(value, 'JSON object');
  const keys = Object.keys(record);
  if (keys.length > MAXIMUM_JSON_OBJECT_KEYS) throw new TypeError('JSON object exceeds the transport key bound');
  const result: Record<string, ProductBrowserLocalJson> = Object.create(null) as Record<string, ProductBrowserLocalJson>;
  for (const key of keys) result[key] = snapshotJsonValue(record[key], depth + 1);
  return Object.freeze(result);
}

function parseBoundedJson(value: string, maximumBytes: number): unknown {
  if (typeof value !== 'string' || new TextEncoder().encode(value).byteLength > maximumBytes) {
    throw new ProductBrowserLocalTransportError(
      'output_decode_failed',
      `Product Browser local runtime output exceeds ${String(maximumBytes)} bytes`,
      { route: ROUTES.outputs },
    );
  }
  try {
    return JSON.parse(value) as unknown;
  } catch (cause) {
    throw new ProductBrowserLocalTransportError(
      'output_decode_failed',
      'Product Browser local runtime output is not valid JSON',
      { cause, route: ROUTES.outputs },
    );
  }
}

function decodeOutputFragment(
  value: unknown,
  maximumAggregateBytes: number,
): {
  readonly transferId: string;
  readonly runtime: RustyApplicationRuntimeIdentity;
  readonly fragmentIndex: number;
  readonly fragmentCount: number;
  readonly aggregateBytes: number;
  readonly data: string;
} {
  const record = requireRecord(value, 'output fragment');
  requireKnownFields(record, [
    'schemaVersion', 'transferId', 'runtime', 'fragmentIndex', 'fragmentCount',
    'aggregateBytes', 'data',
  ], 'output fragment');
  if (record['schemaVersion'] !== 1) throw new TypeError('output fragment schemaVersion must equal 1');
  if (!Number.isSafeInteger(record['fragmentCount'])
    || (record['fragmentCount'] as number) < 2
    || (record['fragmentCount'] as number) > MAXIMUM_RUNTIME_OUTPUT_FRAGMENTS) {
    throw new TypeError('output fragment count is outside the retained stream bound');
  }
  const fragmentCount = record['fragmentCount'] as number;
  if (!Number.isSafeInteger(record['fragmentIndex'])
    || (record['fragmentIndex'] as number) < 0
    || (record['fragmentIndex'] as number) >= fragmentCount) {
    throw new TypeError('output fragment index is outside its transfer');
  }
  if (!Number.isSafeInteger(record['aggregateBytes'])
    || (record['aggregateBytes'] as number) <= MAXIMUM_RUNTIME_OUTPUT_EVENT_BYTES
    || (record['aggregateBytes'] as number) > maximumAggregateBytes) {
    throw new TypeError('output fragment aggregate length is outside the configured bound');
  }
  if (typeof record.data !== 'string') throw new TypeError('output fragment data must be text');
  const dataBytes = new TextEncoder().encode(record.data).byteLength;
  if (dataBytes === 0 || dataBytes > MAXIMUM_RUNTIME_OUTPUT_FRAGMENT_DATA_BYTES) {
    throw new TypeError('output fragment data is outside the event payload bound');
  }
  return {
    transferId: requireU64Text(record['transferId'], 'output fragment transferId'),
    runtime: decodeRuntimeIdentity(record.runtime),
    fragmentIndex: record['fragmentIndex'] as number,
    fragmentCount,
    aggregateBytes: record['aggregateBytes'] as number,
    data: record.data,
  };
}

function decodeOutputLagEvent(
  value: string,
  maximumBytes: number,
): ProductBrowserRuntimeTerminalFailure {
  const record = requireRecord(parseBoundedJson(value, maximumBytes), 'output-lag event');
  requireKnownFields(record, ['code'], 'output-lag event');
  if (record.code !== 'DEV_HOST_OUTPUT_LAG') {
    throw new ProductBrowserLocalTransportError(
      'output_decode_failed',
      'Product Browser local runtime output-lag event code is invalid',
      { route: ROUTES.outputs },
    );
  }
  return Object.freeze({
    kind: 'output-lag',
    diagnostic: 'Product Browser local runtime output stream lost retained output; a fresh snapshot is required',
  });
}

function decodeFault(
  record: ProductBrowserWireRecord,
  name: string,
): { readonly code: string; readonly disposition: ProductBrowserHostFaultDisposition } {
  if (record.accepted !== true && record.accepted !== false) {
    throw new TypeError(`${name} accepted must be boolean`);
  }
  const code = requireFaultCode(record.code, `${name} code`);
  const disposition = requireCatalogValue<ProductBrowserHostFaultDisposition>(
    record['disposition'],
    `${name} disposition`,
    HOST_FAULT_DISPOSITIONS,
  );
  if ((record.accepted === true) !== (disposition === 'accepted')) {
    throw new TypeError(`${name} accepted and disposition are incoherent`);
  }
  return { code, disposition };
}

function decodeOperationResult(
  value: unknown,
  expectedOperation: ProductBrowserRuntimeOperationKind,
): ProductBrowserRuntimeOperationResult {
  const record = requireRecord(value, 'operation result');
  requireKnownFields(record, ['accepted', 'code', 'disposition', 'operation', 'binding', 'nextInputSequence', 'admittedThrough', 'readout', 'diagnostic'], 'operation result');
  if (record.accepted !== true && record.accepted !== false) {
    throw new TypeError('accepted must be boolean');
  }
  if (record.operation !== expectedOperation) {
    throw new TypeError(`operation must be ${expectedOperation}`);
  }
  const fault = decodeFault(record, 'operation result');
  if ((record.binding === undefined) !== (record['nextInputSequence'] === undefined)) {
    throw new TypeError('operation binding and nextInputSequence must be present together');
  }
  if (record.accepted === false
    && (record.binding !== undefined || record['nextInputSequence'] !== undefined || record.readout !== undefined)
    && fault.disposition !== 'resync-required') {
    throw new TypeError('only resync-required operation results may include current binding, input cursor, or readout');
  }
  if (record.accepted === true && record.diagnostic !== undefined) {
    throw new TypeError('accepted operation result cannot include diagnostic');
  }
  return {
    accepted: record.accepted,
    ...fault,
    operation: expectedOperation,
    ...(record.binding === undefined ? {} : { binding: decodeRuntimeIdentity(record.binding) }),
    ...(record['nextInputSequence'] === undefined
      ? {}
      : { nextInputSequence: requireU64Text(record['nextInputSequence'], 'operation nextInputSequence') }),
    ...(record['admittedThrough'] === undefined
      ? {}
      : { admittedThrough: requireU64Text(record['admittedThrough'], 'operation admittedThrough') }),
    ...(record.readout === undefined ? {} : { readout: decodeRuntimeReadout(record.readout) }),
    ...(record.diagnostic === undefined ? {} : { diagnostic: requireDiagnostic(record.diagnostic) }),
  };
}

function decodeConnectionResult(value: unknown): ProductBrowserRuntimeOperationResult {
  const record = requireRecord(value, 'connection result');
  if (record.operation !== 'start' && record.operation !== 'connect') {
    throw new TypeError('connection operation must be start or connect');
  }
  return decodeOperationResult(value, record.operation);
}

function decodeInputResult(value: unknown): ProductBrowserRuntimeInputResult {
  const record = requireRecord(value, 'input result');
  requireKnownFields(record, ['accepted', 'code', 'disposition', 'count', 'acceptedCount', 'droppedCount', 'acceptedThrough', 'consumedThrough', 'nextInputSequence', 'binding', 'readout', 'diagnostic'], 'input result');
  if (record.accepted !== true && record.accepted !== false) {
    throw new TypeError('accepted must be boolean');
  }
  if (!Number.isSafeInteger(record.count) || (record.count as number) < 0) {
    throw new TypeError(`count must be a non-negative integer no greater than ${String(MAXIMUM_INPUT_BATCH_LENGTH)}`);
  }
  const fault = decodeFault(record, 'input result');
  // The Rust host rejects strict-decode failures before wire admission. That
  // response preserves the host-bounded submitted count diagnostically, so it
  // can exceed the smaller outgoing/admitted batch limit without widening it.
  if ((record.count as number) > MAXIMUM_INPUT_BATCH_LENGTH
    && (record.accepted !== false || fault.disposition !== 'resync-required')) {
    throw new TypeError(`count must be a non-negative integer no greater than ${String(MAXIMUM_INPUT_BATCH_LENGTH)}`);
  }
  if (record['acceptedCount'] !== undefined
    && (!Number.isSafeInteger(record['acceptedCount'])
      || (record['acceptedCount'] as number) < 0
      || (record['acceptedCount'] as number) > (record.count as number))) {
    throw new TypeError('acceptedCount must be a non-negative integer no greater than count');
  }
  if (record['droppedCount'] !== undefined
    && (!Number.isSafeInteger(record['droppedCount'])
      || (record['droppedCount'] as number) < 0
      || (record['droppedCount'] as number) > (record.count as number))) {
    throw new TypeError('droppedCount must be a non-negative integer no greater than count');
  }
  const acceptedCount = record['acceptedCount'] === undefined
    ? (record.accepted ? record.count as number : 0)
    : record['acceptedCount'] as number;
  const droppedCount = record['droppedCount'] === undefined ? 0 : record['droppedCount'] as number;
  if (acceptedCount + droppedCount !== (record.count as number)) {
    throw new TypeError('input result acceptedCount and droppedCount must account for count');
  }
  if (record.accepted === false
    && (record.binding !== undefined || record.readout !== undefined || record['nextInputSequence'] !== undefined)
    && (fault.disposition !== 'rejected-recoverable' && fault.disposition !== 'resync-required')) {
    throw new TypeError('only recoverable input results may include current binding, input cursor, or readout');
  }
  if (record['nextInputSequence'] !== undefined && record.binding === undefined) {
    throw new TypeError('input nextInputSequence requires a runtime binding');
  }
  if (record.accepted === false
    && (record.binding !== undefined || record.readout !== undefined)
    && record['nextInputSequence'] === undefined) {
    throw new TypeError('recoverable input result with current state must include nextInputSequence');
  }
  if ((record.binding !== undefined) !== (record.readout !== undefined)) {
    throw new TypeError('input binding and readout must be present together');
  }
  const acceptedThrough = record['acceptedThrough'] === undefined
    ? undefined
    : requireU64Text(record['acceptedThrough'], 'input acceptedThrough');
  const consumedThrough = record['consumedThrough'] === undefined
    ? undefined
    : requireU64Text(record['consumedThrough'], 'input consumedThrough');
  if (acceptedCount === 0 && acceptedThrough !== undefined) {
    throw new TypeError('input acceptedThrough requires an accepted event');
  }
  if (acceptedThrough !== undefined && consumedThrough !== undefined
    && BigInt(acceptedThrough) > BigInt(consumedThrough)) {
    throw new TypeError('input acceptedThrough cannot exceed consumedThrough');
  }
  if (record.accepted === true && record.diagnostic !== undefined) {
    throw new TypeError('accepted input result cannot include diagnostic');
  }
  return {
    accepted: record.accepted,
    ...fault,
    count: record.count as number,
    acceptedCount,
    droppedCount,
    ...(acceptedThrough === undefined ? {} : { acceptedThrough }),
    ...(consumedThrough === undefined ? {} : { consumedThrough }),
    ...(record['nextInputSequence'] === undefined
      ? {}
      : { nextInputSequence: requireU64Text(record['nextInputSequence'], 'input nextInputSequence') }),
    ...(record.binding === undefined ? {} : { binding: decodeRuntimeIdentity(record.binding) }),
    ...(record.readout === undefined ? {} : { readout: decodeRuntimeReadout(record.readout) }),
    ...(record.diagnostic === undefined ? {} : { diagnostic: requireDiagnostic(record.diagnostic) }),
  };
}

function decodeAudioFeedbackResult(
  value: unknown,
  expectedRuntime: RustyApplicationRuntimeIdentity,
  submittedFacts: readonly ProductBrowserAudioFeedbackFact[],
): ProductBrowserAudioFeedbackResult {
  const record = requireRecord(value, 'audio feedback result');
  requireKnownFields(record, ['accepted', 'code', 'disposition', 'runtime', 'acceptedThroughFactId', 'diagnostic'], 'audio feedback result');
  if (record.accepted !== true && record.accepted !== false) {
    throw new TypeError('audio feedback accepted must be boolean');
  }
  const runtime = decodeRuntimeIdentity(record.runtime);
  const fault = decodeFault(record, 'audio feedback result');
  if (!sameRuntimeIdentity(runtime, expectedRuntime)) {
    throw new TypeError('audio feedback result runtime does not match request runtime');
  }
  const expectedThroughFactId = submittedFacts.length === 0
    ? undefined
    : submittedFacts[submittedFacts.length - 1]!.factId;
  if (record.accepted === false) {
    if (record.acceptedThroughFactId !== undefined) {
      throw new TypeError('rejected audio feedback cannot include acceptedThroughFactId');
    }
    return Object.freeze({
      accepted: false,
      ...fault,
      runtime,
      ...(record.diagnostic === undefined ? {} : { diagnostic: requireDiagnostic(record.diagnostic) }),
    });
  }
  if (record.diagnostic !== undefined) throw new TypeError('accepted audio feedback cannot include diagnostic');
  if (expectedThroughFactId === undefined) {
    if (record.acceptedThroughFactId !== undefined) {
      throw new TypeError('empty audio feedback cannot include acceptedThroughFactId');
    }
    return Object.freeze({ accepted: true, ...fault, runtime });
  }
  const acceptedThroughFactId = requireU64Text(
    record.acceptedThroughFactId,
    'audio feedback acceptedThroughFactId',
  );
  if (acceptedThroughFactId !== expectedThroughFactId) {
    throw new TypeError('audio feedback acknowledgement boundary does not match submitted facts');
  }
  return Object.freeze({ accepted: true, ...fault, runtime, acceptedThroughFactId });
}

function decodeGhostPlateFeedbackResult(
  value: unknown,
  expectedRuntime: RustyApplicationRuntimeIdentity,
): ProductBrowserGhostPlateFeedbackResult {
  const record = requireRecord(value, 'ghost plate feedback result');
  requireKnownFields(record, ['accepted', 'code', 'disposition', 'runtime', 'diagnostic'], 'ghost plate feedback result');
  if (record.accepted !== true && record.accepted !== false) throw new TypeError('ghost plate feedback accepted must be boolean');
  const runtime = decodeRuntimeIdentity(record.runtime);
  const fault = decodeFault(record, 'ghost plate feedback result');
  if (!sameRuntimeIdentity(runtime, expectedRuntime)) throw new TypeError('ghost plate feedback result runtime does not match request runtime');
  if (record.accepted) {
    if (record.diagnostic !== undefined) throw new TypeError('accepted ghost plate feedback cannot include diagnostic');
    return Object.freeze({ accepted: true, ...fault, runtime });
  }
  return Object.freeze({
    accepted: false,
    ...fault,
    runtime,
    ...(record.diagnostic === undefined ? {} : { diagnostic: requireDiagnostic(record.diagnostic) }),
  });
}

function decodeRendererDiagnosticsResult(
  value: unknown,
  expectedRuntime: RustyApplicationRuntimeIdentity,
): ProductBrowserRendererDiagnosticsFeedbackResult {
  const record = requireRecord(value, 'renderer diagnostics result');
  requireKnownFields(record, ['accepted', 'code', 'disposition', 'runtime', 'diagnostic'], 'renderer diagnostics result');
  if (record.accepted !== true && record.accepted !== false) {
    throw new TypeError('renderer diagnostics accepted must be boolean');
  }
  const runtime = decodeRuntimeIdentity(record.runtime);
  const fault = decodeFault(record, 'renderer diagnostics result');
  if (!sameRuntimeIdentity(runtime, expectedRuntime)) {
    throw new TypeError('renderer diagnostics result runtime does not match request runtime');
  }
  const diagnostic = record.diagnostic === undefined
    ? undefined
    : requireDiagnostic(record.diagnostic);
  if (record.accepted && diagnostic !== undefined) {
    throw new TypeError('accepted renderer diagnostics cannot include diagnostic');
  }
  return Object.freeze({ accepted: record.accepted, ...fault, runtime, ...(diagnostic === undefined ? {} : { diagnostic }) });
}

function decodeBrowserDiagnosticsResult(value: unknown): ProductBrowserDiagnosticsResult {
  const record = requireRecord(value, 'browser diagnostics result');
  requireKnownFields(record, ['accepted', 'reported'], 'browser diagnostics result');
  if (record.accepted !== true || !Number.isSafeInteger(record.reported) || (record.reported as number) < 1 || (record.reported as number) > 10) {
    throw new TypeError('browser diagnostics result is invalid');
  }
  return Object.freeze({ accepted: true, reported: record.reported as number });
}

function decodeAnimationFeedbackResult(
  value: unknown,
  expectedRuntime: RustyApplicationRuntimeIdentity,
  submittedFacts: readonly ProductBrowserAnimationFeedbackFact[],
): ProductBrowserAnimationFeedbackResult {
  const record = requireRecord(value, 'animation feedback result');
  requireKnownFields(record, ['accepted', 'code', 'disposition', 'runtime', 'acceptedThroughFactId', 'diagnostic'], 'animation feedback result');
  if (record.accepted !== true && record.accepted !== false) throw new TypeError('animation feedback accepted must be boolean');
  const runtime = decodeRuntimeIdentity(record.runtime);
  const fault = decodeFault(record, 'animation feedback result');
  if (!sameRuntimeIdentity(runtime, expectedRuntime)) throw new TypeError('animation feedback result runtime does not match request runtime');
  const expectedThroughFactId = submittedFacts.length === 0 ? undefined : submittedFacts[submittedFacts.length - 1]!.factId;
  if (!record.accepted) {
    if (record.acceptedThroughFactId !== undefined) throw new TypeError('rejected animation feedback cannot include acceptedThroughFactId');
    return Object.freeze({ accepted: false, ...fault, runtime, ...(record.diagnostic === undefined ? {} : { diagnostic: requireDiagnostic(record.diagnostic) }) });
  }
  if (record.diagnostic !== undefined) throw new TypeError('accepted animation feedback cannot include diagnostic');
  if (expectedThroughFactId === undefined) {
    if (record.acceptedThroughFactId !== undefined) throw new TypeError('empty animation feedback cannot include acceptedThroughFactId');
    return Object.freeze({ accepted: true, ...fault, runtime });
  }
  const acceptedThroughFactId = requireU64Text(record.acceptedThroughFactId, 'animation feedback acceptedThroughFactId');
  if (acceptedThroughFactId !== expectedThroughFactId) throw new TypeError('animation feedback acknowledgement boundary does not match submitted facts');
  return Object.freeze({ accepted: true, ...fault, runtime, acceptedThroughFactId });
}

function decodeTimelineCompletionResult(
  value: unknown,
  expectedTicket: string,
): ProductBrowserTimelineCompletionResult {
  const record = requireRecord(value, 'timeline completion result');
  requireKnownFields(record, ['accepted', 'code', 'disposition', 'ticket', 'binding', 'readout', 'diagnostic'], 'timeline completion result');
  if (record.accepted !== true && record.accepted !== false) {
    throw new TypeError('accepted must be boolean');
  }
  const ticket = requireU64Text(record.ticket, 'timeline result ticket');
  const fault = decodeFault(record, 'timeline completion result');
  if (ticket !== expectedTicket) throw new TypeError('ticket does not match completion request');
  if ((record.binding === undefined) !== (record.readout === undefined)) {
    throw new TypeError('timeline binding and readout must be present together');
  }
  if (record.accepted === true && record.diagnostic !== undefined) {
    throw new TypeError('accepted timeline result cannot include diagnostic');
  }
  const binding = record.binding === undefined ? undefined : decodeRuntimeIdentity(record.binding);
  const readout = record.readout === undefined ? undefined : decodeRuntimeReadout(record.readout);
  if (binding !== undefined && readout !== undefined && !sameRuntimeIdentity(binding, readout.runtime)) {
    throw new TypeError('timeline result binding does not match its readout');
  }
  return {
    accepted: record.accepted,
    ...fault,
    ticket,
    ...(binding === undefined ? {} : { binding }),
    ...(readout === undefined ? {} : { readout }),
    ...(record.diagnostic === undefined ? {} : { diagnostic: requireDiagnostic(record.diagnostic) }),
  };
}

function decodeRuntimeOutput(value: unknown): ProductBrowserRuntimeOutput {
  const record = requireRecord(value, 'runtime output');
  switch (record.kind) {
    case 'binding':
      requireKnownFields(record, ['kind', 'runtime', 'nextInputSequence'], 'binding output');
      return {
        kind: 'binding',
        runtime: decodeRuntimeIdentity(record.runtime),
        nextInputSequence: requireU64Text(record['nextInputSequence'], 'binding nextInputSequence'),
      };
    case 'frame':
      requireKnownFields(record, ['kind', 'frame'], 'frame output');
      return { kind: 'frame', frame: decodeFrame(record.frame, 'frame') };
    case 'view-composition':
      requireKnownFields(record, ['kind', 'composition'], 'view composition output');
      return { kind: 'view-composition', composition: decodeViewComposition(record.composition) };
    case 'animation-cue-definitions':
      requireKnownFields(record, ['kind', 'definitions'], 'animation cue definitions output');
      return { kind: 'animation-cue-definitions', definitions: decodeAnimationCueDefinitions(record['definitions']) };
    case 'presentation':
      requireKnownFields(record, ['kind', 'frame'], 'presentation output');
      return { kind: 'presentation', frame: decodeFrame(record.frame, 'presentation') };
    case 'ui-projection':
      requireKnownFields(record, ['kind', 'envelope'], 'UI projection output');
      return { kind: 'ui-projection', envelope: decodeUiProjection(record.envelope) };
    case 'runtime-readout':
      requireKnownFields(record, ['kind', 'readout'], 'runtime readout output');
      return { kind: 'runtime-readout', readout: decodeRuntimeReadout(record.readout) };
    case 'runtime-progress':
      requireKnownFields(record, ['kind', 'owner'], 'runtime progress output');
      if (record['owner'] !== 'rust-host') {
        throw new TypeError('runtime progress owner is invalid');
      }
      return { kind: 'runtime-progress', owner: 'rust-host' };
    case 'runtime-input-result':
      requireKnownFields(record, ['kind', 'result'], 'runtime input result output');
      return {
        kind: 'runtime-input-result',
        result: decodeInputResult(record['result']),
      };
    default:
      throw new TypeError('runtime output kind is not admitted');
  }
}

function decodeRuntimeOutputBatch(value: unknown): readonly ProductBrowserRuntimeOutput[] {
  const record = requireRecord(value, 'runtime output');
  if (record.kind !== 'runtime-output-batch') {
    return Object.freeze([decodeRuntimeOutput(value)]);
  }
  requireKnownFields(record, ['kind', 'outputs'], 'runtime output batch');
  const outputs = requirePlainArray(record['outputs'], 'runtime output batch outputs');
  if (outputs.length === 0 || outputs.length > MAXIMUM_CONNECTION_BASELINE_OUTPUTS) {
    throw new TypeError(
      `runtime output batch must contain 1 to ${String(MAXIMUM_CONNECTION_BASELINE_OUTPUTS)} outputs`,
    );
  }
  return Object.freeze(outputs.map(decodeRuntimeOutput));
}

function decodeAnimationCueDefinitions(
  value: unknown,
): readonly RustyApplicationAnimationCueDefinition[] {
  const values = requirePlainArray(value, 'animation cue definitions');
  if (values.length > MAXIMUM_ANIMATION_CUE_DEFINITIONS) {
    throw new TypeError('animation cue definition replacement exceeds 128 definitions');
  }
  const keys = new Set<string>();
  return Object.freeze(values.map((value) => {
    const record = requireRecord(value, 'animation cue definition');
    requireKnownFields(
      record,
      ['cueId', 'asset', 'clip', 'atSeconds', 'signalDomain', 'signalId'],
      'animation cue definition',
    );
    const cueId = requireBoundedString(
      record['cueId'],
      'animation cue id',
      MAXIMUM_ANIMATION_CUE_TEXT_BYTES,
    );
    const asset = requireBoundedString(
      record['asset'],
      'animation cue asset',
      MAXIMUM_ANIMATION_CUE_TEXT_BYTES,
    );
    const clip = requireBoundedString(
      record['clip'],
      'animation cue clip',
      MAXIMUM_ANIMATION_CUE_TEXT_BYTES,
    );
    const signalId = requireBoundedString(
      record['signalId'],
      'animation cue signal id',
      MAXIMUM_ANIMATION_CUE_TEXT_BYTES,
    );
    const signalDomain = requireCatalogValue<'audio' | 'particle'>(
      record['signalDomain'],
      'animation cue signal domain',
      new Set(['audio', 'particle']),
    );
    const atSeconds = requireFiniteNumber(
      record['atSeconds'],
      'animation cue marker',
      0,
      Number.MAX_VALUE,
    );
    const key = JSON.stringify([asset, clip, cueId]);
    if (keys.has(key)) throw new TypeError(`duplicate animation cue definition ${key}`);
    keys.add(key);
    return Object.freeze({
      cueId,
      asset,
      clip,
      atSeconds,
      signal: Object.freeze({ domain: signalDomain, id: signalId }),
    });
  }));
}

function decodeViewComposition(value: unknown): RendererViewComposition {
  return validateRendererViewComposition(value as RendererViewComposition);
}

function decodeFrame(value: unknown, name: string): RustyApplicationFrame | RustyApplicationPresentationFrame {
  const record = requireRecord(value, name);
  return record as RustyApplicationFrame;
}

function decodeUiProjection(value: unknown): RustyApplicationUiProjectionEnvelope {
  const record = requireRecord(value, 'UI projection');
  requireKnownFields(record, ['artifact', 'runtime', 'sequence', 'stream', 'contract', 'value'], 'UI projection');
  if (record.artifact !== 'rusty.product.ui-projection') throw new TypeError('UI projection artifact is invalid');
  return {
    artifact: 'rusty.product.ui-projection',
    runtime: decodeRuntimeIdentity(record.runtime),
    sequence: requireU64Text(record.sequence, 'sequence'),
    stream: requireIdentity(record.stream, 'stream'),
    contract: requireIdentity(record.contract, 'contract'),
    value: snapshotJsonValue(record.value) as RustyApplicationUiProjectionEnvelope['value'],
  };
}

function decodeRuntimeIdentity(value: unknown): RustyApplicationRuntimeIdentity {
  const record = requireRecord(value, 'runtime identity');
  requireKnownFields(record, ['instanceId', 'generation', 'controlRevision'], 'runtime identity');
  return {
    instanceId: requireU64Text(record.instanceId, 'instanceId'),
    generation: requireU64Text(record.generation, 'generation'),
    controlRevision: requireU64Text(record.controlRevision, 'controlRevision'),
  };
}

function sameRuntimeIdentity(
  left: RustyApplicationRuntimeIdentity,
  right: RustyApplicationRuntimeIdentity,
): boolean {
  return left.instanceId === right.instanceId
    && left.generation === right.generation
    && left.controlRevision === right.controlRevision;
}

function decodeRuntimeReadout(value: unknown): ProductBrowserRuntimeReadout {
  const record = requireRecord(value, 'runtime readout');
  requireKnownFields(record, [
    'artifact',
    'runtime',
    'mode',
    'state',
    'admittedSimulationSteps',
    'admittedPresentations',
    'droppedRealtimeSteps',
    'clockRegressions',
    'scaledRemainder',
    'lastObservedTimeNs',
    'fault',
  ], 'runtime readout');
  if (record.artifact !== 'rusty.product.runtime-readout') throw new TypeError('runtime readout artifact is invalid');
  const mode = record.mode;
  if (mode !== 'realtime' && mode !== 'demand' && mode !== 'external') throw new TypeError('runtime readout mode is invalid');
  const state = record.state;
  if (state !== 'created' && state !== 'running' && state !== 'paused' && state !== 'faulted' && state !== 'shutdown') {
    throw new TypeError('runtime readout state is invalid');
  }
  const fault = record.fault;
  if (fault !== null && fault !== 'owner-reported' && fault !== 'counter-exhausted') {
    throw new TypeError('runtime readout fault is invalid');
  }
  if (record.scaledRemainder !== null
    && (!Number.isSafeInteger(record.scaledRemainder)
      || (record.scaledRemainder as number) < 0
      || (record.scaledRemainder as number) > 4_294_967_295)) {
    throw new TypeError('runtime readout scaledRemainder must be a u32 or null');
  }
  return {
    artifact: 'rusty.product.runtime-readout',
    runtime: decodeRuntimeIdentity(record.runtime),
    mode,
    state,
    admittedSimulationSteps: requireU64Text(record.admittedSimulationSteps, 'admittedSimulationSteps'),
    admittedPresentations: requireU64Text(record.admittedPresentations, 'admittedPresentations'),
    droppedRealtimeSteps: requireU64Text(record.droppedRealtimeSteps, 'droppedRealtimeSteps'),
    clockRegressions: requireU64Text(record.clockRegressions, 'clockRegressions'),
    scaledRemainder: record.scaledRemainder as number | null,
    lastObservedTimeNs: record.lastObservedTimeNs === null
      ? null
      : requireU64Text(record.lastObservedTimeNs, 'lastObservedTimeNs'),
    fault,
  };
}

function requireRecord(value: unknown, name: string): ProductBrowserWireRecord {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new TypeError(`${name} must be an object`);
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new TypeError(`${name} must be a plain object`);
  }
  for (const key of Reflect.ownKeys(value)) {
    if (typeof key !== 'string') throw new TypeError(`${name} cannot contain symbol keys`);
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (descriptor === undefined || !('value' in descriptor) || descriptor.enumerable !== true) {
      throw new TypeError(`${name} cannot contain getters or non-enumerable fields`);
    }
  }
  return value as ProductBrowserWireRecord;
}

function requirePlainArray(value: unknown, name: string): readonly unknown[] {
  if (!Array.isArray(value) || Object.getPrototypeOf(value) !== Array.prototype) {
    throw new TypeError(`${name} must be a plain array`);
  }
  const lengthDescriptor = Object.getOwnPropertyDescriptor(value, 'length');
  if (lengthDescriptor === undefined
    || !('value' in lengthDescriptor)
    || lengthDescriptor.value !== value.length
    || lengthDescriptor.enumerable !== false
    || lengthDescriptor.configurable !== false) {
    throw new TypeError(`${name} has a non-canonical length descriptor`);
  }
  for (const key of Reflect.ownKeys(value)) {
    if (key === 'length') continue;
    if (typeof key !== 'string' || !/^(?:0|[1-9]\d*)$/u.test(key)) {
      throw new TypeError(`${name} contains an extra property`);
    }
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (descriptor === undefined
      || !('value' in descriptor)
      || descriptor.enumerable !== true) {
      throw new TypeError(`${name} contains a non-canonical array entry descriptor`);
    }
  }
  return value;
}

function requireKnownFields(
  record: ProductBrowserWireRecord,
  allowed: readonly string[],
  name: string,
): void {
  for (const key of Reflect.ownKeys(record)) {
    if (typeof key !== 'string') throw new TypeError(`${name} cannot contain symbol keys`);
    if (!allowed.includes(key)) throw new TypeError(`${name} contains unknown field ${key}`);
  }
}

function requireDiagnostic(value: unknown): string {
  if (typeof value !== 'string' || value.length === 0
    || new TextEncoder().encode(value).byteLength > 1_024) {
    throw new TypeError('diagnostic must be a non-empty string no greater than 1024 bytes');
  }
  return value;
}
