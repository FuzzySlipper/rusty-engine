import assert from "node:assert/strict";
import test from "node:test";

import {
  RuntimeProjectionAdapter,
  derivePlayerCameraPose,
  entityHandle,
  type RuntimeBrowserState,
} from "./projection.ts";

function state(projection: RuntimeBrowserState["projection"]): RuntimeBrowserState {
  return {
    tick: 0,
    entityRevision: 0,
    projection,
    doorState: "closed",
    encounterState: "active",
    motionState: "moving",
    navigationState: "following",
    playerMotionState: "idle",
    player: {
      id: 1,
      position: [0.5, 0.5, 0.5],
      yawDegrees: 180,
      pitchDegrees: -10,
      bindings: {
        moveForward: "KeyW",
        moveBackward: "KeyS",
        moveLeft: "KeyA",
        moveRight: "KeyD",
        mouseLook: "pointer",
      },
    },
    enemies: [],
    lastEvents: [],
  };
}

test("whole Rust readouts become create update and destroy diffs", () => {
  const adapter = new RuntimeProjectionAdapter();
  const original = {
    id: 3,
    name: "exit",
    asset: "mesh/security-door",
    translation: [0, 0, 8] as const,
    visible: true,
  };

  const created = adapter.apply(state([original]));
  assert.deepEqual(created.ops.map((op) => op.op), ["create", "create"]);
  assert.equal(created.ops[1]?.op === "create" ? created.ops[1].handle : null, entityHandle(3));

  const updated = adapter.apply(
    state([{ ...original, translation: [0, 3, 8] as const }]),
  );
  assert.deepEqual(updated.ops.map((op) => op.op), ["update"]);

  const destroyed = adapter.apply(state([]));
  assert.deepEqual(destroyed.ops.map((op) => op.op), ["destroy"]);
  assert.equal(adapter.trackedEntityCount, 0);
});

test("camera pose is rebuilt as a presentation offset from accepted player state", () => {
  const player = state([]).player;
  const camera = derivePlayerCameraPose(player);

  assert.ok(Math.abs(camera.position[0] - 0.5) < 0.000_001);
  assert.equal(camera.position[1], 3.2);
  assert.equal(camera.position[2], -5.5);
  assert.equal(camera.yawDegrees, 180);
  assert.equal(camera.pitchDegrees, -10);
  assert.equal("camera" in player, false);
});
