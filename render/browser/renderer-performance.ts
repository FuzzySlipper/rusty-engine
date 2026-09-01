import {
  mountRendererSurface,
  type RendererSurfaceAutomaticSubmissionPacingSample,
  type RendererSurfaceSubmissionSample,
} from '@rusty-engine/renderer-host';

const WarmupSubmissions = 20;
const MeasuredSubmissions = 200;
const FixedFrameIntervalMilliseconds = 1_000 / 60;

export interface RendererPerformanceProbeResult {
  readonly schemaVersion: 1;
  readonly lane: 'browser-renderer-submission';
  readonly iterations: number;
  readonly unit: 'milliseconds';
  readonly minimum: number;
  readonly median: number;
  readonly p95: number;
  readonly maximum: number;
  readonly mean: number;
  readonly renderer: string | null;
  readonly vendor: string | null;
  readonly canvas: {
    readonly cssWidth: number;
    readonly cssHeight: number;
    readonly backingWidth: number;
    readonly backingHeight: number;
  };
  readonly submission: RendererSurfaceSubmissionSample;
  readonly pacing: RendererSurfaceAutomaticSubmissionPacingSample;
}

declare global {
  interface Window {
    __rustyRendererPerformance?: Promise<RendererPerformanceProbeResult>;
  }
}

const canvas = document.querySelector<HTMLCanvasElement>('#renderer');
if (canvas === null) throw new Error('renderer performance canvas is missing');
const surface = mountRendererSurface(canvas, { autoStart: false, pixelRatio: 1 });

window.__rustyRendererPerformance = Promise.resolve().then(() => {
  for (let index = 0; index < WarmupSubmissions; index += 1) {
    surface.renderOnce(index * FixedFrameIntervalMilliseconds);
  }
  const durations: number[] = [];
  for (let index = 0; index < MeasuredSubmissions; index += 1) {
    const sample = surface.renderOnce(
      (index + WarmupSubmissions) * FixedFrameIntervalMilliseconds,
    );
    if (sample.backendSubmissionDurationMs !== null) {
      durations.push(sample.backendSubmissionDurationMs);
    }
  }
  if (durations.length !== MeasuredSubmissions) {
    throw new Error('renderer submission clock was unavailable during the performance probe');
  }
  durations.sort((left, right) => left - right);
  const gl = canvas.getContext('webgl2');
  const extension = gl?.getExtension('WEBGL_debug_renderer_info') ?? null;
  const renderer = gl === null || extension === null
    ? null
    : String(gl.getParameter(extension.UNMASKED_RENDERER_WEBGL));
  const vendor = gl === null || extension === null
    ? null
    : String(gl.getParameter(extension.UNMASKED_VENDOR_WEBGL));
  const result = Object.freeze({
    schemaVersion: 1 as const,
    lane: 'browser-renderer-submission' as const,
    iterations: durations.length,
    unit: 'milliseconds' as const,
    minimum: durations[0]!,
    median: percentile(durations, 0.5),
    p95: percentile(durations, 0.95),
    maximum: durations.at(-1)!,
    mean: durations.reduce((sum, duration) => sum + duration, 0) / durations.length,
    renderer,
    vendor,
    canvas: Object.freeze({
      cssWidth: canvas.clientWidth,
      cssHeight: canvas.clientHeight,
      backingWidth: canvas.width,
      backingHeight: canvas.height,
    }),
    submission: surface.submission(),
    pacing: surface.automaticSubmissionPacing(),
  });
  surface.dispose();
  return result;
});

function percentile(sorted: readonly number[], fraction: number): number {
  return sorted[Math.round((sorted.length - 1) * fraction)]!;
}
