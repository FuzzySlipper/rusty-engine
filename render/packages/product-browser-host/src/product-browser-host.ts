import { installPlaytestInspection } from './playtest-inspection.js';
import {
  mountRustyApplication,
  type RustyApplicationHost,
  type RustyApplicationHostReadout,
  type RustyApplicationRuntimeIdentity,
  type RustyApplicationRuntimeInputOptions,
  type RustyApplicationRuntimeState,
  type RustyApplicationUiLifecyclePort,
  type RustyApplicationUiLifecycleResult,
  type RustyApplicationUiMount,
  type RustyApplicationUiProjectionOptions,
  type RustyApplicationPresentationAspectBounds,
} from '@rusty-engine/application-host';
import type {
  ProductHostBrowserDiagnosticsReport,
  ProductHostBrowserDiagnosticsResult,
  ProductHostCursorMode,
  ProductHostInputResult,
  ProductHostOperationResult,
  ProductHostRenderOutput,
  ProductHostRuntimeMode,
  ProductHostRuntimeOutput,
  ProductHostRuntimeReadout,
  ProductHostTimelineCompletion,
  ProductHostTimelineCompletionResult,
  RuntimeInputWireEvent,
  RuntimeUiProjectionEnvelope,
} from './generated/contracts.js';
import { createProductBrowserCadence, type ProductBrowserCadence } from './realtime-cadence.js';

/** Fixed current artifact identity; compatibility follows actual code changes. */
export const PRODUCT_BROWSER_HOST_ARTIFACT = 'rusty.product.browser-host' as const;

/**
 * Selects the owner that admits fixed-step realtime work.
 *
 * `rust-host` is the product host's scheduler: the page still drains typed
 * input on each animation frame, but it never asks the runtime to advance
 * from its own clock. `browser` advances realtime from the page cadence.
 */
export type ProductBrowserRealtimeAdvanceOwner = 'browser' | 'rust-host';

/**
 * The fixed lifecycle operation vocabulary carried by the local bridge. The
 * operation union is deliberately closed: product code cannot turn the
 * bridge into a method-name RPC or invoke an arbitrary runtime operation.
 */
export type ProductBrowserLifecycleOperation =
  | { readonly kind: 'start' }
  | { readonly kind: 'pause'; readonly runtime: RustyApplicationRuntimeIdentity }
  | { readonly kind: 'resume'; readonly runtime: RustyApplicationRuntimeIdentity }
  | { readonly kind: 'restart' }
  | { readonly kind: 'shutdown' }
  | { readonly kind: 'report-fault' };

export type ProductBrowserRuntimeOutputListener = (
  output: ProductHostRuntimeOutput,
) => void;

export type ProductBrowserRuntimeOutputBatchListener = (
  outputs: readonly ProductHostRuntimeOutput[],
  metadata: ProductBrowserRuntimeOutputBatchMetadata,
) => void;

/**
 * Browser-local framing for one ordered output delivery. The epoch is local
 * to the attached EventSource; it is not a second runtime identity. A
 * recovery marker has no outputs: the page ignores later incremental outputs
 * until the following complete baseline replaces what it lost.
 */
export interface ProductBrowserRuntimeOutputBatchMetadata {
  readonly epoch: number;
  readonly baseline: boolean;
  readonly recovery: 'none' | 'fresh-baseline-required';
}

/** A terminal local-transport failure: the host stops. */
export interface ProductBrowserRuntimeTerminalFailure {
  /** The fixed Engine failure lane; products never supply an arbitrary event name. */
  readonly kind: 'runtime-failure';
  readonly diagnostic: string;
}

export type ProductBrowserRuntimeTerminalFailureListener = (
  failure: ProductBrowserRuntimeTerminalFailure,
) => void;

/**
 * The page's local runtime transport. Its operation surface is fixed and
 * named; it has no generic `call` or arbitrary message method.
 */
