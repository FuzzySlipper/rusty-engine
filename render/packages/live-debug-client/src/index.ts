/**
 * Transport-neutral client for the product-owned generated live-debug catalog.
 * Descriptor data is read-only help/completion data; this client never derives
 * command schemas or dispatches anything except one command-line string.
 */

export interface LiveDebugParameterDescriptor {
  readonly name: string;
  readonly type: string;
}

export interface LiveDebugCommandDescriptor {
  readonly name: string;
  readonly description: string;
  readonly parameters: readonly LiveDebugParameterDescriptor[];
}

export interface LiveDebugCatalog {
  readonly available: boolean;
  readonly commands: readonly LiveDebugCommandDescriptor[];
}

export interface LiveDebugResult {
  readonly succeeded: boolean;
  readonly message: string;
}

/** Bounded process-owned diagnostic event; it is not the presentation stream. */
export interface LiveDebugDiagnosticEvent {
  readonly sequence: string;
  readonly monotonicNanoseconds: string;
  readonly severity: 'debug' | 'info' | 'warning' | 'error';
  readonly disposition: 'accepted' | 'rejected-recoverable' | 'degraded' | 'resync-required' | 'terminal';
  readonly source: string;
  readonly code: string;
  readonly message: string;
  readonly fields?: readonly { readonly key: string; readonly value: string }[];
}

export interface LiveDebugDiagnosticsBatch {
  readonly events: readonly LiveDebugDiagnosticEvent[];
  readonly floorSequence: string;
  readonly throughSequence: string;
  readonly nextCursor: string;
  readonly readMonotonicNanoseconds: string;
  readonly lagged: boolean;
  readonly warningCount: string;
  readonly errorCount: string;
  readonly droppedCount: string;
  /** Optional host-owned product lane facts; renderer telemetry is separate. */
  readonly telemetry?: LiveDebugTelemetrySnapshot;
}

export type LiveDebugOperationKind =
  | 'connect'
  | 'start'
  | 'pause'
  | 'resume'
  | 'restart'
  | 'shutdown'
  | 'report-fault'
  | 'replace-control'
  | 'release-control'
  | 'input'
  | 'advance-realtime'
  | 'admit-demand-step'
  | 'admit-external-step'
  | 'complete-timeline'
  | 'report-audio-feedback'
  | 'report-animation-feedback'
  | 'report-ghost-plate-feedback'
  | 'report-renderer-diagnostics'
  | 'execute-debug';

/** Bounded product/runtime lane observations returned by the Engine host. */
export interface LiveDebugTelemetrySnapshot {
  readonly inFlightOperation: LiveDebugOperationKind | null;
  readonly inFlightAgeMs: string | null;
  readonly lastProductAdmissionLatencyMs: string | null;
  readonly lastInputAdmissionLatencyMs: string | null;
  readonly queuedInputBatches: number;
  readonly queuedInputEvents: number;
  readonly inputBatchCapacity: number;
  readonly oldestInputAgeMs: string | null;
  readonly inputOverflowPending: boolean;
  /** Progress rate in millihertz (1000 = one update per second). */
  readonly runtimeProgressRateMillihertz: string | null;
  readonly runtimeProgressAgeMs: string | null;
  readonly connections: number;
  readonly subscribers: number;
  readonly outputQueueItems: number;
  readonly outputQueueCapacity: number;
  readonly outputQueueFloor: string;
  readonly outputBindingActive: boolean;
}

export interface LiveDebugTransport {
  catalog(signal?: AbortSignal): Promise<LiveDebugCatalog>;
  execute(command: string, signal?: AbortSignal): Promise<LiveDebugResult>;
  diagnostics?(after?: string, signal?: AbortSignal): Promise<LiveDebugDiagnosticsBatch>;
}

export interface LiveDebugHttpTransportOptions {
  /** Defaults to the current page origin, preserving same-origin dev-host use. */
  readonly origin?: string;
  readonly fetch?: typeof globalThis.fetch;
}

const CATALOG_PATH = '/__rusty/product/runtime/debug/catalog';
const EXECUTE_PATH = '/__rusty/product/runtime/debug/execute';
const DIAGNOSTICS_READ_PATH = '/__rusty/product/runtime/diagnostics/read';
const U64_MAX = 18_446_744_073_709_551_615n;

