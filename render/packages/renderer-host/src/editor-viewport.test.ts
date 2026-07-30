import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  renderHandle,
  type CameraBasis,
  type EditorGridDescriptor,
  type EditorGridProjectionReadout,
  type RenderFrameDiff,
} from '@rusty-engine/render-contracts';
import {
  RUSTY_RENDERER_EDITOR_VIEWPORT_CHANNEL_POLICIES,
  createRendererEditorViewportWithBackend,
  type RendererEditorViewportBackendPort,
  type RendererEditorViewportCamera,
} from './editor-viewport.js';

void test('editor viewport isolates equal handles across deterministic runtime authored and overlay channels', () => {
  const backend = new FakeEditorViewportBackend();
  const viewport = createRendererEditorViewportWithBackend(backend, {
    autoStart: false,
    size: { width: 1000, height: 600, pixelRatio: 1 },
  });

  assert.equal(viewport.channels.runtime.replace(primitiveFrame(7, 'runtime', 'scene')).applied, true);
  assert.equal(viewport.channels.authored.replace(primitiveFrame(7, 'authored', 'scene')).applied, true);
  assert.equal(viewport.channels.overlay.replace(primitiveFrame(7, 'selection', 'debug')).applied, true);

  const backendHandles = [
    firstCreatedHandle(backend.frame('runtime')),
    firstCreatedHandle(backend.frame('authored')),
    firstCreatedHandle(backend.frame('overlay')),
  ];
  assert.deepEqual(backendHandles, [7, 7, 7]);
  assert.deepEqual(
    viewport.readout().channels.map((channel) => channel.projection.nodes[0]?.handle),
    [7, 7, 7],
  );

  const rustNamespacedHandle = renderHandle((2 ** 40) + 1);
  assert.equal(
    viewport.channels.authored.replace(
      primitiveFrame(rustNamespacedHandle, 'rust-projection', 'scene'),
    ).applied,
    true,
  );
  assert.equal(firstCreatedHandle(backend.frame('authored')), rustNamespacedHandle);
  assert.deepEqual(
    RUSTY_RENDERER_EDITOR_VIEWPORT_CHANNEL_POLICIES.map(({ channel, order, depthPolicy }) => ({
      channel,
      order,
      depthPolicy,
    })),
    [
      { channel: 'runtime', order: 0, depthPolicy: 'shared_scene_depth' },
      { channel: 'authored', order: 1, depthPolicy: 'shared_scene_depth' },
      { channel: 'overlay', order: 2, depthPolicy: 'overlay_after_depth_clear' },
    ],
  );

  const beforeFailedReplace = viewport.channels.authored.snapshot();
  const rejected = viewport.channels.authored.replace({
    schemaVersion: 1,
    ops: [{
      op: 'update',
      handle: renderHandle(999),
      transform: null,
      material: null,
      visible: false,
      metadata: null,
    }],
  });
  assert.equal(rejected.applied, false);
  assert.equal(rejected.diagnostics[0]?.code, 'invalid_frame');
  assert.deepEqual(viewport.channels.authored.snapshot(), beforeFailedReplace);
  assert.equal(firstCreatedHandle(backend.frame('authored')), rustNamespacedHandle);

  const malformed = viewport.channels.authored.replace(
    { ops: null } as unknown as RenderFrameDiff,
  );
  assert.equal(malformed.applied, false);
  assert.equal(malformed.diagnostics[0]?.code, 'invalid_frame');
  assert.deepEqual(viewport.channels.authored.snapshot(), beforeFailedReplace);

  const backendFrameBeforeMalformedOps = structuredClone(backend.frame('authored'));
  for (const op of [null, 42] as const) {
    const malformedOperation = viewport.channels.authored.replaceChunks([
      { schemaVersion: 1, ops: [op as never] },
    ]);
    assert.equal(malformedOperation.applied, false);
    assert.equal(malformedOperation.diagnostics[0]?.code, 'invalid_frame');
    assert.equal(malformedOperation.generation, beforeFailedReplace.generation);
    assert.deepEqual(viewport.channels.authored.snapshot(), beforeFailedReplace);
    assert.deepEqual(backend.frame('authored'), backendFrameBeforeMalformedOps);
  }

  const updated = viewport.channels.runtime.apply({
    schemaVersion: 1,
    ops: [{
      op: 'update',
      handle: renderHandle(7),
      transform: null,
      material: null,
      visible: false,
      metadata: null,
    }],
  });
  assert.equal(updated.applied, true);
  assert.equal(viewport.channels.runtime.snapshot().projection.nodes[0]?.visible, false);

  const overlaySceneCreate = viewport.channels.overlay.apply(primitiveFrame(8, 'bad-overlay', 'scene'));
  assert.equal(overlaySceneCreate.applied, false);
  assert.equal(overlaySceneCreate.diagnostics[0]?.code, 'overlay_requires_debug_layer');
});

