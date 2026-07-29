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

const SURFACE_STATISTIC_SCOPES = {
  drawCallCount: 'perSubmission',
  renderHandleCount: 'liveResident',
  geometryResourceCount: 'liveResident',
  materialResourceCount: 'liveResident',
  textureResourceCount: 'liveResident',
  animatedInstanceCount: 'liveResident',
  triangleCount: 'perSubmission',
} as const satisfies Readonly<
  Record<keyof RendererSurfaceStatisticsInput, RendererSurfaceStatisticScope>
>;

const SURFACE_STATISTICS_KEYS = new Set<string>([
  'schemaVersion',
  ...Object.keys(SURFACE_STATISTIC_SCOPES),
]);

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

/** @internal Validates an untrusted statistics value before telemetry admission. */
export function assertRendererSurfaceStatisticsSample(
  value: unknown,
): asserts value is RendererSurfaceStatisticsSample {
  const sample = record(value, 'renderer surface statistics');
  const keys = Object.keys(sample);
  if (
    keys.length !== SURFACE_STATISTICS_KEYS.size
    || keys.some((key) => !SURFACE_STATISTICS_KEYS.has(key))
  ) {
    throw new Error('renderer surface statistics must have the complete supported shape');
  }
  if (sample['schemaVersion'] !== RUSTY_RENDERER_SURFACE_STATISTICS_SCHEMA_VERSION) {
    throw new Error('renderer surface statistics schemaVersion must be 1');
  }
  for (const [name, scope] of Object.entries(SURFACE_STATISTIC_SCOPES)) {
    assertStatistic(sample[name], scope, name);
  }
}

function assertStatistic(
  value: unknown,
  expectedScope: RendererSurfaceStatisticScope,
  name: string,
): asserts value is RendererSurfaceStatistic {
  const statistic = record(value, `renderer surface statistic ${name}`);
  const keys = Object.keys(statistic);
  if (
    keys.length !== 3
    || !keys.includes('scope')
    || !keys.includes('status')
    || !keys.includes('value')
  ) {
    throw new Error(`renderer surface statistic ${name} must have scope, status, and value`);
  }
  if (statistic['scope'] !== expectedScope) {
    throw new Error(`renderer surface statistic ${name} must use ${expectedScope} scope`);
  }
  if (statistic['status'] === 'available') {
    if (!Number.isSafeInteger(statistic['value']) || (statistic['value'] as number) < 0) {
      throw new Error(
        `renderer surface statistic ${name} available value must be a non-negative safe integer`,
      );
    }
    return;
  }
  if (statistic['status'] !== 'unavailable' && statistic['status'] !== 'unsupported') {
    throw new Error(`renderer surface statistic ${name} status is unsupported`);
  }
  if (statistic['value'] !== null) {
    throw new Error(
      `renderer surface statistic ${name} ${String(statistic['status'])} value must be null`,
    );
  }
}

function record(value: unknown, name: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error(`${name} must be an object`);
  }
  return value as Record<string, unknown>;
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
