import { mountAshaRendererBrowserSurface } from "@asha/renderer-three/backend";

import "./style.css";
import { SerializedActionQueue } from "./action-queue.js";
import { HeldMovementInput } from "./held-movement.js";
import {
  BrowserPresentationFeedbackSink,
  PresentationFeedbackAdapter,
} from "./presentation-feedback.js";
import {
  RuntimeProjectionAdapter,
  derivePlayerCameraPose,
  type RuntimeBrowserState,
  type RuntimePlayerBindings,
} from "./projection.js";

type ResolvedPlayerAction =
  | { readonly kind: "move"; readonly forward: number; readonly right: number }
  | { readonly kind: "look"; readonly yawDelta: number; readonly pitchDelta: number };
type ResolvedAttackAction = { readonly kind: "attack" };
type ResolvedInputAction = ResolvedPlayerAction | ResolvedAttackAction;

const canvas = requiredElement("viewport", HTMLCanvasElement);
const encounterState = requiredElement("encounter-state", HTMLElement);
const revision = requiredElement("revision", HTMLElement);
const doorCaption = requiredElement("door-caption", HTMLElement);
const enemyList = requiredElement("enemy-list", HTMLElement);
const motionState = requiredElement("motion-state", HTMLElement);
const navigationState = requiredElement("navigation-state", HTMLElement);
const playerMotionState = requiredElement("player-motion-state", HTMLElement);
const combatState = requiredElement("combat-state", HTMLElement);
const playerPose = requiredElement("player-pose", HTMLElement);
const weaponState = requiredElement("weapon-state", HTMLElement);
const environmentState = requiredElement("environment-state", HTMLElement);
const eventList = requiredElement("event-list", HTMLOListElement);
const rendererStatus = requiredElement("renderer-status", HTMLElement);
const smokeResult = requiredElement("smoke-result", HTMLElement);
const feedbackLayer = requiredElement("feedback-layer", HTMLElement);
const feedbackAudioStatus = requiredElement("feedback-audio-status", HTMLElement);
const projection = new RuntimeProjectionAdapter();
const presentationFeedback = new PresentationFeedbackAdapter(
  new BrowserPresentationFeedbackSink(feedbackLayer, feedbackAudioStatus),
);
const eventHistory: string[] = [];
const smokeMode = new URLSearchParams(location.search).has("smoke");
let actionRejectionCount = 0;
let lastActionRejection: string | null = null;
const actionQueue = new SerializedActionQueue(recordActionRejection);

let current = await requestState("/api/state");
const heldMovement = new HeldMovementInput({
  bindings: () => current.player.bindings,
  intervalMilliseconds: () => current.player.moveStepSeconds * 1_000,
  dispatch: enqueuePlayerAction,
});
const surface = mountAshaRendererBrowserSurface(canvas, {
  autoStart: true,
  camera: {
    initialPose: derivePlayerCameraPose(current.player),
    projection: { fovYDegrees: 50, near: 0.1, far: 100 },
  },
  clearColor: 0x071012,
  frame: projection.apply(current),
  pixelRatio: Math.min(globalThis.devicePixelRatio ?? 1, 2),
});
renderReadout(current);
applyPresentationFeedback(true);
updateRendererStatus();

requiredElement("primary-fire", HTMLButtonElement).addEventListener("click", () => {
  void presentationFeedback.activateAudio();
  enqueueAttackAction({ kind: "attack" });
});
requiredElement("reset", HTMLButtonElement).addEventListener("click", () => {
  void presentationFeedback.activateAudio();
  heldMovement.clear();
  void perform("/api/reset");
});
requiredElement("run-motion", HTMLButtonElement).addEventListener("click", () => {
  void presentationFeedback.activateAudio();
  void perform("/api/motion-phase");
});
requiredElement("run-navigation", HTMLButtonElement).addEventListener("click", () => {
  void presentationFeedback.activateAudio();
  void perform("/api/navigation-phase");
});

