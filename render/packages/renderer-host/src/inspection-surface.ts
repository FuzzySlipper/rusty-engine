// Projection-only interactive viewer for downstream visual-authoring tools.

import type {
  AnimatedMeshPlaybackCommand,
  EditorGridDescriptor,
  EditorGridProjectionReadout,
  PerspectiveProjection,
  RenderFrameDiff,
  RenderHandle,
} from '@rusty-engine/render-contracts';
import type { RendererAnimatedMeshSampleReadout } from './surface.js';
import type {
  RendererEditorViewport,
  RendererEditorViewportBufferSource,
  RendererEditorViewportCamera,
  RendererEditorViewportChannelReceipt,
  RendererEditorViewportGridReceipt,
  RendererEditorViewportPickReceipt,
  RendererEditorViewportPickRequest,
  RendererEditorViewportSize,
  RendererEditorViewportSizeReceipt,
} from './editor-viewport.js';
import type {
  RendererAnimatedMeshResourceManifest,
  RendererAnimatedMeshResourceResolver,
} from './animated-mesh-host.js';
import type {
  RendererMeshResourceManifest,
  RendererMeshResourceResolver,
} from './mesh-resource-host.js';
import type {
  RendererTextureResourceManifest,
  RendererTextureResourceResolver,
} from './texture-resource-host.js';
import type { RendererSurfaceSubmissionSample } from './surface-statistics.js';
import {
  assertRendererSurfaceSourceTime,
  RendererSurfaceTimingTracker,
  type RendererSurfaceTimingSource,
} from './surface-timing.js';
import { resolveRendererStoredEditorCamera } from './stored-editor-camera.js';

export const RUSTY_RENDERER_INSPECTION_SURFACE_COMPATIBILITY_VERSION = 'inspection-surface.v1';

type InspectionVector = readonly [number, number, number];

export interface RendererInspectionSurfaceControlsOptions {
  readonly enabled?: boolean;
  readonly initialPosition?: InspectionVector;
  readonly initialTarget?: InspectionVector;
  /** World units travelled per second while a movement key is held. */
  readonly moveSpeed?: number;
  /** Multiplier applied while the configured boost key is held. */
  readonly boostMultiplier?: number;
  readonly invertLookY?: boolean;
  readonly invertPanY?: boolean;
  readonly keyboard?: Partial<RendererInspectionSurfaceKeyboardBindings>;
  /** Orbit degrees applied per mouse pixel while the primary button is held. */
  readonly orbitDegreesPerPixel?: number;
  /** Orbit degrees applied per second while a focused arrow key is held. */
  readonly keyboardOrbitDegreesPerSecond?: number;
  /** Smallest allowed distance between the inspection camera and its target. */
  readonly minimumDistance?: number;
  /** Largest allowed distance between the inspection camera and its target. */
  readonly maximumDistance?: number;
  /** Multiplicative camera-distance change for each focused keyboard or wheel step. */
  readonly zoomFactorPerStep?: number;
  readonly projection?: PerspectiveProjection;
}

export interface RendererInspectionSurfaceKeyboardBindings {
  readonly moveForward: string;
  readonly moveBackward: string;
  readonly moveLeft: string;
  readonly moveRight: string;
  readonly moveDown: string;
  readonly moveUp: string;
  readonly boost: string;
}

export interface RendererInspectionSurfaceControlPreferences {
  readonly moveSpeed: number;
  readonly boostMultiplier: number;
  readonly invertLookY: boolean;
  readonly invertPanY: boolean;
  readonly keyboard: RendererInspectionSurfaceKeyboardBindings;
}

export interface RendererInspectionSurfaceOptions {
  readonly animatedMeshManifest?: RendererAnimatedMeshResourceManifest;
  readonly autoStart?: boolean;
  readonly bufferSource?: RendererEditorViewportBufferSource;
  readonly clearColor?: number;
  readonly controls?: RendererInspectionSurfaceControlsOptions;
  /** A complete retained projection frame. Later replacements are atomic. */
  readonly frame?: RenderFrameDiff;
  /** Optional engine-owned procedural editor grid shown with the inspection projection. */
  readonly initialGrid?: EditorGridDescriptor | null;
  readonly meshResourceManifest?: RendererMeshResourceManifest;
  readonly pixelRatio?: number;
  readonly resolveAnimatedMeshResource?: RendererAnimatedMeshResourceResolver;
  readonly resolveMeshResource?: RendererMeshResourceResolver;
  readonly resolveTextureResource?: RendererTextureResourceResolver;
  readonly textureResourceManifest?: RendererTextureResourceManifest;
}

export type RendererInspectionSurfaceStatus = 'mounted' | 'running' | 'stopped' | 'disposed';

export type RendererInspectionCameraChange =
  | 'initial_camera'
  | 'frame_bounds'
  | 'focus_target'
  | 'keyboard_movement'
  | 'keyboard_orbit'
  | 'keyboard_zoom'
  | 'pointer_orbit'
  | 'pointer_pan'
  | 'wheel_zoom';

