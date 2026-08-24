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

/** Fixed current artifact identity; compatibility follows actual code changes. */
export const PRODUCT_BROWSER_HOST_ARTIFACT = 'rusty.product.browser-host' as const;

export type ProductBrowserRuntimeMode = 'realtime' | 'demand' | 'external';

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
  readonly kind: 'output-lag';
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
  readonly mountUi: RustyApplicationUiMount;
  readonly runtimeInput?: Omit<RustyApplicationRuntimeInputOptions, 'binding'> & {
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
 * it drains the public input port and advances the supplied runtime only from
 * the application-host's existing renderer cadence callback.
 */
export async function mountProductBrowserHost(
  options: ProductBrowserHostOptions,
): Promise<ProductBrowserHost> {
  validateOptions(options);
  const transport = options.transport;
  const queue = createOperationQueue();
  let state: ProductBrowserHostReadout['state'] = 'starting';
  let runtimeReadout: ProductBrowserRuntimeReadout | null = null;
  let application: RustyApplicationHost | null = null;
  let unsubscribeOutputs: (() => void) | null = null;
  let unsubscribeTerminalFailures: (() => void) | null = null;
  let disposal: Promise<void> | null = null;
  let started = false;
  let cadenceInFlight = false;
  let pendingCadenceTimeMs: number | null = null;
  let failure: ProductBrowserHostError | null = null;
  const pendingOutputs: ProductBrowserRuntimeOutput[] = [];
  const maximumPendingOutputs = 64;

  const requireApplication = (): RustyApplicationHost => {
    if (application === null || state === 'disposed') {
      throw new ProductBrowserHostError(
        'disposed',
        'Product Browser Host is disposed or has not mounted',
      );
    }
    return application;
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
    }
    return error;
  };

  const applyOutput = (output: ProductBrowserRuntimeOutput): void => {
    if (application === null) {
      if (pendingOutputs.length >= maximumPendingOutputs) {
        reportFailure(
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
              reportFailure(
                new ProductBrowserHostError(
                  'output_failed',
                  receipt.diagnostics.map((item) => item.message).join('; '),
                ),
                'output_failed',
              );
            }
          }).catch((cause: unknown) => {
            reportFailure(cause, 'output_failed');
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
      reportFailure(cause, 'output_failed');
    }
  };

  const applyTerminalFailure = (terminalFailure: ProductBrowserRuntimeTerminalFailure): void => {
    reportFailure(
      new ProductBrowserHostError('transport_failed', terminalFailure.diagnostic),
      'transport_failed',
    );
    // A retained-output gap cannot be resumed safely. Close the local
    // transport immediately; the host remains visibly failed until a fresh
    // runtime snapshot is mounted.
    void Promise.resolve(transport.dispose()).catch((cause: unknown) => {
      reportFailure(cause, 'transport_failed');
    });
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

  const enqueueCadence = (timeMs: number): void => {
    if (!started || state !== 'ready') return;
    if (cadenceInFlight) {
      // Keep only the newest observed host time while the Rust operation is
      // outstanding. Input remains in application-host's bounded ingress
      // queue; one slow local request must not create one promise per RAF.
      pendingCadenceTimeMs = timeMs;
      return;
    }
    cadenceInFlight = true;
    const operation = queue.enqueue(async () => {
      const host = requireApplication();
      host.input?.sampleController();
      const batch = host.input?.drain() ?? [];
      if (batch.length > 0) applyInputResult(await transport.input(batch));
      if (options.lifecycleMode === 'realtime') {
        const result = await transport.advanceRealtime(toNanoseconds(timeMs));
        applyOperationResult(result);
      }
    });
    void operation.then(
      () => finishCadence(),
      (cause: unknown) => {
        reportFailure(cause, 'transport_failed');
        finishCadence();
      },
    );
  };

  const finishCadence = (): void => {
    cadenceInFlight = false;
    const nextTimeMs = pendingCadenceTimeMs;
    pendingCadenceTimeMs = null;
    if (nextTimeMs !== null && started && state === 'ready') enqueueCadence(nextTimeMs);
  };

  let runtimeInput: RustyApplicationRuntimeInputOptions | undefined;
  if (options.runtimeInput !== undefined) {
    const { binding, ...runtimeInputOptions } = options.runtimeInput;
    runtimeInput = {
      ...runtimeInputOptions,
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
              onCadence: enqueueCadence,
            },
          }
        : {
            renderer: {
              ...options.renderer,
              onCadence: enqueueCadence,
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
    host: application?.readout() ?? null,
    runtime: runtimeReadout,
    lastFailure: failure?.message ?? null,
  });

  const completeTimeline = (
    completion: ProductBrowserTimelineCompletion,
  ): Promise<ProductBrowserTimelineCompletionResult> => {
    if (state === 'disposed') {
      return Promise.reject(new ProductBrowserHostError('disposed', 'Product Browser Host is disposed'));
    }
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
      throw reportFailure(cause, 'transport_failed');
    });
  };

  const admitDemandStep = (): Promise<ProductBrowserRuntimeOperationResult> => {
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
      throw reportFailure(cause, 'transport_failed');
    });
  };

  const admitExternalStep = (step: string): Promise<ProductBrowserRuntimeOperationResult> => {
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
      throw reportFailure(cause, 'transport_failed');
    });
  };

  const dispose = (): Promise<void> => {
    if (disposal !== null) return disposal;
    disposal = (async () => {
      if (state === 'disposed') return;
      state = 'disposed';
      started = false;
      pendingCadenceTimeMs = null;
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

function toNanoseconds(timeMs: number): string {
  if (!Number.isFinite(timeMs) || timeMs < 0) return '0';
  const nanoseconds = BigInt(Math.round(timeMs * 1_000_000));
  return nanoseconds.toString(10);
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
        "const root = document.querySelector('#application');",
        "if (root === null) throw new Error('generated Product Browser Host root is missing');",
        'const bridge = createProductBridge();',
        'const host = await mountProductBrowserHost({',
        '  root,',
        '  transport: bridge.transport,',
        '  lifecycleMode: bridge.lifecycleMode,',
        "  initialInteractionMode: 'gameplay',",
        '  mountUi: mountProductUi,',
        '  uiProjection: bridge.uiProjection,',
        '  runtimeInput: bridge.runtimeInput,',
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