void test('editor viewport readout exposes retained generic lights and voxel material style', () => {
  const backend = new FakeEditorViewportBackend();
  const viewport = createRendererEditorViewportWithBackend(backend, { autoStart: false });
  const frame = primitiveFrame(7, 'voxel-preview', 'scene');
  const node = frame.ops[0];
  assert.equal(node?.op, 'create');
  const styledFrame: RenderFrameDiff = {
    schemaVersion: 1,
    ops: [
      ...(node?.op === 'create' ? [{
        ...node,
        node: { ...node.node, material: { color: [1, 1, 1, 0.75] as const, wireframe: true } },
      }] : []),
      {
        op: 'createLight', handle: renderHandle(8), parent: null,
        light: {
          kind: 'directional', color: [1, 0.9, 0.8], intensity: 2, enabled: true,
          direction: [-1, -2, -1], shadowIntent: 'disabled',
        },
      },
    ],
  };
  assert.equal(viewport.channels.authored.replace(styledFrame).applied, true);
  const projection = viewport.readout().channels.find((channel) => channel.channel === 'authored')?.projection;
  assert.equal(projection?.lights[0]?.light.kind, 'directional');
  assert.equal(projection?.nodes[0]?.material?.wireframe, true);
  assert.equal(projection?.nodes[0]?.material?.color[3], 0.75);
});

void test('editor viewport owns bounded lifecycle resize clear and channel disposal', () => {
  const backend = new FakeEditorViewportBackend();
  const viewport = createRendererEditorViewportWithBackend(backend, {
    autoStart: false,
    size: { width: 640, height: 360, pixelRatio: 1 },
  });

  assert.equal(viewport.readout().status, 'mounted');
  viewport.start();
  assert.equal(viewport.readout().status, 'running');
  assert.equal(backend.starts, 1);
  const submission = viewport.renderOnce(33);
  assert.deepEqual(backend.renderTimes, [33]);
  assert.deepEqual(submission, {
    schemaVersion: 1,
    drawCallCount: { scope: 'perSubmission', status: 'available', value: 4 },
    renderHandleCount: { scope: 'liveResident', status: 'available', value: 3 },
    geometryResourceCount: { scope: 'liveResident', status: 'available', value: 2 },
    materialResourceCount: { scope: 'liveResident', status: 'available', value: 2 },
    textureResourceCount: { scope: 'liveResident', status: 'available', value: 0 },
    animatedInstanceCount: { scope: 'liveResident', status: 'available', value: 0 },
    triangleCount: { scope: 'perSubmission', status: 'available', value: 24 },
  });
  viewport.stop();
  assert.equal(viewport.readout().status, 'stopped');
  assert.equal(backend.stops, 1);

  const resized = viewport.resize({ width: 1280, height: 720, pixelRatio: 2 });
  assert.equal(resized.applied, true);
  assert.deepEqual(backend.sizes.at(-1), { width: 1280, height: 720, pixelRatio: 2 });
  const invalidResize = viewport.resize({ width: 0, height: 720, pixelRatio: 2 });
  assert.equal(invalidResize.applied, false);
  assert.equal(invalidResize.diagnostics[0]?.code, 'invalid_viewport_size');

  assert.equal(viewport.channels.authored.replace(primitiveFrame(4, 'preview', 'scene')).applied, true);
  const disposedChannel = viewport.channels.authored.dispose();
  assert.equal(disposedChannel.applied, true);
  assert.equal(viewport.channels.authored.snapshot().disposed, true);
  assert.equal(viewport.channels.authored.snapshot().projection.nodes.length, 0);
  assert.equal(viewport.channels.authored.apply(primitiveFrame(5, 'late', 'scene')).applied, false);

  viewport.dispose();
  viewport.dispose();
  assert.equal(viewport.readout().status, 'disposed');
  assert.equal(backend.disposals, 1);
  assert.equal(viewport.channels.runtime.apply(primitiveFrame(1, 'late-runtime', 'scene')).applied, false);
  assert.equal(viewport.renderOnce().drawCallCount.status, 'unavailable');
});