export interface RendererInspectionSurfaceReadout {
  readonly kind: 'rusty_renderer_inspection_surface_readout.v1';
  readonly compatibilityVersion: typeof RUSTY_RENDERER_INSPECTION_SURFACE_COMPATIBILITY_VERSION;
  /** Camera input here is disposable inspection state owned by this surface. */
  readonly role: 'projection_only_inspection';
  readonly camera: RendererEditorViewportCamera;
  readonly cameraDistance: number;
  readonly cameraRevision: number;
  readonly controlPreferences: RendererInspectionSurfaceControlPreferences;
  readonly dragging: boolean;
  readonly grid: EditorGridProjectionReadout | null;
  readonly gridRevision: number;
  readonly lastCameraChange: RendererInspectionCameraChange;
  readonly pressedMovementKeys: readonly string[];
  readonly pressedOrbitKeys: readonly string[];
  readonly retainedFrameHash: string;
  readonly retainedOpCount: number;
  /** Incremental live projection state, separate from authored inspection content. */
  readonly runtimeFrameHash: string;
  readonly runtimeGeneration: number;
  readonly runtimeRetainedOpCount: number;
  readonly status: RendererInspectionSurfaceStatus;
  readonly viewportHash: string;
}

export interface RendererInspectionSurface {
  readonly kind: 'rusty_renderer_inspection_surface.v1';
  readonly role: 'projection_only_inspection';
  readonly canvas: HTMLCanvasElement;
  readonly camera: () => RendererEditorViewportCamera;
  /** Apply one incremental authored frame to the already retained authored channel. */
  readonly applyAuthoredFrame: (frame: RenderFrameDiff) => RendererEditorViewportChannelReceipt;
  /** Apply one incremental, projection-only runtime frame to the retained runtime channel. */
  readonly applyRuntimeFrame: (frame: RenderFrameDiff) => RendererEditorViewportChannelReceipt;
  /** Clear retained runtime projection without disturbing authored inspection content. */
  readonly clearRuntimeProjection: () => RendererEditorViewportChannelReceipt;
  /** Clear disposable editor overlays without disturbing authored or runtime content. */
  readonly clearOverlayProjection: () => RendererEditorViewportChannelReceipt;
  /** Replace host-user input preferences without resetting the disposable camera pose. */
  readonly configureControlPreferences: (
    preferences: RendererInspectionSurfaceControlPreferences,
  ) => void;
  readonly dispose: () => void;
  /** Retarget the disposable orbit pivot while preserving camera orientation and distance. */
  readonly focusTarget: (target: InspectionVector) => boolean;
  /** Frame finite world bounds from a deterministic front inspection view. */
  readonly frameBounds: (bounds: {
    readonly min: InspectionVector;
    readonly max: InspectionVector;
  }) => boolean;
  readonly grid: () => EditorGridProjectionReadout | null;
  readonly pick: (request: RendererEditorViewportPickRequest) => RendererEditorViewportPickReceipt;
  /** Project a world point through the exact mounted editor backend camera. */
  readonly projectWorldPoint: RendererEditorViewport['projectWorldPoint'];
  readonly readout: () => RendererInspectionSurfaceReadout;
  readonly renderOnce: (timeMs?: number) => void;
  /** Deterministically sample an authored animated mesh without mutating project state. */
  readonly sampleAnimatedMesh: (
    handle: RenderHandle,
    clipId: string,
    normalizedTime: number,
  ) => RendererAnimatedMeshSampleReadout;
  /** Apply disposable authored-channel playback for human inspection only. */
  readonly setAnimatedMeshPlayback: (
    handle: RenderHandle,
    playback: AnimatedMeshPlaybackCommand,
  ) => void;
  /** Atomically replace authored content from individually bounded transport frames. */
  readonly replaceAuthoredFrameChunks: (
    chunks: readonly RenderFrameDiff[],
  ) => RendererEditorViewportChannelReceipt;
  /** Atomically replace authored content from one bounded frame. */
  readonly replaceFrame: (frame: RenderFrameDiff) => RendererEditorViewportChannelReceipt;
  /** Atomically replace disposable debug-layer editor overlays. */
  readonly replaceOverlayFrame: (frame: RenderFrameDiff) => RendererEditorViewportChannelReceipt;
  readonly resize: (size: RendererEditorViewportSize) => RendererEditorViewportSizeReceipt;
  readonly resizeToCanvas: () => RendererEditorViewportSizeReceipt;
  readonly setGrid: (descriptor: EditorGridDescriptor | null) => RendererEditorViewportGridReceipt;
  readonly start: () => void;
  readonly stop: () => void;
  /** Read the latest automatic, mount, or explicit immutable submission sample. */
  readonly submission: () => RendererSurfaceSubmissionSample;
}

interface RendererInspectionAnimationScheduler {
  readonly cancel: (handle: number) => void;
  readonly now: () => number;
  readonly request: (callback: (timeMs: number) => void) => number;
}

interface RendererInspectionResizeObserver {
  readonly disconnect: () => void;
  readonly observe: (target: Element) => void;
}

interface RendererInspectionEnvironment {
  readonly animation: RendererInspectionAnimationScheduler;
  readonly createResizeObserver: (
    callback: () => void,
  ) => RendererInspectionResizeObserver | null;
  readonly devicePixelRatio: () => number;
}

interface InspectionControls {
  readonly camera: () => RendererEditorViewportCamera;
  readonly cameraDistance: () => number;
  readonly cameraRevision: () => number;
  readonly clearInputState: () => void;
  readonly configurePreferences: (
    preferences: RendererInspectionSurfaceControlPreferences,
  ) => void;
  readonly controlPreferences: () => RendererInspectionSurfaceControlPreferences;
  readonly dispose: () => void;
  readonly dragging: () => boolean;
  readonly frameBounds: (bounds: {
    readonly min: InspectionVector;
    readonly max: InspectionVector;
  }) => boolean;
  readonly focusTarget: (target: InspectionVector) => boolean;
  readonly lastCameraChange: () => RendererInspectionCameraChange;
  readonly pressedMovementKeys: () => readonly string[];
  readonly pressedOrbitKeys: () => readonly string[];
  readonly update: (deltaSeconds: number) => void;
}

