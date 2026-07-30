// Browser/canvas surface built on the retained ThreeRenderer.

import * as THREE from 'three';
import { RenderProjection } from '@rusty-engine/render-projection';
import {
  renderHandle,
  type CameraBasis,
  type Geometry,
  type RenderFrameDiff,
  type RenderHandle,
  type RenderLayer,
  type RenderNode,
  type PerspectiveProjection,
  type Transform,
} from '@rusty-engine/render-contracts';
import {
  ThreeRenderer,
  type MeshBufferSource,
  type MeshResourceSource,
  type RendererProjectionIdentity,
  type ThreeRendererResourceStatistics,
} from './three-renderer.js';
import type { AnimatedMeshAssetSource, AnimatedMeshPlaybackReadout } from './animated-mesh.js';
import { renderBrowserSurfaceFrame } from './browser-surface-render-pass.js';
import {
  RendererGpuSubmissionFence,
  type RendererGpuSubmissionFenceDriver,
} from './gpu-submission-fence.js';
import {
  RendererGpuSubmissionDuty,
  type RendererGpuSubmissionDutySample,
  type RendererGpuSubmissionClass,
  type RendererGpuSubmissionTimerDriver,
} from './gpu-submission-duty.js';
import { classifyGpuSubmissionRendererName } from './gpu-submission-class.js';
import { resolveRendererPixelRatio } from './software-renderer-resolution.js';

export interface ProjectedThreeRenderResult {
  readonly projection: RenderProjection;
  readonly renderer: ThreeRenderer;
  readonly structuralSnapshot: string;
}

export interface RendererBrowserSurfaceOptions {
  readonly animatedMeshSource?: AnimatedMeshAssetSource;
  readonly autoStart?: boolean;
  readonly camera?: RendererBrowserSurfaceCameraOptions;
  readonly clearColor?: number;
  readonly frame?: RenderFrameDiff;
  readonly meshBufferSource?: MeshBufferSource;
  readonly meshResourceSource?: MeshResourceSource;
  readonly pixelRatio?: number;
}

export interface RendererBrowserSurfaceCameraPose {
  readonly position: readonly [number, number, number];
  readonly pitchDegrees: number;
  readonly yawDegrees: number;
}

export type RendererBrowserSurfaceCameraBasis = CameraBasis;

export interface RendererBrowserSurfaceCameraOptions {
  readonly initialBasis?: RendererBrowserSurfaceCameraBasis;
  readonly initialPose?: RendererBrowserSurfaceCameraPose;
  readonly projection?: PerspectiveProjection;
}

export interface RendererBrowserSurfaceWorldProjection {
  readonly xPixels: number;
  readonly yPixels: number;
  readonly depth: number;
  readonly distance: number;
  readonly insideViewport: boolean;
  readonly occluded: false;
}

export type RendererBrowserSurfacePickRay =
  | {
      readonly kind: 'viewport';
      /** Normalized device coordinates, each bounded to [-1, 1]. */
      readonly point: readonly [number, number];
    }
  | {
      readonly kind: 'world_ray';
      readonly direction: readonly [number, number, number];
      readonly origin: readonly [number, number, number];
    };

export interface RendererBrowserSurfacePickFilter {
  readonly handles?: readonly RenderHandle[];
  readonly labels?: readonly string[];
  readonly layers?: readonly RenderLayer[];
  readonly tags?: readonly string[];
}

export interface RendererBrowserSurfacePickRequest {
  readonly filter?: RendererBrowserSurfacePickFilter;
  readonly maxDistance?: number;
  readonly ray: RendererBrowserSurfacePickRay;
}

export type RendererBrowserSurfacePickDiagnosticCode =
  | 'invalid_viewport_point'
  | 'invalid_world_ray'
  | 'invalid_max_distance'
  | 'filter_limit_exceeded';

