// Backend-neutral retained editor/product viewport.

import type {
  CameraBasis,
  CameraPose,
  EditorGridDescriptor,
  EditorGridProjectionReadout,
  PerspectiveProjection,
  RenderDiff,
  RenderFrameDiff,
  RenderHandle,
  RenderLayer,
} from '@rusty-engine/render-contracts';
import { RenderProjection, type RenderProjectionSnapshot } from '@rusty-engine/render-projection';
import { mountRendererEditorBackend } from '@rusty-engine/renderer-three/backend';
import {
  loadRendererAnimatedMeshSource,
  type RendererAnimatedMeshResourceManifest,
  type RendererAnimatedMeshResourceResolver,
} from './animated-mesh-host.js';

export const RUSTY_RENDERER_EDITOR_VIEWPORT_COMPATIBILITY_VERSION = 'editor-viewport.v1';
export const RUSTY_RENDERER_EDITOR_VIEWPORT_MAX_FRAME_OPS = 4096;
export const RUSTY_RENDERER_EDITOR_VIEWPORT_MAX_RETAINED_OPS = 8192;

export type RendererEditorViewportChannel = 'runtime' | 'authored' | 'overlay';
export type RendererEditorViewportStatus = 'mounted' | 'running' | 'stopped' | 'disposed';
export type RendererEditorViewportCameraSource = 'stored_editor' | 'caller';

export interface RendererEditorViewportChannelPolicy {
  readonly channel: RendererEditorViewportChannel;
  readonly order: number;
  readonly allowedLayers: readonly RenderLayer[];
  readonly depthPolicy: 'shared_scene_depth' | 'overlay_after_depth_clear';
}

export const RUSTY_RENDERER_EDITOR_VIEWPORT_CHANNEL_POLICIES: readonly RendererEditorViewportChannelPolicy[] = [
  {
    channel: 'runtime',
    order: 0,
    allowedLayers: ['scene', 'debug'],
    depthPolicy: 'shared_scene_depth',
  },
  {
    channel: 'authored',
    order: 1,
    allowedLayers: ['scene', 'debug'],
    depthPolicy: 'shared_scene_depth',
  },
  {
    channel: 'overlay',
    order: 2,
    allowedLayers: ['debug'],
    depthPolicy: 'overlay_after_depth_clear',
  },
] as const;

export interface RendererEditorViewportCamera {
  readonly source: RendererEditorViewportCameraSource;
  readonly pose: CameraPose;
  readonly basis: CameraBasis;
  readonly projection: PerspectiveProjection;
}

export interface RendererEditorViewportSize {
  readonly width: number;
  readonly height: number;
  readonly pixelRatio: number;
}

export interface RendererEditorViewportBufferSource {
  readonly borrow: (handle: number) => Uint8Array;
  readonly release: (handle: number) => void;
}

export interface RendererEditorViewportOptions {
  readonly animatedMeshManifest?: RendererAnimatedMeshResourceManifest;
  readonly autoStart?: boolean;
  readonly bufferSource?: RendererEditorViewportBufferSource;
  readonly clearColor?: number;
  readonly initialCamera?: RendererEditorViewportCamera;
  readonly initialGrid?: EditorGridDescriptor | null;
  readonly pixelRatio?: number;
  readonly resolveAnimatedMeshResource?: RendererAnimatedMeshResourceResolver;
}

export type RendererEditorViewportDiagnosticCode =
  | 'backend_rejected'
  | 'channel_disposed'
  | 'frame_limit_exceeded'
  | 'invalid_camera'
  | 'invalid_frame'
  | 'invalid_grid'
  | 'invalid_handle'
  | 'invalid_pick_request'
  | 'invalid_viewport_size'
  | 'overlay_requires_debug_layer'
  | 'viewport_disposed';

export interface RendererEditorViewportDiagnostic {
  readonly channel: RendererEditorViewportChannel | null;
  readonly code: RendererEditorViewportDiagnosticCode;
  readonly message: string;
  readonly recoverable: boolean;
}

export interface RendererEditorViewportChannelSnapshot {
  readonly channel: RendererEditorViewportChannel;
  readonly disposed: boolean;
  readonly generation: number;
  readonly hash: string;
  readonly retainedOpCount: number;
  readonly projection: RenderProjectionSnapshot;
}

export interface RendererEditorViewportChannelReceipt {
  readonly applied: boolean;
  readonly channel: RendererEditorViewportChannel;
  readonly diagnostics: readonly RendererEditorViewportDiagnostic[];
  readonly generation: number;
  readonly snapshotHash: string;
}

