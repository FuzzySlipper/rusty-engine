// Explicit browser/canvas composition over the renderer-neutral projection and Three backend.

import type {
  CameraBasis,
  PerspectiveProjection,
  PresentationFrameDiff,
  RenderFrameDiff,
  RenderHandle,
  RenderLayer,
  RendererViewComposition,
} from '@rusty-engine/render-contracts';
import {
  RenderProjection,
  type RenderProjectionInstruction,
  type RenderProjectionSnapshot,
} from '@rusty-engine/render-projection';
import {
  createRendererBrowserSurfaceFrame,
  mountRendererBrowserSurface,
  RendererLightingPolicyError,
  type AnimatedMeshAssetSource,
  type RendererBrowserSurface,
  type RendererBrowserSurfacePickDiagnostic,
  type RendererBrowserSurfaceSubmissionStatistics,
} from '@rusty-engine/renderer-three/backend';
import {
  animationPlaybackReadout,
  loadRendererAnimatedMeshSource,
  type RendererAnimatedMeshFrameReceipt,
  type RendererAnimatedMeshPlaybackReadout,
  type RendererAnimatedMeshProjection,
  type RendererAnimatedMeshResourceManifest,
  type RendererAnimatedMeshResourceResolver,
} from './animated-mesh-host.js';
import {
  RendererPresentationHostSet,
  type RendererPresentationFrameReceipt,
} from './presentation-host-set.js';
import {
  assertRendererSurfaceSourceTime,
  RendererSurfaceTimingTracker,
  type RendererSurfaceTimingSample,
  type RendererSurfaceTimingSource,
} from './surface-timing.js';
import {
  createRendererSurfaceSubmissionSample,
  type RendererSurfaceSubmissionSample,
} from './surface-statistics.js';
import {
  RendererSurfaceAutomaticSubmissionAdmissionObservation,
  type RendererSurfaceAutomaticSubmissionAdmissionSample,
  type RendererSurfaceAutomaticSubmissionCallbackPhases,
} from './surface-admission-observation.js';
import {
  RendererSurfaceSubmissionDemand,
  type RendererSurfaceViewportState,
} from './surface-submission-demand.js';

export const RUSTY_RENDERER_HOST_COMPATIBILITY_VERSION = 'renderer-host.v1';
export const RUSTY_RENDERER_SURFACE_LIGHTING_SCHEMA_VERSION = 1;
export const RUSTY_RENDERER_SURFACE_MAX_ACTIVE_SHADOW_LIGHTS = 8;

export type RendererSurfaceDefaultLightingMode = 'neutral' | 'disabled';

export interface RendererSurfaceLightingOptions {
  readonly schemaVersion: 1;
  readonly defaultLights?: {
    readonly world?: RendererSurfaceDefaultLightingMode;
    readonly viewmodel?: RendererSurfaceDefaultLightingMode;
  };
  readonly shadows?: {
    readonly enabled?: boolean;
    readonly maximumActiveLights?: number;
  };
}

export interface RendererSurfaceLightingReadout {
  readonly schemaVersion: 1;
  readonly defaultLights: {
    readonly world: RendererSurfaceDefaultLightingMode;
    readonly viewmodel: RendererSurfaceDefaultLightingMode;
  };
  readonly neutralLightCounts: { readonly world: number; readonly viewmodel: number };
  readonly shadows: {
    readonly enabled: boolean;
    readonly maximumActiveLights: number;
    readonly activeLights: number;
    readonly requestedUnsupportedLights: number;
  };
  readonly retainedLights: ReturnType<RendererBrowserSurface['lightingReadout']>['retainedLights'];
}

export class RendererSurfaceLightingError extends Error {
  readonly code = 'invalid_lighting_policy' as const;

  constructor(message: string) {
    super(message);
    this.name = 'RendererSurfaceLightingError';
  }
}

export type RendererBackendFamily = 'threejs';

export interface RendererBackendDiagnostics {
  readonly family: RendererBackendFamily;
  readonly implementation: 'rusty-engine-renderer-backend';
  readonly publicContract: 'rusty-renderer-surface.v1';
}

export type RendererSurfaceAutomaticSubmissionPacingMode =
  | 'completionOnly'
  | 'timerFailed'
  | 'timerQuery';

export type RendererSurfaceAutomaticSubmissionPacingState =
  | 'disposed'
  | 'idle'
  | 'measuring'
  | 'ready'
  | 'waiting';

export type RendererSurfaceAutomaticSubmissionClass =
  | 'accelerated'
  | 'software'
  | 'unknown';

/** Renderer-owned observation of the latest automatic admission decision. */
export interface RendererSurfaceAutomaticSubmissionPacingSample {
  readonly schemaVersion: 1;
  readonly mode: RendererSurfaceAutomaticSubmissionPacingMode;
  readonly state: RendererSurfaceAutomaticSubmissionPacingState;
  readonly rendererClass: RendererSurfaceAutomaticSubmissionClass;
  readonly timerDurationMs: number | null;
  readonly completionAgeMs: number | null;
  readonly completionAllowanceMs: number;
  readonly effectiveDurationMs: number | null;
  readonly targetDutyFraction: number | null;
  readonly admittedAtMs: number | null;
  readonly admissionObservedAtMs: number | null;
  readonly observedAtMs: number | null;
  readonly automaticSubmissionCapacity: number;
  readonly automaticSubmissionLimit: number;
  readonly completionFenceMode: 'active' | 'disabled' | 'unsupported';
  readonly maximumPendingSubmissions: number;
  readonly pendingSubmissionCount: number;
  readonly maximumPendingMeasurements: number;
  readonly pendingMeasurementCount: number;
  readonly hostAdmission: RendererSurfaceAutomaticSubmissionAdmissionSample;
}

