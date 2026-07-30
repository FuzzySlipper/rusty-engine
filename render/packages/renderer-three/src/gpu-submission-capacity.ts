import type { RendererGpuSubmissionClass } from './gpu-submission-duty.js';

export const ACCELERATED_AUTOMATIC_SUBMISSION_CAPACITY = 8;

/**
 * Select the bounded automatic-submission capacity.
 *
 * A timer query is itself a completion observation for the command stream it
 * encloses, so accelerated WebGL can retain a bounded measurement ring even
 * when sync fences are unavailable. A sync-fence ring remains an additional
 * completion bound when the backend exposes it. Software, unknown, and timer
 * fallback paths stay at one automatic submission.
 */
export function automaticSubmissionCapacity(
  rendererClass: RendererGpuSubmissionClass,
  timerQueriesAvailable: boolean,
): number {
  return rendererClass === 'accelerated' && timerQueriesAvailable
    ? ACCELERATED_AUTOMATIC_SUBMISSION_CAPACITY
    : 1;
}