export interface ProductBrowserRuntimeAdapter {
  /**
   * Resolves the Engine-owned fresh connection baseline. Local generated
   * hosts use this instead of issuing `start` on every browser mount.
   */
  readonly connect?: () => Promise<ProductHostOperationResult>;
  readonly lifecycle: (
    operation: ProductBrowserLifecycleOperation,
  ) => Promise<ProductHostOperationResult>;
  /** Advances only the current input control fence; it does not fault or restart the product. */
  readonly replaceControl?: (
    runtime: RustyApplicationRuntimeIdentity,
  ) => Promise<ProductHostOperationResult>;
  readonly input: (
    batch: readonly RuntimeInputWireEvent[],
  ) => Promise<ProductHostInputResult>;
  readonly reportBrowserDiagnostics?: (
    report: ProductHostBrowserDiagnosticsReport,
  ) => Promise<ProductHostBrowserDiagnosticsResult>;
  readonly advanceRealtime: (
    observedTimeNs: string,
  ) => Promise<ProductHostOperationResult>;
  readonly admitDemandStep?: () => Promise<ProductHostOperationResult>;
  readonly admitExternalStep?: (
    step: string,
  ) => Promise<ProductHostOperationResult>;
  readonly completeTimeline?: (
    completion: ProductHostTimelineCompletion,
  ) => Promise<ProductHostTimelineCompletionResult>;
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
  /** Resolves once the page has received every output through `through`. */
  readonly waitUntilOutputSequence?: (through: string) => Promise<void>;
  /** Resolves once an asynchronous output subscription can receive runtime publications. */
  readonly waitUntilOutputSubscriptionReady?: () => Promise<void>;
  /** Confirms that the page has applied the baseline of `epoch`. */
  readonly confirmOutputBaseline?: (epoch: number) => void;
  readonly dispose: () => Promise<void> | void;
}

export interface ProductBrowserHostOptions {
  readonly root: HTMLElement;
  readonly transport: ProductBrowserRuntimeAdapter;
  readonly lifecycleMode: ProductHostRuntimeMode;
  /**
   * Owner of realtime simulation admission. Defaults to `browser`. Only
   * realtime products read it; demand and external products ignore it.
   */
  readonly realtimeAdvanceOwner?: ProductBrowserRealtimeAdvanceOwner;
  /** Where the runtime draws the world: streamed to this page (the default) or to the desktop window. */
  readonly output?: ProductHostRenderOutput;
  readonly mountUi: RustyApplicationUiMount;
  readonly runtimeInput?: Omit<RustyApplicationRuntimeInputOptions, 'binding' | 'onAvailable'> & {
    readonly binding?: RustyApplicationRuntimeIdentity;
  };
  readonly uiProjection?: Omit<ProductBrowserUiProjectionOptions, 'binding'> & {
    readonly binding?: RustyApplicationRuntimeIdentity;
  };
  readonly presentationAspectBounds?: RustyApplicationPresentationAspectBounds;
  readonly initialInteractionMode?: 'gameplay' | 'interface' | 'modal';
  /** Engine-selected gameplay cursor behavior; defaults to pointer lock for FPS products. */
  readonly gameplayCursorMode?: ProductHostCursorMode;
  readonly inputContext?: string;
  readonly loadingLabel?: string;
  readonly failureLabel?: string;
  /** Start the Rust runtime after the Engine host has mounted. Defaults true. */
  readonly autoStart?: boolean;
}

export interface ProductBrowserUiProjectionOptions {
  readonly expectedStream?: string;
  readonly expectedContract: string;
  readonly maximumSubscribers?: number;
}

export interface ProductBrowserHostReadout {
  readonly artifact: typeof PRODUCT_BROWSER_HOST_ARTIFACT;
  readonly state: 'starting' | 'ready' | 'degraded' | 'failed' | 'disposed';
  readonly mode: ProductHostRuntimeMode;
  readonly realtimeAdvanceOwner: ProductBrowserRealtimeAdvanceOwner;
  readonly host: RustyApplicationHostReadout | null;
  readonly runtime: ProductHostRuntimeReadout | null;
  readonly lastFailure: string | null;
}