/** Creates the default same-origin HTTP transport without owning UI state. */
export function createLiveDebugHttpTransport(options: LiveDebugHttpTransportOptions = {}): LiveDebugTransport {
  const request = options.fetch ?? globalThis.fetch;
  const origin = options.origin ?? globalThis.location?.origin;
  if (origin === undefined || origin === 'null') throw new Error('A live-debug HTTP origin is required outside a browser page.');
  const url = (path: string): string => new URL(path, origin).toString();
  return {
    async catalog(signal?: AbortSignal): Promise<LiveDebugCatalog> {
      const response = await request(url(CATALOG_PATH), { method: 'GET', signal });
      if (response.status === 404) return { available: false, commands: [] };
      return decodeCatalog(await requireSuccess(response));
    },
    async execute(command: string, signal?: AbortSignal): Promise<LiveDebugResult> {
      const response = await request(url(EXECUTE_PATH), {
        method: 'POST', signal, headers: { 'content-type': 'text/plain; charset=utf-8' }, body: command,
      });
      const message = await response.text();
      if (response.status === 200) return { succeeded: true, message };
      if (response.status === 422) return { succeeded: false, message };
      throw new Error(message || `Live-debug host request failed (${response.status}).`);
    },
    async diagnostics(after?: string, signal?: AbortSignal): Promise<LiveDebugDiagnosticsBatch> {
      if (after !== undefined && !canonicalU64(after)) {
        throw new Error('Live-debug diagnostics cursor is invalid.');
      }
      const response = await request(url(DIAGNOSTICS_READ_PATH), {
        method: 'POST', signal, headers: { 'content-type': 'application/json' },
        body: JSON.stringify(after === undefined ? {} : { after }),
      });
      return decodeDiagnosticsBatch(await requireSuccess(response));
    },
  };
}

/** Small UI/CLI-neutral helper for catalog-derived completion. */
export function completeLiveDebug(catalog: LiveDebugCatalog, prefix: string): readonly LiveDebugCommandDescriptor[] {
  return catalog.commands.filter((command) => command.name.startsWith(prefix));
}

async function requireSuccess(response: Response): Promise<unknown> {
  const body: unknown = await response.json().catch(() => null);
  if (!response.ok) {
    const error = body as { error?: { code?: unknown; diagnostic?: unknown } } | null;
    const code = typeof error?.error?.code === 'string' ? error.error.code : `HTTP_${response.status}`;
    const diagnostic = typeof error?.error?.diagnostic === 'string' ? error.error.diagnostic : 'Live-debug host request failed.';
    throw new Error(`${code}: ${diagnostic}`);
  }
  return body;
}

function decodeCatalog(value: unknown): LiveDebugCatalog {
  const candidate = object(value);
  if (typeof candidate.available !== 'boolean' || !Array.isArray(candidate.commands)) throw new Error('Live-debug catalog response is invalid.');
  if (!candidate.available) {
    if (candidate.commands.length !== 0) throw new Error('Unavailable live-debug catalogs cannot carry commands.');
    return { available: false, commands: [] };
  }
  return { available: true, commands: candidate.commands.map(decodeCommand) };
}

function decodeCommand(value: unknown): LiveDebugCommandDescriptor {
  const candidate = object(value);
  if (typeof candidate.name !== 'string' || typeof candidate.description !== 'string' || !Array.isArray(candidate.parameters)) throw new Error('Live-debug command descriptor is invalid.');
  return { name: candidate.name, description: candidate.description, parameters: candidate.parameters.map(decodeParameter) };
}

function decodeParameter(value: unknown): LiveDebugParameterDescriptor {
  const candidate = object(value);
  if (typeof candidate.name !== 'string' || typeof candidate.type !== 'string') throw new Error('Live-debug parameter descriptor is invalid.');
  return { name: candidate.name, type: candidate.type };
}