const DEFAULT_PROJECTION: PerspectiveProjection = {
  fovYDegrees: 55,
  near: 0.05,
  far: 1000,
};
const DEFAULT_KEYBOARD_BINDINGS: RendererInspectionSurfaceKeyboardBindings = {
  moveForward: 'KeyW',
  moveBackward: 'KeyS',
  moveLeft: 'KeyA',
  moveRight: 'KeyD',
  moveDown: 'KeyQ',
  moveUp: 'KeyE',
  boost: 'ShiftLeft',
};
const ORBIT_KEYS = ['ArrowDown', 'ArrowLeft', 'ArrowRight', 'ArrowUp'] as const;
const MAXIMUM_PITCH_DEGREES = 85;

export async function mountRendererInspectionSurface(
  canvas: HTMLCanvasElement,
  options: RendererInspectionSurfaceOptions = {},
): Promise<RendererInspectionSurface> {
  const { mountRendererEditorViewport } = await import('./editor-viewport.js');
  const viewport = await mountRendererEditorViewport(canvas, {
    autoStart: false,
    ...(options.animatedMeshManifest === undefined
      ? {}
      : { animatedMeshManifest: options.animatedMeshManifest }),
    ...(options.bufferSource === undefined ? {} : { bufferSource: options.bufferSource }),
    ...(options.clearColor === undefined ? {} : { clearColor: options.clearColor }),
    ...(options.meshResourceManifest === undefined
      ? {}
      : { meshResourceManifest: options.meshResourceManifest }),
    ...(options.pixelRatio === undefined ? {} : { pixelRatio: options.pixelRatio }),
    ...(options.resolveAnimatedMeshResource === undefined
      ? {}
      : { resolveAnimatedMeshResource: options.resolveAnimatedMeshResource }),
    ...(options.resolveMeshResource === undefined
      ? {}
      : { resolveMeshResource: options.resolveMeshResource }),
    ...(options.resolveTextureResource === undefined
      ? {}
      : { resolveTextureResource: options.resolveTextureResource }),
    ...(options.textureResourceManifest === undefined
      ? {}
      : { textureResourceManifest: options.textureResourceManifest }),
  });
  try {
    return createRendererInspectionSurfaceWithViewport(
      canvas,
      viewport,
      options,
      browserInspectionEnvironment(),
    );
  } catch (error) {
    viewport.dispose();
    throw error;
  }
}

