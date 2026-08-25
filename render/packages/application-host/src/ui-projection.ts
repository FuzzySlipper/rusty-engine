import type { RustyApplicationRuntimeIdentity } from './input-ingress.js';

/** The one Product UI projection artifact admitted by the application host. */
export const RUSTY_APPLICATION_UI_PROJECTION_ARTIFACT =
  'rusty.product.ui-projection' as const;

export const RUSTY_APPLICATION_UI_PROJECTION_DEFAULT_STREAM = 'product.ui';

export const RUSTY_APPLICATION_UI_PROJECTION_MAX_BYTES = 65_536;
export const RUSTY_APPLICATION_UI_PROJECTION_MAX_WIRE_BYTES = 262_144;
export const RUSTY_APPLICATION_UI_PROJECTION_MAX_NODES = 2_048;
export const RUSTY_APPLICATION_UI_PROJECTION_MAX_DEPTH = 16;
export const RUSTY_APPLICATION_UI_PROJECTION_MAX_STRING_BYTES = 8_192;
export const RUSTY_APPLICATION_UI_PROJECTION_MAX_ARRAY_LENGTH = 512;
export const RUSTY_APPLICATION_UI_PROJECTION_MAX_OBJECT_KEYS = 256;
export const RUSTY_APPLICATION_UI_PROJECTION_MAX_SUBSCRIBERS = 64;
export const RUSTY_APPLICATION_UI_PROJECTION_U64_MAXIMUM =
  18_446_744_073_709_551_615n;

export type RustyApplicationUiProjectionJson =
  | null
  | boolean
  | number
  | string
  | readonly RustyApplicationUiProjectionJson[]
  | { readonly [key: string]: RustyApplicationUiProjectionJson };

/**
 * A strict worker-to-DOM projection envelope. The value is detached and
 * deeply frozen before it crosses into a mounted product UI.
 */
export interface RustyApplicationUiProjectionEnvelope {
  readonly artifact: typeof RUSTY_APPLICATION_UI_PROJECTION_ARTIFACT;
  readonly runtime: RustyApplicationRuntimeIdentity;
  readonly sequence: string;
  readonly stream: string;
  readonly contract: string;
  readonly value: RustyApplicationUiProjectionJson;
}

export interface RustyApplicationUiProjectionReadout {
  readonly artifact: typeof RUSTY_APPLICATION_UI_PROJECTION_ARTIFACT;
  readonly expectedStream: string;
  readonly expectedContract: string;
  readonly runtime: RustyApplicationRuntimeIdentity | null;
  readonly sequence: string | null;
  readonly hasCurrent: boolean;
  readonly acceptedCount: number;
  readonly rejectedCount: number;
  readonly subscriberCount: number;
  readonly state: 'ready' | 'disposed';
}

export interface RustyApplicationUiProjectionView {
  /** Returns the current immutable envelope, or null before the first value. */
  readonly current: () => RustyApplicationUiProjectionEnvelope | null;
  /** Subscribe to the current value. Rebinding publishes null before later values. */
  readonly subscribe: (
    listener: (value: RustyApplicationUiProjectionEnvelope | null) => void,
  ) => () => void;
}

export interface RustyApplicationUiProjectionPort extends RustyApplicationUiProjectionView {
  /** Rebind the projection epoch and clear the current snapshot. */
  readonly bindRuntime: (runtime: RustyApplicationRuntimeIdentity) => boolean;
  /** Admit one Rust worker envelope into the current bound epoch. */
  readonly ingest: (envelope: unknown) => boolean;
  /** Alias used by adapters that model worker messages as received values. */
  readonly receive: (envelope: unknown) => boolean;
  readonly readout: () => RustyApplicationUiProjectionReadout;
  readonly dispose: () => void;
}