window.addEventListener("keydown", (event) => {
  const action = resolveKeyboardAction(event.code, current.player.bindings);
  if (action === null) {
    return;
  }
  void presentationFeedback.activateAudio();
  event.preventDefault();
  if (action.kind === "move") {
    heldMovement.press(event.code);
  } else if (!event.repeat) {
    enqueueResolvedAction(action);
  }
});
window.addEventListener("keyup", (event) => {
  if (heldMovement.release(event.code)) {
    event.preventDefault();
  }
});
window.addEventListener("blur", () => {
  heldMovement.clear();
});
document.addEventListener("visibilitychange", () => {
  if (document.hidden) {
    heldMovement.clear();
  }
});
canvas.addEventListener("click", () => {
  void presentationFeedback.activateAudio();
  void canvas.requestPointerLock();
});
canvas.addEventListener("mousedown", (event) => {
  if (document.pointerLockElement !== canvas) {
    return;
  }
  const action = resolvePointerButtonAction(event.button, current.player.bindings);
  if (action !== null) {
    void presentationFeedback.activateAudio();
    event.preventDefault();
    enqueueAttackAction(action);
  }
});
window.addEventListener("mousemove", (event) => {
  if (!smokeMode && document.pointerLockElement !== canvas) {
    return;
  }
  const action = resolvePointerAction(
    event.movementX,
    event.movementY,
    current.player.bindings,
  );
  if (action !== null) {
    enqueuePlayerAction(action);
  }
});

if (smokeMode) {
  await perform("/api/reset");
  const initialPlayerPosition = current.player.position;
  const initialPlayerYaw = current.player.yawDegrees;
  const heldCode = current.player.bindings.moveForward;
  window.dispatchEvent(new KeyboardEvent("keydown", { code: heldCode }));
  await delay(current.player.moveStepSeconds * 8_000);
  window.dispatchEvent(new KeyboardEvent("keyup", { code: heldCode }));
  await actionQueue.settled();
  const playerMoved = current.player.position.some(
    (value, axis) => Math.abs(value - initialPlayerPosition[axis]!) > 0.01,
  );
  const playerBlocked = current.playerMotionState === "blocked";
  const releasedPlayerPosition = current.player.position;
  await delay(current.player.moveStepSeconds * 2_000);
  await actionQueue.settled();
  const playerStopped = current.player.position.every(
    (value, axis) => Math.abs(value - releasedPlayerPosition[axis]!) < 0.000_001,
  );
  document.body.dataset.heldInput = playerMoved && playerBlocked && playerStopped
    ? "pass"
    : "fail";
  window.dispatchEvent(new MouseEvent("mousemove", { movementX: 20, movementY: 0 }));
  await actionQueue.settled();
  const playerLooked = current.player.yawDegrees !== initialPlayerYaw;
  const initialEnemyPosition = current.enemies.find((enemy) => enemy.id === 4)?.position;
  await perform("/api/navigation-step");
  const movingEnemyPosition = current.enemies.find((enemy) => enemy.id === 4)?.position;
  const movingTargetAdvanced = initialEnemyPosition !== undefined && movingEnemyPosition !== undefined
    && movingEnemyPosition.some(
      (value, axis) => Math.abs(value - initialEnemyPosition[axis]!) > 0.001,
    );
  await aimAtEnemy(4);
  await firePrimary();
  const movingTargetDamaged = current.enemies.find((enemy) => enemy.id === 4)?.currentHealth === 40;
  const rejectionsBeforeCooldown = actionRejectionCount;
  await firePrimary();
  const cooldownRejected =
    actionRejectionCount === rejectionsBeforeCooldown + 1 &&
    current.enemies.find((enemy) => enemy.id === 4)?.currentHealth === 40;
  const yawBeforeRecovery = current.player.yawDegrees;
  await enqueuePlayerAction({ kind: "look", yawDelta: 0.25, pitchDelta: 0 });
  const lookRecoveredAfterRejection = current.player.yawDegrees !== yawBeforeRecovery;
  await perform("/api/navigation-phase");
  await aimAtEnemy(4);
  await firePrimary();
  await aimAtEnemy(5);
  await firePrimary();
  await enqueuePlayerAction({ kind: "look", yawDelta: 0.25, pitchDelta: 0 });
  await aimAtEnemy(5);
  await firePrimary();
  const combatHit = current.combatState === "hit";
  const openGateTraversed = await walkPlayerPath([
    [1.5, 9.5],
    [4.5, 9.5],
    [4.5, 12.5],
  ]);
  if (openGateTraversed) {
    await turnPlayerToward(
      4.5 - current.player.position[0],
      10.5 - current.player.position[2],
    );
  }
  document.body.dataset.gatePassage = openGateTraversed ? "pass" : "fail";
  const queueRecovered = cooldownRejected && lookRecoveredAfterRejection && openGateTraversed;
  document.body.dataset.queueRecovery = queueRecovered ? "pass" : "fail";
  const cooldownRecovered =
    cooldownRejected && current.enemies.find((enemy) => enemy.id === 4)?.currentHealth === 0;
  document.body.dataset.cooldown = cooldownRecovered ? "pass" : "fail";
  await perform("/api/motion-phase");
  surface.renderOnce();
  const door = current.projection.find((node) => node.id === 3);
  const passed =
    current.encounterState === "cleared" &&
    current.doorState === "open" &&
    door?.translation?.[1] === 4 &&
    current.enemies.every((enemy) => enemy.state === "defeated") &&
    current.motionState === "blocked" &&
    current.navigationState === "arrived" &&
    playerMoved &&
    playerBlocked &&
    playerStopped &&
    playerLooked &&
    movingTargetAdvanced &&
    movingTargetDamaged &&
    queueRecovered &&
    cooldownRecovered &&
    current.projection.find((node) => node.id === 4)?.translation?.[0] === 7.5 &&
    (current.projection.find((node) => node.id === 10)?.translation?.[0] ?? -4) > 2 &&
    current.generatedEnvironment?.seed === 4 &&
    combatHit &&
    openGateTraversed &&
    current.enemies.every((enemy) => enemy.currentHealth === 0) &&
    eventHistory.includes("CombatHit") &&
    eventHistory.includes("DamageApplied") &&
    current.voxelMeshes.length === 1 &&
    surface.snapshot().includes("loading-bay-exit") &&
    surface.snapshot().includes("generated-room-chunk");
  smokeResult.dataset.status = passed ? "pass" : "fail";
  smokeResult.textContent = passed
    ? "PASS · Rust facts reached retained WebGL projection"
    : "FAIL · Product proof did not converge";
  document.body.dataset.smokeStatus = passed ? "pass" : "fail";
}