export interface RendererBrowserSurfacePickDiagnostic {
  readonly code: RendererBrowserSurfacePickDiagnosticCode;
  readonly message: string;
}

export interface RendererBrowserSurfacePickHit {
  readonly channel: 'render_projection';
  readonly distance: number;
  readonly handle: RenderHandle;
  readonly label: string | null;
  readonly layer: RenderLayer;
  readonly normal: readonly [number, number, number];
  readonly position: readonly [number, number, number];
  readonly sourceTrace: {
    readonly entity: number;
    readonly kind: 'render_metadata_entity';
  } | null;
  readonly tags: readonly string[];
}

export interface RendererBrowserSurfacePickReceipt {
  readonly diagnostics: readonly RendererBrowserSurfacePickDiagnostic[];
  readonly hit: RendererBrowserSurfacePickHit | null;
  readonly kind: 'rusty_renderer_browser_surface_pick.v1';
}

/** Exact Three submission facts consumed by the renderer-neutral host adapter. */
export interface RendererBrowserSurfaceSubmissionStatistics extends ThreeRendererResourceStatistics {
  readonly schemaVersion: 1;
  readonly drawCallCount: number;
  readonly triangleCount: number;
}

export interface RendererBrowserSurface {
  readonly kind: 'rusty_renderer_browser_surface.v1';
  readonly canvas: HTMLCanvasElement;
  readonly renderer: ThreeRenderer;
  readonly frame: RenderFrameDiff;
  readonly cameraPose: () => RendererBrowserSurfaceCameraPose;
  readonly cameraProjection: () => PerspectiveProjection;
  readonly projectWorldPoint: (
    position: readonly [number, number, number],
  ) => RendererBrowserSurfaceWorldProjection;
  readonly animatedMeshPlayback: (handle: import('@rusty-engine/render-contracts').RenderHandle) => AnimatedMeshPlaybackReadout | undefined;
  readonly applyFrame: (frame: RenderFrameDiff) => void;
  readonly pick: (request: RendererBrowserSurfacePickRequest) => RendererBrowserSurfacePickReceipt;
  readonly snapshot: () => string;
  /** Internal automatic-loop readiness; explicit renderOnce remains unconditional. */
  readonly automaticSubmissionReady: () => boolean;
  /** Immutable backend pacing state and latest completed admission decision. */
  readonly automaticSubmissionPacing: () => RendererGpuSubmissionDutySample;
  readonly renderOnce: (timeMs?: number) => RendererBrowserSurfaceSubmissionStatistics;
  readonly setCameraPose: (
    pose: RendererBrowserSurfaceCameraPose,
    basis?: RendererBrowserSurfaceCameraBasis,
  ) => void;
  readonly start: () => void;
  readonly stop: () => void;
  readonly dispose: () => void;
}

/**
 * Apply a render frame through the renderer-neutral projection and then the
 * retained Three.js renderer. This is the package-root bridge used by demo
 * proofs: no authority state, no raw transport, no arbitrary JSON tunnel.
 */
export function renderProjectedFrame(
  frame: RenderFrameDiff,
  renderer: ThreeRenderer = new ThreeRenderer(),
): ProjectedThreeRenderResult {
  const projection = new RenderProjection();
  projection.applyFrame(frame);
  renderer.applyFrame(frame);
  return {
    projection,
    renderer,
    structuralSnapshot: renderer.snapshot(),
  };
}

/**
 * A tiny public browser surface for consumers that need to prove the real
 * renderer path: Rusty Engine render diffs -> retained ThreeRenderer -> WebGL canvas.
 *
 * The consumer owns only the canvas element. Three.js scene/camera/WebGL details
 * stay inside `@rusty-engine/renderer-three`.
 */
