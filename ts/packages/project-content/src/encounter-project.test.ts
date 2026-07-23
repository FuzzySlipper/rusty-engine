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