export interface RustyApplicationUiProjectionOptions {
  readonly expectedStream?: string;
  /** Product/source-linked contract identity; the host never invents one. */
  readonly expectedContract: string;
  readonly binding?: RustyApplicationRuntimeIdentity;
  readonly maximumBytes?: number;
  readonly maximumWireBytes?: number;
  readonly maximumNodes?: number;
  readonly maximumDepth?: number;
  readonly maximumStringBytes?: number;
  readonly maximumArrayLength?: number;
  readonly maximumObjectKeys?: number;
  readonly maximumSubscribers?: number;
}

export type RustyApplicationUiProjectionErrorCode =
  | 'disposed'
  | 'invalid_envelope'
  | 'invalid_runtime'
  | 'invalid_sequence'
  | 'invalid_stream'
  | 'invalid_contract'
  | 'artifact_mismatch'
  | 'stream_mismatch'
  | 'contract_mismatch'
  | 'runtime_unbound'
  | 'runtime_mismatch'
  | 'sequence_not_increasing'
  | 'value_invalid'
  | 'value_limit_exceeded'
  | 'subscriber_limit_exceeded';

export class RustyApplicationUiProjectionError extends Error {
  readonly code: RustyApplicationUiProjectionErrorCode;

  constructor(code: RustyApplicationUiProjectionErrorCode, message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = 'RustyApplicationUiProjectionError';
    this.code = code;
  }
}

interface ProjectionLimits {
  readonly maximumBytes: number;
  readonly maximumWireBytes: number;
  readonly maximumNodes: number;
  readonly maximumDepth: number;
  readonly maximumStringBytes: number;
  readonly maximumArrayLength: number;
  readonly maximumObjectKeys: number;
  readonly maximumSubscribers: number;
}

const EMPTY_SUBSCRIBERS: ReadonlySet<ProjectionListener> = new Set();
type ProjectionListener = (
  value: RustyApplicationUiProjectionEnvelope | null,
) => void;

/**
 * Creates the host-owned projection channel. This is intentionally a small
 * ingress/store, not a query bus or product-state bridge: adapters bind an
 * epoch and deliver envelopes, while mounted UI can only read and subscribe.
 */
