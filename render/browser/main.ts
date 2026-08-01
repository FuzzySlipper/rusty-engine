/// <reference types="vite/client" />

import {
  animationProjectionHandle,
  billboardHandle,
  renderHandle,
  telemetryOverlayHandle,
  type MeshPayloadDescriptor,
  type PresentationFrameDiff,
  type RenderFrameDiff,
} from '@rusty-engine/render-contracts';
import {
  RendererAnimationHost,
  RendererAudioHost,
  RendererBillboardHost,
  RendererDomParticleBillboardSink,
  RendererDomTelemetryOverlaySink,
  RendererLiveTelemetryCollector,
  RendererParticleHost,
  RendererPresentationHostSet,
  RendererTelemetryOverlayHost,
  captureRendererAnimatedMesh,
  mountRendererAnimatedMeshSurface,
  mountRendererInspectionSurface,
  mountRendererSurface,
  type RendererSurfaceAutomaticSubmissionPacingSample,
  type RendererSurfaceStatisticsSample,
} from '@rusty-engine/renderer-host';

import characterUrl from '../../fixtures/render/assets/kenney-retro-character/character-medium.glb?url';

interface BrowserProof {
  readonly animatedCapture: {
    readonly asset: string;
    readonly contactSheetPng: boolean;
    readonly contentHash: string | null;
    readonly diagnostics: readonly (readonly string[])[];
    readonly imageCount: number;
    readonly individualPngs: boolean;
    readonly normalizedTimes: readonly number[];
    readonly providerRevision: string;
    readonly statisticsAvailable: readonly boolean[];
    readonly worldBoundsPresent: readonly boolean[];
  };
  readonly animationClip: string | null;
  readonly audioApplied: number;
  audioResumeDiagnostics: readonly string[] | null;
  readonly automaticSubmissionPacing: RendererSurfaceAutomaticSubmissionPacingSample;
  readonly automaticSubmissionPacingSamples:
    readonly RendererSurfaceAutomaticSubmissionPacingSample[];
  readonly automaticSubmissionIntervalsMs: readonly (number | null)[];
  readonly automaticSubmissionSourceTimesMs: readonly number[];
  readonly autoFrameIntervalMs: number | null;
  readonly autoStartRenderCount: number;
  readonly backendSubmissionDurationMs: number | null;
  readonly batchedStaticPickHandle: number | null;
  readonly batchedStaticFarStatistics: RendererSurfaceStatisticsSample;
  readonly batchedStaticRecreateStatistics: RendererSurfaceStatisticsSample;
  readonly batchedStaticResetStatistics: RendererSurfaceStatisticsSample;
  readonly batchedStaticStatistics: RendererSurfaceStatisticsSample;
  readonly batchedStaticDisposed: boolean;
  readonly billboardText: string | null;
  readonly context: string;
  readonly explicitFrameIntervalMs: number | null;
  readonly hostSurfaceKind: string;
  readonly inspectionGridLines: number | null;
  readonly inspectionRendererStatistics: RendererSurfaceStatisticsSample;
  readonly inspectionSurfaceKind: string;
  readonly lightCount: number;
  readonly particleElementCount: number;
  readonly pickHandle: number | null;
  readonly presentationDiagnostics: readonly string[];
  readonly projectionInsideViewport: boolean;
  readonly ready: true;
  readonly rendererStatistics: RendererSurfaceStatisticsSample;
  readonly rendererBufferPixelRatio: readonly [number, number];
  readonly replacementDisposedWithHistoricalSample: boolean;
  readonly replacementDisposedRenderRejected: boolean;
  readonly replacementRenderSequence: number;
  readonly replacementStatistics: RendererSurfaceStatisticsSample;
  readonly resetRendererStatistics: RendererSurfaceStatisticsSample;
  readonly snapshot: string;
  readonly staticMeshRecreateApplied: boolean;
  readonly staticMeshRecreateDisposed: boolean;
  readonly staticMeshRecreateSnapshot: string;
  readonly staticMeshRecreateStatistics: RendererSurfaceStatisticsSample;
  readonly staticDemandApplied: boolean;
  readonly staticDemandCameraPosition: readonly [number, number, number];
  readonly staticDemandCameraRenderCount: number;
  readonly staticDemandDirtyRenderCount: number;
  readonly staticDemandIdleRenderCount: number;
  readonly staticDemandRejectedApplied: boolean;
  readonly staticDemandRejectedRenderCount: number;
  readonly telemetryText: string | null;
  readonly viewmodelAnimationClip: string | null;
  readonly viewmodelNodeCount: number;
  readonly viewmodelPickExcluded: boolean;
  readonly voxelFrame: number | null;
  readonly voxelFrameSwapApplied: boolean;
}

declare global {
  interface Window {
    __rustyRenderDispose?: () => Promise<void>;
    __rustyRenderFailure?: string;
    __rustyRenderProof?: BrowserProof;
    __rustyRenderCameraPose?: () => readonly [number, number, number];
    __rustyRenderBackendSnapshot?: () => string;
    __rustyRenderStartAudio?: () => Promise<void>;
    __rustyRenderSetCameraPose?: (position: readonly [number, number, number]) => void;
    __rustyRenderTick?: (timeMs: number) => void;
    __rustyRenderViewmodelState?: () => readonly string[];
  }
}

