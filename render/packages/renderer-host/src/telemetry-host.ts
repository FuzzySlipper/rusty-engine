import type {
  PresentationFrameDiff,
  PresentationOp,
  TelemetryOverlayDescriptor,
  TelemetryOverlayHandle,
  TelemetryOverlayPatch,
} from '@rusty-engine/render-contracts';
import type {
  LiveTelemetryCounter,
  LiveTelemetryDiagnostic,
  LiveTelemetryMetric,
  LiveTelemetrySnapshot,
  TelemetryOverlayDiagnostic,
  TelemetryOverlayReadout,
} from './host-types.js';
import {
  RUSTY_RENDERER_SURFACE_MAX_TIMING_DURATION_MS,
  type RendererSurfaceFrameIntervalStatus,
  type RendererSurfaceSubmissionDurationStatus,
  type RendererSurfaceTimingSample,
  type RendererSurfaceTimingSource,
} from './surface-timing.js';

type DurationCounter = 'frameTimeMs' | 'backendSubmissionDurationMs';
type CountCounter = Exclude<LiveTelemetryCounter, DurationCounter>;
type TelemetryPresentationOp = Extract<
  PresentationOp,
  { readonly domain: 'telemetryOverlay' }
>;

const COUNTER_ORDER: readonly CountCounter[] = [
  'entityCount',
  'activeCapabilityCount',
  'residentChunkCount',
  'dirtyChunkCount',
  'renderDiffCount',
  'renderHandleCount',
  'drawCallCount',
  'activeAudioSourceCount',
  'activeBillboardCount',
  'activeParticleCount',
  'droppedFeedbackCount',
];

const SURFACE_TIMING_SOURCES: readonly RendererSurfaceTimingSource[] = [
  'mount',
  'animationFrame',
  'explicit',
  'cameraReset',
];
const FRAME_INTERVAL_STATUSES: readonly RendererSurfaceFrameIntervalStatus[] = [
  'available',
  'firstFrame',
  'sourceTimeRegressed',
  'sourceTimeGapExceeded',
];
const SUBMISSION_DURATION_STATUSES: readonly RendererSurfaceSubmissionDurationStatus[] = [
  'available',
  'clockUnavailable',
  'clockRegressed',
  'durationExceeded',
];

export interface RendererLiveTelemetryCollectorOptions {
  readonly expectedCounters: readonly CountCounter[];
  readonly maxFrameTimeSamples?: number;
}

export interface RendererLiveTelemetrySample {
  readonly sourceTick: number;
  /** Inter-frame cadence in milliseconds, not backend CPU/GPU work duration. */
  readonly frameTimeMs: number;
  readonly counters: Readonly<Partial<Record<CountCounter, number | null | undefined>>>;
}

export interface RendererSurfaceTelemetrySample {
  readonly sourceTick: number;
  readonly timing: RendererSurfaceTimingSample;
  readonly counters: Readonly<Partial<Record<CountCounter, number | null | undefined>>>;
}

interface DurationObservation {
  readonly counter: DurationCounter;
  readonly value: number | null;
  readonly unavailableMessage: string | null;
}

interface ResolvedTelemetrySample {
  readonly sourceTick: number;
  readonly durations: readonly DurationObservation[];
  readonly counters: Readonly<Partial<Record<CountCounter, number | null | undefined>>>;
}

export class RendererLiveTelemetryCollector {
  readonly #expectedCounters: ReadonlySet<CountCounter>;
  readonly #maxFrameTimeSamples: number;
  readonly #frameTimeHistory: number[] = [];
  #sampleSequence = 0;
  #snapshot: LiveTelemetrySnapshot | null = null;