export function createRustyApplicationUiProjection(
  options: RustyApplicationUiProjectionOptions,
): RustyApplicationUiProjectionPort {
  const expectedStream = validateProductIdentity(
    options.expectedStream ?? RUSTY_APPLICATION_UI_PROJECTION_DEFAULT_STREAM,
    'expected UI projection stream',
  );
  const expectedContract = validateProductIdentity(
    options.expectedContract,
    'expected UI projection contract',
  );
  const limits = normalizeLimits(options);
  let runtime = options.binding === undefined
    ? null
    : validateRuntimeIdentity(options.binding, 'projection binding');
  let current: RustyApplicationUiProjectionEnvelope | null = null;
  let lastSequence: bigint | null = null;
  let acceptedCount = 0;
  let rejectedCount = 0;
  let disposed = false;
  let subscribers: ReadonlySet<ProjectionListener> = EMPTY_SUBSCRIBERS;

  const notify = (value: RustyApplicationUiProjectionEnvelope | null): void => {
    for (const listener of subscribers) {
      try {
        listener(value);
      } catch {
        // A product view callback cannot compromise the host's projection lane.
      }
    }
  };
  const requireActive = (): void => {
    if (disposed) {
      throw new RustyApplicationUiProjectionError(
        'disposed',
        'Rusty Application UI projection is disposed',
      );
    }
  };
  const bindRuntime = (nextRuntime: RustyApplicationRuntimeIdentity): boolean => {
    requireActive();
    const normalized = validateRuntimeIdentity(nextRuntime, 'projection binding');
    if (runtime !== null && sameRuntime(runtime, normalized)) return false;
    if (runtime !== null && runtime.instanceId === normalized.instanceId) {
      const priorGeneration = BigInt(runtime.generation);
      const nextGeneration = BigInt(normalized.generation);
      const priorControlRevision = BigInt(runtime.controlRevision);
      const nextControlRevision = BigInt(normalized.controlRevision);
      if (nextGeneration < priorGeneration) {
        throw new RustyApplicationUiProjectionError(
          'runtime_mismatch',
          'UI projection runtime generation cannot move backward within one instance',
        );
      }
      if (nextGeneration > priorGeneration && nextControlRevision <= priorControlRevision) {
        throw new RustyApplicationUiProjectionError(
          'runtime_mismatch',
          'UI projection control revision must advance with generation',
        );
      }
      if (nextGeneration === priorGeneration && nextControlRevision < priorControlRevision) {
        throw new RustyApplicationUiProjectionError(
          'runtime_mismatch',
          'UI projection control revision cannot move backward within one generation',
        );
      }
    }
    runtime = normalized;
    current = null;
    lastSequence = null;
    // Retain subscribers across an epoch transition so a mounted UI remains
    // live, but make the cleared snapshot observable before any new value.
    notify(null);
    return true;
  };
  const ingest = (rawEnvelope: unknown): boolean => {
    requireActive();
    try {
      const envelope = validateEnvelope(rawEnvelope, expectedStream, expectedContract, limits);
      if (runtime === null) {
        throw new RustyApplicationUiProjectionError(
          'runtime_unbound',
          'UI projection cannot be admitted before a runtime binding',
        );
      }
      if (!sameRuntime(runtime, envelope.runtime)) {
        throw new RustyApplicationUiProjectionError(
          'runtime_mismatch',
          'UI projection envelope runtime does not match the bound runtime',
        );
      }
      const sequence = BigInt(envelope.sequence);
      if (lastSequence !== null && sequence <= lastSequence) {
        throw new RustyApplicationUiProjectionError(
          'sequence_not_increasing',
          'UI projection sequence must strictly increase within one runtime epoch',
        );
      }
      lastSequence = sequence;
      current = envelope;
      acceptedCount += 1;
      notify(envelope);
      return true;
    } catch (cause) {
      rejectedCount += 1;
      throw cause instanceof RustyApplicationUiProjectionError
        ? cause
        : new RustyApplicationUiProjectionError(
          'invalid_envelope',
          cause instanceof Error ? cause.message : String(cause),
          { cause },
        );
    }
  };
  const currentValue = (): RustyApplicationUiProjectionEnvelope | null => current;
  const subscribe = (listener: ProjectionListener): (() => void) => {
    requireActive();
    if (typeof listener !== 'function') {
      throw new TypeError('UI projection subscriber must be a function');
    }
    if (subscribers.size >= limits.maximumSubscribers) {
      throw new RustyApplicationUiProjectionError(
        'subscriber_limit_exceeded',
        `UI projection subscriber count cannot exceed ${String(limits.maximumSubscribers)}`,
      );
    }
    const next = new Set(subscribers);
    next.add(listener);
    subscribers = next;
    try {
      listener(current);
    } catch {
      // Initial delivery follows the same isolation rule as later delivery.
    }
    let active = true;
    return () => {
      if (!active) return;
      active = false;
      const updated = new Set(subscribers);
      updated.delete(listener);
      subscribers = updated.size === 0 ? EMPTY_SUBSCRIBERS : updated;
    };
  };
  const readout = (): RustyApplicationUiProjectionReadout => Object.freeze({
    artifact: RUSTY_APPLICATION_UI_PROJECTION_ARTIFACT,
    expectedStream,
    expectedContract,
    runtime,
    sequence: lastSequence?.toString(10) ?? null,
    hasCurrent: current !== null,
    acceptedCount,
    rejectedCount,
    subscriberCount: subscribers.size,
    state: disposed ? 'disposed' : 'ready',
  });
  const dispose = (): void => {
    if (disposed) return;
    disposed = true;
    current = null;
    lastSequence = null;
    const priorSubscribers = subscribers;
    subscribers = EMPTY_SUBSCRIBERS;
    for (const listener of priorSubscribers) {
      try {
        listener(null);
      } catch {
        // Disposal remains idempotent even when a UI owner is already gone.
      }
    }
  };

  return Object.freeze({
    current: currentValue,
    subscribe,
    bindRuntime,
    ingest,
    receive: ingest,
    readout,
    dispose,
  });
}

