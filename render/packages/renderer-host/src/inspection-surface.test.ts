import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  renderHandle,
  type EditorGridDescriptor,
  type EditorGridProjectionReadout,
  type RenderDiff,
  type RenderFrameDiff,
} from '@rusty-engine/render-contracts';
import {
  RUSTY_RENDERER_EDITOR_VIEWPORT_MAX_FRAME_OPS,
  RUSTY_RENDERER_EDITOR_VIEWPORT_MAX_RETAINED_OPS,
  createRendererEditorViewportWithBackend,
  type RendererEditorViewportBackendPort,
  type RendererEditorViewportSize,
} from './editor-viewport.js';
import {
  createRendererInspectionSurfaceWithViewport,
  type RendererInspectionSurfaceControlsOptions,
} from './inspection-surface.js';

void test('inspection surface owns coherent pointer orbit plus focused movement orbit and zoom', () => {
  const harness = createInspectionHarness({ autoStart: false });
  const beforeOrbit = harness.surface.camera();
  const initialRevision = harness.surface.readout().cameraRevision;

  harness.canvas.emit('pointerdown', pointerEvent({ button: 0, clientX: 100, clientY: 100, pointerId: 7 }));
  assert.equal(harness.surface.readout().dragging, true);
  assert.equal(harness.canvas.hasPointerCapture(7), true);
  harness.canvas.emit('pointermove', pointerEvent({ buttons: 1, clientX: 140, clientY: 80, pointerId: 7 }));
  harness.canvas.emit('pointerup', pointerEvent({ button: 0, clientX: 140, clientY: 80, pointerId: 7 }));

  const afterOrbit = harness.surface.camera();
  assert.notDeepEqual(afterOrbit.pose.position, beforeOrbit.pose.position);
  assert.notDeepEqual(afterOrbit.basis.forward, beforeOrbit.basis.forward);
  assert.equal(harness.surface.readout().dragging, false);
  assert.equal(harness.canvas.hasPointerCapture(7), false);
  assert.equal(harness.surface.readout().lastCameraChange, 'pointer_orbit');
  assert.ok(harness.surface.readout().cameraRevision > initialRevision);

  let beforeMovement = afterOrbit;
  for (const [index, code] of ['KeyW', 'KeyA', 'KeyS', 'KeyD'].entries()) {
    harness.document.emit('keydown', keyboardEvent(code));
    assert.deepEqual(harness.surface.readout().pressedMovementKeys, [code]);
    harness.surface.renderOnce((index + 1) * 1000);
    const afterMovement = harness.surface.camera();
    assert.notDeepEqual(afterMovement.pose.position, beforeMovement.pose.position);
    harness.document.emit('keyup', keyboardEvent(code));
    beforeMovement = afterMovement;
  }
  assert.deepEqual(harness.surface.readout().pressedMovementKeys, []);

  const beforeKeyboardOrbit = harness.surface.camera();
  harness.document.emit('keydown', keyboardEvent('ArrowLeft', 'ArrowLeft'));
  assert.deepEqual(harness.surface.readout().pressedOrbitKeys, ['ArrowLeft']);
  harness.surface.renderOnce(5000);
  harness.document.emit('keyup', keyboardEvent('ArrowLeft', 'ArrowLeft'));
  assert.notDeepEqual(harness.surface.camera().pose.position, beforeKeyboardOrbit.pose.position);
  assert.equal(harness.surface.readout().lastCameraChange, 'keyboard_orbit');
  assert.deepEqual(harness.surface.readout().pressedOrbitKeys, []);

  const beforeKeyboardZoom = harness.surface.readout().cameraDistance;
  harness.document.emit('keydown', keyboardEvent('Equal', '+'));
  assert.ok(harness.surface.readout().cameraDistance < beforeKeyboardZoom);
  assert.equal(harness.surface.readout().lastCameraChange, 'keyboard_zoom');

  const beforeWheelZoom = harness.surface.readout().cameraDistance;
  harness.canvas.emit('wheel', wheelEvent(120));
  assert.ok(harness.surface.readout().cameraDistance > beforeWheelZoom);
  assert.equal(harness.surface.readout().lastCameraChange, 'wheel_zoom');

  assert.equal(harness.surface.role, 'projection_only_inspection');
  assert.equal(harness.surface.readout().role, 'projection_only_inspection');
  assert.equal(harness.surface.readout().camera.source, 'stored_editor');
});