  constructor(options: RendererLiveTelemetryCollectorOptions) {
    this.#expectedCounters = new Set(options.expectedCounters);
    this.#maxFrameTimeSamples = boundedInteger(
      options.maxFrameTimeSamples ?? 60,
      1,
      240,
      'maxFrameTimeSamples',
    );
  }

  sample(input: RendererLiveTelemetrySample): LiveTelemetrySnapshot {
    return this.#record({
      sourceTick: input.sourceTick,
      durations: [{ counter: 'frameTimeMs', value: input.frameTimeMs, unavailableMessage: null }],
      counters: input.counters,
    });
  }

  sampleSurface(input: RendererSurfaceTelemetrySample): LiveTelemetrySnapshot {
    validateSurfaceTiming(input.timing);
    return this.#record({
      sourceTick: input.sourceTick,
      durations: [
        durationObservation(
          'frameTimeMs',
          input.timing.frameIntervalMs,
          input.timing.frameIntervalStatus,
        ),
        durationObservation(
          'backendSubmissionDurationMs',
          input.timing.backendSubmissionDurationMs,
          input.timing.backendSubmissionDurationStatus,
        ),
      ],
      counters: input.counters,
    });
  }

  #record(input: ResolvedTelemetrySample): LiveTelemetrySnapshot {
    if (!Number.isSafeInteger(input.sourceTick) || input.sourceTick < 0) {
      throw new Error('sourceTick must be a non-negative safe integer');
    }
    const diagnostics: LiveTelemetryDiagnostic[] = [];
    const metrics: LiveTelemetryMetric[] = [];
    for (const duration of input.durations) {
      if (duration.value === null) {
        diagnostics.push({
          code: 'counterUnavailable',
          counter: duration.counter,
          message: duration.unavailableMessage ?? `${duration.counter} is unavailable`,
        });
        continue;
      }
      if (!validMetric(duration.value)) {
        diagnostics.push({
          code: 'invalidSample',
          counter: duration.counter,
          message: `${duration.counter} must be finite and non-negative`,
        });
        continue;
      }
      if (duration.counter === 'frameTimeMs') {
        this.#frameTimeHistory.push(duration.value);
        if (this.#frameTimeHistory.length > this.#maxFrameTimeSamples) {
          this.#frameTimeHistory.splice(
            0,
            this.#frameTimeHistory.length - this.#maxFrameTimeSamples,
          );
        }
      }
      metrics.push(metric(duration.counter, duration.value, 'durationMs', 'ms'));
    }
    for (const counter of COUNTER_ORDER) {
      const value = input.counters[counter];
      if (value === null || value === undefined) {
        if (this.#expectedCounters.has(counter)) {
          diagnostics.push({
            code: 'counterUnavailable',
            counter,
            message: `${counter} is unavailable from the current owner adapter`,
          });
        }
        continue;
      }
      if (!validMetric(value)) {
        diagnostics.push({
          code: 'invalidSample',
          counter,
          message: `${counter} must be finite and non-negative`,
        });
        continue;
      }
      metrics.push(metric(counter, value, 'gauge', 'count'));
    }
    this.#sampleSequence += 1;
    this.#snapshot = {
      schemaVersion: 1,
      sourceTick: input.sourceTick,
      sampleSequence: this.#sampleSequence,
      metrics,
      frameTimeHistoryMs: [...this.#frameTimeHistory],
      diagnostics,
    };
    return this.readSnapshot();
  }

  readSnapshot(): LiveTelemetrySnapshot {
    if (this.#snapshot === null) {
      throw new Error('live telemetry has not sampled any owner counters');
    }
    return {
      ...this.#snapshot,
      metrics: [...this.#snapshot.metrics],
      frameTimeHistoryMs: [...this.#snapshot.frameTimeHistoryMs],
      diagnostics: [...this.#snapshot.diagnostics],
    };
  }

  tryReadSnapshot(): LiveTelemetrySnapshot | null {
    return this.#snapshot === null ? null : this.readSnapshot();
  }
}

export interface RendererTelemetryOverlaySink {
  render(
    handle: TelemetryOverlayHandle,
    descriptor: TelemetryOverlayDescriptor,
    snapshot: LiveTelemetrySnapshot | null,
  ): void;
  destroy(handle: TelemetryOverlayHandle): void;
}

export interface RendererTelemetryOverlayHostOptions {
  readonly collector: RendererLiveTelemetryCollector;
  readonly sink: RendererTelemetryOverlaySink;
}

export interface RendererTelemetryOverlayFrameReceipt {
  readonly applied: number;
  readonly diagnostics: readonly TelemetryOverlayDiagnostic[];
  readonly readout: TelemetryOverlayReadout;
}

interface ActiveOverlay {
  descriptor: TelemetryOverlayDescriptor;
  lastRenderedMs: number | null;
}

export class RendererTelemetryOverlayHost {
  readonly #collector: RendererLiveTelemetryCollector;
  readonly #sink: RendererTelemetryOverlaySink;
  readonly #active = new Map<number, ActiveOverlay>();
  readonly #diagnostics: TelemetryOverlayDiagnostic[] = [];
  #renderedSnapshots = 0;

  constructor(options: RendererTelemetryOverlayHostOptions) {
    this.#collector = options.collector;
    this.#sink = options.sink;
  }

  applyPresentation(frame: PresentationFrameDiff): RendererTelemetryOverlayFrameReceipt {
    const diagnostics: TelemetryOverlayDiagnostic[] = [];
    let applied = 0;
    for (const operation of frame.ops) {
      if (operation.domain !== 'telemetryOverlay') {
        continue;
      }
      const diagnostic = this.#applyOperation(operation);
      if (diagnostic === null) {
        applied += 1;
      } else {
        diagnostics.push(diagnostic);
        this.#diagnostics.push(diagnostic);
      }
    }
    return { applied, diagnostics, readout: this.readout() };
  }

  sample(input: RendererLiveTelemetrySample, elapsedMs: number): LiveTelemetrySnapshot {
    if (!Number.isFinite(elapsedMs) || elapsedMs < 0) {
      throw new Error('elapsedMs must be finite and non-negative');
    }
    return this.#renderSnapshot(this.#collector.sample(input), elapsedMs);
  }

  sampleSurface(input: RendererSurfaceTelemetrySample, elapsedMs: number): LiveTelemetrySnapshot {
    if (!Number.isFinite(elapsedMs) || elapsedMs < 0) {
      throw new Error('elapsedMs must be finite and non-negative');
    }
    return this.#renderSnapshot(this.#collector.sampleSurface(input), elapsedMs);
  }

  #renderSnapshot(snapshot: LiveTelemetrySnapshot, elapsedMs: number): LiveTelemetrySnapshot {
    for (const [rawHandle, overlay] of this.#active) {
      if (!overlay.descriptor.visible) {
        continue;
      }
      if (
        overlay.lastRenderedMs === null
        || elapsedMs - overlay.lastRenderedMs >= overlay.descriptor.refreshIntervalMs
      ) {
        this.#sink.render(rawHandle as TelemetryOverlayHandle, overlay.descriptor, snapshot);
        overlay.lastRenderedMs = elapsedMs;
        this.#renderedSnapshots += 1;
      }
    }
    return snapshot;
  }

  setVisible(handle: TelemetryOverlayHandle, visible: boolean): boolean {
    const overlay = this.#active.get(handle as number);
    if (overlay === undefined) {
      return false;
    }
    overlay.descriptor = { ...overlay.descriptor, visible };
    overlay.lastRenderedMs = null;
    this.#sink.render(handle, overlay.descriptor, this.#collector.tryReadSnapshot());
    return true;
  }

  toggleVisible(handle: TelemetryOverlayHandle): boolean | null {
    const overlay = this.#active.get(handle as number);
    if (overlay === undefined) {
      return null;
    }
    const visible = !overlay.descriptor.visible;
    this.setVisible(handle, visible);
    return visible;
  }

  readout(): TelemetryOverlayReadout {
    return {
      activeOverlays: this.#active.size,
      renderedSnapshots: this.#renderedSnapshots,
      diagnostics: [...this.#diagnostics],
    };
  }

  cleanup(): void {
    for (const rawHandle of this.#active.keys()) {
      this.#sink.destroy(rawHandle as TelemetryOverlayHandle);
    }
    this.#active.clear();
  }

  #applyOperation(operation: TelemetryPresentationOp): TelemetryOverlayDiagnostic | null {
    const rawHandle = operation.op.handle as number;
    try {
      if (operation.op.op === 'create') {
        if (this.#active.has(rawHandle)) {
          return diagnostic(operation, 'duplicateHandle', 'overlay handle is already active');
        }
        this.#active.set(rawHandle, {
          descriptor: operation.op.descriptor,
          lastRenderedMs: null,
        });
        this.#sink.render(
          operation.op.handle,
          operation.op.descriptor,
          this.#collector.tryReadSnapshot(),
        );
        return null;
      }
      const active = this.#active.get(rawHandle);
      if (active === undefined) {
        return diagnostic(operation, 'unknownHandle', 'overlay handle is not active');
      }
      if (operation.op.op === 'update') {
        active.descriptor = applyPatch(active.descriptor, operation.op.patch);
        active.lastRenderedMs = null;
        this.#sink.render(
          operation.op.handle,
          active.descriptor,
          this.#collector.tryReadSnapshot(),
        );
      } else {
        this.#active.delete(rawHandle);
        this.#sink.destroy(operation.op.handle);
      }
      return null;
    } catch (error) {
      return diagnostic(
        operation,
        'hostFailure',
        error instanceof Error ? error.message : String(error),
      );
    }
  }
}