function normalizeLimits(options: RustyApplicationUiProjectionOptions): ProjectionLimits {
  return Object.freeze({
    maximumBytes: boundedInteger(
      options.maximumBytes ?? RUSTY_APPLICATION_UI_PROJECTION_MAX_BYTES,
      256,
      RUSTY_APPLICATION_UI_PROJECTION_MAX_BYTES,
      'maximumBytes',
    ),
    maximumWireBytes: boundedInteger(
      options.maximumWireBytes ?? RUSTY_APPLICATION_UI_PROJECTION_MAX_WIRE_BYTES,
      256,
      RUSTY_APPLICATION_UI_PROJECTION_MAX_WIRE_BYTES,
      'maximumWireBytes',
    ),
    maximumNodes: boundedInteger(
      options.maximumNodes ?? RUSTY_APPLICATION_UI_PROJECTION_MAX_NODES,
      1,
      RUSTY_APPLICATION_UI_PROJECTION_MAX_NODES,
      'maximumNodes',
    ),
    maximumDepth: boundedInteger(
      options.maximumDepth ?? RUSTY_APPLICATION_UI_PROJECTION_MAX_DEPTH,
      1,
      RUSTY_APPLICATION_UI_PROJECTION_MAX_DEPTH,
      'maximumDepth',
    ),
    maximumStringBytes: boundedInteger(
      options.maximumStringBytes ?? RUSTY_APPLICATION_UI_PROJECTION_MAX_STRING_BYTES,
      1,
      RUSTY_APPLICATION_UI_PROJECTION_MAX_STRING_BYTES,
      'maximumStringBytes',
    ),
    maximumArrayLength: boundedInteger(
      options.maximumArrayLength ?? RUSTY_APPLICATION_UI_PROJECTION_MAX_ARRAY_LENGTH,
      1,
      RUSTY_APPLICATION_UI_PROJECTION_MAX_ARRAY_LENGTH,
      'maximumArrayLength',
    ),
    maximumObjectKeys: boundedInteger(
      options.maximumObjectKeys ?? RUSTY_APPLICATION_UI_PROJECTION_MAX_OBJECT_KEYS,
      1,
      RUSTY_APPLICATION_UI_PROJECTION_MAX_OBJECT_KEYS,
      'maximumObjectKeys',
    ),
    maximumSubscribers: boundedInteger(
      options.maximumSubscribers ?? RUSTY_APPLICATION_UI_PROJECTION_MAX_SUBSCRIBERS,
      1,
      RUSTY_APPLICATION_UI_PROJECTION_MAX_SUBSCRIBERS,
      'maximumSubscribers',
    ),
  });
}

function boundedInteger(value: number, minimum: number, maximum: number, name: string): number {
  if (!Number.isSafeInteger(value) || value < minimum || value > maximum) {
    throw new RangeError(`${name} must be a safe integer within [${String(minimum)}, ${String(maximum)}]`);
  }
  return value;
}

