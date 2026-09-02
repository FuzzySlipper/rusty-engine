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

const MAXIMUM_PENDING_INPUT_ENVELOPES = 1_024;

interface PendingInputPulse {
  readonly timeMs: number;
  readonly batch: readonly RustyApplicationRuntimeInputEnvelope[];
  priorCadenceTimeMs: number | null;
  priorDemandAdmission: boolean;
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
  const pendingInputPulses: PendingInputPulse[] = [];
  let pendingInputEnvelopeCount = 0;
  let disposed = false;
  let lastOperation: Promise<void> = Promise.resolve();

  const enqueue = (
    timeMs: number,
    demandAdmission = false,
    capturedInput: readonly RustyApplicationRuntimeInputEnvelope[] | null = null,
  ): void => {
    if (disposed || !dependencies.isReady()) return;
    if (cadenceInFlight) {
      if (capturedInput !== null && capturedInput.length > 0) {
        if (pendingInputEnvelopeCount + capturedInput.length > MAXIMUM_PENDING_INPUT_ENVELOPES) {
          disposed = true;
          pendingInputPulses.length = 0;
          pendingInputEnvelopeCount = 0;
          dependencies.onFailure(new Error(
            `pending input exceeds ${String(MAXIMUM_PENDING_INPUT_ENVELOPES)} envelopes`,
          ));
          return;
        }
        const preserveCadenceBeforeInput = pendingCadenceTimeMs !== null
          && orderingTime(pendingCadenceTimeMs) <= orderingTime(timeMs);
        pendingInputPulses.push({
          timeMs,
          batch: capturedInput,
          priorCadenceTimeMs: preserveCadenceBeforeInput ? pendingCadenceTimeMs : null,
          priorDemandAdmission: preserveCadenceBeforeInput && pendingDemandAdmission,
        });
        if (preserveCadenceBeforeInput) {
          pendingCadenceTimeMs = null;
          pendingDemandAdmission = false;
        }
        pendingInputEnvelopeCount += capturedInput.length;
      }
      // Keep only the newest observed host time while the Rust operation is
      // outstanding. Sparse input pulses are captured separately above so a
      // later edge cannot erase an earlier held state. Ordinary renderer
      // cadence remains coalesced and never creates one promise per RAF.
      if (capturedInput === null) {
        pendingCadenceTimeMs = timeMs;
        pendingDemandAdmission ||= demandAdmission;
      }
      return;
    }
    cadenceInFlight = true;
    const operation = dependencies.enqueueOperation(async () => {
      const batch = capturedInput ?? dependencies.sampleInput();
      if (batch.length > 0) await dependencies.sendInput(batch);
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

  const finish = (): void => {
    cadenceInFlight = false;
    const inputPulse = pendingInputPulses[0];
    if (inputPulse?.priorCadenceTimeMs !== null && inputPulse?.priorCadenceTimeMs !== undefined) {
      const priorCadenceTimeMs = inputPulse.priorCadenceTimeMs;
      const priorDemandAdmission = inputPulse.priorDemandAdmission;
      inputPulse.priorCadenceTimeMs = null;
      inputPulse.priorDemandAdmission = false;
      if (!disposed && dependencies.isReady()) {
        enqueue(priorCadenceTimeMs, priorDemandAdmission);
      }
      return;
    }
    if (inputPulse !== undefined && (pendingCadenceTimeMs === null
      || orderingTime(inputPulse.timeMs) <= orderingTime(pendingCadenceTimeMs))) {
      pendingInputPulses.shift();
      pendingInputEnvelopeCount -= inputPulse.batch.length;
      if (!disposed && dependencies.isReady()) {
        enqueue(
          inputPulse.timeMs,
          dependencies.lifecycleMode === 'demand',
          inputPulse.batch,
        );
      }
      return;
    }
    const nextTimeMs = pendingCadenceTimeMs;
    const demandAdmission = pendingDemandAdmission;
    pendingCadenceTimeMs = null;
    pendingDemandAdmission = false;
    if (nextTimeMs !== null && !disposed && dependencies.isReady()) {
      enqueue(nextTimeMs, demandAdmission);
    }
  };

  return Object.freeze({
    enqueue,
    pulseInput: (timeMs: number): void => {
      if (disposed || !dependencies.isReady()) return;
      const batch = dependencies.sampleInput();
      if (batch.length === 0) return;
      enqueue(timeMs, dependencies.lifecycleMode === 'demand', batch);
    },
    pulseRustHost: (): void => {
      if (dependencies.realtimeAdvanceOwner === 'rust-host') enqueue(0);
    },
    settle: async (): Promise<void> => {
      while (cadenceInFlight || pendingCadenceTimeMs !== null || pendingInputPulses.length > 0) {
        await lastOperation;
      }
    },
    dispose: (): void => {
      disposed = true;
      pendingCadenceTimeMs = null;
      pendingDemandAdmission = false;
      pendingInputPulses.length = 0;
      pendingInputEnvelopeCount = 0;
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