function durationObservation(
  counter: DurationCounter,
  value: number | null,
  status: string,
): DurationObservation {
  return {
    counter,
    value,
    unavailableMessage: value === null
      ? `${counter} is unavailable because renderer surface timing status is ${status}`
      : null,
  };
}

function validateSurfaceTiming(timing: RendererSurfaceTimingSample): void {
  if (timing.schemaVersion !== 1) {
    throw new Error('renderer surface timing schemaVersion must be 1');
  }
  if (!Number.isSafeInteger(timing.renderSequence) || timing.renderSequence < 1) {
    throw new Error('renderer surface timing renderSequence must be a positive safe integer');
  }
  if (!SURFACE_TIMING_SOURCES.includes(timing.source)) {
    throw new Error('renderer surface timing source is unsupported');
  }
  if (
    !Number.isFinite(timing.sourceTimeMs)
    || timing.sourceTimeMs < 0
    || timing.sourceTimeMs > Number.MAX_SAFE_INTEGER
  ) {
    throw new Error('renderer surface timing sourceTimeMs is outside the supported range');
  }
  if (!FRAME_INTERVAL_STATUSES.includes(timing.frameIntervalStatus)) {
    throw new Error('renderer surface frameIntervalStatus is unsupported');
  }
  if (!SUBMISSION_DURATION_STATUSES.includes(timing.backendSubmissionDurationStatus)) {
    throw new Error('renderer surface backendSubmissionDurationStatus is unsupported');
  }
  validateTimingMetric(
    timing.frameIntervalMs,
    timing.frameIntervalStatus === 'available',
    'frameIntervalMs',
  );
  validateTimingMetric(
    timing.backendSubmissionDurationMs,
    timing.backendSubmissionDurationStatus === 'available',
    'backendSubmissionDurationMs',
  );
}

