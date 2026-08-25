import type {
  RustyApplicationFrame,
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
import type {
  ProductBrowserLifecycleOperation,
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
  outputs: 'outputs',
});

const MAXIMUM_RUNTIME_RESPONSE_BYTES = 512 * 1024;
const MAXIMUM_RUNTIME_OUTPUT_BYTES = 256 * 1024;
const DEFAULT_MAXIMUM_RESPONSE_BYTES = MAXIMUM_RUNTIME_RESPONSE_BYTES;
const DEFAULT_MAXIMUM_OUTPUT_BYTES = MAXIMUM_RUNTIME_OUTPUT_BYTES;
const MAXIMUM_CONFIGURED_BYTES = 16 * 1024 * 1024;
const UINT64_MAX_DECIMAL = '18446744073709551615';
const MAXIMUM_INPUT_BATCH_LENGTH = 1_024;
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
  readonly lastObservedTimeNs?: unknown;
}

export type ProductBrowserLocalFetch = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>;

/** Minimal EventSource shape kept injectable for deterministic headless tests. */
export interface ProductBrowserLocalEventSource {
  onmessage: ((event: { readonly data: string }) => void) | null;
  onerror: ((event: unknown) => void) | null;
  readonly addEventListener?: (
    type: 'rusty-output-lag',
    listener: (event: { readonly data: string }) => void,
  ) => void;
  readonly removeEventListener?: (
    type: 'rusty-output-lag',
    listener: (event: { readonly data: string }) => void,
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
  let streamLagListener: ((event: { readonly data: string }) => void) | null = null;
  let terminalFailure: ProductBrowserRuntimeTerminalFailure | null = null;
  const listeners = new Set<(output: ProductBrowserRuntimeOutput) => void>();
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
      stream.close();
      stream = null;
    }
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

  const post = async <T>(route: string, body: unknown, decode: (value: unknown) => T): Promise<T> => {
    ensureOpen();
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
        signal: abortController.signal,
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
    try {
      ensureOpen();
      return decode(value);
    } catch (cause) {
      if (cause instanceof ProductBrowserLocalTransportError) throw cause;
      throw new ProductBrowserLocalTransportError(
        'response_decode_failed',
        `Product Browser local runtime returned an invalid response for ${route}: ${cause instanceof Error ? cause.message : String(cause)}`,
        { cause, route },
      );
    }
  };

  const lifecycle = (operation: ProductBrowserLifecycleOperation): Promise<ProductBrowserRuntimeOperationResult> =>
    post(ROUTES.lifecycle[operation.kind], {}, (value) => decodeOperationResult(value, operation.kind));

