import { browserAttachmentEvidence } from './attachment-evidence.js';
import type { RustyApplicationRuntimeIdentity } from '@rusty-engine/application-host';
import {
  RUNTIME_BASE_PATH,
  type ProductDevBrowserDiagnosticsReport,
  type ProductDevBrowserDiagnosticsResult,
  type ProductDevConnectionBaseline,
  type ProductDevControlRequest,
  type ProductDevEmptyRequest,
  type ProductDevExternalRequest,
  type ProductDevInputRequest,
  type ProductDevInputResult,
  type ProductDevLifecycleRequest,
  type ProductDevOperationResult,
  type ProductDevRealtimeRequest,
  type ProductDevRuntimeOutput,
  type ProductDevTimelineCompletion,
  type ProductDevTimelineCompletionResult,
  type RuntimeInputWireEvent,
} from './generated/contracts.js';
import type {
  ProductBrowserLifecycleOperation,
  ProductBrowserRuntimeAdapter,
  ProductBrowserRuntimeOutputBatchListener,
  ProductBrowserRuntimeOutputBatchMetadata,
  ProductBrowserRuntimeTerminalFailure,
  ProductBrowserRuntimeTerminalFailureListener,
} from './product-browser-host.js';

/** Fixed identity for the Engine-owned browser-to-local-runtime transport. */
export const PRODUCT_BROWSER_LOCAL_TRANSPORT_ARTIFACT =
  'rusty.product.local-runtime-transport' as const;

/**
 * The runtime's operation routes. The endpoint is deliberately an
 * operation-specific route set, rather than a method-name RPC endpoint or a
 * generic message tunnel.
 */
const ROUTES = Object.freeze({
  lifecycle: Object.freeze({
    start: 'lifecycle/start',
    pause: 'lifecycle/pause',
    resume: 'lifecycle/resume',
    restart: 'lifecycle/restart',
    shutdown: 'lifecycle/shutdown',
    'report-fault': 'lifecycle/report-fault',
  }),
  control: Object.freeze({
    replace: 'control/replace',
  }),
  input: 'input',
  advanceRealtime: 'advance-realtime',
  admitDemandStep: 'admit-demand-step',
  admitExternalStep: 'admit-external-step',
  completeTimeline: 'timeline-completion',
  browserDiagnostics: 'browser-diagnostics',
  outputs: 'outputs',
  freshOutputs: 'outputs/fresh',
});

const EVENT_SOURCE_CLOSED = 2;
const FRESH_RETRY_INITIAL_DELAY_MS = 250;
const FRESH_RETRY_MAX_DELAY_MS = 2_000;

export type ProductBrowserLocalFetch = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>;

/** Minimal EventSource shape kept injectable for deterministic headless tests. */
export interface ProductBrowserLocalEventSource {
  onopen: ((event: unknown) => void) | null;
  onmessage: ((event: { readonly data: string; readonly lastEventId: string }) => void) | null;
  onerror: ((event: unknown) => void) | null;
  /** `2` (CLOSED) once the browser has stopped reconnecting this stream. */
  readonly readyState?: number;
  readonly addEventListener?: (
    type: 'rusty-output-baseline' | 'rusty-ui-reloaded',
    listener: (event: { readonly data: string; readonly lastEventId: string }) => void,
  ) => void;
  readonly removeEventListener?: (
    type: 'rusty-output-baseline' | 'rusty-ui-reloaded',
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
  /** Stream errors are surfaced here; the operation surface remains closed. */
  readonly onTransportError?: (error: ProductBrowserLocalTransportError) => void;
  /**
   * Reloads the page when its UI module may be out of date: the host swapped
   * the served UI, or a new runtime incarnation answered. Injectable for tests;
   * defaults to `location.reload()`.
   */
  readonly reloadPage?: () => void;
}

export type ProductBrowserLocalTransportErrorCode =
  | 'invalid_options'
  | 'disposed'
  | 'request_failed'
  | 'response_decode_failed'
  | 'output_decode_failed'
  | 'stream_failed';

/**
 * What the browser can prove about a mutating request after an error.
 *
 * `outcome-unknown` is deliberately not retry advice: callers must fence and
 * rebaseline state before sending another mutation. A committed response can
 * still require an output resynchronization when its headers say so. Output
 * projection consumes that resynchronization in #7761; this transport only
 * preserves the proof when response-body delivery later fails.
 */
export interface ProductBrowserLocalTransportMutationState {
  readonly certainty: 'outcome-unknown' | 'not-applied' | 'committed';
  readonly outputRecovery: 'none' | 'fresh-baseline-required';
  /** Canonical retained-output cursor observed with the response, if any. */
  readonly outputThrough: string | null;
}

const UNKNOWN_MUTATION: ProductBrowserLocalTransportMutationState = Object.freeze({
  certainty: 'outcome-unknown',
  outputRecovery: 'none',
  outputThrough: null,
});

const NOT_APPLIED_MUTATION: ProductBrowserLocalTransportMutationState = Object.freeze({
  certainty: 'not-applied',
  outputRecovery: 'none',
  outputThrough: null,
});

export class ProductBrowserLocalTransportError extends Error {
  readonly code: ProductBrowserLocalTransportErrorCode;
  readonly route: string | null;
  /** Explicit request-outcome and required output-recovery posture. */
  readonly mutation: ProductBrowserLocalTransportMutationState;

