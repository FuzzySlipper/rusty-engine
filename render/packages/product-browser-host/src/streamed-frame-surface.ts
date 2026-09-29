import type {
  PresentationFrameDiff,
  RendererViewComposition,
} from '@rusty-engine/render-contracts';
import type {
  RendererPresentationFrameReceipt,
  RendererPresentationHostSet,
  RendererSurface,
  RendererSurfaceDiagnosticsReadout,
  RendererSurfaceOptions,
  RendererSurfaceResourceOptions,
  RendererSurfaceStatistic,
  RendererSurfaceSubmissionSample,
} from '@rusty-engine/renderer-host';

/**
 * The world surface of the streaming browser mode.
 *
 * The runtime renders the world with wgpu; this surface pulls the frames it
 * draws, one request per frame, and paints them on the Engine canvas under
 * the product UI. It
 * realizes nothing of the world itself: frames, view compositions and world
 * presentation ops were already applied in the runtime, so they are
 * acknowledged here. Audio, video and the telemetry overlay stay browser
 * presentation hosts and receive their ops as before. The canvas stays the application's focus,
 * pointer-lock and input target.
 *
 * Wire format: see `rust/crates/product-dev-host/src/frames.rs`.
 */
export const PRODUCT_BROWSER_FRAME_STREAM_PATH = '/__rusty/product/runtime/frames';

const FRAME_MAGIC = 0x31465352; // "RSF1", little-endian
const FRAME_MIN_HEADER_BYTES = 40;
const FRAME_FORMAT_JPEG = 1;
const FRAME_FORMAT_RGBA8 = 2;
const FRAME_FLAG_HELD = 1;
const RETRY_DELAY_MS = 500;
/** UI-host domains the browser still realizes; the runtime renders the rest. */
/**
 * Domains the browser still realizes. The browser's video element plays
 * over the page UI, which a streamed frame lies under, so streaming leaves
 * video here; the desktop window draws video over the UI itself (#8791).
 */
const STREAMED_BROWSER_DOMAINS: ReadonlySet<string> = new Set(['audio', 'video', 'telemetryOverlay']);
const WINDOW_BROWSER_DOMAINS: ReadonlySet<string> = new Set(['audio', 'telemetryOverlay']);

interface StreamedFrame {
  readonly sequence: number;
  readonly step: number;
  readonly width: number;
  readonly height: number;
  readonly format: number;
  readonly held: boolean;
  readonly payload: Uint8Array;
}

/** Parses one `RSF1` frame: its header, then its payload. */
export function parseStreamedFrame(bytes: Uint8Array): StreamedFrame {
  const header = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (bytes.byteLength < FRAME_MIN_HEADER_BYTES || header.getUint32(0, true) !== FRAME_MAGIC) {
    throw new Error('frame response is not an RSF1 frame');
  }
  const headerBytes = header.getUint32(4, true);
  return {
    sequence: Number(header.getBigUint64(8, true)),
    step: Number(header.getBigUint64(16, true)),
    width: header.getUint32(24, true),
    height: header.getUint32(28, true),
    format: header.getUint8(32),
    held: (header.getUint8(33) & FRAME_FLAG_HELD) !== 0,
    payload: bytes.subarray(headerBytes, headerBytes + header.getUint32(36, true)),
  };
}

async function decodeFrame(frame: StreamedFrame): Promise<ImageBitmap> {
  if (frame.format === FRAME_FORMAT_JPEG) {
    return createImageBitmap(new Blob([frame.payload as Uint8Array<ArrayBuffer>], { type: 'image/jpeg' }));
  }
  if (frame.format === FRAME_FORMAT_RGBA8) {
    const pixels = new Uint8ClampedArray(frame.payload.buffer as ArrayBuffer, frame.payload.byteOffset, frame.payload.byteLength);
    return createImageBitmap(new ImageData(pixels, frame.width, frame.height));
  }
  throw new Error(`frame stream sent unknown format ${frame.format}`);
}

type Unsupported = (member: string) => never;
const unsupported: Unsupported = (member) => {
  throw new Error(`${member} is not available in the streaming browser mode; the runtime renders the world`);
};

const UNSUPPORTED_STATISTIC: RendererSurfaceStatistic = Object.freeze({
  scope: 'perSubmission',
  status: 'unsupported',
  value: null,
});

/**
 * Mounts the streaming surface on the Engine canvas. The signature matches
 * the Three surface's mount so the application host can take either.
 */