export function mountRendererBrowserSurface(
  canvas: HTMLCanvasElement,
  options: RendererBrowserSurfaceOptions = {},
): RendererBrowserSurface {
  const renderer = new ThreeRenderer(
    {
      ...(options.animatedMeshSource === undefined
        ? {} : { animatedMeshSource: options.animatedMeshSource }),
      ...(options.meshBufferSource === undefined
        ? {} : { meshBufferSource: options.meshBufferSource }),
      ...(options.meshResourceSource === undefined
        ? {} : { meshResourceSource: options.meshResourceSource }),
    },
  );
  // Defined retained materials use MeshStandardMaterial. Keep the browser host responsible
  // for a small neutral light rig; the retained projection carries appearance
  // parameters, never renderer-owned light state or gameplay authority.
  const ambientLight = new THREE.HemisphereLight(0xffffff, 0x263238, 2.4);
  const keyLight = new THREE.DirectionalLight(0xffffff, 2.2);
  keyLight.position.set(5, 8, 6);
  renderer.scene.add(ambientLight, keyLight);
  const viewmodelAmbientLight = new THREE.HemisphereLight(0xffffff, 0x263238, 2.4);
  const viewmodelKeyLight = new THREE.DirectionalLight(0xffffff, 2.2);
  viewmodelKeyLight.position.set(2, 3, 2);
  renderer.viewmodelScene.add(viewmodelAmbientLight, viewmodelKeyLight);
  const frame = options.frame ?? createRendererBrowserSurfaceFrame();
  renderer.applyFrame(frame);

  const webgl = new THREE.WebGLRenderer({ canvas, antialias: true });
  const webglContext = webgl.getContext();
  const gpuSubmissionClass = classifyGpuSubmissionRenderer(webglContext);
  const gpuSubmissionFence = new RendererGpuSubmissionFence(
    webGl2SubmissionFenceDriver(webglContext),
  );
  const gpuSubmissionDuty = new RendererGpuSubmissionDuty(
    webGl2SubmissionTimerDriver(webglContext),
    { rendererClass: gpuSubmissionClass },
  );
  webgl.autoClear = false;
  // One surface submission contains both world and viewmodel render passes.
  // Disable Three's per-pass reset so its public info object accumulates exact
  // counts until this owner resets it before the next submission.
  webgl.info.autoReset = false;
  webgl.setClearColor(options.clearColor ?? 0x101820, 1);
  const requestedPixelRatio = options.pixelRatio ?? globalThis.devicePixelRatio ?? 1;
  const pixelRatio = resolveRendererPixelRatio(
    requestedPixelRatio,
    gpuSubmissionClass,
  );
  webgl.setPixelRatio(pixelRatio);

  const cameraProjection = validatePerspectiveProjection(
    options.camera?.projection ?? { fovYDegrees: 55, near: 0.1, far: 100 },
  );
  const camera = new THREE.PerspectiveCamera(
    cameraProjection.fovYDegrees,
    1,
    cameraProjection.near,
    cameraProjection.far,
  );
  camera.name = 'world-camera';
  const viewmodelCamera = new THREE.PerspectiveCamera(
    cameraProjection.fovYDegrees,
    1,
    cameraProjection.near,
    cameraProjection.far,
  );
  viewmodelCamera.name = 'viewmodel-camera';
  const raycaster = new THREE.Raycaster();
  const center = new THREE.Vector2(0, 0);
  const cameraLookTarget = new THREE.Vector3();
  let currentCameraPose: RendererBrowserSurfaceCameraPose =
    options.camera?.initialPose ?? {
      position: [0, 1.62, 8],
      pitchDegrees: 0,
      yawDegrees: 0,
    };
  let currentCameraBasis = options.camera?.initialBasis ?? null;

  let animationFrame: number | null = null;
  let lastRenderTimeMs: number | null = null;
  let logicalViewport = { width: 0, height: 0 };
  let disposed = false;

  const setCameraPose = (
    pose: RendererBrowserSurfaceCameraPose,
    basis?: RendererBrowserSurfaceCameraBasis,
  ): void => {
    currentCameraPose = pose;
    currentCameraBasis = basis ?? null;
    camera.position.set(pose.position[0], pose.position[1], pose.position[2]);
    if (currentCameraBasis === null) {
      camera.up.set(0, 1, 0);
      camera.rotation.order = 'YXZ';
      camera.rotation.x = degreesToRadians(pose.pitchDegrees);
      camera.rotation.y = degreesToRadians(pose.yawDegrees);
      camera.rotation.z = 0;
      return;
    }
    camera.up.set(currentCameraBasis.up[0], currentCameraBasis.up[1], currentCameraBasis.up[2]);
    cameraLookTarget.set(
      camera.position.x + currentCameraBasis.forward[0],
      camera.position.y + currentCameraBasis.forward[1],
      camera.position.z + currentCameraBasis.forward[2],
    );
    camera.lookAt(cameraLookTarget);
  };

  const resize = (): void => {
    const width = Math.max(
      1,
      canvas.clientWidth || Math.round(canvas.width / requestedPixelRatio) || 800,
    );
    const height = Math.max(
      1,
      canvas.clientHeight || Math.round(canvas.height / requestedPixelRatio) || 450,
    );
    if (logicalViewport.width !== width || logicalViewport.height !== height) {
      webgl.setSize(width, height, false);
      logicalViewport = { width, height };
    }
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
    viewmodelCamera.aspect = width / height;
    viewmodelCamera.updateProjectionMatrix();
  };

  const renderOnce = (
    timeMs = globalThis.performance?.now() ?? 0,
  ): RendererBrowserSurfaceSubmissionStatistics => {
    if (disposed) throw new Error('renderer browser surface is disposed');
    resize();
    const deltaSeconds =
      lastRenderTimeMs === null
        ? 0
        : Math.min(0.05, Math.max(0, (timeMs - lastRenderTimeMs) / 1000));
    lastRenderTimeMs = timeMs;
    webgl.info.reset();
    gpuSubmissionDuty.begin();
    try {
      renderBrowserSurfaceFrame(
        webgl,
        camera,
        viewmodelCamera,
        renderer,
        deltaSeconds,
      );
    } catch (cause) {
      gpuSubmissionDuty.aborted();
      throw cause;
    }
    gpuSubmissionDuty.submitted();
    gpuSubmissionFence.submitted();
    return Object.freeze({
      schemaVersion: 1,
      drawCallCount: webgl.info.render.calls,
      triangleCount: webgl.info.render.triangles,
      ...renderer.resourceStatistics(),
    });
  };

  const automaticSubmissionReady = (): boolean => {
    // Poll both completion owners independently. A pending exact fence still
    // blocks admission, but it must not prevent the timer query from becoming
    // observable and computing the next duty deadline.
    const fenceReady = gpuSubmissionFence.ready();
    const dutyReady = gpuSubmissionDuty.ready();
    return fenceReady && dutyReady;
  };

  const projectWorldPoint = (
    position: readonly [number, number, number],
  ): RendererBrowserSurfaceWorldProjection => {
    resize();
    camera.updateMatrixWorld(true);
    return projectWorldPointWithPerspectiveCamera(
      camera,
      logicalViewport,
      position,
    );
  };

  const tick = (timeMs: number): void => {
    if (automaticSubmissionReady()) {
      renderOnce(timeMs);
    }
    animationFrame = globalThis.requestAnimationFrame(tick);
  };

  const start = (): void => {
    if (disposed) throw new Error('renderer browser surface is disposed');
    if (animationFrame !== null) {
      return;
    }
    animationFrame = globalThis.requestAnimationFrame(tick);
  };

  const stop = (): void => {
    if (animationFrame === null) {
      return;
    }
    globalThis.cancelAnimationFrame(animationFrame);
    animationFrame = null;
  };

  const dispose = (): void => {
    if (disposed) return;
    stop();
    gpuSubmissionFence.dispose();
    gpuSubmissionDuty.dispose();
    webgl.dispose();
    renderer.dispose();
    disposed = true;
  };

  setCameraPose(currentCameraPose, currentCameraBasis ?? undefined);
  renderOnce(0);
  if (options.autoStart !== false) {
    start();
  }

  return {
    kind: 'rusty_renderer_browser_surface.v1',
    canvas,
    renderer,
    frame,
    automaticSubmissionPacing: () => gpuSubmissionDuty.sample(),
    automaticSubmissionReady,
    animatedMeshPlayback: (handle) => renderer.animatedMeshPlayback(handle),
    applyFrame: (nextFrame) => renderer.applyFrame(nextFrame),
    cameraPose: () => currentCameraPose,
    cameraProjection: () => cameraProjection,
    projectWorldPoint,
    pick: (request) => pickProjectedObject(renderer, camera, raycaster, center, request),
    snapshot: () => renderer.snapshot(),
    renderOnce,
    setCameraPose,
    start,
    stop,
    dispose,
  };
}

