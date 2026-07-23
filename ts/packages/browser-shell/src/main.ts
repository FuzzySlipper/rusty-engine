import { mountAshaRendererBrowserSurface } from "@asha/renderer-three/backend";

import "./style.css";
import {
  RuntimeProjectionAdapter,
  derivePlayerCameraPose,
  type RuntimeBrowserState,
  type RuntimePlayerBindings,
} from "./projection.js";

type ResolvedPlayerAction =
  | { readonly kind: "move"; readonly forward: number; readonly right: number }
  | { readonly kind: "look"; readonly yawDelta: number; readonly pitchDelta: number };

const canvas = requiredElement("viewport", HTMLCanvasElement);
const encounterState = requiredElement("encounter-state", HTMLElement);
const revision = requiredElement("revision", HTMLElement);
const doorCaption = requiredElement("door-caption", HTMLElement);
const enemyList = requiredElement("enemy-list", HTMLElement);
const motionState = requiredElement("motion-state", HTMLElement);
const navigationState = requiredElement("navigation-state", HTMLElement);
const playerMotionState = requiredElement("player-motion-state", HTMLElement);
const playerPose = requiredElement("player-pose", HTMLElement);
const environmentState = requiredElement("environment-state", HTMLElement);
const eventList = requiredElement("event-list", HTMLOListElement);
const rendererStatus = requiredElement("renderer-status", HTMLElement);
const smokeResult = requiredElement("smoke-result", HTMLElement);
const projection = new RuntimeProjectionAdapter();
const eventHistory: string[] = [];
const smokeMode = new URLSearchParams(location.search).has("smoke");
let actionQueue: Promise<void> = Promise.resolve();

let current = await requestState("/api/state");
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
updateRendererStatus();

for (const button of document.querySelectorAll<HTMLButtonElement>("[data-enemy-id]")) {
  button.addEventListener("click", () => {
    const enemy = Number(button.dataset.enemyId);
    void perform(`/api/defeat/${String(enemy)}`);
  });
}
requiredElement("reset", HTMLButtonElement).addEventListener("click", () => {
  void perform("/api/reset");
});
requiredElement("run-motion", HTMLButtonElement).addEventListener("click", () => {
  void perform("/api/motion-phase");
});
requiredElement("run-navigation", HTMLButtonElement).addEventListener("click", () => {
  void perform("/api/navigation-phase");
});

window.addEventListener("keydown", (event) => {
  const action = resolveKeyboardAction(event.code, current.player.bindings);
  if (action === null) {
    return;
  }
  event.preventDefault();
  enqueuePlayerAction(action);
});
canvas.addEventListener("click", () => {
  void canvas.requestPointerLock();
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
  for (let action = 0; action < 12; action += 1) {
    window.dispatchEvent(
      new KeyboardEvent("keydown", { code: current.player.bindings.moveForward }),
    );
  }
  await actionQueue;
  const playerMoved = current.player.position.some(
    (value, axis) => Math.abs(value - initialPlayerPosition[axis]!) > 0.01,
  );
  const playerBlocked = current.playerMotionState === "blocked";
  window.dispatchEvent(new MouseEvent("mousemove", { movementX: 20, movementY: 0 }));
  await actionQueue;
  const playerLooked = current.player.yawDegrees !== initialPlayerYaw;
  await perform("/api/navigation-phase");
  await perform("/api/defeat/4");
  await perform("/api/defeat/5");
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
    playerLooked &&
    current.projection.find((node) => node.id === 4)?.translation?.[0] === 7.5 &&
    (current.projection.find((node) => node.id === 10)?.translation?.[0] ?? -4) > 2 &&
    current.generatedEnvironment?.seed === 4 &&
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
  }
  eventHistory.push(...current.lastEvents);
  const frame = projection.apply(current);
  if (frame.ops.length > 0) {
    surface.applyFrame(frame);
  }
  surface.setCameraPose(derivePlayerCameraPose(current.player));
  surface.renderOnce();
  renderReadout(current);
  updateRendererStatus();
}

function renderReadout(state: RuntimeBrowserState): void {
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
  playerPose.textContent = `${state.player.position.map((value) => value.toFixed(1)).join(", ")} · YAW ${state.player.yawDegrees.toFixed(0)}°`;
  environmentState.textContent = state.generatedEnvironment === null
    ? "STATIC"
    : `SEED ${String(state.generatedEnvironment.seed)} · ${String(state.generatedEnvironment.meshQuads)} QUADS · ${state.generatedEnvironment.outputHash.slice(0, 8)}`;
  enemyList.replaceChildren(
    ...state.enemies.map((enemy) => {
      const row = document.createElement("div");
      row.className = "enemy-row";
      row.dataset.state = enemy.state;
      const name = document.createElement("span");
      name.textContent = enemy.name;
      const status = document.createElement("strong");
      status.textContent = enemy.state.toUpperCase();
      row.append(name, status);
      return row;
    }),
  );
  eventList.replaceChildren(
    ...(eventHistory.length === 0
      ? ["Awaiting action"]
      : eventHistory
    ).map((event) => {
        const item = document.createElement("li");
        item.textContent = event;
        return item;
      }),
  );
}

function enqueuePlayerAction(action: ResolvedPlayerAction): void {
  actionQueue = actionQueue.then(() => performPlayerAction(action));
}

async function performPlayerAction(action: ResolvedPlayerAction): Promise<void> {
  current = await requestState("/api/player-action", "POST", action);
  eventHistory.push(...current.lastEvents);
  const frame = projection.apply(current);
  if (frame.ops.length > 0) {
    surface.applyFrame(frame);
  }
  surface.setCameraPose(derivePlayerCameraPose(current.player));
  surface.renderOnce();
  renderReadout(current);
  updateRendererStatus();
}

function updateRendererStatus(): void {
  rendererStatus.textContent = `${surface.kind} · ${String(projection.trackedEntityCount)} entities · ${String(projection.trackedMeshCount)} voxel meshes`;
}

export function resolveKeyboardAction(
  code: string,
  bindings: RuntimePlayerBindings,
): ResolvedPlayerAction | null {
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
  return null;
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
    yawDelta: Math.max(-1, Math.min(1, movementX / 20)),
    pitchDelta: Math.max(-1, Math.min(1, movementY / 20)),
  };
}

async function requestState(
  path: string,
  method = "GET",
  body?: ResolvedPlayerAction,
): Promise<RuntimeBrowserState> {
  const response = await fetch(path, {
    method,
    ...(body === undefined
      ? {}
      : { body: JSON.stringify(body), headers: { "Content-Type": "application/json" } }),
  });
  if (!response.ok) {
    throw new Error(`${method} ${path} failed with ${String(response.status)}`);
  }
  return (await response.json()) as RuntimeBrowserState;
}

function requiredElement<T extends Element>(id: string, constructor: { new (): T }): T {
  const element = document.getElementById(id);
  if (!(element instanceof constructor)) {
    throw new Error(`missing required element #${id}`);
  }
  return element;
}
