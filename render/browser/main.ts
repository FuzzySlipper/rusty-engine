/// <reference types="vite/client" />

import {
  animationProjectionHandle,
  billboardHandle,
  renderHandle,
  telemetryOverlayHandle,
  type AnimationRigSignature,
  type MeshPayloadDescriptor,
  type PresentationFrameDiff,
  type RenderFrameDiff,
  type RendererViewComposition,
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
  mountRendererSurfaceWithResources,
  type RendererSurfaceAutomaticSubmissionPacingSample,
  type RendererParticleSinkReadout,
  type RendererParticleSink,
  type RendererSurface,
  type RendererSurfaceStatisticsSample,
} from '@rusty-engine/renderer-host';
import { mountRendererBrowserSurface } from '@rusty-engine/renderer-three';
import { animationRigFingerprint, loadAnimatedMeshGlbResource } from '@rusty-engine/renderer-three/backend';
import * as THREE from 'three';

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
  readonly clipPack: {
    readonly effectiveClips: readonly { readonly id: string; readonly origin: 'embedded' | 'pack' }[];
    readonly normalizedTimes: readonly number[];
    readonly independentInstances: boolean;
  };
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
  readonly defaultLightingReadout: ReturnType<RendererSurface['lightingReadout']>;
  readonly visibilityReadout: ReturnType<RendererSurface['visibilityReadout']>;
  readonly authoredLightingReadout: ReturnType<RendererSurface['lightingReadout']>;
  readonly authoredLightingRejected: {
    readonly applied: boolean;
    readonly diagnostic: string | null;
    readonly retainedLightCount: number;
  };
  readonly rejectedMountCleanup: {
    readonly pointerLockRequests: number;
    readonly rejected: boolean;
    readonly tabIndex: number;
    readonly touchAction: string;
  };
  readonly particleReadout: RendererParticleSinkReadout;
  readonly particlePerformance: readonly ParticlePerformanceSample[];
  readonly pickHandle: number | null;
  readonly presentationDiagnostics: readonly string[];
  readonly projectionInsideViewport: boolean;
  readonly ready: true;
  readonly rendererStatistics: RendererSurfaceStatisticsSample;
  readonly rendererBufferPixelRatio: readonly [number, number];
  readonly spriteBillboardPixels: {
    readonly initialSpherical: readonly (readonly [number, number, number, number])[];
    readonly initialCylindrical: readonly (readonly [number, number, number, number])[];
    readonly elevatedSpherical: readonly (readonly [number, number, number, number])[];
    readonly elevatedCylindrical: readonly (readonly [number, number, number, number])[];
  };
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
  readonly staticMeshTexturePixels: readonly (readonly [number, number, number, number])[];
  readonly skyBackgroundPixels: {
    readonly initial: readonly [number, number, number, number];
    readonly translated: readonly [number, number, number, number];
    readonly rotated: readonly [number, number, number, number];
    readonly cleared: readonly [number, number, number, number];
  };
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
  readonly voxelSurfaceAtlasPixels: readonly {
    readonly orientation: 'standard' | 'rotated';
    readonly pixels: readonly (readonly [number, number, number, number])[];
  }[];
  readonly voxelSurfaceSpecializations: readonly unknown[];
  readonly viewComposition: {
    readonly cameraUpdateApplied: boolean;
    readonly cameraUpdateTargetStatus: string | null;
    readonly cameraPosition: readonly [number, number, number] | null;
    readonly disposedResources: { readonly presentationCount: number; readonly targetCount: number };
    readonly drawCallCount: number;
    readonly frameReplacementApplied: boolean;
    readonly frameReplacementTargetStatus: string | null;
    readonly invalidApplied: boolean;
    readonly narrowPixels: readonly (readonly [number, number, number, number])[];
    readonly pixels: readonly (readonly [number, number, number, number])[];
    readonly readout: ReturnType<RendererSurface['viewCompositionReadout']>;
    readonly resizeApplied: boolean;
    readonly staleApplied: boolean;
    readonly staleDiagnostic: string | null;
  };
}

interface ParticlePerformanceSample {
  readonly mode: 'domBillboard' | 'threeBillboard' | 'instancedCube';
  readonly count: number;
  readonly simulatedFrames: number;
  readonly createMs: number;
  readonly averageUpdateAndRenderMs: number;
  readonly teardownMs: number;
  readonly drawCallDelta: number | null;
  readonly active: RendererParticleSinkReadout;
  readonly afterTeardown: RendererParticleSinkReadout;
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
  await decodeBrowserImage(spriteUrl);
  const audioBytes = waveFixture();
  const audioHash = await sha256(audioBytes);
  const animatedFixtureResponse = await fetch(characterUrl);
  if (!animatedFixtureResponse.ok) {
    throw new Error(`animated fixture fetch failed: ${String(animatedFixtureResponse.status)}`);
  }
  const animatedFixtureBytes = await animatedFixtureResponse.arrayBuffer();
  const animatedFixtureResource = await loadAnimatedMeshGlbResource(
    ASSET,
    animatedFixtureBytes.slice(0),
    CONTENT_HASH,
  );
  const idleClip = animatedFixtureResource.clips.find((clip) => clip.name === 'idle');
  if (!idleClip) throw new Error('animated fixture is missing idle clip for clip-pack proof');
  const clipPackRig = rigForFixtureClip(animatedFixtureResource.scene, idleClip);