function webGl2SubmissionFenceDriver(
  context: WebGLRenderingContext | WebGL2RenderingContext,
): RendererGpuSubmissionFenceDriver | null {
  if (!('fenceSync' in context)) {
    return null;
  }
  const webgl2 = context;
  return {
    create: () => webgl2.fenceSync(webgl2.SYNC_GPU_COMMANDS_COMPLETE, 0),
    delete: (fence) => webgl2.deleteSync(fence as WebGLSync),
    flush: () => webgl2.flush(),
    poll: (fence) => {
      const status = webgl2.clientWaitSync(fence as WebGLSync, 0, 0);
      if (status === webgl2.TIMEOUT_EXPIRED) {
        return 'pending';
      }
      if (status === webgl2.ALREADY_SIGNALED || status === webgl2.CONDITION_SATISFIED) {
        return 'signaled';
      }
      return 'failed';
    },
  };
}

function webGl2SubmissionTimerDriver(
  context: WebGLRenderingContext | WebGL2RenderingContext,
): RendererGpuSubmissionTimerDriver | null {
  if (!('createQuery' in context)) {
    return null;
  }
  const webgl2 = context;
  const timer = webgl2.getExtension('EXT_disjoint_timer_query_webgl2');
  if (timer === null) {
    return null;
  }
  return {
    begin: () => {
      const query = webgl2.createQuery();
      if (query === null) {
        return null;
      }
      webgl2.beginQuery(timer.TIME_ELAPSED_EXT, query);
      return query;
    },
    delete: (query) => webgl2.deleteQuery(query as WebGLQuery),
    end: () => webgl2.endQuery(timer.TIME_ELAPSED_EXT),
    now: () => globalThis.performance?.now() ?? 0,
    poll: (query) => {
      if (webgl2.getParameter(timer.GPU_DISJOINT_EXT) === true) {
        return { status: 'failed' };
      }
      if (webgl2.getQueryParameter(
        query as WebGLQuery,
        webgl2.QUERY_RESULT_AVAILABLE,
      ) !== true) {
        return { status: 'pending' };
      }
      const nanoseconds = webgl2.getQueryParameter(
        query as WebGLQuery,
        webgl2.QUERY_RESULT,
      );
      return typeof nanoseconds === 'number'
        ? { durationMs: nanoseconds / 1_000_000, status: 'complete' }
        : { status: 'failed' };
    },
  };
}

