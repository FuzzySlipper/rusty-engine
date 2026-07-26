export const RUSTY_RENDERER_SURFACE_TIMING_SCHEMA_VERSION = 1;
export const RUSTY_RENDERER_SURFACE_MAX_TIMING_DURATION_MS = 60_000;

export type RendererSurfaceTimingSource =
  | 'mount'
  | 'animationFrame'
  | 'explicit'
  | 'cameraReset';

export type RendererSurfaceFrameIntervalStatus =
  | 'available'
  | 'firstFrame'
  | 'sourceTimeRegressed'
  | 'sourceTimeGapExceeded';

export type RendererSurfaceSubmissionDurationStatus =
  | 'available'
  | 'clockUnavailable'
  | 'clockRegressed'
  | 'durationExceeded';

/**
 * Immutable timing for one successfully submitted renderer-host frame.
 *
 * `frameIntervalMs` is cadence: the difference between this frame's source
 * timestamp and the previous submitted frame's source timestamp. It is not
 * CPU or GPU work duration. `backendSubmissionDurationMs` is synchronous host
 * clock time spent inside the backend `renderOnce` call; it does not claim GPU
 * completion time.
 */
export interface RendererSurfaceTimingSample {
  readonly schemaVersion: 1;
  readonly renderSequence: number;
  readonly source: RendererSurfaceTimingSource;
  readonly sourceTimeMs: number;
  readonly frameIntervalMs: number | null;
  readonly frameIntervalStatus: RendererSurfaceFrameIntervalStatus;
  readonly backendSubmissionDurationMs: number | null;
  readonly backendSubmissionDurationStatus: RendererSurfaceSubmissionDurationStatus;
}

interface RendererSurfaceTimingRecord {
  readonly source: RendererSurfaceTimingSource;
  readonly sourceTimeMs: number;
  readonly backendSubmissionStartedMs: number;
  readonly backendSubmissionEndedMs: number;
}

export class RendererSurfaceTimingTracker {
  #lastSourceTimeMs: number | null = null;
  #renderSequence = 0;
  #latest: RendererSurfaceTimingSample | null = null;

  record(record: RendererSurfaceTimingRecord): RendererSurfaceTimingSample {
    assertRendererSurfaceSourceTime(record.sourceTimeMs);
    if (this.#renderSequence === Number.MAX_SAFE_INTEGER) {
      throw new Error('renderer surface timing sequence is exhausted');
    }

    const frameInterval = resolveFrameInterval(this.#lastSourceTimeMs, record.sourceTimeMs);
    const submissionDuration = resolveSubmissionDuration(
      record.backendSubmissionStartedMs,
      record.backendSubmissionEndedMs,
    );
    const sample = Object.freeze({
      schemaVersion: RUSTY_RENDERER_SURFACE_TIMING_SCHEMA_VERSION,
      renderSequence: this.#renderSequence + 1,
      source: record.source,
      sourceTimeMs: record.sourceTimeMs,
      frameIntervalMs: frameInterval.value,
      frameIntervalStatus: frameInterval.status,
      backendSubmissionDurationMs: submissionDuration.value,
      backendSubmissionDurationStatus: submissionDuration.status,
    } satisfies RendererSurfaceTimingSample);

    this.#lastSourceTimeMs = record.sourceTimeMs;
    this.#renderSequence = sample.renderSequence;
    this.#latest = sample;
    return sample;
  }

  read(): RendererSurfaceTimingSample {
    if (this.#latest === null) {
      throw new Error('renderer surface has not submitted a frame');
    }
    return this.#latest;
  }
}

export function assertRendererSurfaceSourceTime(value: number): void {
  if (!Number.isFinite(value) || value < 0 || value > Number.MAX_SAFE_INTEGER) {
    throw new Error('renderer surface source time must be finite and in 0..=Number.MAX_SAFE_INTEGER');
  }
}

function resolveFrameInterval(
  previous: number | null,
  current: number,
): { readonly value: number | null; readonly status: RendererSurfaceFrameIntervalStatus } {
  if (previous === null) {
    return { value: null, status: 'firstFrame' };
  }
  const interval = current - previous;
  if (interval < 0) {
    return { value: null, status: 'sourceTimeRegressed' };
  }
  if (interval > RUSTY_RENDERER_SURFACE_MAX_TIMING_DURATION_MS) {
    return { value: null, status: 'sourceTimeGapExceeded' };
  }
  return { value: interval, status: 'available' };
}

function resolveSubmissionDuration(
  started: number,
  ended: number,
): {
  readonly value: number | null;
  readonly status: RendererSurfaceSubmissionDurationStatus;
} {
  if (!Number.isFinite(started) || !Number.isFinite(ended) || started < 0 || ended < 0) {
    return { value: null, status: 'clockUnavailable' };
  }
  const duration = ended - started;
  if (duration < 0) {
    return { value: null, status: 'clockRegressed' };
  }
  if (duration > RUSTY_RENDERER_SURFACE_MAX_TIMING_DURATION_MS) {
    return { value: null, status: 'durationExceeded' };
  }
  return { value: duration, status: 'available' };
}