function validateEnvelope(
  raw: unknown,
  expectedStream: string,
  expectedContract: string,
  limits: ProjectionLimits,
): RustyApplicationUiProjectionEnvelope {
  if (!isPlainRecord(raw)) {
    throw new RustyApplicationUiProjectionError(
      'invalid_envelope',
      'UI projection envelope must be a plain object',
    );
  }
  assertExactKeys(raw, ['artifact', 'contract', 'runtime', 'sequence', 'stream', 'value'], 'UI projection envelope');
  const artifact = readDataProperty(raw, 'artifact', 'UI projection envelope');
  if (artifact !== RUSTY_APPLICATION_UI_PROJECTION_ARTIFACT) {
    throw new RustyApplicationUiProjectionError(
      'artifact_mismatch',
      `UI projection artifact must be ${RUSTY_APPLICATION_UI_PROJECTION_ARTIFACT}`,
    );
  }
  const stream = validateProductIdentity(
    readDataProperty(raw, 'stream', 'UI projection envelope'),
    'UI projection stream',
  );
  if (stream !== expectedStream) {
    throw new RustyApplicationUiProjectionError(
      'stream_mismatch',
      `UI projection stream ${stream} does not match expected ${expectedStream}`,
    );
  }
  const contract = validateProductIdentity(
    readDataProperty(raw, 'contract', 'UI projection envelope'),
    'UI projection contract',
  );
  if (contract !== expectedContract) {
    throw new RustyApplicationUiProjectionError(
      'contract_mismatch',
      `UI projection contract ${contract} does not match expected ${expectedContract}`,
    );
  }
  const runtime = validateRuntimeIdentity(
    readDataProperty(raw, 'runtime', 'UI projection envelope'),
    'UI projection runtime',
  );
  const sequence = validateCanonicalU64(
    readDataProperty(raw, 'sequence', 'UI projection envelope'),
    'UI projection sequence',
  );
  const value = detachJson(
    readDataProperty(raw, 'value', 'UI projection envelope'),
    limits,
  );
  const envelope = {
    artifact: RUSTY_APPLICATION_UI_PROJECTION_ARTIFACT,
    runtime,
    sequence,
    stream,
    contract,
    value,
  } satisfies RustyApplicationUiProjectionEnvelope;
  const encoded = JSON.stringify(envelope);
  if (new TextEncoder().encode(encoded).byteLength > limits.maximumWireBytes) {
    throw new RustyApplicationUiProjectionError(
      'value_limit_exceeded',
      `UI projection envelope exceeds ${String(limits.maximumWireBytes)} bytes`,
    );
  }
  return Object.freeze(envelope);
}

