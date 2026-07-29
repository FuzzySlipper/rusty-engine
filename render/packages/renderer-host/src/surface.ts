// Explicit browser/canvas composition over the renderer-neutral projection and Three backend.

import type {
  CameraBasis,
  PerspectiveProjection,
  PresentationFrameDiff,
  RenderFrameDiff,
  RenderHandle,
  RenderLayer,
} from '@rusty-engine/render-contracts';
import {
  RenderProjection,
  type RenderProjectionInstruction,
  type RenderProjectionSnapshot,
} from '@rusty-engine/render-projection';
import {
  createRendererBrowserSurfaceFrame,
  mountRendererBrowserSurface,
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

export const RUSTY_RENDERER_HOST_COMPATIBILITY_VERSION = 'renderer-host.v1';

export type RendererBackendFamily = 'threejs';

export interface RendererBackendDiagnostics {
  readonly family: RendererBackendFamily;
  readonly implementation: 'rusty-engine-renderer-backend';
  readonly publicContract: 'rusty-renderer-surface.v1';
}

export interface RendererSurfaceOptions {
  readonly autoStart?: boolean;
  readonly clearColor?: number;
  readonly controls?: RendererSurfaceControlsOptions;
  readonly frame?: RenderFrameDiff;
  readonly meshBufferSource?: RendererSurfaceMeshBufferSource;
  readonly pixelRatio?: number;
  readonly presentationHosts?: RendererPresentationHostSet;
  readonly projection?: PerspectiveProjection;
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

export interface RendererSurface {
  readonly kind: 'rusty_renderer_surface.v1';
  readonly backend: RendererBackendDiagnostics;
  readonly canvas: HTMLCanvasElement;
  readonly animationProjection: RendererAnimatedMeshProjection;
  readonly animatedMeshPlayback: (handle: RenderHandle) => RendererAnimatedMeshPlaybackReadout;
  readonly applyFrame: (frame: RenderFrameDiff) => RendererAnimatedMeshFrameReceipt;
  readonly applyPresentation: (
    frame: PresentationFrameDiff,
  ) => Promise<RendererPresentationFrameReceipt>;
  readonly cameraPose: () => RendererSurfaceCameraPose;
  readonly cameraProjection: () => PerspectiveProjection;
  readonly inputReadout: () => RendererSurfaceInputReadout;
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
  const frame = options.frame ?? createRendererDefaultSurfaceFrame();
  const projection = new RenderProjection();
  projection.applyFrame(frame);
  const controls = createRendererSurfaceFirstPersonControls(canvas, options.controls);
  const backendSurface = mountRendererBrowserSurface(canvas, {
    autoStart: false,
    ...(animatedMeshSource === undefined ? {} : { animatedMeshSource }),
    ...(options.meshBufferSource === undefined ? {} : { meshBufferSource: options.meshBufferSource }),
    camera: {
      initialPose: controls.cameraPose(),
      ...(options.projection === undefined ? {} : { projection: options.projection }),
    },
    ...(options.clearColor === undefined ? {} : { clearColor: options.clearColor }),
    ...(options.pixelRatio === undefined ? {} : { pixelRatio: options.pixelRatio }),
    frame,
  });
  const animationProjection = surfaceAnimationProjection(backendSurface, contentHashes);
  let presentationHosts = options.presentationHosts ?? null;
  let animationFrame: number | null = null;
  let lastRenderTimeMs: number | null = null;
  const timing = new RendererSurfaceTimingTracker();
  let latestSubmission: RendererSurfaceSubmissionSample | null = null;
  let disposed = false;

  const renderFrame = (
    timeMs: number,
    source: RendererSurfaceTimingSource,
  ): RendererSurfaceSubmissionSample => {
    if (disposed) throw new Error('renderer surface is disposed');
    assertRendererSurfaceSourceTime(timeMs);
    const deltaSeconds = lastRenderTimeMs === null
      ? 0
      : Math.min(0.05, Math.max(0, (timeMs - lastRenderTimeMs) / 1_000));
    lastRenderTimeMs = timeMs;
    controls.update(deltaSeconds);
    const camera = controls.cameraSnapshot();
    backendSurface.setCameraPose(camera.pose, camera.basis);
    presentationHosts?.advance(deltaSeconds);
    const backendSubmissionStartedMs = surfaceTimingNow();
    const backendStatistics = backendSurface.renderOnce(timeMs);
    const backendSubmissionEndedMs = surfaceTimingNow();
    latestSubmission = surfaceSubmissionSample(timing.record({
      source,
      sourceTimeMs: timeMs,
      backendSubmissionStartedMs,
      backendSubmissionEndedMs,
    }), backendStatistics);
    return latestSubmission;
  };
  const renderOnce = (
    timeMs = globalThis.performance?.now() ?? 0,
  ): RendererSurfaceSubmissionSample => {
    return renderFrame(timeMs, 'explicit');
  };

  const tick = (timeMs: number): void => {
    renderFrame(timeMs, 'animationFrame');
    animationFrame = globalThis.requestAnimationFrame(tick);
  };
  const start = (): void => {
    if (disposed) throw new Error('renderer surface is disposed');
    if (animationFrame === null) {
      animationFrame = globalThis.requestAnimationFrame(tick);
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
    applyFrame,
    applyPresentation: async (presentationFrame) => {
      return (presentationHosts ?? new RendererPresentationHostSet({})).apply(presentationFrame);
    },
    cameraPose: controls.cameraPose,
    cameraProjection: backendSurface.cameraProjection,
    inputReadout: controls.inputReadout,
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
      controls.setCameraPose(pose, basis);
      backendSurface.setCameraPose(pose, basis);
    },
    setPresentationHosts: (hosts) => {
      presentationHosts = hosts;
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
