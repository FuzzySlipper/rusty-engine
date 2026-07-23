import type {
  EntityDefinition,
  PlayerInputBindingsDefinition,
  ProjectContent,
  Vec3,
} from "./schema.js";

export const ENCOUNTER_IDS = {
  actor: 1,
  encounter: 2,
  exit: 3,
  firstEnemy: 4,
  motionProbe: 10,
} as const;

export interface EncounterProjectOptions {
  readonly navigationGoal?: Vec3;
  readonly navigationSpeedUnitsPerSecond?: number;
  readonly playerBindings?: PlayerInputBindingsDefinition;
  readonly generationSeed?: number;
}

export function encounterGateProject(
  enemyNames: readonly string[],
  options: EncounterProjectOptions = {},
): ProjectContent {
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
    translation: index === 0 ? [1.5, 1.5, 6.5] : [6.5, 1.5, 2.5],
    collision: { enabled: true, staticCollider: false },
    renderable: { asset: "mesh/security-sentry", visible: true },
    enemy: true,
    ...(index === 0
      ? {
          kinematic: { halfExtents: [0.25, 0.25, 0.25], velocity: [0, 0, 0] },
          navigation: {
            goal: options.navigationGoal ?? [7.5, 1.5, 6.5],
            speedUnitsPerSecond: options.navigationSpeedUnitsPerSecond ?? 4,
            maxVisited: 512,
          },
        }
      : {}),
  }));
  const members = enemies.map((enemy) => enemy.id);

  return {
    schemaVersion: 5,
    entities: [
      {
        id: ENCOUNTER_IDS.actor,
        name: "player",
        translation: [1.5, 1.5, 2.5],
        collision: { enabled: true, staticCollider: false },
        renderable: { asset: "primitive/player-marker", visible: true },
        kinematic: { halfExtents: [0.25, 0.25, 0.25], velocity: [0, 0, 0] },
        playerController: {
          moveSpeedUnitsPerSecond: 4,
          moveStepSeconds: 0.1,
          lookDegreesPerUnit: 12,
          initialYawDegrees: 0,
          initialPitchDegrees: -10,
          bindings: options.playerBindings ?? {
            moveForward: "KeyW",
            moveBackward: "KeyS",
            moveLeft: "KeyA",
            moveRight: "KeyD",
            mouseLook: "pointer",
          },
        },
      },
      {
        id: ENCOUNTER_IDS.encounter,
        name: "loading-bay-encounter",
        encounter: { members, exit: ENCOUNTER_IDS.exit },
      },
      {
        id: ENCOUNTER_IDS.exit,
        name: "loading-bay-exit",
        translation: [4.5, 1, 11],
        collision: { enabled: true, staticCollider: true },
        renderable: { asset: "mesh/security-door", visible: true },
        door: { openTranslation: [4.5, 4, 11], autoCloseAfterTicks: null },
      },
      ...enemies,
      {
        id: ENCOUNTER_IDS.motionProbe,
        name: "spatial-probe",
        translation: [1.5, 1.5, 8.5],
        renderable: { asset: "primitive/spatial-probe", visible: true },
        kinematic: { halfExtents: [0.25, 0.25, 0.25], velocity: [5, 0, 0] },
      },
    ],
    generatedVoxelEnvironment: {
      seed: options.generationSeed ?? 4,
      voxelSize: 1,
      chunkSize: 16,
      width: 7,
      height: 4,
      length: 10,
    },
  };
}

export const generatedEncounterProjects = {
  "encounter-gate.project.json": encounterGateProject(["sentry-alpha", "sentry-beta"]),
  "encounter-gate-solo.project.json": encounterGateProject(["sentry-alpha"]),
} as const;