const ASSET = 'mesh-animation/kenney-retro-character-medium';
const CONTENT_HASH = 'sha256:c71255a41c0373f0d2ef52593369d5fd9d2f6220ae548aff8cd6bf5edb403674';

async function main(): Promise<void> {
  const canvas = required<HTMLCanvasElement>('#renderer');
  const inspectionCanvas = required<HTMLCanvasElement>('#inspection');
  const overlays = required<HTMLElement>('#overlays');
  const audioButton = required<HTMLButtonElement>('#enable-audio');
  const spriteBytes = new TextEncoder().encode(
    '<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><circle cx="4" cy="4" r="4" fill="#ffd54f"/></svg>',
  );
  const spriteHash = await sha256(spriteBytes.buffer);
  const spriteUrl = URL.createObjectURL(new Blob([spriteBytes], { type: 'image/svg+xml' }));
  const audioBytes = waveFixture();
  const audioHash = await sha256(audioBytes);

  const surface = await mountRendererAnimatedMeshSurface(canvas, {
    animatedMeshManifest: {
      kind: 'rusty_renderer_animated_mesh_resources.v1',
      resources: [{
        asset: ASSET,
        contentHash: CONTENT_HASH,
        clipIds: ['idle', 'run', 'jump'],
      }],
    },
    resolveAnimatedMeshResource: async () => {
      const response = await fetch(characterUrl);
      if (!response.ok) throw new Error(`animated fixture fetch failed: ${String(response.status)}`);
      return response.arrayBuffer();
    },
    controls: { enabled: true },
    frame: browserFrame(),
    pixelRatio: 1,
  });
  const voxelFrameSwap = surface.applyFrame({
    schemaVersion: 1,
    ops: [{ op: 'setVoxelObjectFrame', handle: renderHandle(108), frame: 1 }],
  });
  if (!voxelFrameSwap.applied) {
    throw new Error(`voxel frame swap failed: ${voxelFrameSwap.diagnostics.map((item) => item.message).join('; ')}`);
  }

  const audio = new RendererAudioHost({
    resolveResource: async (clip) => ({ bytes: audioBytes.slice(0), contentHash: clip.contentHash }),
  });
  const billboard = new RendererBillboardHost({
    container: overlays,
    projectWorld: (position) => ({ ...surface.projectWorldPoint(position), occluded: false }),
    resolveEntityPosition: () => null,
  });
  const particleSink = new RendererDomParticleBillboardSink({
    container: overlays,
    projectWorld: surface.projectWorldPoint,
  });
  const particle = new RendererParticleHost({
    resolveEntityPosition: () => null,
    resolveResource: async () => ({ bytes: spriteBytes.buffer.slice(0), url: spriteUrl }),
    sink: particleSink,
  });
  const telemetryCollector = new RendererLiveTelemetryCollector({
    expectedCounters: [
      'entityCount',
      'drawCallCount',
      'renderHandleCount',
      'geometryResourceCount',
      'materialResourceCount',
      'textureResourceCount',
      'animatedInstanceCount',
      'triangleCount',
    ],
  });
  const telemetrySink = new RendererDomTelemetryOverlaySink({ container: overlays });
  const telemetry = new RendererTelemetryOverlayHost({
    collector: telemetryCollector,
    sink: telemetrySink,
  });
  const animation = new RendererAnimationHost(surface.animationProjection);
  const hosts = new RendererPresentationHostSet({
    animation,
    audio,
    billboard,
    particle,
    telemetryOverlay: telemetry,
  });
  surface.setPresentationHosts(hosts);

  const presentation = await surface.applyPresentation(
    browserPresentationFrame(audioHash, spriteHash),
  );
  const renderSequenceBeforeAutoFrame = surface.timing().renderSequence;
  surface.start();
  surface.start();
  const automaticSubmissionPacingSamples: RendererSurfaceAutomaticSubmissionPacingSample[] = [];
  const automaticSubmissionIntervalsMs: (number | null)[] = [];
  const automaticSubmissionSourceTimesMs: number[] = [];
  for (let index = 0; index < 4; index += 1) {
    const sequenceBeforeFrame = surface.timing().renderSequence;
    await waitForAnimationFrame(
      () => surface.timing().renderSequence > sequenceBeforeFrame,
    );
    automaticSubmissionPacingSamples.push(surface.automaticSubmissionPacing());
    automaticSubmissionIntervalsMs.push(surface.submission().frameIntervalMs);
    automaticSubmissionSourceTimesMs.push(surface.submission().sourceTimeMs);
  }
  surface.stop();
  const autoSubmission = surface.submission();
  const automaticSubmissionPacing = surface.automaticSubmissionPacing();
  const animatedCapture = captureRendererAnimatedMesh(surface, {
    handle: renderHandle(105),
    clip: 'run',
    normalizedTimes: [0, 0.5, 1],
    providerRevision: '1111111111111111111111111111111111111111',
    overlaysIncluded: true,
  });
  telemetry.sampleSurface({
    sourceTick: 1,
    timing: autoSubmission,
    counters: { entityCount: 4 },
  }, autoSubmission.sourceTimeMs);
  surface.resetCamera();
  const resetSubmission = surface.submission();
  surface.renderOnce(16);
  const explicitTiming = surface.renderOnce(66);

  const replacementCanvas = document.createElement('canvas');
  replacementCanvas.width = 64;
  replacementCanvas.height = 64;
  const replacementSurface = mountRendererSurface(replacementCanvas, {
    autoStart: false,
    frame: replacementFrame(),
    pixelRatio: 1,
  });
  const replacementSubmission = replacementSurface.submission();
  replacementSurface.dispose();
  let replacementDisposedRenderRejected = false;
  try {
    replacementSurface.renderOnce(1);
  } catch {
    replacementDisposedRenderRejected = true;
  }
  const replacementDisposedWithHistoricalSample =
    replacementSurface.submission() === replacementSubmission
    && replacementSurface.snapshot() === '(empty scene)\n';

  const staticDemandCanvas = document.createElement('canvas');
  staticDemandCanvas.width = 64;
  staticDemandCanvas.height = 64;
  const staticDemandSurface = mountRendererSurface(staticDemandCanvas, {
    autoStart: false,
    frame: replacementFrame(),
    pixelRatio: 1,
  });
  const staticDemandMountSequence = staticDemandSurface.submission().renderSequence;
  staticDemandSurface.start();
  await waitForAnimationFrame(
    () => staticDemandSurface.submission().renderSequence > staticDemandMountSequence,
  );
  const staticDemandIdleSequence = staticDemandSurface.submission().renderSequence;
  const staticDemandRejected = staticDemandSurface.applyFrame({
    schemaVersion: 1,
    ops: [{
      op: 'update',
      handle: renderHandle(999),
      transform: identity([1, 0, -3], [1, 1, 1]),
      material: null,
      visible: null,
      metadata: null,
    }],
  });
  await waitAnimationFrames(3);
  const staticDemandRejectedSequence = staticDemandSurface.submission().renderSequence;
  const staticDemandApplied = staticDemandSurface.applyFrame({
    schemaVersion: 1,
    ops: [{
      op: 'update',
      handle: renderHandle(1),
      transform: identity([1, 0, -3], [1, 1, 1]),
      material: null,
      visible: null,
      metadata: null,
    }],
  });
  await waitForAnimationFrame(
    () => staticDemandSurface.submission().renderSequence > staticDemandIdleSequence,
  );
  const staticDemandDirtySequence = staticDemandSurface.submission().renderSequence;
  staticDemandSurface.setCameraPose({
    position: [1, 1.62, 8],
    pitchDegrees: 0,
    yawDegrees: 0,
  });
  staticDemandSurface.setCameraPose({
    position: [2, 1.62, 8],
    pitchDegrees: 0,
    yawDegrees: 0,
  });
  staticDemandSurface.setCameraPose({
    position: [3, 1.62, 8],
    pitchDegrees: 0,
    yawDegrees: 0,
  });
  await waitForAnimationFrame(
    () => staticDemandSurface.submission().renderSequence > staticDemandDirtySequence,
  );
  const staticDemandCameraSequence = staticDemandSurface.submission().renderSequence;
  const staticDemandCameraPosition = staticDemandSurface.cameraPose().position;
  staticDemandSurface.stop();
  staticDemandSurface.dispose();

  const staticMeshCanvas = document.createElement('canvas');
  staticMeshCanvas.width = 64;
  staticMeshCanvas.height = 64;
  const staticMeshSurface = mountRendererSurface(staticMeshCanvas, {
    autoStart: false,
    frame: staticMeshLifetimeFrame(),
    pixelRatio: 1,
  });
  const staticMeshRecreate = staticMeshSurface.applyFrame({
    schemaVersion: 1,
    ops: [
      { op: 'destroy', handle: renderHandle(1) },
      {
        op: 'createStaticMeshInstance',
        handle: renderHandle(2),
        parent: null,
        instance: {
          asset: 'mesh/static-lifetime-proof',
          transform: identity([0, 0, -2], [1, 1, 1]),
          visible: true,
          materialOverrides: [],
          metadata: metadata('static-lifetime-recreated'),
        },
      },
    ],
  });
  staticMeshSurface.renderOnce(1);
  const staticMeshRecreateSnapshot = staticMeshSurface.snapshot();
  const staticMeshRecreateStatistics = staticMeshSurface.submission().statistics;
  staticMeshSurface.dispose();
  const staticMeshRecreateDisposed = staticMeshSurface.snapshot() === '(empty scene)\n';

  const batchedStaticCanvas = document.createElement('canvas');
  batchedStaticCanvas.width = 64;
  batchedStaticCanvas.height = 64;
  const batchedStaticSurface = mountRendererSurface(batchedStaticCanvas, {
    autoStart: false,
    frame: batchedStaticMeshFrame(),
    pixelRatio: 1,
  });
  batchedStaticSurface.renderOnce(1);
  const batchedStaticStatistics = batchedStaticSurface.submission().statistics;
  const batchedStaticPick = batchedStaticSurface.pick({
    filter: { handles: [renderHandle(1_150)] },
    ray: { kind: 'viewport', point: [0, 0] },
    maxDistance: 20,
  });
  batchedStaticSurface.setCameraPose({
    position: [512, 1.62, 8],
    yawDegrees: 0,
    pitchDegrees: 0,
  });
  batchedStaticSurface.renderOnce(1.25);
  const batchedStaticFarStatistics = batchedStaticSurface.submission().statistics;
  batchedStaticCanvas.width = 96;
  batchedStaticCanvas.height = 80;
  batchedStaticSurface.resetCamera();
  batchedStaticSurface.renderOnce(1.5);
  const batchedStaticResetStatistics = batchedStaticSurface.submission().statistics;
  batchedStaticSurface.applyFrame({
    schemaVersion: 1,
    ops: [
      ...Array.from({ length: 299 }, (_, index) => ({
        op: 'destroy' as const,
        handle: renderHandle(1_000 + index),
      })),
      {
        op: 'update' as const,
        handle: renderHandle(1_299),
        transform: identity([0, 1.62, -3], [1, 1, 1]),
        material: null,
        visible: null,
        metadata: null,
      },
      {
        op: 'createStaticMeshInstance' as const,
        handle: renderHandle(2_000),
        parent: null,
        instance: {
          asset: 'mesh/static-batch-proof',
          transform: identity([0, 1.62, -3], [1, 1, 1]),
          visible: true,
          materialOverrides: [],
          metadata: metadata('static-batch-recreated', 2_000),
        },
      },
    ],
  });
  batchedStaticSurface.renderOnce(2);
  const batchedStaticRecreateStatistics = batchedStaticSurface.submission().statistics;
  batchedStaticSurface.dispose();
  const batchedStaticDisposed = batchedStaticSurface.snapshot() === '(empty scene)\n';

  const inspection = await mountRendererInspectionSurface(inspectionCanvas, {
    autoStart: false,
    frame: inspectionFrame(),
    initialGrid: {
      visible: true,
      grid: {
        coordinateSystem: 'rightHandedYUp',
        origin: [0, 0, 0],
        spacing: [1, 1, 1],
      },
      plane: 'xz',
      snapAnchor: 'boundary',
      style: {
        minorColor: [0.35, 0.4, 0.45, 0.45],
        majorColor: [0.55, 0.6, 0.7, 0.7],
        xAxisColor: [1, 0.25, 0.25, 0.9],
        yAxisColor: [0.25, 1, 0.25, 0.9],
        zAxisColor: [0.25, 0.55, 1, 0.9],
        majorLineEvery: 5,
        opacity: 1,
        fadeStart: 20,
        fadeEnd: 80,
      },
    },
    pixelRatio: 1,
  });
  inspection.renderOnce(16);
  const inspectionSubmission = inspection.submission();

  const context = canvas.getContext('webgl2') ?? canvas.getContext('webgl');
  if (context === null || context.isContextLost()) throw new Error('real WebGL context is unavailable');
  const pick = surface.pick({ ray: { kind: 'viewport', point: [0, 0] }, maxDistance: 20 });
  const viewmodelPick = surface.pick({
    filter: { handles: [renderHandle(110), renderHandle(111)] },
    ray: { kind: 'viewport', point: [0.65, -0.35] },
    maxDistance: 20,
  });
  const projected = surface.projectWorldPoint([0, 0, -5]);
  const snapshot = surface.snapshot();
  const projection = surface.projectionSnapshot();
  const voxelNode = surface.projectionSnapshot().nodes.find((node) => node.handle === renderHandle(108));
  const proof: BrowserProof = {
    animatedCapture: {
      asset: animatedCapture.manifest.asset,
      contactSheetPng: animatedCapture.contactSheetPngDataUrl.startsWith('data:image/png;base64,'),
      contentHash: animatedCapture.manifest.contentHash,
      diagnostics: animatedCapture.manifest.samples.map(
        (sample) => sample.diagnostics.map((diagnostic) => diagnostic.code),
      ),
      imageCount: animatedCapture.images.length,
      individualPngs: animatedCapture.images.every((image) => image.pngDataUrl.startsWith('data:image/png;base64,')),
      normalizedTimes: animatedCapture.manifest.samples.map((sample) => sample.normalizedTime),
      providerRevision: animatedCapture.manifest.providerRevision,
      statisticsAvailable: animatedCapture.manifest.samples.map(
        (sample) => sample.statistics.animatedInstanceCount.status === 'available',
      ),
      worldBoundsPresent: animatedCapture.manifest.samples.map(
        (sample) => sample.sampledWorldBounds !== null,
      ),
    },
    animationClip: surface.animatedMeshPlayback(renderHandle(105)).selectedClip,
    audioApplied: presentation.domains.find((domain) => domain.domain === 'audio')?.applied ?? 0,
    audioResumeDiagnostics: null,
    automaticSubmissionPacing,
    automaticSubmissionPacingSamples,
    automaticSubmissionIntervalsMs,
    automaticSubmissionSourceTimesMs,
    autoFrameIntervalMs: autoSubmission.frameIntervalMs,
    autoStartRenderCount: autoSubmission.renderSequence - renderSequenceBeforeAutoFrame,
    backendSubmissionDurationMs: autoSubmission.backendSubmissionDurationMs,
    batchedStaticFarStatistics,
    batchedStaticPickHandle: batchedStaticPick.hint?.handle ?? null,
    batchedStaticRecreateStatistics,
    batchedStaticResetStatistics,
    batchedStaticStatistics,
    batchedStaticDisposed,
    billboardText: overlays.querySelector('[data-rusty-billboard-handle]')?.textContent ?? null,
    context: context instanceof WebGL2RenderingContext ? 'webgl2' : 'webgl',
    explicitFrameIntervalMs: explicitTiming.frameIntervalMs,
    hostSurfaceKind: surface.kind,
    inspectionGridLines: inspection.readout().grid?.renderedLineCount ?? null,
    inspectionRendererStatistics: inspectionSubmission.statistics,
    inspectionSurfaceKind: inspection.kind,
    lightCount: snapshot.match(/kind light\//gu)?.length ?? 0,
    particleElementCount: particleSink.activeCount,
    pickHandle: pick.hint?.handle ?? null,
    presentationDiagnostics: presentation.diagnostics.map((diagnostic) => diagnostic.code),
    projectionInsideViewport: projected.insideViewport,
    ready: true,
    rendererBufferPixelRatio: [
      canvas.width / canvas.clientWidth,
      canvas.height / canvas.clientHeight,
    ],
    rendererStatistics: autoSubmission.statistics,
    replacementDisposedRenderRejected,
    replacementDisposedWithHistoricalSample,
    replacementRenderSequence: replacementSubmission.renderSequence,
    replacementStatistics: replacementSubmission.statistics,
    resetRendererStatistics: resetSubmission.statistics,
    snapshot,
    staticMeshRecreateApplied: staticMeshRecreate.applied,
    staticMeshRecreateDisposed,
    staticMeshRecreateSnapshot,
    staticMeshRecreateStatistics,
    staticDemandApplied: staticDemandApplied.applied,
    staticDemandCameraPosition,
    staticDemandCameraRenderCount: staticDemandCameraSequence - staticDemandDirtySequence,
    staticDemandDirtyRenderCount: staticDemandDirtySequence - staticDemandIdleSequence,
    staticDemandIdleRenderCount: staticDemandIdleSequence - staticDemandMountSequence,
    staticDemandRejectedApplied: staticDemandRejected.applied,
    staticDemandRejectedRenderCount:
      staticDemandRejectedSequence - staticDemandIdleSequence,
    telemetryText: overlays.querySelector('[data-rusty-telemetry-handle]')?.textContent ?? null,
    viewmodelAnimationClip: surface.animatedMeshPlayback(renderHandle(111)).selectedClip,
    viewmodelNodeCount: projection.nodes.filter((node) => node.layer === 'viewmodel').length,
    viewmodelPickExcluded: viewmodelPick.hint === null,
    voxelFrame: voxelNode?.kind === 'voxelObject' ? voxelNode.frame : null,
    voxelFrameSwapApplied: voxelFrameSwap.applied,
  };
  window.__rustyRenderProof = proof;
  window.__rustyRenderBackendSnapshot = () => surface.snapshot();
  window.__rustyRenderCameraPose = () => surface.cameraPose().position;
  window.__rustyRenderSetCameraPose = (position) => {
    surface.setCameraPose({ position, pitchDegrees: 0, yawDegrees: 0 });
    surface.renderOnce(100);
  };
  window.__rustyRenderTick = (timeMs) => surface.renderOnce(timeMs);
  window.__rustyRenderViewmodelState = () => surface.projectionSnapshot().nodes
    .filter((node) => node.layer === 'viewmodel')
    .map((node) => `${String(node.handle)}:${JSON.stringify(node.transform)}`);
  window.__rustyRenderStartAudio = async () => {
    proof.audioResumeDiagnostics = (await audio.resume()).map((diagnostic) => diagnostic.code);
  };
  audioButton.addEventListener('click', () => void window.__rustyRenderStartAudio?.());
  window.__rustyRenderDispose = async () => {
    inspection.dispose();
    surface.dispose();
    animation.cleanup();
    billboard.dispose();
    particle.dispose();
    particleSink.dispose();
    telemetry.cleanup();
    telemetrySink.dispose();
    await audio.dispose();
    URL.revokeObjectURL(spriteUrl);
  };
}

async function waitAnimationFrames(count: number): Promise<void> {
  for (let index = 0; index < count; index += 1) {
    await new Promise<void>((resolve) => globalThis.requestAnimationFrame(() => resolve()));
  }
}

async function waitForAnimationFrame(
  predicate: () => boolean,
  maximumFrames = 120,
): Promise<void> {
  for (let index = 0; index < maximumFrames; index += 1) {
    if (predicate()) {
      return;
    }
    await waitAnimationFrames(1);
  }
  throw new Error(`renderer condition did not settle within ${maximumFrames} animation frames`);
}

function replacementFrame(): RenderFrameDiff {
  return {
    schemaVersion: 1,
    ops: [
      {
        op: 'create', handle: renderHandle(1), parent: null,
        node: {
          geometry: { kind: 'cube' },
          material: { color: [1, 1, 1, 1], wireframe: false },
          transform: identity([0, 0, -3], [1, 1, 1]), visible: true, layer: 'scene',
          metadata: metadata('replacement-world'),
        },
      },
      {
        op: 'create', handle: renderHandle(2), parent: null,
        node: {
          geometry: { kind: 'cube' },
          material: { color: [1, 1, 1, 1], wireframe: false },
          transform: identity([0, 0, -1], [1, 1, 1]), visible: true, layer: 'viewmodel',
          metadata: metadata('replacement-viewmodel'),
        },
      },
    ],
  };
}

function staticMeshLifetimeFrame(): RenderFrameDiff {
  return {
    schemaVersion: 1,
    ops: [
      {
        op: 'defineStaticMesh',
        asset: {
          asset: 'mesh/static-lifetime-proof',
          payload: trianglePayload(),
          materialSlots: [{ slot: 0, material: 'material/static-lifetime-proof' }],
          collision: { kind: 'visualOnly' },
        },
      },
      {
        op: 'createStaticMeshInstance',
        handle: renderHandle(1),
        parent: null,
        instance: {
          asset: 'mesh/static-lifetime-proof',
          transform: identity([0, 0, -2], [1, 1, 1]),
          visible: true,
          materialOverrides: [],
          metadata: metadata('static-lifetime-initial'),
        },
      },
    ],
  };
}

function batchedStaticMeshFrame(): RenderFrameDiff {
  return {
    schemaVersion: 1,
    ops: [
      {
        op: 'defineStaticMesh',
        asset: {
          asset: 'mesh/static-batch-proof',
          payload: trianglePayload(),
          materialSlots: [{ slot: 0, material: 'material/static-batch-proof' }],
          collision: { kind: 'visualOnly' },
        },
      },
      ...Array.from({ length: 300 }, (_, index) => ({
        op: 'createStaticMeshInstance' as const,
        handle: renderHandle(1_000 + index),
        parent: null,
        instance: {
          asset: 'mesh/static-batch-proof',
          transform: identity(
            [index < 200 ? 0 : 512, 1.62, -2],
            [1, 1, 1],
          ),
          visible: true,
          materialOverrides: [],
          metadata: metadata(`static-batch-${String(index)}`, 1_000 + index),
        },
      })),
    ],
  };
}

function browserPresentationFrame(audioHash: string, spriteHash: string): PresentationFrameDiff {
  return {
    schemaVersion: 1,
    ops: [
      {
        domain: 'animation',
        meta: { sequence: 0 },
        op: {
          op: 'create',
          handle: animationProjectionHandle(1),
          descriptor: {
            target: renderHandle(105),
            asset: ASSET,
            contentHash: CONTENT_HASH,
            tickDurationMillis: 50,
            controller: {
              entity: 1,
              graphId: 'browser-proof',
              graphVersion: 1,
              stateId: 'run',
              revision: 0,
              controllerTick: 0,
              motion: {
                clipA: 'run',
                clipB: null,
                blendWeightMilli: 0,
                speedMilli: 1_000,
              },
              transition: null,
              transitionFact: null,
            },
          },
        },
      },
      {
        domain: 'audio',
        meta: { sequence: 1 },
        op: {
          op: 'emit',
          signalId: 'browser-proof-tone',
          descriptor: {
            clip: { asset: 'audio/browser-proof-tone', contentHash: audioHash },
            bus: 'ui',
            volume: 0.05,
            pitch: 1,
            looping: false,
            spatialBlend: 0,
            attenuation: 1,
            pan: 0,
            emitter: { kind: 'global2d' },
          },
        },
      },
      {
        domain: 'billboard',
        meta: { sequence: 2 },
        op: {
          op: 'create',
          handle: billboardHandle(1),
          descriptor: {
            anchor: { kind: 'world', position: [0, 2.6, -5] },
            content: {
              kind: 'text',
              localizationKey: 'browser.proof',
              fallbackText: 'Shared renderer host',
              arguments: [],
            },
            font: { kind: 'system', family: 'sans-serif' },
            heightPixels: 16,
            color: [1, 1, 1, 1],
            background: [0, 0, 0, 0.75],
            maxDistance: 50,
            layer: 'alwaysOnTop',
            visible: true,
          },
        },
      },
      {
        domain: 'particle',
        meta: { sequence: 3 },
        op: {
          op: 'emit',
          signalId: 'browser-proof-sparks',
          descriptor: {
            anchor: { kind: 'world', position: [-1, 1.6, -5] },
            sprite: {
              asset: 'sprite/browser-proof-spark',
              contentHash: spriteHash,
              frameCount: 1,
            },
            ratePerSecond: 0,
            burstCount: 2,
            lifetimeSeconds: [1, 1],
            velocityMin: [0, 0.2, 0],
            velocityMax: [0, 0.2, 0],
            acceleration: [0, 0, 0],
            sizeCurve: [{ age: 0, value: 0.5 }, { age: 1, value: 0.1 }],
            colorCurve: [
              { age: 0, color: [1, 0.8, 0.2, 1] },
              { age: 1, color: [1, 0.2, 0, 0] },
            ],
            flipbookFramesPerSecond: 0,
            seed: 7,
            maxParticles: 4,
            visible: true,
          },
        },
      },
      {
        domain: 'telemetryOverlay',
        meta: { sequence: 4 },
        op: {
          op: 'create',
          handle: telemetryOverlayHandle(1),
          descriptor: {
            title: 'Renderer proof',
            corner: 'topRight',
            refreshIntervalMs: 100,
            maxFrameTimeSamples: 10,
            visible: true,
          },
        },
      },
    ],
  };
}

function browserFrame(): RenderFrameDiff {
  return {
    schemaVersion: 1,
    ops: [
      {
        op: 'defineMaterial',
        material: {
          schemaVersion: 2,
          id: 'material/browser-proof',
          color: [0.25, 0.7, 0.9, 1],
          texture: null,
          roughness: 0.8,
          textureTint: [1, 1, 1, 1],
          emissionColor: [0, 0, 0],
          emissionIntensity: 0,
          uvStrategy: 'flat',
        },
      },
      {
        op: 'defineTexture',
        texture: {
          id: 'texture/browser-proof', width: 2, height: 2, filter: 'nearest',
          wrap: 'clamp', contentHash: null, version: 1,
        },
      },
      {
        op: 'defineSpriteAtlas',
        atlas: {
          id: 'sprite/browser-proof',
          texture: 'texture/browser-proof',
          frames: [{ frame: 0, uvMin: [0, 0], uvMax: [1, 1] }],
        },
      },
      {
        op: 'defineStaticMesh',
        asset: {
          asset: 'mesh/browser-proof',
          payload: trianglePayload(),
          materialSlots: [{ slot: 0, material: 'material/browser-proof' }],
          collision: { kind: 'visualOnly' },
        },
      },
      {
        op: 'defineAnimatedMesh',
        asset: {
          asset: ASSET,
          runtimeFormat: 'glb',
          contentHash: CONTENT_HASH,
          clips: [
            { id: 'idle', name: 'idle', durationSeconds: null },
            { id: 'run', name: 'run', durationSeconds: null },
            { id: 'jump', name: 'jump', durationSeconds: null },
          ],
          defaultClip: 'idle',
          materialSlots: [],
          bounds: { min: [-0.02, -0.01, 0], max: [0.02, 0.01, 0.04] },
        },
      },
      {
        op: 'defineVoxelObject',
        asset: {
          asset: 'voxel-object/browser-proof',
          contentHash: 'sha256:browser-voxel-object',
          meshes: [
            { payload: voxelObjectPayload(1) },
            { payload: voxelObjectPayload(1.75) },
          ],
          frames: [{ id: 'default', mesh: 0 }, { id: 'pulse/0', mesh: 1 }],
          materialSlots: [{ slot: 0, material: 'material/browser-proof' }],
        },
      },
      {
        op: 'create', handle: renderHandle(100), parent: null,
        node: {
          geometry: { kind: 'group' },
          material: { color: [1, 1, 1, 1], wireframe: false },
          transform: identity([0, 0, 0], [1, 1, 1]), visible: true, layer: 'scene',
          metadata: metadata('browser-root'),
        },
      },
      {
        op: 'create', handle: renderHandle(101), parent: renderHandle(100),
        node: {
          geometry: { kind: 'cube' },
          material: { color: [0.9, 0.4, 0.2, 1], wireframe: false },
          transform: identity([0, 1.62, -5], [1.5, 1.5, 1.5]), visible: true, layer: 'scene',
          metadata: metadata('pick-target', 101),
        },
      },
      {
        op: 'createStaticMeshInstance', handle: renderHandle(104), parent: null,
        instance: {
          asset: 'mesh/browser-proof', transform: identity([-2, 0, -4], [2, 2, 2]),
          visible: true, materialOverrides: [], metadata: metadata('static-proof'),
        },
      },
      {
        op: 'createAnimatedMeshInstance', handle: renderHandle(105), parent: null,
        instance: {
          asset: ASSET, transform: identity([2, 0, -5], [30, 30, 30]), visible: true,
          materialOverrides: [], playback: null, metadata: metadata('animated-proof'),
        },
      },
      {
        op: 'createSprite', handle: renderHandle(106), parent: null,
        sprite: {
          asset: 'sprite/browser-proof', frame: 0, pivot: [0.5, 0.5], size: [1, 1],
          sizeMode: 'world', billboard: 'none', tint: [1, 1, 0.2, 0.9], renderOrder: 3,
          depth: 'default', shading: 'unlit', visible: true,
          transform: identity([3, 2, -4], [1, 1, 1]),
          attachment: { sourceEntity: null, sourceSceneNode: null, attachmentPoint: null },
          metadata: metadata('sprite-proof'),
        },
      },
      {
        op: 'createLight', handle: renderHandle(107), parent: null,
        light: {
          kind: 'point', color: [1, 0.8, 0.6], intensity: 2, enabled: true,
          position: [0, 3, -2], range: 20, decay: 2, shadowIntent: 'disabled',
        },
      },
      {
        op: 'createVoxelObjectInstance', handle: renderHandle(108), parent: null,
        instance: {
          asset: 'voxel-object/browser-proof', frame: 0,
          transform: identity([0, 0.5, -3], [1, 1, 1]), visible: true,
          materialOverrides: [], metadata: metadata('voxel-object-proof'),
        },
      },
      {
        op: 'create', handle: renderHandle(109), parent: null,
        node: {
          geometry: { kind: 'group' },
          material: { color: [1, 1, 1, 1], wireframe: false },
          transform: identity([0, 0, 0], [1, 1, 1]), visible: true, layer: 'viewmodel',
          metadata: metadata('viewmodel-root'),
        },
      },
      {
        op: 'createStaticMeshInstance', handle: renderHandle(110), parent: renderHandle(109),
        instance: {
          asset: 'mesh/browser-proof',
          transform: identity([0.65, -0.35, -1.4], [0.6, 0.6, 0.6]),
          visible: true, materialOverrides: [], metadata: metadata('viewmodel-static-proof'),
        },
      },
      {
        op: 'createAnimatedMeshInstance', handle: renderHandle(111), parent: renderHandle(109),
        instance: {
          asset: ASSET,
          transform: identity([-0.45, -0.4, -1.5], [20, 20, 20]),
          visible: true,
          materialOverrides: [],
          playback: {
            kind: 'play',
            clip: 'idle',
            loop: 'repeat',
            speed: 1,
            weight: 1,
            restart: true,
            fadeSeconds: null,
          },
          metadata: metadata('viewmodel-animated-proof'),
        },
      },
    ],
  };
}

function inspectionFrame(): RenderFrameDiff {
  return {
    schemaVersion: 1,
    ops: [{
      op: 'create',
      handle: renderHandle(1),
      parent: null,
      node: {
        geometry: { kind: 'cube' },
        material: { color: [0.25, 0.65, 0.9, 1], wireframe: false },
        transform: identity([0, 0.5, 0], [1, 1, 1]),
        visible: true,
        layer: 'scene',
        metadata: metadata('inspection-cube'),
      },
    }],
  };
}

function trianglePayload(): MeshPayloadDescriptor {
  return {
    layout: {
      vertexCount: 3, indexCount: 3, indexWidth: 'u32',
      attributes: [
        { name: 'position', components: 3, kind: 'f32' },
        { name: 'normal', components: 3, kind: 'f32' },
      ],
    },
    groups: [{ materialSlot: 0, start: 0, count: 3 }],
    bounds: { min: [-0.5, -0.5, 0], max: [0.5, 0.5, 0] },
    source: {
      kind: 'inline', positions: [-0.5, -0.5, 0, 0.5, -0.5, 0, 0, 0.5, 0],
      normals: [0, 0, 1, 0, 0, 1, 0, 0, 1], indices: [0, 1, 2],
    },
    provenance: 'staticAsset',
  };
}

function voxelObjectPayload(scale: number): MeshPayloadDescriptor {
  const payload = trianglePayload();
  if (payload.source.kind !== 'inline') throw new Error('browser voxel fixture must stay inline');
  return {
    ...payload,
    bounds: { min: [-0.5 * scale, -0.5 * scale, 0], max: [0.5 * scale, 0.5 * scale, 0] },
    source: {
      ...payload.source,
      positions: payload.source.positions.map((value) => value * scale),
    },
    provenance: 'voxelObject',
  };
}

function identity(
  translation: readonly [number, number, number],
  scale: readonly [number, number, number],
) {
  return { translation, rotation: [0, 0, 0, 1] as const, scale };
}

function metadata(label: string, sourceEntity: number | null = null) {
  return { sourceEntity, sourceSceneNode: null, tags: [] as string[], label };
}

function waveFixture(): ArrayBuffer {
  const sampleRate = 8_000;
  const sampleCount = 640;
  const bytes = new ArrayBuffer(44 + sampleCount * 2);
  const view = new DataView(bytes);
  writeAscii(view, 0, 'RIFF');
  view.setUint32(4, bytes.byteLength - 8, true);
  writeAscii(view, 8, 'WAVE');
  writeAscii(view, 12, 'fmt ');
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, 1, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * 2, true);
  view.setUint16(32, 2, true);
  view.setUint16(34, 16, true);
  writeAscii(view, 36, 'data');
  view.setUint32(40, sampleCount * 2, true);
  for (let index = 0; index < sampleCount; index += 1) {
    const envelope = 1 - index / sampleCount;
    const sample = Math.sin((index / sampleRate) * Math.PI * 2 * 440) * envelope;
    view.setInt16(44 + index * 2, Math.round(sample * 3_000), true);
  }
  return bytes;
}

function writeAscii(view: DataView, offset: number, value: string): void {
  for (let index = 0; index < value.length; index += 1) {
    view.setUint8(offset + index, value.charCodeAt(index));
  }
}

async function sha256(bytes: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', bytes);
  return Array.from(new Uint8Array(digest))
    .map((byte) => byte.toString(16).padStart(2, '0'))
    .join('');
}

function required<T extends Element>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (element === null) throw new Error(`browser proof element ${selector} is missing`);
  return element;
}

void main().catch((error: unknown) => {
  window.__rustyRenderFailure = error instanceof Error ? `${error.name}: ${error.message}` : String(error);
});