void test('inspection controls apply configurable six-axis movement, boost, inversion, and pointer pan', () => {
  const harness = createInspectionHarness({ autoStart: false });
  harness.surface.configureControlPreferences({
    moveSpeed: 10,
    boostMultiplier: 3,
    invertLookY: true,
    invertPanY: true,
    keyboard: {
      moveForward: 'KeyI',
      moveBackward: 'KeyK',
      moveLeft: 'KeyJ',
      moveRight: 'KeyL',
      moveDown: 'KeyU',
      moveUp: 'KeyO',
      boost: 'ShiftRight',
    },
  });
  assert.deepEqual(harness.surface.readout().controlPreferences, {
    moveSpeed: 10,
    boostMultiplier: 3,
    invertLookY: true,
    invertPanY: true,
    keyboard: {
      moveForward: 'KeyI',
      moveBackward: 'KeyK',
      moveLeft: 'KeyJ',
      moveRight: 'KeyL',
      moveDown: 'KeyU',
      moveUp: 'KeyO',
      boost: 'ShiftRight',
    },
  });

  harness.canvas.focus();
  const beforeUp = harness.surface.camera();
  harness.document.emit('keydown', keyboardEvent('KeyO'));
  harness.document.emit('keydown', keyboardEvent('ShiftRight'));
  harness.surface.renderOnce(1000);
  harness.document.emit('keyup', keyboardEvent('KeyO'));
  harness.document.emit('keyup', keyboardEvent('ShiftRight'));
  const afterUp = harness.surface.camera();
  assert.ok(afterUp.pose.position[1] > beforeUp.pose.position[1]);
  assert.ok(afterUp.pose.position[1] - beforeUp.pose.position[1] > 2.9);

  const beforeOrbit = harness.surface.camera();
  harness.canvas.emit('pointerdown', pointerEvent({ button: 0, clientX: 50, clientY: 50, pointerId: 8 }));
  harness.canvas.emit('pointermove', pointerEvent({ buttons: 1, clientX: 50, clientY: 70, pointerId: 8 }));
  harness.canvas.emit('pointerup', pointerEvent({ button: 0, pointerId: 8 }));
  assert.ok(harness.surface.camera().pose.pitchDegrees > beforeOrbit.pose.pitchDegrees);

  const beforePan = harness.surface.camera();
  harness.canvas.emit('pointerdown', pointerEvent({ button: 1, clientX: 50, clientY: 50, pointerId: 9 }));
  harness.canvas.emit('pointermove', pointerEvent({ buttons: 4, clientX: 80, clientY: 70, pointerId: 9 }));
  harness.canvas.emit('pointerup', pointerEvent({ button: 1, pointerId: 9 }));
  assert.notDeepEqual(harness.surface.camera().pose.position, beforePan.pose.position);
  assert.equal(harness.surface.readout().lastCameraChange, 'pointer_pan');
});

void test('inspection surface retargets its orbit pivot without changing orientation or distance', () => {
  const harness = createInspectionHarness({
    autoStart: false,
    controls: {
      initialPosition: [8, 6, 10],
      initialTarget: [1, 2, 3],
    },
  });
  const before = harness.surface.readout();
  const offset = before.camera.pose.position.map(
    (coordinate, axis) => coordinate - [1, 2, 3][axis]!,
  );

  assert.equal(harness.surface.focusTarget([20, -4, 7]), true);

  const focused = harness.surface.readout();
  assertVectorApproximately(
    focused.camera.pose.position.map((coordinate, axis) =>
      coordinate - [20, -4, 7][axis]!),
    offset,
  );
  assertVectorApproximately(focused.camera.basis.forward, before.camera.basis.forward);
  assertVectorApproximately(focused.camera.basis.right, before.camera.basis.right);
  assertVectorApproximately(focused.camera.basis.up, before.camera.basis.up);
  assert.equal(focused.cameraDistance, before.cameraDistance);
  assert.equal(focused.cameraRevision, before.cameraRevision + 1);
  assert.equal(focused.lastCameraChange, 'focus_target');

  assert.equal(harness.surface.focusTarget([Number.NaN, 0, 0]), false);
  assert.deepEqual(harness.surface.readout(), focused);
});

void test('inspection surface frames finite bounds from a deterministic front view', () => {
  const harness = createInspectionHarness({ autoStart: false });

  assert.equal(harness.surface.frameBounds({ min: [-1, 0, -0.5], max: [1, 2, 0.5] }), true);
  const framed = harness.surface.readout();
  assertVectorApproximately(framed.camera.basis.forward, [0, 0, -1]);
  assertVectorApproximately(framed.camera.pose.position, [0, 1, framed.cameraDistance]);
  assert.equal(framed.lastCameraChange, 'frame_bounds');

  assert.equal(
    harness.surface.frameBounds({ min: [1, 0, 0], max: [-1, 2, 1] }),
    false,
  );
  assert.deepEqual(harness.surface.readout(), framed);
});

void test('inspection surface applies replaces and clears the engine procedural grid', () => {
  const initialGrid = editorGridDescriptor();
  const harness = createInspectionHarness({ autoStart: false, initialGrid });

  assert.deepEqual(harness.surface.grid()?.descriptor, initialGrid);
  assert.deepEqual(harness.surface.readout().grid?.descriptor, initialGrid);
  assert.equal(harness.surface.readout().gridRevision, 1);

  const cleared = harness.surface.setGrid(null);
  assert.equal(cleared.applied, true);
  assert.equal(harness.surface.grid(), null);
  assert.equal(harness.surface.readout().gridRevision, 2);

  const replacement = {
    ...initialGrid,
    grid: { ...initialGrid.grid, spacing: [2, 2, 2] as const },
  };
  const replaced = harness.surface.setGrid(replacement);
  assert.equal(replaced.applied, true);
  assert.deepEqual(harness.surface.readout().grid?.descriptor, replacement);
  assert.equal(harness.surface.readout().gridRevision, 3);
});

void test('inspection controls bound pitch and camera distance under repeated focused input', () => {
  const harness = createInspectionHarness({
    autoStart: false,
    controls: {
      initialPosition: [0, 0, 5],
      minimumDistance: 2,
      maximumDistance: 10,
    },
  });
  harness.canvas.focus();

  for (let index = 0; index < 32; index += 1) {
    harness.document.emit('keydown', keyboardEvent('Equal', '+'));
  }
  assert.equal(harness.surface.readout().cameraDistance, 2);
  for (let index = 0; index < 32; index += 1) {
    harness.document.emit('keydown', keyboardEvent('Minus', '-'));
  }
  assert.equal(harness.surface.readout().cameraDistance, 10);

  harness.document.emit('keydown', keyboardEvent('ArrowUp', 'ArrowUp'));
  for (let index = 1; index <= 32; index += 1) {
    harness.surface.renderOnce(index * 100);
  }
  harness.document.emit('keyup', keyboardEvent('ArrowUp', 'ArrowUp'));
  assert.ok(Math.abs(harness.surface.camera().pose.pitchDegrees) <= 85.000_001);
});

