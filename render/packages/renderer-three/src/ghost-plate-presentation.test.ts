import assert from 'node:assert/strict';
import test from 'node:test';
import * as THREE from 'three';

import type {
  GhostPlateCaptureLighting,
  GhostPlateDescriptor,
  RenderFrameDiff,
} from '@rusty-engine/render-contracts';
import {
  RendererThreeGhostPlatePresentation,
  type RendererThreeGhostPlateBackend,
} from './ghost-plate-presentation.js';

const GHOST_CAPTURE_MAX_RETAINED_BYTES = 256 * 1024 * 1024;
const GHOST_CAPTURE_BYTES_PER_PIXEL_PER_SECTOR = 20;
const SOURCE_HANDLE = 101 as GhostPlateDescriptor['source'];
const ISOLATED_LIGHTING: GhostPlateCaptureLighting = {
  mode: 'isolated',
  ambientColor: [1, 1, 1],
  ambientIntensity: 0,
  keyDirection: [1, 0, 0],
  keyColor: [1, 1, 1],
  keyIntensity: 0,
  fillDirection: [0, 1, 0],
  fillColor: [1, 1, 1],
  fillIntensity: 0,
};

void test('ghost admission accounts live peers and replacement peak while retaining the old plate', () => {
  const renderer = new FakeRenderer();
  const backend = new FakeBackend();
  const first = createPresentation(renderer, backend);
  const second = createPresentation(renderer, backend);
  const third = createPresentation(renderer, backend);
  const fourth = createPresentation(renderer, backend);
  const fifth = createPresentation(renderer, backend);
  try {
    // Two live presentations leave enough room for a 75x75 candidate, but
    // current + candidate is the replacement peak and exceeds the budget.
    assert.equal(first.create(descriptor(2589)).applied, true);
    assert.equal(second.create(descriptor(2590)).applied, true);
    assert.equal(third.create(descriptor(75)).applied, true);
    const before = third.readout();
    const rejected = third.recapture({ ...before.capture, resolution: 74 });
    assert.equal(rejected.applied, false);
    assert.equal(rejected.diagnostics[0]?.code, 'captureFailed');
    assert.equal(third.readout().capture.resolution, 75);
    assert.deepEqual(third.readout().capture, before.capture);

    // Releasing the current plate must return its whole estimate to the
    // renderer aggregate. The adjacent 74x74 candidate now fits, while 75x75
    // is one boundary step too large for the remaining aggregate.
    third.dispose();
    assert.equal(fourth.create(descriptor(74)).applied, true);
    assert.equal(fifth.create(descriptor(75)).applied, false);
    assert.equal(fifth.readout().capture.resolution, 8, 'rejected presentation remains inactive');
  } finally {
    fifth.dispose();
    fourth.dispose();
    third.dispose();
    second.dispose();
    first.dispose();
    backend.dispose();
  }
});

void test('failed ghost candidates do not debit accounting from another live presentation', () => {
  const renderer = new FakeRenderer();
  const backend = new FakeBackend();
  const stable = createPresentation(renderer, backend);
  const failing = createPresentation(renderer, backend);
  const oversized = createPresentation(renderer, backend);
  let rejectBackendAdd = false;
  const originalAdd = backend.scene.add.bind(backend.scene);
  backend.scene.add = (...objects: THREE.Object3D[]) => {
    if (rejectBackendAdd) throw new Error('synthetic presentation attach failure');
    originalAdd(...objects);
    return backend.scene;
  };
  try {
    assert.equal(stable.create(descriptor(2590)).applied, true);
    rejectBackendAdd = true;
    const failed = failing.create(descriptor(2000));
    assert.equal(failed.applied, false);
    assert.equal(failed.diagnostics[0]?.code, 'captureFailed');
    rejectBackendAdd = false;

    // If the failed candidate had been charged (or its cleanup had debited
    // another live plate), this candidate would fit incorrectly. With the
    // stable 2590x2590 plate still accounted, it must be rejected. A leaked
    // 2000x2000 debit would incorrectly bring this candidate just under the
    // aggregate limit.
    const rejected = oversized.create(descriptor(3270));
    assert.equal(rejected.applied, false);
    assert.equal(stable.readout().capture.resolution, 2590);
  } finally {
    backend.scene.add = originalAdd;
    oversized.dispose();
    failing.dispose();
    stable.dispose();
    backend.dispose();
  }
});

