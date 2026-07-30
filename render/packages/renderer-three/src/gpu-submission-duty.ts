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
  | 'ready'
  | 'waiting';

/**
 * Immutable observation of the current pacing state and latest completed
 * automatic-admission decision.
 */
export interface RendererGpuSubmissionDutySample {
  readonly schemaVersion: 1;
  readonly mode: RendererGpuSubmissionDutyMode;
  readonly state: RendererGpuSubmissionDutyState;
  readonly timerDurationMs: number | null;
  readonly completionAgeMs: number | null;
  readonly effectiveDurationMs: number | null;
  readonly targetDutyFraction: number | null;
  readonly admittedAtMs: number | null;
  readonly observedAtMs: number | null;
}

interface RendererGpuSubmissionClock {
  readonly now: () => number;
}

const FAST_GPU_DURATION_MS = 8;
const COMPLETION_POLL_ALLOWANCE_MS = 17;
const MAXIMUM_GPU_DUTY_FRACTION = 0.5;
const MAXIMUM_GPU_HEADROOM_MS = 100;
const MINIMUM_GPU_DUTY_FRACTION = 0.2;

/**
 * Leaves completion-derived browser headroom after automatic WebGL work.
 *
 * A timer query measures the previous submission without blocking the browser
 * thread. Because software renderers may report a short GPU timer duration
 * while their completion still occupies browser CPU, the estimator also
 * includes completion wall latency beyond one ordinary 60 Hz polling interval.
 * The next automatic submission is admitted after the effective duration plus
 * a completion-derived, bounded idle interval. Work completed within that
 * polling allowance retains the timer-derived fast path: four-millisecond work
 * remains 120 Hz capable and eight-millisecond work remains 60 Hz capable.
 * Slower completion progressively reduces target duty toward twenty percent so
 * software rendering yields materially more browser and host CPU time without
 * adding a second loop or a fixed frame-rate cap.
 *
 * Explicit rendering remains caller-owned. Beginning a replacement submission
 * discards any older measurement and never waits for this optional pacing
 * mechanism.
 */
export class RendererGpuSubmissionDuty {
  readonly #clock: RendererGpuSubmissionClock;
  readonly #driver: RendererGpuSubmissionTimerDriver | null;
  #active: object | null = null;
  #disposed = false;
  #notBeforeMs = 0;
  #pending: object | null = null;
  #sample: RendererGpuSubmissionDutySample;
  #submittedAtMs: number | null = null;
  #timerDisabled = false;

