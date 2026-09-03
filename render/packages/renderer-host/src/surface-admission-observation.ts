import type {
  RendererSurfaceAutomaticSubmissionClass,
  RendererSurfaceAutomaticSubmissionPacingMode,
  RendererSurfaceAutomaticSubmissionPacingState,
} from './surface.js';
import type {
  RendererSurfaceSubmissionDemandDecision,
} from './surface-submission-demand.js';

export const RUSTY_RENDERER_SURFACE_ADMISSION_HISTORY_LIMIT = 64;
export const RUSTY_RENDERER_SURFACE_INTERVAL_HISTORY_LIMIT = 256;

export type RendererSurfaceAutomaticSubmissionAdmissionOutcome =
  | 'admitted'
  | 'backendBlocked'
  | 'noDemand';

export interface RendererSurfaceAutomaticSubmissionAdmissionBackend {
  readonly mode: RendererSurfaceAutomaticSubmissionPacingMode;
  readonly state: RendererSurfaceAutomaticSubmissionPacingState;
  readonly rendererClass: RendererSurfaceAutomaticSubmissionClass;
  readonly timerDurationMs: number | null;
  readonly effectiveDurationMs: number | null;
  readonly admittedAtMs: number | null;
  readonly admissionObservedAtMs: number | null;
  readonly observedAtMs: number | null;
  readonly automaticSubmissionLimit: number;
  readonly pendingMeasurementCount: number;
  readonly completionFenceMode: 'active' | 'disabled' | 'unsupported';
  readonly maximumPendingSubmissions: number;
  readonly pendingSubmissionCount: number;
}

export interface RendererSurfaceAutomaticSubmissionAdmissionAttempt {
  readonly schemaVersion: 1;
  readonly sequence: number;
  readonly sourceTimeMs: number;
  readonly outcome: RendererSurfaceAutomaticSubmissionAdmissionOutcome;
  readonly demand: RendererSurfaceSubmissionDemandDecision;
  readonly backend: RendererSurfaceAutomaticSubmissionAdmissionBackend;
  readonly callback: RendererSurfaceAutomaticSubmissionCallbackPhases;
}

/**
 * Wall-clock phase boundaries for one host RAF callback.
 *
 * These immutable timestamps attribute callback work without polling,
 * scheduling, or submitting anything beyond the owning RAF callback. A null
 * phase was not reached for that outcome.
 */
export interface RendererSurfaceAutomaticSubmissionCallbackPhases {
  readonly schemaVersion: 1;
  readonly callbackStartedAtMs: number;
  readonly successorQueuedAtMs: number;
  readonly demandObservedAtMs: number;
  readonly backendReadinessObservedAtMs: number | null;
  readonly controlsUpdatedAtMs: number | null;
  readonly cameraUpdatedAtMs: number | null;
  readonly presentationAdvancedAtMs: number | null;
  readonly backendSubmittedAtMs: number | null;
  readonly callbackEndedAtMs: number;
}

/** Bounded immutable observation of every recent host RAF admission attempt. */
export interface RendererSurfaceAutomaticSubmissionAdmissionSample {
  readonly schemaVersion: 1;
  readonly attemptCount: number;
  readonly admittedCount: number;
  readonly backendBlockedCount: number;
  readonly noDemandCount: number;
  readonly firstAttemptAtMs: number | null;
  readonly lastAttemptAtMs: number | null;
  readonly demandCounts: {
    readonly requested: number;
    readonly viewportChanged: number;
    readonly controls: number;
    readonly presentation: number;
    readonly retainedAnimation: number;
  };
  readonly recentCallbackIntervalsMs: readonly number[];
  readonly recentSubmissionIntervalsMs: readonly number[];
  readonly recentAttempts: readonly RendererSurfaceAutomaticSubmissionAdmissionAttempt[];
}

export interface RendererSurfaceAutomaticSubmissionBackendReadout {
  readonly mode: RendererSurfaceAutomaticSubmissionPacingMode;
  readonly state: RendererSurfaceAutomaticSubmissionPacingState;
  readonly rendererClass: RendererSurfaceAutomaticSubmissionClass;
  readonly timerDurationMs: number | null;
  readonly effectiveDurationMs: number | null;
  readonly admittedAtMs: number | null;
  readonly admissionObservedAtMs: number | null;
  readonly observedAtMs: number | null;
  readonly automaticSubmissionLimit: number;
  readonly pendingMeasurementCount: number;
  readonly completionFenceMode: 'active' | 'disabled' | 'unsupported';
  readonly maximumPendingSubmissions: number;
  readonly pendingSubmissionCount: number;
}