  constructor(
    code: ProductBrowserLocalTransportErrorCode,
    message: string,
    options?: ErrorOptions & {
      readonly route?: string;
      readonly mutation?: ProductBrowserLocalTransportMutationState;
    },
  ) {
    super(message, options);
    this.name = 'ProductBrowserLocalTransportError';
    this.code = code;
    this.route = options?.route ?? null;
    this.mutation = options?.mutation ?? NOT_APPLIED_MUTATION;
  }
}

type ProductBrowserCommitDisposition = 'not-applied' | 'unknown' | 'committed' | 'resync-required';

function decodeCommitDisposition(
  headers: Headers,
  route: string,
): ProductBrowserCommitDisposition {
  const disposition = headers.get('x-rusty-commit-disposition');
  const resync = headers.get('x-rusty-resync-outputs');
  if (disposition === null && resync === null) {
    throw new ProductBrowserLocalTransportError(
      'response_decode_failed',
      `Product Browser local runtime response for ${route} omitted its commit disposition`,
      { route, mutation: UNKNOWN_MUTATION },
    );
  }
  if (disposition === null) throw new ProductBrowserLocalTransportError(
    'response_decode_failed',
    `Product Browser local runtime response for ${route} named a resync without a commit disposition`,
    { route, mutation: UNKNOWN_MUTATION },
  );
  if ((disposition === 'committed' || disposition === 'not-applied' || disposition === 'unknown')
    && resync === null) return disposition;
  if (disposition === 'resync-required' && resync === 'fresh') return disposition;
  throw new ProductBrowserLocalTransportError(
    'response_decode_failed',
    `Product Browser local runtime response for ${route} has an unknown or incoherent commit disposition`,
    { route, mutation: UNKNOWN_MUTATION },
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
  const basePath = options.basePath ?? RUNTIME_BASE_PATH;
  const attachment = browserAttachmentEvidence(basePath);
  const fetchImpl = options.fetch ?? resolveFetch();
  const eventSourceConstructor = options.eventSource ?? resolveEventSource();
  const reloadPage = options.reloadPage ?? (() => globalThis.location.reload());
  let disposed = false;
  let stream: ProductBrowserLocalEventSource | null = null;
  let streamBaselineListener: ((event: { readonly data: string; readonly lastEventId: string }) => void) | null = null;
  let currentOutputBinding: RustyApplicationRuntimeIdentity | null = null;
  // The runtime incarnation this page's UI module was loaded against.
  let pageInstanceId: string | null = null;
  let outputSubscriptionReady: Promise<void> | null = null;
  let resolveOutputSubscriptionReady: (() => void) | null = null;
  let connectionReady: Promise<ProductDevOperationResult> | null = null;
  let resolveConnectionReady: ((result: ProductDevOperationResult) => void) | null = null;
  let rejectConnectionReady: ((error: ProductBrowserLocalTransportError) => void) | null = null;
  let connectionBaselineComplete = false;
  let pendingConnectionOutputs: ProductDevRuntimeOutput[] = [];
  let terminalFailure: ProductBrowserRuntimeTerminalFailure | null = null;
  let nextOutputEpoch = 0;
  let currentOutputEpoch = 0;
  let freshOutputRecovery: Promise<void> | null = null;
  let observedOutputSequence = 0n;
  const outputSequenceWaiters = new Set<{
    readonly through: bigint;
    readonly resolve: (outcome: 'observed' | 'fresh-baseline' | 'closed') => void;
  }>();
  const listeners = new Set<(output: ProductDevRuntimeOutput) => void>();
  const batchListeners = new Set<ProductBrowserRuntimeOutputBatchListener>();
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

  const releaseOutputSequenceWaiters = (
    outcome: 'fresh-baseline' | 'closed',
  ): void => {
    for (const waiter of [...outputSequenceWaiters]) {
      outputSequenceWaiters.delete(waiter);
      waiter.resolve(outcome);
    }
  };

  const settleOutputSequenceWaiters = (): void => {
    for (const waiter of [...outputSequenceWaiters]) {
      if (waiter.through > observedOutputSequence) continue;
      outputSequenceWaiters.delete(waiter);
      waiter.resolve('observed');
    }
  };

  const observeOutputSequence = (value: string): void => {
    const sequence = BigInt(value);
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
    const outcome = await new Promise<'observed' | 'fresh-baseline' | 'closed'>((resolve) => {
      outputSequenceWaiters.add({ through, resolve });
    });
    // A fenced fresh baseline is a complete retained projection at the
    // moment the old cursor became unusable. It therefore satisfies an
    // already-committed response boundary without replaying that operation.
    if (outcome === 'fresh-baseline') return;
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
      if (streamBaselineListener !== null) {
        stream.removeEventListener?.('rusty-output-baseline', streamBaselineListener);
        streamBaselineListener = null;
      }
      stream.close();
      stream = null;
    }
    pendingConnectionOutputs = [];
    releaseOutputSequenceWaiters('closed');
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

  const reconnectFreshOutputsOnce = async (): Promise<void> => {
    if (stream === null || listeners.size === 0 || !connectionBaselineComplete) {
      throw new ProductBrowserLocalTransportError(
        'stream_failed',
        'Product Browser local runtime cannot resync a committed response without an established output subscription',
        { route: ROUTES.freshOutputs },
      );
    }
    if (streamBaselineListener !== null) stream.removeEventListener?.('rusty-output-baseline', streamBaselineListener);
    stream.close();
    stream = null;
    streamBaselineListener = null;
    pendingConnectionOutputs = [];
    currentOutputBinding = null;
    connectionBaselineComplete = false;
    observedOutputSequence = 0n;
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

  const publishFreshBaselineRequired = (): void => {
    publishOutputBatch([], {
      epoch: currentOutputEpoch,
      baseline: false,
      recovery: 'fresh-baseline-required',
    });
  };

  // Output lag and resync-required responses can arrive together. Keep one
  // replacement attach in flight so they cannot race two baseline streams.
  const reconnectFreshOutputs = (): Promise<void> => {
    if (freshOutputRecovery !== null) return freshOutputRecovery;
    let recovery: Promise<void>;
    recovery = reconnectFreshOutputsOnce().finally(() => {
      if (freshOutputRecovery === recovery) freshOutputRecovery = null;
    });
    freshOutputRecovery = recovery;
    // The stream callback owns reporting a failed recovery; this keeps the
    // shared promise from becoming an unhandled rejection for output-only
    // consumers.
    void recovery.catch(() => undefined);
    return recovery;
  };

  const recoverFreshOutputsOrTerminal = async (route: string): Promise<void> => {
    publishFreshBaselineRequired();
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
  };

  const post = async <Body, Result>(
    route: string,
    body: Body,
    allowAfterDispose = false,
  ): Promise<Result> => {
    if (!allowAfterDispose) ensureOpen();
    const url = `${basePath}${route}`;
    const outputEpochAtRequest = currentOutputEpoch;
    const encodedBody = JSON.stringify(body);
    let response: Response;
    try {
      response = await fetchImpl(url, {
        method: 'POST',
        credentials: 'same-origin',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
          'x-rusty-browser-attachment': attachment.read().id,
        },
        body: encodedBody,
        ...(allowAfterDispose ? {} : { signal: abortController.signal }),
      });
    } catch (cause) {
      throw new ProductBrowserLocalTransportError(
        'request_failed',
        `Product Browser local runtime request failed for ${route}: ${cause instanceof Error ? cause.message : String(cause)}`,
        { cause, route, mutation: UNKNOWN_MUTATION },
      );
    }
    // A non-OK HTTP status is an explicit rejection, regardless of a missing
    // or malformed optional commit header and regardless of later body I/O.
    if (!response.ok) {
      throw new ProductBrowserLocalTransportError(
        'request_failed',
        `Product Browser local runtime rejected ${route} with HTTP ${String(response.status)}`,
        { route, mutation: NOT_APPLIED_MUTATION },
      );
    }
    let commitDisposition: ProductBrowserCommitDisposition;
    try {
      // Headers arrive with the Response, before body delivery can truncate.
      // Preserve this completed-operation fact if reading JSON later fails.
      commitDisposition = decodeCommitDisposition(response.headers, route);
    } catch (cause) {
      const error = cause instanceof ProductBrowserLocalTransportError
        ? cause
        : new ProductBrowserLocalTransportError(
          'response_decode_failed',
          `Product Browser local runtime returned an invalid commit disposition for ${route}`,
          { cause, route, mutation: UNKNOWN_MUTATION },
        );
      throw error;
    }
    const mutationCertainty = commitDisposition === 'not-applied' ? 'not-applied' as const
      : commitDisposition === 'unknown' ? 'outcome-unknown' as const : 'committed' as const;
    const outputThroughHeader = response.headers.get('x-rusty-output-through');
    const outputThrough = outputThroughHeader === null ? null : BigInt(outputThroughHeader);
    const mutation = Object.freeze({
      certainty: mutationCertainty,
      outputRecovery: commitDisposition === 'resync-required'
        ? 'fresh-baseline-required' as const
        : 'none' as const,
      outputThrough: outputThrough?.toString(10) ?? null,
    });
    const contentType = response.headers.get('content-type')?.toLowerCase() ?? '';
    if (!contentType.startsWith('application/json')) {
      const error = new ProductBrowserLocalTransportError(
        'response_decode_failed',
        `Product Browser local runtime response for ${route} must use application/json`,
        { route, mutation },
      );
      if (mutation.outputRecovery === 'fresh-baseline-required') {
        await recoverFreshOutputsOrTerminal(route);
      }
      throw error;
    }
    let text: string;
    try {
      text = await response.text();
    } catch (cause) {
      const source = cause instanceof ProductBrowserLocalTransportError
        ? cause
        : new ProductBrowserLocalTransportError(
          'request_failed',
          `Product Browser local runtime response could not be read for ${route}`,
          { cause, route, mutation },
        );
      const error = new ProductBrowserLocalTransportError(
        source.code,
        source.message,
        { cause: source, route, mutation },
      );
      if (mutation.outputRecovery === 'fresh-baseline-required') {
        await recoverFreshOutputsOrTerminal(route);
      }
      throw error;
    }
    let value: unknown;
    try {
      value = JSON.parse(text) as unknown;
    } catch (cause) {
      const error = new ProductBrowserLocalTransportError(
        'response_decode_failed',
        `Product Browser local runtime returned invalid JSON for ${route}`,
        { cause, route, mutation },
      );
      if (mutation.outputRecovery === 'fresh-baseline-required') {
        await recoverFreshOutputsOrTerminal(route);
      }
      throw error;
    }
    if (!allowAfterDispose) ensureOpen();
    if (commitDisposition === 'resync-required') {
      await recoverFreshOutputsOrTerminal(route);
    } else if (outputThrough !== null) {
      if (currentOutputEpoch === outputEpochAtRequest) {
        await waitUntilOutputSequence(outputThrough);
      } else {
        // The request belongs to the now-discarded attachment. Opening the
        // replacement stream is not sufficient: its complete baseline must
        // arrive before this committed operation can be observed as settled.
        await freshOutputRecovery;
      }
    }
    return value as Result;
  };

  const lifecycle = (operation: ProductBrowserLifecycleOperation): Promise<ProductDevOperationResult> =>
    post<ProductDevLifecycleRequest, ProductDevOperationResult>(ROUTES.lifecycle[operation.kind], {});

  const replaceControl = (
    runtime: RustyApplicationRuntimeIdentity,
  ): Promise<ProductDevOperationResult> =>
    post<ProductDevControlRequest, ProductDevOperationResult>(ROUTES.control.replace, { runtime });

  const input = (
    batch: readonly RuntimeInputWireEvent[],
  ): Promise<ProductDevInputResult> =>
    post<ProductDevInputRequest, ProductDevInputResult>(ROUTES.input, { batch: [...batch] });

  const reportBrowserDiagnostics = (
    report: ProductDevBrowserDiagnosticsReport,
  ): Promise<ProductDevBrowserDiagnosticsResult> =>
    // The first terminal host report must survive closing the SSE transport.
    // This exact route remains bounded and does not reopen the runtime API.
    post<ProductDevBrowserDiagnosticsReport, ProductDevBrowserDiagnosticsResult>(
      ROUTES.browserDiagnostics,
      { ...report, attachment: attachment.read() },
      true,
    );

  const advanceRealtime = (observedTimeNs: string): Promise<ProductDevOperationResult> =>
    post<ProductDevRealtimeRequest, ProductDevOperationResult>(ROUTES.advanceRealtime, { observedTimeNs });

  const admitDemandStep = (): Promise<ProductDevOperationResult> =>
    post<ProductDevEmptyRequest, ProductDevOperationResult>(ROUTES.admitDemandStep, {});

  const admitExternalStep = (step: string): Promise<ProductDevOperationResult> =>
    post<ProductDevExternalRequest, ProductDevOperationResult>(ROUTES.admitExternalStep, { step });

  const completeTimeline = (
    completion: ProductDevTimelineCompletion,
  ): Promise<ProductDevTimelineCompletionResult> =>
    post<ProductDevTimelineCompletion, ProductDevTimelineCompletionResult>(ROUTES.completeTimeline, completion);

  const publishOutputBatch = (
    outputs: readonly ProductDevRuntimeOutput[],
    metadata: ProductBrowserRuntimeOutputBatchMetadata = {
      epoch: currentOutputEpoch,
      baseline: false,
      recovery: 'none',
    },
  ): void => {
    const batch = Object.freeze([...outputs]);
    if (metadata.baseline) {
      const binding = batch.find((output) => output.kind === 'binding');
      if (binding?.kind === 'binding') attachment.stage(metadata.epoch, {
        runtime: binding.runtime,
        nextInputSequence: binding.nextInputSequence,
      });
    }
    for (const output of batch) {
      if (output.kind === 'binding') currentOutputBinding = output.runtime;
    }
    for (const candidate of [...batchListeners]) {
      try {
        candidate(batch, metadata);
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

  const stageOrPublishOutputBatch = (
    outputs: readonly ProductDevRuntimeOutput[],
    epoch: number,
  ): void => {
    if (connectionBaselineComplete) {
      const replacementBinding = outputs.find((output) => output.kind === 'binding');
      if (replacementBinding !== undefined
        && currentOutputBinding !== null
        && replacementBinding.runtime.instanceId !== currentOutputBinding.instanceId) {
        failOutputStream(new ProductBrowserLocalTransportError(
          'output_decode_failed',
          'Product Browser local runtime changed incarnation without a fresh output baseline',
          { route: ROUTES.outputs },
        ));
        return;
      }
      publishOutputBatch(outputs, { epoch, baseline: false, recovery: 'none' });
      return;
    }
    for (const output of outputs) {
      if (output.kind === 'binding') currentOutputBinding = output.runtime;
      pendingConnectionOutputs.push(output);
    }
  };

  const failOutputStream = (cause: unknown): void => {
    const error = cause instanceof ProductBrowserLocalTransportError
      ? cause
      : new ProductBrowserLocalTransportError(
        'output_decode_failed',
        `Product Browser local runtime emitted an invalid output stream: ${cause instanceof Error ? cause.message : String(cause)}`,
        { cause, route: ROUTES.outputs },
    );
    if (connectionBaselineComplete) {
      publishFreshBaselineRequired();
      reportTransportError(error);
      void reconnectFreshOutputs().catch((cause: unknown) => {
        const recoveryError = cause instanceof ProductBrowserLocalTransportError
          ? cause
          : new ProductBrowserLocalTransportError(
            'stream_failed',
            `Product Browser local runtime fresh output recovery failed: ${cause instanceof Error ? cause.message : String(cause)}`,
            { cause, route: ROUTES.freshOutputs },
          );
        reportTerminalFailure(
          { kind: 'runtime-failure', diagnostic: recoveryError.message },
          recoveryError,
        );
      });
      return;
    }
    reportTerminalFailure({ kind: 'runtime-failure', diagnostic: error.message }, error);
  };

  // A closed stream before its baseline (for example a 503 while the
  // supervisor replaces the runtime) is not retried by EventSource. Reopen
  // the fresh route with a short backoff until a runtime answers.
  let freshRetryDelayMs = FRESH_RETRY_INITIAL_DELAY_MS;
  const openFreshStream = (): void => {
    const attachedStream = new eventSourceConstructor(`${basePath}${ROUTES.freshOutputs}`);
    const outputEpoch = nextOutputEpoch + 1;
    nextOutputEpoch = outputEpoch;
    currentOutputEpoch = outputEpoch;
    attachment.begin(outputEpoch);
    stream = attachedStream;
    const ownsProjection = (): boolean => !disposed
      && terminalFailure === null
      && stream === attachedStream
      && currentOutputEpoch === outputEpoch;
    attachedStream.onopen = () => {
      if (!ownsProjection()) return;
      resolveOutputSubscriptionReady?.();
      resolveOutputSubscriptionReady = null;
    };
    streamBaselineListener = (event) => {
      if (!ownsProjection()) return;
      try {
        if (event.lastEventId !== '') {
          throw new TypeError('connection baseline completion must not carry a reconnect cursor');
        }
        const { outputThrough, ...result } = JSON.parse(event.data) as ProductDevConnectionBaseline;
        observedOutputSequence = BigInt(outputThrough);
        settleOutputSequenceWaiters();
        if (!result.accepted) {
          throw new ProductBrowserLocalTransportError(
            'request_failed',
            result.diagnostic ?? 'Product Browser local runtime rejected the browser connection',
            { route: ROUTES.freshOutputs },
          );
        }
        if (connectionBaselineComplete) {
          throw new TypeError('connection baseline completion was duplicated without a reconnect');
        }
        const instanceId = currentOutputBinding?.instanceId ?? null;
        if (pageInstanceId !== null && instanceId !== null && instanceId !== pageInstanceId) {
          // A new incarnation is a new product, possibly with a new UI. Start
          // the page over instead of re-attaching its old UI module.
          reloadPage();
          return;
        }
        pageInstanceId ??= instanceId;
        connectionBaselineComplete = true;
        freshRetryDelayMs = FRESH_RETRY_INITIAL_DELAY_MS;
        const baselineOutputs = pendingConnectionOutputs;
        pendingConnectionOutputs = [];
        publishOutputBatch(baselineOutputs, {
          epoch: outputEpoch,
          baseline: true,
          recovery: 'none',
        });
        releaseOutputSequenceWaiters('fresh-baseline');
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
    attachedStream.addEventListener?.('rusty-output-baseline', streamBaselineListener);
    attachedStream.addEventListener?.('rusty-ui-reloaded', () => {
      if (ownsProjection()) reloadPage();
    });
    attachedStream.onmessage = (event) => {
      if (!ownsProjection()) return;
      try {
        const outputs = JSON.parse(event.data) as ProductDevRuntimeOutput[];
        if (connectionBaselineComplete) {
          observeOutputSequence(event.lastEventId);
        }
        stageOrPublishOutputBatch(
          outputs,
          outputEpoch,
        );
      } catch (cause) {
        const error = cause instanceof ProductBrowserLocalTransportError
          ? cause
          : new ProductBrowserLocalTransportError(
            'output_decode_failed',
            `Product Browser local runtime emitted an invalid output: ${cause instanceof Error ? cause.message : String(cause)}`,
            { cause, route: ROUTES.outputs },
          );
        failOutputStream(error);
      }
    };
    attachedStream.onerror = (event) => {
      if (!ownsProjection()) return;
      if (!connectionBaselineComplete) {
        pendingConnectionOutputs = [];
        currentOutputBinding = null;
        if (attachedStream.readyState === EVENT_SOURCE_CLOSED) {
          const delay = freshRetryDelayMs;
          freshRetryDelayMs = Math.min(delay * 2, FRESH_RETRY_MAX_DELAY_MS);
          setTimeout(() => {
            if (!ownsProjection() || connectionBaselineComplete) return;
            attachedStream.close();
            openFreshStream();
          }, delay);
        }
      } else {
        // A cursor belongs to this host process, not merely its URL. After
        // interruption the server may be a new process whose counter is
        // below OR above ours. Attach a complete retained baseline instead
        // of allowing EventSource to reuse Last-Event-ID across incarnations.
        // Only output is recovered; no mutation or input is replayed.
        void recoverFreshOutputsOrTerminal(ROUTES.freshOutputs).catch(() => undefined);
      }
      const error = new ProductBrowserLocalTransportError(
        'stream_failed',
        `Product Browser local runtime output stream failed${event instanceof Error ? `: ${event.message}` : ''}`,
        { route: ROUTES.outputs },
      );
      // Before a baseline completes, the fresh route is retried (by
      // EventSource, or above once it has given up). Established
      // subscriptions were replaced above with that same cursor-free path.
      reportTransportError(error);
    };
  };

  const subscribeOutputs = (
    listener: (output: ProductDevRuntimeOutput) => void,
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
        connectionReady = new Promise<ProductDevOperationResult>((resolve, reject) => {
          resolveConnectionReady = resolve;
          rejectConnectionReady = reject;
        });
        // Output-only consumers still need terminal stream failures without being
        // forced to await connect(). Keep the shared promise observable for
        // connect callers while preventing an unhandled rejection otherwise.
        void connectionReady.catch(() => undefined);
        connectionBaselineComplete = false;
        pendingConnectionOutputs = [];
        openFreshStream();
      } catch (cause) {
        listeners.delete(listener);
        // openFreshStream may have assigned the stream before failing.
        (stream as ProductBrowserLocalEventSource | null)?.close();
        stream = null;
        streamBaselineListener = null;
        pendingConnectionOutputs = [];
        releaseOutputSequenceWaiters('closed');
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
        if (streamBaselineListener !== null) {
          stream?.removeEventListener?.('rusty-output-baseline', streamBaselineListener);
          streamBaselineListener = null;
        }
        stream?.close();
        stream = null;
        pendingConnectionOutputs = [];
        releaseOutputSequenceWaiters('closed');
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
    listener: ProductBrowserRuntimeOutputBatchListener,
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

  const connect = async (): Promise<ProductDevOperationResult> => {
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
    if (streamBaselineListener !== null) {
      stream?.removeEventListener?.('rusty-output-baseline', streamBaselineListener);
      streamBaselineListener = null;
    }
    stream?.close();
    stream = null;
    pendingConnectionOutputs = [];
    releaseOutputSequenceWaiters('closed');
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
    replaceControl,
    input,
    reportBrowserDiagnostics,
    advanceRealtime,
    admitDemandStep,
    admitExternalStep,
    completeTimeline,
    subscribeTerminalFailures,
    subscribeOutputs,
    subscribeOutputBatches,
    waitUntilOutputSubscriptionReady,
    waitUntilOutputSequence: (through: string) => waitUntilOutputSequence(BigInt(through)),
    recoverOutputProjection: () => recoverFreshOutputsOrTerminal(ROUTES.freshOutputs),
    confirmOutputBaseline: (epoch: number) => attachment.confirm(epoch),
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