function classifyGpuSubmissionRenderer(
  context: WebGLRenderingContext | WebGL2RenderingContext,
): RendererGpuSubmissionClass {
  let renderer: unknown;
  try {
    const debug = context.getExtension('WEBGL_debug_renderer_info');
    if (debug === null) {
      return 'unknown';
    }
    renderer = context.getParameter(debug.UNMASKED_RENDERER_WEBGL);
  } catch {
    return 'unknown';
  }
  return classifyGpuSubmissionRendererName(renderer);
}

export function projectWorldPointWithPerspectiveCamera(
  camera: THREE.PerspectiveCamera,
  viewport: { readonly width: number; readonly height: number },
  position: readonly [number, number, number],
): RendererBrowserSurfaceWorldProjection {
  const projected = new THREE.Vector3(...position).project(camera);
  const distance = camera.position.distanceTo(new THREE.Vector3(...position));
  const insideViewport =
    projected.x >= -1 && projected.x <= 1
    && projected.y >= -1 && projected.y <= 1
    && projected.z >= -1 && projected.z <= 1;
  return {
    xPixels: ((projected.x + 1) / 2) * viewport.width,
    yPixels: ((1 - projected.y) / 2) * viewport.height,
    depth: Math.max(0, Math.min(1, (projected.z + 1) / 2)),
    distance,
    insideViewport,
    occluded: false,
  };
}