export interface RendererSurfaceOptions {
  readonly autoStart?: boolean;
  readonly clearColor?: number;
  readonly controls?: RendererSurfaceControlsOptions;
  readonly frame?: RenderFrameDiff;
  readonly lighting?: RendererSurfaceLightingOptions;
  readonly meshBufferSource?: RendererSurfaceMeshBufferSource;
  readonly pixelRatio?: number;
  readonly presentationHosts?: RendererPresentationHostSet;
  readonly projection?: PerspectiveProjection;
  readonly viewComposition?: RendererViewComposition;
}

export interface RendererSurfaceMeshBufferSource {
  readonly acquireBuffer: (handle: number) => { readonly bytes: Uint8Array };
  readonly releaseBuffer: (handle: number) => void;
}

export interface RendererAnimatedMeshSurfaceOptions extends RendererSurfaceOptions {
  readonly animatedMeshManifest: RendererAnimatedMeshResourceManifest;
  readonly resolveAnimatedMeshResource: RendererAnimatedMeshResourceResolver;
}

export interface RendererSurfaceControlsOptions {
  /** Controls are opt-in so mounting a renderer never captures input implicitly. */
  readonly enabled?: boolean;
  readonly eyeHeight?: number;
  readonly initialPitchDegrees?: number;
  readonly initialPosition?: readonly [number, number, number];
  readonly initialYawDegrees?: number;
  readonly mouseSensitivity?: number;
  readonly moveSpeed?: number;
  /** Optional caller-owned collision or movement constraint. */
  readonly resolveMovement?: RendererSurfaceMovementResolver;
}

export interface RendererSurfaceCameraPose {
  readonly position: readonly [number, number, number];
  readonly pitchDegrees: number;
  readonly yawDegrees: number;
}

export type RendererSurfaceCameraBasis = CameraBasis;

export interface RendererSurfaceMovementInput {
  readonly deltaSeconds: number;
  readonly moveForward: number;
  readonly moveRight: number;
  readonly moveSpeedUnitsPerSecond: number;
  readonly pitchDeltaDegrees: number;
  readonly poseBefore: RendererSurfaceCameraPose;
  readonly sequence: number;
  readonly yawDeltaDegrees: number;
}

export interface RendererSurfaceMovementResult {
  readonly basis?: RendererSurfaceCameraBasis;
  readonly blockedAxes?: readonly string[];
  readonly collided?: boolean;
  readonly pose: RendererSurfaceCameraPose;
  readonly resolutionId?: string | null;
}

export type RendererSurfaceMovementResolver = (
  input: RendererSurfaceMovementInput,
) => RendererSurfaceMovementResult;

export interface RendererSurfaceMovementState {
  readonly mode: 'free_camera' | 'caller_resolved';
  readonly blockedAxes: readonly string[];
  readonly collided: boolean;
  readonly resolutionId: string | null;
}

export interface RendererSurfaceInputReadout {
  readonly enabled: boolean;
  readonly pointerLocked: boolean;
  readonly pressedCodes: readonly string[];
}

export type RendererSurfaceVec3 = readonly [number, number, number];

export interface RendererSurfaceWorldProjection {
  readonly xPixels: number;
  readonly yPixels: number;
  readonly depth: number;
  readonly distance: number;
  readonly insideViewport: boolean;
  readonly occluded: false;
}

export type RendererSurfacePickRay =
  | {
      readonly kind: 'viewport';
      readonly point: readonly [number, number];
    }
  | {
      readonly kind: 'world_ray';
      readonly direction: RendererSurfaceVec3;
      readonly origin: RendererSurfaceVec3;
    };

export interface RendererSurfacePickFilter {
  readonly handles?: readonly RenderHandle[];
  readonly labels?: readonly string[];
  readonly layers?: readonly RenderLayer[];
  readonly tags?: readonly string[];
}

export interface RendererSurfacePickRequest {
  readonly filter?: RendererSurfacePickFilter;
  readonly maxDistance?: number;
  readonly ray: RendererSurfacePickRay;
}

export interface RendererSurfacePickHint {
  readonly channel: 'render_projection';
  readonly distance: number;
  readonly handle: RenderHandle;
  readonly label: string | null;
  readonly layer: RenderLayer;
  readonly normal: RendererSurfaceVec3;
  readonly position: RendererSurfaceVec3;
  readonly sourceTrace: {
    readonly entity: number;
    readonly kind: 'render_metadata_entity';
  } | null;
  readonly tags: readonly string[];
}

export interface RendererSurfacePickReceipt {
  readonly diagnostics: readonly RendererBrowserSurfacePickDiagnostic[];
  readonly hint: RendererSurfacePickHint | null;
  readonly kind: 'rusty_renderer_surface_pick.v1';
}

export interface RendererSurfaceProjectionReceipt {
  readonly instructions: readonly RenderProjectionInstruction[];
  readonly snapshot: RenderProjectionSnapshot;
}

export type RendererAnimatedMeshSampleDiagnosticCode =
  | 'bone_matrix_non_finite'
  | 'bone_matrix_singular'
  | 'node_quaternion_invalid'
  | 'node_scale_invalid'
  | 'node_transform_non_finite'
  | 'sampled_bounds_implausible'
  | 'vertex_budget_exceeded';