function decodeDiagnosticsBatch(value: unknown): LiveDebugDiagnosticsBatch {
  const candidate = object(value);
  if (!Array.isArray(candidate.events)
    || !canonicalU64(candidate.floorSequence)
    || !canonicalU64(candidate.throughSequence)
    || !canonicalU64(candidate.nextCursor)
    || !canonicalU64(candidate.readMonotonicNanoseconds)
    || typeof candidate.lagged !== 'boolean'
    || !canonicalU64(candidate.warningCount)
    || !canonicalU64(candidate.errorCount)
    || !canonicalU64(candidate.droppedCount)) {
    throw new Error('Live-debug diagnostics response is invalid.');
  }
  return Object.freeze({
    events: Object.freeze(candidate.events.map(decodeDiagnosticEvent)),
    floorSequence: candidate.floorSequence,
    throughSequence: candidate.throughSequence,
    nextCursor: candidate.nextCursor,
    readMonotonicNanoseconds: candidate.readMonotonicNanoseconds,
    lagged: candidate.lagged,
    warningCount: candidate.warningCount,
    errorCount: candidate.errorCount,
    droppedCount: candidate.droppedCount,
    ...(candidate.telemetry === undefined ? {} : { telemetry: decodeTelemetrySnapshot(candidate.telemetry) }),
  });
}

function decodeTelemetrySnapshot(value: unknown): LiveDebugTelemetrySnapshot {
  const candidate = object(value);
  const fields = [
    'inFlightOperation', 'inFlightAgeMs', 'lastProductAdmissionLatencyMs',
    'lastInputAdmissionLatencyMs', 'queuedInputBatches', 'queuedInputEvents',
    'inputBatchCapacity', 'oldestInputAgeMs', 'inputOverflowPending',
    'runtimeProgressRateMillihertz', 'runtimeProgressAgeMs', 'connections',
    'subscribers', 'outputQueueItems', 'outputQueueCapacity', 'outputQueueFloor',
    'outputBindingActive',
  ];
  if (Object.keys(candidate).some((key) => !fields.includes(key))) {
    throw new Error('Live-debug telemetry snapshot contains unknown fields.');
  }
  const operation = candidate.inFlightOperation;
  const admittedOperations: readonly LiveDebugOperationKind[] = [
    'connect', 'start', 'pause', 'resume', 'restart', 'shutdown', 'report-fault',
    'replace-control', 'release-control', 'input', 'advance-realtime', 'admit-demand-step',
    'admit-external-step', 'complete-timeline', 'report-audio-feedback',
    'report-animation-feedback', 'report-ghost-plate-feedback',
    'report-renderer-diagnostics', 'execute-debug',
  ];
  if (operation !== null && !admittedOperations.includes(operation as LiveDebugOperationKind)) {
    throw new Error('Live-debug telemetry in-flight operation is invalid.');
  }
  for (const field of [
    'inFlightAgeMs', 'lastProductAdmissionLatencyMs', 'lastInputAdmissionLatencyMs',
    'oldestInputAgeMs', 'runtimeProgressRateMillihertz', 'runtimeProgressAgeMs',
  ]) {
    if (candidate[field] !== null && !canonicalU64(candidate[field])) {
      throw new Error(`Live-debug telemetry ${field} is invalid.`);
    }
  }
  for (const field of [
    'queuedInputBatches', 'queuedInputEvents', 'inputBatchCapacity', 'connections',
    'subscribers', 'outputQueueItems', 'outputQueueCapacity',
  ]) {
    if (!boundedCount(candidate[field])) throw new Error(`Live-debug telemetry ${field} is invalid.`);
  }
  if (typeof candidate.inputOverflowPending !== 'boolean'
    || typeof candidate.outputBindingActive !== 'boolean'
    || !canonicalU64(candidate.outputQueueFloor)) {
    throw new Error('Live-debug telemetry snapshot is invalid.');
  }
  return Object.freeze({
    inFlightOperation: operation as LiveDebugOperationKind | null,
    inFlightAgeMs: candidate.inFlightAgeMs as string | null,
    lastProductAdmissionLatencyMs: candidate.lastProductAdmissionLatencyMs as string | null,
    lastInputAdmissionLatencyMs: candidate.lastInputAdmissionLatencyMs as string | null,
    queuedInputBatches: candidate.queuedInputBatches as number,
    queuedInputEvents: candidate.queuedInputEvents as number,
    inputBatchCapacity: candidate.inputBatchCapacity as number,
    oldestInputAgeMs: candidate.oldestInputAgeMs as string | null,
    inputOverflowPending: candidate.inputOverflowPending,
    runtimeProgressRateMillihertz: candidate.runtimeProgressRateMillihertz as string | null,
    runtimeProgressAgeMs: candidate.runtimeProgressAgeMs as string | null,
    connections: candidate.connections as number,
    subscribers: candidate.subscribers as number,
    outputQueueItems: candidate.outputQueueItems as number,
    outputQueueCapacity: candidate.outputQueueCapacity as number,
    outputQueueFloor: candidate.outputQueueFloor,
    outputBindingActive: candidate.outputBindingActive,
  });
}