/** Internal conformance seam; downstream consumers use the package-root mount helper. */
export function createRendererInspectionSurfaceWithViewport(
  canvas: HTMLCanvasElement,
  viewport: RendererEditorViewport,
  options: RendererInspectionSurfaceOptions = {},
  environment: RendererInspectionEnvironment = browserInspectionEnvironment(),
): RendererInspectionSurface {
  const controls = createInspectionControls(canvas, viewport, options.controls);
  let animationHandle: number | null = null;
  let gridRevision = 0;
  let lastRenderTimeMs: number | null = null;
  const timing = new RendererSurfaceTimingTracker();
  let latestSubmission: RendererSurfaceSubmissionSample | null = null;
  let status: RendererInspectionSurfaceStatus = 'mounted';

  const resizeToCanvas = (): RendererEditorViewportSizeReceipt => viewport.resize({
    width: Math.max(1, Math.round(canvas.clientWidth || canvas.width || 1)),
    height: Math.max(1, Math.round(canvas.clientHeight || canvas.height || 1)),
    pixelRatio: options.pixelRatio ?? environment.devicePixelRatio(),
  });

  let resizeObserver: RendererInspectionResizeObserver | null = null;

  const renderFrame = (
    timeMs: number,
    source: RendererSurfaceTimingSource,
  ): void => {
    if (status === 'disposed') {
      return;
    }
    assertRendererSurfaceSourceTime(timeMs);
    const deltaSeconds = lastRenderTimeMs === null
      ? 0
      : Math.min(0.1, Math.max(0, (timeMs - lastRenderTimeMs) / 1000));
    lastRenderTimeMs = timeMs;
    controls.update(deltaSeconds);
    const backendSubmissionStartedMs = surfaceTimingNow();
    const statistics = viewport.renderOnce(timeMs);
    const backendSubmissionEndedMs = surfaceTimingNow();
    latestSubmission = Object.freeze({
      ...timing.record({
        source,
        sourceTimeMs: timeMs,
        backendSubmissionStartedMs,
        backendSubmissionEndedMs,
      }),
      statistics,
    });
  };

  const renderOnce = (timeMs = environment.animation.now()): void => {
    renderFrame(timeMs, 'explicit');
  };

  const tick = (timeMs: number): void => {
    if (status !== 'running') {
      return;
    }
    renderFrame(timeMs, 'animationFrame');
    animationHandle = environment.animation.request(tick);
  };

  const start = (): void => {
    if (status === 'disposed' || status === 'running') {
      return;
    }
    status = 'running';
    lastRenderTimeMs = null;
    animationHandle = environment.animation.request(tick);
  };

  const stop = (): void => {
    if (status === 'disposed') {
      return;
    }
    if (animationHandle !== null) {
      environment.animation.cancel(animationHandle);
      animationHandle = null;
    }
    status = 'stopped';
    lastRenderTimeMs = null;
    controls.clearInputState();
  };

  const replaceFrame = (frame: RenderFrameDiff): RendererEditorViewportChannelReceipt =>
    viewport.channels.authored.replace(frame);

  const applyAuthoredFrame = (frame: RenderFrameDiff): RendererEditorViewportChannelReceipt =>
    viewport.channels.authored.apply(frame);

  const replaceAuthoredFrameChunks = (
    chunks: readonly RenderFrameDiff[],
  ): RendererEditorViewportChannelReceipt => viewport.channels.authored.replaceChunks(chunks);

  const applyRuntimeFrame = (frame: RenderFrameDiff): RendererEditorViewportChannelReceipt =>
    viewport.channels.runtime.apply(frame);

  const clearRuntimeProjection = (): RendererEditorViewportChannelReceipt =>
    viewport.channels.runtime.clear();

  const clearOverlayProjection = (): RendererEditorViewportChannelReceipt =>
    viewport.channels.overlay.clear();

  const replaceOverlayFrame = (frame: RenderFrameDiff): RendererEditorViewportChannelReceipt =>
    viewport.channels.overlay.replace(frame);

  const setGrid = (
    descriptor: EditorGridDescriptor | null,
  ): RendererEditorViewportGridReceipt => {
    const receipt = viewport.setGrid(descriptor);
    if (receipt.applied) {
      gridRevision += 1;
    }
    return receipt;
  };

  try {
    resizeObserver = environment.createResizeObserver(() => {
      if (status !== 'disposed') {
        resizeToCanvas();
      }
    });
    resizeObserver?.observe(canvas);

    if (options.initialGrid !== undefined) {
      const initialGridReceipt = setGrid(options.initialGrid);
      if (!initialGridReceipt.applied) {
        const diagnostic = initialGridReceipt.diagnostics[0];
        throw new TypeError(diagnostic?.message ?? 'inspection surface rejected its initial grid');
      }
    }

    if (options.frame !== undefined) {
      const initialReceipt = replaceFrame(options.frame);
      if (!initialReceipt.applied) {
        const diagnostic = initialReceipt.diagnostics[0];
        throw new TypeError(diagnostic?.message ?? 'inspection surface rejected its initial frame');
      }
    }

    resizeToCanvas();
    renderFrame(0, 'mount');
    if (options.autoStart !== false) {
      start();
    }
  } catch (error) {
    controls.dispose();
    resizeObserver?.disconnect();
    viewport.dispose();
    throw error;
  }

  return {
    kind: 'rusty_renderer_inspection_surface.v1',
    role: 'projection_only_inspection',
    canvas,
    applyAuthoredFrame,
    applyRuntimeFrame,
    camera: () => controls.camera(),
    clearOverlayProjection,
    clearRuntimeProjection,
    configureControlPreferences: (preferences) => controls.configurePreferences(preferences),
    frameBounds: (bounds) => controls.frameBounds(bounds),
    focusTarget: (target) => controls.focusTarget(target),
    grid: () => viewport.grid(),
    pick: (request) => viewport.pick(request),
    projectWorldPoint: (position) => viewport.projectWorldPoint(position),
    readout: () => {
      const viewportReadout = viewport.readout();
      const authored = viewportReadout.channels.find((channel) => channel.channel === 'authored');
      const runtime = viewportReadout.channels.find((channel) => channel.channel === 'runtime');
      return {
        kind: 'rusty_renderer_inspection_surface_readout.v1',
        compatibilityVersion: RUSTY_RENDERER_INSPECTION_SURFACE_COMPATIBILITY_VERSION,
        role: 'projection_only_inspection',
        camera: controls.camera(),
        cameraDistance: controls.cameraDistance(),
        cameraRevision: controls.cameraRevision(),
        controlPreferences: controls.controlPreferences(),
        dragging: controls.dragging(),
        grid: viewportReadout.grid,
        gridRevision,
        lastCameraChange: controls.lastCameraChange(),
        pressedMovementKeys: controls.pressedMovementKeys(),
        pressedOrbitKeys: controls.pressedOrbitKeys(),
        retainedFrameHash: authored?.hash ?? '',
        retainedOpCount: authored?.retainedOpCount ?? 0,
        runtimeFrameHash: runtime?.hash ?? '',
        runtimeGeneration: runtime?.generation ?? 0,
        runtimeRetainedOpCount: runtime?.retainedOpCount ?? 0,
        status,
        viewportHash: viewportReadout.viewportHash,
      };
    },
    renderOnce,
    sampleAnimatedMesh: (handle, clipId, normalizedTime) =>
      viewport.sampleAnimatedMesh('authored', handle, clipId, normalizedTime),
    setAnimatedMeshPlayback: (handle, playback) =>
      viewport.setAnimatedMeshPlayback('authored', handle, playback),
    replaceAuthoredFrameChunks,
    replaceFrame,
    replaceOverlayFrame,
    resize: (size) => viewport.resize(size),
    resizeToCanvas,
    setGrid,
    start,
    stop,
    submission: () => {
      if (latestSubmission === null) {
        throw new Error('renderer inspection surface has not submitted a frame');
      }
      return latestSubmission;
    },
    dispose: () => {
      if (status === 'disposed') {
        return;
      }
      stop();
      status = 'disposed';
      controls.dispose();
      resizeObserver?.disconnect();
      viewport.dispose();
    },
  };
}

