import type { JsonValue, RuntimeUiProjectionEnvelope } from './generated/contracts.js';
import type { RustyApplicationRuntimeIdentity } from './input-ingress.js';

/** The one Product UI projection artifact admitted by the application host. */
export const RUSTY_APPLICATION_UI_PROJECTION_ARTIFACT =
  'rusty.product.ui-projection' as const;

export const RUSTY_APPLICATION_UI_PROJECTION_DEFAULT_STREAM = 'product.ui';

export const RUSTY_APPLICATION_UI_PROJECTION_MAX_SUBSCRIBERS = 64;

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
  readonly current: () => RuntimeUiProjectionEnvelope | null;
  /** Subscribe to the current value. Rebinding publishes null before later values. */
  readonly subscribe: (
    listener: (value: RuntimeUiProjectionEnvelope | null) => void,
  ) => () => void;
}

export interface RustyApplicationUiProjectionPort extends RustyApplicationUiProjectionView {
  /** Rebind the projection epoch and clear the current snapshot. */
  readonly bindRuntime: (runtime: RustyApplicationRuntimeIdentity) => boolean;
  /** Admit one Rust envelope into the current bound epoch. */
  readonly ingest: (envelope: RuntimeUiProjectionEnvelope) => boolean;
  readonly readout: () => RustyApplicationUiProjectionReadout;
  readonly dispose: () => void;
}

export interface RustyApplicationUiProjectionOptions {
  readonly expectedStream?: string;
  /** Product/source-linked contract identity; the host never invents one. */
  readonly expectedContract: string;
  readonly binding?: RustyApplicationRuntimeIdentity;
  readonly maximumSubscribers?: number;
}

export type RustyApplicationUiProjectionErrorCode =
  | 'disposed'
  | 'stream_mismatch'
  | 'contract_mismatch'
  | 'runtime_unbound'
  | 'runtime_mismatch'
  | 'sequence_not_increasing'
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
  readonly maximumSubscribers: number;
}

const EMPTY_SUBSCRIBERS: ReadonlySet<ProjectionListener> = new Set();
type ProjectionListener = (
  value: RuntimeUiProjectionEnvelope | null,
) => void;

/**
 * Creates the host-owned projection channel. This is intentionally a small
 * ingress/store, not a query bus or product-state bridge: adapters bind an
 * epoch and deliver envelopes, while mounted UI can only read and subscribe.
 */
export function createRustyApplicationUiProjection(
  options: RustyApplicationUiProjectionOptions,
): RustyApplicationUiProjectionPort {
  const expectedStream = options.expectedStream ?? RUSTY_APPLICATION_UI_PROJECTION_DEFAULT_STREAM;
  const expectedContract = options.expectedContract;
  const limits = normalizeLimits(options);
  let runtime = options.binding ?? null;
  let current: RuntimeUiProjectionEnvelope | null = null;
  let lastSequence: bigint | null = null;
  let acceptedCount = 0;
  let rejectedCount = 0;
  let disposed = false;
  let subscribers: ReadonlySet<ProjectionListener> = EMPTY_SUBSCRIBERS;

  const notify = (value: RuntimeUiProjectionEnvelope | null): void => {
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
  const bindRuntime = (normalized: RustyApplicationRuntimeIdentity): boolean => {
    requireActive();
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
  const ingest = (envelope: RuntimeUiProjectionEnvelope): boolean => {
    requireActive();
    const reject = (code: RustyApplicationUiProjectionErrorCode, message: string): never => {
      rejectedCount += 1;
      throw new RustyApplicationUiProjectionError(code, message);
    };
    if (envelope.stream !== expectedStream) {
      reject('stream_mismatch', `UI projection stream ${envelope.stream} does not match expected ${expectedStream}`);
    }
    if (envelope.contract !== expectedContract) {
      reject('contract_mismatch', `UI projection contract ${envelope.contract} does not match expected ${expectedContract}`);
    }
    if (runtime === null) {
      return reject('runtime_unbound', 'UI projection cannot be admitted before a runtime binding');
    }
    if (!sameRuntime(runtime, envelope.runtime)) {
      reject('runtime_mismatch', 'UI projection envelope runtime does not match the bound runtime');
    }
    const sequence = BigInt(envelope.sequence);
    if (lastSequence !== null && sequence <= lastSequence) {
      reject('sequence_not_increasing', 'UI projection sequence must strictly increase within one runtime epoch');
    }
    lastSequence = sequence;
    // Every subscriber shares this value, so none may change it.
    current = Object.freeze({ ...envelope, value: deepFreeze(envelope.value) });
    acceptedCount += 1;
    notify(current);
    return true;
  };
  const currentValue = (): RuntimeUiProjectionEnvelope | null => current;
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
    readout,
    dispose,
  });
}

function normalizeLimits(options: RustyApplicationUiProjectionOptions): ProjectionLimits {
  return Object.freeze({
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

function deepFreeze(value: JsonValue): JsonValue {
  if (value !== null && typeof value === 'object' && !Object.isFrozen(value)) {
    for (const entry of Object.values(value)) deepFreeze(entry ?? null);
    Object.freeze(value);
  }
  return value;
}

function sameRuntime(
  left: RustyApplicationRuntimeIdentity,
  right: RustyApplicationRuntimeIdentity,
): boolean {
  return left.instanceId === right.instanceId
    && left.generation === right.generation
    && left.controlRevision === right.controlRevision;
}