  const surface = await mountRendererAnimatedMeshSurface(canvas, {
    animatedMeshManifest: {
      kind: 'rusty_renderer_animated_mesh_resources.v1',
      resources: [{
        asset: ASSET,
        contentHash: CONTENT_HASH,
        clipIds: ['run', 'jump'],
      }],
      clipPacks: [{
        asset: 'animation-clip-pack/kenney-retro-character-idle',
        contentHash: CONTENT_HASH,
        clipIds: ['idle'],
      }],
    },
    resolveAnimatedMeshResource: async () => animatedFixtureBytes.slice(0),
    controls: { enabled: true },
    frame: browserFrame(clipPackRig),
    pixelRatio: 1,
  });
  const lightingCanvas = document.createElement('canvas');
  lightingCanvas.width = 96;
  lightingCanvas.height = 64;
  const lightingSurface = mountRendererSurface(lightingCanvas, {
    autoStart: false,
    lighting: {
      schemaVersion: 1,
      defaultLights: { world: 'disabled', viewmodel: 'neutral' },
      shadows: { enabled: true, maximumActiveLights: 3 },
    },
    frame: {
      schemaVersion: 1,
      ops: [
        { op: 'createLight', handle: renderHandle(801), parent: null, light: {
          kind: 'ambient', color: [0.05, 0.07, 0.1], intensity: 0.2, enabled: true,
          shadowIntent: 'requested',
        } },
        { op: 'createLight', handle: renderHandle(802), parent: null, light: {
          kind: 'directional', color: [0.3, 0.4, 0.7], intensity: 0.5, enabled: true,
          direction: [-1, -2, -1], shadowIntent: 'requested',
        } },
        { op: 'createLight', handle: renderHandle(803), parent: null, light: {
          kind: 'point', color: [1, 0.4, 0.1], intensity: 5, enabled: true,
          position: [0, 2, 0], range: 10, decay: 2, shadowIntent: 'requested',
        } },
        { op: 'createLight', handle: renderHandle(804), parent: null, light: {
          kind: 'spot', color: [0.2, 0.5, 1], intensity: 3, enabled: true,
          position: [2, 4, 0], direction: [0, -1, 0], range: 12, decay: 2,
          outerAngleRadians: 0.6, penumbra: 0.25, shadowIntent: 'requested',
        } },
      ],
    },
  });
  lightingSurface.renderOnce(1);
  const rejectedLighting = lightingSurface.applyFrame({ schemaVersion: 1, ops: [{
    op: 'createLight', handle: renderHandle(805), parent: null, light: {
      kind: 'point', color: [1, 1, 1], intensity: 1, enabled: true,
      position: [0, 1, 0], range: 5, decay: 2, shadowIntent: 'requested',
    },
  }] });

  const rejectedMountCanvas = document.createElement('canvas');
  rejectedMountCanvas.tabIndex = -1;
  rejectedMountCanvas.style.touchAction = 'pan-x';
  let pointerLockRequests = 0;
  Object.defineProperty(rejectedMountCanvas, 'requestPointerLock', {
    configurable: true,
    value: () => { pointerLockRequests += 1; },
  });
  let rejectedMount = false;
  try {
    mountRendererSurface(rejectedMountCanvas, {
      autoStart: false,
      controls: { enabled: true },
      lighting: {
        schemaVersion: 1,
        shadows: { enabled: true, maximumActiveLights: 1 },
      },
      frame: {
        schemaVersion: 1,
        ops: [-1, 1].map((x, index) => ({
          op: 'createLight' as const,
          handle: renderHandle(901 + index),
          parent: null,
          light: {
            kind: 'point' as const,
            color: [1, 0.8, 0.5] as const,
            intensity: 2,
            enabled: true,
            position: [x, 2, -4] as const,
            range: 8,
            decay: 2,
            shadowIntent: 'requested' as const,
          },
        })),
      },
    });
  } catch {
    rejectedMount = true;
  }
  rejectedMountCanvas.dispatchEvent(new PointerEvent('pointerdown', { button: 0 }));
  const rejectedMountCleanup = {
    pointerLockRequests,
    rejected: rejectedMount,
    tabIndex: rejectedMountCanvas.tabIndex,
    touchAction: rejectedMountCanvas.style.touchAction,
  };
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
  const particleSink = surface.createParticleSink();
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
    clip: 'idle',
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

  const voxelSurfaceProofs = (['standard', 'rotated'] as const).map((orientation) => {
    const proofCanvas = document.createElement('canvas');
    proofCanvas.width = 256;
    proofCanvas.height = 256;
    proofCanvas.style.cssText = 'position:fixed;left:-10000px;top:0;width:256px;height:256px';
    document.body.appendChild(proofCanvas);
    const proofSurface = mountRendererBrowserSurface(proofCanvas, {
      autoStart: false,
      camera: {
        initialPose: { position: [0, 0, 1], pitchDegrees: 0, yawDegrees: 0 },
        projection: { fovYDegrees: 90, near: 0.1, far: 10 },
      },
      clearColor: 0x000000,
      frame: voxelSurfaceBrowserFrame(orientation),
      pixelRatio: 1,
    });
    proofSurface.renderOnce(1);
    const proofContext = proofCanvas.getContext('webgl2') ?? proofCanvas.getContext('webgl');
    if (proofContext === null) throw new Error('voxel texture WebGL context is unavailable');
    const sampleY = Math.floor(proofContext.drawingBufferHeight * 3 / 16);
    const pixels = Array.from({ length: 8 }, (_, index) => {
      const x = Math.floor(proofContext.drawingBufferWidth * (index + 0.5) / 8);
      const pixel = new Uint8Array(4);
      proofContext.readPixels(
        x,
        sampleY,
        1,
        1,
        proofContext.RGBA,
        proofContext.UNSIGNED_BYTE,
        pixel,
      );
      return [...pixel] as [number, number, number, number];
    });
    return { orientation, pixels, proofCanvas, proofSurface };
  });
  const voxelSurfaceAtlasPixels = voxelSurfaceProofs.map(({ orientation, pixels }) => ({
    orientation,
    pixels,
  }));
  const voxelSurfaceSpecializations = voxelSurfaceProofs.flatMap(
    ({ proofSurface }) => proofSurface.renderer.voxelSurfaceMaterialReadout(),
  );