export interface RendererAnimatedMeshSampleReadout {
  readonly handle: RenderHandle;
  readonly asset: string;
  readonly contentHash: string | null;
  readonly clip: string;
  readonly normalizedTime: number;
  readonly durationSeconds: number;
  readonly assetBounds: {
    readonly min: readonly [number, number, number];
    readonly max: readonly [number, number, number];
  };
  readonly sampledWorldBounds: {
    readonly min: readonly [number, number, number];
    readonly max: readonly [number, number, number];
  } | null;
  readonly sampledVertexCount: number;
  readonly boneCount: number;
  readonly skinningFacts: {
    readonly joints: readonly {
      readonly name: string;
      readonly parent: string | null;
      readonly restLocalMatrix: readonly number[];
      readonly inverseBindMatrix: readonly number[] | null;
    }[];
    readonly skinnedMeshCount: number;
    readonly inverseBindMatrixCount: number;
    readonly inverseBindMatricesFinite: boolean;
    readonly weightedVertexCount: number;
    readonly invalidWeightVertexCount: number;
    readonly maximumWeightSumError: number;
    readonly weightsNormalized: boolean;
    readonly interpolationModes: readonly ('discrete' | 'linear' | 'smooth')[];
    readonly instanceRootDistinctFromTemplate: boolean;
    readonly skeletonsIndependentFromTemplate: boolean;
    readonly sharedGeometryCount: number;
    readonly sharedMaterialCount: number;
  };
  readonly diagnostics: readonly {
    readonly code: RendererAnimatedMeshSampleDiagnosticCode;
    readonly message: string;
    readonly node: string | null;
  }[];
}

export interface RendererSurface {
  readonly kind: 'rusty_renderer_surface.v1';
  readonly backend: RendererBackendDiagnostics;
  readonly canvas: HTMLCanvasElement;
  readonly animationProjection: RendererAnimatedMeshProjection;
  readonly animatedMeshPlayback: (handle: RenderHandle) => RendererAnimatedMeshPlaybackReadout;
  readonly sampleAnimatedMesh: (
    handle: RenderHandle,
    clipId: string,
    normalizedTime: number,
  ) => RendererAnimatedMeshSampleReadout;
  readonly applyFrame: (frame: RenderFrameDiff) => RendererAnimatedMeshFrameReceipt;
  readonly applyPresentation: (
    frame: PresentationFrameDiff,
  ) => Promise<RendererPresentationFrameReceipt>;
  readonly automaticSubmissionPacing: () => RendererSurfaceAutomaticSubmissionPacingSample;
  readonly cameraPose: () => RendererSurfaceCameraPose;
  readonly cameraProjection: () => PerspectiveProjection;
  readonly inputReadout: () => RendererSurfaceInputReadout;
  readonly lightingReadout: () => RendererSurfaceLightingReadout;
  readonly configureViews: (
    composition: RendererViewComposition,
  ) => ReturnType<RendererBrowserSurface['configureViews']>;
  readonly viewCompositionReadout: () => ReturnType<RendererBrowserSurface['viewCompositionReadout']>;
  readonly lockPointer: () => void;
  readonly movementState: () => RendererSurfaceMovementState;
  readonly pick: (request: RendererSurfacePickRequest) => RendererSurfacePickReceipt;
  readonly pointerLocked: () => boolean;
  readonly projectWorldPoint: (position: RendererSurfaceVec3) => RendererSurfaceWorldProjection;
  readonly projectionSnapshot: () => RenderProjectionSnapshot;
  /** Submit one explicit frame and return its immutable renderer-owned sample. */
  readonly renderOnce: (timeMs?: number) => RendererSurfaceSubmissionSample;
  readonly resetCamera: () => void;
  /** Synchronize a caller-owned camera, such as an authoritative game player view. */
  readonly setCameraPose: (
    pose: RendererSurfaceCameraPose,
    basis?: RendererSurfaceCameraBasis,
  ) => void;
  /** Attach after mount when an animation host needs this surface's projection port. */
  readonly setPresentationHosts: (hosts: RendererPresentationHostSet | null) => void;
  readonly snapshot: () => string;
  readonly start: () => void;
  readonly stop: () => void;
  /** Read the latest automatic or explicit submission without polling another loop. */
  readonly submission: () => RendererSurfaceSubmissionSample;
  /** Read the latest automatic or explicit frame timing without starting another loop. */
  readonly timing: () => RendererSurfaceTimingSample;
  readonly dispose: () => void;
}

const THREE_BACKEND_DIAGNOSTICS: RendererBackendDiagnostics = {
  family: 'threejs',
  implementation: 'rusty-engine-renderer-backend',
  publicContract: 'rusty-renderer-surface.v1',
};

export function createRendererSurfaceProjection(
  frame: RenderFrameDiff,
): RendererSurfaceProjectionReceipt {
  const projection = new RenderProjection();
  const instructions = projection.applyFrame(frame);
  return { instructions, snapshot: projection.snapshot() };
}

export function createRendererDefaultSurfaceFrame(): RenderFrameDiff {
  return createRendererBrowserSurfaceFrame();
}

export function mountRendererSurface(
  canvas: HTMLCanvasElement,
  options: RendererSurfaceOptions = {},
): RendererSurface {
  return mountPreparedRendererSurface(canvas, options);
}

export async function mountRendererAnimatedMeshSurface(
  canvas: HTMLCanvasElement,
  options: RendererAnimatedMeshSurfaceOptions,
): Promise<RendererSurface> {
  const source = await loadRendererAnimatedMeshSource(
    options.animatedMeshManifest,
    options.resolveAnimatedMeshResource,
  );
  return mountPreparedRendererSurface(
    canvas,
    options,
    source,
    contentHashesByAsset(options.animatedMeshManifest),
  );
}