async function perform(path: string): Promise<void> {
  current = await requestState(path, "POST");
  if (path === "/api/reset") {
    eventHistory.length = 0;
    actionRejectionCount = 0;
    lastActionRejection = null;
  }
  eventHistory.push(...current.lastEvents);
  const frame = projection.apply(current);
  if (frame.ops.length > 0) {
    surface.applyFrame(frame);
  }
  surface.setCameraPose(derivePlayerCameraPose(current.player));
  surface.renderOnce();
  renderReadout(current);
  applyPresentationFeedback(path === "/api/reset");
  updateRendererStatus();
}

function renderReadout(state: RuntimeBrowserState): void {
  eventList.dataset.history = eventHistory.join(",");
  encounterState.textContent = state.encounterState.toUpperCase();
  encounterState.dataset.state = state.encounterState;
  revision.textContent = `REV ${String(state.entityRevision)}`;
  doorCaption.textContent = state.doorState === "open" ? "OPEN" : "LOCKED";
  doorCaption.dataset.state = state.doorState;
  motionState.textContent = state.motionState.toUpperCase();
  motionState.dataset.state = state.motionState;
  navigationState.textContent = state.navigationState.toUpperCase();
  navigationState.dataset.state = state.navigationState;
  playerMotionState.textContent = state.playerMotionState.toUpperCase();
  playerMotionState.dataset.state = state.playerMotionState;
  combatState.textContent = lastActionRejection === null
    ? state.combatState.toUpperCase()
    : "REJECTED";
  combatState.dataset.state = lastActionRejection === null ? state.combatState : "rejected";
  combatState.title = lastActionRejection ?? "";
  playerPose.textContent = `${state.player.position.map((value) => value.toFixed(1)).join(", ")} · YAW ${state.player.yawDegrees.toFixed(0)}°`;
  weaponState.textContent = `${String(state.weapon.damage)} DMG · ${String(state.weapon.ammoRemaining)}/${String(state.weapon.ammoCapacity)} AMMO`;
  environmentState.textContent = state.generatedEnvironment === null
    ? "STATIC"
    : `SEED ${String(state.generatedEnvironment.seed)} · ${String(state.generatedEnvironment.meshQuads)} QUADS · ${state.generatedEnvironment.outputHash.slice(0, 8)}`;
  enemyList.replaceChildren(
    ...state.enemies.map((enemy) => {
      const row = document.createElement("div");
      row.className = "enemy-row";
      row.dataset.entityId = String(enemy.id);
      row.dataset.state = enemy.state;
      const name = document.createElement("span");
      name.textContent = enemy.name;
      const status = document.createElement("strong");
      status.textContent = `${enemy.state.toUpperCase()} · ${String(enemy.currentHealth)}/${String(enemy.maxHealth)} HP`;
      row.append(name, status);
      return row;
    }),
  );
  eventList.replaceChildren(
    ...(eventHistory.length === 0
      ? ["Awaiting action"]
      : eventHistory.slice(-20)
    ).map((event) => {
        const item = document.createElement("li");
        item.textContent = event;
        return item;
      }),
  );
}

