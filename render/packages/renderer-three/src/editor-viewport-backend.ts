// Engine-owned Three.js realization for the backend-neutral editor viewport.

import * as THREE from 'three';
import type {
  CameraBasis,
  CameraPose,
  EditorGridDescriptor,
  EditorGridProjectionReadout,
  PerspectiveProjection,
  RenderFrameDiff,
  RenderHandle,
  RenderLayer,
} from '@rusty-engine/render-contracts';
import type { AnimatedMeshAssetSource } from './animated-mesh.js';
import {
  pickProjectedObject,
  type RendererBrowserSurfacePickFilter,
} from './browser-surface.js';
import {
  ThreeRenderer,
  type MeshBufferSource,
  type MeshResourceSource,
  type ThreeRendererResourceStatistics,
} from './three-renderer.js';
import { ThreeEditorGridProjection } from './editor-grid.js';
import { renderEditorViewportFrame } from './editor-viewport-render-pass.js';

export type RendererEditorBackendChannel = 'runtime' | 'authored' | 'overlay';

export interface RendererEditorBackendCamera {
  readonly basis: CameraBasis;
  readonly pose: CameraPose;
  readonly projection: PerspectiveProjection;
}

export interface RendererEditorBackendSize {
  readonly height: number;
  readonly pixelRatio: number;
  readonly width: number;
}

export interface RendererEditorBackendPickFilter {
  readonly channels?: readonly RendererEditorBackendChannel[];
  readonly handles?: readonly RenderHandle[];
  readonly layers?: readonly RenderLayer[];
  readonly tags?: readonly string[];
}

export interface RendererEditorBackendPickRequest {
  readonly filter?: RendererEditorBackendPickFilter;
  readonly maxDistance?: number;
  readonly point: readonly [number, number];
}

export interface RendererEditorBackendPickHit {
  readonly channel: RendererEditorBackendChannel;
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

export interface RendererEditorBackendPickReceipt {
  readonly diagnostics: readonly { readonly code: string; readonly message: string }[];
  readonly hit: RendererEditorBackendPickHit | null;
}

export interface RendererEditorBackendOptions {
  readonly animatedMeshSource?: AnimatedMeshAssetSource;
  readonly clearColor?: number;
  readonly meshBufferSource?: MeshBufferSource;
  readonly meshResourceSource?: MeshResourceSource;
  readonly pixelRatio?: number;
}

/** Exact Three submission facts consumed by the renderer-neutral editor host. */
export interface RendererEditorBackendSubmissionStatistics
  extends ThreeRendererResourceStatistics {
  readonly schemaVersion: 1;
  readonly drawCallCount: number;
  readonly triangleCount: number;
}

export interface RendererEditorBackend {
  readonly dispose: () => void;
  readonly gridReadout: () => EditorGridProjectionReadout | null;
  readonly pick: (request: RendererEditorBackendPickRequest) => RendererEditorBackendPickReceipt;
  readonly renderOnce: (timeMs?: number) => RendererEditorBackendSubmissionStatistics;
  readonly replaceChannel: (channel: RendererEditorBackendChannel, frame: RenderFrameDiff) => void;
  readonly resize: (size: RendererEditorBackendSize) => void;
  readonly setCamera: (camera: RendererEditorBackendCamera) => void;
  readonly setGrid: (descriptor: EditorGridDescriptor | null) => void;
  readonly snapshot: () => string;
  readonly start: () => void;
  readonly stop: () => void;
}

const CHANNEL_ORDER: readonly RendererEditorBackendChannel[] = [
  'runtime',
  'authored',
  'overlay',
];

/** Engine-internal retained channel set used by the mounted WebGL backend. */
export class RendererEditorProjectionChannels {
  readonly #options: RendererEditorBackendOptions;
  readonly #renderers = new Map<RendererEditorBackendChannel, ThreeRenderer>();

  constructor(options: RendererEditorBackendOptions = {}) {
    this.#options = options;
    for (const channel of CHANNEL_ORDER) {
      this.#renderers.set(channel, createChannelRenderer(options));
    }
  }