void test('editor viewport preserves unavailable and unsupported backend statistics', () => {
  const backend = new FakeEditorViewportBackend();
  const viewport = createRendererEditorViewportWithBackend(backend, { autoStart: false });
  backend.submission = {
    drawCallCount: null,
    renderHandleCount: undefined,
    geometryResourceCount: 2,
    materialResourceCount: 3,
    textureResourceCount: null,
    animatedInstanceCount: undefined,
    triangleCount: 12,
  };

  const submission = viewport.renderOnce(50);

  assert.deepEqual(submission.drawCallCount, {
    scope: 'perSubmission',
    status: 'unavailable',
    value: null,
  });
  assert.deepEqual(submission.renderHandleCount, {
    scope: 'liveResident',
    status: 'unsupported',
    value: null,
  });
  assert.deepEqual(submission.textureResourceCount, {
    scope: 'liveResident',
    status: 'unavailable',
    value: null,
  });
  assert.deepEqual(submission.animatedInstanceCount, {
    scope: 'liveResident',
    status: 'unsupported',
    value: null,
  });
  assert.equal(Object.isFrozen(submission), true);
  assert.equal(Object.isFrozen(submission.drawCallCount), true);
});

void test('editor viewport validates and realizes one public procedural grid outside scene channels', () => {
  const backend = new FakeEditorViewportBackend();
  const viewport = createRendererEditorViewportWithBackend(backend, { autoStart: false });
  assert.equal(viewport.grid(), null);

  const descriptor = editorGridDescriptor();
  const applied = viewport.setGrid(descriptor);
  assert.equal(applied.applied, true);
  assert.deepEqual(applied.grid?.descriptor, descriptor);
  assert.equal(viewport.readout().grid?.renderedLineCount, 42);
  assert.equal(viewport.channels.overlay.snapshot().retainedOpCount, 0);

  const cameraHashBefore = viewport.readout().camera.pose.position;
  const moved = viewport.setCamera({
    ...viewport.camera(),
    pose: { ...viewport.camera().pose, position: [8, 10, 12] },
  });
  assert.equal(moved.applied, true);
  assert.notDeepEqual(viewport.readout().camera.pose.position, cameraHashBefore);
  assert.deepEqual(viewport.grid()?.descriptor.grid.spacing, [0.5, 1, 0.25]);

  const rejected = viewport.setGrid({
    ...descriptor,
    grid: { ...descriptor.grid, coordinateSystem: 'leftHandedZUp' as never },
  });
  assert.equal(rejected.applied, false);
  assert.equal(rejected.diagnostics[0]?.code, 'invalid_grid');
  assert.deepEqual(viewport.grid()?.descriptor, descriptor);

  const cleared = viewport.setGrid(null);
  assert.equal(cleared.applied, true);
  assert.equal(cleared.grid, null);
});

