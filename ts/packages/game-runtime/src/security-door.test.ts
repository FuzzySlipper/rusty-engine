import assert from "node:assert/strict";
import test from "node:test";

import { ProjectDoorRuntime } from "./runtime.js";
import {
  decideSecurityDoorWave,
  decodeSecurityDoorState,
  securityDoorState,
} from "./security-door.js";

test("trusted TypeScript opens and closes through one batch per invocation wave", () => {
  const runtime = ProjectDoorRuntime.create(securityDoorState(3));
  try {
    const interaction = runtime.beginInteraction();
    assert.equal(interaction.invocations.length, 1);
    assert.equal(interaction.invocations[0]?.events[0]?.kind, "interaction");

    const opened = runtime.apply(decideSecurityDoorWave(interaction));
    assert.equal(opened.revisionAfter, 1);
    assert.equal(opened.engineFacts.length, 2);
    assert.equal(opened.projectFacts[0]?.kind, "door.opened");
    assert.equal(decodeSecurityDoorState(opened.stateRecords[0]!.payload).doorState, "open");
    assert.equal(opened.pendingMessageCount, 1);
    assert.deepEqual(opened.projection[0]?.translation, [0, 3, 0]);

    assert.equal(runtime.advanceBy(2), null);
    const closeWave = runtime.advanceBy(1);
    assert.ok(closeWave);
    assert.equal(closeWave.invocations[0]?.events[0]?.kind, "message");
    const closed = runtime.apply(decideSecurityDoorWave(closeWave));
    assert.equal(closed.revisionAfter, 2);
    assert.equal(closed.projectFacts[0]?.kind, "door.closed");
    assert.equal(decodeSecurityDoorState(closed.stateRecords[0]!.payload).doorState, "closed");
    assert.equal(closed.pendingMessageCount, 0);
    assert.deepEqual(closed.projection[0]?.translation, [0, 0, 0]);

    const stats = runtime.bridgeStats();
    assert.equal(stats.gameplayCalls, 5);
    assert.ok(stats.bytesIn > 0);
    assert.ok(stats.bytesOut > 0);
  } finally {
    assert.equal(runtime.close(), true);
  }
});

test("latched variation changes only project state", () => {
  const runtime = ProjectDoorRuntime.create(securityDoorState(null));
  try {
    const interaction = runtime.beginInteraction();
    const opened = runtime.apply(decideSecurityDoorWave(interaction));
    assert.equal(decodeSecurityDoorState(opened.stateRecords[0]!.payload).doorState, "open");
    assert.equal(opened.pendingMessageCount, 0);
    assert.equal(runtime.advanceBy(20), null);
    assert.equal(
      decodeSecurityDoorState(runtime.readout().stateRecords[0]!.payload).doorState,
      "open",
    );
  } finally {
    runtime.close();
  }
});

test("save and reopen preserve project state and stable scheduled message", () => {
  const runtime = ProjectDoorRuntime.create(securityDoorState(5));
  const ids = runtime.ids;
  const interaction = runtime.beginInteraction();
  runtime.apply(decideSecurityDoorWave(interaction));
  assert.equal(runtime.advanceBy(2), null);
  const snapshot = runtime.save();
  runtime.close();

  const restored = ProjectDoorRuntime.restore(snapshot, ids);
  try {
    const readout = restored.readout();
    assert.equal(readout.tick, 2);
    assert.equal(readout.pendingMessageCount, 1);
    assert.equal(decodeSecurityDoorState(readout.stateRecords[0]!.payload).doorState, "open");
    const closeWave = restored.advanceBy(3);
    assert.ok(closeWave);
    const closed = restored.apply(decideSecurityDoorWave(closeWave));
    assert.equal(decodeSecurityDoorState(closed.stateRecords[0]!.payload).doorState, "closed");
  } finally {
    restored.close();
  }
});