  constructor(
    driver: RendererGpuSubmissionTimerDriver | null,
    clock: RendererGpuSubmissionClock = driver ?? defaultSubmissionClock(),
  ) {
    this.#driver = driver;
    this.#clock = clock;
    this.#sample = dutySample(
      driver === null ? 'completionOnly' : 'timerQuery',
      'idle',
    );
  }

  begin(): void {
    if (this.#disposed) {
      return;
    }
    this.#discardActive();
    this.#discardPending();
    this.#notBeforeMs = 0;
    this.#submittedAtMs = null;
    this.#sample = updateDutySample(this.#sample, {
      mode: this.#mode(),
      state: 'idle',
    });
    if (this.#driver === null || this.#timerDisabled) {
      return;
    }
    try {
      this.#active = this.#driver.begin();
      if (this.#active === null) {
        this.#disableTimer();
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
      this.#disablePacing();
      return;
    }
    this.#submittedAtMs = submittedAtMs;
    this.#sample = updateDutySample(this.#sample, {
      mode: this.#mode(),
      state: 'measuring',
    });
    if (this.#driver === null || this.#timerDisabled || this.#active === null) {
      return;
    }
    const query = this.#active;
    this.#active = null;
    try {
      this.#driver.end(query);
      this.#pending = query;
    } catch {
      this.#delete(query);
      this.#disableTimer();
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

  ready(): boolean {
    if (this.#disposed) {
      return true;
    }
    const nowMs = this.#readNow();
    if (nowMs === null) {
      this.#disablePacing();
      return true;
    }
    if (this.#pending !== null) {
      let result: RendererGpuSubmissionTimerPoll;
      if (this.#driver === null) {
        this.#disableTimer();
        result = { status: 'failed' };
      } else {
        try {
          result = this.#driver.poll(this.#pending);
        } catch {
          this.#disableTimer();
          result = { status: 'failed' };
        }
      }
      if (result.status === 'pending') {
        this.#sample = updateDutySample(this.#sample, {
          mode: this.#mode(),
          state: 'measuring',
        });
        return false;
      }
      if (result.status === 'failed') {
        this.#disableTimer();
      } else {
        const query = this.#pending;
        this.#pending = null;
        this.#delete(query);
        if (!Number.isFinite(result.durationMs) || result.durationMs < 0) {
          this.#disableTimer();
        } else {
          this.#completeDecision(nowMs, result.durationMs);
        }
      }
    }
    if (this.#submittedAtMs !== null && this.#pending === null) {
      this.#completeDecision(nowMs, null);
    }
    const ready = nowMs >= this.#notBeforeMs;
    this.#sample = updateDutySample(this.#sample, {
      mode: this.#mode(),
      state: ready ? 'ready' : 'waiting',
    });
    return ready;
  }

  sample(): RendererGpuSubmissionDutySample {
    return this.#sample;
  }

  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#discardActive();
    this.#discardPending();
    this.#submittedAtMs = null;
    this.#notBeforeMs = 0;
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
    if (this.#pending === null) {
      return;
    }
    const query = this.#pending;
    this.#pending = null;
    this.#delete(query);
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

  #disablePacing(): void {
    this.#discardActive();
    this.#discardPending();
    this.#submittedAtMs = null;
    this.#notBeforeMs = 0;
    this.#timerDisabled = true;
    this.#sample = updateDutySample(this.#sample, {
      mode: 'timerFailed',
      state: 'ready',
    });
  }

  #completeDecision(nowMs: number, timerDurationMs: number | null): void {
    if (this.#submittedAtMs === null) {
      return;
    }
    const completionAgeMs = Math.max(0, nowMs - this.#submittedAtMs);
    const completionPressureMs = Math.max(
      0,
      completionAgeMs - COMPLETION_POLL_ALLOWANCE_MS,
    );
    const effectiveDurationMs = Math.max(
      timerDurationMs ?? 0,
      completionPressureMs,
    );
    const targetDutyFraction = Math.min(
      MAXIMUM_GPU_DUTY_FRACTION,
      Math.max(
        MINIMUM_GPU_DUTY_FRACTION,
        (MAXIMUM_GPU_DUTY_FRACTION * FAST_GPU_DURATION_MS)
          / Math.max(effectiveDurationMs, Number.EPSILON),
      ),
    );
    const headroomMs = Math.min(
      MAXIMUM_GPU_HEADROOM_MS,
      effectiveDurationMs * ((1 / targetDutyFraction) - 1),
    );
    this.#notBeforeMs = this.#submittedAtMs + effectiveDurationMs + headroomMs;
    this.#sample = Object.freeze({
      schemaVersion: 1,
      mode: this.#mode(),
      state: nowMs >= this.#notBeforeMs ? 'ready' : 'waiting',
      timerDurationMs,
      completionAgeMs,
      effectiveDurationMs,
      targetDutyFraction,
      admittedAtMs: this.#notBeforeMs,
      observedAtMs: nowMs,
    });
    this.#submittedAtMs = null;
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
): RendererGpuSubmissionDutySample {
  return Object.freeze({
    schemaVersion: 1,
    mode,
    state,
    timerDurationMs: null,
    completionAgeMs: null,
    effectiveDurationMs: null,
    targetDutyFraction: null,
    admittedAtMs: null,
    observedAtMs: null,
  });
}

function updateDutySample(
  current: RendererGpuSubmissionDutySample,
  update: Partial<Pick<RendererGpuSubmissionDutySample, 'mode' | 'state'>>,
): RendererGpuSubmissionDutySample {
  return Object.freeze({
    ...current,
    ...update,
  });
}