void test('inspection controls clamp the initial camera to the pitch bound before mount readout', () => {
  for (const initialPosition of [[0, 100, 1], [0, -100, 1]] as const) {
    const harness = createInspectionHarness({
      autoStart: false,
      controls: { initialPosition },
    });

    const initialReadout = harness.surface.readout();
    assert.ok(Math.abs(initialReadout.camera.pose.pitchDegrees) <= 85.000_001);
    assert.equal(initialReadout.cameraRevision, 1);
    assert.equal(initialReadout.lastCameraChange, 'initial_camera');
    harness.surface.dispose();
  }
});

void test('inspection surface retains accepted frames, fails closed on malformed replacement, and owns resize', () => {
  const harness = createInspectionHarness({ autoStart: false, frame: primitiveFrame(7) });
  const accepted = harness.surface.readout();
  assert.equal(accepted.retainedOpCount, 1);

  const malformed = harness.surface.replaceFrame({ ops: null } as unknown as RenderFrameDiff);
  assert.equal(malformed.applied, false);
  assert.equal(malformed.diagnostics[0]?.code, 'invalid_frame');
  assert.equal(harness.surface.readout().retainedFrameHash, accepted.retainedFrameHash);
  assert.equal(harness.surface.readout().retainedOpCount, 1);

  harness.canvas.clientWidth = 900;
  harness.canvas.clientHeight = 500;
  const canvasResize = harness.surface.resizeToCanvas();
  assert.equal(canvasResize.applied, true);
  assert.deepEqual(harness.backend.sizes.at(-1), { width: 900, height: 500, pixelRatio: 1.5 });

  const explicitResize = harness.surface.resize({ width: 1200, height: 700, pixelRatio: 2 });
  assert.equal(explicitResize.applied, true);
  assert.deepEqual(harness.backend.sizes.at(-1), { width: 1200, height: 700, pixelRatio: 2 });
});

void test('inspection surface incrementally applies an authored patch after its retained base', () => {
  const harness = createInspectionHarness({ autoStart: false, frame: primitiveFrame(7) });
  const initial = harness.surface.readout();

  const applied = harness.surface.applyAuthoredFrame({
    schemaVersion: 1,
    ops: [updateVisibility(7, false)],
  });

  assert.equal(applied.applied, true);
  assert.equal(applied.channel, 'authored');
  assert.equal(applied.generation, 2);
  assert.equal(harness.surface.readout().retainedOpCount, 2);
  assert.notEqual(harness.surface.readout().retainedFrameHash, initial.retainedFrameHash);
  assert.equal(harness.backend.frames.get('authored')?.ops.length, 2);
});

void test('inspection surface publishes immutable mount explicit and automatic submission samples', () => {
  const harness = createInspectionHarness({ autoStart: true, frame: primitiveFrame(7) });
  const mounted = harness.surface.submission();
  assert.equal(mounted.renderSequence, 1);
  assert.equal(mounted.source, 'mount');
  assert.equal(mounted.sourceTimeMs, 0);
  assert.equal(mounted.statistics.drawCallCount.value, 4);
  assert.equal(mounted.statistics.renderHandleCount.value, 3);
  assert.equal(Object.isFrozen(mounted), true);
  assert.equal(Object.isFrozen(mounted.statistics), true);

  harness.backend.submission = {
    drawCallCount: 9,
    renderHandleCount: 8,
    geometryResourceCount: 7,
    materialResourceCount: 6,
    textureResourceCount: 5,
    animatedInstanceCount: 4,
    triangleCount: 3,
  };
  harness.animation.runNext(16);
  const automatic = harness.surface.submission();
  assert.equal(automatic.renderSequence, 2);
  assert.equal(automatic.source, 'animationFrame');
  assert.equal(automatic.sourceTimeMs, 16);
  assert.equal(automatic.statistics.drawCallCount.value, 9);
  assert.equal(mounted.statistics.drawCallCount.value, 4);

  harness.surface.renderOnce(33);
  const explicit = harness.surface.submission();
  assert.equal(explicit.renderSequence, 3);
  assert.equal(explicit.source, 'explicit');
  assert.equal(explicit.sourceTimeMs, 33);
  assert.equal(explicit.frameIntervalMs, 17);
  assert.equal(explicit.frameIntervalStatus, 'available');
});

void test('inspection surface delegates disposable authored animation sampling and playback', () => {
  const harness = createInspectionHarness({ autoStart: false, frame: primitiveFrame(7) });
  const before = harness.surface.readout().retainedFrameHash;

  const sample = harness.surface.sampleAnimatedMesh(renderHandle(7), 'run', 0.5);
  harness.surface.setAnimatedMeshPlayback(renderHandle(7), {
    kind: 'play',
    clip: 'idle',
    loop: 'repeat',
    speed: 1,
    weight: 1,
    restart: true,
    fadeSeconds: 0.2,
  });

  assert.equal(sample.clip, 'run');
  assert.equal(sample.normalizedTime, 0.5);
  assert.deepEqual(harness.backend.animationSamples, [['authored', 7, 'run', 0.5]]);
  assert.equal(harness.backend.playbackCommands[0]?.[0], 'authored');
  assert.equal(harness.backend.playbackCommands[0]?.[1], 7);
  assert.equal(harness.backend.playbackCommands[0]?.[2].kind, 'play');
  assert.equal(harness.surface.readout().retainedFrameHash, before);
});