function validatePerspectiveProjection(projection: PerspectiveProjection): PerspectiveProjection {
  const values = [projection.fovYDegrees, projection.near, projection.far];
  if (
    !values.every(Number.isFinite)
    || projection.fovYDegrees <= 0
    || projection.fovYDegrees >= 180
    || projection.near <= 0
    || projection.far <= projection.near
  ) {
    throw new RangeError('camera projection must have a finite FOV in (0, 180) and 0 < near < far');
  }
  return {
    fovYDegrees: projection.fovYDegrees,
    near: projection.near,
    far: projection.far,
  };
}

export function createRendererBrowserSurfaceFrame(): RenderFrameDiff {
  const cubeSpecs = createBrowserSurfaceCubeSpecs();
  return {
    schemaVersion: 1,
    ops: [
      {
        op: 'create',
        handle: renderHandle(4103001),
        parent: null,
        node: primitiveNode('rusty-renderer-flat-plane', 'cube', [0, -0.08, 0], [18, 0.16, 18], [
          0.16,
          0.22,
          0.2,
          1,
        ]),
      },
      {
        op: 'create',
        handle: renderHandle(4103002),
        parent: null,
        node: primitiveNode('rusty-renderer-collision-wall-north', 'cube', [0, 0.5, -2.5], [6, 3, 1], [
          0.32,
          0.38,
          0.42,
          1,
        ]),
      },
      {
        op: 'create',
        handle: renderHandle(4103003),
        parent: null,
        node: primitiveNode('rusty-renderer-collision-wall-south', 'cube', [0, 0.5, 2.5], [6, 3, 1], [
          0.32,
          0.38,
          0.42,
          1,
        ]),
      },
      {
        op: 'create',
        handle: renderHandle(4103004),
        parent: null,
        node: primitiveNode('rusty-renderer-collision-wall-west', 'cube', [-2.5, 0.5, 0], [1, 3, 6], [
          0.27,
          0.34,
          0.37,
          1,
        ]),
      },
      {
        op: 'create',
        handle: renderHandle(4103005),
        parent: null,
        node: primitiveNode('rusty-renderer-collision-wall-east', 'cube', [2.5, 0.5, 0], [1, 3, 6], [
          0.27,
          0.34,
          0.37,
          1,
        ]),
      },
      ...cubeSpecs.map((cube, index) => ({
        op: 'create' as const,
        handle: renderHandle(4103100 + index),
        parent: null,
        node: primitiveNode(
          `rusty-renderer-random-cube-${String(index + 1).padStart(2, '0')}`,
          'cube',
          [cube.position[0], cube.size[1] / 2, cube.position[1]],
          cube.size,
          cube.color,
        ),
      })),
    ],
  };
}

const MAX_PICK_FILTER_VALUES = 128;

