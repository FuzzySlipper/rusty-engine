import { mountRendererSurface } from '@rusty-engine/renderer-host';
import { renderHandle, type RenderFrameDiff, type MeshPayloadDescriptor } from '@rusty-engine/render-contracts';

// Version the authored workload whenever geometry, camera, lighting or sampling changes.
const WorkloadVersion = 2;
const WarmupFrames = 30;
const MeasuredFrames = 120;
const Repeats = 3;
const Workloads = [{ id: 'retained-cubes-256', cubes: 256, grid: 0 },
  { id: 'relief-grid-64', cubes: 0, grid: 64 },
  { id: 'relief-grid-256', cubes: 0, grid: 256 }];
const records: Array<ReturnType<typeof summarize> & { lane: string; workload: { id: string } }> = [];
const status = document.querySelector<HTMLPreElement>('#status')!;
const canvas = document.querySelector<HTMLCanvasElement>('#renderer')!;
declare global { interface Window { __rustyPerformanceBaseline?: Promise<readonly unknown[]>; } }
window.__rustyPerformanceBaseline = Promise.resolve().then(run).catch(async (error: unknown) => {
  status.textContent = String(error);
  await fetch('/performance-results', { method: 'POST', body: JSON.stringify({ error: String(error) }) });
  throw error;
});

async function run(): Promise<readonly unknown[]> {
  const browser = { userAgent: navigator.userAgent, hardwareConcurrency: navigator.hardwareConcurrency,
    secureContext: isSecureContext, crossOriginIsolated };
  const clockResolutionMs = clockResolution();
  for (let run = 0; run < Repeats; run++) {
    for (const workload of Workloads) {
      status.textContent = `Run ${run + 1}/${Repeats}: ${workload.id}`;
      let cameraStep = 0;
      const surface = mountRendererSurface(canvas, { autoStart: false, pixelRatio: 1,
        frame: { schemaVersion: 1, ops: [] },
        onAnimationFrame: () => {
          // Feed demand before Engine admission, independently of measured progress.
          const phase = cameraStep++ % 60;
          surface.setCameraPose({ position: [0, 0, 0], yawDegrees: (phase <= 30 ? phase : 60 - phase) * 0.1 - 1.5, pitchDegrees: 0 });
        },
      });
      try {
        const frame = scene(workload);
        const start = performance.now();
        const receipt = surface.applyFrame(frame);
        if (receipt.diagnostics.length) throw new Error(JSON.stringify(receipt.diagnostics));
        const realizationMs = performance.now() - start;
        surface.setCameraPose({ position: [0, 0, 0], yawDegrees: 0, pitchDegrees: 0 });
        surface.start();
        const cpu: number[] = [];
        const intervals: number[] = [];
        const gpu: number[] = [];
        const observation: number[] = [];
        let sequence = -1;
        let frameCount = 0;
        let previousFrame: number | null = null;
        let timerObservedAt: number | null = null;
        const deadline = performance.now() + 120_000;
        while (cpu.length < MeasuredFrames) {
          await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
          if (document.visibilityState !== 'visible') throw new Error('Performance page became hidden');
          if (performance.now() > deadline) throw new Error(`Timed out measuring ${workload.id}: ${frameCount} frames in 120 seconds`);
          const readStarted = performance.now();
          const d = surface.diagnosticsReadout();
          const readDuration = performance.now() - readStarted;
          if (d.cadence.state !== 'ready' || d.cadence.failures.length) throw new Error(JSON.stringify(d.cadence));
          if (d.submission.renderSequence === sequence) continue;
          sequence = d.submission.renderSequence;
          if (sequence === 0) continue;
          frameCount++;
          if (frameCount % 30 === 0) status.textContent = `Run ${run + 1}/${Repeats}: ${workload.id}, frame ${frameCount}`;
          if (frameCount <= WarmupFrames) continue;
          const duration = d.submission.backendSubmissionDurationMs;
          if (duration === null) throw new Error('CPU submission duration unavailable');
          cpu.push(duration);
          observation.push(readDuration);
          const sourceTime = d.submission.sourceTimeMs;
          if (sourceTime !== null && previousFrame !== null) intervals.push(sourceTime - previousFrame);
          previousFrame = sourceTime;
          const pacing = d.pacing;
          if (pacing.mode === 'timerQuery' && pacing.timerDurationMs !== null
            && pacing.observedAtMs !== timerObservedAt) {
            gpu.push(pacing.timerDurationMs);
            timerObservedAt = pacing.observedAtMs;
          }
        }
        surface.stop();
        const diagnostics = surface.diagnosticsReadout();
        const stats = diagnostics.submission.statistics;
        if (stats.drawCallCount.status !== 'available' || stats.drawCallCount.value <= 0) {
          throw new Error('Loaded workload produced no measured draws');
        }
        const common = { schemaVersion: 1, run,
          workload: { ...workload, version: WorkloadVersion, warmupFrames: WarmupFrames, measuredFrames: MeasuredFrames,
            submissionMode: 'automatic-callback-camera-demand' },
          renderer: diagnostics.renderer, vendor: diagnostics.vendor, browser, clockResolutionMs,
          canvas: diagnostics.canvas, rendererClass: diagnostics.pacing.rendererClass,
          realizationMs, statistics: stats, gpuTimingAvailable: gpu.length > 0,
          pacing: { mode: diagnostics.pacing.mode, pending: diagnostics.pacing.pendingSubmissionCount } };
        for (const [lane, samples] of [['renderer-cpu-submission', cpu], ['renderer-frame-interval', intervals],
          ['renderer-gpu-timer', gpu], ['renderer-diagnostics-read', observation], ['renderer-apply-frame', [realizationMs]]] as const) {
          if (!samples.length) continue;
          const record = { ...common, lane, ...summarize(samples) };
          records.push(record);
          console.log(`RUSTY_PERF ${JSON.stringify(record)}`);
        }
      } finally { surface.dispose(); }
    }
  }
  const response = await fetch('/performance-results', { method: 'POST', body: JSON.stringify({ records }) });
  if (!response.ok) throw new Error(`Result capture failed: ${response.status}`);
  status.textContent = `Complete: ${records.length} measurements saved.\n${records.map((r) => `${r.workload.id} ${r.lane}: median ${r.median.toFixed(3)} ms, p95 ${r.p95.toFixed(3)} ms`).join('\n')}`;
  return records;
}

