import type { EntityDefinition, ProjectContent } from "./schema.js";

export const ENCOUNTER_IDS = {
  actor: 1,
  encounter: 2,
  exit: 3,
  firstEnemy: 4,
} as const;

export function encounterGateProject(enemyNames: readonly string[]): ProjectContent {
  if (enemyNames.length === 0) {
    throw new Error("an encounter gate requires at least one enemy");
  }
  const normalizedNames = enemyNames.map((name) => name.trim());
  if (normalizedNames.some((name) => name.length === 0)) {
    throw new Error("enemy names must not be empty");
  }

  const enemies: EntityDefinition[] = normalizedNames.map((name, index) => ({
    id: ENCOUNTER_IDS.firstEnemy + index,
    name,
    translation: [index * 2, 0, 4],
    collision: { enabled: true, staticCollider: false },
    renderable: { asset: "mesh/security-sentry", visible: true },
    enemy: true,
  }));
  const members = enemies.map((enemy) => enemy.id);

  return {
    schemaVersion: 1,
    entities: [
      { id: ENCOUNTER_IDS.actor, name: "player" },
      {
        id: ENCOUNTER_IDS.encounter,
        name: "loading-bay-encounter",
        encounter: { members, exit: ENCOUNTER_IDS.exit },
      },
      {
        id: ENCOUNTER_IDS.exit,
        name: "loading-bay-exit",
        translation: [0, 0, 8],
        collision: { enabled: true, staticCollider: true },
        renderable: { asset: "mesh/security-door", visible: true },
        door: { openTranslation: [0, 3, 8], autoCloseAfterTicks: null },
      },
      ...enemies,
    ],
  };
}

export const generatedProjects = {
  "encounter-gate.project.json": encounterGateProject(["sentry-alpha", "sentry-beta"]),
  "encounter-gate-solo.project.json": encounterGateProject(["sentry-alpha"]),
} as const;