export function pickProjectedObject(
  renderer: ThreeRenderer,
  camera: THREE.PerspectiveCamera,
  raycaster: THREE.Raycaster,
  center: THREE.Vector2,
  request: RendererBrowserSurfacePickRequest,
): RendererBrowserSurfacePickReceipt {
  const diagnostics = validatePickRequest(request);
  if (diagnostics.length > 0) {
    return { diagnostics, hit: null, kind: 'rusty_renderer_browser_surface_pick.v1' };
  }

  renderer.prepareStaticInstanceBatchesForPicking();
  renderer.scene.updateMatrixWorld(true);
  configurePickRay(raycaster, camera, center, request.ray);
  raycaster.far = request.maxDistance ?? Number.POSITIVE_INFINITY;
  const intersections = raycaster.intersectObjects(renderer.scene.children, true);
  for (const intersection of intersections) {
    const identity = renderer.projectionIdentityForObject(
      intersection.object,
      intersection.instanceId,
    );
    if (identity === undefined || !pickIdentityMatches(identity, request.filter)) {
      continue;
    }
    const worldNormal = intersection.face?.normal.clone() ?? new THREE.Vector3(0, 0, 0);
    if (intersection.face !== null && intersection.face !== undefined) {
      worldNormal.copy(renderer.projectionWorldNormalForObject(
        intersection.object,
        intersection.instanceId,
        intersection.face.normal,
      ));
    }
    return {
      diagnostics: [],
      hit: {
        channel: 'render_projection',
        distance: Number(intersection.distance.toFixed(4)),
        handle: identity.handle,
        label: identity.metadata.label,
        layer: identity.layer,
        normal: [worldNormal.x, worldNormal.y, worldNormal.z],
        position: [intersection.point.x, intersection.point.y, intersection.point.z],
        sourceTrace: identity.metadata.sourceEntity === null
          ? null
          : { entity: identity.metadata.sourceEntity, kind: 'render_metadata_entity' },
        tags: [...identity.metadata.tags],
      },
      kind: 'rusty_renderer_browser_surface_pick.v1',
    };
  }
  return { diagnostics: [], hit: null, kind: 'rusty_renderer_browser_surface_pick.v1' };
}

function configurePickRay(
  raycaster: THREE.Raycaster,
  camera: THREE.PerspectiveCamera,
  center: THREE.Vector2,
  request: RendererBrowserSurfacePickRay,
): void {
  if (request.kind === 'viewport') {
    center.set(request.point[0], request.point[1]);
    raycaster.setFromCamera(center, camera);
    return;
  }
  raycaster.set(
    new THREE.Vector3(...request.origin),
    new THREE.Vector3(...request.direction).normalize(),
  );
}

function validatePickRequest(
  request: RendererBrowserSurfacePickRequest,
): RendererBrowserSurfacePickDiagnostic[] {
  if (request.maxDistance !== undefined && (!Number.isFinite(request.maxDistance) || request.maxDistance <= 0)) {
    return [{ code: 'invalid_max_distance', message: 'maxDistance must be finite and greater than zero' }];
  }
  const filterCounts = [
    request.filter?.handles?.length ?? 0,
    request.filter?.labels?.length ?? 0,
    request.filter?.layers?.length ?? 0,
    request.filter?.tags?.length ?? 0,
  ];
  if (filterCounts.some((count) => count > MAX_PICK_FILTER_VALUES)) {
    return [{ code: 'filter_limit_exceeded', message: `pick filters may contain at most ${MAX_PICK_FILTER_VALUES} values` }];
  }
  if (request.ray.kind === 'viewport') {
    const [x, y] = request.ray.point;
    if (![x, y].every(Number.isFinite) || x < -1 || x > 1 || y < -1 || y > 1) {
      return [{ code: 'invalid_viewport_point', message: 'viewport coordinates must be finite and within [-1, 1]' }];
    }
    return [];
  }
  const values = [...request.ray.origin, ...request.ray.direction];
  const directionLength = Math.hypot(...request.ray.direction);
  if (!values.every(Number.isFinite) || directionLength === 0) {
    return [{ code: 'invalid_world_ray', message: 'world ray values must be finite and direction must be non-zero' }];
  }
  return [];
}