void test('inspection submission preserves classified counters and lifecycle history', () => {
  const harness = createInspectionHarness({ autoStart: false, frame: primitiveFrame(7) });
  harness.backend.submission = {
    drawCallCount: null,
    renderHandleCount: undefined,
    geometryResourceCount: 2,
    materialResourceCount: 3,
    textureResourceCount: null,
    animatedInstanceCount: undefined,
    triangleCount: 12,
  };
  harness.surface.renderOnce(25);
  const classified = harness.surface.submission();
  assert.equal(classified.statistics.drawCallCount.status, 'unavailable');
  assert.equal(classified.statistics.renderHandleCount.status, 'unsupported');
  assert.equal(classified.statistics.textureResourceCount.status, 'unavailable');
  assert.equal(classified.statistics.animatedInstanceCount.status, 'unsupported');

  harness.canvas.clientWidth = 900;
  harness.canvas.clientHeight = 500;
  assert.equal(harness.surface.resizeToCanvas().applied, true);
  harness.surface.stop();
  assert.equal(harness.surface.submission(), classified);

  harness.surface.dispose();
  assert.equal(harness.surface.submission(), classified);

  const replacement = createInspectionHarness({ autoStart: false, frame: primitiveFrame(8) });
  assert.equal(replacement.surface.submission().renderSequence, 1);
  assert.notEqual(replacement.surface.submission(), classified);
  replacement.surface.dispose();
});

void test('inspection surface atomically replaces authored content at exact chunk and retained limits', () => {
  const harness = createInspectionHarness({ autoStart: false, frame: primitiveFrame(7) });
  const chunks = authoredHistoryChunks(41, [
    RUSTY_RENDERER_EDITOR_VIEWPORT_MAX_FRAME_OPS,
    RUSTY_RENDERER_EDITOR_VIEWPORT_MAX_FRAME_OPS,
  ]);

  const receipt = harness.surface.replaceAuthoredFrameChunks(chunks);

  assert.equal(receipt.applied, true);
  assert.equal(receipt.channel, 'authored');
  assert.equal(receipt.generation, 2);
  assert.equal(harness.surface.readout().retainedOpCount, RUSTY_RENDERER_EDITOR_VIEWPORT_MAX_RETAINED_OPS);
  assert.equal(harness.backend.frames.get('authored')?.ops.length, RUSTY_RENDERER_EDITOR_VIEWPORT_MAX_RETAINED_OPS);
  assert.equal(harness.surface.readout().runtimeRetainedOpCount, 0);
});

void test('inspection chunk replacement rejects malformed bounds and ordering without changing authored state', () => {
  const harness = createInspectionHarness({ autoStart: false });
  const seeded = harness.surface.replaceFrame(primitiveFrame(7));
  assert.equal(seeded.applied, true);
  const accepted = harness.surface.readout();
  const backendFrame = structuredClone(harness.backend.frames.get('authored'));
  const cases: readonly {
    readonly chunks: readonly RenderFrameDiff[];
    readonly code: string;
    readonly name: string;
  }[] = [
    {
      name: 'malformed chunk list',
      chunks: null as unknown as readonly RenderFrameDiff[],
      code: 'invalid_frame',
    },
    {
      name: 'malformed nested frame',
      chunks: [{ ops: null } as unknown as RenderFrameDiff],
      code: 'invalid_frame',
    },
    {
      name: 'null operation',
      chunks: [{ schemaVersion: 1, ops: [null as never] }],
      code: 'invalid_frame',
    },
    {
      name: 'primitive operation',
      chunks: [{ schemaVersion: 1, ops: [42 as never] }],
      code: 'invalid_frame',
    },
    {
      name: 'malformed object operation',
      chunks: [{ schemaVersion: 1, ops: [{ op: 'create', handle: 8, parent: null, node: null } as never] }],
      code: 'invalid_frame',
    },
    {
      name: 'duplicate handle across chunks',
      chunks: [primitiveFrame(8), primitiveFrame(8)],
      code: 'invalid_frame',
    },
    {
      name: 'update before create',
      chunks: [{ schemaVersion: 1, ops: [updateVisibility(8, false)] }, primitiveFrame(8)],
      code: 'invalid_frame',
    },
    {
      name: 'invalid logical handle',
      chunks: [invalidLogicalHandleFrame()],
      code: 'invalid_handle',
    },
    {
      name: 'over-limit chunk',
      chunks: authoredHistoryChunks(9, [RUSTY_RENDERER_EDITOR_VIEWPORT_MAX_FRAME_OPS + 1]),
      code: 'frame_limit_exceeded',
    },
    {
      name: 'retained total overflow',
      chunks: authoredHistoryChunks(10, [
        RUSTY_RENDERER_EDITOR_VIEWPORT_MAX_FRAME_OPS,
        RUSTY_RENDERER_EDITOR_VIEWPORT_MAX_FRAME_OPS,
        1,
      ]),
      code: 'frame_limit_exceeded',
    },
  ];

  for (const testCase of cases) {
    const receipt = harness.surface.replaceAuthoredFrameChunks(testCase.chunks);
    assert.equal(receipt.applied, false, testCase.name);
    assert.equal(receipt.diagnostics[0]?.code, testCase.code, testCase.name);
    assert.equal(receipt.generation, seeded.generation, testCase.name);
    assertAuthoredReadoutUnchanged(harness.surface.readout(), accepted, testCase.name);
    assert.deepEqual(harness.backend.frames.get('authored'), backendFrame, testCase.name);
  }
});

void test('inspection chunk replacement rolls back when backend realization rejects the candidate', () => {
  const harness = createInspectionHarness({ autoStart: false, frame: primitiveFrame(7) });
  const accepted = harness.surface.readout();
  const backendFrame = structuredClone(harness.backend.frames.get('authored'));
  harness.backend.rejectChannel = 'authored';

  const receipt = harness.surface.replaceAuthoredFrameChunks([
    primitiveFrame(12),
    { schemaVersion: 1, ops: [updateVisibility(12, false)] },
  ]);

  assert.equal(receipt.applied, false);
  assert.equal(receipt.diagnostics[0]?.code, 'backend_rejected');
  assertAuthoredReadoutUnchanged(harness.surface.readout(), accepted, 'backend rejection');
  assert.deepEqual(harness.backend.frames.get('authored'), backendFrame);
});