function detachJson(value: unknown, limits: ProjectionLimits): RustyApplicationUiProjectionJson {
  let nodes = 0;
  const ancestors = new WeakSet<object>();
  const textEncoder = new TextEncoder();
  const visit = (candidate: unknown, depth: number, path: string): RustyApplicationUiProjectionJson => {
    nodes += 1;
    if (nodes > limits.maximumNodes) {
      throw new RustyApplicationUiProjectionError(
        'value_limit_exceeded',
        `UI projection value exceeds ${String(limits.maximumNodes)} JSON nodes`,
      );
    }
    if (depth > limits.maximumDepth) {
      throw new RustyApplicationUiProjectionError(
        'value_limit_exceeded',
        `UI projection value exceeds depth ${String(limits.maximumDepth)} at ${path}`,
      );
    }
    if (candidate === null || typeof candidate === 'boolean') return candidate;
    if (typeof candidate === 'string') {
      if (textEncoder.encode(candidate).byteLength > limits.maximumStringBytes) {
        throw new RustyApplicationUiProjectionError(
          'value_limit_exceeded',
          `UI projection string exceeds ${String(limits.maximumStringBytes)} bytes at ${path}`,
        );
      }
      return candidate;
    }
    if (typeof candidate === 'number') {
      if (!Number.isFinite(candidate)) {
        throw new RustyApplicationUiProjectionError(
          'value_invalid',
          `UI projection number must be finite at ${path}`,
        );
      }
      if (Number.isInteger(candidate) && !Number.isSafeInteger(candidate)) {
        throw new RustyApplicationUiProjectionError(
          'value_invalid',
          `UI projection integer must be a safe integer at ${path}`,
        );
      }
      return candidate;
    }
    if (!isPlainRecord(candidate) && !Array.isArray(candidate)) {
      throw new RustyApplicationUiProjectionError(
        'value_invalid',
        `UI projection value must contain only plain JSON at ${path}`,
      );
    }
    if (ancestors.has(candidate)) {
      throw new RustyApplicationUiProjectionError(
        'value_invalid',
        `UI projection value cannot contain a cycle at ${path}`,
      );
    }
    ancestors.add(candidate);
    try {
      if (Array.isArray(candidate)) {
        let prototype: object | null;
        try {
          prototype = Object.getPrototypeOf(candidate);
        } catch (cause) {
          throw new RustyApplicationUiProjectionError(
            'value_invalid',
            `UI projection array prototype could not be inspected at ${path}`,
            { cause },
          );
        }
        if (prototype !== Array.prototype) {
          throw new RustyApplicationUiProjectionError(
            'value_invalid',
            `UI projection array must use the plain Array prototype at ${path}`,
          );
        }
        const lengthDescriptor = Object.getOwnPropertyDescriptor(candidate, 'length');
        if (lengthDescriptor === undefined || !('value' in lengthDescriptor)
          || lengthDescriptor.enumerable !== false || typeof lengthDescriptor.value !== 'number') {
          throw new RustyApplicationUiProjectionError(
            'value_invalid',
            `UI projection array length must be an intrinsic data property at ${path}`,
          );
        }
        const length = lengthDescriptor.value;
        if (length > limits.maximumArrayLength) {
          throw new RustyApplicationUiProjectionError(
            'value_limit_exceeded',
            `UI projection array exceeds ${String(limits.maximumArrayLength)} entries at ${path}`,
          );
        }
        const keys = Reflect.ownKeys(candidate);
        if (keys.length !== length + 1 || !keys.includes('length')) {
          throw new RustyApplicationUiProjectionError(
            'value_invalid',
            `UI projection array must contain only dense indexed entries at ${path}`,
          );
        }
        const array: RustyApplicationUiProjectionJson[] = [];
        for (let index = 0; index < length; index += 1) {
          const key = String(index);
          const descriptor = Object.getOwnPropertyDescriptor(candidate, key);
          if (descriptor === undefined || !('value' in descriptor) || descriptor.enumerable !== true) {
            throw new RustyApplicationUiProjectionError(
              'value_invalid',
              `UI projection array must contain dense data entries at ${path}[${String(index)}]`,
            );
          }
          array.push(visit(descriptor.value, depth + 1, `${path}[${String(index)}]`));
        }
        return Object.freeze(array);
      }
      const keys = Reflect.ownKeys(candidate);
      if (keys.some((key) => typeof key !== 'string')) {
        throw new RustyApplicationUiProjectionError(
          'value_invalid',
          `UI projection object cannot contain symbol keys at ${path}`,
        );
      }
      if (keys.length > limits.maximumObjectKeys) {
        throw new RustyApplicationUiProjectionError(
          'value_limit_exceeded',
          `UI projection object exceeds ${String(limits.maximumObjectKeys)} keys at ${path}`,
        );
      }
      const object: Record<string, RustyApplicationUiProjectionJson> = {};
      for (const key of keys as string[]) {
        const descriptor = Object.getOwnPropertyDescriptor(candidate, key);
        if (descriptor === undefined || !('value' in descriptor) || descriptor.enumerable !== true) {
          throw new RustyApplicationUiProjectionError(
            'value_invalid',
            `UI projection object must contain enumerable data entries at ${path}.${key}`,
          );
        }
        Object.defineProperty(object, key, {
          configurable: true,
          enumerable: true,
          value: visit(descriptor.value, depth + 1, `${path}.${key}`),
          writable: true,
        });
      }
      return Object.freeze(object);
    } finally {
      ancestors.delete(candidate);
    }
  };
  const detached = visit(value, 0, 'value');
  let encoded: string;
  try {
    encoded = JSON.stringify(detached);
  } catch (cause) {
    throw new RustyApplicationUiProjectionError(
      'value_invalid',
      'UI projection value could not be encoded as JSON',
      { cause },
    );
  }
  if (textEncoder.encode(encoded).byteLength > limits.maximumBytes) {
    throw new RustyApplicationUiProjectionError(
      'value_limit_exceeded',
      `UI projection value exceeds ${String(limits.maximumBytes)} bytes`,
    );
  }
  return detached;
}