function mountPreparedRendererSurface(
  canvas: HTMLCanvasElement,
  options: RendererSurfaceOptions,
  animatedMeshSource?: AnimatedMeshAssetSource,
  contentHashes: ReadonlyMap<string, string> = new Map(),
): RendererSurface {
  const lighting = normalizeSurfaceLighting(options.lighting);
  const frame = options.frame ?? createRendererDefaultSurfaceFrame();
  const projection = new RenderProjection();
  projection.applyFrame(frame);
  const controls = createRendererSurfaceFirstPersonControls(canvas, options.controls);
  let backendSurface: RendererBrowserSurface;
  try {
    backendSurface = mountRendererBrowserSurface(canvas, {
      autoStart: false,
      ...(animatedMeshSource === undefined ? {} : { animatedMeshSource }),
      ...(options.meshBufferSource === undefined ? {} : { meshBufferSource: options.meshBufferSource }),
      camera: {
        initialPose: controls.cameraPose(),
        ...(options.projection === undefined ? {} : { projection: options.projection }),
      },
      ...(options.clearColor === undefined ? {} : { clearColor: options.clearColor }),
      ...(options.pixelRatio === undefined ? {} : { pixelRatio: options.pixelRatio }),
      lighting,
      frame,
      ...(options.viewComposition === undefined
        ? {} : { viewComposition: options.viewComposition }),
    });
  } catch (cause) {
    controls.dispose();
    throw cause;
  }
  const animationProjection = surfaceAnimationProjection(backendSurface, contentHashes);
  let presentationHosts = options.presentationHosts ?? null;
  let animationFrame: number | null = null;
  let lastRenderTimeMs: number | null = null;
  const timing = new RendererSurfaceTimingTracker();
  let latestSubmission: RendererSurfaceSubmissionSample | null = null;
  const submissionDemand = new RendererSurfaceSubmissionDemand(surfaceViewport(canvas));
  const automaticSubmissionAdmission =
    new RendererSurfaceAutomaticSubmissionAdmissionObservation();
  let disposed = false;
  const continuousDemand = () => ({
    controls: controls.requiresAnimationFrame(),
    presentation: presentationHosts?.requiresAnimationFrame() ?? false,
    retainedAnimation: hasRetainedAnimation(latestSubmission),
  });
  const requestAutomaticSubmission = (): void => {
    submissionDemand.request();
  };

  interface RenderFrameResult {
    readonly submission: RendererSurfaceSubmissionSample;
    readonly controlsUpdatedAtMs: number;
    readonly cameraUpdatedAtMs: number;
    readonly presentationAdvancedAtMs: number;
    readonly backendSubmittedAtMs: number;
  }

  const renderFrame = (
    timeMs: number,
    source: RendererSurfaceTimingSource,
  ): RenderFrameResult => {
    if (disposed) throw new Error('renderer surface is disposed');
    assertRendererSurfaceSourceTime(timeMs);
    const deltaSeconds = lastRenderTimeMs === null
      ? 0
      : Math.min(0.05, Math.max(0, (timeMs - lastRenderTimeMs) / 1_000));
    lastRenderTimeMs = timeMs;
    controls.update(deltaSeconds);
    const controlsUpdatedAtMs = surfaceTimingNow();
    const camera = controls.cameraSnapshot();
    backendSurface.setCameraPose(camera.pose, camera.basis);
    const cameraUpdatedAtMs = surfaceTimingNow();
    presentationHosts?.advance(deltaSeconds);
    const presentationAdvancedAtMs = surfaceTimingNow();
    const backendSubmissionStartedMs = surfaceTimingNow();
    const backendStatistics = backendSurface.renderOnce(timeMs);
    const backendSubmissionEndedMs = surfaceTimingNow();
    latestSubmission = surfaceSubmissionSample(timing.record({
      source,
      sourceTimeMs: timeMs,
      backendSubmissionStartedMs,
      backendSubmissionEndedMs,
    }), backendStatistics);
    submissionDemand.submitted(surfaceViewport(canvas));
    return {
      submission: latestSubmission,
      controlsUpdatedAtMs,
      cameraUpdatedAtMs,
      presentationAdvancedAtMs,
      backendSubmittedAtMs: backendSubmissionEndedMs,
    };
  };
  const renderOnce = (
    timeMs = globalThis.performance?.now() ?? 0,
  ): RendererSurfaceSubmissionSample => {
    return renderFrame(timeMs, 'explicit').submission;
  };

  const tick = (timeMs: number): void => {
    const callbackStartedAtMs = surfaceTimingNow();
    // Register the sole successor before camera, presentation, and WebGL work.
    // Browsers can otherwise miss the next display scheduling window when the
    // callback requests its successor only after submitting the current frame.
    animationFrame = globalThis.requestAnimationFrame(tick);
    const successorQueuedAtMs = surfaceTimingNow();
    const demand = submissionDemand.consumeDecision(
      surfaceViewport(canvas),
      continuousDemand(),
    );
    const demandObservedAtMs = surfaceTimingNow();
    if (!demand.shouldSubmit) {
      const callbackEndedAtMs = surfaceTimingNow();
      automaticSubmissionAdmission.record(
        timeMs,
        'noDemand',
        demand,
        backendSurface.automaticSubmissionPacing(),
        callbackPhases({
          callbackStartedAtMs,
          successorQueuedAtMs,
          demandObservedAtMs,
          callbackEndedAtMs,
        }),
      );
    } else {
      const ready = backendSurface.automaticSubmissionReady(timeMs);
      const backendReadinessObservedAtMs = surfaceTimingNow();
      const backendPacing = backendSurface.automaticSubmissionPacing();
      if (ready) {
        const rendered = renderFrame(timeMs, 'animationFrame');
        const callbackEndedAtMs = surfaceTimingNow();
        automaticSubmissionAdmission.record(
          timeMs,
          'admitted',
          demand,
          backendPacing,
          callbackPhases({
            callbackStartedAtMs,
            successorQueuedAtMs,
            demandObservedAtMs,
            backendReadinessObservedAtMs,
            controlsUpdatedAtMs: rendered.controlsUpdatedAtMs,
            cameraUpdatedAtMs: rendered.cameraUpdatedAtMs,
            presentationAdvancedAtMs: rendered.presentationAdvancedAtMs,
            backendSubmittedAtMs: rendered.backendSubmittedAtMs,
            callbackEndedAtMs,
          }),
        );
      } else {
        const callbackEndedAtMs = surfaceTimingNow();
        automaticSubmissionAdmission.record(
          timeMs,
          'backendBlocked',
          demand,
          backendPacing,
          callbackPhases({
            callbackStartedAtMs,
            successorQueuedAtMs,
            demandObservedAtMs,
            backendReadinessObservedAtMs,
            callbackEndedAtMs,
          }),
        );
        requestAutomaticSubmission();
      }
    }
  };
  const start = (): void => {
    if (disposed) throw new Error('renderer surface is disposed');
    if (animationFrame === null) {
      animationFrame = globalThis.requestAnimationFrame(tick);
      requestAutomaticSubmission();
    }
  };
  const stop = (): void => {
    if (animationFrame !== null) {
      globalThis.cancelAnimationFrame(animationFrame);
      animationFrame = null;
    }
  };
  const applyFrame = (nextFrame: RenderFrameDiff): RendererAnimatedMeshFrameReceipt => {
    try {
      // Phase one is renderer-neutral and non-committing. ThreeRenderer performs
      // its own complete backend/resource preflight, so neither store advances
      // until both agree that the whole frame is applicable.
      projection.validateFrame(nextFrame);
      backendSurface.applyFrame(nextFrame);
      projection.applyFrame(nextFrame);
      requestAutomaticSubmission();
      return { applied: true, diagnostics: [] };
    } catch (cause) {
      return {
        applied: false,
        diagnostics: [{
          code: cause instanceof RendererLightingPolicyError
            ? 'renderer_lighting_policy_rejected'
            : 'animated_mesh_frame_rejected',
          message: cause instanceof Error ? cause.message : String(cause),
          asset: null,
          handle: null,
        }],
      };
    }
  };

  renderFrame(0, 'mount');
  if (options.autoStart !== false) {
    start();
  }

  return {
    kind: 'rusty_renderer_surface.v1',
    backend: THREE_BACKEND_DIAGNOSTICS,
    canvas,
    animationProjection,
    animatedMeshPlayback: (handle) => animationProjection.playback(handle),
    sampleAnimatedMesh: (handle, clipId, normalizedTime) =>
      backendSurface.sampleAnimatedMesh(handle, clipId, normalizedTime),
    applyFrame,
    applyPresentation: async (presentationFrame) => {
      const receipt = await (presentationHosts ?? new RendererPresentationHostSet({}))
        .apply(presentationFrame);
      if (receipt.applied > 0) {
        requestAutomaticSubmission();
      }
      return receipt;
    },
    automaticSubmissionPacing: () => Object.freeze({
      ...backendSurface.automaticSubmissionPacing(),
      hostAdmission: automaticSubmissionAdmission.sample(),
    }),
    cameraPose: controls.cameraPose,
    cameraProjection: backendSurface.cameraProjection,
    inputReadout: controls.inputReadout,
    lightingReadout: backendSurface.lightingReadout,
    configureViews: (composition) => {
      const receipt = backendSurface.configureViews(composition);
      if (receipt.applied) requestAutomaticSubmission();
      return receipt;
    },
    viewCompositionReadout: backendSurface.viewCompositionReadout,
    lockPointer: controls.lockPointer,
    movementState: controls.movementState,
    pick: (request) => {
      const receipt = backendSurface.pick(request);
      return {
        diagnostics: receipt.diagnostics,
        hint: receipt.hit,
        kind: 'rusty_renderer_surface_pick.v1',
      };
    },
    pointerLocked: controls.pointerLocked,
    projectWorldPoint: backendSurface.projectWorldPoint,
    projectionSnapshot: () => projection.snapshot(),
    renderOnce,
    resetCamera: () => {
      controls.resetCamera();
      lastRenderTimeMs = null;
      renderFrame(0, 'cameraReset');
    },
    setCameraPose: (pose, basis) => {
      const before = controls.cameraSnapshot();
      controls.setCameraPose(pose, basis);
      backendSurface.setCameraPose(pose, basis);
      if (!sameCameraSnapshot(before, controls.cameraSnapshot())) {
        requestAutomaticSubmission();
      }
    },
    setPresentationHosts: (hosts) => {
      presentationHosts = hosts;
      requestAutomaticSubmission();
    },
    snapshot: backendSurface.snapshot,
    start,
    stop,
    submission: () => {
      if (latestSubmission === null) {
        throw new Error('renderer surface has not submitted a frame');
      }
      return latestSubmission;
    },
    timing: timing.read.bind(timing),
    dispose: () => {
      if (disposed) return;
      stop();
      controls.dispose();
      backendSurface.dispose();
      disposed = true;
    },
  };
}