function surfaceTimingNow(): number {
  return globalThis.performance?.now() ?? 0;
}

function createInspectionControls(
  canvas: HTMLCanvasElement,
  viewport: RendererEditorViewport,
  options: RendererInspectionSurfaceControlsOptions | undefined,
): InspectionControls {
  const enabled = options?.enabled !== false;
  let preferences = validateControlPreferences({
    moveSpeed: options?.moveSpeed ?? 5,
    boostMultiplier: options?.boostMultiplier ?? 4,
    invertLookY: options?.invertLookY ?? false,
    invertPanY: options?.invertPanY ?? false,
    keyboard: { ...DEFAULT_KEYBOARD_BINDINGS, ...options?.keyboard },
  });
  const orbitDegreesPerPixel = requirePositiveFinite(
    options?.orbitDegreesPerPixel ?? 0.24,
    'inspection orbitDegreesPerPixel',
  );
  const keyboardOrbitDegreesPerSecond = requirePositiveFinite(
    options?.keyboardOrbitDegreesPerSecond ?? 90,
    'inspection keyboardOrbitDegreesPerSecond',
  );
  const minimumDistance = requirePositiveFinite(
    options?.minimumDistance ?? 0.1,
    'inspection minimumDistance',
  );
  const maximumDistance = requirePositiveFinite(
    options?.maximumDistance ?? 10_000,
    'inspection maximumDistance',
  );
  const zoomFactorPerStep = requireUnitInterval(
    options?.zoomFactorPerStep ?? 0.85,
    'inspection zoomFactorPerStep',
  );
  if (maximumDistance <= minimumDistance) {
    throw new TypeError('inspection maximumDistance must be greater than minimumDistance');
  }
  const projection = options?.projection ?? DEFAULT_PROJECTION;
  const initialPosition = options?.initialPosition ?? [4, 4, 8];
  let target: InspectionVector = [...(options?.initialTarget ?? [0, 0, 0])];
  const offset = subtract(initialPosition, target);
  let distance = vectorLength(offset);
  if (!allFinite([initialPosition, target]) || distance <= 0.000_001) {
    throw new TypeError('inspection initialPosition and initialTarget must be finite and distinct');
  }
  if (distance < minimumDistance || distance > maximumDistance) {
    throw new TypeError('inspection initial camera distance must be within its configured bounds');
  }
  let yawRadians = Math.atan2(offset[0], offset[2]);
  let pitchRadians = clamp(
    Math.asin(clamp(offset[1] / distance, -1, 1)),
    degreesToRadians(-MAXIMUM_PITCH_DEGREES),
    degreesToRadians(MAXIMUM_PITCH_DEGREES),
  );
  let camera = resolveCamera(positionFromOrbit(target, distance, yawRadians, pitchRadians), target, projection);
  let cameraRevision = 0;
  let lastCameraChange: RendererInspectionCameraChange = 'initial_camera';
  let activePointerId: number | null = null;
  let activePointerMode: 'orbit' | 'pan' | null = null;
  let lastPointerPosition: readonly [number, number] | null = null;
  const pressedMovementKeys = new Set<string>();
  const pressedOrbitKeys = new Set<string>();
  const ownerDocument = canvas.ownerDocument;
  const ownerWindow = ownerDocument.defaultView;
  const originalTabIndex = canvas.tabIndex;
  const originalTouchAction = canvas.style.touchAction;

  if (canvas.tabIndex < 0) {
    canvas.tabIndex = 0;
  }
  canvas.style.touchAction = 'none';

  const commitCamera = (
    nextTarget: InspectionVector,
    nextYawRadians: number,
    nextPitchRadians: number,
    nextDistance: number,
    change: RendererInspectionCameraChange,
  ): boolean => {
    const nextCamera = resolveCamera(
      positionFromOrbit(nextTarget, nextDistance, nextYawRadians, nextPitchRadians),
      nextTarget,
      projection,
    );
    const receipt = viewport.setCamera(nextCamera);
    if (!receipt.applied) {
      return false;
    }
    target = nextTarget;
    yawRadians = nextYawRadians;
    pitchRadians = nextPitchRadians;
    distance = nextDistance;
    camera = nextCamera;
    cameraRevision += 1;
    lastCameraChange = change;
    return true;
  };

  const clearPointerState = (): void => {
    const pointerId = activePointerId;
    activePointerId = null;
    activePointerMode = null;
    lastPointerPosition = null;
    if (pointerId === null) {
      return;
    }
    try {
      if (canvas.hasPointerCapture(pointerId)) {
        canvas.releasePointerCapture(pointerId);
      }
    } catch {
      // Capture may already have been released by pointer cancellation or DOM removal.
    }
  };

  const clearInputState = (): void => {
    clearPointerState();
    pressedMovementKeys.clear();
    pressedOrbitKeys.clear();
  };

  const onPointerDown = (event: PointerEvent): void => {
    if (
      !enabled
      || (event.button !== 0 && event.button !== 1)
      || event.isPrimary === false
      || !Number.isFinite(event.clientX)
      || !Number.isFinite(event.clientY)
    ) {
      return;
    }
    event.preventDefault();
    canvas.focus({ preventScroll: true });
    clearPointerState();
    activePointerId = event.pointerId;
    activePointerMode = event.button === 1 ? 'pan' : 'orbit';
    lastPointerPosition = [event.clientX, event.clientY];
    try {
      canvas.setPointerCapture(event.pointerId);
    } catch {
      activePointerId = null;
      activePointerMode = null;
      lastPointerPosition = null;
    }
  };
  const onPointerMove = (event: PointerEvent): void => {
    if (!enabled || activePointerId !== event.pointerId || lastPointerPosition === null) {
      return;
    }
    if (!Number.isFinite(event.clientX) || !Number.isFinite(event.clientY)) {
      return;
    }
    const movementX = event.clientX - lastPointerPosition[0];
    const movementY = event.clientY - lastPointerPosition[1];
    lastPointerPosition = [event.clientX, event.clientY];
    if (movementX === 0 && movementY === 0) {
      return;
    }
    event.preventDefault();
    if (activePointerMode === 'pan') {
      const panScale = Math.max(0.0025, distance * 0.0015);
      const panY = movementY * (preferences.invertPanY ? -1 : 1);
      const nextTarget = add(
        target,
        add(
          scale(camera.basis.right, -movementX * panScale),
          scale(camera.basis.up, panY * panScale),
        ),
      );
      commitCamera(nextTarget, yawRadians, pitchRadians, distance, 'pointer_pan');
      return;
    }
    const lookY = movementY * (preferences.invertLookY ? -1 : 1);
    const nextYawRadians = yawRadians - degreesToRadians(movementX * orbitDegreesPerPixel);
    const nextPitchRadians = clamp(
      pitchRadians + degreesToRadians(lookY * orbitDegreesPerPixel),
      degreesToRadians(-MAXIMUM_PITCH_DEGREES),
      degreesToRadians(MAXIMUM_PITCH_DEGREES),
    );
    commitCamera(target, nextYawRadians, nextPitchRadians, distance, 'pointer_orbit');
  };
  const onPointerEnd = (event: PointerEvent): void => {
    if (activePointerId === event.pointerId) {
      clearPointerState();
    }
  };
  const onPointerCancel = (event: PointerEvent): void => {
    if (activePointerId === event.pointerId) {
      clearInputState();
    }
  };
  const onLostPointerCapture = (event: PointerEvent): void => {
    if (activePointerId === event.pointerId) {
      activePointerId = null;
      activePointerMode = null;
      lastPointerPosition = null;
    }
  };
  const applyZoom = (
    factor: number,
    change: 'keyboard_zoom' | 'wheel_zoom',
  ): void => {
    const nextDistance = clamp(distance * factor, minimumDistance, maximumDistance);
    if (Math.abs(nextDistance - distance) <= 0.000_001) {
      return;
    }
    commitCamera(target, yawRadians, pitchRadians, nextDistance, change);
  };
  const onKeyDown = (event: KeyboardEvent): void => {
    if (!enabled || ownerDocument.activeElement !== canvas) {
      return;
    }
    if (isMovementKey(event.code, preferences.keyboard)) {
      event.preventDefault();
      pressedMovementKeys.add(event.code);
      return;
    }
    if (isOrbitKey(event.code)) {
      event.preventDefault();
      pressedOrbitKeys.add(event.code);
      return;
    }
    const zoomDirection = keyboardZoomDirection(event);
    if (zoomDirection !== null) {
      event.preventDefault();
      applyZoom(zoomDirection === 'in' ? zoomFactorPerStep : 1 / zoomFactorPerStep, 'keyboard_zoom');
    }
  };
  const onKeyUp = (event: KeyboardEvent): void => {
    if (isMovementKey(event.code, preferences.keyboard) && pressedMovementKeys.delete(event.code)) {
      event.preventDefault();
    } else if (isOrbitKey(event.code) && pressedOrbitKeys.delete(event.code)) {
      event.preventDefault();
    }
  };
  const onWheel = (event: WheelEvent): void => {
    if (!enabled || ownerDocument.activeElement !== canvas || !Number.isFinite(event.deltaY) || event.deltaY === 0) {
      return;
    }
    event.preventDefault();
    applyZoom(event.deltaY < 0 ? zoomFactorPerStep : 1 / zoomFactorPerStep, 'wheel_zoom');
  };
  const onVisibilityChange = (): void => {
    if (ownerDocument.visibilityState !== 'visible') {
      clearInputState();
    }
  };

  if (!commitCamera(target, yawRadians, pitchRadians, distance, 'initial_camera')) {
    throw new TypeError('inspection camera was rejected during mount');
  }
  canvas.addEventListener('pointerdown', onPointerDown);
  canvas.addEventListener('pointermove', onPointerMove);
  canvas.addEventListener('pointerup', onPointerEnd);
  canvas.addEventListener('pointercancel', onPointerCancel);
  canvas.addEventListener('lostpointercapture', onLostPointerCapture);
  canvas.addEventListener('wheel', onWheel, { passive: false });
  canvas.addEventListener('blur', clearInputState);
  ownerDocument.addEventListener('pointerup', onPointerEnd);
  ownerDocument.addEventListener('pointercancel', onPointerCancel);
  ownerDocument.addEventListener('keydown', onKeyDown);
  ownerDocument.addEventListener('keyup', onKeyUp);
  ownerDocument.addEventListener('visibilitychange', onVisibilityChange);
  ownerWindow?.addEventListener('blur', clearInputState);

  return {
    camera: () => camera,
    cameraDistance: () => distance,
    cameraRevision: () => cameraRevision,
    clearInputState,
    configurePreferences: (next) => {
      preferences = validateControlPreferences(next);
      clearInputState();
    },
    controlPreferences: () => ({
      ...preferences,
      keyboard: { ...preferences.keyboard },
    }),
    dragging: () => activePointerId !== null,
    frameBounds: (bounds) => {
      if (!allFinite([bounds.min, bounds.max])) return false;
      const extent: InspectionVector = [
        bounds.max[0] - bounds.min[0],
        bounds.max[1] - bounds.min[1],
        bounds.max[2] - bounds.min[2],
      ];
      if (extent.some((value) => value < 0)) return false;
      const center: InspectionVector = [
        (bounds.min[0] + bounds.max[0]) / 2,
        (bounds.min[1] + bounds.max[1]) / 2,
        (bounds.min[2] + bounds.max[2]) / 2,
      ];
      const halfFov = Math.tan(camera.projection.fovYDegrees * Math.PI / 360);
      const aspect = Math.max(0.1, (canvas.clientWidth || canvas.width) / (canvas.clientHeight || canvas.height));
      const fittedDistance = Math.max(
        extent[1] / 2 / halfFov,
        extent[0] / 2 / (halfFov * aspect),
      ) * 1.35 + extent[2] / 2;
      return commitCamera(
        center,
        0,
        0,
        clamp(fittedDistance, minimumDistance, maximumDistance),
        'frame_bounds',
      );
    },
    focusTarget: (nextTarget) => {
      if (!allFinite([nextTarget])) return false;
      return commitCamera(
        [...nextTarget],
        yawRadians,
        pitchRadians,
        distance,
        'focus_target',
      );
    },
    lastCameraChange: () => lastCameraChange,
    pressedMovementKeys: () => [...pressedMovementKeys].sort(),
    pressedOrbitKeys: () => [...pressedOrbitKeys].sort(),
    update: (deltaSeconds) => {
      if (!enabled || deltaSeconds <= 0) {
        return;
      }
      const forwardAxis = (pressedMovementKeys.has(preferences.keyboard.moveForward) ? 1 : 0)
        - (pressedMovementKeys.has(preferences.keyboard.moveBackward) ? 1 : 0);
      const rightAxis = (pressedMovementKeys.has(preferences.keyboard.moveRight) ? 1 : 0)
        - (pressedMovementKeys.has(preferences.keyboard.moveLeft) ? 1 : 0);
      const upAxis = (pressedMovementKeys.has(preferences.keyboard.moveUp) ? 1 : 0)
        - (pressedMovementKeys.has(preferences.keyboard.moveDown) ? 1 : 0);
      if (forwardAxis !== 0 || rightAxis !== 0 || upAxis !== 0) {
        const movement = cameraMovement(camera, forwardAxis, rightAxis, upAxis);
        if (movement !== null) {
          const boosted = pressedMovementKeys.has(preferences.keyboard.boost);
          const step = preferences.moveSpeed
            * (boosted ? preferences.boostMultiplier : 1)
            * deltaSeconds;
          const nextTarget = add(target, scale(movement, step));
          commitCamera(nextTarget, yawRadians, pitchRadians, distance, 'keyboard_movement');
        }
      }
      const yawAxis = (pressedOrbitKeys.has('ArrowLeft') ? 1 : 0)
        - (pressedOrbitKeys.has('ArrowRight') ? 1 : 0);
      const pitchAxis = (pressedOrbitKeys.has('ArrowUp') ? 1 : 0)
        - (pressedOrbitKeys.has('ArrowDown') ? 1 : 0);
      if (yawAxis !== 0 || pitchAxis !== 0) {
        const orbitStepRadians = degreesToRadians(keyboardOrbitDegreesPerSecond * deltaSeconds);
        const nextYawRadians = yawRadians + yawAxis * orbitStepRadians;
        const nextPitchRadians = clamp(
          pitchRadians + pitchAxis * orbitStepRadians,
          degreesToRadians(-MAXIMUM_PITCH_DEGREES),
          degreesToRadians(MAXIMUM_PITCH_DEGREES),
        );
        commitCamera(target, nextYawRadians, nextPitchRadians, distance, 'keyboard_orbit');
      }
    },
    dispose: () => {
      canvas.removeEventListener('pointerdown', onPointerDown);
      canvas.removeEventListener('pointermove', onPointerMove);
      canvas.removeEventListener('pointerup', onPointerEnd);
      canvas.removeEventListener('pointercancel', onPointerCancel);
      canvas.removeEventListener('lostpointercapture', onLostPointerCapture);
      canvas.removeEventListener('wheel', onWheel);
      canvas.removeEventListener('blur', clearInputState);
      ownerDocument.removeEventListener('pointerup', onPointerEnd);
      ownerDocument.removeEventListener('pointercancel', onPointerCancel);
      ownerDocument.removeEventListener('keydown', onKeyDown);
      ownerDocument.removeEventListener('keyup', onKeyUp);
      ownerDocument.removeEventListener('visibilitychange', onVisibilityChange);
      ownerWindow?.removeEventListener('blur', clearInputState);
      clearInputState();
      canvas.tabIndex = originalTabIndex;
      canvas.style.touchAction = originalTouchAction;
    },
  };
}

