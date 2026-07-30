import type {
  RendererSurfaceAutomaticSubmissionClass,
  RendererSurfaceAutomaticSubmissionPacingMode,
  RendererSurfaceAutomaticSubmissionPacingState,
} from './surface.js';
import type {
  RendererSurfaceSubmissionDemandDecision,
} from './surface-submission-demand.js';

export const RUSTY_RENDERER_SURFACE_ADMISSION_HISTORY_LIMIT = 64;

export type RendererSurfaceAutomaticSubmissionAdmissionOutcome =
  | 'admitted'
  | 'backendBlocked'
  | 'noDemand';

export interface RendererSurfaceAutomaticSubmissionAdmissionBackend {
  readonly mode: RendererSurfaceAutomaticSubmissionPacingMode;
  readonly state: RendererSurfaceAutomaticSubmissionPacingState;
  readonly rendererClass: RendererSurfaceAutomaticSubmissionClass;
  readonly admittedAtMs: number | null;
  readonly admissionObservedAtMs: number | null;
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
}

/** Bounded immutable observation of every recent host RAF admission attempt. */
export interface RendererSurfaceAutomaticSubmissionAdmissionSample {
  readonly schemaVersion: 1;
  readonly attemptCount: number;
  readonly admittedCount: number;
  readonly backendBlockedCount: number;
  readonly noDemandCount: number;
  readonly recentAttempts: readonly RendererSurfaceAutomaticSubmissionAdmissionAttempt[];
}

export interface RendererSurfaceAutomaticSubmissionBackendReadout {
  readonly mode: RendererSurfaceAutomaticSubmissionPacingMode;
  readonly state: RendererSurfaceAutomaticSubmissionPacingState;
  readonly rendererClass: RendererSurfaceAutomaticSubmissionClass;
  readonly admittedAtMs: number | null;
  readonly admissionObservedAtMs: number | null;
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
  readonly #recentAttempts: RendererSurfaceAutomaticSubmissionAdmissionAttempt[] = [];

  record(
    sourceTimeMs: number,
    outcome: RendererSurfaceAutomaticSubmissionAdmissionOutcome,
    demand: RendererSurfaceSubmissionDemandDecision,
    backend: RendererSurfaceAutomaticSubmissionBackendReadout,
  ): void {
    this.#attemptCount += 1;
    switch (outcome) {
      case 'admitted':
        this.#admittedCount += 1;
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
      backend: Object.freeze({
        mode: backend.mode,
        state: backend.state,
        rendererClass: backend.rendererClass,
        admittedAtMs: backend.admittedAtMs,
        admissionObservedAtMs: backend.admissionObservedAtMs,
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
      recentAttempts: Object.freeze([...this.#recentAttempts]),
    });
  }
}
