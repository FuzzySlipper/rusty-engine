import assert from "node:assert/strict";
import test from "node:test";

import { RuntimeProjectionAdapter, entityHandle, type RuntimeBrowserState } from "./projection.ts";

function state(projection: RuntimeBrowserState["projection"]): RuntimeBrowserState {
  return {
    tick: 0,
    worldRevision: 0,
    projection,
    doorState: "closed",
    encounterState: "active",
    motionState: "moving",
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