void test('inspection surface keeps incremental runtime projection distinct from authored replacement', () => {
  const harness = createInspectionHarness({ autoStart: false, frame: primitiveFrame(7) });
  const initial = harness.surface.readout();

  const created = harness.surface.applyRuntimeFrame(primitiveFrame(8));
  assert.equal(created.applied, true);
  assert.equal(created.channel, 'runtime');
  assert.equal(harness.surface.readout().runtimeGeneration, 1);
  assert.equal(harness.surface.readout().runtimeRetainedOpCount, 1);
  assert.equal(harness.surface.readout().retainedFrameHash, initial.retainedFrameHash);
  assert.equal(harness.surface.readout().retainedOpCount, 1);

  const updated = harness.surface.applyRuntimeFrame({
    schemaVersion: 1,
    ops: [{
      op: 'update',
      handle: renderHandle(8),
      transform: {
        translation: [3, 2, 1],
        rotation: [0, 0, 0, 1],
        scale: [1, 1, 1],
      },
      material: null,
      visible: null,
      metadata: null,
    }],
  });
  assert.equal(updated.applied, true);
  assert.equal(harness.surface.readout().runtimeGeneration, 2);
  assert.equal(harness.surface.readout().runtimeRetainedOpCount, 2);

  const deleted = harness.surface.applyRuntimeFrame({
    schemaVersion: 1,
    ops: [{ op: 'destroy', handle: renderHandle(8) }],
  });
  assert.equal(deleted.applied, true);
  assert.equal(harness.surface.readout().runtimeGeneration, 3);
  assert.equal(harness.surface.readout().runtimeRetainedOpCount, 3);

  const cleared = harness.surface.clearRuntimeProjection();
  assert.equal(cleared.applied, true);
  assert.equal(cleared.channel, 'runtime');
  assert.equal(harness.surface.readout().runtimeGeneration, 4);
  assert.equal(harness.surface.readout().runtimeRetainedOpCount, 0);
  assert.equal(harness.surface.readout().retainedFrameHash, initial.retainedFrameHash);
});

void test('inspection surface replaces and clears disposable debug overlays independently', () => {
  const harness = createInspectionHarness({ autoStart: false, frame: primitiveFrame(7) });
  const authored = structuredClone(harness.backend.frames.get('authored'));
  const overlayCreate = primitiveCreate(90);
  const replaced = harness.surface.replaceOverlayFrame({
    schemaVersion: 1,
    ops: [{
      ...overlayCreate,
      node: { ...overlayCreate.node, layer: 'debug' },
    }],
  });

  assert.equal(replaced.applied, true);
  assert.equal(replaced.channel, 'overlay');
  assert.equal(harness.backend.frames.get('overlay')?.ops.length, 1);
  assert.deepEqual(harness.backend.frames.get('authored'), authored);

  const cleared = harness.surface.clearOverlayProjection();
  assert.equal(cleared.applied, true);
  assert.equal(cleared.channel, 'overlay');
  assert.equal(harness.backend.frames.get('overlay')?.ops.length, 0);
  assert.deepEqual(harness.backend.frames.get('authored'), authored);
});

void test('inspection surface rejects malformed and over-limit runtime frames without changing retained runtime state', () => {
  const harness = createInspectionHarness({ autoStart: false });
  assert.equal(harness.surface.applyRuntimeFrame(primitiveFrame(8)).applied, true);
  const accepted = harness.surface.readout();

  const malformed = harness.surface.applyRuntimeFrame({ ops: null } as unknown as RenderFrameDiff);
  assert.equal(malformed.applied, false);
  assert.equal(malformed.diagnostics[0]?.code, 'invalid_frame');
  assertRuntimeReadoutUnchanged(harness.surface.readout(), accepted);

  const repeatedUpdates = Array.from(
    { length: RUSTY_RENDERER_EDITOR_VIEWPORT_MAX_FRAME_OPS + 1 },
    () => ({
      op: 'update' as const,
      handle: renderHandle(8),
      transform: null,
      material: null,
      visible: true,
      metadata: null,
    }),
  );
  const overLimit = harness.surface.applyRuntimeFrame({ schemaVersion: 1, ops: repeatedUpdates });
  assert.equal(overLimit.applied, false);
  assert.equal(overLimit.diagnostics[0]?.code, 'frame_limit_exceeded');
  assertRuntimeReadoutUnchanged(harness.surface.readout(), accepted);
});

void test('inspection input fail-safes clear latched movement and drag before rendering resumes', () => {
  const cases = [
    {
      name: 'window blur',
      interrupt: (harness: ReturnType<typeof createInspectionHarness>) => {
        harness.window.emit('blur', event());
      },
    },
    {
      name: 'document visibility loss',
      interrupt: (harness: ReturnType<typeof createInspectionHarness>) => {
        harness.document.visibilityState = 'hidden';
        harness.document.emit('visibilitychange', event());
      },
    },
    {
      name: 'pointer cancellation',
      interrupt: (harness: ReturnType<typeof createInspectionHarness>) => {
        harness.canvas.emit('pointercancel', pointerEvent({ pointerId: 3 }));
      },
    },
  ] as const;

  for (const lifecycleCase of cases) {
    const harness = createInspectionHarness({ autoStart: false });
    harness.canvas.emit('pointerdown', pointerEvent({ button: 0, clientX: 10, clientY: 10, pointerId: 3 }));
    harness.document.emit('keydown', keyboardEvent('KeyW'));
    assert.equal(harness.surface.readout().dragging, true, lifecycleCase.name);
    assert.deepEqual(harness.surface.readout().pressedMovementKeys, ['KeyW'], lifecycleCase.name);
    const cameraBeforeInterrupt = harness.surface.camera();

    lifecycleCase.interrupt(harness);
    assert.equal(harness.surface.readout().dragging, false, lifecycleCase.name);
    assert.deepEqual(harness.surface.readout().pressedMovementKeys, [], lifecycleCase.name);

    harness.canvas.emit('pointermove', pointerEvent({ buttons: 1, clientX: 80, clientY: 40, pointerId: 3 }));
    harness.surface.renderOnce(1000);
    assert.deepEqual(harness.surface.camera(), cameraBeforeInterrupt, lifecycleCase.name);
    harness.surface.dispose();
  }
});