function surfaceSubmissionSample(
  timing: RendererSurfaceTimingSample,
  statistics: RendererBrowserSurfaceSubmissionStatistics,
): RendererSurfaceSubmissionSample {
  return createRendererSurfaceSubmissionSample(timing, {
    drawCallCount: statistics.drawCallCount,
    renderHandleCount: statistics.renderHandleCount,
    geometryResourceCount: statistics.geometryResourceCount,
    materialResourceCount: statistics.materialResourceCount,
    textureResourceCount: statistics.textureResourceCount,
    animatedInstanceCount: statistics.animatedInstanceCount,
    triangleCount: statistics.triangleCount,
  });
}

function surfaceTimingNow(): number {
  return globalThis.performance?.now() ?? 0;
}

function callbackPhases(
  input: {
    readonly callbackStartedAtMs: number;
    readonly successorQueuedAtMs: number;
    readonly demandObservedAtMs: number;
    readonly backendReadinessObservedAtMs?: number;
    readonly controlsUpdatedAtMs?: number;
    readonly cameraUpdatedAtMs?: number;
    readonly presentationAdvancedAtMs?: number;
    readonly backendSubmittedAtMs?: number;
    readonly callbackEndedAtMs: number;
  },
): RendererSurfaceAutomaticSubmissionCallbackPhases {
  return Object.freeze({
    schemaVersion: 1,
    callbackStartedAtMs: input.callbackStartedAtMs,
    successorQueuedAtMs: input.successorQueuedAtMs,
    demandObservedAtMs: input.demandObservedAtMs,
    backendReadinessObservedAtMs: input.backendReadinessObservedAtMs ?? null,
    controlsUpdatedAtMs: input.controlsUpdatedAtMs ?? null,
    cameraUpdatedAtMs: input.cameraUpdatedAtMs ?? null,
    presentationAdvancedAtMs: input.presentationAdvancedAtMs ?? null,
    backendSubmittedAtMs: input.backendSubmittedAtMs ?? null,
    callbackEndedAtMs: input.callbackEndedAtMs,
  });
}