  const billboardSurfaceProofs = (['spherical', 'cylindrical'] as const).map((mode) => {
    const proofCanvas = document.createElement('canvas');
    proofCanvas.width = 256;
    proofCanvas.height = 256;
    proofCanvas.style.cssText = 'position:fixed;left:-10000px;top:0;width:256px;height:256px';
    document.body.appendChild(proofCanvas);
    const frame = billboardBrowserFrame(mode);
    const proofSurface = mountRendererBrowserSurface(proofCanvas, {
      autoStart: false,
      camera: {
        initialPose: { position: [0, 0, 4], pitchDegrees: 0, yawDegrees: 0 },
        projection: { fovYDegrees: 60, near: 0.1, far: 20 },
      },
      clearColor: 0x000000,
      frame,
      pixelRatio: 1,
    });
    proofSurface.renderOnce(1);
    const proofContext = proofCanvas.getContext('webgl2') ?? proofCanvas.getContext('webgl');
    if (proofContext === null) throw new Error('billboard WebGL context is unavailable');
    const initialPixels = readBillboardPixels(proofContext);
    proofSurface.setCameraPose({
      position: [2, 4, 4],
      pitchDegrees: -41.810314895,
      yawDegrees: -26.565051177,
    });
    proofSurface.renderOnce(2);
    const elevatedPixels = readBillboardPixels(proofContext);
    return { mode, initialPixels, elevatedPixels, proofCanvas, proofSurface };
  });

  const staticMeshTextureCanvas = document.createElement('canvas');
  staticMeshTextureCanvas.width = 256;
  staticMeshTextureCanvas.height = 128;
  staticMeshTextureCanvas.style.cssText =
    'position:fixed;left:-10000px;top:0;width:256px;height:128px';
  document.body.appendChild(staticMeshTextureCanvas);
  const staticMeshTextureFrame = staticMeshTextureBrowserFrame();
  const staticMeshTextureDefinition = staticMeshTextureFrame.ops.find(
    (operation) => operation.op === 'defineTexture',
  );
  const staticMeshTextureSource = staticMeshTextureDefinition?.op === 'defineTexture'
    ? staticMeshTextureDefinition.texture.payload?.source
    : undefined;
  if (
    staticMeshTextureDefinition === undefined
    || staticMeshTextureSource?.kind !== 'inline'
  ) {
    throw new Error('static mesh texture proof fixture has no inline texture payload');
  }
  const staticMeshTexturePayload = staticMeshTextureDefinition.texture.payload;
  if (staticMeshTexturePayload === undefined) {
    throw new Error('static mesh texture proof fixture has no texture payload');
  }
  const staticMeshTextureResource =
    `texture-resource/${staticMeshTexturePayload.contentHash.slice('sha256:'.length)}`;
  const staticMeshTextureResourceFrame: RenderFrameDiff = {
    ...staticMeshTextureFrame,
    ops: staticMeshTextureFrame.ops.map((operation) => operation.op === 'defineTexture'
      ? {
          ...operation,
          texture: {
            ...operation.texture,
            payload: {
              ...staticMeshTexturePayload,
              source: { kind: 'resource', resource: staticMeshTextureResource },
            },
          },
        }
      : operation),
  };
  const staticMeshTextureSurface = await mountRendererSurfaceWithResources(staticMeshTextureCanvas, {
    autoStart: false,
    controls: {
      enabled: false,
      initialPosition: [0, 0, 1],
      initialPitchDegrees: 0,
      initialYawDegrees: 0,
    },
    clearColor: 0x000000,
    frame: staticMeshTextureResourceFrame,
    pixelRatio: 1,
    projection: { fovYDegrees: 90, near: 0.1, far: 10 },
    textureResourceManifest: {
      kind: 'rusty_renderer_texture_resources.v1',
      resources: [{
        resource: staticMeshTextureResource,
        contentHash: staticMeshTexturePayload.contentHash,
        byteLength: staticMeshTexturePayload.byteLength,
      }],
    },
    resolveTextureResource: () => Promise.resolve(
      Uint8Array.from(staticMeshTextureSource.encodedBytes).buffer,
    ),
  });
  staticMeshTextureSurface.renderOnce(1);
  const staticMeshTextureContext = staticMeshTextureCanvas.getContext('webgl2')
    ?? staticMeshTextureCanvas.getContext('webgl');
  if (staticMeshTextureContext === null) {
    throw new Error('static mesh texture WebGL context is unavailable');
  }
  const staticMeshTexturePixels = [0.375, 0.625].map((fraction) => {
    const pixel = new Uint8Array(4);
    staticMeshTextureContext.readPixels(
      Math.floor(staticMeshTextureContext.drawingBufferWidth * fraction),
      Math.floor(staticMeshTextureContext.drawingBufferHeight / 2),
      1,
      1,
      staticMeshTextureContext.RGBA,
      staticMeshTextureContext.UNSIGNED_BYTE,
      pixel,
    );
    return [...pixel] as [number, number, number, number];
  });

  const skyCanvas = document.createElement('canvas');
  skyCanvas.width = 128;
  skyCanvas.height = 128;
  skyCanvas.style.cssText = 'position:fixed;left:-10000px;top:0;width:128px;height:128px';
  document.body.appendChild(skyCanvas);
  const skyTexture = {
    ...staticMeshTextureDefinition.texture,
    id: 'texture/sky-background-proof',
  };
  const skySurface = mountRendererSurface(skyCanvas, {
    autoStart: false,
    clearColor: 0x123456,
    controls: {
      enabled: false,
      initialPosition: [0, 0, 0],
      initialPitchDegrees: 0,
      initialYawDegrees: 0,
    },
    frame: {
      schemaVersion: 1,
      ops: [
        { op: 'defineTexture', texture: skyTexture },
        { op: 'setSkyBackground', background: { texture: skyTexture.id } },
      ],
    },
    pixelRatio: 1,
    projection: { fovYDegrees: 90, near: 0.1, far: 10 },
  });
  const skyContext = skyCanvas.getContext('webgl2') ?? skyCanvas.getContext('webgl');
  if (skyContext === null) throw new Error('sky background WebGL context is unavailable');
  const readSkyPixel = (): [number, number, number, number] => {
    const pixel = new Uint8Array(4);
    skyContext.readPixels(
      Math.floor(skyContext.drawingBufferWidth / 2),
      Math.floor(skyContext.drawingBufferHeight / 2),
      1,
      1,
      skyContext.RGBA,
      skyContext.UNSIGNED_BYTE,
      pixel,
    );
    return [...pixel] as [number, number, number, number];
  };
  skySurface.renderOnce(1);
  const skyInitial = readSkyPixel();
  skySurface.setCameraPose({ position: [37, -12, 91], pitchDegrees: 0, yawDegrees: 0 });
  skySurface.renderOnce(2);
  const skyTranslated = readSkyPixel();
  skySurface.setCameraPose({ position: [37, -12, 91], pitchDegrees: 0, yawDegrees: 180 });
  skySurface.renderOnce(3);
  const skyRotated = readSkyPixel();
  skySurface.applyFrame({
    schemaVersion: 1,
    ops: [{ op: 'setSkyBackground', background: null }],
  });
  skySurface.renderOnce(4);
  const skyCleared = readSkyPixel();
  const skyBackgroundPixels = {
    initial: skyInitial,
    translated: skyTranslated,
    rotated: skyRotated,
    cleared: skyCleared,
  };