void test('inspection surface start stop and disposal release animation input renderer and resize resources', () => {
  const harness = createInspectionHarness({ autoStart: true });
  assert.equal(harness.surface.readout().status, 'running');
  assert.equal(harness.animation.pendingCount(), 1);

  harness.animation.runNext(16);
  assert.equal(harness.animation.pendingCount(), 1);
  harness.canvas.emit('pointerdown', pointerEvent({ button: 0, clientX: 10, clientY: 10, pointerId: 9 }));
  harness.document.emit('keydown', keyboardEvent('ArrowUp', 'ArrowUp'));
  harness.surface.stop();
  assert.equal(harness.surface.readout().status, 'stopped');
  assert.equal(harness.animation.pendingCount(), 0);
  assert.equal(harness.surface.readout().dragging, false);
  assert.deepEqual(harness.surface.readout().pressedOrbitKeys, []);

  harness.surface.start();
  assert.equal(harness.surface.readout().status, 'running');
  const cameraBeforeDispose = harness.surface.camera();
  harness.surface.dispose();
  harness.surface.dispose();
  assert.equal(harness.surface.readout().status, 'disposed');
  assert.equal(harness.animation.pendingCount(), 0);
  assert.equal(harness.backend.disposals, 1);
  assert.equal(harness.resizeObserver.disconnects, 1);

  harness.canvas.emit('pointerdown', pointerEvent({ button: 0, clientX: 10, clientY: 10, pointerId: 11 }));
  harness.canvas.emit('pointermove', pointerEvent({ buttons: 1, clientX: 100, clientY: 100, pointerId: 11 }));
  harness.document.emit('keydown', keyboardEvent('KeyW'));
  harness.surface.renderOnce(1000);
  assert.deepEqual(harness.surface.camera(), cameraBeforeDispose);
  const runtimeAfterDispose = harness.surface.applyRuntimeFrame(primitiveFrame(99));
  assert.equal(runtimeAfterDispose.applied, false);
  assert.equal(runtimeAfterDispose.diagnostics[0]?.code, 'viewport_disposed');
  assert.equal(harness.canvas.listenerCount(), 0);
  assert.equal(harness.document.listenerCount(), 0);
  assert.equal(harness.window.listenerCount(), 0);
});

void test('inspection surface rejects a malformed initial frame and disposes the prepared viewport', () => {
  const backend = new FakeEditorViewportBackend();
  const viewport = createRendererEditorViewportWithBackend(backend, { autoStart: false });
  const document = new FakeDocument();
  const canvas = new FakeCanvas(document);
  const animation = new FakeAnimationScheduler();
  const resizeObserver = new FakeResizeObserver();

  assert.throws(
    () => createRendererInspectionSurfaceWithViewport(
      canvas as unknown as HTMLCanvasElement,
      viewport,
      { autoStart: false, frame: { ops: null } as unknown as RenderFrameDiff },
      inspectionEnvironment(animation, resizeObserver),
    ),
    /render frame ops must be an array/,
  );
  assert.equal(backend.disposals, 1);
  assert.equal(resizeObserver.disconnects, 1);
  assert.equal(canvas.listenerCount(), 0);
  assert.equal(document.listenerCount(), 0);
  assert.equal(document.window.listenerCount(), 0);
});

function createInspectionHarness(options: {
  readonly autoStart: boolean;
  readonly controls?: RendererInspectionSurfaceControlsOptions;
  readonly frame?: RenderFrameDiff;
  readonly initialGrid?: EditorGridDescriptor | null;
}) {
  const backend = new FakeEditorViewportBackend();
  const viewport = createRendererEditorViewportWithBackend(backend, { autoStart: false });
  const document = new FakeDocument();
  const canvas = new FakeCanvas(document);
  const animation = new FakeAnimationScheduler();
  const resizeObserver = new FakeResizeObserver();
  const surface = createRendererInspectionSurfaceWithViewport(
    canvas as unknown as HTMLCanvasElement,
    viewport,
    {
      autoStart: options.autoStart,
      ...(options.controls === undefined ? {} : { controls: options.controls }),
      ...(options.frame === undefined ? {} : { frame: options.frame }),
      ...(options.initialGrid === undefined ? {} : { initialGrid: options.initialGrid }),
    },
    inspectionEnvironment(animation, resizeObserver),
  );
  return { animation, backend, canvas, document, resizeObserver, surface, window: document.window };
}

function assertRuntimeReadoutUnchanged(
  actual: ReturnType<ReturnType<typeof createInspectionHarness>['surface']['readout']>,
  expected: ReturnType<ReturnType<typeof createInspectionHarness>['surface']['readout']>,
): void {
  assert.equal(actual.runtimeFrameHash, expected.runtimeFrameHash);
  assert.equal(actual.runtimeGeneration, expected.runtimeGeneration);
  assert.equal(actual.runtimeRetainedOpCount, expected.runtimeRetainedOpCount);
}