function pickIdentityMatches(
  identity: RendererProjectionIdentity,
  filter: RendererBrowserSurfacePickFilter | undefined,
): boolean {
  if (filter === undefined) {
    return true;
  }
  if (filter.handles !== undefined && !filter.handles.includes(identity.handle)) {
    return false;
  }
  if (filter.labels !== undefined && (identity.metadata.label === null || !filter.labels.includes(identity.metadata.label))) {
    return false;
  }
  if (filter.layers !== undefined && !filter.layers.includes(identity.layer)) {
    return false;
  }
  if (filter.tags !== undefined && !filter.tags.every((tag) => identity.metadata.tags.some((value) => value === tag))) {
    return false;
  }
  return true;
}

interface BrowserSurfaceCubeSpec {
  readonly color: readonly [number, number, number, number];
  readonly position: readonly [number, number];
  readonly size: readonly [number, number, number];
}

function createBrowserSurfaceCubeSpecs(): readonly BrowserSurfaceCubeSpec[] {
  const random = deterministicUnitGenerator(0x4103c0de);
  const colors: readonly (readonly [number, number, number, number])[] = [
    [0.28, 0.66, 0.92, 1],
    [0.92, 0.54, 0.32, 1],
    [0.46, 0.78, 0.42, 1],
    [0.82, 0.58, 0.92, 1],
    [0.92, 0.76, 0.28, 1],
  ];
  const cubes: BrowserSurfaceCubeSpec[] = [
    {
      color: colors[0] as readonly [number, number, number, number],
      position: [0, -1.35],
      size: [0.62, 2.2, 0.62],
    },
    {
      color: colors[1] as readonly [number, number, number, number],
      position: [1.25, -0.65],
      size: [0.48, 0.85, 0.48],
    },
    {
      color: colors[2] as readonly [number, number, number, number],
      position: [-1.15, -0.9],
      size: [0.52, 1.05, 0.52],
    },
    {
      color: colors[3] as readonly [number, number, number, number],
      position: [0.85, 1.1],
      size: [0.44, 0.75, 0.44],
    },
  ];
  for (let index = cubes.length; index < 28; index += 1) {
    const width = round2(0.55 + random() * 1.55);
    const height = round2(0.65 + random() * 2.8);
    const depth = round2(0.55 + random() * 1.55);
    let x = round2(-7 + random() * 14);
    let z = round2(-7 + random() * 14);
    if (x > -3.5 && x < 3.5 && z > -3.5 && z < 3.5) {
      z = round2(z < 0 ? z - 3.75 : z + 3.75);
    }
    cubes.push({
      color: colors[index % colors.length] as readonly [number, number, number, number],
      position: [x, z],
      size: [width, height, depth],
    });
  }
  return cubes;
}

function primitiveNode(
  label: string,
  kind: Exclude<Geometry['kind'], 'line' | 'group'>,
  translation: readonly [number, number, number],
  scale: readonly [number, number, number],
  color: readonly [number, number, number, number],
): RenderNode {
  return {
    geometry: { kind },
    material: { color, wireframe: false },
    transform: identityTransform(translation, scale),
    visible: true,
    layer: 'scene',
    metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label },
  };
}

function identityTransform(
  translation: readonly [number, number, number],
  scale: readonly [number, number, number],
): Transform {
  return {
    translation,
    rotation: [0, 0, 0, 1],
    scale,
  };
}

function deterministicUnitGenerator(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state / 0x100000000;
  };
}

function degreesToRadians(degrees: number): number {
  return (degrees * Math.PI) / 180;
}

function round2(value: number): number {
  return Number(value.toFixed(2));
}

// ── Snapshot lines (deterministic golden artifact) ────────────────────────────