function resolveCamera(
  position: InspectionVector,
  target: InspectionVector,
  projection: PerspectiveProjection,
): RendererEditorViewportCamera {
  const resolution = resolveRendererStoredEditorCamera({ position, target, up: [0, 1, 0], projection });
  if (!resolution.ok) {
    throw new TypeError(resolution.diagnostic.message);
  }
  return resolution.camera;
}

function cameraMovement(
  camera: RendererEditorViewportCamera,
  forwardAxis: number,
  rightAxis: number,
  upAxis: number,
): InspectionVector | null {
  const forward = normalizeHorizontal(camera.basis.forward);
  const right = normalizeHorizontal(camera.basis.right);
  if (forward === null || right === null) {
    return null;
  }
  return normalize([
    forward[0] * forwardAxis + right[0] * rightAxis,
    upAxis,
    forward[2] * forwardAxis + right[2] * rightAxis,
  ]);
}

function positionFromOrbit(
  target: InspectionVector,
  distance: number,
  yawRadians: number,
  pitchRadians: number,
): InspectionVector {
  const horizontalDistance = Math.cos(pitchRadians) * distance;
  return [
    target[0] + Math.sin(yawRadians) * horizontalDistance,
    target[1] + Math.sin(pitchRadians) * distance,
    target[2] + Math.cos(yawRadians) * horizontalDistance,
  ];
}