function assertAuthoredReadoutUnchanged(
  actual: ReturnType<ReturnType<typeof createInspectionHarness>['surface']['readout']>,
  expected: ReturnType<ReturnType<typeof createInspectionHarness>['surface']['readout']>,
  message: string,
): void {
  assert.equal(actual.retainedFrameHash, expected.retainedFrameHash, message);
  assert.equal(actual.retainedOpCount, expected.retainedOpCount, message);
  assert.equal(actual.viewportHash, expected.viewportHash, message);
}

function assertVectorApproximately(
  actual: readonly number[],
  expected: readonly number[],
  tolerance = 0.000_000_001,
): void {
  assert.equal(actual.length, expected.length);
  for (const [index, coordinate] of actual.entries()) {
    assert.ok(
      Math.abs(coordinate - expected[index]!) <= tolerance,
      `coordinate ${String(index)}: expected ${String(expected[index])}, received ${String(coordinate)}`,
    );
  }
}

function inspectionEnvironment(
  animation: FakeAnimationScheduler,
  resizeObserver: FakeResizeObserver,
) {
  return {
    animation,
    createResizeObserver: () => resizeObserver,
    devicePixelRatio: () => 1.5,
  };
}

class FakeAnimationScheduler {
  readonly #callbacks = new Map<number, (timeMs: number) => void>();
  #nextHandle = 1;

  request(callback: (timeMs: number) => void): number {
    const handle = this.#nextHandle;
    this.#nextHandle += 1;
    this.#callbacks.set(handle, callback);
    return handle;
  }

  cancel(handle: number): void {
    this.#callbacks.delete(handle);
  }

  now(): number {
    return 0;
  }

  pendingCount(): number {
    return this.#callbacks.size;
  }

  runNext(timeMs: number): void {
    const entry = this.#callbacks.entries().next().value as [number, (timeMs: number) => void] | undefined;
    assert.ok(entry);
    this.#callbacks.delete(entry[0]);
    entry[1](timeMs);
  }
}

class FakeResizeObserver {
  disconnects = 0;
  observations = 0;

  observe(): void {
    this.observations += 1;
  }

  disconnect(): void {
    this.disconnects += 1;
  }
}

type FakeListener = EventListenerOrEventListenerObject;

class FakeEventSource {
  readonly #listeners = new Map<string, Set<FakeListener>>();

  addEventListener(type: string, listener: FakeListener | null): void {
    if (listener === null) return;
    const listeners = this.#listeners.get(type) ?? new Set<FakeListener>();
    listeners.add(listener);
    this.#listeners.set(type, listeners);
  }

  removeEventListener(type: string, listener: FakeListener | null): void {
    if (listener === null) return;
    this.#listeners.get(type)?.delete(listener);
  }

  emit(type: string, event: Event): void {
    for (const listener of this.#listeners.get(type) ?? []) {
      if (typeof listener === 'function') listener(event);
      else listener.handleEvent(event);
    }
  }

  listenerCount(): number {
    return [...this.#listeners.values()].reduce((total, listeners) => total + listeners.size, 0);
  }
}

class FakeDocument extends FakeEventSource {
  activeElement: Element | null = null;
  visibilityState: DocumentVisibilityState = 'visible';
  readonly window = new FakeEventSource();

  get defaultView(): Window {
    return this.window as unknown as Window;
  }
}

class FakeCanvas extends FakeEventSource {
  readonly #capturedPointers = new Set<number>();
  clientHeight = 360;
  clientWidth = 640;
  height = 360;
  width = 640;
  tabIndex = -1;
  readonly style = { touchAction: '' };

  constructor(readonly fakeDocument: FakeDocument) {
    super();
  }

  get ownerDocument(): Document {
    return this.fakeDocument as unknown as Document;
  }

  focus(): void {
    this.fakeDocument.activeElement = this as unknown as Element;
  }

  hasPointerCapture(pointerId: number): boolean {
    return this.#capturedPointers.has(pointerId);
  }

  releasePointerCapture(pointerId: number): void {
    this.#capturedPointers.delete(pointerId);
  }

  setPointerCapture(pointerId: number): void {
    this.#capturedPointers.add(pointerId);
  }
}

function pointerEvent(options: {
  readonly button?: number;
  readonly buttons?: number;
  readonly clientX?: number;
  readonly clientY?: number;
  readonly pointerId: number;
}): PointerEvent {
  return {
    button: options.button ?? -1,
    buttons: options.buttons ?? 0,
    clientX: options.clientX ?? 0,
    clientY: options.clientY ?? 0,
    isPrimary: true,
    pointerId: options.pointerId,
    preventDefault: () => undefined,
  } as unknown as PointerEvent;
}

function event(): Event {
  return {} as Event;
}

function wheelEvent(deltaY: number): WheelEvent {
  return {
    deltaY,
    preventDefault: () => undefined,
  } as unknown as WheelEvent;
}

function keyboardEvent(code: string, key = code): KeyboardEvent {
  return {
    code,
    key,
    preventDefault: () => undefined,
  } as unknown as KeyboardEvent;
}

class FakeEditorViewportBackend implements RendererEditorViewportBackendPort {
  readonly frames = new Map<string, RenderFrameDiff>();
  readonly sizes: RendererEditorViewportSize[] = [];
  disposals = 0;
  grid: EditorGridDescriptor | null = null;
  rejectChannel: 'runtime' | 'authored' | 'overlay' | null = null;
  submission: ReturnType<RendererEditorViewportBackendPort['renderOnce']> = {
    drawCallCount: 4,
    renderHandleCount: 3,
    geometryResourceCount: 2,
    materialResourceCount: 2,
    textureResourceCount: 0,
    animatedInstanceCount: 0,
    triangleCount: 24,
  };
  readonly animationSamples: Array<[
    'runtime' | 'authored' | 'overlay',
    number,
    string,
    number,
  ]> = [];
  readonly playbackCommands: Array<[
    'runtime' | 'authored' | 'overlay',
    number,
    Parameters<RendererEditorViewportBackendPort['setAnimatedMeshPlayback']>[2],
  ]> = [];