  const compositionCanvas = document.createElement('canvas');
  compositionCanvas.width = 320;
  compositionCanvas.height = 200;
  compositionCanvas.style.cssText = 'width:320px;height:200px';
  document.body.appendChild(compositionCanvas);
  const compositionSurface = mountRendererSurface(compositionCanvas, {
    autoStart: false,
    clearColor: 0x020408,
    controls: { enabled: false, initialPosition: [0, 2, 8] },
    frame: viewCompositionFrame(),
    pixelRatio: 1,
    viewComposition: viewComposition(1, 128),
  });
  const compositionSubmission = compositionSurface.renderOnce(1);
  const compositionContext = compositionCanvas.getContext('webgl2')
    ?? compositionCanvas.getContext('webgl');
  if (compositionContext === null) throw new Error('view composition WebGL context is unavailable');
  const pixels = readCompositionPixels(compositionContext);
  compositionSurface.start();
  compositionSurface.stop();
  const compositionFrameReplacement = compositionSurface.applyFrame({
    schemaVersion: 1,
    ops: [{
      op: 'update',
      handle: renderHandle(1),
      transform: identity([-2, 1, -5], [3, 3, 3]),
      material: null,
      visible: null,
      metadata: null,
    }],
  });
  const frameReplacementTargetStatus = compositionSurface
    .viewCompositionReadout().targets[0]?.status ?? null;
  const resizeReceipt = compositionSurface.configureViews(viewComposition(2, 192));
  compositionSurface.renderOnce(2);
  const cameraUpdateReceipt = compositionSurface.configureViews(viewComposition(2, 192, 7));
  const cameraUpdateTargetStatus = compositionSurface
    .viewCompositionReadout().targets[0]?.status ?? null;
  compositionSurface.renderOnce(3);
  const staleReceipt = compositionSurface.configureViews(viewComposition(1, 128));
  const invalidComposition = structuredClone(viewComposition(2, 192)) as unknown as {
    presentations: Array<{ destination: { kind: string } }>;
  };
  invalidComposition.presentations[0]!.destination.kind = 'offscreen';
  const invalidReceipt = compositionSurface.configureViews(
    invalidComposition as unknown as RendererViewComposition,
  );
  compositionCanvas.style.width = '160px';
  compositionCanvas.style.height = '100px';
  compositionSurface.renderOnce(4);
  const narrowPixels = readCompositionPixels(compositionContext);
  const compositionReadout = compositionSurface.viewCompositionReadout();
  compositionSurface.dispose();
  const disposedResources = compositionSurface.viewCompositionReadout().resources;
  compositionCanvas.remove();

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
  const visibilityReadout = surface.visibilityReadout();
  const voxelNode = surface.projectionSnapshot().nodes.find((node) => node.handle === renderHandle(108));
  const particlePerformance = await measureParticlePerformance(surface, overlays, spriteUrl);
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
    clipPack: {
      effectiveClips: surface.animatedMeshPlayback(renderHandle(105)).effectiveClips.map(({ id, origin }) => ({ id, origin })),
      normalizedTimes: animatedCapture.manifest.samples.map((sample) => sample.normalizedTime),
      independentInstances: surface.animatedMeshPlayback(renderHandle(105)).selectedClip === 'idle'
        && surface.animatedMeshPlayback(renderHandle(111)).selectedClip === 'run'
        && surface.animatedMeshPlayback(renderHandle(105)).mixerTimeSeconds
          !== surface.animatedMeshPlayback(renderHandle(111)).mixerTimeSeconds,
    },
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
    defaultLightingReadout: surface.lightingReadout(),
    visibilityReadout,
    authoredLightingReadout: lightingSurface.lightingReadout(),
    authoredLightingRejected: {
      applied: rejectedLighting.applied,
      diagnostic: rejectedLighting.diagnostics[0]?.code ?? null,
      retainedLightCount: lightingSurface.lightingReadout().retainedLights.length,
    },
    rejectedMountCleanup,
    particleReadout: particleSink.readout(),
    particlePerformance,
    pickHandle: pick.hint?.handle ?? null,
    presentationDiagnostics: presentation.diagnostics.map((diagnostic) => diagnostic.code),
    projectionInsideViewport: projected.insideViewport,
    ready: true,
    rendererBufferPixelRatio: [
      canvas.width / canvas.clientWidth,
      canvas.height / canvas.clientHeight,
    ],
    rendererStatistics: autoSubmission.statistics,
    spriteBillboardPixels: {
      initialSpherical: billboardSurfaceProofs.find(({ mode }) => mode === 'spherical')!.initialPixels,
      initialCylindrical: billboardSurfaceProofs.find(({ mode }) => mode === 'cylindrical')!.initialPixels,
      elevatedSpherical: billboardSurfaceProofs.find(({ mode }) => mode === 'spherical')!.elevatedPixels,
      elevatedCylindrical: billboardSurfaceProofs.find(({ mode }) => mode === 'cylindrical')!.elevatedPixels,
    },
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
    staticMeshTexturePixels,
    skyBackgroundPixels,
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
    voxelSurfaceAtlasPixels,
    voxelSurfaceSpecializations,
    viewComposition: {
      cameraUpdateApplied: cameraUpdateReceipt.applied,
      cameraUpdateTargetStatus,
      cameraPosition: compositionReadout.cameras
        .find((camera) => camera.id === 'camera.overview')?.pose.position ?? null,
      disposedResources,
      drawCallCount: compositionSubmission.statistics.drawCallCount.value ?? -1,
      frameReplacementApplied: compositionFrameReplacement.applied,
      frameReplacementTargetStatus,
      invalidApplied: invalidReceipt.applied,
      narrowPixels,
      pixels,
      readout: compositionReadout,
      resizeApplied: resizeReceipt.applied,
      staleApplied: staleReceipt.applied,
      staleDiagnostic: staleReceipt.diagnostics[0]?.code ?? null,
    },
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
    for (const { proofCanvas, proofSurface } of billboardSurfaceProofs) {
      proofSurface.dispose();
      proofCanvas.remove();
    }
    for (const { proofCanvas, proofSurface } of voxelSurfaceProofs) {
      proofSurface.dispose();
      proofCanvas.remove();
    }
    staticMeshTextureSurface.dispose();
    staticMeshTextureCanvas.remove();
    skySurface.dispose();
    skyCanvas.remove();
    lightingSurface.dispose();
    lightingCanvas.remove();
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

async function measureParticlePerformance(
  surface: RendererSurface,
  overlays: HTMLElement,
  spriteUrl: string,
): Promise<readonly ParticlePerformanceSample[]> {
  const samples: ParticlePerformanceSample[] = [];
  const counts = [64, 512, 4_096] as const;
  const modes = ['domBillboard', 'threeBillboard', 'instancedCube'] as const;
  let submissionTime = 10_000;

  for (const count of counts) {
    for (const mode of modes) {
      const sink: RendererParticleSink & {
        readout(): RendererParticleSinkReadout;
        dispose(): void;
      } = mode === 'domBillboard'
        ? new RendererDomParticleBillboardSink({
            container: overlays,
            projectWorld: (position) => ({
              depth: position[2],
              insideViewport: true,
              xPixels: 320 + position[0],
              yPixels: 180 - position[1],
            }),
          })
        : surface.createParticleSink();
      const visual = mode === 'instancedCube'
        ? { kind: 'cube' as const }
        : { kind: 'billboard' as const, frameCount: 1, spriteUrl };
      const baselineDrawCalls = surface.renderOnce(submissionTime += 1)
        .statistics.drawCallCount.value;
      const createStarted = performance.now();
      for (let index = 0; index < count; index += 1) {
        sink.create({
          id: index + 1,
          position: [(index % 64) * 0.05, Math.floor(index / 64) * 0.05, -5],
          size: 0.08,
          color: [0.3, 0.8, 1, 1],
          frameIndex: 0,
          visual,
        });
      }
      const createMs = performance.now() - createStarted;
      const active = sink.readout();
      if (mode === 'threeBillboard') await waitAnimationFrames(2);
      if (mode === 'domBillboard') void overlays.offsetHeight;
      let activeDrawCalls = surface.renderOnce(submissionTime += 1)
        .statistics.drawCallCount.value;
      const frameStarted = performance.now();
      const simulatedFrames = 8;
      for (let frame = 1; frame <= simulatedFrames; frame += 1) {
        for (let index = 0; index < count; index += 1) {
          sink.update({
            id: index + 1,
            position: [
              (index % 64) * 0.05,
              Math.floor(index / 64) * 0.05 - frame * 0.01,
              -5,
            ],
            size: 0.08 - frame * 0.004,
            color: [0.3, 0.8, 1, 1 - frame / (simulatedFrames + 1)],
            frameIndex: 0,
            visual,
          });
        }
        if (mode === 'domBillboard') {
          void overlays.offsetHeight;
        }
        activeDrawCalls = surface.renderOnce(submissionTime += 1).statistics.drawCallCount.value;
      }
      const averageUpdateAndRenderMs =
        (performance.now() - frameStarted) / simulatedFrames;
      const teardownStarted = performance.now();
      for (let index = 0; index < count; index += 1) sink.destroy(index + 1);
      sink.dispose();
      const teardownMs = performance.now() - teardownStarted;
      const afterTeardown = sink.readout();
      samples.push({
        mode,
        count,
        simulatedFrames,
        createMs,
        averageUpdateAndRenderMs,
        teardownMs,
        drawCallDelta: baselineDrawCalls === null || activeDrawCalls === null
          ? null
          : activeDrawCalls - baselineDrawCalls,
        active,
        afterTeardown,
      });
    }
  }
  return samples;
}

async function waitAnimationFrames(count: number): Promise<void> {
  for (let index = 0; index < count; index += 1) {
    await new Promise<void>((resolve) => globalThis.requestAnimationFrame(() => resolve()));
  }
}

async function decodeBrowserImage(url: string): Promise<void> {
  const image = new Image();
  image.src = url;
  await image.decode();
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
        domain: 'particle',
        meta: { sequence: 4 },
        op: {
          op: 'emit',
          signalId: 'browser-proof-cubes',
          descriptor: {
            anchor: { kind: 'world', position: [1, 1.4, -5] },
            visual: { kind: 'cube' },
            ratePerSecond: 0,
            burstCount: 2,
            lifetimeSeconds: [2, 2],
            velocityMin: [-0.5, -8, -0.5],
            velocityMax: [0.5, -7, 0.5],
            acceleration: [0, -4, 0],
            sizeCurve: [{ age: 0, value: 0.18 }, { age: 1, value: 0.05 }],
            colorCurve: [
              { age: 0, color: [0.35, 0.8, 1, 1] },
              { age: 1, color: [0.1, 0.3, 1, 0] },
            ],
            flipbookFramesPerSecond: 0,
            seed: 8,
            maxParticles: 4,
            visible: true,
            collision: {
              radius: 0.09,
              restitution: 0.45,
              friction: 0.2,
              maximumImpacts: 4,
              sleepSpeed: 0.15,
              limitBehavior: 'sleep',
              volumes: [{ kind: 'plane', normal: [0, 1, 0], offset: -0.35 }],
            },
          },
        },
      },
      {
        domain: 'telemetryOverlay',
        meta: { sequence: 5 },
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

function rigForFixtureClip(scene: THREE.Object3D, clip: THREE.AnimationClip): AnimationRigSignature {
  const bones = new Map<string, THREE.Bone>();
  scene.traverse((node) => {
    if (node instanceof THREE.Bone) bones.set(node.name, node);
  });
  const required = new Set<string>();
  for (const track of clip.tracks) {
    const nodeName = THREE.PropertyBinding.parseTrackName(track.name).nodeName;
    if (!nodeName || !bones.has(nodeName)) {
      throw new Error(`fixture clip contains an unresolved joint track ${track.name}`);
    }
    let current: THREE.Bone | undefined = bones.get(nodeName);
    while (current) {
      required.add(current.name);
      current = current.parent instanceof THREE.Bone ? current.parent : undefined;
    }
  }
  const joints = [...required].sort().map((id) => {
    const bone = bones.get(id);
    if (!bone) throw new Error(`fixture rig lost required joint ${id}`);
    const parent = bone.parent instanceof THREE.Bone && required.has(bone.parent.name)
      ? bone.parent.name
      : null;
    return { id, parent };
  });
  const roots = joints.filter((joint) => joint.parent === null);
  if (roots.length !== 1) throw new Error('fixture clip must resolve to exactly one root joint');
  return {
    joints,
    bindRestHash: animationRigFingerprint(scene),
    bindRestConvention: 'localMatrixV1',
    rootConvention: 'authoredRootTranslation',
    rootJointId: roots[0]!.id,
  };
}

function browserFrame(clipPackRig: AnimationRigSignature): RenderFrameDiff {
  return {
    schemaVersion: 1,
    ops: [
      {
        op: 'defineTexture',
        texture: {
          id: 'texture/browser-proof', width: 2, height: 1, filter: 'nearest',
          wrap: 'repeat',
          contentHash: 'sha256:a58d5395a03945e56638dba7ae6158b2fdaf013610a798c059a6d88231a052ae',
          version: 1,
          payload: {
            encoding: 'pngRgba8',
            colorSpace: 'srgb',
            contentHash: 'sha256:a58d5395a03945e56638dba7ae6158b2fdaf013610a798c059a6d88231a052ae',
            byteLength: 72,
            source: {
              kind: 'inline',
              encodedBytes: [
                137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82,
                0, 0, 0, 2, 0, 0, 0, 1, 8, 6, 0, 0, 0, 244, 34, 127, 138, 0,
                0, 0, 15, 73, 68, 65, 84, 120, 156, 99, 248, 207, 0, 68, 255, 25,
                26, 0, 16, 121, 3, 126, 153, 113, 48, 89, 0, 0, 0, 0, 73, 69, 78,
                68, 174, 66, 96, 130,
              ],
            },
          },
        },
      },
      {
        op: 'defineMaterial',
        material: {
          schemaVersion: 3,
          id: 'material/browser-proof',
          color: [0.25, 0.7, 0.9, 1],
          texture: 'texture/browser-proof',
          roughness: 0.8,
          textureTint: [1, 1, 1, 1],
          emissionColor: [0, 0, 0],
          emissionIntensity: 0,
          uvStrategy: 'atlas',
          voxelSurface: {
            schemaVersion: 1,
            filter: 'nearest',
            wrap: 'repeat',
            alphaMode: { kind: 'opaque' },
            mapping: {
              kind: 'repeat',
              texture: 'texture/browser-proof',
              textureVersion: 1,
              textureContentHash: 'sha256:a58d5395a03945e56638dba7ae6158b2fdaf013610a798c059a6d88231a052ae',
              tileScaleCells: [1, 1],
              tileOriginCells: [0, 0],
            },
          },
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
            { id: 'run', name: 'run', durationSeconds: null },
            { id: 'jump', name: 'jump', durationSeconds: null },
          ],
          clipPacks: [{
            asset: 'animation-clip-pack/kenney-retro-character-idle',
            runtimeFormat: 'glb',
            contentHash: CONTENT_HASH,
            rig: clipPackRig,
            clips: [{ id: 'idle', name: 'idle', durationSeconds: null }],
            provenance: {
              producer: 'rusty-engine-render-fixture',
              sourceHash: CONTENT_HASH,
              targetHash: CONTENT_HASH,
              license: 'CC0-1.0',
            },
          }],
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
            clip: 'run',
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

function voxelSurfaceBrowserFrame(orientation: 'standard' | 'rotated'): RenderFrameDiff {
  const contentHash = 'sha256:8c599da0e9d37d07bb7b917fc111a0351c09423022e802b23014377d9261be50';
  const suffix = orientation === 'standard' ? 'standard' : 'rotated';
  const tileCoordinates = orientation === 'standard'
    ? [-1,-1, 1,-1, 1,1, -1,1]
    : [1,-1, 1,1, -1,1, -1,-1];
  const tileScaleCells: readonly [number, number] = orientation === 'standard'
    ? [1, 1]
    : [0.5, 2];
  const tileOriginCells: readonly [number, number] = orientation === 'standard'
    ? [0, 0]
    : [0.25, -0.5];
  return {
    schemaVersion: 1,
    ops: [
      {
        op: 'defineTexture',
        texture: {
          id: `texture/voxel-atlas-proof-${suffix}`,
          width: 6,
          height: 6,
          filter: 'nearest',
          wrap: 'clamp',
          contentHash,
          version: 1,
          payload: {
            encoding: 'pngRgba8',
            colorSpace: 'srgb',
            contentHash,
            byteLength: 114,
            source: {
              kind: 'inline',
              encodedBytes: [
                137,80,78,71,13,10,26,10,0,0,0,13,73,72,68,82,0,0,0,6,0,0,0,6,
                8,6,0,0,0,224,204,239,72,0,0,0,57,73,68,65,84,120,156,117,139,193,
                13,0,32,8,3,25,141,209,58,154,155,157,5,19,195,67,129,54,192,65,56,
                248,40,176,211,233,214,195,4,93,160,246,1,82,168,46,117,216,248,72,
                175,197,42,55,188,224,165,13,219,0,62,8,51,133,226,122,0,0,0,0,73,
                69,78,68,174,66,96,130,
              ],
            },
          },
        },
      },
      {
        op: 'defineMaterial',
        material: {
          schemaVersion: 3,
          id: `material/voxel-atlas-proof-${suffix}`,
          color: [1, 1, 1, 1],
          texture: `texture/voxel-atlas-proof-${suffix}`,
          roughness: 1,
          textureTint: [1, 1, 1, 1],
          emissionColor: [0, 0, 0],
          emissionIntensity: 0,
          uvStrategy: 'atlas',
          voxelSurface: {
            schemaVersion: 1,
            filter: 'nearest',
            wrap: 'clamp',
            alphaMode: { kind: 'opaque' },
            mapping: {
              kind: 'atlas',
              atlas: `sprite-sheet/voxel-atlas-proof-${suffix}`,
              atlasVersion: 1,
              atlasContentHash: 'atlas-proof',
              texture: `texture/voxel-atlas-proof-${suffix}`,
              textureVersion: 1,
              textureContentHash: contentHash,
              region: {
                id: 'red',
                contentMin: [1, 1],
                contentExtent: [4, 4],
                padding: { left: 1, right: 1, bottom: 1, top: 1 },
                inset: 'halfTexel',
              },
              tileScaleCells,
              tileOriginCells,
            },
          },
        },
      },
      {
        op: 'defineStaticMesh',
        asset: {
          asset: `mesh/voxel-atlas-proof-${suffix}`,
          payload: {
            layout: {
              vertexCount: 4,
              indexCount: 6,
              indexWidth: 'u32',
              attributes: [
                { name: 'position', components: 3, kind: 'f32' },
                { name: 'normal', components: 3, kind: 'f32' },
                { name: 'uv', components: 2, kind: 'f32' },
              ],
            },
            groups: [{ materialSlot: 0, start: 0, count: 6 }],
            bounds: { min: [-1, -1, 0], max: [1, 1, 0] },
            source: {
              kind: 'inline',
              positions: [-1,-1,0, 1,-1,0, 1,1,0, -1,1,0],
              normals: [0,0,1, 0,0,1, 0,0,1, 0,0,1],
              uvs: tileCoordinates,
              indices: [0,1,2, 0,2,3],
            },
            provenance: 'voxelChunk',
          },
          materialSlots: [{ slot: 0, material: `material/voxel-atlas-proof-${suffix}` }],
          collision: { kind: 'visualOnly' },
        },
      },
      {
        op: 'createStaticMeshInstance',
        handle: renderHandle(1),
        parent: null,
        instance: {
          asset: `mesh/voxel-atlas-proof-${suffix}`,
          transform: identity([0, 0, 0], [1, 1, 1]),
          visible: true,
          materialOverrides: [],
          metadata: metadata(`voxel-atlas-visible-proof-${suffix}`),
        },
      },
    ],
  };
}

function billboardBrowserFrame(mode: 'spherical' | 'cylindrical'): RenderFrameDiff {
  // Asymmetric upright PNG: red top row, green bottom row. The visible browser
  // proof catches any recurrence of the former bottom-up sprite convention.
  const contentHash = 'sha256:51ab9cdbe436375f510ed5b05fd7106c2e6518d6279bb1e043d4bd9e100692f5';
  const suffix = mode === 'spherical' ? 'spherical' : 'cylindrical';
  const texture = `texture/billboard-proof-${suffix}`;
  const atlas = `sprite/billboard-proof-${suffix}`;
  return {
    schemaVersion: 1,
    ops: [
      {
        op: 'defineTexture',
        texture: {
          id: texture,
          width: 2,
          height: 2,
          filter: 'nearest',
          wrap: 'clamp',
          contentHash,
          version: 1,
          payload: {
            encoding: 'pngRgba8',
            colorSpace: 'srgb',
            contentHash,
            byteLength: 74,
            source: {
              kind: 'inline',
              encodedBytes: [
                137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82,
                0, 0, 0, 2, 0, 0, 0, 2, 8, 6, 0, 0, 0, 114, 182, 13, 36, 0,
                0, 0, 17, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240, 31,
                132, 65, 8, 12, 1, 69, 204, 7, 249, 202, 39, 25, 207, 0, 0, 0,
                0, 73, 69, 78, 68, 174, 66, 96, 130,
              ],
            },
          },
        },
      },
      {
        op: 'defineSpriteAtlas',
        atlas: {
          id: atlas,
          texture,
          frames: [{ frame: 0, uvMin: [0, 0], uvMax: [1, 1] }],
        },
      },
      {
        op: 'createSprite',
        handle: renderHandle(1),
        parent: null,
        sprite: {
          asset: atlas,
          frame: 0,
          pivot: [0.5, 0.5],
          size: [2, 2],
          sizeMode: 'world',
          billboard: mode,
          tint: [1, 1, 1, 1],
          renderOrder: 0,
          depth: 'default',
          shading: 'unlit',
          visible: true,
          transform: identity([0, 0, 0], [1, 1, 1]),
          attachment: { sourceEntity: null, sourceSceneNode: null, attachmentPoint: null },
          metadata: metadata(`billboard-${suffix}`),
        },
      },
    ],
  };
}

function readBillboardPixels(
  context: WebGLRenderingContext | WebGL2RenderingContext,
): readonly (readonly [number, number, number, number])[] {
  return Array.from({ length: 1024 }, (_, index) => {
    const x = index % 32;
    const y = Math.floor(index / 32);
    const pixel = new Uint8Array(4);
    context.readPixels(
      Math.floor(context.drawingBufferWidth * (x + 0.5) / 32),
      Math.floor(context.drawingBufferHeight * (y + 0.5) / 32),
      1,
      1,
      context.RGBA,
      context.UNSIGNED_BYTE,
      pixel,
    );
    return [...pixel] as [number, number, number, number];
  });
}

function staticMeshTextureBrowserFrame(): RenderFrameDiff {
  const contentHash = 'sha256:a58d5395a03945e56638dba7ae6158b2fdaf013610a798c059a6d88231a052ae';
  return {
    schemaVersion: 1,
    ops: [
      {
        op: 'defineTexture',
        texture: {
          id: 'texture/static-mesh-uv-proof',
          width: 2,
          height: 1,
          filter: 'nearest',
          wrap: 'clamp',
          contentHash,
          version: 1,
          payload: {
            encoding: 'pngRgba8',
            colorSpace: 'srgb',
            contentHash,
            byteLength: 72,
            source: {
              kind: 'inline',
              encodedBytes: [
                137,80,78,71,13,10,26,10,0,0,0,13,73,72,68,82,0,0,0,2,0,0,0,1,
                8,6,0,0,0,244,34,127,138,0,0,0,15,73,68,65,84,120,156,99,248,
                207,0,68,255,25,26,0,16,121,3,126,153,113,48,89,0,0,0,0,73,69,
                78,68,174,66,96,130,
              ],
            },
          },
        },
      },
      {
        op: 'defineMaterial',
        material: {
          schemaVersion: 3,
          id: 'material/static-mesh-uv-proof',
          color: [1, 1, 1, 1],
          texture: 'texture/static-mesh-uv-proof',
          roughness: 1,
          textureTint: [1, 1, 1, 1],
          emissionColor: [0, 0, 0],
          emissionIntensity: 0,
          uvStrategy: 'planar',
        },
      },
      {
        op: 'defineStaticMesh',
        asset: {
          asset: 'mesh/static-mesh-uv-proof',
          payload: {
            layout: {
              vertexCount: 4,
              indexCount: 6,
              indexWidth: 'u32',
              attributes: [
                { name: 'position', components: 3, kind: 'f32' },
                { name: 'normal', components: 3, kind: 'f32' },
                { name: 'uv', components: 2, kind: 'f32' },
              ],
            },
            groups: [{ materialSlot: 0, start: 0, count: 6 }],
            bounds: { min: [-1, -0.5, 0], max: [1, 0.5, 0] },
            source: {
              kind: 'inline',
              positions: [-1,-0.5,0, 1,-0.5,0, 1,0.5,0, -1,0.5,0],
              normals: [0,0,1, 0,0,1, 0,0,1, 0,0,1],
              uvs: [0,0, 1,0, 1,1, 0,1],
              indices: [0,1,2, 0,2,3],
            },
            provenance: 'staticAsset',
          },
          materialSlots: [{ slot: 0, material: 'material/static-mesh-uv-proof' }],
          collision: { kind: 'visualOnly' },
        },
      },
      {
        op: 'createStaticMeshInstance',
        handle: renderHandle(1),
        parent: null,
        instance: {
          asset: 'mesh/static-mesh-uv-proof',
          transform: identity([0, 0, 0], [1, 1, 1]),
          visible: true,
          materialOverrides: [],
          metadata: metadata('static-mesh-uv-visible-proof'),
        },
      },
    ],
  };
}

function viewComposition(
  revision: number,
  targetSize: number,
  overviewZ = revision === 1 ? 8 : 7.5,
): RendererViewComposition {
  return {
    schemaVersion: 1,
    cameras: [
      {
        id: 'camera.front-inset',
        pose: { position: [0, 2, 8], pitchDegrees: -8, yawDegrees: 0 },
        projection: { kind: 'perspective', fovYDegrees: 55, near: 0.1, far: 50 },
      },
      {
        id: 'camera.overview',
        pose: { position: [0, 3, overviewZ], pitchDegrees: -10, yawDegrees: 0 },
        projection: { kind: 'perspective', fovYDegrees: 58, near: 0.1, far: 50 },
      },
    ],
    targets: [{
      id: 'target.overview', revision, width: targetSize, height: targetSize,
      color: 'rgba8_srgb', depth: 'depth24', sampling: 'nearest',
    }],
    views: [
      {
        id: 'view.front-inset', cameraId: 'camera.front-inset', order: 10,
        target: { kind: 'primary' },
        viewport: { x: 0.03, y: 0.04, width: 0.34, height: 0.35 },
      },
      {
        id: 'view.overview', cameraId: 'camera.overview', order: 20,
        target: { kind: 'offscreen', targetId: 'target.overview', targetRevision: revision },
        viewport: { x: 0, y: 0, width: 1, height: 1 },
      },
    ],
    presentations: [{
      id: 'presentation.overview', sourceTargetId: 'target.overview',
      sourceTargetRevision: revision, order: 30,
      destination: {
        kind: 'primary',
        viewport: { x: 0.58, y: 0.52, width: 0.38, height: 0.42 },
      },
    }],
  };
}

function viewCompositionFrame(): RenderFrameDiff {
  return {
    schemaVersion: 1,
    ops: [
      {
        op: 'create', handle: renderHandle(1), parent: null,
        node: {
          geometry: { kind: 'cube' },
          material: { color: [1, 0.04, 0.02, 1], wireframe: false },
          transform: identity([-2, 1, -5], [3, 3, 3]), visible: true, layer: 'scene',
          metadata: metadata('composition-red'),
        },
      },
      {
        op: 'create', handle: renderHandle(2), parent: null,
        node: {
          geometry: { kind: 'cube' },
          material: { color: [0.02, 1, 0.04, 1], wireframe: false },
          transform: identity([2, 1, -5], [3, 3, 3]), visible: true, layer: 'scene',
          metadata: metadata('composition-green'),
        },
      },
    ],
  };
}

function readCompositionPixels(
  context: WebGLRenderingContext | WebGL2RenderingContext,
): readonly (readonly [number, number, number, number])[] {
  const positions = [
    [0.69, 0.71],
    [0.8, 0.71],
    [0.35, 0.45],
    [0.55, 0.45],
  ] as const;
  return positions.map(([x, y]) => {
    const pixel = new Uint8Array(4);
    context.readPixels(
      Math.floor(context.drawingBufferWidth * x),
      Math.floor(context.drawingBufferHeight * y),
      1,
      1,
      context.RGBA,
      context.UNSIGNED_BYTE,
      pixel,
    );
    return [...pixel] as [number, number, number, number];
  });
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
        { name: 'uv', components: 2, kind: 'f32' },
      ],
    },
    groups: [{ materialSlot: 0, start: 0, count: 3 }],
    bounds: { min: [-0.5, -0.5, 0], max: [0.5, 0.5, 0] },
    source: {
      kind: 'inline', positions: [-0.5, -0.5, 0, 0.5, -0.5, 0, 0, 0.5, 0],
      normals: [0, 0, 1, 0, 0, 1, 0, 0, 1],
      uvs: [0, 0, 2, 0, 1, 1],
      indices: [0, 1, 2],
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