function summarize(samples: readonly number[]) {
  const sorted = [...samples].sort((a, b) => a - b);
  const percentile = (p: number) => sorted[Math.round((sorted.length - 1) * p)]!;
  return { iterations: samples.length, unit: 'milliseconds', samples, minimum: sorted[0]!,
    median: percentile(0.5), p95: percentile(0.95), maximum: sorted.at(-1)!,
    mean: samples.reduce((a, b) => a + b, 0) / samples.length };
}
const metadata = { sourceEntity: null, sourceSceneNode: null, tags: [], label: 'performance-fixture' };
const transform = (translation: readonly [number, number, number], scale: readonly [number, number, number]) =>
  ({ translation, scale, rotation: [0, 0, 0, 1] as const });
function scene(workload: typeof Workloads[number]): RenderFrameDiff {
  if (workload.cubes) return { schemaVersion: 1, ops: Array.from({ length: workload.cubes }, (_, i) => ({
    op: 'create', handle: renderHandle(i + 1), parent: null, node: {
      geometry: { kind: 'cube' }, material: { color: [0.35 + (i % 3) * 0.1, 0.55, 0.7, 1], wireframe: false },
      transform: transform([(i % 16 - 7.5) * 0.5, (Math.floor(i / 16) - 7.5) * 0.5, -9], [0.4, 0.4, 0.4]),
      visible: true, layer: 'scene', metadata,
    },
  })) };
  const payload = relief(workload.grid);
  return { schemaVersion: 1, ops: [
    { op: 'defineMaterial', material: { schemaVersion: 3, id: 'perf/stone', color: [0.5, 0.6, 0.7, 1], texture: null, roughness: 1, textureTint: [1, 1, 1, 1], emissionColor: [0, 0, 0], emissionIntensity: 0, uvStrategy: 'planar' } },
    { op: 'defineStaticMesh', asset: { asset: 'perf/relief', payload, materialSlots: [{slot: 0, material: 'perf/stone'}], collision: { kind: 'visualOnly' } } },
    { op: 'createStaticMeshInstance', handle: renderHandle(1), parent: null,
      instance: { asset: 'perf/relief', transform: transform([0, 0, -9], [1, 1, 1]), visible: true, materialOverrides: [], metadata } },
  ] };
}
function relief(n: number): MeshPayloadDescriptor {
  const positions: number[] = [], normals: number[] = [], uvs: number[] = [], indices: number[] = [];
  for (let y = 0; y <= n; y++) for (let x = 0; x <= n; x++) {
    const px = (x / n - 0.5) * 9, py = (y / n - 0.5) * 7;
    const z = 0.3 * Math.sin(px * 4) * Math.cos(py * 3);
    const dx = 1.2 * Math.cos(px * 4) * Math.cos(py * 3), dy = -0.9 * Math.sin(px * 4) * Math.sin(py * 3);
    const length = Math.hypot(dx, dy, 1);
    positions.push(px, py, z); normals.push(-dx / length, -dy / length, 1 / length); uvs.push(x / n, y / n);
  }
  for (let y = 0; y < n; y++) for (let x = 0; x < n; x++) {
    const a = y * (n + 1) + x, b = a + 1, c = a + n + 1, d = c + 1;
    indices.push(a, b, d, a, d, c);
  }
  return { layout: { vertexCount: positions.length / 3, indexCount: indices.length, indexWidth: 'u32',
    attributes: [{ name: 'position', components: 3, kind: 'f32' }, { name: 'normal', components: 3, kind: 'f32' },
      { name: 'uv', components: 2, kind: 'f32' }] }, groups: [{ materialSlot: 0, start: 0, count: indices.length }],
    bounds: { min: [-4.5, -3.5, -0.3], max: [4.5, 3.5, 0.3] },
    source: { kind: 'inline', positions, normals, uvs, indices }, provenance: 'staticAsset' };
}

// Calibrate once outside warmup/measurement. A zero sample below this observed
// clock quantum means unresolved duration, never free work.
function clockResolution(): number {
  const deadline = performance.now() + 20;
  let previous = performance.now(), minimum = Infinity;
  while (performance.now() < deadline) {
    const now = performance.now();
    if (now > previous) minimum = Math.min(minimum, now - previous);
    previous = now;
  }
  return Number.isFinite(minimum) ? minimum : 0;
}