void test('successful replacement adjusts current accounting once and disposal releases the exact boundary', () => {
  const renderer = new FakeRenderer();
  const backend = new FakeBackend();
  const presentation = createPresentation(renderer, backend);
  const peer = createPresentation(renderer, backend);
  const large = createPresentation(renderer, backend);
  const exact = createPresentation(renderer, backend);
  const oneOver = createPresentation(renderer, backend);
  try {
    assert.equal(presentation.create(descriptor(2000)).applied, true);
    assert.equal(peer.create(descriptor(1000)).applied, true);
    assert.equal(presentation.recapture({ ...descriptor(1000).capture }).applied, true);
    assert.equal(presentation.readout().capture.resolution, 1000);

    // The successful 2000 -> 1000 replacement leaves 40,000,000 bytes
    // accounted across both live presentations. A 3464 candidate cannot fit;
    // an under-debit would incorrectly admit it.
    assert.equal(large.create(descriptor(3464)).applied, false);

    presentation.dispose();
    peer.dispose();
    const exactBytes = 3663 * 3663 * GHOST_CAPTURE_BYTES_PER_PIXEL_PER_SECTOR;
    assert.ok(exactBytes < GHOST_CAPTURE_MAX_RETAINED_BYTES);
    assert.equal(exact.create(descriptor(3663)).applied, true);
    exact.dispose();
    assert.equal(oneOver.create(descriptor(3664)).applied, false);
  } finally {
    oneOver.dispose();
    exact.dispose();
    large.dispose();
    peer.dispose();
    presentation.dispose();
    backend.dispose();
  }
});

