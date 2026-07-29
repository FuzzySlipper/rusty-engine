import type { RendererSurfaceTimingSample } from './surface-timing.js';

export const RUSTY_RENDERER_SURFACE_STATISTICS_SCHEMA_VERSION = 1;

export type RendererSurfaceStatisticScope = 'perSubmission' | 'liveResident';
export type RendererSurfaceStatisticStatus = 'available' | 'unavailable' | 'unsupported';

export type RendererSurfaceStatistic =
  | {
      readonly scope: RendererSurfaceStatisticScope;
      readonly status: 'available';
      readonly value: number;
    }
  | {
      readonly scope: RendererSurfaceStatisticScope;
      readonly status: 'unavailable' | 'unsupported';
      readonly value: null;
    };

/**
 * Renderer-neutral counters captured for one successful surface submission.
 *
 * `perSubmission` values describe only that submission. `liveResident` values
 * describe backend-owned retained state immediately after that submission.
 * No value is cumulative.
 */
export interface RendererSurfaceStatisticsSample {
  readonly schemaVersion: 1;
  readonly drawCallCount: RendererSurfaceStatistic;
  readonly renderHandleCount: RendererSurfaceStatistic;
  readonly geometryResourceCount: RendererSurfaceStatistic;
  readonly materialResourceCount: RendererSurfaceStatistic;
  readonly textureResourceCount: RendererSurfaceStatistic;
  readonly animatedInstanceCount: RendererSurfaceStatistic;
  readonly triangleCount: RendererSurfaceStatistic;
}

/** Timing plus backend-owned counters for one successful renderer submission. */
export interface RendererSurfaceSubmissionSample extends RendererSurfaceTimingSample {
  readonly statistics: RendererSurfaceStatisticsSample;
}

/**
 * Backend observation admitted at the renderer-host boundary.
 *
 * A non-negative safe integer is available, `null` is temporarily unavailable,
 * and `undefined` means the backend does not support the counter.
 */
export interface RendererSurfaceStatisticsInput {
  readonly drawCallCount: number | null | undefined;
  readonly renderHandleCount: number | null | undefined;
  readonly geometryResourceCount: number | null | undefined;
  readonly materialResourceCount: number | null | undefined;
  readonly textureResourceCount: number | null | undefined;
  readonly animatedInstanceCount: number | null | undefined;
  readonly triangleCount: number | null | undefined;
}

export function createRendererSurfaceSubmissionSample(
  timing: RendererSurfaceTimingSample,
  input: RendererSurfaceStatisticsInput,
): RendererSurfaceSubmissionSample {
  return Object.freeze({
    ...timing,
    statistics: createRendererSurfaceStatisticsSample(input),
  });
}

export function createRendererSurfaceStatisticsSample(
  input: RendererSurfaceStatisticsInput,
): RendererSurfaceStatisticsSample {
  return Object.freeze({
    schemaVersion: RUSTY_RENDERER_SURFACE_STATISTICS_SCHEMA_VERSION,
    drawCallCount: statistic('perSubmission', input.drawCallCount),
    renderHandleCount: statistic('liveResident', input.renderHandleCount),
    geometryResourceCount: statistic('liveResident', input.geometryResourceCount),
    materialResourceCount: statistic('liveResident', input.materialResourceCount),
    textureResourceCount: statistic('liveResident', input.textureResourceCount),
    animatedInstanceCount: statistic('liveResident', input.animatedInstanceCount),
    triangleCount: statistic('perSubmission', input.triangleCount),
  });
}

function statistic(
  scope: RendererSurfaceStatisticScope,
  value: number | null | undefined,
): RendererSurfaceStatistic {
  if (value === undefined) {
    return Object.freeze({ scope, status: 'unsupported', value: null });
  }
  if (value === null) {
    return Object.freeze({ scope, status: 'unavailable', value: null });
  }
  if (!Number.isSafeInteger(value) || value < 0) {
    return Object.freeze({ scope, status: 'unavailable', value: null });
  }
  return Object.freeze({ scope, status: 'available', value });
}
