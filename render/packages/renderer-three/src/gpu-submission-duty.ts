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

export type RendererGpuSubmissionClass =
  | 'accelerated'
  | 'software'
  | 'unknown';

/**
 * Immutable observation of the current pacing state and latest completed
 * automatic-admission decision.
 */
export interface RendererGpuSubmissionDutySample {
  readonly schemaVersion: 1;
  readonly mode: RendererGpuSubmissionDutyMode;
  readonly state: RendererGpuSubmissionDutyState;
  readonly rendererClass: RendererGpuSubmissionClass;
  readonly timerDurationMs: number | null;
  readonly completionAgeMs: number | null;
  readonly completionAllowanceMs: number;
  readonly effectiveDurationMs: number | null;
  readonly targetDutyFraction: number | null;
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
  readonly deadlineOriginMs: number;
  readonly query: object;
  readonly submittedAtMs: number;
}

interface RendererGpuSubmissionActiveMeasurement {
  readonly deadlineOriginMs: number | null;
  readonly query: object;
}

const FAST_GPU_DURATION_MS = 8;
const COMPLETION_POLL_ALLOWANCE_MS = 17;
const MAXIMUM_GPU_DUTY_FRACTION = 0.5;
const MAXIMUM_ADDITIONAL_GPU_HEADROOM_MS = 100;
const MINIMUM_GPU_DUTY_FRACTION = 0.2;

/**
 * Leaves completion-derived browser headroom after automatic WebGL work.
 *
 * A timer query measures the previous submission without blocking the browser
 * thread. Positively identified software renderers can report a short GPU timer
 * duration while asynchronous completion still occupies browser CPU, so their
 * complete observed wall latency contributes to effective work. A valid timer
 * result on positively identified accelerated hardware is authoritative for
 * execution duration: delayed animation-frame polling does not become GPU
 * work. Unknown renderers and timing fallback paths retain one ordinary 60 Hz
 * polling allowance before wall latency adds pressure. The next automatic
 * submission is admitted after the effective duration plus a
 * completion-derived, bounded idle interval. The accelerated fast path keeps
 * four-millisecond work 120 Hz capable and eight-millisecond work 60 Hz
 * capable. Slower completion progressively reduces target duty toward twenty
 * percent so software rendering yields materially more browser and host CPU
 * time without adding a second loop or a fixed frame-rate cap.
 *
 * Explicit rendering remains caller-owned. Beginning a replacement submission
 * discards any older measurement and never waits for this optional pacing
 * mechanism.
 */
