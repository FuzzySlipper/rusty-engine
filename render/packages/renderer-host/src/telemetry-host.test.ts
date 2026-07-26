import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  telemetryOverlayHandle,
  type PresentationFrameDiff,
  type PresentationOp,
  type TelemetryOverlayDescriptor,
  type TelemetryOverlayHandle,
} from '@rusty-engine/render-contracts';
import type { LiveTelemetrySnapshot } from './host-types.js';
import { RendererPresentationHostSet } from './presentation-host-set.js';
import {
  RendererLiveTelemetryCollector,
  RendererTelemetryOverlayHost,
  type RendererTelemetryOverlaySink,
} from './telemetry-host.js';
import { RendererSurfaceTimingTracker } from './surface-timing.js';

class FakeOverlaySink implements RendererTelemetryOverlaySink {
  readonly rendered: Array<{
    handle: TelemetryOverlayHandle;
    descriptor: TelemetryOverlayDescriptor;
    snapshot: LiveTelemetrySnapshot | null;
  }> = [];
  readonly destroyed: TelemetryOverlayHandle[] = [];

  render(
    handle: TelemetryOverlayHandle,
    descriptor: TelemetryOverlayDescriptor,
    snapshot: LiveTelemetrySnapshot | null,
  ): void {
    this.rendered.push({ handle, descriptor, snapshot });
  }

  destroy(handle: TelemetryOverlayHandle): void {
    this.destroyed.push(handle);
  }
}

function frame(
  op: Extract<PresentationOp, { readonly domain: 'telemetryOverlay' }>['op'],
): PresentationFrameDiff {
  return {
    schemaVersion: 1,
    ops: [{ domain: 'telemetryOverlay', meta: { sequence: 0 }, op }],
  };
}

function descriptor(): TelemetryOverlayDescriptor {
  return {
    title: 'RUSTY runtime',
    corner: 'topRight',
    refreshIntervalMs: 250,
    maxFrameTimeSamples: 3,
    visible: true,
  };
}

void test('headless live telemetry omits unavailable counters and preserves bounded history', () => {
  const collector = new RendererLiveTelemetryCollector({
    expectedCounters: ['entityCount', 'drawCallCount', 'renderDiffCount'],
    maxFrameTimeSamples: 2,
  });
  collector.sample({
    sourceTick: 4,
    frameTimeMs: 16,
    counters: { entityCount: 2, drawCallCount: null, renderDiffCount: 5 },
  });
  const snapshot = collector.sample({
    sourceTick: 5,
    frameTimeMs: 17,
    counters: { entityCount: 3, renderDiffCount: 7 },
  });

  assert.deepEqual(snapshot.frameTimeHistoryMs, [16, 17]);
  assert.deepEqual(snapshot.metrics.map((metric) => metric.counter), [
    'frameTimeMs',
    'entityCount',
    'renderDiffCount',
  ]);
  assert.equal(snapshot.diagnostics[0]?.code, 'counterUnavailable');
  assert.equal(snapshot.diagnostics[0]?.counter, 'drawCallCount');
  assert.deepEqual(collector.readSnapshot(), snapshot);
});

void test('telemetry overlay projects the same snapshot and local toggle changes no sample', () => {
  const collector = new RendererLiveTelemetryCollector({
    expectedCounters: ['entityCount', 'activeParticleCount'],
  });
  const sink = new FakeOverlaySink();
  const host = new RendererTelemetryOverlayHost({ collector, sink });
  const handle = telemetryOverlayHandle(1);
  const created = host.applyPresentation(frame({
    op: 'create',
    handle,
    descriptor: descriptor(),
  }));
  assert.equal(created.applied, 1);
  assert.equal(created.readout.activeOverlays, 1);

  const surfaceTiming = new RendererSurfaceTimingTracker();
  surfaceTiming.record({
    source: 'mount',
    sourceTimeMs: 0,
    backendSubmissionStartedMs: 1,
    backendSubmissionEndedMs: 2,
  });
  const timing = surfaceTiming.record({
    source: 'animationFrame',
    sourceTimeMs: 16.5,
    backendSubmissionStartedMs: 20,
    backendSubmissionEndedMs: 22.25,
  });
  const snapshot = host.sampleSurface({
    sourceTick: 8,
    timing,
    counters: { entityCount: 2, activeParticleCount: 12 },
  }, 250);
  assert.deepEqual(sink.rendered.at(-1)?.snapshot, snapshot);
  assert.deepEqual(snapshot.metrics.map((metric) => metric.counter), [
    'frameTimeMs',
    'backendSubmissionDurationMs',
    'entityCount',
    'activeParticleCount',
  ]);
  assert.deepEqual(snapshot.frameTimeHistoryMs, [16.5]);
  assert.equal(host.toggleVisible(handle), false);
  assert.deepEqual(collector.readSnapshot(), snapshot, 'toggle is projection-local');
  assert.equal(sink.rendered.at(-1)?.descriptor.visible, false);

  const updated = host.applyPresentation(frame({
    op: 'update',
    handle,
    patch: {
      title: null,
      corner: 'bottomRight',
      refreshIntervalMs: null,
      maxFrameTimeSamples: null,
      visible: true,
    },
  }));
  assert.equal(updated.applied, 1);
  assert.equal(sink.rendered.at(-1)?.descriptor.corner, 'bottomRight');
  host.applyPresentation(frame({ op: 'destroy', handle }));
  assert.deepEqual(sink.destroyed, [handle]);
});

void test('surface telemetry reports unavailable first cadence without inventing a zero', () => {
  const collector = new RendererLiveTelemetryCollector({ expectedCounters: [] });
  const timing = new RendererSurfaceTimingTracker().record({
    source: 'mount',
    sourceTimeMs: 0,
    backendSubmissionStartedMs: 4,
    backendSubmissionEndedMs: 5.5,
  });
  const snapshot = collector.sampleSurface({ sourceTick: 0, timing, counters: {} });

  assert.deepEqual(snapshot.frameTimeHistoryMs, []);
  assert.deepEqual(snapshot.metrics.map((metric) => metric.counter), [
    'backendSubmissionDurationMs',
  ]);
  assert.equal(snapshot.diagnostics[0]?.code, 'counterUnavailable');
  assert.equal(snapshot.diagnostics[0]?.counter, 'frameTimeMs');

  assert.throws(() => collector.sampleSurface({
    sourceTick: 1,
    timing: { ...timing, frameIntervalStatus: 'available' },
    counters: {},
  }), /availability status/u);
  assert.deepEqual(collector.readSnapshot(), snapshot, 'rejected timing is non-mutating');
});

void test('missing overlay realization does not block scene or other telemetry access', async () => {
  const collector = new RendererLiveTelemetryCollector({ expectedCounters: ['entityCount'] });
  const snapshot = collector.sample({
    sourceTick: 8,
    frameTimeMs: 16,
    counters: { entityCount: 2 },
  });
  const receipt = await new RendererPresentationHostSet({}).apply(frame({
    op: 'create',
    handle: telemetryOverlayHandle(2),
    descriptor: descriptor(),
  }));
  const overlay = receipt.domains.find((domain) => domain.domain === 'telemetryOverlay');
  assert.equal(overlay?.diagnostics[0]?.code, 'unavailableHost');
  assert.deepEqual(collector.readSnapshot(), snapshot);
});
