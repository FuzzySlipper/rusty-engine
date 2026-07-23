import type {
  HostEntityView,
  JsonValue,
  ProjectDecision,
  ProjectDecisionBatch,
  ProjectFact,
  ProjectInvocation,
  ProjectInvocationWave,
  ProjectScheduleRequest,
  ProjectWorldCommand,
} from "./types.js";

export interface SecurityDoorState {
  readonly [key: string]: JsonValue;
  readonly doorState: "closed" | "open";
  readonly closedTranslation: readonly [number, number, number];
  readonly openTranslation: readonly [number, number, number];
  readonly autoCloseTicks: number | null;
}

export function securityDoorState(autoCloseTicks: number | null): SecurityDoorState {
  return {
    doorState: "closed",
    closedTranslation: [0, 0, 0],
    openTranslation: [0, 3, 0],
    autoCloseTicks,
  };
}

export function decideSecurityDoorWave(wave: ProjectInvocationWave): ProjectDecisionBatch {
  return {
    schemaVersion: 1,
    expectedWorldRevision: wave.expectedWorldRevision,
    decisions: wave.invocations.map(decideInvocation),
  };
}

function decideInvocation(invocation: ProjectInvocation): ProjectDecision {
  if (invocation.behavior.behaviorType !== "securityDoorController") {
    throw new Error(`unsupported behavior ${invocation.behavior.behaviorType}`);
  }
  const door = requiredDoor(invocation.related);
  let state = decodeSecurityDoorState(invocation.state.payload);
  const commands: ProjectWorldCommand[] = [];
  const schedules: ProjectScheduleRequest[] = [];
  const facts: ProjectFact[] = [];
  let changed = false;

  for (const event of invocation.events) {
    if (event.kind === "interaction" && state.doorState === "closed") {
      commands.push(
        { op: "setTranslation", entity: door.entity, translation: state.openTranslation },
        { op: "setCollisionEnabled", entity: door.entity, enabled: false },
      );
      if (state.autoCloseTicks !== null) {
        schedules.push({
          op: "upsert",
          messageId: "close",
          dueAfterTicks: state.autoCloseTicks,
          messageKind: "close",
          payload: {},
        });
      }
      facts.push({ kind: "door.opened", version: 1, payload: { door: door.entity } });
      state = { ...state, doorState: "open" };
      changed = true;
      continue;
    }
    if (event.kind === "message" && event.messageKind === "close" && state.doorState === "open") {
      commands.push(
        { op: "setCollisionEnabled", entity: door.entity, enabled: true },
        { op: "setTranslation", entity: door.entity, translation: state.closedTranslation },
      );
      facts.push({ kind: "door.closed", version: 1, payload: { door: door.entity } });
      state = { ...state, doorState: "closed" };
      changed = true;
    }
  }

  return {
    invocationId: invocation.invocationId,
    commands,
    stateUpdate: changed
      ? {
          expectedRevision: invocation.state.revision,
          version: invocation.state.version,
          payload: state,
        }
      : null,
    schedules,
    facts,
  };
}

function requiredDoor(related: readonly HostEntityView[]): HostEntityView {
  const door = related.find((entity) => entity.collision !== null && entity.translation !== null);
  if (door === undefined) {
    throw new Error("security door behavior requires one spatial collision entity");
  }
  return door;
}

export function decodeSecurityDoorState(value: JsonValue): SecurityDoorState {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error("security door state must be an object");
  }
  const record = value as Record<string, JsonValue>;
  const doorState = record["doorState"];
  const closedTranslation = tuple3(record["closedTranslation"], "closedTranslation");
  const openTranslation = tuple3(record["openTranslation"], "openTranslation");
  const autoCloseTicks = record["autoCloseTicks"];
  if (doorState !== "closed" && doorState !== "open") {
    throw new Error("security door state has an invalid doorState");
  }
  if (
    autoCloseTicks !== null &&
    (typeof autoCloseTicks !== "number" || !Number.isInteger(autoCloseTicks) || autoCloseTicks <= 0)
  ) {
    throw new Error("autoCloseTicks must be a positive integer or null");
  }
  return {
    doorState,
    closedTranslation,
    openTranslation,
    autoCloseTicks,
  };
}

function tuple3(value: JsonValue | undefined, field: string): readonly [number, number, number] {
  if (
    !Array.isArray(value) ||
    value.length !== 3 ||
    value.some((component) => typeof component !== "number" || !Number.isFinite(component))
  ) {
    throw new Error(`${field} must be a finite three-number tuple`);
  }
  return [value[0] as number, value[1] as number, value[2] as number];
}