function surfaceAnimationProjection(
  surface: RendererBrowserSurface,
  contentHashes: ReadonlyMap<string, string>,
): RendererAnimatedMeshProjection {
  return {
    kind: 'rusty_renderer_animated_mesh_projection.v1',
    applyFrame: (frame) => {
      try {
        surface.applyFrame(frame);
        return { applied: true, diagnostics: [] };
      } catch (cause) {
        return {
          applied: false,
          diagnostics: [{
            code: 'animated_mesh_frame_rejected',
            message: cause instanceof Error ? cause.message : String(cause),
            asset: null,
            handle: null,
          }],
        };
      }
    },
    // The mounted browser surface advances its mixer exactly once in renderOnce.
    advance: () => ({ applied: true, diagnostics: [] }),
    playback: (handle) => {
      const playback = surface.animatedMeshPlayback(handle);
      return animationPlaybackReadout(
        handle,
        playback,
        playback === undefined ? null : contentHashes.get(playback.asset) ?? null,
      );
    },
    snapshot: surface.snapshot,
    hasAnimationTarget: (handle) => surface.renderer.has(handle),
    setAnimationControllerWeights: (handle, clips) => {
      surface.renderer.setAnimationControllerWeights(handle, clips);
    },
    hasAnimationClips: (handle, clipIds) =>
      surface.renderer.hasAnimationControllerClips(handle, clipIds),
    clearAnimationControllerWeights: (handle) => {
      surface.renderer.clearAnimationControllerWeights(handle);
    },
  };
}

interface RendererSurfaceCameraSnapshot {
  readonly basis?: RendererSurfaceCameraBasis;
  readonly pose: RendererSurfaceCameraPose;
}

interface RendererSurfaceFirstPersonControls {
  readonly cameraPose: () => RendererSurfaceCameraPose;
  readonly cameraSnapshot: () => RendererSurfaceCameraSnapshot;
  readonly dispose: () => void;
  readonly inputReadout: () => RendererSurfaceInputReadout;
  readonly lockPointer: () => void;
  readonly movementState: () => RendererSurfaceMovementState;
  readonly pointerLocked: () => boolean;
  readonly requiresAnimationFrame: () => boolean;
  readonly resetCamera: () => void;
  readonly setCameraPose: (
    pose: RendererSurfaceCameraPose,
    basis?: RendererSurfaceCameraBasis,
  ) => void;
  readonly update: (deltaSeconds: number) => void;
}