export class RendererGpuSubmissionDuty {
  readonly #clock: RendererGpuSubmissionClock;
  readonly #completionAllowanceMs: number;
  readonly #driver: RendererGpuSubmissionTimerDriver | null;
  readonly #maximumPendingMeasurements: number;
  readonly #rendererClass: RendererGpuSubmissionClass;
  #active: RendererGpuSubmissionActiveMeasurement | null = null;
  #disposed = false;
  #fallbackSubmittedAtMs: number | null = null;
  #minimumIntervalMs = 0;
  #notBeforeMs = 0;
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
    this.#completionAllowanceMs = this.#rendererClass === 'software'
      ? 0
      : COMPLETION_POLL_ALLOWANCE_MS;
    this.#sample = dutySample(
      driver === null ? 'completionOnly' : 'timerQuery',
      'idle',
      this.#rendererClass,
      this.#completionAllowanceMs,
    );
  }

  begin(submissionSourceTimeMs?: number): void {
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
      const observedAtMs = this.#readNow();
      const deadlineOriginMs = acceleratedDeadlineOrigin(
        this.#rendererClass,
        submissionSourceTimeMs,
        observedAtMs,
      );
      const query = this.#driver.begin();
      if (query === null) {
        this.#disableTimer();
      } else {
        this.#active = { query, deadlineOriginMs };
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
    const active = this.#active;
    const deadlineOriginMs = acceleratedDeadlineOrigin(
      this.#rendererClass,
      active?.deadlineOriginMs,
      submittedAtMs,
    );
    this.#notBeforeMs = Math.max(
      this.#notBeforeMs,
      deadlineOriginMs + this.#minimumIntervalMs,
    );
    this.#sample = updateDutySample(this.#sample, {
      mode: this.#mode(),
      state: 'measuring',
    });
    if (this.#driver === null || this.#timerDisabled || active === null) {
      this.#fallbackSubmittedAtMs = submittedAtMs;
      return;
    }
    const { query } = active;
    this.#active = null;
    try {
      this.#driver.end(query);
      this.#pending.push({
        deadlineOriginMs: acceleratedDeadlineOrigin(
          this.#rendererClass,
          active.deadlineOriginMs,
          submittedAtMs,
        ),
        query,
        submittedAtMs,
      });
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
    const { query } = this.#active;
    this.#active = null;
    try {
      this.#driver.end(query);
    } catch {
      // A failed render must still release the optional query object. The
      // renderer's original failure remains the actionable error.
    }
    this.#delete(query);
  }

  ready(submissionSourceTimeMs?: number): boolean {
    if (this.#disposed) {
      return true;
    }
    const nowMs = this.#readNow();
    if (nowMs === null) {
      this.#disablePacing();
      return true;
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
      this.#completeDecision(
        nowMs,
        result.durationMs,
        pending.deadlineOriginMs,
        pending.submittedAtMs,
      );
    }
    if (this.#fallbackSubmittedAtMs !== null) {
      const submittedAtMs = this.#fallbackSubmittedAtMs;
      this.#fallbackSubmittedAtMs = null;
      this.#completeDecision(nowMs, null, submittedAtMs, submittedAtMs);
    }
    const admissionLimit = this.#mode() === 'timerQuery'
      ? this.#maximumPendingMeasurements
      : 1;
    const capacityAvailable = this.#pending.length < admissionLimit;
    const deadlineTimeMs = acceleratedDeadlineOrigin(
      this.#rendererClass,
      submissionSourceTimeMs,
      nowMs,
    );
    const ready = capacityAvailable && deadlineTimeMs >= this.#notBeforeMs;
    this.#sample = updateDutySample(this.#sample, {
      mode: this.#mode(),
      state: ready
        ? 'ready'
        : capacityAvailable
          ? 'waiting'
          : 'measuring',
      ...(ready ? { admissionObservedAtMs: nowMs } : {}),
    });
    return ready;
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
    this.#minimumIntervalMs = 0;
    this.#notBeforeMs = 0;
    this.#disposed = true;
    this.#sample = updateDutySample(this.#sample, { state: 'disposed' });
  }

  #discardActive(): void {
    if (this.#driver === null || this.#active === null) {
      return;
    }
    const { query } = this.#active;
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

  #disablePacing(): void {
    this.#discardActive();
    this.#discardPending();
    this.#fallbackSubmittedAtMs = null;
    this.#minimumIntervalMs = 0;
    this.#notBeforeMs = 0;
    this.#timerDisabled = true;
    this.#sample = updateDutySample(this.#sample, {
      mode: 'timerFailed',
      state: 'ready',
    });
  }

  #completeDecision(
    nowMs: number,
    timerDurationMs: number | null,
    startedAtMs: number,
    submittedAtMs: number,
  ): void {
    const completionAgeMs = Math.max(0, nowMs - submittedAtMs);
    const completionPressureMs = Math.max(
      0,
      completionAgeMs - this.#completionAllowanceMs,
    );
    const acceleratedTimerIsAuthoritative =
      this.#rendererClass === 'accelerated' && timerDurationMs !== null;
    const effectiveDurationMs = acceleratedTimerIsAuthoritative
      ? timerDurationMs
      : Math.max(timerDurationMs ?? 0, completionPressureMs);
    const requestedDutyFraction = Math.min(
      MAXIMUM_GPU_DUTY_FRACTION,
      Math.max(
        MINIMUM_GPU_DUTY_FRACTION,
        (MAXIMUM_GPU_DUTY_FRACTION * FAST_GPU_DURATION_MS)
          / Math.max(effectiveDurationMs, Number.EPSILON),
      ),
    );
    const requestedHeadroomMs = effectiveDurationMs
      * ((1 / requestedDutyFraction) - 1);
    // The base headroom makes the stated fifty-percent maximum duty real even
    // when one software-rendered submission takes seconds. Only the additional
    // progressive headroom is capped, so exceptional work cannot collapse back
    // toward continuous GPU/CPU saturation while still retaining a bounded
    // latency penalty beyond equal work/headroom.
    const headroomMs = effectiveDurationMs + Math.min(
      MAXIMUM_ADDITIONAL_GPU_HEADROOM_MS,
      Math.max(0, requestedHeadroomMs - effectiveDurationMs),
    );
    const targetDutyFraction = effectiveDurationMs <= Number.EPSILON
      ? MAXIMUM_GPU_DUTY_FRACTION
      : effectiveDurationMs / (effectiveDurationMs + headroomMs);
    this.#minimumIntervalMs = effectiveDurationMs + headroomMs;
    const deadlineOriginMs = acceleratedTimerIsAuthoritative
      ? startedAtMs
      : submittedAtMs;
    this.#notBeforeMs = Math.max(
      this.#notBeforeMs,
      deadlineOriginMs + this.#minimumIntervalMs,
    );
    this.#sample = Object.freeze({
      schemaVersion: 1,
      mode: this.#mode(),
      state: nowMs >= this.#notBeforeMs ? 'ready' : 'waiting',
      rendererClass: this.#rendererClass,
      timerDurationMs,
      completionAgeMs,
      completionAllowanceMs: this.#completionAllowanceMs,
      effectiveDurationMs,
      targetDutyFraction,
      admittedAtMs: this.#notBeforeMs,
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

function acceleratedDeadlineOrigin(
  rendererClass: RendererGpuSubmissionClass,
  submissionSourceTimeMs: number | null | undefined,
  fallbackTimeMs: number | null,
): number {
  return rendererClass === 'accelerated'
    && submissionSourceTimeMs !== null
    && submissionSourceTimeMs !== undefined
    && Number.isFinite(submissionSourceTimeMs)
    && submissionSourceTimeMs >= 0
    ? submissionSourceTimeMs
    : fallbackTimeMs ?? 0;
}

function dutySample(
  mode: RendererGpuSubmissionDutyMode,
  state: RendererGpuSubmissionDutyState,
  rendererClass: RendererGpuSubmissionClass,
  completionAllowanceMs: number,
): RendererGpuSubmissionDutyDecisionSample {
  return Object.freeze({
    schemaVersion: 1,
    mode,
    state,
    rendererClass,
    timerDurationMs: null,
    completionAgeMs: null,
    completionAllowanceMs,
    effectiveDurationMs: null,
    targetDutyFraction: null,
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
