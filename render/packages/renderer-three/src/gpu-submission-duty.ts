export type RendererGpuSubmissionTimerPoll =
  | { readonly status: 'failed' }
  | { readonly status: 'pending' }
  | {
      readonly durationMs: number;
      readonly status: 'complete';
    };

export interface RendererGpuSubmissionTimerDriver {
  readonly begin: () => object | null;
  readonly delete: (query: object) => void;
  readonly end: (query: object) => void;
  readonly now: () => number;
  readonly poll: (query: object) => RendererGpuSubmissionTimerPoll;
}

export type RendererGpuSubmissionDutyMode =
  | 'completionOnly'
  | 'timerFailed'
  | 'timerQuery';

export type RendererGpuSubmissionDutyState =
  | 'disposed'
  | 'idle'
  | 'measuring'
  | 'ready';

export type RendererGpuSubmissionClass =
  | 'accelerated'
  | 'software'
  | 'unknown';

/**
 * Immutable observation of the latest completed GPU submission measurement.
 */
export interface RendererGpuSubmissionDutySample {
  readonly schemaVersion: 1;
  readonly mode: RendererGpuSubmissionDutyMode;
  readonly state: RendererGpuSubmissionDutyState;
  readonly rendererClass: RendererGpuSubmissionClass;
  readonly timerDurationMs: number | null;
  readonly completionAgeMs: number | null;
  /** Measured duration: the timer result, or completion age without a timer. */
  readonly effectiveDurationMs: number | null;
  readonly admittedAtMs: number | null;
  readonly admissionObservedAtMs: number | null;
  readonly observedAtMs: number | null;
  readonly maximumPendingMeasurements: number;
  readonly pendingMeasurementCount: number;
}

type RendererGpuSubmissionDutyDecisionSample = Omit<
  RendererGpuSubmissionDutySample,
  'maximumPendingMeasurements' | 'pendingMeasurementCount'
>;

interface RendererGpuSubmissionClock {
  readonly now: () => number;
}

interface RendererGpuSubmissionDutyOptions {
  readonly clock?: RendererGpuSubmissionClock;
  readonly maximumPendingMeasurements?: number;
  readonly rendererClass?: RendererGpuSubmissionClass;
}

interface RendererGpuSubmissionPendingMeasurement {
  readonly query: object;
  readonly submittedAtMs: number;
}

/**
 * Measures automatic WebGL submissions without pacing them.
 *
 * A timer query measures a submission without blocking the browser thread;
 * without a timer the observed completion age stands in. Measurements feed
 * renderer diagnostics only. Submission cadence is owned by the animation
 * frame and the completion fence, never by a duty-cycle policy. Beginning a
 * submission discards the oldest measurement when the bounded ring is full.
 */
export class RendererGpuSubmissionDuty {
  readonly #clock: RendererGpuSubmissionClock;
  readonly #driver: RendererGpuSubmissionTimerDriver | null;
  readonly #maximumPendingMeasurements: number;
  readonly #rendererClass: RendererGpuSubmissionClass;
  #active: object | null = null;
  #disposed = false;
  #fallbackSubmittedAtMs: number | null = null;
  readonly #pending: RendererGpuSubmissionPendingMeasurement[] = [];
  #sample: RendererGpuSubmissionDutyDecisionSample;
  #timerDisabled = false;

