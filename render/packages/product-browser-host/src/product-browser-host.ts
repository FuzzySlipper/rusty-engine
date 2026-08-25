import {
  mountRustyApplication,
  type RustyApplicationFrame,
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
} from '@rusty-engine/application-host';
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
  | 'advance-realtime'
  | 'admit-demand-step'
  | 'admit-external-step';

export interface ProductBrowserRuntimeOperationResult {
  readonly accepted: boolean;
  readonly operation: ProductBrowserRuntimeOperationKind;
  readonly binding?: RustyApplicationRuntimeIdentity;
  readonly readout?: ProductBrowserRuntimeReadout;
  readonly diagnostic?: string;
}

export interface ProductBrowserRuntimeInputResult {
  readonly accepted: boolean;
  readonly count: number;
  readonly binding?: RustyApplicationRuntimeIdentity;
  readonly readout?: ProductBrowserRuntimeReadout;
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
}

export type ProductBrowserRuntimeOutput =
  | ProductBrowserRuntimeBindingOutput
  /** Fixed host evidence that one Rust-owned realtime advance was accepted. */
  | { readonly kind: 'runtime-progress'; readonly owner: 'rust-host' }
  | { readonly kind: 'frame'; readonly frame: RustyApplicationFrame }
  | {
      readonly kind: 'presentation';
      readonly frame: RustyApplicationPresentationFrame;
    }
  | {
      readonly kind: 'ui-projection';
      readonly envelope: RustyApplicationUiProjectionEnvelope;
    }
  | { readonly kind: 'runtime-readout'; readonly readout: ProductBrowserRuntimeReadout };

export type ProductBrowserRuntimeOutputListener = (
  output: ProductBrowserRuntimeOutput,
) => void;

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
  readonly lifecycle: (
    operation: ProductBrowserLifecycleOperation,
  ) => Promise<ProductBrowserRuntimeOperationResult>;
  readonly input: (
    batch: readonly RustyApplicationRuntimeInputEnvelope[],
  ) => Promise<ProductBrowserRuntimeInputResult>;
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
  readonly dispose: () => Promise<void> | void;
}

/** The transport kept by the generated bridge and consumed by the host. */
export interface ProductBrowserRuntimeTransport {
  readonly lifecycle: ProductBrowserRuntimeAdapter['lifecycle'];
  readonly input: ProductBrowserRuntimeAdapter['input'];
  readonly advanceRealtime: ProductBrowserRuntimeAdapter['advanceRealtime'];
  readonly admitDemandStep?: NonNullable<ProductBrowserRuntimeAdapter['admitDemandStep']>;
  readonly admitExternalStep?: NonNullable<ProductBrowserRuntimeAdapter['admitExternalStep']>;
  readonly completeTimeline?: NonNullable<ProductBrowserRuntimeAdapter['completeTimeline']>;
  readonly subscribeTerminalFailures?: NonNullable<ProductBrowserRuntimeAdapter['subscribeTerminalFailures']>;
  readonly subscribeOutputs: ProductBrowserRuntimeAdapter['subscribeOutputs'];
  readonly dispose: ProductBrowserRuntimeAdapter['dispose'];
}

