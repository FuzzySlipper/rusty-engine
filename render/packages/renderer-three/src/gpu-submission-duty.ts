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

const TARGET_GPU_DUTY_FRACTION = 0.5;
const MAXIMUM_GPU_HEADROOM_MS = 100;

/**
 * Leaves completion-derived browser headroom after automatic WebGL work.
 *
 * A timer query measures the previous submission without blocking the browser
 * thread. The next automatic submission is admitted after the measured GPU
 * duration plus an equal, bounded idle interval. Fast backends whose work and
 * headroom fit within one display interval therefore retain display-rate
 * rendering, while slower software renderers yield CPU time without adding a
 * second loop or a fixed frame-rate cap.
 *
 * Explicit rendering remains caller-owned. Beginning a replacement submission
 * discards any older measurement and never waits for this optional pacing
 * mechanism.
 */
export class RendererGpuSubmissionDuty {
  readonly #driver: RendererGpuSubmissionTimerDriver | null;
  #active: object | null = null;
  #disabled = false;
  #notBeforeMs = 0;
  #pending: object | null = null;
  #submittedAtMs: number | null = null;

  constructor(driver: RendererGpuSubmissionTimerDriver | null) {
    this.#driver = driver;
  }

  begin(): void {
    if (this.#driver === null || this.#disabled) {
      return;
    }
    this.#discardActive();
    this.#discardPending();
    this.#notBeforeMs = 0;
    this.#submittedAtMs = null;
    try {
      this.#active = this.#driver.begin();
      if (this.#active === null) {
        this.#disabled = true;
      }
    } catch {
      this.#disable();
    }
  }

  submitted(): void {
    if (this.#driver === null || this.#disabled || this.#active === null) {
      return;
    }
    const query = this.#active;
    this.#active = null;
    try {
      this.#driver.end(query);
      this.#pending = query;
      this.#submittedAtMs = this.#driver.now();
    } catch {
      this.#delete(query);
      this.#disable();
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
    if (this.#driver === null || this.#disabled) {
      return true;
    }
    const nowMs = this.#now();
    if (this.#pending !== null) {
      let result: RendererGpuSubmissionTimerPoll;
      try {
        result = this.#driver.poll(this.#pending);
      } catch {
        this.#disable();
        return true;
      }
      if (result.status === 'pending') {
        return false;
      }
      if (result.status === 'failed') {
        this.#disable();
        return true;
      }
      const query = this.#pending;
      this.#pending = null;
      this.#delete(query);
      if (
        this.#submittedAtMs === null
        || !Number.isFinite(result.durationMs)
        || result.durationMs < 0
      ) {
        this.#disable();
        return true;
      }
      const headroomMs = Math.min(
        MAXIMUM_GPU_HEADROOM_MS,
        result.durationMs * ((1 / TARGET_GPU_DUTY_FRACTION) - 1),
      );
      this.#notBeforeMs = this.#submittedAtMs + result.durationMs + headroomMs;
      this.#submittedAtMs = null;
    }
    return nowMs >= this.#notBeforeMs;
  }

  dispose(): void {
    this.#disable();
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
      this.#disabled = true;
    }
  }

  #disable(): void {
    this.#discardActive();
    this.#discardPending();
    this.#submittedAtMs = null;
    this.#notBeforeMs = 0;
    this.#disabled = true;
  }

  #now(): number {
    if (this.#driver === null) {
      return 0;
    }
    let nowMs: number;
    try {
      nowMs = this.#driver.now();
    } catch {
      this.#disable();
      return 0;
    }
    if (!Number.isFinite(nowMs) || nowMs < 0) {
      this.#disable();
      return 0;
    }
    return nowMs;
  }
}