function browserInspectionEnvironment(): RendererInspectionEnvironment {
  return {
    animation: {
      cancel: (handle) => globalThis.cancelAnimationFrame(handle),
      now: () => globalThis.performance?.now() ?? 0,
      request: (callback) => globalThis.requestAnimationFrame(callback),
    },
    createResizeObserver: (callback) => {
      if (globalThis.ResizeObserver === undefined) {
        return null;
      }
      return new globalThis.ResizeObserver(callback);
    },
    devicePixelRatio: () => globalThis.devicePixelRatio ?? 1,
  };
}

function isMovementKey(
  code: string,
  keyboard: RendererInspectionSurfaceKeyboardBindings,
): boolean {
  return Object.values(keyboard).some((movementKey) => movementKey === code);
}

function isOrbitKey(code: string): code is (typeof ORBIT_KEYS)[number] {
  return ORBIT_KEYS.some((orbitKey) => orbitKey === code);
}

function keyboardZoomDirection(event: KeyboardEvent): 'in' | 'out' | null {
  if (event.code === 'NumpadAdd' || event.key === '+') {
    return 'in';
  }
  if (event.code === 'Minus' || event.code === 'NumpadSubtract' || event.key === '-') {
    return 'out';
  }
  return null;
}

function requirePositiveFinite(value: number, label: string): number {
  if (!Number.isFinite(value) || value <= 0) {
    throw new TypeError(`${label} must be finite and positive`);
  }
  return value;
}