export function mountStreamedFrameSurface(
  canvas: HTMLCanvasElement,
  options: RendererSurfaceOptions | RendererSurfaceResourceOptions,
): RendererSurface {
  return mountRuntimeRenderedSurface(canvas, options, true);
}

/**
 * Mounts the desktop shell's surface: the runtime presents the world to the
 * native window under this page, so the canvas stays transparent and only
 * the input, focus and browser presentation hosts remain.
 */
export function mountWindowSurface(
  canvas: HTMLCanvasElement,
  options: RendererSurfaceOptions | RendererSurfaceResourceOptions,
): RendererSurface {
  return mountRuntimeRenderedSurface(canvas, options, false);
}

function mountRuntimeRenderedSurface(
  canvas: HTMLCanvasElement,
  options: RendererSurfaceOptions | RendererSurfaceResourceOptions,
  streamed: boolean,
): RendererSurface {
  const browserDomains = streamed ? STREAMED_BROWSER_DOMAINS : WINDOW_BROWSER_DOMAINS;
  const context = streamed ? canvas.getContext('2d', { alpha: false }) : null;
  if (streamed && context === null) throw new Error('the streaming surface needs a 2D canvas context');
  const pixelRatio = options.pixelRatio ?? 1;
  let hosts: RendererPresentationHostSet | null = options.presentationHosts ?? null;
  let composition: RendererViewComposition | null = options.viewComposition ?? null;
  let compositionRevision = 0;
  let latest: { frame: StreamedFrame; bitmap: ImageBitmap } | null = null;
  let pending: StreamedFrame | null = null;
  let decoding = false;
  let drawnSequence = 0;
  let drawnCount = 0;
  let lastDrawSourceMs: number | null = null;
  let lastDrawDurationMs: number | null = null;
  let submission: RendererSurfaceSubmissionSample | null = null;
  let receivedFrames = 0;
  let receivedBytes = 0;
  let pulling: AbortController | null = null;
  let animationFrame: number | null = null;
  let lastAnimationMs: number | null = null;
  let running = false;
  let disposed = false;

  const backingSize = (): [number, number] => [
    Math.max(1, Math.round(canvas.clientWidth * pixelRatio)),
    Math.max(1, Math.round(canvas.clientHeight * pixelRatio)),
  ];

  const draw = (source: 'animationFrame' | 'explicit', timeMs: number): RendererSurfaceSubmissionSample => {
    const started = performance.now();
    if (latest !== null) context?.drawImage(latest.bitmap, 0, 0, canvas.width, canvas.height);
    lastDrawDurationMs = performance.now() - started;
    const interval = lastDrawSourceMs === null ? null : timeMs - lastDrawSourceMs;
    lastDrawSourceMs = timeMs;
    drawnCount += 1;
    submission = Object.freeze({
      schemaVersion: 1,
      renderSequence: drawnCount,
      source,
      sourceTimeMs: timeMs,
      frameIntervalMs: interval !== null && interval >= 0 ? interval : null,
      frameIntervalStatus: interval === null ? 'firstFrame' : interval < 0 ? 'sourceTimeRegressed' : 'available',
      backendSubmissionDurationMs: lastDrawDurationMs,
      backendSubmissionDurationStatus: 'available',
      statistics: Object.freeze({
        schemaVersion: 1,
        drawCallCount: UNSUPPORTED_STATISTIC,
        renderHandleCount: UNSUPPORTED_STATISTIC,
        geometryResourceCount: UNSUPPORTED_STATISTIC,
        materialResourceCount: UNSUPPORTED_STATISTIC,
        textureResourceCount: UNSUPPORTED_STATISTIC,
        animatedInstanceCount: UNSUPPORTED_STATISTIC,
        triangleCount: UNSUPPORTED_STATISTIC,
      }),
    });
    return submission;
  };

  const showFrame = (frame: StreamedFrame, bitmap: ImageBitmap): void => {
    latest?.bitmap.close();
    latest = { frame, bitmap };
    // The harness can correlate a screenshot with the step it shows.
    canvas.dataset['rustyFrameSequence'] = String(frame.sequence);
    canvas.dataset['rustyFrameStep'] = String(frame.step);
    canvas.dataset['rustyFrameHeld'] = frame.held ? 'true' : 'false';
    draw('animationFrame', performance.now());
    drawnSequence = frame.sequence;
  };

  // Decode only the newest frame; frames that arrive meanwhile replace it.
  const decodeLatest = (): void => {
    if (decoding || pending === null || disposed) return;
    const frame = pending;
    pending = null;
    decoding = true;
    void decodeFrame(frame).then((bitmap) => {
      if (disposed) bitmap.close();
      else showFrame(frame, bitmap);
    }, () => undefined).finally(() => {
      decoding = false;
      decodeLatest();
    });
  };

  // Frames are pulled, never pushed, so a slow page skips frames instead of
  // queueing them in socket buffers. Two requests are outstanding: one takes
  // the next frame and the other already waits for the frame after it, so a
  // request's setup is never between a frame being drawn and its display.
  const pull = (controller: AbortController): void => {
    let newest = 0;
    let nextAfter = 0;
    const ask = async (after: number): Promise<void> => {
      while (!controller.signal.aborted) {
        const [width, height] = backingSize();
        try {
          const response = await fetch(
            `${PRODUCT_BROWSER_FRAME_STREAM_PATH}?after=${after}&width=${width}&height=${height}`,
            { cache: 'no-store', signal: controller.signal },
          );
          if (response.status === 204) continue;
          if (!response.ok) throw new Error(`frame request refused: HTTP ${response.status}`);
          const frame = parseStreamedFrame(new Uint8Array(await response.arrayBuffer()));
          if (frame.sequence > newest) {
            newest = frame.sequence;
            receivedFrames += 1;
            receivedBytes += frame.payload.byteLength;
            pending = frame;
            decodeLatest();
          }
          nextAfter = Math.max(nextAfter + 1, newest);
          void ask(nextAfter);
          return;
        } catch {
          if (controller.signal.aborted) return;
          await new Promise((resolve) => setTimeout(resolve, RETRY_DELAY_MS));
        }
      }
    };
    void ask(0);
    void ask(0);
  };

  // The canvas backs its CSS size; the next request asks for that size.
  const resize = (): void => {
    const [width, height] = backingSize();
    if (canvas.width === width && canvas.height === height) return;
    canvas.width = width;
    canvas.height = height;
    if (latest !== null) context?.drawImage(latest.bitmap, 0, 0, width, height);
  };
  const resizeObserver = new ResizeObserver(resize);
  resizeObserver.observe(canvas);

  // The one Engine cadence: input sampling and browser presentation hosts.
  const onAnimationFrame = (timeMs: number): void => {
    animationFrame = null;
    if (!running || disposed) return;
    const deltaSeconds = lastAnimationMs === null ? 0 : Math.max(0, timeMs - lastAnimationMs) / 1000;
    lastAnimationMs = timeMs;
    hosts?.advance(deltaSeconds);
    options.onAnimationFrame?.(timeMs);
    animationFrame = requestAnimationFrame(onAnimationFrame);
  };

  const start = (): void => {
    if (running || disposed) return;
    running = true;
    resize();
    if (streamed) {
      pulling = new AbortController();
      pull(pulling);
    }
    animationFrame = requestAnimationFrame(onAnimationFrame);
  };

  const stop = (): void => {
    running = false;
    pulling?.abort();
    pulling = null;
    if (animationFrame !== null) cancelAnimationFrame(animationFrame);
    animationFrame = null;
    lastAnimationMs = null;
  };

  const syncListener = (): void => {
    const pose = composition === null ? null : primaryListenerPose(composition);
    if (pose !== null) hosts?.syncListener(pose);
  };

  const cameraPose = (): ReturnType<RendererSurface['cameraPose']> => {
    const camera = composition === null ? undefined : primaryCamera(composition);
    return camera === undefined
      ? { position: [0, 0, 0], yawDegrees: 0, pitchDegrees: 0 }
      : { position: camera.pose.position, yawDegrees: camera.pose.yawDegrees, pitchDegrees: camera.pose.pitchDegrees };
  };

  const diagnosticsReadout = (): RendererSurfaceDiagnosticsReadout => ({
    schemaVersion: 1,
    renderer: streamed ? 'render-wgpu (streamed)' : 'render-wgpu (desktop window)',
    vendor: null,
    canvas: {
      cssWidth: canvas.clientWidth,
      cssHeight: canvas.clientHeight,
      backingWidth: canvas.width,
      backingHeight: canvas.height,
      effectivePixelRatio: pixelRatio,
    },
    submission: submission ?? draw('explicit', performance.now()),
    pacing: {
      schemaVersion: 1,
      mode: 'completionOnly',
      state: running ? 'ready' : 'idle',
      rendererClass: 'unknown',
      timerDurationMs: null,
      completionAgeMs: null,
      effectiveDurationMs: lastDrawDurationMs,
      admittedAtMs: lastDrawSourceMs,
      admissionObservedAtMs: lastDrawSourceMs,
      observedAtMs: performance.now(),
      automaticSubmissionCapacity: 1,
      automaticSubmissionLimit: 1,
      completionFenceMode: 'unsupported',
      maximumPendingSubmissions: 1,
      pendingSubmissionCount: decoding ? 1 : 0,
      maximumPendingMeasurements: 0,
      pendingMeasurementCount: 0,
      hostAdmission: {
        schemaVersion: 1,
        attemptCount: receivedFrames,
        admittedCount: drawnCount,
        backendBlockedCount: 0,
        noDemandCount: 0,
        firstAttemptAtMs: null,
        lastAttemptAtMs: lastDrawSourceMs,
        demandCounts: { requested: receivedFrames, viewportChanged: 0, controls: 0, presentation: 0, retainedAnimation: 0 },
        recentCallbackIntervalsMs: [],
        recentSubmissionIntervalsMs: [],
        recentAttempts: [],
      },
    },
    resources: {
      definedTextureCount: 0,
      skyBackground: { textureId: null, contentHash: null, resource: null },
      realizedTextures: [],
      spriteAtlasCount: 0,
      spriteFallbackCount: 0,
      materialFallbackCount: 0,
      voxelSpecializedMaterialCount: 0,
    },
    cadence: { state: 'ready', retainedFailureCount: 0, evictedFailureCount: 0, failures: [] },
  });

  const noAnimation = (): never => unsupported('animated mesh sampling');
  const surface: RendererSurface = {
    kind: 'rusty_renderer_surface.v1',
    backend: {
      family: streamed ? 'streamed-frames' : 'desktop-window',
      implementation: 'rusty-engine-renderer-backend',
      publicContract: 'rusty-renderer-surface.v1',
    },
    canvas,
    // The runtime realizes animation, particles and ghost plates; these
    // browser hosts never receive their ops in this mode.
    animationProjection: {
      kind: 'rusty_renderer_animated_mesh_projection.v1',
      applyFrame: () => ({ applied: true, outcome: 'applied', diagnostics: [] }),
      advance: () => ({ applied: true, outcome: 'applied', diagnostics: [] }),
      playback: noAnimation,
      snapshot: () => '{}',
      hasAnimationTarget: () => false,
      setAnimationControllerWeights: noAnimation,
      hasAnimationClips: () => false,
      clearAnimationControllerWeights: () => undefined,
      subscribeInspections: () => () => undefined,
      subscribeNaturalCompletions: () => () => undefined,
    },
    createParticleSink: () => ({
      create: () => undefined,
      update: () => undefined,
      destroy: () => undefined,
      readout: () => ({ activeParticles: 0, activeBatches: 0, billboardBatches: 0, cubeBatches: 0, allocatedSlots: 0, highWaterMark: 0 }),
      dispose: () => undefined,
    }),
    createGhostPlatePresentation: () => unsupported('ghost plates'),
    animatedMeshPlayback: noAnimation,
    sampleAnimatedMesh: noAnimation,
    applyFrame: () => ({ applied: true, outcome: 'applied', diagnostics: [] }),
    applyPresentation: async (frame: PresentationFrameDiff): Promise<RendererPresentationFrameReceipt> => {
      const browserOps = frame.ops.filter((op) => browserDomains.has(op.domain));
      const runtimeOps = frame.ops.length - browserOps.length;
      if (hosts === null || browserOps.length === 0) {
        return { schemaVersion: 1, applied: runtimeOps, outcome: 'applied', domains: [], diagnostics: [] };
      }
      const receipt = await hosts.apply({ ...frame, ops: browserOps });
      return { ...receipt, applied: receipt.applied + runtimeOps };
    },
    audioRealizedFacts: () => hosts?.readAudioRealizedFacts() ?? null,
    videoRealizedFacts: () => hosts?.readVideoRealizedFacts() ?? null,
    // Animation facts reach the Engine from the runtime's renderer.
    animationRealizedFacts: () => null,
    ghostPlateReadout: () => null,
    automaticSubmissionPacing: () => diagnosticsReadout().pacing,
    diagnosticsReadout,
    cameraPose,
    cameraProjection: () => unsupported('camera projection'),
    inputReadout: () => ({ enabled: false, pointerLocked: document.pointerLockElement === canvas, pressedCodes: [] }),
    lightingReadout: () => unsupported('lighting readout'),
    visibilityReadout: () => unsupported('visibility readout'),
    configureViews: (next: RendererViewComposition) => {
      composition = next;
      compositionRevision += 1;
      syncListener();
      return { applied: true, outcome: 'applied', diagnostics: [], revision: compositionRevision };
    },
    viewCompositionReadout: () => ({
      schemaVersion: 1,
      revision: compositionRevision,
      cameras: composition?.cameras ?? [],
      cameraSamples: [],
      sourceCameras: composition?.cameras ?? [],
      targets: [],
      views: composition?.views ?? [],
      presentations: composition?.presentations ?? [],
      resources: { presentationCount: composition?.presentations.length ?? 0, targetCount: 0 },
    }),
    lockPointer: () => {
      void canvas.requestPointerLock();
    },
    movementState: () => ({ mode: 'caller_resolved', blockedAxes: [], collided: false, resolutionId: null }),
    // No browser or runtime caller asks the renderer for a pick.
    pick: () => unsupported('renderer picking'),
    pointerLocked: () => document.pointerLockElement === canvas,
    projectWorldPoint: () => unsupported('world point projection'),
    releaseInput: () => undefined,
    acknowledgeAudioRealizedFacts: (throughFactId) => hosts?.acknowledgeAudioRealizedFacts(throughFactId) ?? false,
    acknowledgeAnimationRealizedFacts: () => false,
    resetAudioRealizationOwner: () => hosts?.resetAudioRealizationOwner() ?? false,
    acknowledgeVideoRealizedFacts: (throughFactId) => hosts?.acknowledgeVideoRealizedFacts(throughFactId) ?? false,
    resetVideoRealizationOwner: () => hosts?.resetVideoRealizationOwner() ?? false,
    resetAnimationRealizationOwner: () => false,
    resetCameraMotion: () => undefined,
    retainResources: () => undefined,
    renderOnce: (timeMs) => draw('explicit', timeMs ?? performance.now()),
    // Held time, drawing, the observer camera and explicit frames are the
    // runtime renderer's (`engine.renderer.*`); the page never asks this
    // surface for them.
    inspection: () => unsupported('surface inspection (use engine.renderer.camera, .drawing and .frame)'),
    executeRenderOutput: async () => unsupported('render output images'),
    resetCamera: () => undefined,
    setCameraPose: () => undefined,
    setPresentationHosts: (next) => {
      hosts = next;
      syncListener();
    },
    nodeReadout: () => unsupported('node readout'),
    snapshot: () => JSON.stringify({
      backend: 'streamed-frames',
      frameSequence: drawnSequence,
      frameStep: latest?.frame.step ?? null,
      held: latest?.frame.held ?? null,
      receivedFrames,
      receivedBytes,
      size: [canvas.width, canvas.height],
    }),
    start,
    stop,
    submission: () => {
      if (submission === null) throw new Error('renderer surface has not submitted a frame');
      return submission;
    },
    timing: () => {
      if (submission === null) throw new Error('renderer surface has not submitted a frame');
      const { statistics: _statistics, ...timing } = submission;
      return timing;
    },
    dispose: () => {
      if (disposed) return;
      stop();
      disposed = true;
      resizeObserver.disconnect();
      latest?.bitmap.close();
      latest = null;
    },
  };
  if (options.autoStart !== false) start();
  return surface;
}