void test('captured ghost scenes stay isolated across placement updates and change only on explicit recapture', () => {
  const renderer = new FakeRenderer();
  const backend = new CapturedSceneBackend();
  const presentation = createPresentation(renderer, backend);
  const initial = capturedScene('initial-frozen-pose');
  const recaptured = capturedScene('explicitly-recaptured-pose');
  try {
    assert.equal(presentation.create({ ...descriptor(8), capturedScene: initial }).applied, true);
    assert.deepEqual(backend.realizedLabels, ['initial-frozen-pose']);
    assert.equal(backend.disposals, 1);

    assert.equal(presentation.update({
      placement: {
        ...descriptor(8).placement,
        transform: { translation: [9, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
      },
    }).applied, true);
    assert.deepEqual(backend.realizedLabels, ['initial-frozen-pose', 'initial-frozen-pose']);

    assert.equal(presentation.recapture(null, recaptured).applied, true);
    assert.deepEqual(backend.realizedLabels, [
      'initial-frozen-pose',
      'initial-frozen-pose',
      'explicitly-recaptured-pose',
    ]);
    assert.equal(backend.disposals, 3);
  } finally {
    presentation.dispose();
    backend.dispose();
  }
});

function createPresentation(
  renderer: FakeRenderer,
  backend: RendererThreeGhostPlateBackend,
): RendererThreeGhostPlatePresentation {
  return new RendererThreeGhostPlatePresentation({
    webgl: renderer as unknown as THREE.WebGLRenderer,
    backend,
    invalidate: () => undefined,
    onDispose: () => undefined,
  });
}

function descriptor(resolution: number): GhostPlateDescriptor {
  return {
    source: SOURCE_HANDLE,
    placement: {
      transform: {
        translation: [0, 0, 0],
        rotation: [0, 0, 0, 1],
        scale: [1, 1, 1],
      },
      width: 2,
      height: 2,
    },
    capture: {
      resolution,
      azimuthDegrees: 0,
      elevationDegrees: 0,
      near: 0.1,
      far: 20,
      fieldOfViewDegrees: 35,
      lighting: ISOLATED_LIGHTING,
    },
    config: {
      depthRetention: 0.12,
      anchorPolicy: 'bounds-center',
      anchorValue: 0.5,
      plateMapping: 'plate-locked',
      shellMode: 'whole-mesh',
      shellDepthEpsilon: 0.12,
      sectorCount: 1,
      sectorHysteresisDegrees: 0,
    },
  };
}

class FakeBackend implements RendererThreeGhostPlateBackend {
  readonly scene = new THREE.Scene();
  readonly source = new THREE.Mesh(
    new THREE.BoxGeometry(1, 1, 1),
    new THREE.MeshBasicMaterial({ color: 0x88aaff }),
  );

  constructor() {
    this.source.name = 'retained-source';
    this.scene.add(this.source);
  }

  objectFor(handle: GhostPlateDescriptor['source']): THREE.Object3D | undefined {
    return handle === SOURCE_HANDLE ? this.source : undefined;
  }

  dispose(): void {
    this.scene.clear();
    this.source.geometry.dispose();
    const material = this.source.material;
    if (Array.isArray(material)) material.forEach((item) => item.dispose());
    else material.dispose();
  }
}

class CapturedSceneBackend implements RendererThreeGhostPlateBackend {
  readonly scene = new THREE.Scene();
  readonly realizedLabels: string[] = [];
  disposals = 0;

  objectFor(_handle: GhostPlateDescriptor['source']): THREE.Object3D | undefined {
    throw new Error('captured ghost realization must not read the live source');
  }

  createIsolatedCaptureScene(frame: RenderFrameDiff) {
    const source = new THREE.Mesh(
      new THREE.BoxGeometry(1, 1, 1),
      new THREE.MeshBasicMaterial({ color: 0x88aaff }),
    );
    const label = frame.ops[0]?.op === 'create' ? frame.ops[0].node.metadata.label : null;
    if (label === null) throw new Error('test capture frame must create the frozen source');
    this.realizedLabels.push(label);
    const scene = new THREE.Scene();
    scene.add(source);
    return {
      scene,
      objectFor: (handle: GhostPlateDescriptor['source']) => handle === SOURCE_HANDLE ? source : undefined,
      sceneFor: (handle: GhostPlateDescriptor['source']) => handle === SOURCE_HANDLE ? scene : undefined,
      dispose: () => {
        this.disposals += 1;
        scene.clear();
        source.geometry.dispose();
        const material = source.material;
        if (Array.isArray(material)) material.forEach((item) => item.dispose());
        else material.dispose();
      },
    };
  }

  dispose(): void {
    this.scene.clear();
  }
}

function capturedScene(label: string): RenderFrameDiff {
  return {
    schemaVersion: 1,
    ops: [{
      op: 'create',
      handle: SOURCE_HANDLE,
      parent: null,
      node: {
        geometry: { kind: 'cube' },
        material: { color: [1, 1, 1, 1], wireframe: false },
        transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
        visible: true,
        layer: 'scene',
        metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label },
      },
    }],
  };
}

class FakeRenderer {
  autoClear = true;
  clearAlpha = 1;
  clearColor = new THREE.Color(0);
  renderTarget: THREE.WebGLRenderTarget | null = null;
  scissor = new THREE.Vector4(0, 0, 1, 1);
  scissorTest = false;
  viewport = new THREE.Vector4(0, 0, 1, 1);
  readonly xr = { enabled: false };

  clear(_color: boolean, _depth: boolean, _stencil: boolean): void {}

  getClearAlpha(): number {
    return this.clearAlpha;
  }

  getClearColor(target: THREE.Color): THREE.Color {
    return target.copy(this.clearColor);
  }

  getRenderTarget(): THREE.WebGLRenderTarget | null {
    return this.renderTarget;
  }

  getScissor(target: THREE.Vector4): THREE.Vector4 {
    return target.copy(this.scissor);
  }

  getScissorTest(): boolean {
    return this.scissorTest;
  }

  getViewport(target: THREE.Vector4): THREE.Vector4 {
    return target.copy(this.viewport);
  }

  render(_scene: THREE.Scene, _camera: THREE.Camera): void {}

  setClearColor(color: THREE.ColorRepresentation | THREE.Color, alpha = 1): void {
    if (color instanceof THREE.Color) this.clearColor.copy(color);
    else this.clearColor.set(color);
    this.clearAlpha = alpha;
  }

  setRenderTarget(target: THREE.WebGLRenderTarget | null): void {
    this.renderTarget = target;
  }

  setScissor(value: THREE.Vector4 | number, y?: number, width?: number, height?: number): void {
    assignVector(this.scissor, value, y, width, height);
  }

  setScissorTest(enabled: boolean): void {
    this.scissorTest = enabled;
  }

  setViewport(value: THREE.Vector4 | number, y?: number, width?: number, height?: number): void {
    assignVector(this.viewport, value, y, width, height);
  }
}

function assignVector(
  target: THREE.Vector4,
  value: THREE.Vector4 | number,
  y?: number,
  width?: number,
  height?: number,
): void {
  if (value instanceof THREE.Vector4) {
    target.copy(value);
    return;
  }
  target.set(value, y ?? 0, width ?? 0, height ?? 0);
}