function boundedCount(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 && value <= 1_000_000;
}

function decodeDiagnosticEvent(value: unknown): LiveDebugDiagnosticEvent {
  const candidate = object(value);
  const severity = candidate.severity;
  const disposition = candidate.disposition;
  if (!canonicalU64(candidate.sequence) || !canonicalU64(candidate.monotonicNanoseconds)
    || !['debug', 'info', 'warning', 'error'].includes(String(severity))
    || !['accepted', 'rejected-recoverable', 'degraded', 'resync-required', 'terminal'].includes(String(disposition))
    || typeof candidate.source !== 'string' || typeof candidate.code !== 'string' || typeof candidate.message !== 'string') {
    throw new Error('Live-debug diagnostic event is invalid.');
  }
  const fields = candidate.fields === undefined ? undefined : decodeDiagnosticFields(candidate.fields);
  return Object.freeze({
    sequence: candidate.sequence,
    monotonicNanoseconds: candidate.monotonicNanoseconds,
    severity: severity as LiveDebugDiagnosticEvent['severity'],
    disposition: disposition as LiveDebugDiagnosticEvent['disposition'],
    source: candidate.source,
    code: candidate.code,
    message: candidate.message,
    ...(fields === undefined ? {} : { fields }),
  });
}

function decodeDiagnosticFields(value: unknown): readonly { readonly key: string; readonly value: string }[] {
  if (!Array.isArray(value) || value.length > 8) throw new Error('Live-debug diagnostic fields are invalid.');
  return Object.freeze(value.map((field) => {
    const candidate = object(field);
    if (typeof candidate.key !== 'string' || typeof candidate.value !== 'string') {
      throw new Error('Live-debug diagnostic field is invalid.');
    }
    return Object.freeze({ key: candidate.key, value: candidate.value });
  }));
}

/** Computes a browser renderer observation age from the process-owned sink clock. */
export function diagnosticRendererObservationAgeMilliseconds(
  batch: LiveDebugDiagnosticsBatch,
  event: LiveDebugDiagnosticEvent,
): number | null {
  if (event.source !== 'browser-host') return null;
  const encodedAge = event.fields?.find((field) => field.key === 'renderer-observation-age-ms')?.value;
  if (encodedAge === undefined || !/^\d+$/u.test(encodedAge)) return null;
  const reportedAge = BigInt(encodedAge);
  const elapsed = BigInt(batch.readMonotonicNanoseconds) - BigInt(event.monotonicNanoseconds);
  const age = reportedAge + (elapsed > 0n ? elapsed / 1_000_000n : 0n);
  return age > BigInt(Number.MAX_SAFE_INTEGER) ? null : Number(age);
}

/**
 * Computes how old a diagnostic event is at the response read clock. This is
 * distinct from any age fact carried by the event itself (for example the
 * browser host's renderer observation age).
 */
export function diagnosticEventAgeMilliseconds(
  batch: LiveDebugDiagnosticsBatch,
  event: LiveDebugDiagnosticEvent,
): number | null {
  const elapsed = BigInt(batch.readMonotonicNanoseconds) - BigInt(event.monotonicNanoseconds);
  if (elapsed < 0n || elapsed / 1_000_000n > BigInt(Number.MAX_SAFE_INTEGER)) return null;
  return Number(elapsed / 1_000_000n);
}

function canonicalU64(value: unknown): value is string {
  return typeof value === 'string'
    && /^(?:0|[1-9]\d*)$/u.test(value)
    && BigInt(value) <= U64_MAX;
}

function object(value: unknown): Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new Error('Live-debug response is invalid.');
  return value as Record<string, unknown>;
}