function applyPresentationFeedback(reset = false): void {
  doorCaption.dataset.entityId = "3";
  playerMotionState.dataset.entityId = String(current.player.id);
  const receipt = presentationFeedback.apply(current, reset);
  feedbackLayer.dataset.lastCueCount = String(receipt.cueCount);
  feedbackLayer.dataset.failedOperations = String(receipt.failedOperations);
  feedbackLayer.dataset.scheduledSounds = String(receipt.scheduledSounds);
}

function enqueuePlayerAction(action: ResolvedPlayerAction): Promise<void> {
  return actionQueue.enqueue(() => performPlayerAction(action));
}

function enqueueAttackAction(action: ResolvedAttackAction): Promise<void> {
  return actionQueue.enqueue(() => performAttackAction(action));
}

function enqueueResolvedAction(action: ResolvedInputAction): void {
  if (action.kind === "attack") {
    enqueueAttackAction(action);
  } else {
    enqueuePlayerAction(action);
  }
}

async function performPlayerAction(action: ResolvedPlayerAction): Promise<void> {
  current = await requestState("/api/player-action", "POST", action);
  lastActionRejection = null;
  eventHistory.push(...current.lastEvents);
  const frame = projection.apply(current);
  if (frame.ops.length > 0) {
    surface.applyFrame(frame);
  }
  surface.setCameraPose(derivePlayerCameraPose(current.player));
  surface.renderOnce();
  renderReadout(current);
  applyPresentationFeedback();
  updateRendererStatus();
}

async function performAttackAction(action: ResolvedAttackAction): Promise<void> {
  current = await requestState("/api/attack", "POST", action);
  lastActionRejection = null;
  eventHistory.push(...current.lastEvents);
  const frame = projection.apply(current);
  if (frame.ops.length > 0) {
    surface.applyFrame(frame);
  }
  surface.setCameraPose(derivePlayerCameraPose(current.player));
  surface.renderOnce();
  renderReadout(current);
  applyPresentationFeedback();
  updateRendererStatus();
}

async function aimAtEnemy(enemyId: number): Promise<void> {
  const enemy = current.enemies.find((candidate) => candidate.id === enemyId);
  if (enemy === undefined) {
    throw new Error(`enemy ${String(enemyId)} is absent`);
  }
  const offset = enemy.position.map(
    (value, axis) => value - current.player.position[axis]!,
  ) as [number, number, number];
  const horizontal = Math.hypot(offset[0], offset[2]);
  const desiredYaw = normalizeDegrees((Math.atan2(-offset[0], -offset[2]) * 180) / Math.PI);
  const desiredPitch = (Math.atan2(offset[1], horizontal) * 180) / Math.PI;
  for (let step = 0; step < 40; step += 1) {
    const yawDifference = normalizeDegrees(desiredYaw - current.player.yawDegrees);
    const pitchDifference = desiredPitch - current.player.pitchDegrees;
    if (Math.abs(yawDifference) < 0.01 && Math.abs(pitchDifference) < 0.01) {
      return;
    }
    await enqueuePlayerAction({
      kind: "look",
      yawDelta: clampUnit(yawDifference / current.player.lookDegreesPerUnit),
      pitchDelta: clampUnit(pitchDifference / current.player.lookDegreesPerUnit),
    });
  }
  throw new Error(`could not aim at enemy ${String(enemyId)}`);
}

async function firePrimary(): Promise<void> {
  const action = resolvePointerButtonAction(0, current.player.bindings);
  if (action === null) {
    throw new Error("authored primary-fire binding did not resolve Mouse0");
  }
  await enqueueAttackAction(action);
}

async function walkPlayerPath(
  waypoints: readonly (readonly [number, number])[],
): Promise<boolean> {
  for (const waypoint of waypoints) {
    if (!await walkPlayerTo(waypoint)) {
      return false;
    }
  }
  return true;
}