function requireUnitInterval(value: number, label: string): number {
  if (!Number.isFinite(value) || value <= 0 || value >= 1) {
    throw new TypeError(`${label} must be finite and between zero and one`);
  }
  return value;
}

function validateControlPreferences(
  value: RendererInspectionSurfaceControlPreferences,
): RendererInspectionSurfaceControlPreferences {
  const boostMultiplier = requirePositiveFinite(
    value.boostMultiplier,
    'inspection boostMultiplier',
  );
  if (boostMultiplier < 1) {
    throw new TypeError('inspection boostMultiplier must be at least one');
  }
  if (typeof value.invertLookY !== 'boolean' || typeof value.invertPanY !== 'boolean') {
    throw new TypeError('inspection camera inversion preferences must be boolean');
  }
  const requireBinding = (binding: unknown, name: string): string => {
    if (typeof binding !== 'string' || binding.trim().length === 0 || binding.length > 64) {
      throw new TypeError(`inspection ${name} keyboard binding must be a bounded non-empty code`);
    }
    return binding;
  };
  const keyboard: RendererInspectionSurfaceKeyboardBindings = {
    moveForward: requireBinding(value.keyboard?.moveForward, 'moveForward'),
    moveBackward: requireBinding(value.keyboard?.moveBackward, 'moveBackward'),
    moveLeft: requireBinding(value.keyboard?.moveLeft, 'moveLeft'),
    moveRight: requireBinding(value.keyboard?.moveRight, 'moveRight'),
    moveDown: requireBinding(value.keyboard?.moveDown, 'moveDown'),
    moveUp: requireBinding(value.keyboard?.moveUp, 'moveUp'),
    boost: requireBinding(value.keyboard?.boost, 'boost'),
  };
  return {
    moveSpeed: requirePositiveFinite(value.moveSpeed, 'inspection moveSpeed'),
    boostMultiplier,
    invertLookY: value.invertLookY,
    invertPanY: value.invertPanY,
    keyboard,
  };
}

function allFinite(vectors: readonly InspectionVector[]): boolean {
  return vectors.every((vector) => vector.every(Number.isFinite));
}

function vectorLength(vector: InspectionVector): number {
  return Math.hypot(vector[0], vector[1], vector[2]);
}

function normalize(vector: InspectionVector): InspectionVector | null {
  const length = vectorLength(vector);
  return length <= 0.000_001 ? null : scale(vector, 1 / length);
}

function normalizeHorizontal(vector: InspectionVector): InspectionVector | null {
  return normalize([vector[0], 0, vector[2]]);
}

function add(left: InspectionVector, right: InspectionVector): InspectionVector {
  return [left[0] + right[0], left[1] + right[1], left[2] + right[2]];
}

function subtract(left: InspectionVector, right: InspectionVector): InspectionVector {
  return [left[0] - right[0], left[1] - right[1], left[2] - right[2]];
}

function scale(vector: InspectionVector, amount: number): InspectionVector {
  return [vector[0] * amount, vector[1] * amount, vector[2] * amount];
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function degreesToRadians(degrees: number): number {
  return (degrees * Math.PI) / 180;
}