  replaceChannel(channel: 'runtime' | 'authored' | 'overlay', frame: RenderFrameDiff): void {
    if (this.rejectChannel === channel) {
      throw new Error(`fixture rejected ${channel} replacement`);
    }
    this.frames.set(channel, structuredClone(frame));
  }

  setCamera(): void {}

  setGrid(descriptor: EditorGridDescriptor | null): void {
    this.grid = descriptor === null ? null : structuredClone(descriptor);
  }

  gridReadout(): EditorGridProjectionReadout | null {
    return this.grid === null ? null : {
      descriptor: structuredClone(this.grid),
      bounds: null,
      minorLineStep: this.grid.grid.spacing[0],
      renderedLineCount: 42,
    };
  }

  resize(size: RendererEditorViewportSize): void {
    this.sizes.push(structuredClone(size));
  }

  pick(): ReturnType<RendererEditorViewportBackendPort['pick']> {
    return { diagnostics: [], hit: null };
  }

  renderOnce(): ReturnType<RendererEditorViewportBackendPort['renderOnce']> {
    return this.submission;
  }

  sampleAnimatedMesh(
    channel: 'runtime' | 'authored' | 'overlay',
    handle: Parameters<RendererEditorViewportBackendPort['sampleAnimatedMesh']>[1],
    clipId: string,
    normalizedTime: number,
  ): ReturnType<RendererEditorViewportBackendPort['sampleAnimatedMesh']> {
    this.animationSamples.push([channel, handle, clipId, normalizedTime]);
    return {
      handle,
      asset: 'animated/test',
      contentHash: 'sha256:test',
      clip: clipId,
      normalizedTime,
      durationSeconds: 1,
      assetBounds: { min: [-1, 0, -1], max: [1, 2, 1] },
      sampledWorldBounds: { min: [-1, 0, -1], max: [1, 2, 1] },
      sampledVertexCount: 24,
      boneCount: 12,
      skinningFacts: {
        joints: [],
        skinnedMeshCount: 1,
        inverseBindMatrixCount: 12,
        inverseBindMatricesFinite: true,
        weightedVertexCount: 24,
        invalidWeightVertexCount: 0,
        maximumWeightSumError: 0,
        weightsNormalized: true,
        interpolationModes: ['linear'],
        instanceRootDistinctFromTemplate: true,
        skeletonsIndependentFromTemplate: true,
        sharedGeometryCount: 1,
        sharedMaterialCount: 1,
      },
      diagnostics: [],
    };
  }

  setAnimatedMeshPlayback(
    channel: 'runtime' | 'authored' | 'overlay',
    handle: Parameters<RendererEditorViewportBackendPort['setAnimatedMeshPlayback']>[1],
    playback: Parameters<RendererEditorViewportBackendPort['setAnimatedMeshPlayback']>[2],
  ): void {
    this.playbackCommands.push([channel, handle, playback]);
  }

  start(): void {}

  stop(): void {}

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
      origin: [0, 0, 0],
      spacing: [1, 1, 1],
    },
    plane: 'xz',
    snapAnchor: 'boundary',
    style: {
      minorColor: [0.2, 0.2, 0.2, 0.5],
      majorColor: [0.4, 0.4, 0.4, 0.8],
      xAxisColor: [1, 0, 0, 1],
      yAxisColor: [0, 1, 0, 1],
      zAxisColor: [0, 0, 1, 1],
      majorLineEvery: 10,
      opacity: 0.8,
      fadeStart: 20,
      fadeEnd: 100,
    },
  };
}

function primitiveFrame(handle: number): RenderFrameDiff {
  return {
    schemaVersion: 1,
    ops: [primitiveCreate(handle)],
  };
}

function primitiveCreate(handle: number): Extract<RenderDiff, { readonly op: 'create' }> {
  return {
    op: 'create',
    handle: renderHandle(handle),
    parent: null,
    node: {
      layer: 'scene',
      geometry: { kind: 'cube' },
      transform: {
        translation: [0, 0, 0],
        rotation: [0, 0, 0, 1],
        scale: [1, 1, 1],
      },
      material: { color: [0.3, 0.5, 0.8, 1], wireframe: false },
      visible: true,
      metadata: {
        sourceEntity: null,
        sourceSceneNode: null,
        tags: [],
        label: 'inspection-fixture',
      },
    },
  };
}

function invalidLogicalHandleFrame(): RenderFrameDiff {
  return {
    schemaVersion: 1,
    ops: [{ ...primitiveCreate(8), handle: (Number.MAX_SAFE_INTEGER + 1) as never }],
  };
}

function updateVisibility(handle: number, visible: boolean): RenderDiff {
  return {
    op: 'update',
    handle: renderHandle(handle),
    transform: null,
    material: null,
    visible,
    metadata: null,
  };
}

function authoredHistoryChunks(
  handle: number,
  chunkLengths: readonly number[],
): readonly RenderFrameDiff[] {
  let operationIndex = 0;
  return chunkLengths.map((length) => ({
    schemaVersion: 1,
    ops: Array.from({ length }, () => {
      const currentIndex = operationIndex;
      operationIndex += 1;
      return currentIndex === 0
        ? primitiveCreate(handle)
        : updateVisibility(handle, currentIndex % 2 === 0);
    }),
  }));
}