type CompositionCamera = RendererViewComposition['cameras'][number];

function primaryCamera(composition: RendererViewComposition): CompositionCamera | undefined {
  const view = composition.views
    .filter((candidate) => candidate.target.kind === 'primary')
    .sort((left, right) => left.order - right.order || left.id.localeCompare(right.id))[0];
  return view === undefined ? undefined : composition.cameras.find((camera) => camera.id === view.cameraId);
}

/** The listener follows the primary view's camera, as the Three surface does. */
function primaryListenerPose(
  composition: RendererViewComposition,
): Parameters<RendererPresentationHostSet['syncListener']>[0] | null {
  const camera = primaryCamera(composition);
  if (camera === undefined) return null;
  const { pose } = camera;
  if (camera.basis !== undefined) {
    return { position: pose.position, forward: normalize(camera.basis.forward), up: normalize(camera.basis.up) };
  }
  const yaw = (pose.yawDegrees * Math.PI) / 180;
  const pitch = (pose.pitchDegrees * Math.PI) / 180;
  return {
    position: pose.position,
    forward: [Math.sin(yaw) * Math.cos(pitch), Math.sin(pitch), -Math.cos(yaw) * Math.cos(pitch)],
    up: [-Math.sin(yaw) * Math.sin(pitch), Math.cos(pitch), Math.cos(yaw) * Math.sin(pitch)],
  };
}

function normalize(vector: readonly [number, number, number]): [number, number, number] {
  const length = Math.hypot(...vector) || 1;
  return [vector[0] / length, vector[1] / length, vector[2] / length];
}
