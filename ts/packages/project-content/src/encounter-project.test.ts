import assert from "node:assert/strict";
import test from "node:test";

import { ENCOUNTER_IDS, encounterGateProject } from "./encounter-project.js";

test("encounter membership and exit relationships are explicit authored content", () => {
  const project = encounterGateProject(["alpha", "beta"]);
  const encounter = project.entities.find((entity) => entity.id === ENCOUNTER_IDS.encounter);
  assert.deepEqual(encounter?.encounter, {
    members: [ENCOUNTER_IDS.firstEnemy, ENCOUNTER_IDS.firstEnemy + 1],
    exit: ENCOUNTER_IDS.exit,
  });
});

test("enemy count is a content-only variation", () => {
  const project = encounterGateProject(["only-enemy"]);
  assert.equal(project.entities.filter((entity) => entity.enemy === true).length, 1);
  assert.deepEqual(
    project.entities.find((entity) => entity.id === ENCOUNTER_IDS.encounter)?.encounter?.members,
    [ENCOUNTER_IDS.firstEnemy],
  );
});

test("loading bay composes a visible kinematic probe over authored voxel collision", () => {
  const project = encounterGateProject(["only-enemy"]);
  const probe = project.entities.find((entity) => entity.id === ENCOUNTER_IDS.motionProbe);

  assert.deepEqual(probe?.kinematic, {
    halfExtents: [0.25, 0.25, 0.25],
    velocity: [5, 0, 0],
  });
  assert.deepEqual(project.voxelCollision?.solidVoxels, [
    [3, 0, 4],
    [3, 0, 6],
  ]);
});

test("autonomous navigation is explicit data on the responsible enemy", () => {
  const project = encounterGateProject(["pathfinder", "guard"]);
  const navigator = project.entities.find((entity) => entity.id === ENCOUNTER_IDS.firstEnemy);

  assert.deepEqual(navigator?.navigation, {
    goal: [6.5, 0.5, 4.5],
    speedUnitsPerSecond: 4,
    maxVisited: 512,
  });
  assert.deepEqual(navigator?.kinematic?.velocity, [0, 0, 0]);
});

test("navigation target and speed are content-only variations", () => {
  const project = encounterGateProject(["pathfinder"], {
    navigationGoal: [1.5, 0.5, 7.5],
    navigationSpeedUnitsPerSecond: 2,
  });
  const navigator = project.entities.find((entity) => entity.id === ENCOUNTER_IDS.firstEnemy);

  assert.deepEqual(navigator?.navigation, {
    goal: [1.5, 0.5, 7.5],
    speedUnitsPerSecond: 2,
    maxVisited: 512,
  });
});