function createRendererSurfaceFirstPersonControls(
  canvas: HTMLCanvasElement,
  options: RendererSurfaceControlsOptions | undefined,
): RendererSurfaceFirstPersonControls {
  const enabled = options?.enabled === true;
  const document = canvas.ownerDocument;
  const moveSpeed = positiveFinite(options?.moveSpeed ?? 5.8, 'moveSpeed');
  const mouseSensitivity = positiveFinite(
    options?.mouseSensitivity ?? 0.0021,
    'mouseSensitivity',
  );
  const eyeHeight = finite(options?.eyeHeight ?? 1.62, 'eyeHeight');
  const initialPosition = finiteVector(
    options?.initialPosition ?? [0, eyeHeight, 8],
    'initialPosition',
  );
  const resolveMovement = options?.resolveMovement;
  const pressedCodes = new Set<string>();
  let pendingLook: readonly [number, number] = [0, 0];
  let basis: RendererSurfaceCameraBasis | undefined;
  let sequence = 0;
  let pitchRadians = degreesToRadians(finite(options?.initialPitchDegrees ?? 0, 'initialPitchDegrees'));
  let yawRadians = degreesToRadians(finite(options?.initialYawDegrees ?? 0, 'initialYawDegrees'));
  let position: RendererSurfaceVec3 = [...initialPosition];
  let movementState: RendererSurfaceMovementState = emptyMovementState(resolveMovement);
  const originalTabIndex = canvas.tabIndex;
  const originalTouchAction = canvas.style.touchAction;

  if (canvas.tabIndex < 0) {
    canvas.tabIndex = 0;
  }
  canvas.style.touchAction = 'none';

  const pointerLocked = (): boolean => document.pointerLockElement === canvas;
  const hasFocus = (): boolean => pointerLocked() || document.activeElement === canvas;
  const clearInput = (): void => {
    pressedCodes.clear();
    pendingLook = [0, 0];
  };
  const onPointerDown = (event: PointerEvent): void => {
    if (!enabled || event.button !== 0) return;
    event.preventDefault();
    canvas.focus({ preventScroll: true });
    if (!pointerLocked()) void canvas.requestPointerLock();
  };
  const onPointerLockChange = (): void => {
    if (!pointerLocked()) clearInput();
  };
  const onMouseMove = (event: MouseEvent): void => {
    if (!enabled || !pointerLocked()) return;
    pendingLook = [pendingLook[0] + event.movementX, pendingLook[1] + event.movementY];
  };
  const onKeyDown = (event: KeyboardEvent): void => {
    if (!enabled || !hasFocus() || !MOVEMENT_CODES.has(event.code)) return;
    event.preventDefault();
    pressedCodes.add(event.code);
  };
  const onKeyUp = (event: KeyboardEvent): void => {
    if (!MOVEMENT_CODES.has(event.code)) return;
    pressedCodes.delete(event.code);
  };

  canvas.addEventListener('pointerdown', onPointerDown);
  document.addEventListener('pointerlockchange', onPointerLockChange);
  document.addEventListener('mousemove', onMouseMove);
  document.addEventListener('keydown', onKeyDown);
  document.addEventListener('keyup', onKeyUp);
  document.defaultView?.addEventListener('blur', clearInput);

  const cameraPose = (): RendererSurfaceCameraPose => ({
    position: [round4(position[0]), round4(position[1]), round4(position[2])],
    pitchDegrees: round2(radiansToDegrees(pitchRadians)),
    yawDegrees: round2(radiansToDegrees(yawRadians)),
  });

  const resetCamera = (): void => {
    clearInput();
    basis = undefined;
    sequence = 0;
    pitchRadians = degreesToRadians(options?.initialPitchDegrees ?? 0);
    yawRadians = degreesToRadians(options?.initialYawDegrees ?? 0);
    position = [...initialPosition];
    movementState = emptyMovementState(resolveMovement);
  };

  const setCameraPose = (
    pose: RendererSurfaceCameraPose,
    nextBasis?: RendererSurfaceCameraBasis,
  ): void => {
    validateCameraPose(pose);
    if (nextBasis !== undefined) validateCameraBasis(nextBasis);
    position = [...pose.position];
    pitchRadians = degreesToRadians(pose.pitchDegrees);
    yawRadians = degreesToRadians(pose.yawDegrees);
    basis = nextBasis;
  };

  const update = (deltaSeconds: number): void => {
    if (!enabled) return;
    const safeDeltaSeconds = Math.max(0, finite(deltaSeconds, 'deltaSeconds'));
    const moveForward = axis(pressedCodes, 'KeyW', 'KeyS');
    const moveRight = axis(pressedCodes, 'KeyD', 'KeyA');
    const yawDeltaDegrees = pendingLook[0] * radiansToDegrees(mouseSensitivity);
    const pitchDeltaDegrees = -pendingLook[1] * radiansToDegrees(mouseSensitivity);
    pendingLook = [0, 0];
    if (moveForward === 0 && moveRight === 0 && yawDeltaDegrees === 0 && pitchDeltaDegrees === 0) {
      return;
    }

    if (resolveMovement !== undefined) {
      sequence += 1;
      const result = resolveMovement({
        deltaSeconds: safeDeltaSeconds,
        moveForward,
        moveRight,
        moveSpeedUnitsPerSecond: moveSpeed,
        pitchDeltaDegrees,
        poseBefore: cameraPose(),
        sequence,
        yawDeltaDegrees,
      });
      validateCameraPose(result.pose);
      position = [...result.pose.position];
      pitchRadians = degreesToRadians(result.pose.pitchDegrees);
      yawRadians = degreesToRadians(result.pose.yawDegrees);
      basis = result.basis;
      movementState = {
        mode: 'caller_resolved',
        blockedAxes: [...(result.blockedAxes ?? [])],
        collided: result.collided ?? false,
        resolutionId: result.resolutionId ?? null,
      };
      return;
    }

    yawRadians += degreesToRadians(yawDeltaDegrees);
    pitchRadians = clamp(
      pitchRadians + degreesToRadians(pitchDeltaDegrees),
      degreesToRadians(-85),
      degreesToRadians(85),
    );
    basis = undefined;
    const movement = calculateCameraRelativeMovement(yawRadians, moveForward, moveRight);
    if (movement !== null && safeDeltaSeconds > 0) {
      const step = moveSpeed * safeDeltaSeconds;
      position = [
        position[0] + movement[0] * step,
        eyeHeight,
        position[2] + movement[2] * step,
      ];
    }
    movementState = emptyMovementState(undefined);
  };

  return {
    cameraPose,
    cameraSnapshot: () => ({ ...(basis === undefined ? {} : { basis }), pose: cameraPose() }),
    inputReadout: () => ({
      enabled,
      pointerLocked: pointerLocked(),
      pressedCodes: [...pressedCodes].sort(),
    }),
    lockPointer: () => {
      if (enabled && !pointerLocked()) void canvas.requestPointerLock();
    },
    movementState: () => movementState,
    pointerLocked,
    requiresAnimationFrame: () => (
      enabled
      && (
        pressedCodes.size > 0
        || pendingLook[0] !== 0
        || pendingLook[1] !== 0
      )
    ),
    resetCamera,
    setCameraPose,
    update,
    dispose: () => {
      clearInput();
      canvas.removeEventListener('pointerdown', onPointerDown);
      document.removeEventListener('pointerlockchange', onPointerLockChange);
      document.removeEventListener('mousemove', onMouseMove);
      document.removeEventListener('keydown', onKeyDown);
      document.removeEventListener('keyup', onKeyUp);
      document.defaultView?.removeEventListener('blur', clearInput);
      if (pointerLocked()) document.exitPointerLock();
      canvas.tabIndex = originalTabIndex;
      canvas.style.touchAction = originalTouchAction;
    },
  };
}

const MOVEMENT_CODES = new Set(['KeyA', 'KeyD', 'KeyS', 'KeyW']);

function hasRetainedAnimation(
  submission: RendererSurfaceSubmissionSample | null,
): boolean {
  const statistic = submission?.statistics.animatedInstanceCount;
  return statistic?.status === 'available' && statistic.value > 0;
}

function surfaceViewport(canvas: HTMLCanvasElement): RendererSurfaceViewportState {
  return {
    bufferHeight: canvas.height,
    bufferWidth: canvas.width,
    clientHeight: canvas.clientHeight,
    clientWidth: canvas.clientWidth,
  };
}

