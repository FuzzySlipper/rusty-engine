import type { RustyApplicationRuntimeInputEnvelope } from '@rusty-engine/application-host';
import type {
  ProductBrowserRealtimeAdvanceOwner,
  ProductBrowserRuntimeMode,
} from './product-browser-host.js';

/**
 * The small dependency surface used by the Product Browser Host's one
 * renderer-cadence callback. Keeping the owner decision here lets the
 * package test the actual input/advance behavior without manufacturing a
 * second DOM or renderer host.
 */
export interface ProductBrowserCadenceDependencies {
  readonly lifecycleMode: ProductBrowserRuntimeMode;
  readonly realtimeAdvanceOwner: ProductBrowserRealtimeAdvanceOwner;
  readonly isReady: () => boolean;
  readonly enqueueOperation: <T>(operation: () => Promise<T>) => Promise<T>;
  readonly sampleInput: () => readonly RustyApplicationRuntimeInputEnvelope[];
  readonly sendInput: (
    batch: readonly RustyApplicationRuntimeInputEnvelope[],
  ) => Promise<void>;
  readonly advanceRealtime: (observedTimeNs: string) => Promise<void>;
  readonly admitDemandStep: () => Promise<void>;
  readonly onFailure: (cause: unknown) => void;
}

export interface ProductBrowserCadence {
  readonly enqueue: (timeMs: number) => void;
  /**
   * Wakes the same serialized admission lane when input arrives between renderer frames.
   * Browser-owned realtime advances once, demand admits one step, and externally owned
   * modes only deliver the input because their scheduling authority remains external.
   */
  readonly pulseInput: (timeMs: number) => void;
  /** Drains input from one Rust-host runtime output without browser advancement. */
  readonly pulseRustHost: () => void;
  /** Waits for the current cadence operation and any coalesced follow-up. */
  readonly settle: () => Promise<void>;
  readonly dispose: () => void;
}

/**
 * Owns coalescing for the existing application-host cadence callback. It
 * never creates an animation frame source. In `rust-host` mode the callback
 * still drains and sends typed input, but realtime simulation admission is
 * exclusively external to the browser host.
 */