function validateRuntimeIdentity(
  value: unknown,
  name: string,
): RustyApplicationRuntimeIdentity {
  if (!isPlainRecord(value)) {
    throw new RustyApplicationUiProjectionError('invalid_runtime', `${name} must be a plain runtime identity`);
  }
  assertExactKeys(value, ['controlRevision', 'generation', 'instanceId'], name);
  return Object.freeze({
    instanceId: validateCanonicalU64(readDataProperty(value, 'instanceId', name), `${name}.instanceId`),
    generation: validateCanonicalU64(readDataProperty(value, 'generation', name), `${name}.generation`),
    controlRevision: validateCanonicalU64(readDataProperty(value, 'controlRevision', name), `${name}.controlRevision`),
  });
}

function validateCanonicalU64(value: unknown, name: string): string {
  if (typeof value !== 'string' || !/^(?:0|[1-9][0-9]*)$/u.test(value)) {
    throw new RustyApplicationUiProjectionError(
      'invalid_sequence',
      `${name} must be canonical unsigned decimal text`,
    );
  }
  let parsed: bigint;
  try {
    parsed = BigInt(value);
  } catch (cause) {
    throw new RustyApplicationUiProjectionError('invalid_sequence', `${name} must be canonical unsigned decimal text`, { cause });
  }
  if (parsed > RUSTY_APPLICATION_UI_PROJECTION_U64_MAXIMUM) {
    throw new RustyApplicationUiProjectionError('invalid_sequence', `${name} exceeds u64`);
  }
  return value;
}

function validateProductIdentity(value: unknown, name: string): string {
  if (typeof value !== 'string' || new TextEncoder().encode(value).byteLength > 128
    || !/^[a-z0-9](?:[a-z0-9]|[._-](?=[a-z0-9]))*$/u.test(value)) {
    throw new RustyApplicationUiProjectionError(
      name.includes('stream') ? 'invalid_stream' : 'invalid_contract',
      `${name} must be a 1..128 byte lowercase product identity`,
    );
  }
  return value;
}

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  try {
    const prototype = Object.getPrototypeOf(value);
    return prototype === Object.prototype || prototype === null;
  } catch {
    return false;
  }
}

function assertExactKeys(
  value: Record<string, unknown>,
  expected: readonly string[],
  name: string,
): void {
  const keys = Reflect.ownKeys(value);
  if (keys.some((key) => typeof key !== 'string')) {
    throw new RustyApplicationUiProjectionError(
      'invalid_envelope',
      `${name} cannot contain symbol keys`,
    );
  }
  for (const key of keys as string[]) readDataProperty(value, key, name);
  const actual = (keys as string[]).sort();
  const wanted = [...expected].sort();
  if (actual.length !== wanted.length || actual.some((key, index) => key !== wanted[index])) {
    throw new RustyApplicationUiProjectionError(
      'invalid_envelope',
      `${name} must contain exactly ${wanted.join(', ')}`,
    );
  }
}

function readDataProperty(
  value: Record<string, unknown>,
  key: string,
  name: string,
): unknown {
  const descriptor = Object.getOwnPropertyDescriptor(value, key);
  if (descriptor === undefined || !('value' in descriptor) || descriptor.enumerable !== true) {
    throw new RustyApplicationUiProjectionError(
      'invalid_envelope',
      `${name}.${key} must be an enumerable data property`,
    );
  }
  return descriptor.value;
}

function sameRuntime(
  left: RustyApplicationRuntimeIdentity,
  right: RustyApplicationRuntimeIdentity,
): boolean {
  return left.instanceId === right.instanceId
    && left.generation === right.generation
    && left.controlRevision === right.controlRevision;
}