export function createProductBrowserRuntimeTransport(
  adapter: ProductBrowserRuntimeAdapter,
): ProductBrowserRuntimeTransport {
  if (adapter === null || typeof adapter !== 'object') {
    throw new TypeError('Product Browser Host runtime adapter must be an object');
  }
  requireFunction(adapter.lifecycle, 'lifecycle');
  requireFunction(adapter.input, 'input');
  requireFunction(adapter.advanceRealtime, 'advanceRealtime');
  if (adapter.completeTimeline !== undefined) {
    requireFunction(adapter.completeTimeline, 'completeTimeline');
  }
  if (adapter.subscribeTerminalFailures !== undefined) {
    requireFunction(adapter.subscribeTerminalFailures, 'subscribeTerminalFailures');
  }
  requireFunction(adapter.subscribeOutputs, 'subscribeOutputs');
  requireFunction(adapter.dispose, 'dispose');
  return Object.freeze({
    lifecycle: adapter.lifecycle,
    input: adapter.input,
    advanceRealtime: adapter.advanceRealtime,
    ...(adapter.admitDemandStep === undefined ? {} : { admitDemandStep: adapter.admitDemandStep }),
    ...(adapter.admitExternalStep === undefined ? {} : { admitExternalStep: adapter.admitExternalStep }),
    ...(adapter.completeTimeline === undefined ? {} : { completeTimeline: adapter.completeTimeline }),
    ...(adapter.subscribeTerminalFailures === undefined
      ? {}
      : { subscribeTerminalFailures: adapter.subscribeTerminalFailures }),
    subscribeOutputs: adapter.subscribeOutputs,
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
  readonly state: 'starting' | 'ready' | 'failed' | 'disposed';
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
  let transportClosed = false;
  let runtimeProgress = 0;
  const pendingOutputs: ProductBrowserRuntimeOutput[] = [];
  const maximumPendingOutputs = 64;

  // These are deliberately small, product-neutral observation markers. They
  // let an outer host prove that a mounted runtime is still making accepted
  // progress without inspecting a product's UI, facts, or content vocabulary.
  const publishHealth = (): void => {
    const document = options.root.ownerDocument;
    const roots = [options.root, document.body].filter((root): root is HTMLElement => root !== null);
    for (const root of roots) {
      root.dataset['rustyProductHostState'] = state;
      root.dataset['rustyProductRuntimeMode'] = options.lifecycleMode;
      root.dataset['rustyProductRuntimeProgress'] = String(runtimeProgress);
      if (failure === null) {
        delete root.dataset['rustyProductRuntimeFailure'];
      } else {
        root.dataset['rustyProductRuntimeFailure'] = boundedDiagnostic(failure.message);
      }
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

  const requireReady = (): void => {
    if (state === 'ready') return;
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
      failure = error;
      if (state !== 'disposed') state = 'failed';
      publishHealth();
    }
    return error;
  };

  let cadence: ProductBrowserCadence | null = null;
  const closeTransport = (): void => {
    if (transportClosed) return;
    transportClosed = true;
    started = false;
    cadence?.dispose();
    unsubscribeTerminalFailures?.();
    unsubscribeTerminalFailures = null;
    unsubscribeOutputs?.();
    unsubscribeOutputs = null;
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

  const applyOutput = (output: ProductBrowserRuntimeOutput): void => {
    if (application === null) {
      if (pendingOutputs.length >= maximumPendingOutputs) {
        failAndClose(
          new ProductBrowserHostError(
            'output_failed',
            `runtime output buffer exceeded ${String(maximumPendingOutputs)} entries before host mount`,
          ),
          'output_failed',
        );
        return;
      }
      pendingOutputs.push(output);
      return;
    }
    if (state === 'failed' || state === 'disposed') return;
    try {
      const host = requireApplication();
      switch (output.kind) {
        case 'binding':
          host.input?.bindRuntime({
            runtime: output.runtime,
            context: options.inputContext ?? 'gameplay.default',
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
            publishHealth();
          }
          cadence?.pulseRustHost();
          return;
        case 'frame': {
          const receipt = host.renderer.applyFrame(output.frame);
          if (!receipt.applied) {
            throw new ProductBrowserHostError(
              'output_failed',
              receipt.diagnostics.map((item) => item.message).join('; ') || 'retained frame was rejected',
            );
          }
          return;
        }
        case 'presentation':
          // `applyPresentation` is asynchronous because presentation owners
          // may retain resources, but output delivery itself stays on the
          // fixed typed port and never becomes a promise bus.
          void host.renderer.applyPresentation(output.frame).then((receipt) => {
            if (receipt.diagnostics.length > 0) {
              failAndClose(
                new ProductBrowserHostError(
                  'output_failed',
                  receipt.diagnostics.map((item) => item.message).join('; '),
                ),
                'output_failed',
              );
            }
          }).catch((cause: unknown) => {
            failAndClose(cause, 'output_failed');
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
          cadence?.pulseRustHost();
          return;
        default:
          assertNever(output);
      }
    } catch (cause) {
      failAndClose(cause, 'output_failed');
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
  ): void => {
    if (!result.accepted) {
      throw new ProductBrowserHostError(
        rejectedCode,
        result.diagnostic ?? `${result.operation} was rejected by the runtime`,
      );
    }
    if (result.binding !== undefined) applyOutput({ kind: 'binding', runtime: result.binding });
    if (result.readout !== undefined) applyOutput({ kind: 'runtime-readout', readout: result.readout });
  };

  const applyInputResult = (result: ProductBrowserRuntimeInputResult): void => {
    if (!result.accepted) {
      throw new ProductBrowserHostError(
        'transport_failed',
        result.diagnostic ?? 'runtime input batch was rejected by the runtime',
      );
    }
    if (result.binding !== undefined) applyOutput({ kind: 'binding', runtime: result.binding });
    if (result.readout !== undefined) applyOutput({ kind: 'runtime-readout', readout: result.readout });
  };

  cadence = createProductBrowserCadence({
    lifecycleMode: options.lifecycleMode,
    realtimeAdvanceOwner,
    isReady: () => started && state === 'ready',
    enqueueOperation: queue.enqueue,
    sampleInput: () => {
      const host = requireApplication();
      host.input?.sampleController();
      return host.input?.drain() ?? [];
    },
    sendInput: async (batch) => {
      applyInputResult(await transport.input(batch));
    },
    advanceRealtime: async (observedTimeNs) => {
      applyOperationResult(await transport.advanceRealtime(observedTimeNs));
      if (options.lifecycleMode === 'realtime' && realtimeAdvanceOwner === 'browser') {
        runtimeProgress += 1;
        publishHealth();
      }
    },
    onFailure: (cause) => {
      failAndClose(cause, 'transport_failed');
    },
  });

  let runtimeInput: RustyApplicationRuntimeInputOptions | undefined;
  if (options.runtimeInput !== undefined) {
    const { binding, ...runtimeInputOptions } = options.runtimeInput;
    runtimeInput = {
      ...runtimeInputOptions,
      ...(realtimeAdvanceOwner === 'rust-host'
        ? { onAvailable: () => cadence?.pulseRustHost() }
        : {}),
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
    unsubscribeOutputs = transport.subscribeOutputs(applyOutput);
    application = await mountRustyApplication({
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
      ...(options.renderer === undefined
        ? {
            renderer: {
              onCadence: (timeMs) => cadence?.enqueue(timeMs),
            },
          }
        : {
            renderer: {
              ...options.renderer,
              onCadence: (timeMs) => cadence?.enqueue(timeMs),
            },
          }),
    });
    const bufferedOutputs = pendingOutputs.splice(0, pendingOutputs.length);
    for (const output of bufferedOutputs) applyOutput(output);
    if (failure !== null) throw failure;
    if (options.autoStart !== false) {
      const result = await queue.enqueue(() => transport.lifecycle({ kind: 'start' }));
      applyOperationResult(result, 'startup_failed');
      if (failure !== null) throw failure;
    }
    started = true;
    state = 'ready';
    publishHealth();
  } catch (cause) {
    const error = reportFailure(cause, 'startup_failed');
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
    lastFailure: failure?.message ?? null,
  });

  const completeTimeline = (
    completion: ProductBrowserTimelineCompletion,
  ): Promise<ProductBrowserTimelineCompletionResult> => {
    try { requireReady(); } catch (cause) { return Promise.reject(cause); }
    if (transport.completeTimeline === undefined) {
      return Promise.reject(new ProductBrowserHostError(
        'timeline_unavailable',
        'this Product Assembly did not declare a timeline completion lane',
      ));
    }
    return queue.enqueue(async () => {
      const result = await transport.completeTimeline!(completion);
      if (!result.accepted) {
        throw new ProductBrowserHostError(
          'transport_failed',
          result.diagnostic ?? 'timeline completion was rejected by the runtime',
        );
      }
      if (result.binding !== undefined) applyOutput({ kind: 'binding', runtime: result.binding });
      if (result.readout !== undefined) applyOutput({ kind: 'runtime-readout', readout: result.readout });
      return result;
    }).catch((cause: unknown) => {
      throw failAndClose(cause, 'transport_failed');
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
        'this Product Assembly did not declare a demand-step transport lane',
      ));
    }
    return queue.enqueue(async () => {
      const host = requireApplication();
      host.input?.sampleController();
      const batch = host.input?.drain() ?? [];
      if (batch.length > 0) applyInputResult(await transport.input(batch));
      const result = await transport.admitDemandStep!();
      applyOperationResult(result);
      return result;
    }).catch((cause: unknown) => {
      throw failAndClose(cause, 'transport_failed');
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
        'this Product Assembly did not declare an external-step transport lane',
      ));
    }
    return queue.enqueue(async () => {
      const host = requireApplication();
      host.input?.sampleController();
      const batch = host.input?.drain() ?? [];
      if (batch.length > 0) applyInputResult(await transport.input(batch));
      const result = await transport.admitExternalStep!(step);
      applyOperationResult(result);
      return result;
    }).catch((cause: unknown) => {
      throw failAndClose(cause, 'transport_failed');
    });
  };

  const dispose = (): Promise<void> => {
    if (disposal !== null) return disposal;
    disposal = (async () => {
      if (state === 'disposed') return;
      state = 'disposed';
      started = false;
      publishHealth();
      cadence?.dispose();
      unsubscribeTerminalFailures?.();
      unsubscribeTerminalFailures = null;
      unsubscribeOutputs?.();
      unsubscribeOutputs = null;
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
    admitDemandStep,
    admitExternalStep,
    dispose,
  });
}

const MAXIMUM_HEALTH_DIAGNOSTIC_BYTES = 512;

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
  /** Defaults to `browser`; `rust-host` leaves realtime advancement to the packaged Rust host. */
  readonly realtimeAdvanceOwner?: ProductBrowserRealtimeAdvanceOwner;
  readonly uiProjection?: {
    readonly expectedStream: string;
    readonly expectedContract: string;
  } | null;
}

/**
 * Deterministic fixed composition assets copied by the Rusty CLI into the
 * ignored `generated/product-bundle` lane. Only source-linked module paths are
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
        `import { mountProductBrowserHost } from './${PRODUCT_BROWSER_BUNDLE_ENGINE_MODULE}';`,
        "import { createProductBridge } from './bridge.js';",
        `import { mountProductUi } from '${options.uiModule}';`,
        '',
        'const PRODUCT_RENDERER_PRELOAD_TEXTURE_MAX_COUNT = 256;',
        'const PRODUCT_RENDERER_PRELOAD_TEXTURE_MAX_TOTAL_BYTES = 128 * 1024 * 1024;',
        'const PRODUCT_RENDERER_PRELOAD_TEXTURE_MAX_BYTES = 16 * 1024 * 1024;',
        'const PRODUCT_RENDERER_PRELOAD_AUDIO_MAX_COUNT = 64;',
        'const PRODUCT_RENDERER_PRELOAD_AUDIO_MAX_TOTAL_BYTES = 32 * 1024 * 1024;',
        'const PRODUCT_RENDERER_PRELOAD_AUDIO_MAX_BYTES = 8 * 1024 * 1024;',
        'const PRODUCT_RENDERER_PRELOAD_MESH_MAX_COUNT = 1024;',
        'const PRODUCT_RENDERER_PRELOAD_MESH_MAX_TOTAL_BYTES = 64 * 1024 * 1024;',
        'const PRODUCT_RENDERER_PRELOAD_MESH_MAX_BYTES = 16 * 1024 * 1024;',
        '',
        "const root = document.querySelector('#application');",
        "if (root === null) throw new Error('generated Product Browser Host root is missing');",
        'const bridge = createProductBridge();',
        'const rendererInitialContent = await loadProductRendererInitialContent();',
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
        'async function loadProductRendererInitialContent() {',
        "  const descriptorUrl = new URL('./renderer-preload.json', import.meta.url);",
        "  const descriptorResponse = await fetch(descriptorUrl, { cache: 'no-store' });",
        "  if (!descriptorResponse.ok) throw new Error('generated renderer preload descriptor is unavailable');",
        '  const descriptor = decodeProductRendererPreload(await descriptorResponse.json());',
        '  const resources = await Promise.all(descriptor.resources.map(loadProductRendererResource));',
        '  return Object.freeze({',
        "    frame: Object.freeze({ schemaVersion: 1, ops: Object.freeze([]) }),",
        '    resources: Object.freeze(resources),',
        '  });',
        '}',
        '',
        'function decodeProductRendererPreload(value) {',
        "  if (value === null || typeof value !== 'object' || value.artifact !== 'rusty.product.renderer-preload.v1' || !Array.isArray(value.resources)) {",
        "    throw new Error('generated renderer preload descriptor is invalid');",
        '  }',
        '  let textureCount = 0; let textureBytes = 0; let audioCount = 0; let audioBytes = 0; let meshCount = 0; let meshBytes = 0;',
        '  const identities = new Set(); const paths = new Set();',
        '  return Object.freeze({ resources: Object.freeze(value.resources.map((resource, index) => {',
        "    if (resource === null || typeof resource !== 'object' || typeof resource.identity !== 'string' || typeof resource.contentHash !== 'string' || typeof resource.mediaType !== 'string' || typeof resource.path !== 'string' || !Number.isSafeInteger(resource.byteLength)) {",
        "      throw new Error(`generated renderer preload resource ${String(index)} is invalid`);",
        '    }',
        "    const match = /^(texture|audio|mesh)-resource\\/([0-9a-f]{64})$/u.exec(resource.identity);",
        "    if (match === null || resource.contentHash !== `sha256:${match[2]}` || !isSafeProductRendererPath(resource.path) || resource.byteLength < 0 || identities.has(resource.identity) || paths.has(resource.path)) {",
        "      throw new Error(`generated renderer preload resource ${String(index)} is inadmissible`);",
        '    }',
        '    identities.add(resource.identity); paths.add(resource.path);',
        "    if ((match[1] === 'texture' && (resource.mediaType !== 'image/png' || !resource.path.endsWith('.png'))) || (match[1] === 'audio' && (resource.mediaType !== 'audio/wav' || !resource.path.endsWith('.wav'))) || (match[1] === 'mesh' && (resource.mediaType !== 'application/octet-stream' || !resource.path.endsWith('.rmesh')))) {",
        "      throw new Error(`generated renderer preload resource ${String(index)} media is invalid`);",
        '    }',
        "    if (match[1] === 'texture') { textureCount += 1; textureBytes += resource.byteLength; if (textureCount > PRODUCT_RENDERER_PRELOAD_TEXTURE_MAX_COUNT || resource.byteLength === 0 || resource.byteLength > PRODUCT_RENDERER_PRELOAD_TEXTURE_MAX_BYTES || textureBytes > PRODUCT_RENDERER_PRELOAD_TEXTURE_MAX_TOTAL_BYTES) throw new Error(`generated renderer preload texture ${String(index)} exceeds application-host bounds`); }",
        "    else if (match[1] === 'audio') { audioCount += 1; audioBytes += resource.byteLength; if (audioCount > PRODUCT_RENDERER_PRELOAD_AUDIO_MAX_COUNT || resource.byteLength < 44 || resource.byteLength > PRODUCT_RENDERER_PRELOAD_AUDIO_MAX_BYTES || audioBytes > PRODUCT_RENDERER_PRELOAD_AUDIO_MAX_TOTAL_BYTES) throw new Error(`generated renderer preload audio ${String(index)} exceeds application-host bounds`); }",
        "    else { meshCount += 1; meshBytes += resource.byteLength; if (meshCount > PRODUCT_RENDERER_PRELOAD_MESH_MAX_COUNT || resource.byteLength < 16 || resource.byteLength > PRODUCT_RENDERER_PRELOAD_MESH_MAX_BYTES || meshBytes > PRODUCT_RENDERER_PRELOAD_MESH_MAX_TOTAL_BYTES) throw new Error(`generated renderer preload mesh ${String(index)} exceeds application-host bounds`); }",
        '    return Object.freeze({ identity: resource.identity, contentHash: resource.contentHash, mediaType: resource.mediaType, path: resource.path, byteLength: resource.byteLength });',
        '  })) });',
        '}',
        '',
        'function isSafeProductRendererPath(path) {',
        "  return typeof path === 'string' && path.startsWith('content/') && new TextEncoder().encode(path).byteLength <= 512 && !path.startsWith('/') && !path.startsWith('//') && !path.includes('\\\\') && !path.includes('%') && !path.includes(':') && !/[\\u0000-\\u001f\\u007f]/u.test(path) && !/\\s/u.test(path) && path.split('/').every((part) => part.length > 0 && part !== '.' && part !== '..');",
        '}',
        '',
        'async function loadProductRendererResource(resource) {',
        '  const url = new URL(`./${resource.path}`, import.meta.url);',
        '  if (url.origin !== new URL(import.meta.url).origin) throw new Error(\'generated renderer resource must remain same-origin\');',
        "  const response = await fetch(url, { cache: 'no-store' });",
        "  if (!response.ok) throw new Error(`generated renderer resource ${resource.identity} is unavailable`);",
        '  const bytes = new Uint8Array(await response.arrayBuffer());',
        "  if (bytes.byteLength !== resource.byteLength) throw new Error(`generated renderer resource ${resource.identity} length mismatch`);",
        "  if (bytes.byteLength === 0 || (resource.mediaType === 'image/png' && !hasPngSignature(bytes)) || (resource.mediaType === 'audio/wav' && !hasWavSignature(bytes)) || (resource.mediaType === 'application/octet-stream' && !hasMeshResourceHeader(bytes))) throw new Error(`generated renderer resource ${resource.identity} media mismatch`);",
        "  if (typeof crypto === 'undefined' || crypto.subtle === undefined) throw new Error('Web Crypto SHA-256 is required for generated renderer preload');",
        "  const digest = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))).map((byte) => byte.toString(16).padStart(2, '0')).join('');",
        "  if (resource.contentHash !== `sha256:${digest}`) throw new Error(`generated renderer resource ${resource.identity} hash mismatch`);",
        '  return Object.freeze({ identity: resource.identity, contentHash: resource.contentHash, mediaType: resource.mediaType, bytes });',
        '}',
        '',
        'function hasPngSignature(bytes) {',
        '  return bytes.byteLength >= 8 && bytes[0] === 137 && bytes[1] === 80 && bytes[2] === 78 && bytes[3] === 71 && bytes[4] === 13 && bytes[5] === 10 && bytes[6] === 26 && bytes[7] === 10;',
        '}',
        '',
        'function hasWavSignature(bytes) {',
        '  return bytes.byteLength >= 44 && bytes[0] === 82 && bytes[1] === 73 && bytes[2] === 70 && bytes[3] === 70 && bytes[8] === 87 && bytes[9] === 65 && bytes[10] === 86 && bytes[11] === 69;',
        '}',
        '',
        'function hasMeshResourceHeader(bytes) {',
        '  if (bytes.byteLength < 16) return false;',
        '  const magic = [82, 77, 83, 72, 76, 69, 48];',
        '  const version = bytes[7];',
        '  if ((version !== 49 && version !== 50 && version !== 51) || magic.some((byte, index) => bytes[index] !== byte)) return false;',
        '  const header = new DataView(bytes.buffer, bytes.byteOffset, 16);',
        '  return header.getUint32(8, true) === bytes.byteLength && header.getUint32(12, true) !== 0;',
        '}',
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
        `    realtimeAdvanceOwner: ${JSON.stringify(options.realtimeAdvanceOwner ?? 'browser')},`,
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