export function createProductBrowserCadence(
  dependencies: ProductBrowserCadenceDependencies,
): ProductBrowserCadence {
  let cadenceInFlight = false;
  let pendingCadenceTimeMs: number | null = null;
  let pendingDemandAdmission = false;
  // Input ingress is the sole envelope queue. While an operation owns the
  // serialized lane, retain only the first wake timestamp: it gets the next
  // admission opportunity without retaining, copying, or reordering input
  // envelopes outside ingress.
  let pendingInputWakeTimeMs: number | null = null;
  let disposed = false;
  let lastOperation: Promise<void> = Promise.resolve();
  let maximumObservedTimeMs = 0;

  const startOperation = (
    timeMs: number,
    demandAdmission = false,
    inputWake = false,
    sampleInput = true,
  ): void => {
    cadenceInFlight = true;
    const operation = dependencies.enqueueOperation(async () => {
      // enqueueOperation can defer this callback behind an already-admitted
      // runtime operation. Re-evaluate at execution time so a cadence that
      // predates a wake received while it waited cannot drain future input.
      const cadencePrecedesPendingInputWake = !inputWake
        && pendingInputWakeTimeMs !== null
        && orderingTime(timeMs) < orderingTime(pendingInputWakeTimeMs);
      const batch = sampleInput && !cadencePrecedesPendingInputWake
        ? dependencies.sampleInput()
        : [];
      if (batch.length > 0) await dependencies.sendInput(batch);
      // A wake can become redundant when an earlier renderer cadence drains
      // ingress. Preserve the old availability contract by not advancing or
      // admitting demand work solely for that empty wake.
      if (inputWake && batch.length === 0) return;
      if (dependencies.lifecycleMode === 'realtime'
        && dependencies.realtimeAdvanceOwner === 'browser') {
        await dependencies.advanceRealtime(toNanoseconds(timeMs));
      } else if (dependencies.lifecycleMode === 'demand' && demandAdmission) {
        await dependencies.admitDemandStep();
      }
    });
    lastOperation = operation.then(
      () => finish(),
      (cause: unknown) => {
        dependencies.onFailure(cause);
        finish();
      },
    );
  };

  const enqueue = (timeMs: number, demandAdmission = false): void => {
    if (disposed || !dependencies.isReady()) return;
    // requestAnimationFrame timestamps describe the start of the frame, while
    // input wakeups sample performance.now() when the event is handled. A RAF
    // callback can therefore execute after an input wakeup while carrying a
    // slightly older timestamp. Normalize both sources before they enter the
    // one ordered runtime lane; Rust can retain strict regression rejection.
    const monotonicTimeMs = Math.max(maximumObservedTimeMs, orderingTime(timeMs));
    maximumObservedTimeMs = monotonicTimeMs;
    if (cadenceInFlight) {
      // Keep only the newest renderer time while the Rust operation is
      // outstanding. This is intentionally separate from the input wake,
      // whose earlier timestamp determines when ingress next gets sampled.
      pendingCadenceTimeMs = monotonicTimeMs;
      pendingDemandAdmission ||= demandAdmission;
      return;
    }
    startOperation(monotonicTimeMs, demandAdmission);
  };

  const pulseInput = (timeMs: number): void => {
    if (disposed || !dependencies.isReady()) return;
    const monotonicTimeMs = Math.max(maximumObservedTimeMs, orderingTime(timeMs));
    maximumObservedTimeMs = monotonicTimeMs;
    if (cadenceInFlight) {
      // Do not drain while busy. Application ingress retains the bounded,
      // ordered envelope batch and its overflow-clear recovery fact.
      if (pendingInputWakeTimeMs === null) pendingInputWakeTimeMs = monotonicTimeMs;
      return;
    }
    startOperation(monotonicTimeMs, dependencies.lifecycleMode === 'demand', true);
  };

  const finish = (): void => {
    cadenceInFlight = false;
    const inputWakeTimeMs = pendingInputWakeTimeMs;
    if (inputWakeTimeMs !== null && (pendingCadenceTimeMs === null
      || orderingTime(inputWakeTimeMs) <= orderingTime(pendingCadenceTimeMs))) {
      pendingInputWakeTimeMs = null;
      if (!disposed && dependencies.isReady()) {
        startOperation(inputWakeTimeMs, dependencies.lifecycleMode === 'demand', true);
      }
      return;
    }
    const nextTimeMs = pendingCadenceTimeMs;
    const demandAdmission = pendingDemandAdmission;
    pendingCadenceTimeMs = null;
    pendingDemandAdmission = false;
    if (nextTimeMs !== null && !disposed && dependencies.isReady()) {
      // A renderer cadence that predates a queued input wake may still advance
      // its clock, but it must not drain input that became available later.
      // The following wake samples the one ingress queue at its own time.
      const cadencePrecedesInputWake = pendingInputWakeTimeMs !== null
        && orderingTime(nextTimeMs) < orderingTime(pendingInputWakeTimeMs);
      startOperation(nextTimeMs, demandAdmission, false, !cadencePrecedesInputWake);
    }
  };

  return Object.freeze({
    enqueue: (timeMs: number): void => enqueue(timeMs),
    pulseInput,
    pulseRustHost: (): void => {
      if (dependencies.realtimeAdvanceOwner === 'rust-host') enqueue(0);
    },
    settle: async (): Promise<void> => {
      while (cadenceInFlight || pendingCadenceTimeMs !== null || pendingInputWakeTimeMs !== null) {
        await lastOperation;
      }
    },
    dispose: (): void => {
      disposed = true;
      pendingCadenceTimeMs = null;
      pendingDemandAdmission = false;
      pendingInputWakeTimeMs = null;
    },
  });
}

function toNanoseconds(timeMs: number): string {
  if (!Number.isFinite(timeMs) || timeMs < 0) return '0';
  const nanoseconds = BigInt(Math.round(timeMs * 1_000_000));
  return nanoseconds.toString(10);
}

function orderingTime(timeMs: number): number {
  return Number.isFinite(timeMs) && timeMs >= 0 ? timeMs : 0;
}