  const input = (
    batch: readonly RustyApplicationRuntimeInputEnvelope[],
  ): Promise<ProductBrowserRuntimeInputResult> =>
    post(ROUTES.input, { batch: snapshotInputBatch(batch) }, decodeInputResult);

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
        stream = new eventSourceConstructor(`${basePath}${ROUTES.outputs}`);
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
        stream.onmessage = (event) => {
          if (terminalFailure !== null) return;
          try {
            const raw = parseBoundedJson(event.data, maximumOutputBytes);
            const output = decodeRuntimeOutput(raw);
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
          } catch (cause) {
            const error = cause instanceof ProductBrowserLocalTransportError
              ? cause
              : new ProductBrowserLocalTransportError(
                'output_decode_failed',
                `Product Browser local runtime emitted an invalid output: ${cause instanceof Error ? cause.message : String(cause)}`,
                { cause, route: ROUTES.outputs },
              );
            reportTransportError(error);
          }
        };
        stream.onerror = (event) => {
          if (terminalFailure !== null) return;
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
        stream?.close();
        stream = null;
      }
    };
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
    stream?.close();
    stream = null;
    listeners.clear();
    terminalFailureListeners.clear();
  };

  return Object.freeze({
    lifecycle,
    input,
    advanceRealtime,
    admitDemandStep,
    admitExternalStep,
    completeTimeline,
    subscribeTerminalFailures,
    subscribeOutputs,
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

function requireIdentity(value: unknown, name: string): string {
  return requireProductIdentity(value, name);
}

function requireProductIdentity(value: unknown, name: string): string {
  if (typeof value !== 'string'
    || new TextEncoder().encode(value).byteLength > 128
    || !/^[a-z0-9](?:[a-z0-9]|[._-](?=[a-z0-9]))*$/u.test(value)) {
    throw new ProductBrowserLocalTransportError(
      'invalid_options',
      `${name} must be a 1..128 byte lowercase Product Model identity`,
    );
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

function decodeOperationResult(
  value: unknown,
  expectedOperation: ProductBrowserRuntimeOperationKind,
): ProductBrowserRuntimeOperationResult {
  const record = requireRecord(value, 'operation result');
  requireKnownFields(record, ['accepted', 'operation', 'binding', 'readout', 'diagnostic'], 'operation result');
  if (record.accepted !== true && record.accepted !== false) {
    throw new TypeError('accepted must be boolean');
  }
  if (record.operation !== expectedOperation) {
    throw new TypeError(`operation must be ${expectedOperation}`);
  }
  if (record.accepted === false && (record.binding !== undefined || record.readout !== undefined)) {
    throw new TypeError('rejected operation result cannot include binding or readout');
  }
  if (record.accepted === true && record.diagnostic !== undefined) {
    throw new TypeError('accepted operation result cannot include diagnostic');
  }
  return {
    accepted: record.accepted,
    operation: expectedOperation,
    ...(record.binding === undefined ? {} : { binding: decodeRuntimeIdentity(record.binding) }),
    ...(record.readout === undefined ? {} : { readout: decodeRuntimeReadout(record.readout) }),
    ...(record.diagnostic === undefined ? {} : { diagnostic: requireDiagnostic(record.diagnostic) }),
  };
}

function decodeInputResult(value: unknown): ProductBrowserRuntimeInputResult {
  const record = requireRecord(value, 'input result');
  requireKnownFields(record, ['accepted', 'count', 'binding', 'readout', 'diagnostic'], 'input result');
  if (record.accepted !== true && record.accepted !== false) {
    throw new TypeError('accepted must be boolean');
  }
  if (!Number.isSafeInteger(record.count)
    || (record.count as number) < 0
    || (record.count as number) > MAXIMUM_INPUT_BATCH_LENGTH) {
    throw new TypeError(`count must be a non-negative integer no greater than ${String(MAXIMUM_INPUT_BATCH_LENGTH)}`);
  }
  if (record.accepted === false && (record.binding !== undefined || record.readout !== undefined)) {
    throw new TypeError('rejected input result cannot include binding or readout');
  }
  if (record.accepted === true && record.diagnostic !== undefined) {
    throw new TypeError('accepted input result cannot include diagnostic');
  }
  return {
    accepted: record.accepted,
    count: record.count as number,
    ...(record.binding === undefined ? {} : { binding: decodeRuntimeIdentity(record.binding) }),
    ...(record.readout === undefined ? {} : { readout: decodeRuntimeReadout(record.readout) }),
    ...(record.diagnostic === undefined ? {} : { diagnostic: requireDiagnostic(record.diagnostic) }),
  };
}

function decodeTimelineCompletionResult(
  value: unknown,
  expectedTicket: string,
): ProductBrowserTimelineCompletionResult {
  const record = requireRecord(value, 'timeline completion result');
  requireKnownFields(record, ['accepted', 'ticket', 'binding', 'readout', 'diagnostic'], 'timeline completion result');
  if (record.accepted !== true && record.accepted !== false) {
    throw new TypeError('accepted must be boolean');
  }
  const ticket = requireU64Text(record.ticket, 'timeline result ticket');
  if (ticket !== expectedTicket) throw new TypeError('ticket does not match completion request');
  if (record.accepted === false && (record.binding !== undefined || record.readout !== undefined)) {
    throw new TypeError('rejected timeline result cannot include binding or readout');
  }
  if (record.accepted === true && record.diagnostic !== undefined) {
    throw new TypeError('accepted timeline result cannot include diagnostic');
  }
  return {
    accepted: record.accepted,
    ticket,
    ...(record.binding === undefined ? {} : { binding: decodeRuntimeIdentity(record.binding) }),
    ...(record.readout === undefined ? {} : { readout: decodeRuntimeReadout(record.readout) }),
    ...(record.diagnostic === undefined ? {} : { diagnostic: requireDiagnostic(record.diagnostic) }),
  };
}

function decodeRuntimeOutput(value: unknown): ProductBrowserRuntimeOutput {
  const record = requireRecord(value, 'runtime output');
  switch (record.kind) {
    case 'binding':
      requireKnownFields(record, ['kind', 'runtime'], 'binding output');
      return { kind: 'binding', runtime: decodeRuntimeIdentity(record.runtime) };
    case 'frame':
      requireKnownFields(record, ['kind', 'frame'], 'frame output');
      return { kind: 'frame', frame: decodeFrame(record.frame, 'frame') };
    case 'presentation':
      requireKnownFields(record, ['kind', 'frame'], 'presentation output');
      return { kind: 'presentation', frame: decodeFrame(record.frame, 'presentation') };
    case 'ui-projection':
      requireKnownFields(record, ['kind', 'envelope'], 'UI projection output');
      return { kind: 'ui-projection', envelope: decodeUiProjection(record.envelope) };
    case 'runtime-readout':
      requireKnownFields(record, ['kind', 'readout'], 'runtime readout output');
      return { kind: 'runtime-readout', readout: decodeRuntimeReadout(record.readout) };
    default:
      throw new TypeError('runtime output kind is not admitted');
  }
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