function validateTimingMetric(value: number | null, available: boolean, name: string): void {
  if (
    available !== (value !== null)
    || (
      value !== null
      && (!validMetric(value) || value > RUSTY_RENDERER_SURFACE_MAX_TIMING_DURATION_MS)
    )
  ) {
    throw new Error(`renderer surface timing ${name} does not match its availability status`);
  }
}

function metric(
  counter: LiveTelemetryCounter,
  value: number,
  kind: LiveTelemetryMetric['kind'],
  unit: string,
): LiveTelemetryMetric {
  return { counter, kind, value, unit };
}

function validMetric(value: number): boolean {
  return Number.isFinite(value) && value >= 0;
}

function boundedInteger(value: number, min: number, max: number, name: string): number {
  if (!Number.isInteger(value) || value < min || value > max) {
    throw new Error(`${name} must be an integer between ${min} and ${max}`);
  }
  return value;
}

function applyPatch(
  descriptor: TelemetryOverlayDescriptor,
  patch: TelemetryOverlayPatch,
): TelemetryOverlayDescriptor {
  return {
    title: patch.title ?? descriptor.title,
    corner: patch.corner ?? descriptor.corner,
    refreshIntervalMs: patch.refreshIntervalMs ?? descriptor.refreshIntervalMs,
    maxFrameTimeSamples: patch.maxFrameTimeSamples ?? descriptor.maxFrameTimeSamples,
    visible: patch.visible ?? descriptor.visible,
  };
}

function diagnostic(
  operation: TelemetryPresentationOp,
  code: TelemetryOverlayDiagnostic['code'],
  message: string,
): TelemetryOverlayDiagnostic {
  return {
    code,
    sequence: operation.meta.sequence,
    handle: operation.op.handle,
    message,
  };
}