/**
 * Records host admission decisions without polling readiness or scheduling work.
 *
 * The fixed-size history lets a consumer distinguish sparse browser callbacks,
 * absent owner demand, and a backend capacity/duty rejection without installing
 * a second observer loop inside the renderer.
 */
export class RendererSurfaceAutomaticSubmissionAdmissionObservation {
  #admittedCount = 0;
  #attemptCount = 0;
  #backendBlockedCount = 0;
  #noDemandCount = 0;
  #firstAttemptAtMs: number | null = null;
  #lastAttemptAtMs: number | null = null;
  #lastSubmissionAtMs: number | null = null;
  readonly #demandCounts = {
    requested: 0,
    viewportChanged: 0,
    controls: 0,
    presentation: 0,
    retainedAnimation: 0,
  };
  readonly #recentCallbackIntervalsMs: number[] = [];
  readonly #recentSubmissionIntervalsMs: number[] = [];
  readonly #recentAttempts: RendererSurfaceAutomaticSubmissionAdmissionAttempt[] = [];

  record(
    sourceTimeMs: number,
    outcome: RendererSurfaceAutomaticSubmissionAdmissionOutcome,
    demand: RendererSurfaceSubmissionDemandDecision,
    backend: RendererSurfaceAutomaticSubmissionBackendReadout,
    callback: RendererSurfaceAutomaticSubmissionCallbackPhases,
  ): void {
    if (this.#firstAttemptAtMs === null) this.#firstAttemptAtMs = sourceTimeMs;
    if (this.#lastAttemptAtMs !== null) {
      retainInterval(this.#recentCallbackIntervalsMs, sourceTimeMs - this.#lastAttemptAtMs);
    }
    this.#lastAttemptAtMs = sourceTimeMs;
    for (const reason of ['requested', 'viewportChanged', 'controls', 'presentation', 'retainedAnimation'] as const) {
      if (demand[reason]) this.#demandCounts[reason] += 1;
    }
    this.#attemptCount += 1;
    switch (outcome) {
      case 'admitted':
        this.#admittedCount += 1;
        if (this.#lastSubmissionAtMs !== null) {
          retainInterval(this.#recentSubmissionIntervalsMs, sourceTimeMs - this.#lastSubmissionAtMs);
        }
        this.#lastSubmissionAtMs = sourceTimeMs;
        break;
      case 'backendBlocked':
        this.#backendBlockedCount += 1;
        break;
      case 'noDemand':
        this.#noDemandCount += 1;
        break;
    }
    const attempt = Object.freeze({
      schemaVersion: 1 as const,
      sequence: this.#attemptCount,
      sourceTimeMs,
      outcome,
      demand,
      callback: Object.freeze({ ...callback }),
      backend: Object.freeze({
        mode: backend.mode,
        state: backend.state,
        rendererClass: backend.rendererClass,
        timerDurationMs: backend.timerDurationMs,
        effectiveDurationMs: backend.effectiveDurationMs,
        admittedAtMs: backend.admittedAtMs,
        admissionObservedAtMs: backend.admissionObservedAtMs,
        observedAtMs: backend.observedAtMs,
        automaticSubmissionLimit: backend.automaticSubmissionLimit,
        pendingMeasurementCount: backend.pendingMeasurementCount,
        completionFenceMode: backend.completionFenceMode,
        maximumPendingSubmissions: backend.maximumPendingSubmissions,
        pendingSubmissionCount: backend.pendingSubmissionCount,
      }),
    });
    this.#recentAttempts.push(attempt);
    if (this.#recentAttempts.length > RUSTY_RENDERER_SURFACE_ADMISSION_HISTORY_LIMIT) {
      this.#recentAttempts.shift();
    }
  }

  sample(): RendererSurfaceAutomaticSubmissionAdmissionSample {
    return Object.freeze({
      schemaVersion: 1,
      attemptCount: this.#attemptCount,
      admittedCount: this.#admittedCount,
      backendBlockedCount: this.#backendBlockedCount,
      noDemandCount: this.#noDemandCount,
      firstAttemptAtMs: this.#firstAttemptAtMs,
      lastAttemptAtMs: this.#lastAttemptAtMs,
      demandCounts: Object.freeze({ ...this.#demandCounts }),
      recentCallbackIntervalsMs: Object.freeze([...this.#recentCallbackIntervalsMs]),
      recentSubmissionIntervalsMs: Object.freeze([...this.#recentSubmissionIntervalsMs]),
      recentAttempts: Object.freeze([...this.#recentAttempts]),
    });
  }
}

function retainInterval(history: number[], intervalMs: number): void {
  if (!Number.isFinite(intervalMs) || intervalMs < 0) return;
  history.push(intervalMs);
  if (history.length > RUSTY_RENDERER_SURFACE_INTERVAL_HISTORY_LIMIT) history.shift();
}