void test('editor viewport accepts stored and caller cameras and maps pixel picks to typed logical hints', () => {
  const backend = new FakeEditorViewportBackend();
  const viewport = createRendererEditorViewportWithBackend(backend, {
    autoStart: false,
    size: { width: 800, height: 400, pixelRatio: 1 },
  });
  assert.equal(viewport.channels.authored.replace(primitiveFrame(19, 'pick-me', 'scene')).applied, true);

  const callerCamera: RendererEditorViewportCamera = {
    source: 'caller',
    pose: { position: [0, 2, 5], yawDegrees: 180, pitchDegrees: 0 },
    basis: canonicalBasis(),
    projection: { fovYDegrees: 60, near: 0.1, far: 500 },
  };
  const cameraReceipt = viewport.setCamera(callerCamera);
  assert.equal(cameraReceipt.applied, true);
  assert.equal(viewport.camera().source, 'caller');
  assert.deepEqual(backend.cameras.at(-1), callerCamera);

  const cameraBeforeInvalid = viewport.camera();
  const invalidCamera = viewport.setCamera({
    ...callerCamera,
    basis: { ...callerCamera.basis, forward: [0, 0, 0] },
  });
  assert.equal(invalidCamera.applied, false);
  assert.equal(invalidCamera.diagnostics[0]?.code, 'invalid_camera');
  assert.deepEqual(viewport.camera(), cameraBeforeInvalid);

  const backendHandle = firstCreatedHandle(backend.frame('authored'));
  backend.pickResult = {
    diagnostics: [],
    hit: {
      channel: 'authored',
      distance: 4.25,
      handle: renderHandle(backendHandle),
      label: 'pick-me',
      layer: 'scene',
      normal: [0, 1, 0],
      position: [1, 2, 3],
      sourceTrace: null,
      tags: [],
    },
  };
  const beforePickHash = viewport.readout().viewportHash;
  const picked = viewport.pick({
    point: [400, 200],
    filter: { channels: ['authored'], handles: [renderHandle(19)], layers: ['scene'] },
  });
  assert.deepEqual(backend.pickRequests.at(-1)?.point, [0, 0]);
  assert.deepEqual(picked.hint, {
    channel: 'authored',
    distance: 4.25,
    handle: 19,
    label: 'pick-me',
    layer: 'scene',
    normal: [0, 1, 0],
    position: [1, 2, 3],
    sourceTrace: null,
    tags: [],
  });
  assert.equal(viewport.readout().viewportHash, beforePickHash);

  const invalidPick = viewport.pick({ point: [801, 200] });
  assert.equal(invalidPick.hint, null);
  assert.equal(invalidPick.diagnostics[0]?.code, 'invalid_pick_request');

  backend.pickResult = {
    diagnostics: [],
    hit: {
      channel: 'authored',
      distance: 2,
      handle: renderHandle(backendHandle + 1),
      label: 'stale',
      layer: 'scene',
      normal: [0, 1, 0],
      position: [0, 0, 0],
      sourceTrace: null,
      tags: [],
    },
  };
  const stalePick = viewport.pick({ point: [400, 200], filter: { channels: ['authored'] } });
  assert.equal(stalePick.hint, null);
  assert.equal(stalePick.diagnostics[0]?.code, 'backend_rejected');
});

void test('one degraded resource channel leaves healthy runtime and overlay projections intact', () => {
  const backend = new FakeEditorViewportBackend();
  const viewport = createRendererEditorViewportWithBackend(backend, { autoStart: false });
  assert.equal(viewport.channels.runtime.replace(primitiveFrame(1, 'runtime-ok', 'scene')).applied, true);
  const runtimeBefore = viewport.channels.runtime.snapshot();

  backend.rejectChannel = 'authored';
  const degraded = viewport.channels.authored.replace(primitiveFrame(2, 'missing-resource', 'scene'));
  assert.equal(degraded.applied, false);
  assert.equal(degraded.diagnostics[0]?.code, 'backend_rejected');
  assert.deepEqual(viewport.channels.runtime.snapshot(), runtimeBefore);
  assert.equal(viewport.channels.authored.snapshot().projection.nodes.length, 0);

  backend.rejectChannel = null;
  assert.equal(viewport.channels.overlay.replace(primitiveFrame(3, 'debug-ok', 'debug')).applied, true);
  assert.equal(viewport.channels.overlay.snapshot().projection.nodes.length, 1);
  assert.equal(viewport.readout().diagnostics.at(-1)?.channel, 'authored');
});

