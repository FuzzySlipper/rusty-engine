import type { RuntimeInputWireEvent } from './generated/contracts.js';
import type { ProductBrowserRealtimeAdvanceOwner } from './product-browser-host.js';

/**
 * The small dependency surface used by the Product Browser Host's one
 * page-cadence callback (the frame view's animation frame). Keeping the owner
 * decision here lets the package test the actual input/advance behavior
 * without manufacturing a second DOM host.
 */
export interface ProductBrowserCadenceDependencies {
  readonly realtimeAdvanceOwner: ProductBrowserRealtimeAdvanceOwner;
  readonly isReady: () => boolean;
  readonly enqueueOperation: <T>(operation: () => Promise<T>) => Promise<T>;
  readonly sampleInput: () => readonly RuntimeInputWireEvent[];
  readonly sendInput: (
    batch: readonly RuntimeInputWireEvent[],
  ) => Promise<void>;
  readonly advanceRealtime: (observedTimeNs: string) => Promise<void>;
  readonly onFailure: (cause: unknown) => void;
}

export interface ProductBrowserCadence {
  readonly enqueue: (timeMs: number) => void;
  /**
   * Wakes the same serialized admission lane when input arrives between page frames.
   * Browser-owned realtime advances once; under the `rust-host` owner the wake only
   * delivers the input, because that scheduler admits the steps.
   */
  readonly pulseInput: (timeMs: number) => void;
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
      // A wake can become redundant when an earlier page cadence drains
      // ingress. Do not advance solely for that empty wake.
      if (inputWake && batch.length === 0) return;
      if (dependencies.realtimeAdvanceOwner === 'browser') {
        await dependencies.advanceRealtime(toNanoseconds(timeMs));
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

  const enqueue = (timeMs: number): void => {
    if (disposed || !dependencies.isReady()) return;
    // requestAnimationFrame timestamps describe the start of the frame, while
    // input wakeups sample performance.now() when the event is handled. A RAF
    // callback can therefore execute after an input wakeup while carrying a
    // slightly older timestamp. Normalize both sources before they enter the
    // one ordered runtime lane; Rust can retain strict regression rejection.
    const monotonicTimeMs = Math.max(maximumObservedTimeMs, orderingTime(timeMs));
    maximumObservedTimeMs = monotonicTimeMs;
    if (cadenceInFlight) {
      // Keep only the newest cadence time while the Rust operation is
      // outstanding. This is intentionally separate from the input wake,
      // whose earlier timestamp determines when ingress next gets sampled.
      pendingCadenceTimeMs = monotonicTimeMs;
      return;
    }
    startOperation(monotonicTimeMs);
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
    startOperation(monotonicTimeMs, true);
  };

  const finish = (): void => {
    cadenceInFlight = false;
    const inputWakeTimeMs = pendingInputWakeTimeMs;
    if (inputWakeTimeMs !== null && (pendingCadenceTimeMs === null
      || orderingTime(inputWakeTimeMs) <= orderingTime(pendingCadenceTimeMs))) {
      pendingInputWakeTimeMs = null;
      if (!disposed && dependencies.isReady()) {
        startOperation(inputWakeTimeMs, true);
      }
      return;
    }
    const nextTimeMs = pendingCadenceTimeMs;
    pendingCadenceTimeMs = null;
    if (nextTimeMs !== null && !disposed && dependencies.isReady()) {
      // A page cadence that predates a queued input wake may still advance
      // its clock, but it must not drain input that became available later.
      // The following wake samples the one ingress queue at its own time.
      const cadencePrecedesInputWake = pendingInputWakeTimeMs !== null
        && orderingTime(nextTimeMs) < orderingTime(pendingInputWakeTimeMs);
      startOperation(nextTimeMs, false, !cadencePrecedesInputWake);
    }
  };

  return Object.freeze({
    enqueue,
    pulseInput,
    settle: async (): Promise<void> => {
      while (cadenceInFlight || pendingCadenceTimeMs !== null || pendingInputWakeTimeMs !== null) {
        await lastOperation;
      }
    },
    dispose: (): void => {
      disposed = true;
      pendingCadenceTimeMs = null;
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