  renderer(channel: RendererEditorBackendChannel): ThreeRenderer {
    return requireChannelRenderer(this.#renderers, channel);
  }

  replace(channel: RendererEditorBackendChannel, frame: RenderFrameDiff): void {
    const candidate = createChannelRenderer(this.#options);
    try {
      candidate.applyFrame(frame);
    } catch (error) {
      candidate.dispose();
      throw error;
    }
    const previous = this.renderer(channel);
    this.#renderers.set(channel, candidate);
    previous.dispose();
  }

  snapshot(): string {
    return CHANNEL_ORDER.map((channel) =>
      `[${channel}]\n${this.renderer(channel).snapshot()}`,
    ).join('\n');
  }

  resourceStatistics(): ThreeRendererResourceStatistics {
    const statistics = CHANNEL_ORDER.map((channel) =>
      this.renderer(channel).resourceStatistics(),
    );
    return Object.freeze({
      renderHandleCount: sumStatistics(statistics, 'renderHandleCount'),
      geometryResourceCount: sumStatistics(statistics, 'geometryResourceCount'),
      materialResourceCount: sumStatistics(statistics, 'materialResourceCount'),
      textureResourceCount: sumStatistics(statistics, 'textureResourceCount'),
      animatedInstanceCount: sumStatistics(statistics, 'animatedInstanceCount'),
    });
  }

  dispose(): void {
    for (const renderer of this.#renderers.values()) {
      renderer.dispose();
    }
    this.#renderers.clear();
  }
}

export function mountRendererEditorBackend(
  canvas: HTMLCanvasElement,
  options: RendererEditorBackendOptions = {},
): RendererEditorBackend {
  const channels = new RendererEditorProjectionChannels(options);
  const gridProjection = new ThreeEditorGridProjection();

  const webgl = new THREE.WebGLRenderer({ canvas, antialias: true });
  webgl.autoClear = false;
  webgl.setClearColor(options.clearColor ?? 0x101820, 1);
  const camera = new THREE.PerspectiveCamera(55, 1, 0.1, 1000);
  const raycaster = new THREE.Raycaster();
  const pickPoint = new THREE.Vector2();
  const lookTarget = new THREE.Vector3();
  let size: RendererEditorBackendSize = {
    width: Math.max(1, canvas.clientWidth || canvas.width || 800),
    height: Math.max(1, canvas.clientHeight || canvas.height || 450),
    pixelRatio: options.pixelRatio ?? globalThis.devicePixelRatio ?? 1,
  };
  let animationFrame: number | null = null;
  let lastRenderTimeMs: number | null = null;
  let disposed = false;

  const resize = (next: RendererEditorBackendSize): void => {
    requireActive(disposed);
    size = next;
    webgl.setPixelRatio(next.pixelRatio);
    webgl.setSize(next.width, next.height, false);
    camera.aspect = next.width / next.height;
    camera.updateProjectionMatrix();
    gridProjection.resize(next);
  };

  const setCamera = (next: RendererEditorBackendCamera): void => {
    requireActive(disposed);
    camera.position.set(...next.pose.position);
    camera.up.set(...next.basis.up);
    lookTarget.set(
      next.pose.position[0] + next.basis.forward[0],
      next.pose.position[1] + next.basis.forward[1],
      next.pose.position[2] + next.basis.forward[2],
    );
    camera.lookAt(lookTarget);
    camera.fov = next.projection.fovYDegrees;
    camera.near = next.projection.near;
    camera.far = next.projection.far;
    camera.updateProjectionMatrix();
    gridProjection.setCamera(next);
  };

  const renderOnce = (
    timeMs = globalThis.performance?.now() ?? 0,
  ): RendererEditorBackendSubmissionStatistics => {
    requireActive(disposed);
    const deltaSeconds = lastRenderTimeMs === null
      ? 0
      : Math.min(0.05, Math.max(0, (timeMs - lastRenderTimeMs) / 1000));
    lastRenderTimeMs = timeMs;
    webgl.info.reset();
    renderEditorViewportFrame(webgl, camera, gridProjection.scene, channels, deltaSeconds);
    return Object.freeze({
      schemaVersion: 1,
      drawCallCount: webgl.info.render.calls,
      triangleCount: webgl.info.render.triangles,
      ...channels.resourceStatistics(),
    });
  };

  const tick = (timeMs: number): void => {
    renderOnce(timeMs);
    animationFrame = globalThis.requestAnimationFrame(tick);
  };

  const start = (): void => {
    requireActive(disposed);
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

  resize(size);

  return {
    replaceChannel: (channel, frame) => {
      requireActive(disposed);
      channels.replace(channel, frame);
    },
    setCamera,
    setGrid: (descriptor) => {
      requireActive(disposed);
      gridProjection.setDescriptor(descriptor);
    },
    gridReadout: () => gridProjection.readout(),
    resize,
    pick: (request) => pickAcrossChannels(channels, camera, raycaster, pickPoint, request),
    renderOnce,
    start,
    stop,
    snapshot: () => `[grid]\n${gridProjection.snapshot()}\n${channels.snapshot()}`,
    dispose: () => {
      if (disposed) {
        return;
      }
      stop();
      disposed = true;
      gridProjection.dispose();
      channels.dispose();
      webgl.dispose();
    },
  };
}

function sumStatistics(
  statistics: readonly ThreeRendererResourceStatistics[],
  key: keyof ThreeRendererResourceStatistics,
): number {
  let total = 0;
  for (const sample of statistics) {
    total += sample[key];
    if (!Number.isSafeInteger(total)) {
      throw new Error(`editor renderer ${key} exceeds Number.MAX_SAFE_INTEGER`);
    }
  }
  return total;
}

function createChannelRenderer(options: RendererEditorBackendOptions): ThreeRenderer {
  return new ThreeRenderer({
    ...(options.animatedMeshSource === undefined
      ? {}
      : { animatedMeshSource: options.animatedMeshSource }),
    ...(options.meshBufferSource === undefined
      ? {}
      : { meshBufferSource: options.meshBufferSource }),
    ...(options.meshResourceSource === undefined
      ? {}
      : { meshResourceSource: options.meshResourceSource }),
  });
}

function pickAcrossChannels(
  projectionChannels: RendererEditorProjectionChannels,
  camera: THREE.PerspectiveCamera,
  raycaster: THREE.Raycaster,
  point: THREE.Vector2,
  request: RendererEditorBackendPickRequest,
): RendererEditorBackendPickReceipt {
  const requestedChannels = request.filter?.channels ?? CHANNEL_ORDER;
  let selected: RendererEditorBackendPickHit | null = null;
  for (const channel of CHANNEL_ORDER) {
    if (!requestedChannels.includes(channel)) {
      continue;
    }
    const renderer = projectionChannels.renderer(channel);
    const filter: RendererBrowserSurfacePickFilter = {
      ...(request.filter?.handles === undefined ? {} : { handles: request.filter.handles }),
      ...(request.filter?.layers === undefined ? {} : { layers: request.filter.layers }),
      ...(request.filter?.tags === undefined ? {} : { tags: request.filter.tags }),
    };
    const receipt = pickProjectedObject(renderer, camera, raycaster, point, {
      ray: { kind: 'viewport', point: request.point },
      ...(request.maxDistance === undefined ? {} : { maxDistance: request.maxDistance }),
      ...(Object.keys(filter).length === 0 ? {} : { filter }),
    });
    if (receipt.diagnostics.length > 0) {
      return { diagnostics: receipt.diagnostics, hit: null };
    }
    if (receipt.hit !== null && (selected === null || receipt.hit.distance < selected.distance)) {
      selected = { ...receipt.hit, channel };
    }
  }
  return { diagnostics: [], hit: selected };
}

function requireChannelRenderer(
  renderers: ReadonlyMap<RendererEditorBackendChannel, ThreeRenderer>,
  channel: RendererEditorBackendChannel,
): ThreeRenderer {
  const renderer = renderers.get(channel);
  if (renderer === undefined) {
    throw new Error(`editor viewport backend channel ${channel} is unavailable`);
  }
  return renderer;
}

function requireActive(disposed: boolean): void {
  if (disposed) {
    throw new Error('editor viewport backend is disposed');
  }
}