class FakeEditorViewportBackend implements RendererEditorViewportBackendPort {
  readonly frames = new Map<string, RenderFrameDiff>();
  readonly cameras: unknown[] = [];
  readonly sizes: unknown[] = [];
  readonly renderTimes: Array<number | undefined> = [];
  readonly pickRequests: Array<{ readonly point: readonly [number, number] }> = [];
  grid: EditorGridProjectionReadout | null = null;
  starts = 0;
  stops = 0;
  disposals = 0;
  rejectChannel: 'runtime' | 'authored' | 'overlay' | null = null;
  pickResult: ReturnType<RendererEditorViewportBackendPort['pick']> = {
    diagnostics: [],
    hit: null,
  };
  submission: ReturnType<RendererEditorViewportBackendPort['renderOnce']> = {
    drawCallCount: 4,
    renderHandleCount: 3,
    geometryResourceCount: 2,
    materialResourceCount: 2,
    textureResourceCount: 0,
    animatedInstanceCount: 0,
    triangleCount: 24,
  };

  replaceChannel(channel: 'runtime' | 'authored' | 'overlay', frame: RenderFrameDiff): void {
    if (this.rejectChannel === channel) {
      throw new Error(`missing resource in ${channel}`);
    }
    this.frames.set(channel, structuredClone(frame));
  }

  frame(channel: 'runtime' | 'authored' | 'overlay'): RenderFrameDiff {
    return this.frames.get(channel) ?? { schemaVersion: 1, ops: [] };
  }

  setCamera(camera: unknown): void {
    this.cameras.push(structuredClone(camera));
  }

  setGrid(descriptor: EditorGridDescriptor | null): void {
    this.grid = descriptor === null ? null : {
      descriptor: structuredClone(descriptor),
      bounds: { min: [-8, descriptor.grid.origin[1], -8], max: [8, descriptor.grid.origin[1], 8] },
      minorLineStep: 1,
      renderedLineCount: 42,
    };
  }

  gridReadout(): EditorGridProjectionReadout | null {
    return this.grid === null ? null : structuredClone(this.grid);
  }

  resize(size: unknown): void {
    this.sizes.push(structuredClone(size));
  }

  pick(request: { readonly point: readonly [number, number] }): ReturnType<RendererEditorViewportBackendPort['pick']> {
    this.pickRequests.push(structuredClone(request));
    return this.pickResult;
  }

  renderOnce(timeMs?: number): ReturnType<RendererEditorViewportBackendPort['renderOnce']> {
    this.renderTimes.push(timeMs);
    return this.submission;
  }

  start(): void {
    this.starts += 1;
  }

  stop(): void {
    this.stops += 1;
  }

  snapshot(): string {
    return JSON.stringify([...this.frames]);
  }

  dispose(): void {
    this.disposals += 1;
  }
}

function editorGridDescriptor(): EditorGridDescriptor {
  return {
    visible: true,
    grid: {
      coordinateSystem: 'rightHandedYUp',
      origin: [0.25, 0, -0.5],
      spacing: [0.5, 1, 0.25],
    },
    plane: 'xz',
    snapAnchor: 'boundary',
    style: {
      minorColor: [0.1, 0.2, 0.3, 0.4],
      majorColor: [0.2, 0.3, 0.4, 0.8],
      xAxisColor: [1, 0, 0, 1],
      yAxisColor: [0, 1, 0, 1],
      zAxisColor: [0, 0, 1, 1],
      majorLineEvery: 4,
      opacity: 0.9,
      fadeStart: 12,
      fadeEnd: 48,
    },
  };
}

function primitiveFrame(handle: number, label: string, layer: 'scene' | 'debug'): RenderFrameDiff {
  return {
    schemaVersion: 1,
    ops: [{
      op: 'create',
      handle: renderHandle(handle),
      parent: null,
      node: {
        geometry: { kind: 'cube' },
        material: { color: [0.2, 0.4, 0.6, 1], wireframe: layer === 'debug' },
        transform: {
          translation: [0, 0, 0],
          rotation: [0, 0, 0, 1],
          scale: [1, 1, 1],
        },
        visible: true,
        layer,
        metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label },
      },
    }],
  };
}

function firstCreatedHandle(frame: RenderFrameDiff): number {
  const create = frame.ops.find((op) => op.op === 'create');
  assert.ok(create && 'handle' in create);
  return create.handle;
}

function canonicalBasis(): CameraBasis {
  return {
    forward: [0, 0, -1],
    right: [1, 0, 0],
    up: [0, 1, 0],
  };
}