async function walkPlayerTo(
  target: readonly [number, number],
  maxSteps = 64,
): Promise<boolean> {
  for (let step = 0; step < maxSteps; step += 1) {
    const offsetX = target[0] - current.player.position[0];
    const offsetZ = target[1] - current.player.position[2];
    if (Math.hypot(offsetX, offsetZ) <= 0.25) {
      return true;
    }
    await turnPlayerToward(offsetX, offsetZ);
    const action = resolveKeyboardAction(
      current.player.bindings.moveForward,
      current.player.bindings,
    );
    if (action?.kind !== "move") {
      throw new Error("authored move-forward binding did not resolve to movement");
    }
    const before = current.player.position;
    await enqueuePlayerAction(action);
    if (current.player.position.every(
      (value, axis) => Math.abs(value - before[axis]!) < 0.000_001,
    )) {
      return false;
    }
  }
  return false;
}

async function turnPlayerToward(offsetX: number, offsetZ: number): Promise<void> {
  const desiredYaw = normalizeDegrees((Math.atan2(-offsetX, -offsetZ) * 180) / Math.PI);
  for (let step = 0; step < 20; step += 1) {
    const yawDifference = normalizeDegrees(desiredYaw - current.player.yawDegrees);
    if (Math.abs(yawDifference) < 0.01) {
      return;
    }
    await enqueuePlayerAction({
      kind: "look",
      yawDelta: clampUnit(yawDifference / current.player.lookDegreesPerUnit),
      pitchDelta: 0,
    });
  }
  throw new Error("could not orient player toward gate waypoint");
}

function updateRendererStatus(): void {
  rendererStatus.textContent = `${surface.kind} · ${String(projection.trackedEntityCount)} entities · ${String(projection.trackedMeshCount)} voxel meshes`;
}

export function resolveKeyboardAction(
  code: string,
  bindings: RuntimePlayerBindings,
): ResolvedInputAction | null {
  if (code === bindings.moveForward) {
    return { kind: "move", forward: 1, right: 0 };
  }
  if (code === bindings.moveBackward) {
    return { kind: "move", forward: -1, right: 0 };
  }
  if (code === bindings.moveLeft) {
    return { kind: "move", forward: 0, right: -1 };
  }
  if (code === bindings.moveRight) {
    return { kind: "move", forward: 0, right: 1 };
  }
  if (code === bindings.primaryFire) {
    return { kind: "attack" };
  }
  return null;
}

export function resolvePointerButtonAction(
  button: number,
  bindings: RuntimePlayerBindings,
): ResolvedAttackAction | null {
  return bindings.primaryFire === `Mouse${String(button)}` ? { kind: "attack" } : null;
}

export function resolvePointerAction(
  movementX: number,
  movementY: number,
  bindings: RuntimePlayerBindings,
): ResolvedPlayerAction | null {
  if (bindings.mouseLook !== "pointer" || (movementX === 0 && movementY === 0)) {
    return null;
  }
  return {
    kind: "look",
    yawDelta: clampUnit(movementX / 20),
    pitchDelta: clampUnit(movementY / 20),
  };
}

async function requestState(
  path: string,
  method = "GET",
  body?: ResolvedInputAction,
): Promise<RuntimeBrowserState> {
  const response = await fetch(path, {
    method,
    ...(body === undefined
      ? {}
      : { body: JSON.stringify(body), headers: { "Content-Type": "application/json" } }),
  });
  if (!response.ok) {
    const detail = await response.json().catch(() => null) as { readonly error?: unknown } | null;
    const reason = typeof detail?.error === "string" ? `: ${detail.error}` : "";
    throw new Error(`${method} ${path} failed with ${String(response.status)}${reason}`);
  }
  return (await response.json()) as RuntimeBrowserState;
}

function recordActionRejection(error: unknown): void {
  actionRejectionCount += 1;
  lastActionRejection = error instanceof Error ? error.message : String(error);
  eventHistory.push(
    lastActionRejection.includes("CombatRejected") ? "CombatRejected" : "ActionRejected",
  );
  renderReadout(current);
}

function clampUnit(value: number): number {
  return Math.max(-1, Math.min(1, value));
}

function normalizeDegrees(value: number): number {
  return ((value + 180) % 360 + 360) % 360 - 180;
}

function delay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => globalThis.setTimeout(resolve, milliseconds));
}

function requiredElement<T extends Element>(id: string, constructor: { new (): T }): T {
  const element = document.getElementById(id);
  if (!(element instanceof constructor)) {
    throw new Error(`missing required element #${id}`);
  }
  return element;
}