function sameCameraSnapshot(
  left: RendererSurfaceCameraSnapshot,
  right: RendererSurfaceCameraSnapshot,
): boolean {
  return sameVector(left.pose.position, right.pose.position)
    && left.pose.pitchDegrees === right.pose.pitchDegrees
    && left.pose.yawDegrees === right.pose.yawDegrees
    && sameOptionalBasis(left.basis, right.basis);
}

function sameOptionalBasis(
  left: RendererSurfaceCameraBasis | undefined,
  right: RendererSurfaceCameraBasis | undefined,
): boolean {
  if (left === undefined || right === undefined) {
    return left === right;
  }
  return sameVector(left.forward, right.forward)
    && sameVector(left.right, right.right)
    && sameVector(left.up, right.up);
}

function sameVector(
  left: readonly [number, number, number],
  right: readonly [number, number, number],
): boolean {
  return left[0] === right[0] && left[1] === right[1] && left[2] === right[2];
}

function axis(pressed: ReadonlySet<string>, positive: string, negative: string): number {
  return Number(pressed.has(positive)) - Number(pressed.has(negative));
}

function calculateCameraRelativeMovement(
  yawRadians: number,
  forwardAxis: number,
  strafeAxis: number,
): RendererSurfaceVec3 | null {
  const forward: RendererSurfaceVec3 = [-Math.sin(yawRadians), 0, -Math.cos(yawRadians)];
  const right: RendererSurfaceVec3 = [Math.cos(yawRadians), 0, -Math.sin(yawRadians)];
  const movement: RendererSurfaceVec3 = [
    forward[0] * forwardAxis + right[0] * strafeAxis,
    0,
    forward[2] * forwardAxis + right[2] * strafeAxis,
  ];
  const length = Math.hypot(movement[0], movement[2]);
  return length === 0 ? null : [movement[0] / length, 0, movement[2] / length];
}

function emptyMovementState(
  resolver: RendererSurfaceMovementResolver | undefined,
): RendererSurfaceMovementState {
  return {
    mode: resolver === undefined ? 'free_camera' : 'caller_resolved',
    blockedAxes: [],
    collided: false,
    resolutionId: null,
  };
}

function normalizeSurfaceLighting(
  options: RendererSurfaceLightingOptions | undefined,
): {
  readonly schemaVersion: 1;
  readonly defaultLights: {
    readonly world: RendererSurfaceDefaultLightingMode;
    readonly viewmodel: RendererSurfaceDefaultLightingMode;
  };
  readonly shadows: { readonly enabled: boolean; readonly maximumActiveLights: number };
} {
  if (options !== undefined && options.schemaVersion !== RUSTY_RENDERER_SURFACE_LIGHTING_SCHEMA_VERSION) {
    throw new RendererSurfaceLightingError('lighting.schemaVersion must equal 1');
  }
  const world = options?.defaultLights?.world ?? 'neutral';
  const viewmodel = options?.defaultLights?.viewmodel ?? 'neutral';
  if ((world !== 'neutral' && world !== 'disabled')
    || (viewmodel !== 'neutral' && viewmodel !== 'disabled')) {
    throw new RendererSurfaceLightingError('default lighting mode must be neutral or disabled');
  }
  const enabled = options?.shadows?.enabled ?? false;
  if (typeof enabled !== 'boolean') {
    throw new RendererSurfaceLightingError('lighting.shadows.enabled must be boolean');
  }
  const maximumActiveLights = options?.shadows?.maximumActiveLights
    ?? RUSTY_RENDERER_SURFACE_MAX_ACTIVE_SHADOW_LIGHTS;
  if (!Number.isSafeInteger(maximumActiveLights)
    || maximumActiveLights < 0
    || maximumActiveLights > RUSTY_RENDERER_SURFACE_MAX_ACTIVE_SHADOW_LIGHTS) {
    throw new RendererSurfaceLightingError(
      `lighting.shadows.maximumActiveLights must be in 0..=${String(RUSTY_RENDERER_SURFACE_MAX_ACTIVE_SHADOW_LIGHTS)}`,
    );
  }
  return {
    schemaVersion: 1,
    defaultLights: { world, viewmodel },
    shadows: { enabled, maximumActiveLights },
  };
}

function contentHashesByAsset(
  manifest: RendererAnimatedMeshResourceManifest,
): ReadonlyMap<string, string> {
  return new Map(manifest.resources.map((resource) => [resource.asset, resource.contentHash]));
}

function validateCameraPose(pose: RendererSurfaceCameraPose): void {
  finiteVector(pose.position, 'resolved camera position');
  finite(pose.pitchDegrees, 'resolved camera pitch');
  finite(pose.yawDegrees, 'resolved camera yaw');
}

function validateCameraBasis(basis: RendererSurfaceCameraBasis): void {
  finiteVector(basis.forward, 'camera basis forward');
  finiteVector(basis.right, 'camera basis right');
  finiteVector(basis.up, 'camera basis up');
}

function finiteVector(
  value: readonly [number, number, number],
  label: string,
): readonly [number, number, number] {
  value.forEach((component, index) => finite(component, `${label}[${index}]`));
  return value;
}

function finite(value: number, label: string): number {
  if (!Number.isFinite(value)) throw new RangeError(`${label} must be finite`);
  return value;
}

function positiveFinite(value: number, label: string): number {
  if (!Number.isFinite(value) || value <= 0) {
    throw new RangeError(`${label} must be finite and greater than zero`);
  }
  return value;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function degreesToRadians(degrees: number): number {
  return (degrees * Math.PI) / 180;
}

function radiansToDegrees(radians: number): number {
  return (radians * 180) / Math.PI;
}

function round2(value: number): number {
  return Number(value.toFixed(2));
}

function round4(value: number): number {
  return Number(value.toFixed(4));
}