  constructor(
    driver: RendererGpuSubmissionTimerDriver | null,
    options: RendererGpuSubmissionDutyOptions = {},
  ) {
    this.#driver = driver;
    this.#clock = options.clock ?? driver ?? defaultSubmissionClock();
    this.#rendererClass = options.rendererClass ?? 'unknown';
    this.#maximumPendingMeasurements = positiveInteger(
      options.maximumPendingMeasurements ?? 1,
      'maximum pending GPU measurements',
    );
    this.#sample = dutySample(
      driver === null ? 'completionOnly' : 'timerQuery',
      'idle',
      this.#rendererClass,
    );
  }

  begin(): void {
    if (this.#disposed) {
      return;
    }
    this.#discardActive();
    while (this.#pending.length >= this.#maximumPendingMeasurements) {
      this.#discardOldestPending();
    }
    this.#fallbackSubmittedAtMs = null;
    this.#sample = updateDutySample(this.#sample, {
      mode: this.#mode(),
      state: this.#pending.length === 0 ? 'idle' : 'measuring',
    });
    if (this.#driver === null || this.#timerDisabled) {
      return;
    }
    try {
      const query = this.#driver.begin();
      if (query === null) {
        this.#disableTimer();
      } else {
        this.#active = query;
      }
    } catch {
      this.#disableTimer();
    }
  }

  submitted(): void {
    if (this.#disposed) {
      return;
    }
    const submittedAtMs = this.#readNow();
    if (submittedAtMs === null) {
      this.#disableTimer();
      return;
    }
    this.#sample = updateDutySample(this.#sample, {
      mode: this.#mode(),
      state: 'measuring',
    });
    const query = this.#active;
    if (this.#driver === null || this.#timerDisabled || query === null) {
      this.#fallbackSubmittedAtMs = submittedAtMs;
      return;
    }
    this.#active = null;
    try {
      this.#driver.end(query);
      this.#pending.push({ query, submittedAtMs });
    } catch {
      this.#delete(query);
      this.#disableTimer();
      this.#fallbackSubmittedAtMs = submittedAtMs;
    }
  }

  aborted(): void {
    if (this.#driver === null || this.#active === null) {
      return;
    }
    const query = this.#active;
    this.#active = null;
    try {
      this.#driver.end(query);
    } catch {
      // A failed render must still release the optional query object. The
      // renderer's original failure remains the actionable error.
    }
    this.#delete(query);
  }

  /** Collect completed measurements. Never withholds a submission. */
  observe(): void {
    if (this.#disposed) {
      return;
    }
    const nowMs = this.#readNow();
    if (nowMs === null) {
      this.#disableTimer();
      return;
    }
    for (let index = 0; index < this.#pending.length;) {
      const pending = this.#pending[index];
      if (pending === undefined) {
        index += 1;
        continue;
      }
      let result: RendererGpuSubmissionTimerPoll;
      if (this.#driver === null) {
        result = { status: 'failed' };
      } else {
        try {
          result = this.#driver.poll(pending.query);
        } catch {
          result = { status: 'failed' };
        }
      }
      if (result.status === 'pending') {
        index += 1;
        continue;
      }
      if (result.status === 'failed'
        || !Number.isFinite(result.durationMs)
        || result.durationMs < 0) {
        const fallbackSubmittedAtMs = Math.max(
          pending.submittedAtMs,
          ...this.#pending.map((measurement) => measurement.submittedAtMs),
        );
        this.#disableTimer();
        this.#fallbackSubmittedAtMs = fallbackSubmittedAtMs;
        break;
      }
      this.#pending.splice(index, 1);
      this.#delete(pending.query);
      this.#record(nowMs, result.durationMs, pending.submittedAtMs);
    }
    if (this.#fallbackSubmittedAtMs !== null) {
      const submittedAtMs = this.#fallbackSubmittedAtMs;
      this.#fallbackSubmittedAtMs = null;
      this.#record(nowMs, null, submittedAtMs);
    }
    this.#sample = updateDutySample(this.#sample, {
      mode: this.#mode(),
      state: 'ready',
      admissionObservedAtMs: nowMs,
    });
  }

  sample(): RendererGpuSubmissionDutySample {
    return Object.freeze({
      ...this.#sample,
      maximumPendingMeasurements: this.#mode() === 'timerQuery'
        ? this.#maximumPendingMeasurements
        : 1,
      pendingMeasurementCount: this.#pending.length,
    });
  }

  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#discardActive();
    this.#discardPending();
    this.#fallbackSubmittedAtMs = null;
    this.#disposed = true;
    this.#sample = updateDutySample(this.#sample, { state: 'disposed' });
  }

  #discardActive(): void {
    if (this.#driver === null || this.#active === null) {
      return;
    }
    const query = this.#active;
    this.#active = null;
    try {
      this.#driver.end(query);
    } catch {
      // Replacement remains explicit and fail-open for optional measurement.
    }
    this.#delete(query);
  }

  #discardPending(): void {
    for (const pending of this.#pending) {
      this.#delete(pending.query);
    }
    this.#pending.length = 0;
  }

  #discardOldestPending(): void {
    const pending = this.#pending.shift();
    if (pending !== undefined) {
      this.#delete(pending.query);
    }
  }

  #delete(query: object): void {
    if (this.#driver === null) {
      return;
    }
    try {
      this.#driver.delete(query);
    } catch {
      this.#timerDisabled = true;
    }
  }

  #disableTimer(): void {
    this.#discardActive();
    this.#discardPending();
    this.#timerDisabled = true;
    this.#sample = updateDutySample(this.#sample, { mode: 'timerFailed' });
  }

  #record(nowMs: number, timerDurationMs: number | null, submittedAtMs: number): void {
    const completionAgeMs = Math.max(0, nowMs - submittedAtMs);
    this.#sample = Object.freeze({
      schemaVersion: 1,
      mode: this.#mode(),
      state: 'ready',
      rendererClass: this.#rendererClass,
      timerDurationMs,
      completionAgeMs,
      effectiveDurationMs: timerDurationMs ?? completionAgeMs,
      admittedAtMs: nowMs,
      admissionObservedAtMs: null,
      observedAtMs: nowMs,
    });
  }

  #mode(): RendererGpuSubmissionDutyMode {
    if (this.#timerDisabled) {
      return 'timerFailed';
    }
    return this.#driver === null ? 'completionOnly' : 'timerQuery';
  }

  #readNow(): number | null {
    try {
      const nowMs = this.#clock.now();
      return Number.isFinite(nowMs) && nowMs >= 0 ? nowMs : null;
    } catch {
      return null;
    }
  }
}

function defaultSubmissionClock(): RendererGpuSubmissionClock {
  return {
    now: () => globalThis.performance?.now() ?? 0,
  };
}

function dutySample(
  mode: RendererGpuSubmissionDutyMode,
  state: RendererGpuSubmissionDutyState,
  rendererClass: RendererGpuSubmissionClass,
): RendererGpuSubmissionDutyDecisionSample {
  return Object.freeze({
    schemaVersion: 1,
    mode,
    state,
    rendererClass,
    timerDurationMs: null,
    completionAgeMs: null,
    effectiveDurationMs: null,
    admittedAtMs: null,
    admissionObservedAtMs: null,
    observedAtMs: null,
  });
}

function updateDutySample(
  current: RendererGpuSubmissionDutyDecisionSample,
  update: Partial<Pick<
    RendererGpuSubmissionDutyDecisionSample,
    'admissionObservedAtMs' | 'mode' | 'state'
  >>,
): RendererGpuSubmissionDutyDecisionSample {
  return Object.freeze({
    ...current,
    ...update,
  });
}

function positiveInteger(value: number, label: string): number {
  if (!Number.isSafeInteger(value) || value < 1) {
    throw new RangeError(`${label} must be a positive safe integer`);
  }
  return value;
}