export interface ProductBrowserHost {
  readonly kind: 'rusty.product.browser-host';
  readonly application: RustyApplicationHost;
  readonly transport: ProductBrowserRuntimeAdapter;
  readonly readout: () => ProductBrowserHostReadout;
  readonly completeTimeline: (
    completion: ProductHostTimelineCompletion,
  ) => Promise<ProductHostTimelineCompletionResult>;
  readonly admitDemandStep: () => Promise<ProductHostOperationResult>;
  readonly admitExternalStep: (
    step: string,
  ) => Promise<ProductHostOperationResult>;
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

interface ProductBrowserOperationQueue {
  readonly enqueue: <T>(operation: () => Promise<T>) => Promise<T>;
  readonly settle: () => Promise<void>;
}

const MAXIMUM_PENDING_OUTPUTS = 64;
const MAXIMUM_HEALTH_DIAGNOSTIC_BYTES = 512;

/** This is the sole browser cadence observation that can be safely dropped. */
export function isDroppedClockRegression(result: ProductHostOperationResult): boolean {
  return !result.accepted
    && result.disposition === 'rejected-recoverable'
    && result.code === 'CSHARP_LIFECYCLE_CLOCK_REGRESSION'
    && result.operation === 'advance-realtime';
}

/**
 * Buffers runtime outputs that arrive before the application has mounted.
 * Readouts are snapshots and UI projections replace their stream's previous
 * value, so only the newest of each is kept.
 *
 * @internal
 */
export function bufferProductBrowserPreMountOutput(
  pendingOutputs: ProductHostRuntimeOutput[],
  output: ProductHostRuntimeOutput,
  maximumPendingOutputs: number,
): boolean {
  const previous = pendingOutputs.findIndex((pending) =>
    (output.kind === 'runtime-readout' && pending.kind === 'runtime-readout')
    || (output.kind === 'ui-projection'
      && pending.kind === 'ui-projection'
      && pending.envelope.stream === output.envelope.stream
      && pending.envelope.contract === output.envelope.contract));
  if (previous >= 0) {
    pendingOutputs[previous] = output;
    return true;
  }
  if (pendingOutputs.length >= maximumPendingOutputs) return false;
  pendingOutputs.push(output);
  return true;
}

export function syncProductBrowserHealthDatasets(
  roots: readonly Pick<HTMLElement, 'dataset'>[],
  values: {
    readonly state: ProductBrowserHostReadout['state'];
    readonly mode: ProductHostRuntimeMode;
    readonly progress: string;
    readonly failure: string | null;
  },
): void {
  for (const root of roots) {
    if (root.dataset['rustyProductHostState'] !== values.state) {
      root.dataset['rustyProductHostState'] = values.state;
    }
    if (root.dataset['rustyProductRuntimeMode'] !== values.mode) {
      root.dataset['rustyProductRuntimeMode'] = values.mode;
    }
    if (root.dataset['rustyProductRuntimeProgress'] !== values.progress) {
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
  let runtimeReadout: ProductHostRuntimeReadout | null = null;
  const lifecycleListeners = new Set<(state: RustyApplicationRuntimeState | null) => void>();
  const observeReadout = (readout: ProductHostRuntimeReadout): void => {
    const previous = runtimeReadout?.state ?? null;
    runtimeReadout = readout;
    if (readout.state === previous) return;
    for (const listener of [...lifecycleListeners]) {
      if (lifecycleListeners.has(listener)) listener(readout.state);
    }
  };
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
  // A harness holding input (`control/claim`): the page sends none and shows
  // who holds it until a binding without a claim arrives.
  let inputClaim: string | null = null;
  let claimBadge: HTMLElement | null = null;
  const showInputClaim = (label: string | null): void => {
    inputClaim = label;
    const document = options.root.ownerDocument;
    for (const root of [options.root, document.body]) {
      if (root === null) continue;
      if (label === null) delete root.dataset['rustyInputClaim'];
      else root.dataset['rustyInputClaim'] = label;
    }
    if (label === null) {
      claimBadge?.remove();
      claimBadge = null;
      return;
    }
    if (claimBadge === null) {
      claimBadge = document.createElement('div');
      claimBadge.className = 'rusty-input-claim';
      claimBadge.setAttribute('role', 'status');
      Object.assign(claimBadge.style, {
        position: 'fixed', top: '8px', left: '50%', transform: 'translateX(-50%)', zIndex: '2147483647',
        padding: '4px 10px', borderRadius: '4px', pointerEvents: 'none',
        font: '12px system-ui, sans-serif', color: '#fff', background: 'rgba(20, 20, 20, 0.8)',
      });
      document.body?.append(claimBadge);
    }
    claimBadge.textContent = `Input held by ${label}`;
  };
  let inputRecovery: {
    readonly uncertainBinding: RustyApplicationRuntimeIdentity;
    inFlight: boolean;
  } | null = null;
  // After an output gap the transport attaches afresh; the page ignores
  // incremental outputs until that attachment's complete baseline arrives.
  let awaitingBaseline = false;
  let acceptedEpoch = 0;
  let browserDiagnosticsReportInFlight = false;
  let pendingHealthTransition = false;
  let transportClosed = false;
  let runtimeProgress = 0;
  let lastDiagnosticsStatusKey: string | null = null;
  let baselineConfirmationRevision = 0;
  let terminalDiagnosticsReported = false;
  let recoverableClockDiagnosticPending = false;
  let recoverableClockDiagnosticReported = false;
  const pendingOutputs: ProductHostRuntimeOutput[] = [];

  // These are deliberately small, product-neutral observation markers. They
  // let an outer host prove that a mounted runtime is still making accepted
  // progress without inspecting a product's UI, facts, or content vocabulary.
  const publishHealth = (
    reportToTransport = true,
    pageEvents: readonly { readonly kind: 'error' | 'unhandled-rejection'; readonly code: string; readonly message: string }[] = [],
  ): void => {
    const document = options.root.ownerDocument;
    const roots = [options.root, document.body].filter((root): root is HTMLElement => root !== null);
    syncProductBrowserHealthDatasets(roots, {
      state,
      mode: options.lifecycleMode,
      progress: String(runtimeProgress),
      failure: (failure ?? recoveryFailure) === null ? null : boundedDiagnostic((failure ?? recoveryFailure)!.message),
    });
    if (!reportToTransport && pageEvents.length === 0) return;
    const terminal = failure === null
      ? undefined
      : Object.freeze({
        code: `BROWSER_HOST_${failure.code.toUpperCase()}`,
        message: boundedDiagnostic(failure.message),
      });
    const includeTerminal = terminal !== undefined && !terminalDiagnosticsReported;
    const hostState = state === 'starting' ? 'loading' : state;
    const connection = transportClosed ? 'closed' : 'open';
    const statusKey = `${hostState}/${connection}/${baselineConfirmationRevision}`;
    const recoverableEvent = recoverableClockDiagnosticPending && !recoverableClockDiagnosticReported
      ? Object.freeze({
          code: 'CSHARP_LIFECYCLE_CLOCK_REGRESSION' as const,
          message: 'dropped a regressing browser realtime observation; awaiting a later monotonic observation',
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
    const report = Object.freeze({
      hostState,
      runtimeProgress: String(runtimeProgress),
      transportState: connection,
      outputState: connection,
      ...(includeTerminal ? { firstTerminal: terminal } : {}),
      ...(recoverableEvent === undefined ? {} : { recoverableEvent }),
      pageEvents: Object.freeze([...pageEvents]),
    }) as ProductHostBrowserDiagnosticsReport;
    browserDiagnosticsReportInFlight = true;
    const followUp = (): void => {
      browserDiagnosticsReportInFlight = false;
      const flushPendingHealthTransition = pendingHealthTransition;
      pendingHealthTransition = false;
      if (flushPendingHealthTransition) publishHealth();
    };
    void transport.reportBrowserDiagnostics!(report).then(
      () => {
        lastDiagnosticsStatusKey = statusKey;
        if (includeTerminal) terminalDiagnosticsReported = true;
        if (recoverableEvent?.code === 'CSHARP_LIFECYCLE_CLOCK_REGRESSION') {
          recoverableClockDiagnosticPending = false;
          recoverableClockDiagnosticReported = true;
        }
        if (recoverableEvent?.code === 'BROWSER_LOCAL_REQUEST_UNAVAILABLE') {
          recoveryDiagnosticReported = true;
        }
        followUp();
      },
      followUp,
    );
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
      // A recoverable request failure may precede a different terminal fault.
      // Keep its recoverable diagnostic, but attribute closure to this cause.
      failure = error;
      if (state !== 'disposed') state = 'failed';
      // The DOM remains current now; the typed terminal report waits until
      // closeTransport has recorded the durable closed state.
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
    if (state !== 'degraded' || inputRecovery !== null || awaitingBaseline) return;
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
          if (result.readout !== undefined) observeReadout(result.readout);
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

  const beginInputRecovery = (batch: readonly RuntimeInputWireEvent[]): void => {
    const first = batch[0];
    if (first === undefined || inputRecovery !== null
      || (state !== 'ready' && state !== 'degraded')) return;
    inputRecovery = { uncertainBinding: first.runtime, inFlight: false };
    state = 'degraded';
    publishHealth();
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

  const applyOutput = (output: ProductHostRuntimeOutput): void => {
    if (application === null) {
      if (!bufferProductBrowserPreMountOutput(pendingOutputs, output, MAXIMUM_PENDING_OUTPUTS)) {
        failAndClose(
          new ProductBrowserHostError(
            'output_failed',
            `runtime output buffer exceeded ${String(MAXIMUM_PENDING_OUTPUTS)} entries before host mount`,
          ),
          'output_failed',
        );
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
              // Its claim applies before page input resumes under it.
              showInputClaim(output.inputClaim ?? null);
              completeInputRecovery(output.runtime, output.nextInputSequence);
            }
            return;
          }
          currentInputBinding = output.runtime;
          showInputClaim(output.inputClaim ?? null);
          host.input?.bindRuntime({
            runtime: output.runtime,
            context: options.inputContext ?? 'gameplay.default',
            nextSequence: output.nextInputSequence,
          });
          host.uiProjection?.bindRuntime(output.runtime);
          return;
        case 'runtime-input-result':
          applyInputResult(output.result);
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
          observeReadout(output.readout);
          return;
        default:
          assertNever(output);
      }
    } catch (cause) {
      failAndClose(cause, 'output_failed');
    }
  };

  const applyOutputBatch = (
    outputs: readonly ProductHostRuntimeOutput[],
    metadata?: ProductBrowserRuntimeOutputBatchMetadata,
  ): void => {
    if (metadata?.recovery === 'fresh-baseline-required') {
      awaitingBaseline = true;
      if (state === 'ready') state = 'degraded';
      publishHealth();
      return;
    }
    if (metadata !== undefined && metadata.epoch < acceptedEpoch) return;
    if (awaitingBaseline && metadata !== undefined && metadata.baseline !== true) return;
    if (metadata !== undefined) acceptedEpoch = metadata.epoch;
    for (const output of outputs) applyOutput(output);
    if (metadata?.baseline === true && application !== null) {
      awaitingBaseline = false;
      transport.confirmOutputBaseline?.(metadata.epoch);
      // A prior in-flight report must not acknowledge this new baseline.
      baselineConfirmationRevision += 1;
      publishHealth();
    }
    if (state !== 'failed' && state !== 'disposed' && outputs.length > 0) {
      restoreReadyAfterHealthyTransport();
    }
  };

  const applyTerminalFailure = (terminalFailure: ProductBrowserRuntimeTerminalFailure): void => {
    const normalized = normalizeTerminalFailure(terminalFailure);
    failAndClose(
      new ProductBrowserHostError('transport_failed', normalized.diagnostic),
      'transport_failed',
    );
  };

  const operationOutputs = (result: {
    readonly binding?: RustyApplicationRuntimeIdentity;
    readonly nextInputSequence?: string;
    readonly readout?: ProductHostRuntimeReadout;
  }): ProductHostRuntimeOutput[] => {
    // A response can settle after the output stream has already delivered a
    // later change (another page or a tool paused or resumed meanwhile); its
    // older binding and readout must not rewind what the page shows.
    const outputs: ProductHostRuntimeOutput[] = [];
    if (result.binding !== undefined && result.nextInputSequence !== undefined
      && !isOlderRuntimeBinding(result.binding, currentInputBinding)) {
      outputs.push({
        kind: 'binding',
        runtime: result.binding,
        nextInputSequence: result.nextInputSequence,
        // A result does not say who holds input (a harness's input results
        // reach every page), so it keeps the claim the last binding
        // publication set; only a publication changes it.
        ...(inputClaim === null ? {} : { inputClaim }),
      });
    }
    if (result.readout !== undefined
      && !isOlderRuntimeBinding(result.readout.runtime, runtimeReadout?.runtime ?? null)) {
      outputs.push({ kind: 'runtime-readout', readout: result.readout });
    }
    return outputs;
  };

  const applyOperationResult = (
    result: ProductHostOperationResult,
    rejectedCode: ProductBrowserHostError['code'] = 'transport_failed',
    allowDroppedClockRegression = false,
  ): boolean => {
    if (allowDroppedClockRegression && isDroppedClockRegression(result)) {
      recoverableClockDiagnosticPending = true;
      publishHealth();
      return false;
    }
    applyOutputBatch(operationOutputs(result));
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

  function applyInputResult(result: ProductHostInputResult): void {
    if (inputRecovery !== null) {
      // An asynchronous mailbox result for the ambiguous batch is stale by
      // construction. Only the acknowledged control-replace response may
      // establish the replacement cursor.
      return;
    }
    if (result.binding !== undefined && currentInputBinding !== null
      && !sameRuntimeBinding(result.binding, currentInputBinding)
      && !hasFreshRecoveryBinding(result.binding, currentInputBinding)) {
      // Delayed results from a superseded epoch are observations only; they
      // cannot rewind the browser input cursor after a later control fence.
      return;
    }
    applyOutputBatch(operationOutputs(result));
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

  const sendInput = async (batch: readonly RuntimeInputWireEvent[]): Promise<void> => {
    // While a harness holds input, the page's own input is dropped here, so
    // focus changes and page blur cannot clear what the harness holds.
    if (inputClaim !== null) return;
    try {
      applyInputResult(await transport.input(batch));
    } catch (cause) {
      if (isUnknownLocalMutationFailure(cause)) beginInputRecovery(batch);
      throw cause;
    }
  };

  const drainAndSendInput = async (): Promise<void> => {
    const host = requireApplication();
    host.input?.sampleController();
    const batch = host.input?.drain() ?? [];
    if (batch.length > 0) await sendInput(batch);
  };

  cadence = createProductBrowserCadence({
    lifecycleMode: options.lifecycleMode,
    realtimeAdvanceOwner,
    isReady: () => inputRecovery === null
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
      const accepted = applyOperationResult(
        await transport.advanceRealtime(observedTimeNs),
        'transport_failed',
        true,
      );
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
      applyOperationResult(await transport.admitDemandStep());
    },
    onFailure: (cause) => {
      if (isRecoveryGateError(cause)) return;
      recoverOrClose(cause, 'transport_failed');
    },
  });

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

  // Pause and resume name the binding this page holds, so a request made
  // against a runtime that was since restarted or replaced is rejected.
  const requestLifecycle = (kind: 'pause' | 'resume'): Promise<RustyApplicationUiLifecycleResult> => {
    try { requireReady(); } catch (cause) { return Promise.reject(cause); }
    return queue.enqueue(async () => {
      requireReady();
      const runtime = currentInputBinding ?? runtimeReadout?.runtime;
      if (runtime === undefined) {
        throw new ProductBrowserHostError('transport_failed', `${kind} has no runtime binding to name`);
      }
      const result = await transport.lifecycle({ kind, runtime });
      const accepted = applyOperationResult(result);
      return Object.freeze({
        accepted,
        state: runtimeReadout?.state ?? null,
        ...(accepted ? {} : { code: result.code }),
        ...(accepted || result.diagnostic === undefined ? {} : { diagnostic: result.diagnostic }),
      });
    }).catch((cause: unknown) => {
      if (isRecoveryGateError(cause)) throw cause;
      throw recoverOrClose(cause, 'transport_failed');
    });
  };
  const lifecycle: RustyApplicationUiLifecyclePort = Object.freeze({
    state: () => runtimeReadout?.state ?? null,
    subscribe: (listener: (state: RustyApplicationRuntimeState | null) => void) => {
      if (state === 'disposed') return () => undefined;
      lifecycleListeners.add(listener);
      return () => { lifecycleListeners.delete(listener); };
    },
    pause: () => requestLifecycle('pause'),
    resume: () => requestLifecycle('resume'),
  });

  try {
    unsubscribeTerminalFailures = transport.subscribeTerminalFailures?.(applyTerminalFailure) ?? null;
    unsubscribeOutputs = transport.subscribeOutputBatches?.(applyOutputBatch)
      ?? transport.subscribeOutputs((output) => applyOutputBatch([output]));
    application = await mountApplication({
      root: options.root,
      mountUi: options.mountUi,
      lifecycle,
      ...(options.output === undefined ? {} : { output: options.output }),
      onCadence: (timeMs) => cadence?.enqueue(timeMs),
      ...(options.presentationAspectBounds === undefined
        ? {}
        : { presentationAspectBounds: options.presentationAspectBounds }),
      ...(options.initialInteractionMode === undefined
        ? {}
        : { initialInteractionMode: options.initialInteractionMode }),
      ...(options.gameplayCursorMode === undefined
        ? {}
        : { gameplayCursorMode: options.gameplayCursorMode }),
      ...(options.loadingLabel === undefined ? {} : { loadingLabel: options.loadingLabel }),
      ...(options.failureLabel === undefined ? {} : { failureLabel: options.failureLabel }),
      ...(runtimeInput === undefined ? {} : { runtimeInput }),
      ...(projection === undefined ? {} : { uiProjection: projection }),
    });
    const bufferedOutputs = pendingOutputs.splice(0, pendingOutputs.length);
    applyOutputBatch(bufferedOutputs);
    if (failure !== null) throw failure;
    awaitingBaseline = false;
    transport.confirmOutputBaseline?.(acceptedEpoch);
    baselineConfirmationRevision += 1;
    if (options.autoStart !== false) {
      await transport.waitUntilOutputSubscriptionReady?.();
      if (failure !== null) throw failure;
      const result = await queue.enqueue(() => transport.connect?.()
        ?? transport.lifecycle({ kind: 'start' }));
      applyOperationResult(result, 'startup_failed');
      if (failure !== null) throw failure;
    }
    started = true;
    state = 'ready';
    publishHealth();
  } catch (cause) {
    // Output delivery can fail and close the shared transport while the
    // lifecycle response is still in flight. Preserve that first concrete
    // failure instead of replacing it with the resulting aborted fetch.
    const error = failure ?? reportFailure(cause, 'startup_failed');
    cadence.dispose();
    unsubscribeTerminalFailures?.();
    unsubscribeTerminalFailures = null;
    unsubscribeOutputs?.();
    unsubscribeOutputs = null;
    removePageDiagnosticListeners?.();
    removePageDiagnosticListeners = null;
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

  const removePlaytestInspection = installPlaytestInspection(
    () => queue.enqueue(async () => {
      const batch = host.input?.drain() ?? [];
      if (batch.length > 0) await sendInput(batch);
    }),
    async (through) => {
      if (through !== undefined) await transport.waitUntilOutputSequence?.(through);
    },
  );

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
    completion: ProductHostTimelineCompletion,
  ): Promise<ProductHostTimelineCompletionResult> => {
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

  const admitStep = (
    mode: 'demand' | 'external',
    admit: (() => Promise<ProductHostOperationResult>) | undefined,
  ): Promise<ProductHostOperationResult> => {
    try { requireReady(); } catch (cause) { return Promise.reject(cause); }
    if (options.lifecycleMode !== mode) {
      return Promise.reject(new ProductBrowserHostError(
        'invalid_options',
        mode === 'demand'
          ? 'admitDemandStep is only available for demand lifecycle products'
          : 'admitExternalStep is only available for external lifecycle products',
      ));
    }
    if (admit === undefined) {
      return Promise.reject(new ProductBrowserHostError(
        'transport_failed',
        `this native product did not provide a ${mode === 'demand' ? 'demand' : 'external'}-step transport lane`,
      ));
    }
    return queue.enqueue(async () => {
      requireReady();
      await drainAndSendInput();
      const result = await admit();
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
      lifecycleListeners.clear();
      started = false;
      transportClosed = true;
      publishHealth();
      cadence?.dispose();
      removePlaytestInspection();
      unsubscribeTerminalFailures?.();
      unsubscribeTerminalFailures = null;
      unsubscribeOutputs?.();
      unsubscribeOutputs = null;
      removePageDiagnosticListeners?.();
      removePageDiagnosticListeners = null;
      await queue.settle();
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
    admitDemandStep: () => admitStep('demand', transport.admitDemandStep),
    admitExternalStep: (step: string) => admitStep(
      'external',
      transport.admitExternalStep === undefined ? undefined : () => transport.admitExternalStep!(step),
    ),
    dispose,
  });
}

/** True when `candidate` is an earlier binding of the same runtime instance than `current`. */
function isOlderRuntimeBinding(
  candidate: RustyApplicationRuntimeIdentity,
  current: RustyApplicationRuntimeIdentity | null,
): boolean {
  if (current === null || candidate.instanceId !== current.instanceId) return false;
  const generation = BigInt(candidate.generation) - BigInt(current.generation);
  return generation < 0n
    || (generation === 0n && BigInt(candidate.controlRevision) < BigInt(current.controlRevision));
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
  if (value.kind !== 'runtime-failure') {
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
  if (options.realtimeAdvanceOwner !== undefined
    && options.realtimeAdvanceOwner !== 'browser'
    && options.realtimeAdvanceOwner !== 'rust-host') {
    throw new ProductBrowserHostError('invalid_options', 'Product Browser Host realtime advance owner is invalid');
  }
  if (typeof options.mountUi !== 'function') {
    throw new ProductBrowserHostError('invalid_options', 'Product Browser Host mountUi must be a function');
  }
  if (options.uiProjection !== undefined && typeof options.uiProjection.expectedContract !== 'string') {
    throw new ProductBrowserHostError('invalid_options', 'Product Browser Host UI projection requires expectedContract');
  }
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