export interface RendererEditorViewportChannelHandle {
  readonly channel: RendererEditorViewportChannel;
  readonly apply: (frame: RenderFrameDiff) => RendererEditorViewportChannelReceipt;
  readonly clear: () => RendererEditorViewportChannelReceipt;
  readonly dispose: () => RendererEditorViewportChannelReceipt;
  readonly replace: (frame: RenderFrameDiff) => RendererEditorViewportChannelReceipt;
  /** Atomically replace this channel from individually bounded transport chunks. */
  readonly replaceChunks: (
    chunks: readonly RenderFrameDiff[],
  ) => RendererEditorViewportChannelReceipt;
  readonly snapshot: () => RendererEditorViewportChannelSnapshot;
}

export interface RendererEditorViewportPickFilter {
  readonly channels?: readonly RendererEditorViewportChannel[];
  readonly handles?: readonly RenderHandle[];
  readonly layers?: readonly RenderLayer[];
  readonly tags?: readonly string[];
}

export interface RendererEditorViewportPickRequest {
  /** Canvas-relative pixels from the top-left corner. */
  readonly point: readonly [number, number];
  readonly filter?: RendererEditorViewportPickFilter;
  readonly maxDistance?: number;
}

export interface RendererEditorViewportPickHint {
  readonly channel: RendererEditorViewportChannel;
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

export interface RendererEditorViewportPickReceipt {
  readonly diagnostics: readonly RendererEditorViewportDiagnostic[];
  readonly hint: RendererEditorViewportPickHint | null;
  readonly kind: 'rusty_renderer_editor_viewport_pick.v1';
}

export interface RendererEditorViewportCameraReceipt {
  readonly applied: boolean;
  readonly diagnostics: readonly RendererEditorViewportDiagnostic[];
  readonly hash: string;
}

export interface RendererEditorViewportSizeReceipt {
  readonly applied: boolean;
  readonly diagnostics: readonly RendererEditorViewportDiagnostic[];
  readonly size: RendererEditorViewportSize;
}

export interface RendererEditorViewportReadout {
  readonly kind: 'rusty_renderer_editor_viewport_readout.v1';
  readonly compatibilityVersion: typeof RUSTY_RENDERER_EDITOR_VIEWPORT_COMPATIBILITY_VERSION;
  readonly status: RendererEditorViewportStatus;
  readonly camera: RendererEditorViewportCamera;
  readonly size: RendererEditorViewportSize;
  readonly channels: readonly RendererEditorViewportChannelSnapshot[];
  readonly channelPolicies: readonly RendererEditorViewportChannelPolicy[];
  readonly diagnostics: readonly RendererEditorViewportDiagnostic[];
  readonly grid: EditorGridProjectionReadout | null;
  readonly viewportHash: string;
}

export interface RendererEditorViewport {
  readonly kind: 'rusty_renderer_editor_viewport.v1';
  readonly channels: Readonly<Record<RendererEditorViewportChannel, RendererEditorViewportChannelHandle>>;
  readonly camera: () => RendererEditorViewportCamera;
  readonly dispose: () => void;
  readonly grid: () => EditorGridProjectionReadout | null;
  readonly pick: (request: RendererEditorViewportPickRequest) => RendererEditorViewportPickReceipt;
  readonly readout: () => RendererEditorViewportReadout;
  readonly renderOnce: (timeMs?: number) => void;
  readonly resize: (size: RendererEditorViewportSize) => RendererEditorViewportSizeReceipt;
  readonly setCamera: (camera: RendererEditorViewportCamera) => RendererEditorViewportCameraReceipt;
  readonly setGrid: (descriptor: EditorGridDescriptor | null) => RendererEditorViewportGridReceipt;
  readonly start: () => void;
  readonly stop: () => void;
}

export interface RendererEditorViewportGridReceipt {
  readonly applied: boolean;
  readonly diagnostics: readonly RendererEditorViewportDiagnostic[];
  readonly grid: EditorGridProjectionReadout | null;
  readonly hash: string;
}

interface ChannelState {
  readonly channel: RendererEditorViewportChannel;
  disposed: boolean;
  generation: number;
  history: readonly RenderDiff[];
  projection: RenderProjection;
}

export interface RendererEditorViewportBackendPort {
  readonly dispose: () => void;
  readonly gridReadout: () => EditorGridProjectionReadout | null;
  readonly pick: (request: BackendPickRequest) => BackendPickReceipt;
  readonly renderOnce: (timeMs?: number) => void;
  readonly replaceChannel: (channel: RendererEditorViewportChannel, frame: RenderFrameDiff) => void;
  readonly resize: (size: RendererEditorViewportSize) => void;
  readonly setCamera: (camera: Omit<RendererEditorViewportCamera, 'source'>) => void;
  readonly setGrid: (descriptor: EditorGridDescriptor | null) => void;
  readonly snapshot: () => string;
  readonly start: () => void;
  readonly stop: () => void;
}

interface BackendPickRequest {
  readonly filter?: {
    readonly channels?: readonly RendererEditorViewportChannel[];
    readonly handles?: readonly RenderHandle[];
    readonly layers?: readonly RenderLayer[];
    readonly tags?: readonly string[];
  };
  readonly maxDistance?: number;
  readonly point: readonly [number, number];
}

interface BackendPickReceipt {
  readonly diagnostics: readonly { readonly code: string; readonly message: string }[];
  readonly hit: {
    readonly channel: RendererEditorViewportChannel;
    readonly distance: number;
    readonly handle: RenderHandle;
    readonly label: string | null;
    readonly layer: RenderLayer;
    readonly normal: readonly [number, number, number];
    readonly position: readonly [number, number, number];
    readonly sourceTrace: RendererEditorViewportPickHint['sourceTrace'];
    readonly tags: readonly string[];
  } | null;
}

const CHANNELS: readonly RendererEditorViewportChannel[] = ['runtime', 'authored', 'overlay'];
const MAX_PICK_FILTER_VALUES = 128;
const MAX_VIEWPORT_DIMENSION = 16_384;
const MAX_DIAGNOSTICS = 64;

function requireAnimatedMeshResolver(
  resolver: RendererAnimatedMeshResourceResolver | undefined,
): RendererAnimatedMeshResourceResolver {
  if (resolver === undefined) {
    throw new Error('animatedMeshManifest requires an explicit resource resolver');
  }
  return resolver;
}

export async function mountRendererEditorViewport(
  canvas: HTMLCanvasElement,
  options: RendererEditorViewportOptions = {},
): Promise<RendererEditorViewport> {
  const animatedMeshSource = options.animatedMeshManifest === undefined
    ? undefined
    : await loadRendererAnimatedMeshSource(
        options.animatedMeshManifest,
        requireAnimatedMeshResolver(options.resolveAnimatedMeshResource),
      );
  const bufferSource = options.bufferSource;
  const meshBufferSource = bufferSource === undefined
    ? undefined
    : {
        acquireBuffer: (handle: number) => ({ bytes: bufferSource.borrow(handle) }),
        releaseBuffer: (handle: number) => bufferSource.release(handle),
      };
  const backend = mountRendererEditorBackend(canvas, {
    ...(animatedMeshSource === undefined ? {} : { animatedMeshSource }),
    ...(meshBufferSource === undefined ? {} : { meshBufferSource }),
    ...(options.clearColor === undefined ? {} : { clearColor: options.clearColor }),
    ...(options.pixelRatio === undefined ? {} : { pixelRatio: options.pixelRatio }),
  });
  const size = {
    width: Math.max(1, canvas.clientWidth || canvas.width || 800),
    height: Math.max(1, canvas.clientHeight || canvas.height || 450),
    pixelRatio: options.pixelRatio ?? globalThis.devicePixelRatio ?? 1,
  };
  return createRendererEditorViewportWithBackend(backend, {
    ...(options.autoStart === undefined ? {} : { autoStart: options.autoStart }),
    ...(options.initialCamera === undefined ? {} : { initialCamera: options.initialCamera }),
    ...(options.initialGrid === undefined ? {} : { initialGrid: options.initialGrid }),
    size,
  });
}

/** Internal conformance seam; not exported from the package root. */
export function createRendererEditorViewportWithBackend(
  backend: RendererEditorViewportBackendPort,
  options: {
    readonly autoStart?: boolean;
    readonly initialCamera?: RendererEditorViewportCamera;
    readonly initialGrid?: EditorGridDescriptor | null;
    readonly size?: RendererEditorViewportSize;
  } = {},
): RendererEditorViewport {
  const states = new Map<RendererEditorViewportChannel, ChannelState>();
  for (const channel of CHANNELS) {
    states.set(channel, {
      channel,
      disposed: false,
      generation: 0,
      history: [],
      projection: new RenderProjection(),
    });
  }
  const diagnostics: RendererEditorViewportDiagnostic[] = [];
  let status: RendererEditorViewportStatus = 'mounted';
  const requestedCamera = options.initialCamera ?? defaultEditorCamera();
  const cameraIssue = validateCamera(requestedCamera);
  let camera = cameraIssue === null ? requestedCamera : defaultEditorCamera();
  if (cameraIssue !== null) {
    rememberDiagnostic(diagnostics, cameraIssue);
  }
  const requestedSize = options.size ?? { width: 800, height: 450, pixelRatio: 1 };
  const sizeIssue = validateSize(requestedSize);
  let size = sizeIssue === null ? requestedSize : { width: 800, height: 450, pixelRatio: 1 };
  if (sizeIssue !== null) {
    rememberDiagnostic(diagnostics, sizeIssue);
  }
  const requestedGrid = options.initialGrid ?? null;
  const gridIssue = validateGrid(requestedGrid);
  const initialGridDescriptor = gridIssue === null ? cloneGrid(requestedGrid) : null;
  if (gridIssue !== null) {
    rememberDiagnostic(diagnostics, gridIssue);
  }

  backend.resize(size);
  backend.setCamera(camera);
  backend.setGrid(initialGridDescriptor);

  const channelHandles = Object.fromEntries(CHANNELS.map((channel) => [
    channel,
    createChannelHandle(channel, () => status, states, backend, diagnostics),
  ])) as Record<RendererEditorViewportChannel, RendererEditorViewportChannelHandle>;

  const readout = (): RendererEditorViewportReadout => {
    const channelSnapshots = CHANNELS.map((channel) => snapshotChannel(requireState(states, channel)));
    const grid = backend.gridReadout();
    const viewportHash = stableHash({
      camera,
      channels: channelSnapshots.map(({ channel, disposed, generation, hash }) => ({
        channel,
        disposed,
        generation,
        hash,
      })),
      size,
      status,
      grid,
    });
    return {
      kind: 'rusty_renderer_editor_viewport_readout.v1',
      compatibilityVersion: RUSTY_RENDERER_EDITOR_VIEWPORT_COMPATIBILITY_VERSION,
      status,
      camera,
      size,
      channels: channelSnapshots,
      channelPolicies: RUSTY_RENDERER_EDITOR_VIEWPORT_CHANNEL_POLICIES,
      diagnostics: [...diagnostics],
      grid,
      viewportHash,
    };
  };

  const viewport: RendererEditorViewport = {
    kind: 'rusty_renderer_editor_viewport.v1',
    channels: channelHandles,
    camera: () => camera,
    grid: () => backend.gridReadout(),
    setCamera: (next) => {
      const issue = validateCamera(next);
      if (issue !== null || status === 'disposed') {
        const diagnostic = issue ?? viewportDisposedDiagnostic(null);
        rememberDiagnostic(diagnostics, diagnostic);
        return { applied: false, diagnostics: [diagnostic], hash: stableHash(camera) };
      }
      try {
        backend.setCamera(next);
        camera = next;
        return { applied: true, diagnostics: [], hash: stableHash(camera) };
      } catch (error) {
        const diagnostic = backendDiagnostic(null, error);
        rememberDiagnostic(diagnostics, diagnostic);
        return { applied: false, diagnostics: [diagnostic], hash: stableHash(camera) };
      }
    },
    resize: (next) => {
      const issue = validateSize(next);
      if (issue !== null || status === 'disposed') {
        const diagnostic = issue ?? viewportDisposedDiagnostic(null);
        rememberDiagnostic(diagnostics, diagnostic);
        return { applied: false, diagnostics: [diagnostic], size };
      }
      try {
        backend.resize(next);
        size = next;
        return { applied: true, diagnostics: [], size };
      } catch (error) {
        const diagnostic = backendDiagnostic(null, error);
        rememberDiagnostic(diagnostics, diagnostic);
        return { applied: false, diagnostics: [diagnostic], size };
      }
    },
    setGrid: (next) => {
      const issue = validateGrid(next);
      if (issue !== null || status === 'disposed') {
        const diagnostic = issue ?? viewportDisposedDiagnostic(null);
        rememberDiagnostic(diagnostics, diagnostic);
        const grid = backend.gridReadout();
        return { applied: false, diagnostics: [diagnostic], grid, hash: stableHash(grid) };
      }
      try {
        backend.setGrid(next);
        const grid = backend.gridReadout();
        return { applied: true, diagnostics: [], grid, hash: stableHash(grid) };
      } catch (error) {
        const diagnostic = backendDiagnostic(null, error);
        rememberDiagnostic(diagnostics, diagnostic);
        const grid = backend.gridReadout();
        return { applied: false, diagnostics: [diagnostic], grid, hash: stableHash(grid) };
      }
    },
    pick: (request) => pickViewport(status, size, states, backend, diagnostics, request),
    readout,
    renderOnce: (timeMs) => {
      if (status !== 'disposed') {
        backend.renderOnce(timeMs);
      }
    },
    start: () => {
      if (status !== 'disposed' && status !== 'running') {
        backend.start();
        status = 'running';
      }
    },
    stop: () => {
      if (status !== 'disposed' && status !== 'stopped') {
        backend.stop();
        status = 'stopped';
      }
    },
    dispose: () => {
      if (status === 'disposed') {
        return;
      }
      backend.stop();
      backend.dispose();
      status = 'disposed';
      for (const state of states.values()) {
        state.disposed = true;
      }
    },
  };

  if (options.autoStart !== false) {
    viewport.start();
  }
  return viewport;
}

function createChannelHandle(
  channel: RendererEditorViewportChannel,
  viewportStatus: () => RendererEditorViewportStatus,
  states: Map<RendererEditorViewportChannel, ChannelState>,
  backend: RendererEditorViewportBackendPort,
  diagnostics: RendererEditorViewportDiagnostic[],
): RendererEditorViewportChannelHandle {
  const commit = (
    mode: 'apply' | 'replace',
    chunks: unknown,
  ): RendererEditorViewportChannelReceipt => {
    const state = requireState(states, channel);
    if (viewportStatus() === 'disposed') {
      return rejectedChannelReceipt(state, diagnostics, viewportDisposedDiagnostic(channel));
    }
    if (state.disposed) {
      return rejectedChannelReceipt(state, diagnostics, {
        channel,
        code: 'channel_disposed',
        message: `renderer viewport channel ${channel} is disposed`,
        recoverable: false,
      });
    }
    if (!isUnknownArray(chunks)) {
      return rejectedChannelReceipt(
        state,
        diagnostics,
        invalidFrameDiagnostic(channel, 'renderer viewport frame chunks must be an array'),
      );
    }
    const validChunks: RenderFrameDiff[] = [];
    for (const [chunkIndex, frame] of chunks.entries()) {
      if (!hasRenderFrameOps(frame)) {
        return rejectedChannelReceipt(
          state,
          diagnostics,
          invalidFrameDiagnostic(
            channel,
            chunks.length === 1
              ? 'render frame ops must be an array'
              : `renderer viewport frame chunk ${chunkIndex} ops must be an array`,
          ),
        );
      }
      validChunks.push(frame);
    }
    const suppliedOps = validChunks.flatMap((frame) => frame.ops);
    const nextHistory = mode === 'apply' ? [...state.history, ...suppliedOps] : suppliedOps;
    const validation = validateChannelHistory(channel, validChunks, nextHistory);
    if ('diagnostic' in validation) {
      return rejectedChannelReceipt(state, diagnostics, validation.diagnostic);
    }
    try {
      backend.replaceChannel(channel, {
        schemaVersion: 1,
        ops: nextHistory,
      });
    } catch (error) {
      return rejectedChannelReceipt(state, diagnostics, backendDiagnostic(channel, error));
    }
    state.history = nextHistory;
    state.projection = validation.projection;
    state.generation += 1;
    return acceptedChannelReceipt(state);
  };

  return {
    channel,
    apply: (frame) => commit('apply', [frame]),
    replace: (frame) => commit('replace', [frame]),
    replaceChunks: (chunks) => commit('replace', chunks),
    clear: () => commit('replace', []),
    snapshot: () => snapshotChannel(requireState(states, channel)),
    dispose: () => {
      const state = requireState(states, channel);
      if (viewportStatus() === 'disposed' || state.disposed) {
        const diagnostic = viewportStatus() === 'disposed'
          ? viewportDisposedDiagnostic(channel)
          : {
              channel,
              code: 'channel_disposed' as const,
              message: `renderer viewport channel ${channel} is disposed`,
              recoverable: false,
            };
        return rejectedChannelReceipt(state, diagnostics, diagnostic);
      }
      const receipt = commit('replace', []);
      if (receipt.applied) {
        state.disposed = true;
      }
      return receipt;
    },
  };
}

function validateChannelHistory(
  channel: RendererEditorViewportChannel,
  chunks: readonly RenderFrameDiff[],
  history: readonly RenderDiff[],
): { readonly projection: RenderProjection } | { readonly diagnostic: RendererEditorViewportDiagnostic } {
  try {
    const overLimitChunkIndex = chunks.findIndex(
      (frame) => frame.ops.length > RUSTY_RENDERER_EDITOR_VIEWPORT_MAX_FRAME_OPS,
    );
    if (overLimitChunkIndex !== -1 || history.length > RUSTY_RENDERER_EDITOR_VIEWPORT_MAX_RETAINED_OPS) {
      return {
        diagnostic: {
          channel,
          code: 'frame_limit_exceeded',
          message: overLimitChunkIndex !== -1
            ? `renderer viewport frame chunk ${overLimitChunkIndex} exceeds the ${RUSTY_RENDERER_EDITOR_VIEWPORT_MAX_FRAME_OPS} op limit`
            : `renderer viewport replacement exceeds the ${RUSTY_RENDERER_EDITOR_VIEWPORT_MAX_RETAINED_OPS} retained op limit`,
          recoverable: true,
        },
      };
    }
    for (const op of chunks.flatMap((frame) => frame.ops)) {
      if (!isRenderDiffCandidate(op)) {
        return {
          diagnostic: invalidFrameDiagnostic(
            channel,
            'renderer viewport frame operations must be non-array objects',
          ),
        };
      }
      const handleIssue = validateDiffHandles(channel, op);
      if (handleIssue !== null) {
        return { diagnostic: handleIssue };
      }
      if (channel === 'overlay' && createdLayer(op) !== null && createdLayer(op) !== 'debug') {
        return {
          diagnostic: {
            channel,
            code: 'overlay_requires_debug_layer',
            message: 'overlay channel creates must use the debug render layer',
            recoverable: true,
          },
        };
      }
    }
    const projection = new RenderProjection();
    projection.applyFrame({ schemaVersion: 1, ops: history });
    return { projection };
  } catch (error) {
    return { diagnostic: invalidFrameDiagnostic(channel, error instanceof Error ? error.message : String(error)) };
  }
}

function validateDiffHandles(
  channel: RendererEditorViewportChannel,
  op: RenderDiff,
): RendererEditorViewportDiagnostic | null {
  const values: number[] = [];
  if ('handle' in op) {
    values.push(op.handle);
  }
  if ('parent' in op && op.parent !== null) {
    values.push(op.parent);
  }
  if (values.every((value) => Number.isSafeInteger(value) && value >= 0)) {
    return null;
  }
  return {
    channel,
    code: 'invalid_handle',
    message: `render handles must be canonical integers from 0 through ${Number.MAX_SAFE_INTEGER}`,
    recoverable: true,
  };
}

function createdLayer(op: RenderDiff): RenderLayer | null {
  if (op.op === 'create') {
    return op.node.layer;
  }
  if (op.op === 'createStaticMeshInstance'
    || op.op === 'createAnimatedMeshInstance'
    || op.op === 'createSprite'
    || op.op === 'createLight') {
    return 'scene';
  }
  return null;
}

function pickViewport(
  status: RendererEditorViewportStatus,
  size: RendererEditorViewportSize,
  states: ReadonlyMap<RendererEditorViewportChannel, ChannelState>,
  backend: RendererEditorViewportBackendPort,
  diagnostics: RendererEditorViewportDiagnostic[],
  request: RendererEditorViewportPickRequest,
): RendererEditorViewportPickReceipt {
  const issue = validatePickRequest(status, size, request);
  if (issue !== null) {
    rememberDiagnostic(diagnostics, issue);
    return { diagnostics: [issue], hint: null, kind: 'rusty_renderer_editor_viewport_pick.v1' };
  }
  const channels = request.filter?.channels ?? CHANNELS;
  const backendHandles = request.filter?.handles;
  const normalizedX = (request.point[0] / size.width) * 2 - 1;
  const normalizedY = -((request.point[1] / size.height) * 2 - 1);
  const point: readonly [number, number] = [
    normalizedX === 0 ? 0 : normalizedX,
    normalizedY === 0 ? 0 : normalizedY,
  ];
  try {
    const receipt = backend.pick({
      point,
      ...(request.maxDistance === undefined ? {} : { maxDistance: request.maxDistance }),
      filter: {
        channels,
        ...(backendHandles === undefined ? {} : { handles: backendHandles }),
        ...(request.filter?.layers === undefined ? {} : { layers: request.filter.layers }),
        ...(request.filter?.tags === undefined ? {} : { tags: request.filter.tags }),
      },
    });
    if (receipt.diagnostics.length > 0) {
      const projected = receipt.diagnostics.map((entry) => ({
        channel: null,
        code: 'backend_rejected' as const,
        message: `${entry.code}: ${entry.message}`,
        recoverable: true,
      }));
      projected.forEach((entry) => rememberDiagnostic(diagnostics, entry));
      return { diagnostics: projected, hint: null, kind: 'rusty_renderer_editor_viewport_pick.v1' };
    }
    if (receipt.hit === null) {
      return { diagnostics: [], hint: null, kind: 'rusty_renderer_editor_viewport_pick.v1' };
    }
    const hit = receipt.hit;
    const state = states.get(hit.channel);
    const retained = state?.projection.snapshot().nodes.some(
      (node) => node.handle === hit.handle,
    ) ?? false;
    if (state === undefined || state.disposed || !retained) {
      const diagnostic = backendDiagnostic(
        hit.channel,
        'backend returned an unrecognized handle for its channel',
      );
      rememberDiagnostic(diagnostics, diagnostic);
      return { diagnostics: [diagnostic], hint: null, kind: 'rusty_renderer_editor_viewport_pick.v1' };
    }
    return {
      diagnostics: [],
      hint: hit,
      kind: 'rusty_renderer_editor_viewport_pick.v1',
    };
  } catch (error) {
    const diagnostic = backendDiagnostic(null, error);
    rememberDiagnostic(diagnostics, diagnostic);
    return { diagnostics: [diagnostic], hint: null, kind: 'rusty_renderer_editor_viewport_pick.v1' };
  }
}

function validatePickRequest(
  status: RendererEditorViewportStatus,
  size: RendererEditorViewportSize,
  request: RendererEditorViewportPickRequest,
): RendererEditorViewportDiagnostic | null {
  if (status === 'disposed') {
    return viewportDisposedDiagnostic(null);
  }
  const [x, y] = request.point;
  const counts = [
    request.filter?.channels?.length ?? 0,
    request.filter?.handles?.length ?? 0,
    request.filter?.layers?.length ?? 0,
    request.filter?.tags?.length ?? 0,
  ];
  const invalidPoint = !Number.isFinite(x) || !Number.isFinite(y)
    || x < 0 || x > size.width || y < 0 || y > size.height;
  const invalidDistance = request.maxDistance !== undefined
    && (!Number.isFinite(request.maxDistance) || request.maxDistance <= 0);
  const invalidChannel = request.filter?.channels?.some((channel) => !CHANNELS.includes(channel)) ?? false;
  const invalidHandle = request.filter?.handles?.some((handle) =>
    !Number.isSafeInteger(handle) || handle < 0,
  ) ?? false;
  if (invalidPoint || invalidDistance || invalidChannel || invalidHandle
    || counts.some((count) => count > MAX_PICK_FILTER_VALUES)) {
    return {
      channel: null,
      code: 'invalid_pick_request',
      message: 'pick point, distance, channel, handle, and filter bounds must be valid for the current viewport',
      recoverable: true,
    };
  }
  return null;
}

function validateCamera(
  camera: RendererEditorViewportCamera,
): RendererEditorViewportDiagnostic | null {
  const sourceValid = camera.source === 'stored_editor' || camera.source === 'caller';
  const vectors = [camera.pose.position, camera.basis.forward, camera.basis.right, camera.basis.up];
  const finite = vectors.every((vector) => vector.every(Number.isFinite))
    && Number.isFinite(camera.pose.yawDegrees)
    && Number.isFinite(camera.pose.pitchDegrees)
    && Number.isFinite(camera.projection.fovYDegrees)
    && Number.isFinite(camera.projection.near)
    && Number.isFinite(camera.projection.far);
  const basisValid = [camera.basis.forward, camera.basis.right, camera.basis.up]
    .every((vector) => Math.abs(Math.hypot(...vector) - 1) <= 0.01)
    && Math.abs(dot(camera.basis.forward, camera.basis.right)) <= 0.01
    && Math.abs(dot(camera.basis.forward, camera.basis.up)) <= 0.01
    && Math.abs(dot(camera.basis.right, camera.basis.up)) <= 0.01;
  if (!sourceValid || !finite || !basisValid || camera.projection.fovYDegrees <= 0
    || camera.projection.fovYDegrees >= 180 || camera.projection.near <= 0
    || camera.projection.far <= camera.projection.near) {
    return {
      channel: null,
      code: 'invalid_camera',
      message: 'editor viewport camera requires finite pose, orthonormal basis, and valid perspective bounds',
      recoverable: true,
    };
  }
  return null;
}

function validateSize(
  size: RendererEditorViewportSize,
): RendererEditorViewportDiagnostic | null {
  if (!Number.isSafeInteger(size.width) || !Number.isSafeInteger(size.height)
    || size.width <= 0 || size.height <= 0
    || size.width > MAX_VIEWPORT_DIMENSION || size.height > MAX_VIEWPORT_DIMENSION
    || !Number.isFinite(size.pixelRatio) || size.pixelRatio <= 0 || size.pixelRatio > 4) {
    return {
      channel: null,
      code: 'invalid_viewport_size',
      message: `viewport width and height must be integers from 1 through ${MAX_VIEWPORT_DIMENSION}; pixelRatio must be in (0, 4]`,
      recoverable: true,
    };
  }
  return null;
}

function validateGrid(
  descriptor: EditorGridDescriptor | null,
): RendererEditorViewportDiagnostic | null {
  if (descriptor === null) return null;
  const finiteTuple = (values: readonly number[]): boolean => values.every(Number.isFinite);
  const normalizedColor = (values: readonly number[]): boolean =>
    finiteTuple(values) && values.every(value => value >= 0 && value <= 1);
  const coordinateSystemValid = descriptor.grid.coordinateSystem === 'rightHandedYUp';
  const gridValid = finiteTuple(descriptor.grid.origin)
    && finiteTuple(descriptor.grid.spacing)
    && descriptor.grid.spacing.every(value => value > 0);
  const colors = [
    descriptor.style.minorColor,
    descriptor.style.majorColor,
    descriptor.style.xAxisColor,
    descriptor.style.yAxisColor,
    descriptor.style.zAxisColor,
  ];
  const styleValid = colors.every(normalizedColor)
    && Number.isSafeInteger(descriptor.style.majorLineEvery)
    && descriptor.style.majorLineEvery > 0
    && Number.isFinite(descriptor.style.opacity)
    && descriptor.style.opacity >= 0
    && descriptor.style.opacity <= 1
    && Number.isFinite(descriptor.style.fadeStart)
    && Number.isFinite(descriptor.style.fadeEnd)
    && descriptor.style.fadeStart >= 0
    && descriptor.style.fadeEnd > descriptor.style.fadeStart;
  if (!coordinateSystemValid || !gridValid || !styleValid) {
    return {
      channel: null,
      code: 'invalid_grid',
      message: 'editor grid requires right-handed Y-up coordinates, finite positive spacing, normalized colors/opacity, and an increasing fade range',
      recoverable: true,
    };
  }
  return null;
}

function cloneGrid(descriptor: EditorGridDescriptor | null): EditorGridDescriptor | null {
  return descriptor === null ? null : structuredClone(descriptor);
}

function defaultEditorCamera(): RendererEditorViewportCamera {
  return {
    source: 'stored_editor',
    pose: { position: [4, 4, 8], yawDegrees: 0, pitchDegrees: -20 },
    basis: {
      forward: [-0.408248, -0.408248, -0.816497],
      right: [0.894427, 0, -0.447214],
      up: [-0.182574, 0.912871, -0.365148],
    },
    projection: { fovYDegrees: 55, near: 0.05, far: 1000 },
  };
}

function snapshotChannel(state: ChannelState): RendererEditorViewportChannelSnapshot {
  const projection = state.projection.snapshot();
  return {
    channel: state.channel,
    disposed: state.disposed,
    generation: state.generation,
    hash: stableHash({ channel: state.channel, history: state.history, projection }),
    retainedOpCount: state.history.length,
    projection,
  };
}

function acceptedChannelReceipt(state: ChannelState): RendererEditorViewportChannelReceipt {
  const snapshot = snapshotChannel(state);
  return {
    applied: true,
    channel: state.channel,
    diagnostics: [],
    generation: state.generation,
    snapshotHash: snapshot.hash,
  };
}

function rejectedChannelReceipt(
  state: ChannelState,
  diagnostics: RendererEditorViewportDiagnostic[],
  diagnostic: RendererEditorViewportDiagnostic,
): RendererEditorViewportChannelReceipt {
  rememberDiagnostic(diagnostics, diagnostic);
  return {
    applied: false,
    channel: state.channel,
    diagnostics: [diagnostic],
    generation: state.generation,
    snapshotHash: snapshotChannel(state).hash,
  };
}

function invalidFrameDiagnostic(
  channel: RendererEditorViewportChannel,
  message: string,
): RendererEditorViewportDiagnostic {
  return { channel, code: 'invalid_frame', message, recoverable: true };
}

function backendDiagnostic(
  channel: RendererEditorViewportChannel | null,
  error: unknown,
): RendererEditorViewportDiagnostic {
  return {
    channel,
    code: 'backend_rejected',
    message: error instanceof Error ? error.message : String(error),
    recoverable: true,
  };
}

function viewportDisposedDiagnostic(
  channel: RendererEditorViewportChannel | null,
): RendererEditorViewportDiagnostic {
  return {
    channel,
    code: 'viewport_disposed',
    message: 'renderer editor viewport is disposed',
    recoverable: false,
  };
}

function requireState(
  states: ReadonlyMap<RendererEditorViewportChannel, ChannelState>,
  channel: RendererEditorViewportChannel,
): ChannelState {
  const state = states.get(channel);
  if (state === undefined) {
    throw new Error(`renderer editor viewport channel ${channel} is unavailable`);
  }
  return state;
}

function rememberDiagnostic(
  diagnostics: RendererEditorViewportDiagnostic[],
  diagnostic: RendererEditorViewportDiagnostic,
): void {
  diagnostics.push(diagnostic);
  if (diagnostics.length > MAX_DIAGNOSTICS) {
    diagnostics.splice(0, diagnostics.length - MAX_DIAGNOSTICS);
  }
}

function dot(
  left: readonly [number, number, number],
  right: readonly [number, number, number],
): number {
  return left[0] * right[0] + left[1] * right[1] + left[2] * right[2];
}

type StableValue = string | number | boolean | null | readonly StableValue[] | { readonly [key: string]: StableValue | undefined };

function stableHash(value: unknown): string {
  return `fnv1a64:${fnv1a64(stableStringify(value as StableValue))}`;
}

function stableStringify(value: StableValue | undefined): string {
  if (value === undefined) return 'undefined';
  if (value === null || typeof value !== 'object') return JSON.stringify(value);
  if (isStableValueArray(value)) return `[${value.map((entry) => stableStringify(entry)).join(',')}]`;
  const record = value as { readonly [key: string]: StableValue | undefined };
  return `{${Object.keys(record).sort().map((key) =>
    `${JSON.stringify(key)}:${stableStringify(record[key])}`,
  ).join(',')}}`;
}

function hasRenderFrameOps(value: unknown): value is RenderFrameDiff {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const frame = value as { readonly schemaVersion?: unknown; readonly ops?: unknown };
  return frame.schemaVersion === 1 && Array.isArray(frame.ops);
}

function isRenderDiffCandidate(value: unknown): value is RenderDiff {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isUnknownArray(value: unknown): value is readonly unknown[] {
  return Array.isArray(value);
}

function isStableValueArray(value: StableValue): value is readonly StableValue[] {
  return Array.isArray(value);
}

function fnv1a64(value: string): string {
  let hash = 0xcbf29ce484222325n;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= BigInt(value.charCodeAt(index));
    hash = BigInt.asUintN(64, hash * 0x100000001b3n);
  }
  return hash.toString(16).padStart(16, '0');
}
